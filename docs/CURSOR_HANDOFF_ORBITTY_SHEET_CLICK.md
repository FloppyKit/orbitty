# CURSOR HANDOFF — GO-ORBITTY-SHEET-CLICK

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_SHEET_CLICK_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-SHEET-CLICK.md`.

CLAMP (17:26) left a transparent companion on the rail and Lists `on=0`. This punch is the leftover, not a re-clamp rewrite.

## Do (`src-tauri/src/lib.rs`)

### Clamp (reuse the CLAMP helpers; change what they measure)

- `drawer_visible_frame` / clamp predicates: **union of every display's `visibleFrame`**, in **Tauri physical** (same as `outer_position` / `set_position`). Stop using rail-only vf for "is this pane on-screen."
- `clamp_drawer_rect`: 24px handle on **some** display. Fully off **all** displays → `None` (slot fallback). Never 0,0. Never screen-center.
- Saved bounds: if fully off in physical, retry as **points**. Honor the interpretation that has a 24px handle. Persist the physical rect you placed.
- After place: if the drawer rect intersects the **rail** window by more than a 1px edge → slot fallback (`drawer_slot_fallback` / companion `companion_default_frame`).
- Persist skip still: never write a fully-off-all-displays frame.

### Mouse (COVER leftover)

- `ensure_window`: create with `set_ignore_cursor_events(true)`.
- `show_drawer` / `show_labeled`: **remove** the re-enable at the top. Place, `show`, then re-enable **only** if live rect has a 24px handle on some display and does not cover the rail. Otherwise `hide` + click-through.
- Keep nav-guard hide + `set_ignore_cursor_events(true)` on bad URL / non-2xx.

### Front

- After show+focus: if still fully off all displays or not visible → slot fallback + show again; if still off, hide + click-through.
- Already-visible on-screen not covering rail → `set_focus` only. Off or covering rail → re-place then show+focus.

Keep docked inboard slots, COVER (no rail-origin, no slide), `startDragging`, STARS-shares-kanban, `?mode=` ban, hide-not-destroy, `pin_stay_visible` on non-companion. Do not edit `state.json`. Do not unpin.

## Forbidden

Undo RAIL-DRAWER. Screen-center. Re-issue DRAWER-CLAMP. SPLIT. SPIRIT-FLOAT. Boombox. Fake titlebar. `tauri dev`. Stamp. Second writer.

`./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Do not stamp.
