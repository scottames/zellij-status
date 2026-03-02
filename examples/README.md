# Examples

Each directory is a self-contained Zellij config profile with a `config.kdl` and
`layout.kdl`. Try any example, read the comments in `layout.kdl`, and copy the
parts you like into your own config.

## Profiles

| Profile                                     | Mode       | Palette          | What it shows                                 |
| ------------------------------------------- | ---------- | ---------------- | --------------------------------------------- |
| [`minimal`](minimal/)                       | horizontal | neutral          | Bare-minimum starter — 3 colours, 3 widgets   |
| [`default`](default/)                       | horizontal | Catppuccin Mocha | All widgets, split pairs, caps, precedence    |
| [`powerline`](powerline/)                   | horizontal | Catppuccin Mocha | Powerline arrows both directions, fill + caps |
| [`vertical`](vertical/)                     | vertical   | Catppuccin Mocha | Left sidebar with overflow, split rows, caps  |
| [`vertical-right`](vertical-right/)         | vertical   | Tokyo Night      | Right sidebar, reversed cap direction         |
| [`gallery-vertical`](gallery-vertical/)     | vertical   | Gruvbox Dark     | Compare alignment, fill, split, cap styles    |
| [`gallery-horizontal`](gallery-horizontal/) | horizontal | Gruvbox Dark     | Compare spacers, precedence, tab separators   |

## Try an example

With [mise](https://mise.jdx.dev/) (builds the plugin first):

```bash
mise run example <profile>
# e.g. mise run example powerline
```

Without mise:

```bash
cargo build
zellij \
  -s "zellij-status-<profile>" \
  --config-dir ./examples/<profile> \
  --config ./examples/<profile>/config.kdl \
  -n ./examples/<profile>/layout.kdl
```

## Which should I start with?

- **New to zellij-status?** Start with `minimal` — it's the simplest working
  config. Add features one at a time.
- **Want a horizontal bar?** Start with `default` for a full-featured baseline,
  or `powerline` for a styled look.
- **Want a sidebar?** Start with `vertical` (left side) or `vertical-right`
  (right side).
- **Comparing options?** The `gallery-*` profiles show multiple styling
  approaches side-by-side so you can pick what you like.

## Important: template sync

Zellij uses two templates for tab layouts:

- `default_tab_template` — tabs declared in the layout file
- `new_tab_template` — tabs created at runtime (e.g. `Ctrl-t n`)

Both must contain the same plugin configuration block because Zellij creates a
separate plugin instance for each. If you change one, update the other to match.
