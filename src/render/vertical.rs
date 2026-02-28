use std::collections::BTreeMap;
use std::sync::Arc;

use unicode_width::UnicodeWidthStr;

use crate::config::{FormatSection, SectionZone, TextAlign};
use crate::render::bar::{expand_widgets, render_section};
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

    let tabs_anchor = first_tabs_anchor(&state.config.sections);
    let mut anchor_seen = false;

    let mut start_before = Vec::new();
    let mut start_after = Vec::new();
    let mut middle_before = Vec::new();
    let mut middle_after = Vec::new();
    let mut end_before = Vec::new();
    let mut end_after = Vec::new();

    for section in &state.config.sections {
        if !anchor_seen && section_has_tabs_marker(section) {
            anchor_seen = true;
            continue;
        }

        let line = if let Some((left, right)) = section.split_pair() {
            render_split_section_line(left, right, widgets, state, cols, border_fmt)
        } else {
            render_section_line(section, &section.format, widgets, state, cols, border_fmt)
        };

        let Some(line) = line else {
            continue;
        };

        match (section.zone, tabs_anchor.zone, anchor_seen) {
            (SectionZone::Start, SectionZone::Start, true) => start_after.push(line),
            (SectionZone::Start, _, _) => start_before.push(line),
            (SectionZone::Middle, SectionZone::Middle, true) => middle_after.push(line),
            (SectionZone::Middle, _, _) => middle_before.push(line),
            (SectionZone::End, SectionZone::End, true) => end_after.push(line),
            (SectionZone::End, _, _) => end_before.push(line),
        }
    }

    let active_index = state.tabs.iter().position(|t| t.active).unwrap_or(0);
    let mut lines: Vec<String> = Vec::with_capacity(rows);

    if tabs_anchor.zone == SectionZone::End {
        let bottom_non_tab_len = end_before.len() + end_after.len();
        let non_tab_bottom_reserved = bottom_non_tab_len.min(rows);
        let top_budget = rows.saturating_sub(non_tab_bottom_reserved);

        for line in start_before
            .into_iter()
            .chain(start_after)
            .chain(middle_before)
            .chain(middle_after)
        {
            if lines.len() >= top_budget {
                break;
            }
            lines.push(line);
        }

        while lines.len() < top_budget {
            lines.push(build_empty_line(border_fmt, cols, aliases));
        }

        let padding_rows = padding_top.min(
            rows.saturating_sub(lines.len())
                .saturating_sub(non_tab_bottom_reserved),
        );
        let available_tabs = rows
            .saturating_sub(lines.len())
            .saturating_sub(non_tab_bottom_reserved)
            .saturating_sub(padding_rows);
        let tabs_lines = render_tabs_block(
            tabs_widget,
            state,
            active_index,
            tab_count,
            available_tabs,
            cols,
            border_fmt,
            aliases,
            tabs_anchor.align,
        );

        let mut bottom_block = Vec::new();
        bottom_block.extend(end_before);
        for _ in 0..padding_rows {
            bottom_block.push(build_empty_line(border_fmt, cols, aliases));
        }
        bottom_block.extend(tabs_lines);
        bottom_block.extend(end_after);

        let remaining = rows.saturating_sub(lines.len());
        for line in bottom_block
            .into_iter()
            .rev()
            .take(remaining)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            lines.push(line);
        }
    } else {
        let end_lines: Vec<String> = end_before.into_iter().chain(end_after).collect();
        let bottom_reserved = end_lines.len().min(rows);
        let top_budget = rows.saturating_sub(bottom_reserved);

        let (fixed_before, fixed_after) = match tabs_anchor.zone {
            SectionZone::Start => (
                start_before.len(),
                start_after.len() + middle_before.len() + middle_after.len(),
            ),
            SectionZone::Middle => (
                start_before.len() + start_after.len() + middle_before.len(),
                middle_after.len(),
            ),
            SectionZone::End => (0, 0),
        };
        let padding_rows = padding_top.min(top_budget.saturating_sub(fixed_before + fixed_after));
        let available_tabs = top_budget
            .saturating_sub(fixed_before)
            .saturating_sub(padding_rows)
            .saturating_sub(fixed_after);
        let tabs_lines = render_tabs_block(
            tabs_widget,
            state,
            active_index,
            tab_count,
            available_tabs,
            cols,
            border_fmt,
            aliases,
            tabs_anchor.align,
        );

        match tabs_anchor.zone {
            SectionZone::Start => {
                for line in start_before {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
                for _ in 0..padding_rows {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(build_empty_line(border_fmt, cols, aliases));
                }
                for line in tabs_lines {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
                for line in start_after
                    .into_iter()
                    .chain(middle_before)
                    .chain(middle_after)
                {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
            }
            SectionZone::Middle => {
                for line in start_before
                    .into_iter()
                    .chain(start_after)
                    .chain(middle_before)
                {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
                for _ in 0..padding_rows {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(build_empty_line(border_fmt, cols, aliases));
                }
                for line in tabs_lines {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
                for line in middle_after {
                    if lines.len() >= top_budget {
                        break;
                    }
                    lines.push(line);
                }
            }
            SectionZone::End => {}
        }

        while lines.len() < top_budget {
            lines.push(build_empty_line(border_fmt, cols, aliases));
        }

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

fn render_section_line(
    section: &FormatSection,
    format_str: &str,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    cols: usize,
    border_fmt: &str,
) -> Option<String> {
    if format_str.is_empty() {
        return None;
    }
    let fill_part = first_fill_part_for_section(format_str, widgets, state)
        .or_else(|| {
            parse_format_string(format_str, &state.config.color_aliases)
                .into_iter()
                .find(|part| part.fill)
        });
    let expanded = expand_widgets(format_str, widgets, state);
    let parts = parse_format_string(&expanded, &state.config.color_aliases);
    let rendered: String = parts.iter().map(|part| part.render_content()).collect();
    if rendered.is_empty() {
        return None;
    }
    Some(build_plain_line(
        &rendered,
        cols,
        border_fmt,
        &state.config.color_aliases,
        section.align,
        fill_part.as_ref(),
    ))
}

fn render_split_section_line(
    left_format: &str,
    right_format: &str,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
    cols: usize,
    border_fmt: &str,
) -> Option<String> {
    let aliases = &state.config.color_aliases;
    let rendered_left = render_section(left_format, widgets, state, aliases);
    let rendered_right = render_section(right_format, widgets, state, aliases);
    if rendered_left.is_empty() && rendered_right.is_empty() {
        return None;
    }

    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);

    let clipped_right = truncate_ansi_to_width(&rendered_right, content_cols);
    let right_width = strip_ansi_width(&clipped_right);
    let clipped_left = truncate_ansi_to_width(&rendered_left, content_cols.saturating_sub(right_width));
    let left_width = strip_ansi_width(&clipped_left);
    let gap = content_cols.saturating_sub(left_width + right_width);

    let gap_fill = first_fill_part_for_section(left_format, widgets, state)
        .or_else(|| {
            parse_format_string(left_format, aliases)
                .into_iter()
                .find(|part| part.fill)
        })
        .or_else(|| first_fill_part_for_section(right_format, widgets, state))
        .or_else(|| {
            parse_format_string(right_format, aliases)
                .into_iter()
                .find(|part| part.fill)
        });

    let mut line = String::new();
    line.push_str(&clipped_left);
    if gap > 0 {
        if let Some(fill) = gap_fill.as_ref() {
            line.push_str(&fill.render(&" ".repeat(gap)));
        } else {
            line.push_str(&" ".repeat(gap));
        }
    }
    line.push_str(&clipped_right);
    line.push_str(&border);

    Some(line)
}

fn first_fill_part_for_section(
    format_str: &str,
    widgets: &BTreeMap<String, Arc<dyn Widget>>,
    state: &PluginState<'_>,
) -> Option<FormattedPart> {
    let mut chars = format_str.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            continue;
        }

        let mut name = String::new();
        while let Some(&next) = chars.peek() {
            chars.next();
            if next == '}' {
                break;
            }
            name.push(next);
        }

        if name.is_empty() {
            continue;
        }

        if let Some(widget) = widgets.get(&name)
            && let Some(fill) = widget.fill_part(&name, state)
        {
            return Some(fill);
        }
    }

    None
}

fn section_has_tabs_marker(section: &FormatSection) -> bool {
    section.format.contains("{tabs}")
        || section
            .split_left
            .as_ref()
            .is_some_and(|left| left.contains("{tabs}"))
        || section
            .split_right
            .as_ref()
            .is_some_and(|right| right.contains("{tabs}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabsAnchor {
    zone: SectionZone,
    align: TextAlign,
}

fn first_tabs_anchor(sections: &[FormatSection]) -> TabsAnchor {
    sections
        .iter()
        .find(|section| section_has_tabs_marker(section))
        .map(|section| TabsAnchor {
            zone: section.zone,
            align: section.align,
        })
        .unwrap_or(TabsAnchor {
            zone: SectionZone::Middle,
            align: TextAlign::Left,
        })
}

fn render_tabs_block(
    tabs_widget: &TabsWidget,
    state: &PluginState<'_>,
    active_index: usize,
    tab_count: usize,
    available_rows: usize,
    cols: usize,
    border_fmt: &str,
    aliases: &std::collections::BTreeMap<String, String>,
    align: TextAlign,
) -> Vec<String> {
    if tab_count == 0 || available_rows == 0 {
        return Vec::new();
    }

    let (start, end, tabs_above, tabs_below) =
        calculate_visible_range(tab_count, available_rows, active_index);
    let mut lines = Vec::with_capacity(available_rows);
    let start_index = state.config.tabs.start_index;

    if tabs_above > 0 && lines.len() < available_rows {
        let text = state
            .config
            .tabs
            .overflow_above
            .replace("{count}", &tabs_above.to_string());
        lines.push(build_plain_line(
            &text, cols, border_fmt, aliases, align, None,
        ));
    }

    for i in start..end {
        if lines.len() >= available_rows {
            break;
        }
        if let Some(tab) = state.tabs.get(i) {
            let rendered = tabs_widget.render_tab(tab, state, i + start_index);
            let fill_part = if tab.active {
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
                align,
            ));
        }
    }

    if tabs_below > 0 && lines.len() < available_rows {
        let text = state
            .config
            .tabs
            .overflow_below
            .replace("{count}", &tabs_below.to_string());
        lines.push(build_plain_line(
            &text, cols, border_fmt, aliases, align, None,
        ));
    }

    lines
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
    align: TextAlign,
) -> String {
    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);

    let clipped = truncate_ansi_to_width(rendered, content_cols);
    let visible = crate::widgets::tabs::strip_ansi_width(&clipped);
    let (left_pad, right_pad) = align_padding(content_cols.saturating_sub(visible), align);

    let mut line = String::new();

    if let Some(fill_part) = fill_style {
        if left_pad > 0 {
            line.push_str(&fill_part.render(&" ".repeat(left_pad)));
        }
    } else {
        line.push_str(&" ".repeat(left_pad));
    }

    line.push_str(&clipped);
    if let Some(fill_part) = fill_style {
        if right_pad > 0 {
            line.push_str(&fill_part.render(&" ".repeat(right_pad)));
        }
    } else {
        line.push_str(&" ".repeat(right_pad));
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
    align: TextAlign,
    fill_style: Option<&FormattedPart>,
) -> String {
    let border = render_border(border_fmt, aliases);
    let border_width = strip_ansi_width(&border);
    let content_cols = cols.saturating_sub(border_width);

    let clipped = truncate_ansi_to_width(text, content_cols);
    let visible = strip_ansi_width(&clipped);
    let (left_pad, right_pad) = align_padding(content_cols.saturating_sub(visible), align);
    let mut line = String::new();
    if let Some(fill_part) = fill_style {
        if left_pad > 0 {
            line.push_str(&fill_part.render(&" ".repeat(left_pad)));
        }
    } else {
        line.push_str(&" ".repeat(left_pad));
    }
    line.push_str(&clipped);
    if let Some(fill_part) = fill_style {
        if right_pad > 0 {
            line.push_str(&fill_part.render(&" ".repeat(right_pad)));
        }
    } else {
        line.push_str(&" ".repeat(right_pad));
    }
    line.push_str(&border);
    line
}

fn align_padding(space: usize, align: TextAlign) -> (usize, usize) {
    match align {
        TextAlign::Left => (0, space),
        TextAlign::Right => (space, 0),
        TextAlign::Center => {
            let left = space / 2;
            let right = space.saturating_sub(left);
            (left, right)
        }
    }
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
        let line = build_plain_line(
            "abcdefghijklmnopqrstuvwxyz",
            10,
            "",
            &BTreeMap::new(),
            TextAlign::Left,
            None,
        );
        assert_eq!(strip_ansi_width(&line), 10);
    }

    #[test]
    fn plain_line_clips_long_ansi_text_to_width() {
        let text = "\x1b[31mabcdefghijklmnopqrstuvwxyz\x1b[0m";
        let line = build_plain_line(text, 12, "", &BTreeMap::new(), TextAlign::Left, None);
        assert_eq!(strip_ansi_width(&line), 12);
    }

    #[test]
    fn tab_line_fill_avoids_reverse_video() {
        let aliases = BTreeMap::from([("base".to_string(), "#1e1e2e".to_string())]);
        let parts = parse_format_string("#[bg=$base,fg=#ffffff,fill]X", &aliases);
        let line = build_tab_line("X", 8, Some(&parts[0]), "", &aliases, TextAlign::Left);
        assert!(!line.contains("\x1b[7m"));
    }

    #[test]
    fn tabs_anchor_defaults_to_middle_when_missing() {
        let sections = vec![FormatSection {
            index: 1,
            zone: SectionZone::Start,
            align: TextAlign::Left,
            format: "{mode}".to_string(),
            split_left: None,
            split_right: None,
        }];
        assert_eq!(
            first_tabs_anchor(&sections),
            TabsAnchor {
                zone: SectionZone::Middle,
                align: TextAlign::Left,
            }
        );
    }

    #[test]
    fn tabs_anchor_uses_first_tabs_section() {
        let sections = vec![
            FormatSection {
                index: 1,
                zone: SectionZone::Start,
                align: TextAlign::Right,
                format: "{tabs}".to_string(),
                split_left: None,
                split_right: None,
            },
            FormatSection {
                index: 2,
                zone: SectionZone::End,
                align: TextAlign::Left,
                format: "{tabs}".to_string(),
                split_left: None,
                split_right: None,
            },
        ];
        assert_eq!(
            first_tabs_anchor(&sections),
            TabsAnchor {
                zone: SectionZone::Start,
                align: TextAlign::Right,
            }
        );
    }

    #[test]
    fn tabs_anchor_detects_tabs_marker_in_split_section() {
        let sections = vec![FormatSection {
            index: 2,
            zone: SectionZone::End,
            align: TextAlign::Left,
            format: String::new(),
            split_left: Some("{mode}".to_string()),
            split_right: Some("{tabs}".to_string()),
        }];
        assert_eq!(
            first_tabs_anchor(&sections),
            TabsAnchor {
                zone: SectionZone::End,
                align: TextAlign::Left,
            }
        );
    }

    #[test]
    fn plain_line_right_align_adds_left_padding() {
        let line = build_plain_line("abc", 8, "", &BTreeMap::new(), TextAlign::Right, None);
        assert_eq!(strip_ansi_width(&line), 8);
        assert!(line.starts_with("     abc"));
    }

    #[test]
    fn plain_line_center_align_balances_padding() {
        let line = build_plain_line("abc", 8, "", &BTreeMap::new(), TextAlign::Center, None);
        assert_eq!(strip_ansi_width(&line), 8);
        assert!(line.starts_with("  abc"));
    }

    #[test]
    fn plain_line_right_align_fill_colors_left_padding() {
        let aliases = BTreeMap::from([("base".to_string(), "#1e1e2e".to_string())]);
        let parts = parse_format_string("#[bg=$base,fill]X", &aliases);
        let line = build_plain_line("X", 8, "", &aliases, TextAlign::Right, Some(&parts[0]));
        assert!(line.contains('\x1b'));
        assert_eq!(strip_ansi_width(&line), 8);
    }
}
