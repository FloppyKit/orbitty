# Goose / MCP client snippet — orbitty-mcp

**Preferred:** `scripts/write-goose-orbitty-seat.sh` (also run from `install-stable.sh`).
Idempotent merge into `~/.config/goose/config.yaml`. Backup first. Never replaces the whole file.

This snippet is the **manual fallback** if you are not on the blessed install path.

Binary (blessed): `~/.local/bin/orbitty-mcp`

```yaml
orbitty:
  enabled: true
  type: stdio
  name: orbitty
  description: Drive a running Orbitty over its local control socket (windows, companion, packs, state)
  cmd: /Users/dude/.local/bin/orbitty-mcp
  args: []
  envs: {}
  env_keys: []
  timeout: 360
  cwd: null
  bundled: null
```

If you later add `control.token`:

```yaml
  envs:
    ORBITTY_CONTROL_TOKEN: "…"
```

Optional socket override: `ORBITTY_CONTROL_SOCK`.

Orbitty must be running (`~/Applications/Orbitty.app`). MCP is a thin adapter — it does not start the .app.
