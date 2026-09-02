# CURSOR HANDOFF — GO-ORBITTY-ASK-ACTION

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_ASK_ACTION_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/coordinator.rs` — intercept at the top of `companion_send`, **after** empty-check, **before** hermes/berd
- `src/lib/CompanionPage.svelte` — **none** preferred

Prefer all native so every client hits it.

## Do

1. Load `action_shortcuts`. Match in this order:
   - exact `name`, case-insensitive, trimmed
   - whole line equals `command` (trimmed)
   - prefix `run ` / `/run ` / `action ` + name (first token case-insensitive)
2. Match → `run_action(command, cwd)`. Them-line `name: done` / `name: exit N` + clip. **No** Hermes/Berd.
   Pixel lean: slash/prefix → no you-line; bare name as a sentence → keep you-line.
3. Exact name wins when one name contains another. Still two → do **not** run; them-line `which action? use the Launchpad well.`
4. Empty shortcuts / no match → existing send.
5. `please run Build` is **not** a match (not a prefix form).
6. `./scripts/install-stable.sh`; stop.

## Do not

NLP. Dest button. Slash palette. Model-invented shell. Fold TRAY-REMOTE / HOST-CC. `tauri dev`. Stamp.
