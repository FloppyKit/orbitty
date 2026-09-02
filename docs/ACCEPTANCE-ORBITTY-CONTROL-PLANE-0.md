---
title: "Acceptance — GO-ORBITTY-CONTROL-PLANE-0"
status: stamped
created: 2026-08-27
stamped: 2026-08-27
go: GO-ORBITTY-CONTROL-PLANE-0
owner: pixel-artist
---

# Acceptance — GO-ORBITTY-CONTROL-PLANE-0

Dogfood: Orbitty running from `~/Applications/Orbitty.app`; `orbittyctl` on PATH via `~/.local/bin`.

Stamp: `~/.buzz/PLANS/STAMP_ORBITTY_CONTROL_PLANE_0.md` (2026-08-27).

- [x] `orbittyctl status` succeeds while the app is up
- [x] `orbittyctl windows list` matches rail inventory (same windows the UI sees)
- [x] `orbittyctl companion send -- "ping"` lands like UI send (Berd path unchanged)
- [x] `orbittyctl --json windows list` is machine JSON
- [x] Socket at `~/Library/Application Support/space.orbitty.mac/control.sock` is mode `0600`
- [x] Token missing/wrong (when configured) → exit `3`
- [x] Tool not on allowlist → exit `4` (if `control.json` restricts; default full is ok for first dogfood)
- [x] Orbitty quit → ctl exits `2` with a clear one-line message; no hang
- [x] No new network listener (`lsof` / no `0.0.0.0` bind)
- [x] `./scripts/install-stable.sh` installs both the app and `orbittyctl`
- [x] No MCP binary, no in-app model, no TCP control plane
