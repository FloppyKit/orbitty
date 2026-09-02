# Cursor Ultra / Pixel — GO-ORBITTY-MARBLE-HITCH

**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-MARBLE-HITCH`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_HITCH_LOCK_v0.md`
**Part 1k of 2.** Hitch clock + 2-width standoff after rim eyes-on. No wells.

## Why rim hitch remains

Radius KEEP (signed 14:08). Hitch **1, 2, 3, 4, pause** not accepted.

`display_link_output` calls `app.run_on_main_thread(...)` and **waits**. The 16ms thread also hops to main every tick (`tick` = drag / anim / cursor_tick, including 10Hz `save_worlds` + hit-test every panel). When main is busy, vsync callbacks **block or miss**. Missed refreshes = the pause. Glass skipped a beat on purpose; this **misses** a beat.

## Hitch (do this)

1. DisplayLink callback **posts and returns**. Forbidden: blocking `run_on_main_thread` / any wait-for-main from `display_link_output`. Use `dispatch_async(dispatch_get_main_queue(), ^{ drift_motion(n); })` (objc / GCD) or equivalent non-waiting hop. The callback must not sit on the DisplayLink thread waiting for `setFrameOrigin`.
2. **Coalesce.** `AtomicBool` (or equivalent): if a motion hop is already queued or running, skip posting. Prevents a burst of late frames after a stall.
3. Keep CVDisplayLink as the motion clock. 16ms thread must **not** call `drift_pass` / `drift_motion` while `DISPLAY_LINK_ACTIVE`. If create/start fails: 16ms drives `drift_pass` every tick, **never skip**, and that hop must also not deadlock (already on a worker that waits — acceptable on fallback only).
4. Do **not** run `cursor_tick`, `save_worlds`, or hit-test on the DisplayLink callback. Orbit path never sets `worlds_dirty`. Persist pin/drop/toggle/quit only.

Do not add a third clock. Do not CADisplayLink. Do not restore `n % 6` skip. Do not `sleep` inside the callback.

File: `src-tauri/src/habitat_mac.rs` (`display_link_output`, maybe a tiny objc helper). Link CoreVideo already. GCD is libdispatch / Foundation.

## Standoff (same GO)

Hosted circle, live radius kept:

```
r = live_planet.radius + 2.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS
```

Replace the rim `3.0 * (2.0 * MARBLE_RADIUS)` hosted formula (today ~line `live_radius + 3.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS`). Keep-in floor stays `live_r + MARBLE_RADIUS + 2.0 * MARBLE_RADIUS`. Collider / polar join / z-boolean **untouched**.

Settings tooltip: **two** marble-widths off its current size (not three).

## Untouched

Bary 1g numbers. Host join/leave 280/360. marbleDrift default false. Scale 1.0. habitat_stub no-op. Reduce Motion stills. No second NSPanel. No wells / poles / STARS / Talk / MCP / Jacob / n-body / springs / Kepler / scale-throb / dipole / `tauri dev` / `STAMP_*` / rail house / picker.

When done: `./scripts/install-stable.sh`. Tray Quit, ONLY `~/Applications/Orbitty.app`. Summarize: post-and-return yes/no, coalesce yes/no, hosted r formula. **Do not claim hitch is gone — Matt eyes-on.** Do not write `STAMP_*`.
