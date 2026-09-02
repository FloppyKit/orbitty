# CURSOR HANDOFF — GO-ORBITTY-BOOMBOX-KILL

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_BOOMBOX_KILL_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-BOOMBOX-KILL.md`.

Almost only `src-tauri/src/habitat_mac.rs`. Parent BOOMBOX CODED 11:56 leftover this punch.

## Do

### A) Quit must kill afplay

`persist_now` (Exit / ExitRequested) today only `display_link_stop` + `save_worlds`. Rust `Child` Drop does **not** kill. Orphan `afplay` keeps the file after tray Quit.

- `persist_now`: `stop_audio` every planet **before** save.
- Habitat teardown / planet Drop: same.
- `start_audio`: `Command` with `.kill_on_drop(true)`. If afplay still orphans, spawn in its own process group and kill the group.
- Click Pause / Remove already call `stop_audio` — if those also leave sound, the kill path is the same bug.

### B) Off + quieter marble

Matt overrode parent “marble quiet, not mute”:

- Fifth menu step **Off = 0.0** with Quiet 0.25 / Low 0.5 / Med 0.75 / High 1.0. Off stops afplay, does **not** fake a silent child. Checkmark Off.
- `zoom_gain(MIN_RADIUS) = 0.0` (marble silent). Still 1.0 at `EXPANDED_RADIUS`. Linear between is fine.
- `live_volume` 0.0 → `stop_audio`, do not spawn `afplay -v 0`.
- Click play/pause stays the obvious on/off. Off-step is the remembered mute so a later zoom-in does not surprise-blast.

### C) Choose-music picker

`NSOpenPanel runModal` from a non-activating planet panel + DisplayLink still ticking + `setAllowedFileTypes` = beachball.

- Pause DisplayLink (and skip boombox respawn in `cursor_tick`) while the panel is up; resume after.
- Drop `setAllowedFileTypes`. Any file; `afplay` silent-fail if it cannot play. Caption stays `Choose music…` on fail.
- Prefer `beginWithCompletionHandler` over `runModal` if the panel still cannot become key. Do **not** hold the habitat lock across the panel.

### D) Install

`./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden

Playlist. Transcode. NSSlider. SPIRIT-FLOAT. Companion drag. STARS 404. Caption. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
