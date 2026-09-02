# Cursor Ultra — GO-ORBITTY-CONTROL-PLANE-0

**Model:** `kimi-k3-max` if the CLI lists it, else `composer-2.5` · **one writer**  
**Gate:** Matt **GO-ORBITTY-CONTROL-PLANE-0** via Pixel Artist 2026-08-27  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`  
**SoT:** `~/.buzz/PLANS/ORBITTY_CONTROL_PLANE_v0.md`  
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`  
**Think-v9 is stamped.** This GO is legal.

**This GO is CLI + Unix socket only. Do not build MCP. Do not add an in-app model. Do not bind TCP / 0.0.0.0.**

## Law
1. No AI runtime inside Orbitty. Brains stay Berd/Goose/Cursor.
2. UI and agents share **one** command surface — dispatch to the **same functions** the Tauri invoke handlers already call. No shadow behavior.
3. Local-first: socket `0600`, owner = user. Optional token.
4. Fail obvious. Orbitty not running → ctl exit `2` + one line. Auth fail → `3`. Tool denied → `4`. AX/degraded may be `5`. Other → `1`.
5. Dock untouched. No PHI. No arbitrary shell. No arbitrary FS.

## Deliverable
1. **Side-path inside Orbitty.app** — on setup, bind a Unix socket and serve JSON tools.
2. **`orbittyctl` binary** — talks to that socket. Human + `--json`.
3. **install-stable** copies `orbittyctl` onto a documented PATH recipe.
4. **No MCP adapter** (that is `GO-ORBITTY-MCP-0`).

## Paths
| | |
|--|--|
| Socket | `~/Library/Application Support/space.orbitty.mac/control.sock` mode `0600` |
| Token (optional) | same dir `control.token` mode `0600` — required if the file exists **or** `ORBITTY_CONTROL_TOKEN` is set |
| Allowlist (optional) | same dir `control.json` — if missing, local socket client with valid/absent token = **full** |
| App support dir | already used by `state.rs` — reuse it, do not invent a second identifier |

Unlink a stale sock on bind. If bind fails, log once and continue — the .app must still run.

## Protocol (newline JSON, one request → one reply)

Request:
```json
{"v":1,"id":"1","token":"…optional…","tool":"windows.list","args":{}}
```

Reply ok:
```json
{"v":1,"id":"1","ok":true,"data": … }
```

Reply err:
```json
{"v":1,"id":"1","ok":false,"code":2,"error":"Orbitty is not running"}
```

`code` matches ctl exit codes. Keep this tiny; no JSON-RPC framing required.

Prefer **`std::os::unix::net::UnixListener` on a background thread** (clone `AppHandle`). Do not add a second async runtime unless Tauri already forces it and std cannot work.

Auth: if a token is configured, request must present it (field or, for ctl, env/file). Mismatch → code 3. Socket mode 0600 is the other half.

Allowlist v0: `full` | `companion` | `read`. Default **full** when no `control.json`. Denied tool → code 4.
- `read`: `app.status`, `app.accessibility_status`, `windows.list`, `packs.list`, `companion.agents`, `state.get`
- `companion`: read + `companion.*` + `companion.open`
- `full`: v0 tool list below, including destructive window close / sleeper wake / agent set

## v0 tools (only these)

Map to **existing** functions. Do not invent new product behavior.

| Tool | Invoke / fn | Notes |
|------|-------------|--------|
| `app.status` | health: orbitty up + AX summary | new thin wrapper ok |
| `app.accessibility_status` | `accessibility_status` | |
| `windows.list` | `list_windows` | |
| `windows.focus` | `focus_window` | args `{ "id": "…" }` |
| `windows.minimize` | `minimize_window` | `{ "id" }` |
| `windows.restore` | `restore_window` | `{ "id" }` |
| `windows.close` | `close_window` | destructive; full only |
| `companion.open` | `open_companion` | |
| `kanban.open` | `open_kanban` | |
| `stars.open` | `open_stars` | |
| `companion.send` | `coordinator::companion_send` | `{ "line": "…" }` — Berd path **unchanged** |
| `companion.agents` | `companion_agents` | |
| `companion.set_agent` | `set_companion_agent` | `{ "profile": "…" }` |
| `packs.list` | `packs::list_packs` | |
| `sleeper.wake` | `wake_sleeper` | `{ "id": "…" }` full only |
| `state.get` | `load_state` | return as JSON; do not add `state.patch` this GO |

**Not this GO:** pack import/remove, soul set, `run_action`, `launch_agent`, `state.patch`, MCP, TCP, resources.

`app.status` data sketch: `{ "ok": true, "ax": <accessibility_status>, "socket": true }`.

## CLI (`orbittyctl`)

New bin in the same package, e.g. `src-tauri/src/bin/orbittyctl.rs` + `[[bin]]` in `src-tauri/Cargo.toml`. Must **not** launch a second Orbitty / WebView.

```text
orbittyctl status
orbittyctl windows list
orbittyctl windows focus --id <id>
orbittyctl windows minimize --id <id>
orbittyctl windows restore --id <id>
orbittyctl windows close --id <id>
orbittyctl companion open
orbittyctl companion send -- "line"
orbittyctl companion agents
orbittyctl companion set-agent -- <profile>
orbittyctl kanban open
orbittyctl stars open
orbittyctl packs list
orbittyctl sleeper wake --id <id>
orbittyctl state get
orbittyctl --json <same>
```

Token: `--token` flag, else `ORBITTY_CONTROL_TOKEN`, else `control.token` file. Socket path override: `ORBITTY_CONTROL_SOCK` or `--sock`.

Connect fail / no sock → **exit 2**, stderr: `Orbitty is not running` (or equivalent one-liner). Do not hang.

`--json`: print the `data` (or full err object) as JSON on stdout. Default human: compact, readable.

## App wiring
- `lib.rs` `setup`: start the control listener **after** `state::migrate_app_support`.
- On app exit, unlink the sock if we created it (best-effort).
- Never block the UI thread on accept/read.

## install-stable
After copying `Orbitty.app`, also install `orbittyctl`:
- Build produces `src-tauri/target/release/orbittyctl`
- Copy to `$HOME/.local/bin/orbittyctl` (mkdir -p). Document that recipe in the script's Done blurb.
- Do **not** put a second unsigned copy inside a random folder as the dogfood path.

## Out of scope
- MCP (`GO-ORBITTY-MCP-0`)
- CompanionPage / Jacob / thinking PNGs / STARS arcade / PTT
- `save_state` from agents / blind overwrite
- New network listener
- Changing `companion_send` Berd path
- Stamping

## Verify
```bash
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
cd "$HOME/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
# tray Quit, then:
open "$HOME/Applications/Orbitty.app"
# then:
orbittyctl status
orbittyctl windows list
orbittyctl --json windows list
# quit Orbitty from tray, then:
orbittyctl status   # must exit 2
```

Pixel dogfoods. You do not stamp.

Acceptance: `docs/ACCEPTANCE-ORBITTY-CONTROL-PLANE-0.md`
