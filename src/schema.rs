use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::config::is_valid_format_key;

pub const DEFAULT_SCHEMA_PATH: &str = "schema/config-schema.json";
pub const DEFAULT_REFERENCE_PATH: &str = "docs/config-reference.kdl";

#[derive(Debug, Deserialize)]
pub struct ConfigSchema {
    pub version: u32,
    pub title: String,
    pub groups: Vec<SchemaGroup>,
}

#[derive(Debug, Deserialize)]
pub struct SchemaGroup {
    pub id: String,
    pub title: String,
    pub entries: Vec<SchemaEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchemaEntry {
    Key(KeyEntry),
    Pattern(PatternEntry),
}

#[derive(Debug, Deserialize)]
pub struct KeyEntry {
    pub key: String,
    pub title: String,
    pub value_type: ValueType,
    #[serde(default)]
    pub allowed_values: Vec<String>,
    pub default: Option<String>,
    pub example_value: String,
    pub description: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub placeholders: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatternEntry {
    pub pattern: String,
    pub pattern_rule: PatternRule,
    pub title: String,
    pub value_type: ValueType,
    pub description: String,
    #[serde(default)]
    pub allowed_values: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub placeholders: Vec<String>,
    #[serde(default)]
    pub examples: Vec<SchemaExample>,
}

#[derive(Debug, Deserialize)]
pub struct SchemaExample {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    String,
    Integer,
    BoolString,
    Enum,
    FormatString,
    Timezone,
    FormatPrecedence,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PatternRule {
    ColorAlias,
    FormatSection,
    ModeFormat,
    CommandWidgetProperty,
    PipeWidgetFormat,
    ScopedCapStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub key: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

impl ConfigSchema {
    pub fn default_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_SCHEMA_PATH)
    }

    pub fn default_reference_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_REFERENCE_PATH)
    }

    pub fn load_default() -> Result<Self> {
        Self::load_from_path(Self::default_path())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read schema file {}", path.display()))?;
        let schema = serde_json::from_str::<Self>(&contents)
            .with_context(|| format!("failed to parse schema file {}", path.display()))?;
        schema.validate_definition()?;
        Ok(schema)
    }

    pub fn fixed_keys(&self) -> BTreeSet<&str> {
        self.groups
            .iter()
            .flat_map(|group| group.entries.iter())
            .filter_map(|entry| match entry {
                SchemaEntry::Key(entry) => Some(entry.key.as_str()),
                SchemaEntry::Pattern(_) => None,
            })
            .collect()
    }

    pub fn pattern_rules(&self) -> BTreeSet<PatternRule> {
        self.groups
            .iter()
            .flat_map(|group| group.entries.iter())
            .filter_map(|entry| match entry {
                SchemaEntry::Pattern(entry) => Some(entry.pattern_rule),
                SchemaEntry::Key(_) => None,
            })
            .collect()
    }

    pub fn render_reference(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("// {}\n", self.title));
        out.push_str(&format!(
            "// Generated from schema/config-schema.json (v{}). Do not edit directly.\n\n",
            self.version
        ));

        for (group_index, group) in self.groups.iter().enumerate() {
            out.push_str(&format!("// --- {} ---\n", group.title));
            for entry in &group.entries {
                match entry {
                    SchemaEntry::Key(entry) => render_key_entry(&mut out, entry),
                    SchemaEntry::Pattern(entry) => render_pattern_entry(&mut out, entry),
                }
                out.push('\n');
            }

            if group_index + 1 != self.groups.len() {
                out.push('\n');
            }
        }

        out
    }

    pub fn validate_config(&self, config: &BTreeMap<String, String>) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (key, value) in config {
            let matching_entry = self.find_entry(key);
            let Some(entry) = matching_entry else {
                errors.push(ValidationError {
                    key: key.clone(),
                    message: "key is not documented in schema/config-schema.json".to_string(),
                });
                continue;
            };

            if let Err(message) = entry.validate_value(key, value) {
                errors.push(ValidationError {
                    key: key.clone(),
                    message,
                });
            }
        }

        errors
    }

    fn find_entry<'a>(&'a self, key: &str) -> Option<EntryRef<'a>> {
        for group in &self.groups {
            for entry in &group.entries {
                match entry {
                    SchemaEntry::Key(entry) if entry.key == key => {
                        return Some(EntryRef::Key(entry))
                    }
                    SchemaEntry::Pattern(entry) if entry.pattern_rule.matches(key) => {
                        return Some(EntryRef::Pattern(entry));
                    }
                    _ => {}
                }
            }
        }

        None
    }

    fn validate_definition(&self) -> Result<()> {
        if self.groups.is_empty() {
            bail!("schema must define at least one group");
        }

        let mut seen_fixed = BTreeSet::new();
        let mut seen_patterns = BTreeSet::new();

        for group in &self.groups {
            if group.entries.is_empty() {
                bail!(
                    "schema group '{}' must contain at least one entry",
                    group.id
                );
            }

            for entry in &group.entries {
                match entry {
                    SchemaEntry::Key(entry) => {
                        if !seen_fixed.insert(entry.key.clone()) {
                            bail!("duplicate fixed schema key '{}'", entry.key);
                        }
                        validate_shared_metadata(
                            &entry.title,
                            &entry.description,
                            &entry.example_value,
                            &entry.allowed_values,
                            entry.value_type,
                        )?;
                    }
                    SchemaEntry::Pattern(entry) => {
                        if !seen_patterns.insert(entry.pattern_rule) {
                            bail!(
                                "duplicate pattern rule '{:?}' in schema",
                                entry.pattern_rule
                            );
                        }
                        if entry.examples.is_empty() {
                            bail!(
                                "pattern '{}' must define at least one example",
                                entry.pattern
                            );
                        }
                        validate_shared_metadata(
                            &entry.title,
                            &entry.description,
                            &entry.examples[0].value,
                            &entry.allowed_values,
                            entry.value_type,
                        )?;
                        for example in &entry.examples {
                            if !entry.pattern_rule.matches(&example.key) {
                                bail!(
                                    "pattern '{}' example key '{}' does not match rule {:?}",
                                    entry.pattern,
                                    example.key,
                                    entry.pattern_rule
                                );
                            }
                            entry.validate_value(&example.key, &example.value).map_err(
                                |message| {
                                    anyhow::anyhow!(
                                        "pattern '{}' example '{}' invalid: {}",
                                        entry.pattern,
                                        example.key,
                                        message
                                    )
                                },
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

enum EntryRef<'a> {
    Key(&'a KeyEntry),
    Pattern(&'a PatternEntry),
}

impl EntryRef<'_> {
    fn validate_value(&self, key: &str, value: &str) -> Result<(), String> {
        match self {
            Self::Key(entry) => entry.validate_value(value),
            Self::Pattern(entry) => entry.validate_value(key, value),
        }
    }
}

impl KeyEntry {
    fn validate_value(&self, value: &str) -> Result<(), String> {
        validate_value_type(self.value_type, &self.allowed_values, value)
    }
}

impl PatternEntry {
    fn validate_value(&self, key: &str, value: &str) -> Result<(), String> {
        match self.pattern_rule {
            PatternRule::CommandWidgetProperty => validate_command_widget_value(key, value),
            _ => validate_value_type(self.value_type, &self.allowed_values, value),
        }
    }
}

impl PatternRule {
    fn matches(self, key: &str) -> bool {
        match self {
            Self::ColorAlias => key
                .strip_prefix("color_")
                .is_some_and(|suffix| !suffix.is_empty()),
            Self::FormatSection => is_valid_format_key(key),
            Self::ModeFormat => key
                .strip_prefix("mode_")
                .is_some_and(|suffix| !suffix.is_empty()),
            Self::CommandWidgetProperty => [
                "_command",
                "_format",
                "_interval",
                "_rendermode",
                "_clickaction",
                "_cwd",
            ]
            .iter()
            .any(|suffix| {
                key.strip_suffix(suffix).is_some_and(|prefix| {
                    prefix
                        .strip_prefix("command_")
                        .is_some_and(|name| !name.is_empty())
                })
            }),
            Self::PipeWidgetFormat => key.strip_suffix("_format").is_some_and(|prefix| {
                prefix
                    .strip_prefix("pipe_")
                    .is_some_and(|name| !name.is_empty())
            }),
            Self::ScopedCapStyle => ["_cap_bg", "_cap_fg", "_cap_symbol"].iter().any(|suffix| {
                key.strip_suffix(suffix)
                    .is_some_and(|prefix| !prefix.is_empty())
            }),
        }
    }
}

fn validate_shared_metadata(
    title: &str,
    description: &str,
    example_value: &str,
    allowed_values: &[String],
    value_type: ValueType,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("schema entry title must not be empty");
    }
    if description.trim().is_empty() {
        bail!("schema entry description must not be empty");
    }
    validate_value_type(value_type, allowed_values, example_value).map_err(anyhow::Error::msg)?;
    if value_type == ValueType::Enum && allowed_values.is_empty() {
        bail!("enum schema entries must define allowed_values");
    }
    Ok(())
}

fn validate_value_type(
    value_type: ValueType,
    allowed_values: &[String],
    value: &str,
) -> Result<(), String> {
    match value_type {
        ValueType::String | ValueType::FormatString => Ok(()),
        ValueType::Integer => value
            .parse::<u64>()
            .map(|_| ())
            .map_err(|_| format!("expected non-negative integer, got '{value}'")),
        ValueType::BoolString => {
            if value == "true" || value == "false" {
                Ok(())
            } else {
                Err(format!("expected 'true' or 'false', got '{value}'"))
            }
        }
        ValueType::Enum => {
            if allowed_values.iter().any(|candidate| candidate == value) {
                Ok(())
            } else {
                Err(format!(
                    "expected one of {}, got '{value}'",
                    allowed_values.join(", ")
                ))
            }
        }
        ValueType::Timezone => value
            .parse::<chrono_tz::Tz>()
            .map(|_| ())
            .map_err(|_| format!("expected valid IANA timezone, got '{value}'")),
        ValueType::FormatPrecedence => {
            let chars: Vec<char> = value.chars().collect();
            if chars.len() != 3 {
                return Err(format!(
                    "expected a 3-digit precedence order like '132', got '{value}'"
                ));
            }
            let distinct: BTreeSet<char> = chars.iter().copied().collect();
            if distinct != BTreeSet::from(['1', '2', '3']) {
                return Err(format!(
                    "expected digits 1, 2, and 3 exactly once, got '{value}'"
                ));
            }
            Ok(())
        }
    }
}

fn validate_command_widget_value(key: &str, value: &str) -> Result<(), String> {
    if key.ends_with("_interval") {
        return validate_value_type(ValueType::Integer, &[], value);
    }

    if key.ends_with("_rendermode") {
        return validate_value_type(
            ValueType::Enum,
            &[
                "static".to_string(),
                "dynamic".to_string(),
                "raw".to_string(),
            ],
            value,
        );
    }

    Ok(())
}

fn render_key_entry(out: &mut String, entry: &KeyEntry) {
    out.push_str(&format!("// {}\n", entry.title));
    write_comment_block(out, entry);
    out.push_str(&format!(
        "{} {}\n",
        entry.key,
        render_value(entry.value_type, &entry.example_value)
    ));
}

fn render_pattern_entry(out: &mut String, entry: &PatternEntry) {
    out.push_str(&format!("// {}\n", entry.title));
    out.push_str(&format!("// Pattern: {}\n", entry.pattern));
    write_comment_block(out, entry);
    for example in &entry.examples {
        out.push_str(&format!(
            "{} {}\n",
            example.key,
            render_value(entry.value_type, &example.value)
        ));
    }
}

trait CommentMetadata {
    fn description(&self) -> &str;
    fn default_value(&self) -> Option<&str>;
    fn allowed_values(&self) -> &[String];
    fn placeholders(&self) -> &[String];
    fn notes(&self) -> &[String];
}

impl CommentMetadata for KeyEntry {
    fn description(&self) -> &str {
        &self.description
    }

    fn default_value(&self) -> Option<&str> {
        self.default.as_deref()
    }

    fn allowed_values(&self) -> &[String] {
        &self.allowed_values
    }

    fn placeholders(&self) -> &[String] {
        &self.placeholders
    }

    fn notes(&self) -> &[String] {
        &self.notes
    }
}

impl CommentMetadata for PatternEntry {
    fn description(&self) -> &str {
        &self.description
    }

    fn default_value(&self) -> Option<&str> {
        None
    }

    fn allowed_values(&self) -> &[String] {
        &self.allowed_values
    }

    fn placeholders(&self) -> &[String] {
        &self.placeholders
    }

    fn notes(&self) -> &[String] {
        &self.notes
    }
}

fn write_comment_block(out: &mut String, metadata: &impl CommentMetadata) {
    out.push_str(&format!("// {}\n", metadata.description()));
    if !metadata.allowed_values().is_empty() {
        out.push_str(&format!(
            "// Allowed: {}\n",
            metadata.allowed_values().join(" | ")
        ));
    }
    if let Some(default) = metadata.default_value() {
        if default.is_empty() {
            out.push_str("// Default:\n");
        } else {
            out.push_str(&format!("// Default: {}\n", default));
        }
    }
    if !metadata.placeholders().is_empty() {
        out.push_str(&format!(
            "// Placeholders: {}\n",
            metadata.placeholders().join(", ")
        ));
    }
    for note in metadata.notes() {
        out.push_str(&format!("// Note: {}\n", note));
    }
}

fn render_value(value_type: ValueType, value: &str) -> String {
    match value_type {
        ValueType::Integer => value.to_string(),
        _ => quote_kdl_string(value),
    }
}

fn quote_kdl_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing KDL string")
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    fn example_layout_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        for entry in fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")).unwrap()
        {
            let entry = entry.unwrap();
            if !entry.file_type().unwrap().is_dir() {
                continue;
            }

            let layout_path = entry.path().join("layout.kdl");
            if layout_path.exists() {
                paths.push(layout_path);
            }
        }

        paths.sort();
        paths
    }

    fn plugin_config_from_layout(path: &Path) -> BTreeMap<String, String> {
        let contents = fs::read_to_string(path).unwrap();
        let document: kdl::KdlDocument = contents.parse().unwrap();
        let plugin_children = find_plugin_children(&document)
            .unwrap_or_else(|| panic!("did not find plugin block in {}", path.display()));

        let mut config = BTreeMap::new();
        for node in plugin_children.nodes() {
            let key = node.name().value().to_string();
            let value = node
                .get(0)
                .and_then(|entry| kdl_value_to_string(entry.value()))
                .unwrap_or_else(|| panic!("unsupported value for '{}' in {}", key, path.display()));
            config.insert(key, value);
        }

        config
    }

    fn find_plugin_children(document: &kdl::KdlDocument) -> Option<&kdl::KdlDocument> {
        for node in document.nodes() {
            if node.name().value() == "plugin" {
                return node.children();
            }

            if let Some(children) = node.children()
                && let Some(found) = find_plugin_children(children)
            {
                return Some(found);
            }
        }

        None
    }

    fn kdl_value_to_string(value: &kdl::KdlValue) -> Option<String> {
        match value {
            kdl::KdlValue::RawString(text) | kdl::KdlValue::String(text) => Some(text.clone()),
            kdl::KdlValue::Base2(number)
            | kdl::KdlValue::Base8(number)
            | kdl::KdlValue::Base10(number)
            | kdl::KdlValue::Base16(number) => Some(number.to_string()),
            kdl::KdlValue::Base10Float(number) => Some(number.to_string()),
            kdl::KdlValue::Bool(value) => Some(value.to_string()),
            kdl::KdlValue::Null => None,
        }
    }

    #[test]
    fn schema_covers_expected_fixed_keys() {
        let schema = ConfigSchema::load_default().unwrap();

        let expected = BTreeSet::from([
            "layout_mode",
            "format_space",
            "format_precedence",
            "format_hide_on_overlength",
            "tab_normal",
            "tab_active",
            "tab_normal_fullscreen",
            "tab_active_fullscreen",
            "tab_normal_sync",
            "tab_active_sync",
            "tab_rename",
            "tab_separator",
            "tab_overflow_above",
            "tab_overflow_below",
            "tab_max_name_length",
            "tab_padding_top",
            "tab_border",
            "tab_start_index",
            "tab_indicator_sync",
            "tab_indicator_fullscreen",
            "tab_indicator_floating",
            "notification_enabled",
            "notification_format",
            "notification_show_if_empty",
            "notification_format_tab",
            "notification_format_waiting",
            "notification_format_in_progress",
            "notification_format_completed",
            "notification_tab_style",
            "notification_tab_style_waiting",
            "notification_tab_style_in_progress",
            "notification_tab_style_completed",
            "notification_tab_style_apply_to_active",
            "notification_indicator_waiting",
            "notification_indicator_in_progress",
            "notification_indicator_completed",
            "datetime_format",
            "datetime_timezone",
            "swap_layout_format",
            "swap_layout_hide_if_empty",
            "cap_bg",
            "cap_fg",
            "cap_symbol",
        ]);

        assert_eq!(schema.fixed_keys(), expected);
    }

    #[test]
    fn schema_covers_expected_pattern_families() {
        let schema = ConfigSchema::load_default().unwrap();

        let expected = BTreeSet::from([
            PatternRule::ColorAlias,
            PatternRule::FormatSection,
            PatternRule::ModeFormat,
            PatternRule::CommandWidgetProperty,
            PatternRule::PipeWidgetFormat,
            PatternRule::ScopedCapStyle,
        ]);

        assert_eq!(schema.pattern_rules(), expected);
    }

    #[test]
    fn generated_reference_is_in_sync_with_schema() {
        let schema = ConfigSchema::load_default().unwrap();
        let rendered = schema.render_reference();
        let checked_in = fs::read_to_string(ConfigSchema::default_reference_path()).unwrap();

        assert_eq!(rendered, checked_in);
    }

    #[test]
    fn example_layouts_validate_against_schema() {
        let schema = ConfigSchema::load_default().unwrap();

        for path in example_layout_paths() {
            let config = plugin_config_from_layout(&path);
            let errors = schema.validate_config(&config);
            assert!(
                errors.is_empty(),
                "schema validation failed for {}:\n{}",
                path.display(),
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}
