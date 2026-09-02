# LOG — GO-ORBITTY-BOARD-STARS (writer run 2026-08-30)

Sole writer run against `docs/CURSOR_HANDOFF_ORBITTY_BOARD_STARS.md` +
`docs/ACCEPTANCE-ORBITTY-BOARD-STARS.md`. SoT: Pixel lock
(`~/.buzz/PLANS/ORBITTY_BOARD_STARS_SHARE_LOCK_v0.md`) — two rail buttons,
one 780×520 overlay (label stays `kanban`), STARS padded. Not a stamp.

## What changed

### Native (`src-tauri/src`)

- `state.rs` — new additive slice `sharedPaneMode` ("board" | "stars",
  default "board") on `PersistedState` (+ Default impl + header doc). Old
  saves load (serde default); every other window round-trips it.
- `lib.rs` — new `pub(crate) fn show_shared_pane(app, mode)`:
  1. hides a live legacy `stars` webview if one exists (hide-not-destroy;
     nothing spawns it this GO),
  2. persists `sharedPaneMode` BEFORE the show (load-modify-save +
     `state:changed`),
  3. `show_labeled(app, "kanban")` — the one 780×520 overlay,
  4. `emit_to("kanban", "orbitty:shared-pane-mode", mode)` so a warm
     webview swaps pages live.
- `lib.rs` — `open_kanban` → `show_shared_pane("board")`;
  `open_stars` → `show_shared_pane("stars")` (no more 420×560 `stars`
  pane on click). Doc comments updated.
- `lib.rs` — `ensure_window`: for label `kanban` the create-time URL is
  `index.html?mode=<persisted>` so a cold webview mounts the right page
  with zero flash (tauri's protocol handler strips the query for asset
  lookup — verified in tauri 2.11.5 `protocol/tauri.rs`).
- `lib.rs` — tray "Lists" left as a bare `show_labeled("kanban")`: a
  mode-less raise, so reopen restores the last mode.
- `control.rs` — `kanban.open` / `stars.open` route through
  `show_shared_pane` (board / stars). Neither can spawn the legacy stars
  window anymore.
- `bin/orbitty_mcp.rs` — tool descriptions updated ("Show the shared pane
  in Board/STARS mode").

### Frontend (`src`)

- `routes/+page.svelte`
  - `PersistedState` gains optional `sharedPaneMode`.
  - Kanban window: `paneMode` state seeded from `?mode=` URL; listens for
    `orbitty:shared-pane-mode` (warm swaps); one-shot `load_state`
    reconcile on mount closes any create/load race; unlisten on destroy.
  - Mount block: `kanban` window mounts `StarsPage hideLabel="kanban"`
    when `paneMode === "stars"`, else `KanbanPage`. The `stars`-label
    branch stays (dead this GO, harmless).
  - Rail stack still three buttons (Companion · Board · STARS); Board and
    STARS titles now say "shared pane in … mode". `openKanban` /
    `openStars` / `openKanbanFocus` comments updated — same invokes, new
    native behavior.
- `lib/StarsPage.svelte`
  - New `hideLabel` prop (default `"stars"`); the × hides the HOST window.
  - Re-show reset (draft, cursor→today, reload) now driven by the native
    `orbitty:pane-vis` event on the host label (hide-not-destroy panes
    never flip `document.hidden`; the visibilitychange listener stays as
    backup).
  - Check-in markup: radar + 0–4 rows wrapped in `.checkin-grid`; body
    gets `class:review`.
  - Wide layout (`min-width: 640px`): letterboxed column, radar 224px
    square SVG (honest pentagon, not squashed), Check-in puts 0–4 rows
    beside the radar, Review puts the Cloud beside the mean panel (cloud
    flexes to full row height). Check-in | Review segment unchanged.

## Untouched on purpose

Companion window (own window, show/hide), `sky` window (hide-not-destroy,
own label), Cloud inside STARS Review, `pane_spec` sizes (kanban stays
780×520), tray menu, WORDMARK, drawers. No `tauri dev` used.

## Verify

- `npm run check`: 0 errors (33 pre-existing warnings, none new).
- `cargo check`: clean (27 pre-existing warnings, none from this GO).
- `npm run build`: ok.
- `./scripts/install-stable.sh`: **exit 0** (252s). Signed with the stable
  "LiveSprite Local Codesign" identity (no ad-hoc fallback), `--verify
  --deep --strict` clean on the installed copy, installed to
  `~/Applications/Orbitty.app`; `orbittyctl` + `orbitty-mcp` rebuilt and
  installed to `~/.local/bin`; Goose seat merge ok. No `tauri dev` at any
  point.

Dogfood path left to the human (acceptance doc): tray Quit → open only
`~/Applications/Orbitty.app` → AX re-grant → Settings Dark; then Board ↔
STARS swap in the one overlay, radar padded, reopen remembers last mode.
