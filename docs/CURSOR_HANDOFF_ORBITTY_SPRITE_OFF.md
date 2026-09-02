# CURSOR HANDOFF — GO-ORBITTY-SPRITE-OFF

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_SPRITE_OFF_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. Persist `companion.sprite` (bool, default true). Command `set_companion_sprite` like `set_companion_speak`. Register in `lib.rs`. Default impl + serde camelCase.

2. CompanionPage: load the flag with state. R-click `.viewbtn` → toggle (preventDefault). L-click stays Full ↔ Head. Button stays clickable when sprite is off.

3. Sprite off:
   - collapsed idle: no `spiritStage` / `.stage`. Ghost hits omit `.floatwrap`.
   - expanded: no `.exphead`.
   - hosted: no `.wellhead`; `.wellpane` full width (no right border).

4. Do not auto-shrink. Do not change click-through on expanded/hosted.

5. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Mute. L-click cycle. Menu popover. Walker. Action edit. PNG sampling.
