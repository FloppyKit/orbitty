# Architecture

Map of how data moves. Names are the ones in the tree. This file does not change behaviour.

## UI

`src/routes/+page.svelte` reads the Tauri window label and mounts one page from `src/lib/`. The five desks are `RailPage`, `KanbanPage`, `StarsPage`, `CompanionPage`, and `SettingsPage`. The same route also mounts `SkyPage`, `YardWarnPage`, `NextCard`, `CheatSheet`, and `LinkPreviewPopover` for the other labels. A drawer webview mounts only its own page.

## Coordinator

`src-tauri/src/coordinator/`. `send.rs` is the companion send path. It talks to hermes, berd, goose, and grok. The module doc on `mod.rs` says titles-only context and no LLM here.

## State

`src-tauri/src/state/io.rs` is the save door: `load_state` and `save_state` read and write `state.json` under the app data dir. The board path that emits `state:changed` is `board::save_with_events` in `src-tauri/src/board/board.rs`.

## Relay

`src-tauri/src/relay/event_log.rs` appends `events.jsonl`. A log failure does not fail the click. `event_keys.rs` signs. `event_relay.rs` publishes a best-effort EVENT to `ws://127.0.0.1:13120`.

## MCP and the control socket

The MCP bin is `src-tauri/src/bin/orbitty_mcp.rs`. The socket server is `src-tauri/src/agents/control.rs`. The client is `src-tauri/src/bin/orbittyctl/` (`imp::run`).

## Mouth sidecar

`sidecar/mouth` is already its own process. `serve.py` binds `127.0.0.1:13121`.

## Invoke list

`src-tauri/src/lib.rs` holds `generate_handler!` (the invoke list, opened near line 751) and setup. `src-tauri/src/main.rs` calls `orbitty_lib::run`.
