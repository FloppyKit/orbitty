# CURSOR HANDOFF — GO-ORBITTY-SPLIT-HABITAT-TICK

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_HABITAT_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-HABITAT-TICK.md`

Lift DisplayLink + ticks into `src-tauri/src/habitat_mac/tick.rs`. AUDIO already shipped (folder exists). Behavior identical.

## Do

1. New `habitat_mac/tick.rs`. `mod tick;` from `habitat_mac/mod.rs`.
2. Move from section `// ---------- CVDisplayLink motion clock` through `persist_now`. Leave `bring_all_front` / `restore_all` on the host.
3. Re-export `pub fn start` and `pub fn persist_now` so `habitat::start` / `habitat::persist_now` still work (`pub use tick::{start, persist_now};` or keep thin wrappers on `mod.rs`).
4. `cursor_tick` **calls** `audio::start_audio` / `live_volume` / `MUSIC_PICKER_OPEN`. Do not move audio.
5. Move GAIN in `drag_tick` **as-is**: boombox Decor does not slide until `moved >= BOOMBOX_CLICK_SLOP` (14.0); mouse-up kind==3 click if `< slop` else persist. `boombox_click` still lives on host until INPUT — tick calls it there.
6. Do not change hitch 1-2-3. Do not change IDLE-STILL bob=0 unless stepping.
7. `cargo check` + `./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`.

## Forbidden

AUDIO extract (already done). INPUT extract. Hitch rewrite. Retune GAIN. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what moved. Do not stamp. Do not kill Orbitty.
