# CURSOR HANDOFF — GO-ORBITTY-DOCK-TWEAK

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_DOCK_TWEAK_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. `.dest` fill = `var(--panel-fill, var(--panel))` like `.expandbtn`. Keep ring, radius, hover chip, `.dest.on`.

2. `.viewbtn`: `onclick={toggleSprite}`; `oncontextmenu` preventDefault + `toggleView`. Title: L-click hide/show sprite, R-click Full/Head. Stay enabled while sprite is off.

3. `./scripts/install-stable.sh` then git commit `-m "GO-ORBITTY-DOCK-TWEAK"`. Do not push. STOP.

## Do not

`tauri dev`. Stamp. Rust. Fold CAPTION-CHUNK / INSPECTOR-HOST.
