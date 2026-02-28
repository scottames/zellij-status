use std::collections::BTreeMap;
use std::sync::Arc;

use unicode_width::UnicodeWidthStr;

use crate::config::SectionZone;
use crate::render::bar::render_section;
use crate::render::format::{parse_format_string, FormattedPart};
use crate::widgets::{tabs::TabsWidget, PluginState, Widget};

/// Calculate the visible tab window for a vertical list.
///
/// Returns `(start, end, tabs_above, tabs_below)` where:
/// - `start..end` is the slice of `tabs` to render
/// - `tabs_above` is the number of hidden tabs above the visible range
/// - `tabs_below` is the number of hidden tabs below the visible range
///
/// The algorithm centres the active tab, alternating expansion up/down until
/// `available_rows` is filled. Two rows are reserved for overflow indicators
/// when the tab list doesn't fit.
pub fn calculate_visible_range(
    tab_count: usize,
    available_rows: usize,
    active_index: usize,
) -> (usize, usize, usize, usize) {
    if tab_count == 0 {
        return (0, 0, 0, 0);
    }

    // All tabs fit — no overflow.
    if tab_count <= available_rows {
        return (0, tab_count, 0, 0);
    }

    // Reserve 2 rows for overflow indicators (above/below).
    let max_visible = available_rows.saturating_sub(2);
    if max_visible == 0 {
        return (0, 0, tab_count, 0);
    }

    // Start with the active tab and expand outward, alternating up/down.
    let mut start = active_index;
    let mut end = active_index + 1;
    let mut room = max_visible.saturating_sub(1);
    let mut expand_up = true;

    while room > 0 {
        if expand_up && start > 0 {
            start -= 1;
            room -= 1;
        } else if !expand_up && end < tab_count {
            end += 1;
            room -= 1;
        } else if start > 0 {
            start -= 1;
            room -= 1;
        } else if end < tab_count {
            end += 1;
            room -= 1;
        } else {
            break;
        }
        expand_up = !expand_up;
    }

    let tabs_above = start;
    let tabs_below = tab_count.saturating_sub(end);
    (start, end, tabs_above, tabs_below)
}

/// Map a click row to a 1-based tab index (for `switch_tab_to`).
///
/// Returns `None` if the row doesn't correspond to a tab.
pub fn tab_at_row(
    row: usize,
    tab_count: usize,
    available_rows: usize,
    active_index: usize,
) -> Option<usize> {
    if tab_count == 0 {
        return None;
    }

    let (start, end, tabs_above, _tabs_below) =
        calculate_visible_range(tab_count, available_rows, active_index);

    // Row 0 is the overflow-above indicator when present.
    let content_start = usize::from(tabs_above > 0);

    if tabs_above > 0 && row == 0 {
        // Clicking the "above" indicator navigates to the tab just above the window.
        let target = start.saturating_sub(1);
        return Some(target + 1);
    }

    let row_in_tabs = row.saturating_sub(content_start);
    let tab_idx = start + row_in_tabs;

    if tab_idx < end && tab_idx < tab_count {
        return Some(tab_idx + 1);
    }

    // Clicking below the last visible tab (including overflow indicator) goes
    // to the last visible tab.
    let last = end.min(tab_count).saturating_sub(1);
    Some(last + 1)
}

/// Render the vertical tab list to stdout.
///
/// Layout: start-zone (top) → middle-zone/tabs block → end-zone (bottom).
///
/// Outputs exactly `rows` lines (using `println!` for all but the last, which
/// uses `print!` to avoid a trailing newline that Zellij would render as blank).
pub fn render_vertical(
    tabs_widget: &TabsWidget,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    rows: usize,
    cols: usize,
) {
    let config = state.config;
    let aliases = &config.color_aliases;
    let border_fmt = &config.tabs.border;
    let tab_count = state.tabs.len();
    let padding_top = config.tabs.padding_top;

    let (start_formats, middle_formats, end_formats) = split_sections_by_zone(state);

    let start_lines = render_section_lines(&start_formats, widgets, state, cols, border_fmt);
    let middle_lines = render_section_lines(&middle_formats, widgets, state, cols, border_fmt);
    let end_lines = render_section_lines(&end_formats, widgets, state, cols, border_fmt);

    // Keep bottom anchored first, then fit top + padding in remaining rows.
    let bottom_reserved = end_lines.len().min(rows);
    let top_budget = rows.saturating_sub(bottom_reserved);
    let mut start_and_padding: Vec<String> = Vec::with_capacity(top_budget);
    for line in start_lines {
        if start_and_padding.len() >= top_budget {
            break;
        }
        start_and_padding.push(line);
    }
    for _ in 0..padding_top {
        if start_and_padding.len() >= top_budget {
            break;
        }
        start_and_padding.push(build_empty_line(border_fmt, cols, aliases));
    }

    let available = rows
        .saturating_sub(start_and_padding.len())
        .saturating_sub(bottom_reserved)
        .saturating_sub(middle_lines.len());

    let active_index = state.tabs.iter().position(|t| t.active).unwrap_or(0);
    let (start, end, tabs_above, tabs_below) =
        calculate_visible_range(tab_count, available, active_index);

    let start_index = config.tabs.start_index;
    let mut lines: Vec<String> = Vec::with_capacity(rows);

    // Start zone + top padding.
    lines.extend(start_and_padding);

    // Middle-zone lines.
    lines.extend(middle_lines);

    // Overflow-above indicator.
    if tabs_above > 0 {
        let text = config
            .tabs
            .overflow_above
            .replace("{count}", &tabs_above.to_string());
        lines.push(build_plain_line(&text, cols, border_fmt, aliases));
    }

    // Visible tabs.
    for i in start..end {
        if let Some(tab) = state.tabs.get(i) {
            let rendered = tabs_widget.render_tab(tab, state, i + start_index);
            let is_active = tab.active;
            let fill_part = if is_active {
                fill_part_for_format(tabs_widget.select_format(tab, &state.mode.mode), aliases)
            } else {
                None
            };
            lines.push(build_tab_line(
                &rendered,
                cols,
                fill_part.as_ref(),
                border_fmt,
                aliases,
            ));
        }
    }

    // Overflow-below indicator.
    if tabs_below > 0 {
        let text = config
            .tabs
            .overflow_below
            .replace("{count}", &tabs_below.to_string());
        lines.push(build_plain_line(&text, cols, border_fmt, aliases));
    }

    // Fill remaining rows (reserving space for end zone at bottom).
    let target_before_bottom = rows.saturating_sub(bottom_reserved);
    while lines.len() < target_before_bottom {
        lines.push(build_empty_line(border_fmt, cols, aliases));
    }

    // End zone lines (bottom).
    for line in end_lines
        .into_iter()
        .rev()
        .take(bottom_reserved)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        lines.push(line);
    }

    // Safety: pad to exactly `rows` if needed.
    while lines.len() < rows {
        lines.push(build_empty_line(border_fmt, cols, aliases));
    }

    // Print all lines. Last line must not have a trailing newline.
    let last = lines.len().saturating_sub(1);
    for (i, line) in lines.iter().enumerate() {
        if i < last {
            println!("{}\x1b[m", line);
        } else {
            print!("{}\x1b[m", line);
        }
    }
}

/// Render format sections as full-width lines with border.
fn render_section_lines(
    formats: &[String],
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    cols: usize,
    border_fmt: &str,
) -> Vec<String> {
    let aliases = &state.config.color_aliases;
    let mut lines = Vec::new();
    for format_str in formats {
        if format_str.is_empty() {
            continue;
        }
        let rendered = render_section(format_str, widgets, state, aliases);
        if rendered.is_empty() {
            continue;
        }
        lines.push(build_plain_line(&rendered, cols, border_fmt, aliases));
    }
    lines
}

fn split_sections_by_zone(state: &PluginState<'_>) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut start = Vec::new();
    let mut middle = Vec::new();
    let mut end = Vec::new();

    for section in &state.config.sections {
        match section.zone {
            SectionZone::Start => start.push(section.format.clone()),
            SectionZone::Middle => middle.push(section.format.clone()),
            SectionZone::End => end.push(section.format.clone()),
        }
    }

    (start, middle, end)
}

/// Build a tab row: content padded/filled to `cols`, with optional right border.
///
/// When `fill` is true the row background is extended to full width using the
/// bg colour of the first fill-bearing segment. We do this by padding with
/// spaces under the same ANSI reset+bg sequence so the bg visually extends.
fn build_tab_line(
    rendered: &str,
    cols: usize,
    fill_style: Option<&FormattedPart>,
    border_fmt: &str,
    aliases: &std::collections::BTreeMap<String, String>,
) -> String {
    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);

    let clipped = truncate_ansi_to_width(rendered, content_cols);
    let visible = crate::widgets::tabs::strip_ansi_width(&clipped);
    let pad = content_cols.saturating_sub(visible);

    let mut line = String::new();

    line.push_str(&clipped);
    if let Some(fill_part) = fill_style {
        if pad > 0 {
            line.push_str(&fill_part.render(&" ".repeat(pad)));
        }
    } else {
        line.push_str(&" ".repeat(pad));
    }
    line.push_str("\x1b[0m");

    line.push_str(&border);
    line
}

/// Build a plain (unstyled content) row, e.g. for overflow indicators.
fn build_plain_line(
    text: &str,
    cols: usize,
    border_fmt: &str,
    aliases: &std::collections::BTreeMap<String, String>,
) -> String {
    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);

    let clipped = truncate_ansi_to_width(text, content_cols);
    let visible = strip_ansi_width(&clipped);
    let pad = content_cols.saturating_sub(visible);
    let mut line = clipped;
    line.push_str(&" ".repeat(pad));
    line.push_str(&border);
    line
}

/// Build an empty row (all spaces + border).
fn build_empty_line(
    border_fmt: &str,
    cols: usize,
    aliases: &std::collections::BTreeMap<String, String>,
) -> String {
    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);
    let mut line = " ".repeat(content_cols);
    line.push_str(&border);
    line
}

/// Render the border format string to an ANSI string.
fn render_border(border_fmt: &str, aliases: &std::collections::BTreeMap<String, String>) -> String {
    if border_fmt.is_empty() {
        return String::new();
    }
    let parts = crate::render::format::parse_format_string(border_fmt, aliases);
    parts.iter().map(|p| p.render_content()).collect()
}

fn fill_part_for_format(
    format_str: &str,
    aliases: &std::collections::BTreeMap<String, String>,
) -> Option<FormattedPart> {
    parse_format_string(format_str, aliases)
        .into_iter()
        .find(|part| part.fill)
}

/// Strip ANSI escape sequences and return the display width of visible text.
fn strip_ansi_width(s: &str) -> usize {
    crate::widgets::tabs::strip_ansi_width(s)
}

fn truncate_ansi_to_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut width = 0;
    let mut in_escape = false;

    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            out.push(ch);
            continue;
        }

        if in_escape {
            out.push(ch);
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }

        let ch_width = ch.to_string().width();
        if width + ch_width > max_width {
            break;
        }

        out.push(ch);
        width += ch_width;
    }

    if width == max_width {
        out.push_str("\x1b[0m");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::format::parse_format_string;

    // ---- calculate_visible_range ----

    #[test]
    fn visible_range_empty() {
        assert_eq!(calculate_visible_range(0, 10, 0), (0, 0, 0, 0));
    }

    #[test]
    fn visible_range_all_fit() {
        // 5 tabs, 10 rows — everything visible
        let (start, end, above, below) = calculate_visible_range(5, 10, 2);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
        assert_eq!(above, 0);
        assert_eq!(below, 0);
    }

    #[test]
    fn visible_range_exact_fit() {
        // 5 tabs, exactly 5 rows
        let (start, end, above, below) = calculate_visible_range(5, 5, 2);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
        assert_eq!(above, 0);
        assert_eq!(below, 0);
    }

    #[test]
    fn visible_range_overflow_active_first() {
        // 10 tabs, 5 rows — active is tab 0 → window should start at 0
        let (start, end, above, below) = calculate_visible_range(10, 5, 0);
        assert_eq!(start, 0);
        assert_eq!(above, 0);
        assert!(end <= 3); // 5 rows - 2 indicators = 3 visible max
        assert!(below > 0);
    }

    #[test]
    fn visible_range_overflow_active_last() {
        // 10 tabs, 5 rows — active is tab 9 → window near end
        let (_start, end, above, below) = calculate_visible_range(10, 5, 9);
        assert_eq!(end, 10);
        assert_eq!(below, 0);
        assert!(above > 0);
    }

    #[test]
    fn visible_range_active_centred() {
        // 20 tabs, 7 rows — active is tab 10
        // 7 - 2 = 5 visible slots, active centred around index 10
        let (start, end, above, below) = calculate_visible_range(20, 7, 10);
        assert!(start <= 10);
        assert!(end > 10);
        assert_eq!(end - start, 5);
        assert!(above > 0);
        assert!(below > 0);
    }

    #[test]
    fn visible_range_above_plus_below_accounts_for_hidden() {
        let tab_count = 15;
        let rows = 6;
        let active = 7;
        let (start, end, above, below) = calculate_visible_range(tab_count, rows, active);
        assert_eq!(above, start);
        assert_eq!(below, tab_count - end);
        assert_eq!(above + (end - start) + below, tab_count);
    }

    // ---- tab_at_row ----

    #[test]
    fn tab_at_row_no_overflow() {
        // 3 tabs fit in 10 rows → row 0 = tab 1, row 1 = tab 2, row 2 = tab 3
        assert_eq!(tab_at_row(0, 3, 10, 0), Some(1));
        assert_eq!(tab_at_row(1, 3, 10, 1), Some(2));
        assert_eq!(tab_at_row(2, 3, 10, 2), Some(3));
    }

    #[test]
    fn tab_at_row_with_overflow_above() {
        // 10 tabs, 5 rows, active=9 → there will be overflow above
        // row 0 is the overflow indicator
        let result = tab_at_row(0, 10, 5, 9);
        // Should navigate to the tab just above the visible window (not None)
        assert!(result.is_some());
    }

    #[test]
    fn tab_at_row_empty() {
        assert_eq!(tab_at_row(0, 0, 10, 0), None);
    }

    #[test]
    fn plain_line_clips_long_text_to_width() {
        let line = build_plain_line("abcdefghijklmnopqrstuvwxyz", 10, "", &BTreeMap::new());
        assert_eq!(strip_ansi_width(&line), 10);
    }

    #[test]
    fn plain_line_clips_long_ansi_text_to_width() {
        let text = "\x1b[31mabcdefghijklmnopqrstuvwxyz\x1b[0m";
        let line = build_plain_line(text, 12, "", &BTreeMap::new());
        assert_eq!(strip_ansi_width(&line), 12);
    }

    #[test]
    fn tab_line_fill_avoids_reverse_video() {
        let aliases = BTreeMap::from([("base".to_string(), "#1e1e2e".to_string())]);
        let parts = parse_format_string("#[bg=$base,fg=#ffffff,fill]X", &aliases);
        let line = build_tab_line("X", 8, Some(&parts[0]), "", &aliases);
        assert!(!line.contains("\x1b[7m"));
    }
}
