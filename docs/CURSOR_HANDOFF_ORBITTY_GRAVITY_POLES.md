# Cursor Ultra — GO-ORBITTY-GRAVITY-POLES

**Model:** `kimi-k3-max` · **one writer**
**Gate:** **WITHDRAWN 2026-08-28 16:26.** Matt: poles yeeted — planets ARE the sun. **Do not code. Phrase illegal.**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SOLAR_NEST_TALK_v0.md` (Gravity poles) + `ORBITTY_MARBLE_SOLAR_LOCK_v0.md` + this handoff
**Part:** **2 of 2.** Requires **part 1g** `GO-ORBITTY-MARBLE-SOLAR` in the blessed app (large independent tracks around a sun that is not them). **Not ready** on dipole / waltz / host-steal.

User-placed **wells** (product noun: poles / wells). Casual “stars” is talk only — **never** the STARS check-in radar.

Matt 2026-08-28: gravity poles **were the sun to begin with**. You drop one; you drag it; the sun moves. Marbles keep their **own large tracks** around it.

## Why

1g: sun = nearest expanded planet, else frozen barycenter. Part 2: a well **wins as sun** over both. Same kinematic tracks. Expanded never translates. Drag wins. Always orbital — never a radial yank.

If 1g is not in the blessed binary, **stop and say so**. Do not reimplement solar tracks *and* wells in one GO unless Matt stacked the phrases.

## Law

1. **Marble only.** Expanded = still.
2. **Same motion gate as 1g.** `marbleDrift` off **or** `h.reduce_motion` → wells exist but pull nothing. No second toggle.
3. **Drag wins.** Planet drag or well drag: **no** marble translates. After drop, ~2s pin, then may rejoin **its track**.
4. **Move NSPanels.** Wells are tiny native panels, not sprites on one canvas, not a webview.
5. **Noun:** well / pole in UI copy. Never label them STARS. Never write STARS check-ins.
6. **No nest, solar lines, n-body, springs, Kepler, cursor chase, Always-listen, Talk, MCP, ctl, Jacob.**
7. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Place / move / remove

Orbitty does **not** own empty desktop clicks. Do **not** install a full-screen click overlay.

- Settings → Appearance: **Drop well** — one well on the **primary** screen, near center, staggered ~40pt if others exist. Next to **Marbles drift**.
- Optional: planet context menu **Place well here**. Nice; not required if Drop well + drag is enough.
- Well panel **~12pt**, dim disc (not STARS radar, not a star icon). Reuse `make_panel` / `setLevel: 0`. Not a webview.
- Drag the well to move the sun. Right-click → **Remove well**.
- Cap **8** wells total. 9th Drop well is a no-op (or a one-line caption — no modal).

Empty well list is valid. No wells → 1g (expanded, else barycenter).

## Persist

`worlds.json` grows `wells: [{ id, screenId, x, y }]` — screen-local, flipped, same convention as planet `x`/`y`. Additive. Missing key = `[]`. Do **not** put wells in `state.json`. `marbleDrift` stays the only motion switch.

## Motion (reuse 1g tracks)

Well on that screen **is the sun**. Priority: **nearest well** (same screen, hysteresis ≥80pt) **beats** expanded **beats** barycenter.

Each marble: **its own** radius 90–160pt hashed from id, independent phase, mild oval `OVAL_RATIO` 0.88, period 90–140s, even speed. Fake Z = z-order at crossing, **scale 1**. Never a shared stick. Never 24–40pt.

Moving a well moves the sun; tracks follow the well’s center (do not freeze a stale home while the well is being dragged — marbles sit during well-drag, then rejoin).

## Files

| File | Change |
|------|--------|
| `src-tauri/src/habitat_mac.rs` | `Well` + `Habitat.wells`; load/save on `WorldsFile`; tiny NSPanel; drag/remove; `drift_pass` well as sun **using 1g tracks**. |
| `src-tauri/src/habitat_stub.rs` | No-op. |
| `src/lib/SettingsPage.svelte` | Appearance: **Drop well** next to **Marbles drift**. |

No MCP/ctl verbs. No Settings tab.

## Forbidden

Building this before 1g is in the blessed app. Calling wells STARS. Overlay. Default wells. n-body / screensaver / follow-cursor. Orbiting while expanded. Talk / Speech / Handy / MCP / orbittyctl. Linux habitat, Jacob, nest. `tauri dev`. `STAMP_*`.

## Verify (blessed app)

1. No wells + drift on = 1g (wide tracks around expanded, else barycenter). **Not** dipole grapes.
2. Drop well → tiny disc; drag; relaunch → still there.
3. Drift on: marbles on that screen take **independent large** tracks around the well; expanded planet does not move.
4. Drift off **or** Reduce Motion: wells sit, marbles sit.
5. Drag marble or well: motion freezes; click still hits.
6. Remove well: sun gone; leftover follow 1g.
7. Cap 8. `orbittyctl status` unchanged. UI never says STARS.

When done: `./scripts/install-stable.sh`. Summarize. Do **not** write `STAMP_*`.
