# CURSOR HANDOFF — GO-ORBITTY-IDLE-STILL

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_IDLE_STILL_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-IDLE-STILL.md`.

## Do

### 1) Head sits — `src/lib/CompanionPage.svelte` CSS

Remove `bustfloat` from `.floatwrap.headview`. Collapsed Head must not translate. Keep:

- `.spirit.react` / think-v9 (send still yaws once)
- `.exphead .floatwrap` Full-figure float (not this GO)
- Reduce Motion already parks floats — tidy the Head rule if it becomes dead

Do not restyle Head crop, portrait size, or the Full/Head control.

### 2) Walkers stand — `src-tauri/src/habitat_mac.rs`

In `anim_tick`, the sin bob (`* 1.4`) currently hits every non-asleep walker, including `pause_end` stands.

- `bob = 0.0` unless the walker is **stepping** this tick (`pause_end` is `None` and `dist >= 0.025`).
- Sleepers stay `0.0`.
- `pack_frame_ptr` paused branch: freeze `pack_frame_still` (idle[0] else walk[0]). Do not cycle idle while standing. Moving branch unchanged.

Marbles / caps / DisplayLink / roam pause durations unchanged.

### 3) Install

`./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden

Caption pager. WORDMARK. Walker teleport. Boombox. Spirit Clock. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
