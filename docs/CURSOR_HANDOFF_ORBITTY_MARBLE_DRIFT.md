# Cursor Ultra — GO-ORBITTY-MARBLE-DRIFT

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-DRIFT** via Pixel Artist — **issued 2026-08-27**. Code this. Part 1 of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SOLAR_NEST_TALK_v0.md` (Marble fidget) + this handoff
**Part:** **1 of 2.** Part 2 is `GO-ORBITTY-GRAVITY-POLES` (wells). Do not build wells here.

Calming desktop fidget. Collapsed planets (~18pt marbles) may slowly drift near each other. Expanded planets do not move. Drag wins.

## Why now

`anim_tick` in `habitat_mac.rs` already skips marbles (`if planet.is_marble { continue }`). Marbles are still. Matt asked for a mild proximity drift, off unless toggled. Wells (user-placed poles) are a **second** GO after this dogfoods.

## Law

1. **Marble only.** `Planet.is_marble == true`. Expanded panels never translate from this GO (in-place paint-band spin already exists — do not change it).
2. **Off unless toggled.** Default **false**. Settings → Appearance: **Marbles drift**.
3. **Drag wins.** While `h.drag` is set, **no** marble translates. After mouse-up, the dropped marble sits ~2s, then may rejoin. Click targets must not run away.
4. **`accessibilityDisplayShouldReduceMotion` still.** Reuse `h.reduce_motion`. Also still if the toggle is off.
5. **Move NSPanels, not a canvas.** Each marble stays its own window. Slow. Never feel haunted.
6. **No wells / poles / nest / solar lines / n-body toy / cursor chase.**
7. **No STARS, Talk, MCP, ctl, Jacob, Spark, companion.** No `settings.open` tool.
8. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Shape

**Cluster:** marbles on the **same screen** whose centers are within **~220pt** of at least one neighbor. Isolates sit. A cluster slowly orbits its centroid on a **small** ellipse (radius ~12–28pt, period **~60–90s**). Cap speed so a click still hits the disc (`MARBLE_RADIUS` is 18).

**Clamp:** stay on that screen’s visible frame (padding ≥ marble radius + 8). Do not drift onto another display.

**Persist:** write `worlds.json` `x`/`y` through the existing dirty path (`worlds_dirty` → `cursor_tick` ~10Hz `save_worlds`). Do **not** save every anim frame. Drag-end still `persist_position`.

**Tick:** extend `anim_tick` (already dt-capped). Do not add a second timer.

## Files (expected)

| File | Change |
|------|--------|
| `src-tauri/src/state.rs` | Additive `marble_drift: bool` default **false**. Old saves load. |
| `src-tauri/src/habitat.rs` | Snapshot may pass the flag, **or** habitat reads it from state — one path, not two sources of truth. |
| `src-tauri/src/habitat_mac.rs` | `anim_tick`: if toggle && !reduce_motion && is_marble && not-in-drag && not-in-pin-window → translate panel; clamp; mark worlds dirty. Expanded / reduce / toggle-off = today’s still marbles. |
| `src-tauri/src/habitat_stub.rs` | No-op (macOS habitat only). |
| `src/lib/SettingsPage.svelte` | Appearance: **Marbles drift** mini-toggle next to row chrome. Persist via existing load/save state. |
| `src/routes/+page.svelte` | If the rail already pushes habitat snapshots, include the flag **or** let native read state — do not desync. |

Do not invent a second JSON file. `worlds.json` stays positions; `state.json` stays the toggle.

## Forbidden

- Gravity wells / poles / “stars” on the desktop
- Nest / sun+moons / drawn ellipses
- Orbiting while expanded
- Screensaver / n-body / follow-cursor
- Default-on
- Ignoring Reduce Motion
- MCP / orbittyctl tools
- Linux habitat
- Touching Talk / Speech.framework / Handy
- `tauri dev` / `target/` dogfood

## Verify (blessed app)

1. Toggle off (default): marbles stay where you left them.
2. Toggle on, two+ nearby marbles, Reduce Motion **off**: they drift slowly; isolates sit; expanded planet does not translate.
3. Drag a marble: everything freezes; drop: it sits, then may drift; you can still click it.
4. System Settings → Reduce Motion: still, even if toggle on.
5. Quit/relaunch: toggle + positions persist.
6. `orbittyctl status` still AX trusted. No new ctl verbs.

Acceptance: `docs/ACCEPTANCE-ORBITTY-MARBLE-DRIFT.md`

When done: `./scripts/install-stable.sh`. Summarize files + dogfood. Do **not** write STAMP_* (Pixel stamps after Matt eyes-on). AX re-grant expected.
