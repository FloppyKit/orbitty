# Cursor Ultra — GO-ORBITTY-STARS-LOOKBACK

**Model:** `kimi-k3-max` · **one writer**
**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-STARS-LOOKBACK`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_STARS_LOCK_v0.md` + `ORBITTY_STARS_BANDS_LOCK_v0.md` + this handoff

Matt 2026-08-28: we need to **log** the STARS they recorded so they can **look at them from the past**.

The log **already exists**. `stars.checkins` is a `BTreeMap<YYYY-MM-DD, StarsCheckin>` written only by **Save today**. The pane is today-only (`todayKey()`, draft, radar aria “Today's STARS”). This GO is a **look** at that map. It does not grow spokes, does not add APIs, does not write a new DB.

Prefer after **stamp STARS**. Legal once this phrase is typed even if stamp is still waiting — the data is already in `state.json`.

## Ship

1. **Day cursor** in `StarsPage.svelte`. Default = local today. Prev / next. Next past today is a no-op (do not invent tomorrow).
2. **Radar + spoke buttons** show that day’s **saved** check-in when one exists. Empty day = blank pentagon / no active 0–4 — **not** a warning, not a sting, not “you skipped.”
3. **Past days are read-only.** Spoke clicks and **Save today** only arm when the cursor is **today**. Footer copy: `Saved for YYYY-MM-DD` on a past save; `No save` on an empty past day; today’s button stays **Save today**.
4. **Saved-day strip** under the radar: the dates that actually have a check-in (newest last, cap ~14 chips, older reachable via prev). Click a chip → that day. No full calendar widget. No sparkline. No 1–7.
5. Persist nothing new. Recompute from `stars.checkins`. Do not persist the lookback cursor.
6. Prompts strip / companion unchanged. They still key off **today**.
7. Hide-on-close still resets the draft to **today** on re-show (already true). Lookback is in-session.

## Files

| File | Job |
|------|-----|
| `src/lib/StarsPage.svelte` | Day cursor, prev/next, saved-day chips, past read-only, radar reads `checkins[cursor]` |
| `src-tauri/src/state.rs` | **No schema change.** `StarsState.checkins` already the log. |

`habitat_stub` n/a. Native habitat **untouched**.

## Forbidden

- New SQLite / event log
- HealthKit / feeds / clerk / weather copy / consider-menu
- Band-color GO, desk motion, sprite fidget
- Editing or deleting past check-ins
- Save writing a day that is not today
- Growing spokes, 1–7, PHI, “you skipped Sleep”
- MCP/ctl `stars.open` / `stars.save`
- Kanban, Talk, marble, wells, STAMP_*
- `tauri dev`

## Dogfood

1. Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant
2. STARS → Save today → prev → that pentagon is yesterday’s save (or empty)
3. Spoke buttons dead on a past day; Save today disabled or no-ops
4. Next back to today → Save still writes today
5. Companion Prompts still only care about today
