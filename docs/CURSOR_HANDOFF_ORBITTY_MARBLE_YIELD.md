# Cursor Ultra / Pixel — GO-ORBITTY-MARBLE-YIELD

**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-MARBLE-YIELD`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_YIELD_LOCK_v0.md`
**Part 1l of 2.** 16ms post-and-return after hitch eyes-on. No wells. 2-width untouched.

## Why hitch hitch remains

2-width KEEP (signed 14:35). Hitch **1, 2, 3, 4, pause** not accepted. Planet **sprites pause in lockstep** — Matt: that’s the tell. DisplayLink already `dispatch_async_f` + `MOTION_HOP_PENDING`. Not enough.

`start()` still **blocks** every 16ms:

```
app.run_on_main_thread(move || tick(n))  // WAITS
```

`tick` = drag + `anim_tick` (walkers, `setNeedsDisplay`) + `cursor_tick` (hit-test **every** panel + 10Hz `save_worlds`). While that owns main, posted `motion_hop_main` sits. Extra vsyncs **drop** (`MOTION_HOP_PENDING`). Gap for marbles **and** walkers.

Matt 14:39: both-smooth is the wish; **walker hitch is legal** if cheaper. Do not a second compositor.

## Do this

1. **16ms thread posts-and-returns.** Same `dispatch_async_f` onto `_dispatch_main_q` as DisplayLink. Forbidden: blocking `run_on_main_thread` from `start()`’s worker. Coalesce with `TICK_HOP_PENDING` (independent of `MOTION_HOP_PENDING`).
2. **Never drop a vsync because a tick is in flight.** Two flags. If something must drop, drop **cursor or anim**, never `drift_pass` / `drift_motion`.
3. **`cursor_tick` must not miss vsyncs.** Pick one (don’t stack): skip `cursor_tick` while `MOTION_HOP_PENDING`; or hit-test/save ~2Hz not 10Hz; or only hit-test when the mouse moved since last tick. Boombox loop can stay cheap. Orbit path never sets `worlds_dirty`. Persist pin/drop/toggle/quit only.
4. CVDisplayLink stays the marble clock. `drift_tick` still no-ops while `DISPLAY_LINK_ACTIVE`. Hosted r formula **do not touch** (`live_radius + 2.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS`).
5. **Walkers:** `anim_tick` may stay on the 16ms tick. Hitching roam/bob is **accepted**. Do not put walkers on DisplayLink. Do not a second view compositor.

Do not add a third clock. Do not CADisplayLink. Do not restore `n % 6` skip. Do not `sleep` inside either callback.

File: `src-tauri/src/habitat_mac.rs` (`start`, `tick`, `cursor_tick`, hop helpers). GCD already bound.

## Untouched

2-width ring. Collider / polar join / z-boolean. Bary 1g. Host 280/360. marbleDrift default false. Scale 1.0. habitat_stub no-op. Reduce Motion stills. No second NSPanel. No wells / poles / STARS / Talk / MCP / Jacob / n-body / springs / Kepler / `tauri dev` / `STAMP_*` / rail house / picker.

When done: `./scripts/install-stable.sh`. Tray Quit, ONLY `~/Applications/Orbitty.app`. Summarize: 16ms post-and-return yes/no, independent coalesce yes/no, cursor cheaper how. **Do not claim hitch is gone — Matt eyes-on.** Walkers may still hitch. Do not write `STAMP_*`.
