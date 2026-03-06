# agents

## project overview

`zellij-status` — A Zellij plugin combining zjstatus (format engine),
zellij-vertical-tabs (vertical layout), and zellij-attention (notifications)
into one WASM binary. Renders tabs vertically or horizontally with configurable
styling and pipe-based notification icons.

## architecture

```
src/
  state.rs       # ZellijPlugin impl, event handling
  config.rs      # PluginConfig, LayoutMode, parsing
  render/        # format.rs (ANSI), color.rs, bar.rs (horizontal), vertical.rs
  widgets/       # Widget trait + tabs, mode, datetime, session, notification
  notify/        # NotificationTracker, pipe protocol parsing
```

## build & test

- `mise run build` — compile to wasm32-wasip1
- `mise run example` — launch Zellij dev session (e.g.
  `mise run example powerline`)
- `mise run test` — tests run on native host target (auto-detected)
- `mise run check` — trunk check (clippy, formatting)

## before work should be considered complete

1. run checks

```bash
mise run check
```

## visual verification with pilotty

If `pilotty` is available, us it to test the plugin renders correctly in a real
Zellij session. Load the `pilotty` skill if available.

### one-time setup: grant plugin permissions

The first time the plugin runs, Zellij prompts for permissions. Add them to
`~/.cache/zellij/permissions.kdl` so headless sessions don't stall:

```kdl
"/absolute/path/to/target/wasm32-wasip1/debug/zellij-status.wasm" {
    ReadApplicationState
    ChangeApplicationState
    ReadCliPipes
}
```

Use `realpath target/wasm32-wasip1/debug/zellij-status.wasm` for the exact path.

### running a visual test

```bash
# 1. build the plugin
mise run build

# 2. spawn a zellij dev session via pilotty (use any profile: default)
pilotty spawn --name dev-view zellij \
  -s zellij-status-default \
  --config-dir ./examples/default \
  --config ./examples/default/config.kdl \
  -n ./examples/default/layout.kdl

# 3. wait for it to load (if first run, grant permissions first — see above)
sleep 5

# 4. snapshot the terminal to verify rendering
pilotty snapshot -s dev-view --format text

# 5. clean up
pilotty kill -s dev-view
zellij delete-session zellij-status-default --force
```

### notes

- `mise run example` launches the default profile interactively
- `mise run example powerline` launches the powerline profile
- Available profiles live in `examples/<profile>/` (each has config.kdl +
  layout.kdl)
- `--config-dir ./examples/<profile>` isolates the session from your user config
- The wasm path in layout files uses a hyphen: `zellij-status.wasm` (matches the
  crate name); do not use underscores
