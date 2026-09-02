# Cursor Ultra — GO-ORBITTY-MARBLE-GLASS

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-GLASS** via Pixel Artist — **issued 2026-08-28**. Part 1i of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_GLASS_LOCK_v0.md` + `ORBITTY_MARBLE_GLASS_TALK_v0.md` + this handoff

Matt on halo (signed 12:28): **three-width distance is right.** Face-cover, appear/disappear, hitch `1 2 3 4 stop` **not accepted.** Closed 13:02: **track on the planet · crescent · small slow wobble · hitch 100% still there, same GO.**

Do **not** stamp halo. Do **not** relaunch halo. This GO rewrites hosted front + z + hitch. Bary 1g numbers stay.

## Why halo failed

1. **Face.** `host_r` still dives to `HOST_GRAZE_R` ~84pt around a 72pt disc at the equator limbs. That *is* sitting on the face. Three-width front cannot save a path that still kisses the disc.
2. **Pop.** Fake Z restacks **whole NSPanels** by `sin θ` with the host keyed at 0. Crossing the equator coin-flips the marble from fully-in-front to fully-gone. There is no crescent. A non-opaque planet panel already clips whatever is *behind* it to the drawn disc — use that, do not restack the solar system.
3. **Hitch.** Halo took the 10Hz JSON off the 16ms path (~1Hz `last_orbit_save`). Matt 13:02: hitch **still 100%**. The 4-beat was never only JSON. Remaining stall is on the same main-thread hop as `setFrameOrigin`: group `orderFrontRegardless` of every orbiting panel when the `sin θ` sort flips, plus `cursor_tick` (`n % 6`) sharing that hop (hit-test every panel + leftover pretty-JSON). All marbles share that stall — they hitch together because the main thread stalls, not because they share `θ`.

## Picture

> The open planet is a **flat chip**. Sitting on it is a **clear sphere** the size of the chip. The sphere **wobbles** a little, so every once in a while a **corner of the chip sticks out**. Marbles never fly in front of the sphere. They may go behind it (and therefore behind the chip) as a **crescent**. They may graze an exposed chip corner only. The sphere is a **law**, not a drawn bubble. Tracks stay on the **planet**. Three-width standoff stays.

## Law

1. **Marble only** translates under drift. Expanded never moves. Glass never draws.
2. Same gate: `marbleDrift` off **or** `h.reduce_motion` → sit. No second toggle.
3. **Drag wins.** ~2s pin, then ease onto **that marble's** current target — not toward a neighbor.
4. Motion stays on the ~16ms `drift_tick` path once clustered. Do **not** put motion back on `n % 3`.
5. **No wells / poles / Drop well / nest / n-body / springs / velocity / Kepler / mass / screensaver / cursor-chase.** No CADisplayLink this GO unless clock work below still hitches after eyes-on — then stop and say so, do not invent it in the same pass.
6. No Talk / Speech / Handy / MCP / ctl / Jacob / STARS / rail-house / default-on / `tauri dev` / `STAMP_*`.
7. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## A. Hitch — the remaining clock (do this; it is not optional)

Halo's 1Hz save did **not** kill the synced 4-beat. Ordered:

1. **Persist.** Do **not** set `worlds_dirty` from the orbit path at all — not every tick, not ~1Hz. Keep writing live `w.x`/`w.y` in memory. Persist marble live x/y **only** on pin / drop / toggle / quit (`persist_position` / `persist_now` already exist). `cursor_tick` 10Hz `save_worlds` **stays** for scroll-zoom / menu mutations that already set `worlds_dirty`. Kill `last_orbit_save` as an orbit-path dirty gate.
2. **Restack.** Kill the group `sin θ` sort that `orderFrontRegardless`s every orbiting panel (and the host) when the ranking flips. That restack is a hitch **and** the pop. Replace with **boolean** front/back vs **that marble's own host** only (section C). Do not orderFront the whole solar system. Do not orderFront every tick.
3. **Split the hop.** `drift_pass` (setFrameOrigin of drifting marbles) must not share a main-thread turn with `save_worlds` or the full cursor hit-test. Practical: if `n % 6 == 0`, run `cursor_tick` and **skip** `drift_pass` that turn — motion already ran the other five. Or: `cursor_tick` never calls `save_worlds` on a turn that just moved panels; save on the next skipped-motion tick. Pick one. Do not drop motion to 20Hz (`n % 3` for clustered drift **is** a stop).
4. Keep every-tick `drift_pass` once clustered (already), minus the skipped-save tick in (3). No CADisplayLink / n-body / springs. No per-marble clocks.

If hitch remains after 1–3, **stop and say so** in the summary. Do not add CADisplayLink in the same GO.

## B. Hosted track — glass as occlusion, track on the planet

Barycenter (no expanded in range): **do not touch** 1g numbers. `BARY_R` 70–140, `OVAL_RATIO` 0.88, join/leave **220 / 280**, independent phase/period/dir, frozen home, even `dθ`.

Host = expanded planet on that screen. Still: planet sits; independent phase0 / dir / period 90–140s hashed from id; even `dθ`; join ease ~6s ease-in onto **that** marble's target; scale 1.0; drag-pin; host hysteresis ≥80pt; keep-in clamps the **sun** (nudge sun first). Host join/leave **280 / 360** stay.

**Kill `HOST_GRAZE_R` as a path dip onto the face.** Halo's equator dive to ~84pt **is** the remaining face-cover. Front radius stays `HOST_FRONT_R` 198pt (three widths off the 72pt edge). Do not pull front `r` below `EXPANDED_RADIUS + MARBLE_RADIUS + 36` (one width) if keep-in must scale.

**Glass (occlusion mask, not a second sun):**

- One glass per expanded host. Center starts on the planet center. **Wobble** is a slow offset of that center, hashed per host id — **not** followed by the track. Track `r(θ)` / `halo_offset` stays around the **planet** center (plus keep-in sun-nudge of the planet, as today).
- Radius of the glass ≈ planet disc (`EXPANDED_RADIUS` 72, or the world's scroll-zoomed radius if you already have `effective_radius` — match the **drawn** disc, not a guessed 72 if zoomed).
- Wobble amplitude **small**: ~12–24pt (a corner of the 72pt chip can stick out; the face stays covered). Period **slow**: ~20–40s, hashed per host — many marble laps per wobble. Even `dθ` on the wobble too. Invisible. **Never draw it.**
- A marble may sit **in front** of the chip only when its disc does **not** intersect the glass disc. That is the exposed-corner graze. If it intersects the glass, it cannot be in front — it is either beside (front radius already clears) or **behind** (section C).
- Do **not** orbit the wobble. Do **not** lerp the track center onto the glass center.

**Hosted `r(θ)` after this GO:**

- **Front** (boolean front, section C): `r = HOST_FRONT_R` (198). Constant. No graze dip. The only in-front overlap with the **chip** is when glass has wobbled off that corner — geometry of (marble disc ∩ chip) \ glass, not a path dip through the face.
- **Back** (boolean back): `r` may shrink toward `HOST_BACK_R` ≈ 80pt so the silhouette overlaps and the planet disc **clips** a crescent. Blend at the equator so `r` doesn't pop. Keep `HOST_BACK_R` 80.
- No oval squash on the hosted path (`OVAL_RATIO` is bary-only). Drawn scale stays **1.0**.

You may delete `GRAZE_SHARP` / `HOST_GRAZE_R` if unused after the path no longer dips.

## C. Crescent Z — boolean vs host, clip by the disc

Today: sort all orbiting ids by `sin θ`, `orderFrontRegardless` the whole list when it flips, host keyed at 0. That pops **and** hitch-restacks.

Ship, **per marble vs its own host only**:

- `front = sin(θ) > 0` with a small hysteresis band around 0 so it does not chatter at the equator (e.g. switch to back only below −0.08, to front only above +0.08 — pick a deadband, don't chatter).
- **Back:** `orderWindow:relativeTo:` (or equivalent) so **that marble's panel is directly behind that host panel**. The host is non-opaque (`setOpaque: false`, clear background, disc drawn in `drawRect`). Whatever of the marble sits under the drawn disc is clipped to a **crescent**; the rest peeks around the limb. That is the behind. Do **not** `orderOut`. Do **not** hide the marble. Do **not** scale it to 0.
- **Front:** marble panel above its host. Because front `r` is 198, it should clear the glass. If a front marble's disc still intersects the glass (shouldn't, at 198), treat it as back for that tick — glass law wins over front boolean.
- Do **not** restack marbles vs marbles every tick. Marble↔marble pass is allowed only when two marbles actually overlap **and** their front/back vs the host didn't already decide it — and only those two, not the group. Prefer: skip marble↔marble restack this GO if boolean-vs-host is enough.
- `orderFrontRegardless` of the whole habitat is forbidden from `drift_pass`. `bring_all_front` (Home button) stays as today.

Cocoa note: `orderWindow:NSWindowBelow relativeTo:[host windowNumber]` on the marble when back; `NSWindowAbove` when front. Fire **only when that marble's front/back bit flips**, not every tick.

## D. Settings tooltip

Same **Marbles drift** key, default false. Tooltip: when a planet is open, nearby marbles stay about three marble-widths off its face, may tuck behind it as a sliver, and only rarely graze an exposed corner. If none is open, they take slow rings around a quiet midpoint. They do not chase each other. Expanded planets never move. Reduce Motion stills everything.

## Files

- `src-tauri/src/habitat_mac.rs` — `drift_pass` / `halo_offset` / `host_r` / z-order / persist gate / tick split. Constants. Maybe `make_panel` only if you must for stacking (prefer leave it).
- `src/lib/SettingsPage.svelte` — tooltip only.
- `habitat_stub.rs` — no-op.

Do **not** touch Talk / STARS / companion / allowlist / rail house / wells.

When done: `./scripts/install-stable.sh`. Summarize what you changed for hitch (1–3), glass, crescent. Do **not** write `STAMP_*`. Do **not** claim hitch is gone — Matt eyes-on.

## Forbidden

- Drawing the glass / sphere / bubble.
- Orbiting the wobble (track center = glass center).
- `HOST_GRAZE_R` path dip onto the face.
- Group `sin θ` `orderFrontRegardless` of every orbiting panel.
- `worlds_dirty` from the every-tick (or 1Hz) orbit path.
- Dropping clustered motion to `n % 3` / 20Hz.
- CADisplayLink / Kepler / n-body / springs / velocity / mass.
- Wells / poles / Drop well / `wells[]` / STARS / Talk / MCP / Jacob / rail-house.
- `tauri dev` / `target/` dogfood / `STAMP_*`.
- Stamping halo or 1a–1g.
