# Advanced Features

This document explains how to customize `zellij-status` beyond the starter
examples. Use `examples/GUIDE.txt` for an interactive walkthrough, then use this
reference when you want to tune behavior.

## Start from a known-good profile

<!-- prettier-ignore-start -->
>[!NOTE]
> The example profiles use [Nerd Fonts](https://www.nerdfonts.com/) glyphs for
> tab indicators, powerline arrows, and notification icons. Make sure your
> terminal font includes Nerd Fonts glyphs, or substitute plain-text alternatives
> in your own config.
<!-- prettier-ignore-end -->

Run any profile from `examples/` first so you can compare behavior while making
changes:

```bash
mise run example default
# or: mise run example powerline
```

## Format model: how the bar is composed

The bar is built from **sections** — each section is a styled segment of text
rendered in a specific zone of the bar. You define sections using
`format_<index>_<zone>` keys, and the plugin composes them left-to-right (or
top-to-bottom in vertical mode) within each zone.

- `index` controls render order within a zone (`1`, `2`, `3`, ...)
- `zone` depends on layout mode:
  - horizontal: `left`, `center`, `right` (aliases: `start`, `middle`, `end`)
  - vertical: `top`, `middle`, `bottom`

Example (horizontal):

```kdl
format_1_left  "{mode}#[fg=$surface1]|{tabs}"
format_2_right "{notifications}"
format_3_right_left  "#[fg=$subtext0] {session} {command_git}"
format_3_right_right "#[fg=$subtext0]{swap_layout} {datetime} "
format_space "#[bg=$mantle]"
format_precedence "132"
format_hide_on_overlength "true"
```

Notes:

- `format_<index>_<zone>_left` + `..._right` creates a split row pair.
- `format_space` sets the fill style between major zones.
- `format_precedence` and `format_hide_on_overlength` control what disappears
  first on narrow terminals.

## Notifications: per-tab vs aggregate

There are two separate placeholders with different jobs:

- `{notification}`: per-tab icon, used in tab format strings (`tab_normal`,
  `tab_active`, etc.)
- `{notifications}`: aggregate widget, used in a format section and displays a
  global count

Typical notification config:

```kdl
notification_enabled               "true"
notification_format                "#[fg=$peach,bold] {count} "
notification_format_completed      "#[fg=$green,bold]{icon}"
notification_format_in_progress    "{icon}"
notification_format_tab            "{icon}"
notification_format_waiting        "#[fg=$peach,bold]{icon}"
notification_indicator_completed   "✅"
notification_indicator_in_progress "🔄"
notification_indicator_waiting     "⏳"
notification_show_if_empty         "false"
```

`notification_format_*` keys style the per-tab `{notification}` icon.
`notification_format_tab` is the fallback for any state-specific format key you
do not set.

If your tab format already has surrounding style, prefer isolating
`{notification}` in its own segment so icon styling is predictable, for example:

```kdl
tab_normal "#[fg=$overlay0] {index} {name} #[fg=$overlay0]{notification}"
```

Pipe protocol for state changes:

```plaintext
zellij-status::EVENT::PANE_ID
```

| Event         | Meaning                                            |
| ------------- | -------------------------------------------------- |
| `waiting`     | Mark pane as waiting for input                     |
| `in_progress` | Mark pane as actively running (`busy` is an alias) |
| `completed`   | Mark pane as done                                  |

Try it:

```bash
echo $ZELLIJ_PANE_ID # prints the current pane-id, add a digit or two
zellij pipe --name "zellij-status::waiting::$((ZELLIJ_PANE_ID + 1))"
zellij pipe --name "zellij-status::in_progress::$((ZELLIJ_PANE_ID + 1))"
zellij pipe --name "zellij-status::completed::$((ZELLIJ_PANE_ID + 1))"
```

<!-- prettier-ignore-start -->
>[!NOTE]
> Sending the pipe to the same pane that has focus will cause the notification
> to clear as moving focus to any pane with an active notification will clear
> the notification.
<!-- prettier-ignore-end -->

Example behavior differences:

- `default`: shows both per-tab `{notification}` icons and aggregate
  `{notifications}` count.
- `powerline`: includes `{notification}` in tab formats, but does not place
  `{notifications}` in a format section.

## Pipe widget: custom external status text

Pipe widget values use:

`zellij-status::pipe::<name>::<value>`

And render through `pipe_<name>_format`.

Example (`default` profile):

```kdl
pipe_status_format "#[fg=$green]{output}"
```

Try it:

```bash
zellij pipe --name "zellij-status::pipe::status::running"
zellij pipe --name "zellij-status::pipe::status::done"
zellij pipe --name "zellij-status::pipe::status::"
```

The final command clears the value.

## Styling, fill, and caps

Three styling ideas are used across examples:

- `fill` in a style directive stretches that segment background across remaining
  space in a row.
- `{mode_cap}` renders a cap transition arrow based on mode fill color.
- `cap_bg` sets the background behind cap arrows; `cap_symbol` can override the
  glyph.
- If you prefer a classic non-powerline look, use `tab_separator` and/or literal
  separators (like `|`) in `format_*` strings instead of cap widgets.

Example:

```kdl
format_1_left "{mode}{mode_cap}{tabs}"
cap_bg "$nord0"
mode_normal "#[bg=$frost2,fg=$nord0,bold,fill] NORMAL "
```

## Vertical mode specifics

In vertical layouts (`layout_mode "vertical"`):

- zones become `top`, `middle`, and `bottom`
- `{tabs}` is usually anchored in `middle` so it can expand
- overflow indicators (`tab_overflow_above`, `tab_overflow_below`) appear when
  tab rows exceed available height

Right sidebar (see `vertical-right` example) mirrors the left sidebar by
changing pane order and alignment keys, for example
`format_1_middle_right "{tabs}"`.

To move from left sidebar to right sidebar:

- In the tab template, place `children` before the plugin pane so the plugin
  renders on the right
- Optionally
  - Switch tab alignment from `_middle_left` to `_middle_right`
  - Rotate directional styling, e.g. caps/powerline arrows, set `cap_symbol ""`
    to mirror arrow direction

Example:

```kdl
// left sidebar
pane split_direction="vertical" {
    pane size=26 borderless=true {
        plugin location="file:~/.config/zellij/plugins/zellij-status.wasm" {
            layout_mode "vertical"
            format_1_middle_left "{tabs}"
        }
    }
    children
}

// right sidebar
pane split_direction="vertical" {
    children
    pane size=26 borderless=true {
        plugin location="file:~/.config/zellij/plugins/zellij-status.wasm" {
            layout_mode "vertical"
            format_1_middle_right "{tabs}"
            cap_symbol ""
        }
    }
}
```

Use `examples/vertical-right/layout.kdl` as a complete reference.

## Command widget refresher

Command widgets are named by suffix:

```kdl
command_git_command    "git branch --show-current"
command_git_format     "#[fg=$blue] {stdout} "
command_git_interval   "10"
command_git_rendermode "static"
```

- `static`: command output is inserted as literal text (`{stdout}`)
- `dynamic`: command output can include style directives (`#[...]`)

See the [`default`](./examples/default) example for a live usage of the command
widget.

## Troubleshooting

- Notifications never appear:
  - confirm `notification_enabled "true"`
  - confirm `{notification}` exists in your `tab_*` format strings
  - if you want global count, also place `{notifications}` in a `format_*` key
- Pipe value never appears:
  - confirm matching `pipe_<name>_format` exists
  - verify pipe name uses `zellij-status::pipe::<name>::<value>` exactly
- New tabs flash and close immediately:
  - for _horizontal_ layouts: remove `new_tab_template`; Zellij falls back to
    `default_tab_template`
  - for _vertical_ layouts: ensure `new_tab_template` uses `pane command="bash"`
    not `children`
- Widgets disappear in small terminals:
  - expected if `format_hide_on_overlength "true"` is set; tune
    `format_precedence`
