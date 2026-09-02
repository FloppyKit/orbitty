# CURSOR HANDOFF — GO-ORBITTY-HOST-FIT

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_HOST_FIT_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/panes.rs` — `set_companion_host` grow path.
- `src-tauri/src/drawers.rs` — helper only if vf query stays here.

## Do

1. Replace the hardcoded `w: 1080.0, h: 620.0` grow with a picker: display the companion is mostly on (`rect_mostly_on`, else first screen). `vf.w >= vf.h` → 1080×620. `vf.h > vf.w` → 620×1080. No screens → 1080×620.

2. Keep origin at live `cur`, then `clamp_on_displays` + `place_logical_rect`. Stash / restore / persist-idle-only unchanged.

3. Re-apply on every host-on (Launchpad ↔ STARS already re-grows — now it re-fits too).

4. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Persist host frame. Auto-orient idle. Wire Moved / NSApplicationDidChangeScreen. Touch CompanionPage. Fold action-edit / CLEAR / HOST-VIEW.
