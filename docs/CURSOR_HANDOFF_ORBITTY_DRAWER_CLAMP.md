# CURSOR HANDOFF — GO-ORBITTY-DRAWER-CLAMP

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_DRAWER_CLAMP_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-DRAWER-CLAMP.md`.

## Do (`src-tauri/src/lib.rs`)

One clamp helper for **every** drawer label (`companion`, `kanban`, `settings`).

- On plucked restore in `show_drawer`: clamp so a **24px handle** stays on `rail_visible_frame`. Fully off → ignore saved bounds; use that label’s **inboard slot**. Companion with no slot → `companion_default_frame`. Never 0,0. Never screen-center.
- Already-visible: if the live frame is off `visibleFrame`, clamp/re-place then show+focus. On-screen stays `set_focus` only.
- Persist (`persist_plucked_bounds`, companion bounds writer, pluck path): skip the write when the live rect is fully off-display.
- Keep docked inboard slots. Keep COVER (no rail-origin, no slide). Keep `startDragging`. Keep STARS sharing kanban. Keep `?mode=` banned.

## Forbidden

Undo RAIL-DRAWER. Middle-of-screen default. SPLIT. SPIRIT-FLOAT. Boombox. `tauri dev`. Stamp. Second writer. Phrase `GO-ORBITTY-COMPANION-CLAMP` is subsumed — do not implement companion-only.

`./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Do not stamp.
