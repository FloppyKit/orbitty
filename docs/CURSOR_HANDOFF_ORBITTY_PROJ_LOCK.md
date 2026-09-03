# CURSOR HANDOFF — GO-ORBITTY-PROJ-LOCK

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_PROJ_LOCK_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. `locked_projection(is_marble)` → Flat if marble, Oblique if expanded. Use it on planet create and every sync update. Do not copy `WorldState.projection()` onto the live planet.

2. Disc menu: delete **2D disc** / **2.5D** (and their separator). Leave Minimize / Restore / Place / boombox. Delete `menu_set_flat` / `menu_set_oblique` and the `planetSetFlat:` / `planetSetOblique:` methods.

3. Optional: rewrite `worlds.json` `projection` to the derived string when it disagrees so old `"flat"` expanded saves heal. Never a user toggle.

4. `./scripts/install-stable.sh` then git commit `-m "GO-ORBITTY-PROJ-LOCK"`. Do not push. STOP.

## Do not

`tauri dev`. Stamp. Push. CADENCE-EXIT. Station rewrite. CompanionPage. CLAIM-ONE.
