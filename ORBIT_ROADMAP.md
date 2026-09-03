# Orbit Roadmap — 2026-09-02 night

**Open this file in a new session.** That session’s job is to **hand GOs to Cursor Ultra `kimi-k3-max`**, not to implement in Grok. Grok budget is the thing we are saving.

Product: **Orbitty** · tree `~/goose artifacts/livesprite-tauri` · blessed app `~/Applications/Orbitty.app` · id `space.orbitty.mac`

Coder: **one writer** — Cursor Ultra `--model kimi-k3-max`.  
Install: `./scripts/install-stable.sh` then STOP. **No** `tauri dev`. **No** stamp. **No** `git push` unless Matt says.

---

## Where we just landed

Cleanup **code** is in git (local `main`, **26 commits ahead**, not pushed):

| Slice | Status |
|---|---|
| CLEAN-DEAD / TYPES / STAGE / KANBAN-WELLS / HABITAT-DRAW | Coded |
| CLEAN-COMPANION-CAPTION | Coded 2026-09-02 (plus crash-mount leftover commit) |
| CLEAN-COMPANION-DOCK | **CODED** `58004b5` waiting dogfood. `CompanionDock.svelte` exists. 3D leftover not touched. |
| HOST-DOCK leftover | Coded, papers still say waiting dogfood |

**3D leftover (banked):** `GO-ORBITTY-COMPANION-3D` `1290580` + `GO-ORBITTY-COMPANION-PLAY` `710361a`. Matt dogfood: they kind of animate; think is a little head shake; **not breathing**; **T-pose floating**. “Might have to figure out something else.” Do not stamp. Later product punch — not Mixamo in a tidy.

**FRONT-HOLD leftover (bounce 2026-09-03):** DOCK dogfood — dock fell behind planets, ⌘⇧O / Show Companion did not recover, beach balls in planet/walker menus and the agent picker. Grok punched (hard bounce, not a new leftover). CHROME-Z now keys off workspace frontmost (not `isActive`); summon hold does not spend on first-Outside; planet menus pause DisplayLink; `companion_front` skips native `<select>`.

Tree **clear** after that punch + `install-stable`. Identifier `space.orbitty.mac`.

---

## Next (dogfood DOCK, then hologram cascade — WAIT_GO)

Dogfood first: tray Quit → only `~/Applications/Orbitty.app`.

- Launchpad / STARS / Planets dests still toggle the well. Chip still says **Planets**.
- ⚙ last after Expand; opens Settings.
- Composer send / Ask-Action still work.
- T-pose will still T-pose until **#2** of the holo cascade.

Holo cascade: `~/.buzz/PLANS/CASCADE_ORBITTY_COMPANION_HOLO_v0.md`. **#1 ISSUED** 2026-09-03. WAIT_EXTERNAL Mini CoS. Do not poll. Mac tree stays clear.

| # | Phrase | Who | Gate |
|---|--------|-----|------|
| 1 | `GO-ORBITTY-COS-REIDLE` | CoS Mini | **ISSUED.** INBOX `orbitty-companion-reidle-v0`. OUTBOX DROPPED + no-pelvis idle. |
| 2 | `GO-ORBITTY-COMPANION-REIDLE` | Cursor Ultra | #1 drop + tree clear + Matt phrases |
| 3 | `GO-ORBITTY-COMPANION-HOLO` | Cursor Ultra | #2 dogfood |
| 4 | `GO-ORBITTY-COMPANION-HOLO-SPEAK` | Cursor Ultra | #3 dogfood |

Do not invent Mixamo. Do not skip to HOLO while they still bounce.

---

## After that (not same-breath)

| Order | Phrase | When |
|---|---|---|
| waiting | `GO-ORBITTY-CLAIM-ONE` | Papers REAL. Different files. Still needs Matt phrase + tree clear. |
| later | Herding Cats | `GO-ORBITTY-YARD-WARN` first. WAIT_GO. |
| later | CADENCE-EXIT | Done walkers still yoyo. World-runner. |
| later | chrome.css / inspector quiet | Professionalize talk. |
| out | desktop roam / visemes / fork B mesh | Later cascades. |

Cleanup cascade terminal: DOCK was #5; WELLS + DRAW already coded earlier. Do not invent #8.

---

## Hard laws for the Cursor session

1. One phrase. Tree clear. No second writer on `CompanionPage.svelte`.
2. Blessed `~/Applications/Orbitty.app` only.
3. Local commit only. **Do not push.**
4. Identifier `space.orbitty.mac`. `clinical_claim: false`.
5. Do not default off Spirit. Do not Unity. Do not load `three` on the rail.

---

## Copies of this file

- `~/ORBIT_ROADMAP.md` ← **start here**
- `~/goose artifacts/ORBIT_ROADMAP.md`
- `~/goose artifacts/livesprite-tauri/ORBIT_ROADMAP.md`
- `~/.buzz/PLANS/ORBIT_ROADMAP.md`
