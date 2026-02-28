use std::collections::BTreeMap;
use std::sync::Arc;

use crate::config::SectionZone;
use crate::render::format::parse_format_string;
use crate::widgets::{PluginState, Widget};

/// Expand `{widget_name}` placeholders in a format string by calling widgets.
///
/// Each widget's `process()` returns a pre-rendered string (may contain ANSI).
/// Placeholders that don't match a registered widget are left as-is.
pub fn expand_widgets(
    format_str: &str,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
) -> String {
    let mut result = format_str.to_string();
    for (name, widget) in widgets {
        let placeholder = format!("{{{name}}}");
        if result.contains(&placeholder) {
            let value = widget.process(name, state);
            result = result.replace(&placeholder, &value);
        }
    }
    result
}

/// Render a format section: expand widgets, parse format string, render to ANSI.
///
/// Used by both horizontal and vertical renderers.
pub fn render_section(
    format_str: &str,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    aliases: &BTreeMap<String, String>,
) -> String {
    if format_str.is_empty() {
        return String::new();
    }
    let expanded = expand_widgets(format_str, widgets, state);
    let parts = parse_format_string(&expanded, aliases);
    parts.iter().map(|p| p.render_content()).collect()
}

/// Render a horizontal status bar to stdout.
///
/// Layout: `[left][spacer][center][spacer][right]`
///
/// - `format_<n>_start|left|top` → left zone
/// - `format_<n>_middle|center` → center zone
/// - `format_<n>_end|right|bottom` → right zone
/// - `format_space` → style for spacer fill
/// - `format_precedence` → priority order for zone overlength hiding (e.g., "132")
/// - `format_hide_on_overlength` → whether to hide low-priority sections
pub fn render_bar(
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    _rows: usize,
    cols: usize,
) {
    let config = state.config;
    let aliases = &config.color_aliases;

    // Render all sections into zone buckets (start/middle/end).
    let mut zone_chunks = [Vec::new(), Vec::new(), Vec::new()];
    for section in &config.sections {
        let rendered = render_section(&section.format, widgets, state, aliases);
        if rendered.is_empty() {
            continue;
        }
        zone_chunks[section.zone.precedence_index()].push(rendered);
    }

    let mut sections = [
        zone_chunks[SectionZone::Start.precedence_index()].join(""),
        zone_chunks[SectionZone::Middle.precedence_index()].join(""),
        zone_chunks[SectionZone::End.precedence_index()].join(""),
    ];
    let mut widths: Vec<usize> = sections.iter().map(|s| strip_ansi_width(s)).collect();

    // Overlength trimming: hide sections by reverse precedence until they fit.
    if config.hide_on_overlength {
        let total: usize = widths.iter().sum();
        if total > cols {
            let hide_order = reverse_precedence(&config.format_precedence);
            let mut remaining_total = total;
            for &idx in &hide_order {
                if remaining_total <= cols {
                    break;
                }
                remaining_total -= widths[idx];
                sections[idx] = String::new();
                widths[idx] = 0;
            }
        }
    }

    // Calculate spacer widths for left/center/right layout.
    let total_content: usize = widths.iter().sum();
    let total_space = cols.saturating_sub(total_content);

    // Center the middle section: split space evenly between two gaps.
    let left_gap = total_space / 2;
    let right_gap = total_space.saturating_sub(left_gap);

    // Render spacer segments
    let left_spacer = render_spacer(&config.format_space, left_gap, aliases);
    let right_spacer = render_spacer(&config.format_space, right_gap, aliases);

    // Output the bar (single line, no trailing newline).
    print!(
        "{}{}{}{}{}\x1b[0m",
        sections[0], left_spacer, sections[1], right_spacer, sections[2]
    );
}

/// Render N spaces styled according to `format_space`.
fn render_spacer(format_space: &str, width: usize, aliases: &BTreeMap<String, String>) -> String {
    if width == 0 {
        return String::new();
    }

    if format_space.is_empty() {
        return " ".repeat(width);
    }

    // Parse the format_space to extract style, then render spaces with it.
    let parts = parse_format_string(format_space, aliases);
    if let Some(part) = parts.first() {
        // Use the part's style to wrap the spacer characters.
        part.render(&" ".repeat(width))
    } else {
        " ".repeat(width)
    }
}

/// Parse `format_precedence` (e.g., "132") into a hide order (lowest priority first).
///
/// Returns 0-based zone indices in reverse priority order (the zone to hide first
/// is last in precedence).
fn reverse_precedence(precedence: &str) -> Vec<usize> {
    // Parse each char as a 1-based section index, convert to 0-based.
    let order: Vec<usize> = precedence
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| (d as usize).saturating_sub(1)))
        .filter(|&idx| idx < 3)
        .collect();

    // Reverse: the last in precedence string = lowest priority = hide first.
    order.into_iter().rev().collect()
}

/// Measure visible display width of a string, ignoring ANSI escape sequences.
pub fn strip_ansi_width(s: &str) -> usize {
    crate::widgets::tabs::strip_ansi_width(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_widgets_replaces_placeholders() {
        // We can't easily create a full Widget + PluginState in a unit test,
        // so we test the helper functions instead.
    }

    #[test]
    fn reverse_precedence_default() {
        // "123" → hide order: 3, 2, 1 (section 3 hidden first)
        let order = reverse_precedence("123");
        assert_eq!(order, vec![2, 1, 0]);
    }

    #[test]
    fn reverse_precedence_custom() {
        // "132" → hide order: 2, 3, 1 (section 2 hidden first)
        let order = reverse_precedence("132");
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn reverse_precedence_ignores_invalid() {
        let order = reverse_precedence("1x3");
        assert_eq!(order, vec![2, 0]);
    }

    #[test]
    fn render_spacer_empty_format() {
        let result = render_spacer("", 5, &BTreeMap::new());
        assert_eq!(result, "     ");
    }

    #[test]
    fn render_spacer_zero_width() {
        let result = render_spacer("#[bg=red]", 0, &BTreeMap::new());
        assert_eq!(result, "");
    }

    #[test]
    fn render_spacer_with_style() {
        let result = render_spacer("#[bg=red]", 3, &BTreeMap::new());
        // Should contain ANSI codes and 3 spaces
        assert!(result.contains("   "));
        assert!(result.contains('\x1b'));
    }

    #[test]
    fn strip_ansi_width_plain_text() {
        assert_eq!(strip_ansi_width("hello"), 5);
    }

    #[test]
    fn strip_ansi_width_with_codes() {
        assert_eq!(strip_ansi_width("\x1b[31mhello\x1b[0m"), 5);
    }
}
