# CURSOR HANDOFF — GO-ORBITTY-SPLIT-HABITAT-INPUT

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_HABITAT_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-HABITAT-INPUT.md`

Lift mouse / menus / boombox click into `src-tauri/src/habitat_mac/input.rs`. AUDIO + TICK already shipped. Behavior identical.

## Do

1. New `habitat_mac/input.rs`. `mod input;` from `habitat_mac/mod.rs`.
2. Move hit tests (`planet_for_view`, `hit_being`, `over_disc`, `hit_decor`, `contains_interactive`), mouse/scroll/right-click **bodies**, planet NSMenu builders, walker right-click menu (**including Assign to planet ▸**, `walker_assign`, `WalkerHomeEvent`, parent submenu `Sel::from_ptr(null)`), `boombox_click` / `remove_boombox`, `BOOMBOX_CLICK_SLOP` (14.0), `menu_*` boombox callbacks.
3. **Do not move** `layout_decor` / `set_caption` — sync/panels use them. INPUT calls them.
4. Host keeps `register_world_view_class` / `register_menu_target_class` `add_method` pointing at `input::mouse_down` etc.
5. Picker stays in `audio.rs`. Tick stays in `tick.rs`. `drag_tick` calls `input::boombox_click`.
6. Move GAIN click code **as-is**. Do not retune slop or curve. Do not grow Being editor. Do not strip Image chips.
7. Native never writes claims. Occupancy emit stays `habitat:walker-home`.
8. `cargo check` + `./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`.

## Forbidden

Retune GAIN. AUDIO/TICK extract (already done). Change menus. Move `layout_decor` / `set_caption`. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what moved. Do not stamp. Do not kill Orbitty.
