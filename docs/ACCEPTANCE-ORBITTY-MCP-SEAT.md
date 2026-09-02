# ACCEPTANCE — GO-ORBITTY-MCP-SEAT

status: built
created: 2026-08-27
go: GO-ORBITTY-MCP-SEAT 2026-08-27
stamped: no — Matt phrase still required if we stamp this slice separately

- [x] `scripts/write-goose-orbitty-seat.sh` exists and is idempotent
- [x] First run: backup written; `extensions.orbitty.enabled == true`; cmd is `$HOME/.local/bin/orbitty-mcp`; timeout 360
- [x] Second run: still **one** orbitty block; other extensions untouched
- [x] Unreadable yaml: abort, original left in place (or restored from bak)
- [x] `install-stable.sh` calls the helper after installing `orbitty-mcp`
- [x] Snippet doc still works as a manual fallback
- [x] No new Orbitty tools, no TCP, no Berd auto-restart
