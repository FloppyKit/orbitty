# Cursor Ultra — GO-ORBITTY-MCP-0

**Model:** `kimi-k3-max` if the CLI lists it, else `composer-2.5` · **one writer**
**Gate:** Matt **GO-ORBITTY-MCP-0** via Pixel Artist 2026-08-27
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/.buzz/PLANS/ORBITTY_CONTROL_PLANE_v0.md` + `~/.buzz/PLANS/ORBITTY_NEXT_PATH_v0.md`
**Install:** `./scripts/install-stable.sh` copies `orbitty-mcp` to `~/.local/bin` next to `orbittyctl`
**Control-plane 0 is stamped.** This GO is legal.

**This GO is a thin stdio MCP adapter only.** Same v0 tools as `orbittyctl`. No new product behavior. Do not add an in-app model. Do not bind TCP / 0.0.0.0. Do not speak JSON-RPC on the Unix socket. Do not edit `~/.config/goose/config.yaml`. Do not glue WebMCP or Pixelorama MCP. Do not rebuild PTT / Spark / Jacob.

## Law
1. No AI runtime inside Orbitty. Brains stay Berd/Goose/Cursor.
2. UI and agents share **one** command surface — MCP calls the **same** Unix protocol `orbittyctl` already speaks.
3. Local-first: reuse sock `0600` + optional token + `control.json` ACL. MCP does not invent a second auth.
4. Fail obvious. Orbitty not running → MCP error, no hang.
5. Dock untouched. No PHI. No arbitrary shell. No arbitrary FS.

## Deliverable
1. **`orbitty-mcp` binary** — stdio MCP JSON-RPC 2.0 (Content-Length framing). Pure std + serde_json. Never links Tauri. Never opens a WebView.
2. **`install-stable`** builds, codesigns, copies it to `~/.local/bin/orbitty-mcp`.
3. **Docs snippet** for a Goose/MCP client (copy-paste). Do **not** auto-write Matt’s goose config.
4. **No app behavior change.** Do not touch `CompanionPage.svelte`, `control.rs` tool list, or `Orbitty.app` unless a compile fix is required for the new bin (same Cargo.toml `[[bin]]` pattern as `orbittyctl`).

## Paths
| | |
|--|--|
| Socket | `$ORBITTY_CONTROL_SOCK` or `~/Library/Application Support/space.orbitty.mac/control.sock` |
| Token | `$ORBITTY_CONTROL_TOKEN` or `control.token` next to the sock |
| Binary | `src-tauri/src/bin/orbitty_mcp.rs` → `~/.local/bin/orbitty-mcp` |

Reuse the ctl client recipe: connect UnixStream, 360s read timeout (companion.send can poll Berd 5 min), one newline JSON request, one reply line.

## MCP shape
- Server name: `orbitty`
- Protocol: JSON-RPC 2.0, **Content-Length** headers (official MCP / Goose SDK). Logs on **stderr** only.
- `initialize` → capabilities `{ tools: {} }`, `protocolVersion` `2024-11-05` (or echo a client version we know).
- `notifications/initialized` → ignore.
- `tools/list` → v0 tools, tight JSON Schema, one-line descriptions.
- `tools/call` → map `name` + `arguments` onto Unix `{v:1,id,token?,tool,args}`.
- Resources: none. Prompts: none.

### Unix reply → MCP
- connect fail / not found → JSON-RPC error, message `Orbitty is not running (no control socket: PATH)` (same spirit as ctl exit 2).
- `ok: true` → `tools/call` result `content: [{type:"text", text: pretty JSON of data}]`
- `ok: false` → `isError: true` with `error` text; mention code 3 auth / 4 denied / 5 AX when present.

## v0 tools (only these — 1:1 with control.rs / orbittyctl)

| Tool | Args | Notes |
|------|------|--------|
| `app.status` | {} | |
| `app.accessibility_status` | {} | |
| `windows.list` | {} | |
| `windows.focus` | `{id}` | |
| `windows.minimize` | `{id}` | |
| `windows.restore` | `{id}` | |
| `windows.close` | `{id}` | |
| `companion.open` | {} | |
| `kanban.open` | {} | |
| `stars.open` | {} | |
| `companion.send` | `{line}` | Berd path unchanged |
| `companion.agents` | {} | |
| `companion.set_agent` | `{profile}` | |
| `packs.list` | {} | |
| `sleeper.wake` | `{id}` | |
| `state.get` | {} | |

Unknown MCP tool name → MCP error, do not invent a Unix call.

## Cargo
Keep `default-run = "livesprite"` and the existing `livesprite` + `orbittyctl` bins. Add:

```toml
[[bin]]
name = "orbitty-mcp"
path = "src/bin/orbitty_mcp.rs"
```

Comment: Pure std + serde_json — never links Tauri, never opens a WebView.

`install-stable.sh` already builds `orbittyctl` after the .app. Mirror that block for `orbitty-mcp`.

## Forbidden
- New tools (pack import, soul set, run_action, launch_agent, state.patch)
- JSON-RPC on the Unix socket
- TCP / 0.0.0.0
- In-app model
- WebMCP, Pixelorama MCP, ACP host
- Editing goose `config.yaml`
- PTT / mic plist (separate leftover)
- GO-MH-1 files

## Verify (dogfood, not stamp)
1. Orbitty running from `~/Applications/Orbitty.app`
2. `orbitty-mcp` on PATH; a tiny stdio initialize + `tools/list` shows the v0 names
3. `app.status` / `windows.list` through MCP match `orbittyctl`
4. Tray quit → MCP error, no hang
5. `lsof` — no TCP listener on orbitty-mcp
6. Snippet exists; goose config untouched
