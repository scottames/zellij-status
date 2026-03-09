# Examples

Each directory is a self-contained Zellij config profile with a `config.kdl` and
`layout.kdl`. Try any example, read the comments in `layout.kdl`, and copy the
parts you like into your own config.

## Profiles

| Profile                             | Mode       | Palette          | What it shows                                 |
| ----------------------------------- | ---------- | ---------------- | --------------------------------------------- |
| [`minimal`](minimal/)               | horizontal | neutral          | Bare-minimum starter — 3 colours, 3 widgets   |
| [`default`](default/)               | horizontal | Catppuccin Mocha | All widgets, split pairs, caps, precedence    |
| [`powerline`](powerline/)           | horizontal | Catppuccin Mocha | Powerline arrows both directions, fill + caps |
| [`vertical`](vertical/)             | vertical   | Catppuccin Mocha | Left sidebar with overflow, split rows, caps  |
| [`vertical-right`](vertical-right/) | vertical   | Tokyo Night      | Right sidebar, reversed cap direction         |

## Try an example

With [mise](https://mise.jdx.dev/) (builds the plugin first):

```bash
EXAMPLE="powerline"
mise run example "${EXAMPLE}"
```

Without mise:

```bash
EXAMPLE="default"
cargo build
zellij \
  -s "zellij-status-${EXAMPLE}" \
  --config-dir "./examples/${EXAMPLE}" \
  --config "./examples/${EXAMPLE}/config.kdl" \
  -n "./examples/${EXAMPLE}/layout.kdl"
```

## Which should I start with?

- _New to zellij-status?_
  - Start with `minimal` — it's the simplest working config. Add features one at
    a time.
- _Want a horizontal bar?_
  - Start with `default` for a full-featured baseline, or `powerline` for a
    styled look.
- _Want a sidebar?_
  - Start with `vertical` (left side) or `vertical-right` (right side).

## Advanced reference

- Interactive walkthrough, [`GUIDE.txt`](GUIDE.txt), will be printed in the
  first pane in all examples.
- Advanced customization and feature reference:
  [`docs/advanced-features.md`](../docs/advanced-features.md)
- Generated config key reference:
  [`docs/config-reference.kdl`](../docs/config-reference.kdl)

## Template for new tabs

For _horizontal_ layouts, omit `new_tab_template` — Zellij falls back to
`default_tab_template` automatically.

For _vertical_ layouts, define `new_tab_template` with `pane command="bash"` as
the content pane. Using `children` directly in `new_tab_template` is not
supported (see vertical examples for the correct structure).
