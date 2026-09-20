# GO-ORBITTY-DIET-HABITAT-SPLIT — progress note

Updated: 2026-09-20 CT (~18:40)

## Board stamps (this session)
- **Beach-ball / grass-pad / keep-up / yard clamp** → DONE (green). Shipped in `9d7015e` — Pose::Roam uses yard AABB when beach ball / keep-up is on. *(Orbitty board UI not reachable from this executor; stamp via orbittyctl/kanban on ElM4 when connected.)*
- **GO-ORBITTY-DIET-HABITAT-SPLIT** → keep doing. draw_world peel landed on main.

## Peels done (cumulative)
- walker_menu, planet_menu, mouse.rs ← input
- board_geom, decor_art ← draw
- **draw_world peel ← draw** (this session):
  - `draw.rs` gather only (~1230→~340): draw_rect / DrawData pack
  - `draw_world.rs` paint body (nested via `#[path]` from draw.rs)
  - `draw_data.rs` DrawData/WalkerDraw/DecorDraw + shaded/draw_nsimage
  - `draw_fx.rs` locate/saturn/caption/pixel helpers
  - Commits: `d036029` stub → `459e374` path nest → `77e1d3b` fx → `975fb2f` data → `3395d5f` paint

## Still fat (next peel candidates)
- tick.rs ~881
- mod.rs ~1055
- anim.rs ~807
- coordinator.rs ~3203 = SEPARATE GO (DIET-COORD) — do not start

## Parked leftovers (leave open)
- LOAD-FIRST-FRAME, SMALL-BOARD-CLICK, SPAWN-CENTER

## ElM4 follow-up (this executor could not reach the Mac)
1. Pull main on `/Users/dude/goose artifacts/livesprite-tauri`
2. `cargo check` in src-tauri
3. Quit Orbitty; `export PATH="/opt/homebrew/bin:/Users/dude/.local/bin:$PATH"`; `./scripts/install-stable.sh`
4. Open `~/Applications/Orbitty.app`
5. Green-stamp beach-ball card + update DIET-HABITAT board card via orbittyctl/kanban
