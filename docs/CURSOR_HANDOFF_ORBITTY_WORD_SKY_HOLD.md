# Cursor Ultra — GO-ORBITTY-WORD-SKY-HOLD

**Model:** `kimi-k3-max` · **one writer**
**Gate:** waiting Matt phrase `GO-ORBITTY-WORD-SKY-HOLD`. **Do not code until issued.**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_WORD_SKY_HOLD_LOCK_v0.md` + this handoff

Matt 18:07: Sky kind of launches then crashes; Home does not bring it back.

Orbitty did **not** quit. Home is planet panels (`bring_all_front`). Sky reopen is rail **Sky** → `open_sky_window` → `show_labeled("sky")`. The rail `openSky` **swallows** invoke errors. First paint must not hide.

## Ship

1. `show_labeled("sky")` (or a sky-only helper): unhide, `orderFrontRegardless` / equivalent, `set_focus`. If already visible, still front it.
2. If `get_webview_window("sky")` is missing, recreate from `tauri.conf.json` — do not leave the rail click as a silent catch.
3. Rail `openSky`: surface failure or retry recreate; do not empty-catch forever.
4. `ensure_glass` / `pin_stay_visible` must not hide Sky. × and CloseRequested-hide remain the only hides.
5. **Do not** change tokenizer, bag, mute, SkyPage layout, STARS, habitat, Home.

## Forbidden

Bag rewrite. Recs. STARS chrome. `sky.open` ctl/MCP. Home bringing Sky. habitat_mac.rs. Talk. Jacob. brand/. Lookback extras. `tauri dev`. `STAMP_*`.

## Dogfood

Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant → rail **Sky** (not Home). Window stays. × hides. Sky again → same window front. Home still only planets.

When issued: hold the window, install-stable, stop. Do not stamp.
