# CURSOR HANDOFF — GO-ORBITTY-BOOMBOX

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_BOOMBOX_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-BOOMBOX.md`.

Almost only `src-tauri/src/habitat_mac.rs`. Persist volume on the world.

## Do

### 1) Click the placed boombox = play/pause

`hit_decor` Boombox on left-click (not a drag start unless the click moves). Toggle `world.boombox` / `planet.boombox`.

- Music set → `start_audio` / `stop_audio`. Caption `Boombox on` / `Boombox off`.
- No file → caption `Choose music…`. Do **not** set playing. Do not spawn afplay.

### 2) Right-click the boombox sprite

Before the planet disc menu, if `hit_decor` is Boombox, pop a boombox NSMenu:

- Play/Pause (disabled if no music)
- Choose music…
- Remove (clears `boombox_pos`, stops audio, playing off)

### 3) Disc menu Place is a live toggle

Delete the dead **Boombox (placed)** disabled item. Disc item is **Place boombox** when none, **Remove boombox** when placed (always enabled). Choose music… stays on the disc menu. Place still uses the existing drop-next-click path.

### 4) Volume + zoom fader

- Persist `world.boomboxVolume` f32 default `1.0`. Four remembered steps, **no NSSlider**: Quiet 0.25 / Low 0.5 / Med 0.75 / High 1.0. Disc menu (and boombox sprite menu): those four, checkmark current.
- Live output = `boomboxVolume * zoomGain(radius)`.
  - `zoomGain`: 1.0 at `EXPANDED_RADIUS`, falls toward ~0.12 at `MIN_RADIUS` (marble quiet, **not** hard mute).
  - Reduce Motion: skip zoomGain (remembered volume only) **or** mute — Pixel lean: skip zoomGain, keep remembered.
- `start_audio` must pass `afplay -v <live>`. If zoom changes while playing, restart afplay at the new live volume (cursor_tick / anim_tick is fine; do not add a second clock). Kill+respawn is ok; do not gap-loop a second child.

### 5) Keep

Formats wav/mp3/mid/midi/aif/aiff/m4a. Silent fail. One boombox per planet. Loop tick in `cursor_tick`. Dim-when-off draw.

### 6) Install

`./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden

Playlist. Transcode. NSSlider. Second scroll target. Settings chrome. Caption pager. SPIRIT-FLOAT. Companion drag. STARS 404. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
