# CURSOR HANDOFF — GO-ORBITTY-SPLIT-HABITAT-AUDIO

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_HABITAT_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-HABITAT-AUDIO.md`

Lift afplay + picker out of `habitat_mac.rs` into a **folder** so private `Planet` stays crate-private.

GAIN is **STAMPED**. Move the GAIN curve **as-is**. Do not retune. Do not skip this GO for GAIN.

RAIL already shipped.

## Do

1. Replace `src-tauri/src/habitat_mac.rs` with directory `src-tauri/src/habitat_mac/`:
   - `git mv src-tauri/src/habitat_mac.rs src-tauri/src/habitat_mac/mod.rs` (keep blame; do not rewrite the host)
   - add `audio.rs` — the audio slice
2. `habitat.rs` still `mod habitat_mac;`. Public paths `habitat::sync` / `habitat::start` / `habitat::persist_now` unchanged.
3. Move into `audio.rs`: `zoom_gain`, `live_volume`, `kill`/`setpgid`/`SIGKILL`, `start_audio`, `stop_audio`, picker (`MUSIC_PICKER_OPEN`, `MUSIC_PICKER_PANEL`, `music_picker_project`, CompletionBlock, `choose_music_done`, NSOpenPanel begin).
4. Keep GAIN curve: 0 at MIN, ~0.15 mid, 1.0 EXPANDED, 1.25 cap. Off kills afplay. Never spawn `-v 0`.
5. `BOOMBOX_CLICK_SLOP` stays on the host (input constant) until INPUT.
6. `impl Drop for Planet` stays next to Planet in `mod.rs` and **calls** `audio::stop_audio`.
7. Callers (`do_sync`, `cursor_tick`, `display_link_output`, boombox menus) **call** `audio::` — do not move those callers in this GO.
8. `cargo check` + `./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`.

## Forbidden

Retune GAIN. TICK extract. INPUT extract. Restyle. `tauri dev`. Stamp. Second writer. Sibling `src/habitat_audio.rs` (would force `pub Planet`).

Stop when install-stable exits 0. Log what moved. Do not stamp. Do not kill Orbitty.
