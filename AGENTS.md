# agents

## before work should be considered complete

1. run checks

```bash
mise run check
```

## visual verification with pilotty

Use `pilotty` to test the plugin renders correctly in a real Zellij session.
Load the `pilotty` skill if available.

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

# 2. spawn a zellij dev session via pilotty
pilotty spawn --name dev-view zellij \
  -s zellij-status-dev \
  --config-dir ./examples/dev \
  --config ./examples/dev/config.kdl \
  -n ./examples/dev/layout.kdl

# 3. wait for it to load (if first run, grant permissions first — see above)
sleep 5

# 4. snapshot the terminal to verify rendering
pilotty snapshot -s dev-view --format text

# 5. clean up
pilotty kill -s dev-view
zellij delete-session zellij-status-dev --force
```

### notes

- `mise run dev` launches the same session interactively (for manual testing)
- `--config-dir ./examples/dev` isolates the session from your user config
- The wasm path in `examples/dev/layout.kdl` uses a hyphen: `zellij-status.wasm`
  (matches the crate name); do not use underscores
