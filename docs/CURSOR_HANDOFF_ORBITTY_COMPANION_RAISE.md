# CURSOR HANDOFF — GO-ORBITTY-COMPANION-RAISE

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_COMPANION_RAISE_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. `show_drawer("companion")` heavy-raises (`appkit_order_front`), not `appkit_order_chrome`. Other drawers stay light.

2. `companion_clear`: after show/summon, hold mouse-on while cursor is outside the companion frame. Once cursor enters, existing hit-test. Hosted or expanded: always mouse-on, poll idle. Never arm the ghost poll between collapse and host-on.

3. `set_companion_host` when already hosted: `host_changed(true)` only — no second stash, no re-place.

4. `enterHost`: native host-on before (or with) collapse. Swap still invokes `set_companion_host`.

5. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Gear-dock. Walker. Action edit. Restyle CC. Sprite-off revert.
