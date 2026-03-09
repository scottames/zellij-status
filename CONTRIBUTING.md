# Contributing to zellij-status

Thanks for your interest in contributing to zellij-status!

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- The `wasm32-wasip1` target: `rustup target add wasm32-wasip1`
- [Zellij](https://zellij.dev/) for visual testing

Optionally, install [mise](https://mise.jdx.dev/) to use the task runner
shortcuts described below.

## Building

The default cargo target is `wasm32-wasip1` (set in `.cargo/config.toml`), so a
plain `cargo build` produces the WASM plugin:

```bash
cargo build
# output: target/wasm32-wasip1/debug/zellij-status.wasm
```

Or with mise:

```bash
mise run build
```

## Running tests

Tests must run on the **native** target since the WASM runtime doesn't support
the test harness:

```bash
cargo test --lib --target "$(rustc -vV | sed -n 's/^host: //p')"
```

Or with mise (auto-detects your host target):

```bash
mise run test
```

## Regenerating the config reference

The generated config reference is driven by `schema/config-schema.json`.
Regenerate it with:

```bash
mise run config-docs
```

To verify the checked-in reference is current without rewriting the file:

```bash
mise run check-config-docs
```

## Linting and formatting

This project uses [Trunk](https://docs.trunk.io/) for linting (clippy,
formatting, yamllint, markdownlint, etc.):

```bash
trunk check
```

Or with mise (runs tests first, then trunk check):

```bash
mise run check
```

## Trying your changes

Launch any of the bundled example profiles to test the plugin visually:

```bash
mise run example default
```

See [`examples/README.md`](examples/README.md) for available profiles and manual
launch instructions without mise.

On first run, Zellij prompts for plugin permissions — accept them or see the
[README](README.md#quick-start) for details.

## Submitting changes

1. Fork the repo and create a feature branch from `main`.
2. Make your changes — keep PRs focused on a single concern.
3. Ensure `cargo test --target x86_64-unknown-linux-gnu` passes.
4. Ensure `trunk check` passes (or `mise run check`).
5. Open a pull request with a
   [conventional commit](https://www.conventionalcommits.org/) title (e.g.
   `feat: add foo widget`, `fix: handle empty tab name`). PR titles are
   validated automatically.

## Project layout

```plaintext
src/
  state.rs       # ZellijPlugin impl, event handling
  config.rs      # PluginConfig, LayoutMode, parsing
  render/        # format.rs (ANSI), color.rs, bar.rs (horizontal), vertical.rs
  widgets/       # Widget trait + tabs, mode, datetime, session, notification
  notify/        # NotificationTracker, pipe protocol parsing
examples/        # Self-contained config profiles (config.kdl + layout.kdl)
docs/            # Advanced feature reference
```

## Documentation

- [`README.md`](README.md) — overview, installation, quick start
- [`examples/README.md`](examples/README.md) — example profiles and usage
- [`docs/config-reference.kdl`](docs/config-reference.kdl) — generated key-by-key
  config reference
- [`docs/advanced-features.md`](docs/advanced-features.md) — format strings,
  notifications, pipe protocol, caps, vertical layout details
