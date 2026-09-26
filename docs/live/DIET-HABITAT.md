# GO-ORBITTY-DIET-HABITAT-SPLIT — progress note

Updated: 2026-09-20 CT (ball_tick + view_class peel)

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
- mod.rs (Habitat/Planet/Being types stay — do not mega-extract)
- anim.rs (walker pose loop)
- coordinator.rs ~3203 = SEPARATE GO (DIET-COORD) — do not start

## Parked leftovers (leave open)
- LOAD-FIRST-FRAME, SMALL-BOARD-CLICK, SPAWN-CENTER

## ElM4 follow-up (this executor could not reach the Mac)
1. Pull main on `/Users/dude/orbitty`
2. `cargo check` in src-tauri
3. Quit Orbitty; `export PATH="/opt/homebrew/bin:/Users/dude/.local/bin:$PATH"`; `./scripts/install-stable.sh`
4. Open `~/Applications/Orbitty.app`
5. Green-stamp beach-ball card + update DIET-HABITAT board card via orbittyctl/kanban

## 2026-09-20 tick peel
- `tick.rs` ~881 → ~358 (DisplayLink + hop + drift)
- `drag_tick.rs` (~375) — panel/decor/walker/boombox/todo-tree drag + persist_position
- `cursor_tick.rs` (~185) — click-through, boombox audio, locate, captions

## 2026-09-20 ball_tick + view_class peel
- `ball_tick.rs` — keep-up hop + last-stretch glance (from anim.rs). Walker footwork stays in anim_tick.
- `view_class.rs` — OrbittyWorldView + OrbittyPlanetMenuTarget registration (from mod.rs).
- Beach-ball / grass pad already green (Matt 2026-09-20). Parked leftovers stay open: LOAD-FIRST-FRAME, SMALL-BOARD-CLICK, SPAWN-CENTER.

## 2026-09-20 mod peel (after Build ball_tick/view_class)
- `mod.rs` ~886 → ~648
- `worlds_io.rs` — worlds.json load/save
- `api.rs` — sync / locate / bring_* / hide / boombox_playing / restore_all

## 2026-09-21 types peel
- `mod.rs` ~648 → ~447
- `types.rs` — Being / Planet / Habitat / PackFrames (pub(crate) fields)
- Drag / Placement / PendingClick stay in mod (enum fields module-private)

## 2026-09-21 anim_planet peel
- `anim.rs` ~697 → ~86 (clock + planet loop shell)
- `anim_planet.rs` — tick_expanded walker/bob/keep-up body

## 2026-09-21 sync_still peel
- `sync.rs` reduce-motion station snap → `sync_still.rs` (`snap_stations`)
- Roam is not moved. Stand still uses the board south lip.

## 2026-09-21 sync_roster peel
- Walker / dot split (sort + park, marbles empty) → `sync_roster.rs` (`classify`)

## 2026-09-21 sync_spawn peel
- First panel for a project not yet on screen → `sync_spawn.rs` (`materialize`)
- Over the marble cap, or a missing world-view class, still skips that planet.

## 2026-09-21 anim_step peel
- One walker's pose step (Sit / Stand / Work / WalkOff / Roam) → `anim_step.rs` (`step_pose`)
- Sleeper camp, mid-drag, and tree chores stay in `anim_planet.rs`

## 2026-09-21 three fat cuts
- WalkOff arm → `anim_walkoff.rs` (`step_walkoff`)
- Marble track assignment → `marble_tracks.rs` (`assign_tracks`)
- Mouse-up → `mouse_up.rs` (`mouse_up_inner`). The extern wrapper stays in `mouse.rs`.
