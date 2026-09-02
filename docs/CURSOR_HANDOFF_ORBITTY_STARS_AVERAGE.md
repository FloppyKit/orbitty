# Cursor Ultra — GO-ORBITTY-STARS-AVERAGE

**Model:** `kimi-k3-max` · **one writer**
**Gate:** phrase `GO-ORBITTY-STARS-AVERAGE`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_STARS_AVERAGE_LOCK_v0.md` + this handoff

Lookback stays. Calendar stays parked. This GO adds a **range average** under the chip strip.

## Law

1. Switcher week / 14 / month. Default **14**. Persist `stars.averageWindow` only.
2. Rolling calendar window ending at the day cursor. Mean of **saved** days. Unsaved days shrink the denominator. Saved 0 counts.
3. Empty window → `No saves in this window`. Never zero-fill a pentagon.
4. One line under the lookback chips. Per-spoke means, one decimal. Not a new window. Not a second radar.
5. Companion does not recite. Prompts strip untouched. Five spokes stay 0–4. Save today still the only writer of today.
6. Dogfood only `~/Applications/Orbitty.app` after tray Quit.

## Files

`src/lib/StarsPage.svelte` + additive persist on the existing stars slice. Do not touch Sky / habitat / Talk.

## Forbidden

`tauri dev` · `target/` · STAMP_* · calendar grid · companion speech · Sky · second writer.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.
