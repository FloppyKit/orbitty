---
title: "Acceptance — GO-ORBITTY-MCP-0"
status: stamped
created: 2026-08-27
stamped: 2026-08-27
go: GO-ORBITTY-MCP-0
owner: pixel-artist
---

# Acceptance — GO-ORBITTY-MCP-0

Dogfood: Orbitty running from `~/Applications/Orbitty.app`; `orbitty-mcp` on PATH via `~/.local/bin`. Control-plane 0 already stamped. **Stamped 2026-08-27** (`STAMP_ORBITTY_MCP_0.md`).

- [x] MCP client `tools/list` matches the v0 ctl tool names (Goose camelCase)
- [x] `app.status` / `windows.list` through MCP match `orbittyctl`
- [x] `companion.send` through MCP is the same Berd path as ctl / UI
- [x] Orbitty quit → MCP error, no hang
- [x] `./scripts/install-stable.sh` installs `orbitty-mcp` next to `orbittyctl`
- [x] No TCP listener, no JSON-RPC on the Unix socket, no in-app model
- [x] Goose seat: later `GO-ORBITTY-MCP-SEAT` (idempotent merge). Snippet remains the manual fallback (`docs/ORBITTY_MCP_GOOSE_SNIPPET.md`)
- [x] No new tools beyond the stamped control-plane v0 list
