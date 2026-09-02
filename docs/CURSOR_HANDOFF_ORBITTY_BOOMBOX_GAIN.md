# CURSOR HANDOFF — GO-ORBITTY-BOOMBOX-GAIN

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_BOOMBOX_GAIN_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-BOOMBOX-GAIN.md`.

Almost only `src-tauri/src/habitat_mac.rs`. Parent BOOMBOX-KILL CODED 13:43 leftover this punch.

## Do

### A) Click does not drag

Press-release on the boombox sprite = play/pause only. Do not start planet drag or slide the decor on that press.

- Boombox hit: ignore drag until ~12–16px. Under that = click, sprite stays put.
- Do not move `boombox_pos` on a click.
- Planet drag from empty disc is unchanged.

### B) Wider zoom fader

Quiet/Low/Med/High/Off stay the remembered step. Zoom is the live fader on top.

- `zoom_gain(MIN_RADIUS) = 0.0`. Make sure the **actual marble radius** hits MIN_RADIUS so Off-at-marble is real.
- Expanded (and past EXPANDED_RADIUS): gain **> 1.0** so High × zoom can get slightly louder than today's High-at-expanded. Cap ~1.25.
- Curve Pixel-lean: gain 0 at MIN, ~0.15 at mid, 1.0 at EXPANDED, 1.25 at max_radius.
- Off step still stops afplay. Do not spawn `-v 0`.

## Forbidden

Playlist. Transcode. NSSlider. SPIRIT-FLOAT. Companion drag. PANE-404. Drawers. `tauri dev`. Stamp.

`./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Do not stamp.
