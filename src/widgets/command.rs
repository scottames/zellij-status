use std::collections::BTreeMap;

#[cfg(not(test))]
use zellij_tile::shim::run_command;

use super::{PluginState, Widget};

/// Cached result from an external command execution.
#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    /// Process exit code (None if the command hasn't completed).
    pub exit_code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Round-tripped context from `run_command()` (contains "name" and "timestamp").
    pub context: BTreeMap<String, String>,
}

/// How command output is rendered into the format string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RenderMode {
    /// Apply the widget's format string with `{stdout}`/`{stderr}`/`{exit_code}`.
    #[default]
    Static,
    /// Return raw stdout as-is (may contain `#[style]` directives that the
    /// upstream format parser will handle).
    Dynamic,
    /// Return raw stdout with no processing.
    Raw,
}

/// Configuration for a single command widget instance.
#[cfg_attr(test, allow(dead_code))]
struct CommandConfig {
    /// The shell command to execute (parsed into args by [`parse_commandline`]).
    command: String,
    /// Format string with `{stdout}`, `{stderr}`, `{exit_code}` placeholders.
    format: String,
    /// Seconds between re-executions (0 = run once).
    interval: u64,
    /// How output is rendered.
    render_mode: RenderMode,
    /// Optional command to run on click.
    click_action: Option<String>,
    /// Optional working directory for the command.
    cwd: Option<String>,
}

/// Runs external commands and displays their output.
///
/// Multiple command widgets can be configured. Each runs its command
/// asynchronously via `run_command()` and caches the result.
///
/// Config keys (one set per command widget):
/// - `command_NAME_command` — the command to run (required)
/// - `command_NAME_format` — format string with `{stdout}`, `{stderr}`,
///   `{exit_code}` placeholders (default: `"{stdout}"`)
/// - `command_NAME_interval` — seconds between re-runs, 0 = once (default: `"0"`)
/// - `command_NAME_rendermode` — `"static"`, `"dynamic"`, or `"raw"`
///   (default: `"static"`)
/// - `command_NAME_clickaction` — command to fire on click (optional)
/// - `command_NAME_cwd` — working directory (optional)
pub struct CommandWidget {
    configs: BTreeMap<String, CommandConfig>,
}

impl CommandWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut configs = BTreeMap::new();

        // Find all command widgets by looking for _command suffix keys.
        for (key, value) in config {
            if let Some(name) = key.strip_suffix("_command") {
                if !name.starts_with("command_") {
                    continue;
                }

                let format = config
                    .get(&format!("{name}_format"))
                    .cloned()
                    .unwrap_or_else(|| "{stdout}".to_string());

                let interval = config
                    .get(&format!("{name}_interval"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                let render_mode = config
                    .get(&format!("{name}_rendermode"))
                    .map(|v| match v.to_lowercase().as_str() {
                        "dynamic" => RenderMode::Dynamic,
                        "raw" => RenderMode::Raw,
                        _ => RenderMode::Static,
                    })
                    .unwrap_or_default();

                let click_action = config.get(&format!("{name}_clickaction")).cloned();
                let cwd = config.get(&format!("{name}_cwd")).cloned();

                configs.insert(
                    name.to_string(),
                    CommandConfig {
                        command: value.clone(),
                        format,
                        interval,
                        render_mode,
                        click_action,
                        cwd,
                    },
                );
            }
        }

        Self { configs }
    }

    /// Returns all widget names this widget handles (e.g., `["command_git"]`).
    pub fn names(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }

    /// Execute the command if the interval has elapsed since the last run.
    ///
    /// Calls `run_command()` which is async — the result arrives later via
    /// `RunCommandResult` event and is stored in `PluginState.command_results`.
    #[cfg(not(test))]
    fn run_if_needed(&self, name: &str, cmd_config: &CommandConfig, state: &PluginState<'_>) {
        let now = chrono::Utc::now().timestamp();

        if let Some(result) = state.command_results.get(name) {
            if cmd_config.interval == 0 {
                return; // Run-once: already have a result
            }

            let last_run = result
                .context
                .get("timestamp")
                .and_then(|t| t.parse::<i64>().ok())
                .unwrap_or(0);

            if now - last_run < cmd_config.interval as i64 {
                return; // Interval hasn't elapsed yet
            }
        }

        let args = parse_commandline(&cmd_config.command);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let mut context = BTreeMap::new();
        context.insert("name".to_string(), name.to_string());
        context.insert("timestamp".to_string(), now.to_string());

        if let Some(cwd) = &cmd_config.cwd {
            use std::path::PathBuf;
            use zellij_tile::shim::run_command_with_env_variables_and_cwd;
            run_command_with_env_variables_and_cwd(
                &arg_refs,
                BTreeMap::new(),
                PathBuf::from(cwd),
                context,
            );
        } else {
            run_command(&arg_refs, context);
        }
    }
}

impl Widget for CommandWidget {
    fn process(&self, name: &str, state: &PluginState<'_>) -> String {
        let Some(cmd_config) = self.configs.get(name) else {
            return String::new();
        };

        // Fire the command if needed (async — result arrives via event).
        // Gated: run_command() is a WASM host function unavailable in native tests.
        #[cfg(not(test))]
        self.run_if_needed(name, cmd_config, state);

        // Return cached result (empty on first call before result arrives).
        let Some(result) = state.command_results.get(name) else {
            return String::new();
        };

        let stdout = result.stdout.trim_end_matches('\n');
        let stderr = result.stderr.trim_end_matches('\n');
        let exit_code = result
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-1".to_string());

        match cmd_config.render_mode {
            RenderMode::Raw | RenderMode::Dynamic => stdout.to_string(),
            RenderMode::Static => cmd_config
                .format
                .replace("{stdout}", stdout)
                .replace("{stderr}", stderr)
                .replace("{exit_code}", &exit_code),
        }
    }

    fn process_click(&self, name: &str, _state: &PluginState<'_>, _col: usize) {
        #[cfg(not(test))]
        if let Some(cmd_config) = self.configs.get(name) {
            if let Some(click_action) = &cmd_config.click_action {
                let args = parse_commandline(click_action);
                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                run_command(&arg_refs, BTreeMap::new());
            }
        }
    }
}

/// Parse a command string into arguments, respecting quoted strings.
///
/// Splits on whitespace. Content within matching quotes (single or double)
/// is treated as a single argument. Backslash escapes the next character.
///
/// # Examples
///
/// ```
/// # use zellij_status::widgets::command::parse_commandline;
/// assert_eq!(parse_commandline("echo hello"), vec!["echo", "hello"]);
/// assert_eq!(parse_commandline("bash -c 'echo hi'"), vec!["bash", "-c", "echo hi"]);
/// ```
pub fn parse_commandline(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' {
            escape = true;
            continue;
        }

        match in_quote {
            Some(quote_char) if ch == quote_char => {
                in_quote = None;
            }
            Some(_) => {
                current.push(ch);
            }
            None if ch == '"' || ch == '\'' => {
                in_quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            None => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneManifest, TabInfo};

    fn make_state(
        command_results: BTreeMap<String, CommandResult>,
    ) -> (
        Vec<TabInfo>,
        ModeInfo,
        PaneManifest,
        PluginConfig,
        NotificationTracker,
        BTreeMap<String, CommandResult>,
        BTreeMap<String, String>,
    ) {
        let tabs = vec![];
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let config = PluginConfig::from_configuration(std::collections::BTreeMap::new()).unwrap();
        let notifications = NotificationTracker::default();
        let pipe_data = BTreeMap::new();
        (
            tabs,
            mode,
            panes,
            config,
            notifications,
            command_results,
            pipe_data,
        )
    }

    // -- parse_commandline tests --

    #[test]
    fn parse_simple_command() {
        assert_eq!(
            parse_commandline("echo hello world"),
            vec!["echo", "hello", "world"]
        );
    }

    #[test]
    fn parse_double_quoted_args() {
        assert_eq!(
            parse_commandline(r#"bash -c "echo hello""#),
            vec!["bash", "-c", "echo hello"]
        );
    }

    #[test]
    fn parse_single_quoted_args() {
        assert_eq!(
            parse_commandline("bash -c 'echo hello'"),
            vec!["bash", "-c", "echo hello"]
        );
    }

    #[test]
    fn parse_escaped_space() {
        assert_eq!(
            parse_commandline(r"echo hello\ world"),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn parse_empty_string() {
        assert!(parse_commandline("").is_empty());
    }

    #[test]
    fn parse_multiple_spaces() {
        assert_eq!(
            parse_commandline("echo   hello   world"),
            vec!["echo", "hello", "world"]
        );
    }

    #[test]
    fn parse_mixed_quotes() {
        assert_eq!(
            parse_commandline(r#"echo "hello 'world'""#),
            vec!["echo", "hello 'world'"]
        );
    }

    // -- config parsing tests --

    #[test]
    fn parses_command_configs() {
        let config = BTreeMap::from([
            (
                "command_git_command".to_string(),
                "git branch --show-current".to_string(),
            ),
            ("command_git_format".to_string(), " {stdout} ".to_string()),
            ("command_git_interval".to_string(), "5".to_string()),
            ("command_git_rendermode".to_string(), "static".to_string()),
        ]);
        let w = CommandWidget::new(&config);
        assert_eq!(w.names(), vec!["command_git"]);
    }

    #[test]
    fn multiple_commands() {
        let config = BTreeMap::from([
            ("command_git_command".to_string(), "git branch".to_string()),
            ("command_uptime_command".to_string(), "uptime".to_string()),
        ]);
        let w = CommandWidget::new(&config);
        let mut names = w.names();
        names.sort();
        assert_eq!(names, vec!["command_git", "command_uptime"]);
    }

    #[test]
    fn ignores_non_command_keys() {
        let config = BTreeMap::from([
            ("command_git_command".to_string(), "git branch".to_string()),
            ("some_other_command".to_string(), "value".to_string()),
        ]);
        let w = CommandWidget::new(&config);
        assert_eq!(w.names(), vec!["command_git"]);
    }

    // -- process tests --

    #[test]
    fn returns_empty_when_no_result() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(BTreeMap::new());
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config =
            BTreeMap::from([("command_git_command".to_string(), "git branch".to_string())]);
        let w = CommandWidget::new(&widget_config);
        // No cached result yet → empty
        assert_eq!(w.process("command_git", &state), "");
    }

    #[test]
    fn formats_cached_result() {
        let mut results = BTreeMap::new();
        results.insert(
            "command_git".to_string(),
            CommandResult {
                exit_code: Some(0),
                stdout: "main\n".to_string(),
                stderr: String::new(),
                context: BTreeMap::new(),
            },
        );
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(results);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config = BTreeMap::from([
            (
                "command_git_command".to_string(),
                "git branch --show-current".to_string(),
            ),
            ("command_git_format".to_string(), " {stdout} ".to_string()),
        ]);
        let w = CommandWidget::new(&widget_config);
        assert_eq!(w.process("command_git", &state), " main ");
    }

    #[test]
    fn raw_mode_returns_stdout_only() {
        let mut results = BTreeMap::new();
        results.insert(
            "command_raw".to_string(),
            CommandResult {
                exit_code: Some(0),
                stdout: "raw output\n".to_string(),
                stderr: "err".to_string(),
                context: BTreeMap::new(),
            },
        );
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(results);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config = BTreeMap::from([
            ("command_raw_command".to_string(), "echo raw".to_string()),
            ("command_raw_rendermode".to_string(), "raw".to_string()),
        ]);
        let w = CommandWidget::new(&widget_config);
        assert_eq!(w.process("command_raw", &state), "raw output");
    }

    #[test]
    fn exit_code_in_format() {
        let mut results = BTreeMap::new();
        results.insert(
            "command_check".to_string(),
            CommandResult {
                exit_code: Some(1),
                stdout: "fail\n".to_string(),
                stderr: "error msg\n".to_string(),
                context: BTreeMap::new(),
            },
        );
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(results);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config = BTreeMap::from([
            ("command_check_command".to_string(), "false".to_string()),
            (
                "command_check_format".to_string(),
                "{exit_code}:{stderr}".to_string(),
            ),
        ]);
        let w = CommandWidget::new(&widget_config);
        assert_eq!(w.process("command_check", &state), "1:error msg");
    }

    #[test]
    fn unknown_command_returns_empty() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(BTreeMap::new());
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = CommandWidget::new(&BTreeMap::new());
        assert_eq!(w.process("command_unknown", &state), "");
    }
}
