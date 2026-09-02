# CURSOR HANDOFF — GO-ORBITTY-COMPANION-CLAMP

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_COMPANION_CLAMP_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-COMPANION-CLAMP.md`.

## Do (`src-tauri/src/lib.rs` `show_companion` + persist)

- Clamp restored `companion.bounds` so a **24px handle stays on the current monitor** (`visibleFrame`).
- If the saved rect is **fully off** the current display, ignore it and use the trailing-edge first-show default.
- Do **not** persist an off-display frame (skip the throttle-write when the live rect is off).
- Keep `startDragging` unless it still jumps after clamp. Do not invent a fake titlebar.

## Forbidden

PANE-404. SPIRIT-FLOAT. Boombox. Drawers. `tauri dev`. Stamp. Second writer.

`./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Do not stamp.
