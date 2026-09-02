# Cursor Ultra — GO-ORBITTY-EVENT-LOG

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/goose artifacts/ORBITTY_RUNTIME_AND_WORLD.md` §5
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**Gate:** Matt names `GO-ORBITTY-EVENT-LOG`

## Terminal goal
One append-only Nostr-shaped log on disk. Relay is **not** in the write path. `sig` and `pubkey` empty. Failures to write the log never fail the user action.

## Files
- **new** `src-tauri/src/event_log.rs`
- `src-tauri/src/lib.rs` — `mod event_log`; `generate_handler!` for `log_event`
- `src-tauri/src/coordinator.rs` — append on `wake_sleeper_with`, `run_action` start/end, `companion_send` start/end (best-effort)
- `src/lib/KanbanPage.svelte` — `invoke("log_event", …)` on status change and Needed mint/answer/skip/later
- `src-tauri/src/bin/orbittyctl.rs` + `control.rs` — optional read-only `events.tail` if cheap; skip if it fights the v0 tool list. Prefer skip.

## Envelope (one JSON object per line)

```json
{
  "id": "<32-byte hex or uuid-hex, no 0x>",
  "pubkey": "",
  "created_at": 1730000000,
  "kind": 31200,
  "tags": [
    ["t", "local-only"],
    ["trace", "<uuid>"],
    ["e-type", "board-move"],
    ["project", "<projectId or void>"],
    ["actor", "user|agent|action"]
  ],
  "content": "{\"from\":\"todo\",\"to\":\"doing\",\"cardId\":\"…\"}",
  "sig": ""
}
```

- `created_at` = unix seconds (Nostr).
- `kind` = **31200** for all Orbitty runtime events this GO (one kind; type lives in `e-type` tag). Do not invent a kind per type.
- `t` is `local-only` or `publishable`. Default **local-only**. Nothing is publishable this GO.
- `content` is a JSON object string (Nostr content is a string). Keep it small.
- `id` this GO = sha256 of `created_at|e-type|trace|content` hex, **or** uuid-v4 hex. Pick uuid-v4. Real Nostr id hashing waits for keys.

## Path
`~/Library/Application Support/space.orbitty.mac/events.jsonl`
Reuse `app.path().app_data_dir()` (same as `state.rs` / control.sock). Mode `0600` on create. Append-only. Never rewrite, never delete a line.

Create the file if missing. If the dir is missing, create it the same way `state.rs` does.

## `log_event` command
Args: `{ eType: string, traceId?: string, projectId?: string, actor?: string, localOnly?: boolean, content: object }`
- Fill `created_at`, `id`, empty pubkey/sig, tags, kind.
- If `traceId` missing, mint one and return it so the UI can thread a drag.
- Return `{ id, created_at, traceId }`.
- On IO error: log once, return Ok with `{ skipped: true }` — **never** `Err` that bubbles into save_state / send.

## e-type allowlist (only these this GO)
`board-move` · `needed-open` · `needed-answer` · `needed-skip` · `needed-later` · `claim` · `unclaim` · `sleeper-wake` · `action-start` · `action-end` · `companion-send` · `companion-reply`

Unknown e-type → still write, tag as given. Do not crash.

## Do
1. `event_log.rs` with `append` + the command.
2. Wire the hooks above. Kanban: one invoke per user-visible transition, same tick as `save_state`, not a second debounce.
3. Claim/unclaim: emit from the **native** writer that already mutates `claims` (coordinator / rail invoke), not a second frontend guess.
4. `./scripts/install-stable.sh`. Stop.

## Do not
rusqlite · relay · websocket · TCP · signing · compaction · a new Settings pane · PHI in `content` (titles only, no file bodies, no chat transcripts — companion-send logs **e-type and lengths**, not the line) · `tauri dev` · stamp · CADENCE/VOID/SLEEP in this GO

Companion-send content: `{ "n": <chars> }` only.

## Verify
Blessed app. Move a card Todo → Doing. `tail -1` the jsonl shows kind 31200, e-type board-move, local-only, empty sig. Kill -9 during a move: next event still appends (no gap because of a "start transaction").
