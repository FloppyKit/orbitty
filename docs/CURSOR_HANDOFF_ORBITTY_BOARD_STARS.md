# CURSOR HANDOFF — GO-ORBITTY-BOARD-STARS

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_BOARD_STARS_SHARE_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-BOARD-STARS.md`.

## Do
1. Two rail buttons (Board · STARS), **one** overlay. Host size **780×520** (today’s Board). Label stays `kanban`. When the rail hits STARS, mount `StarsPage` inside that same webview (route/flag). When Board, mount `KanbanPage`. Persist last mode (`board` | `stars`) so reopen restores it.
2. `open_stars` → show the same `kanban` window in stars mode (do not create a second 420×560 `stars` pane this GO). Existing `stars` webview: hide-not-destroy if already live; do not spawn it on STARS click.
3. STARS **pads** inside 780×520 — radar honest, not squashed. Extra field / letterbox OK. Check-in | Review from RAIL-REVIEW stays.
4. Companion stays its own window. `sky` hide-not-destroy. Cloud stays inside STARS Review.
5. `+page.svelte` stack: Companion · Board · STARS still three buttons. Board and STARS toggle the shared pane; they do not both create overlays.
6. `./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden
Merge Companion. Destroy sky. Compositor. Drawer animation. Caption punch. WORDMARK. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
