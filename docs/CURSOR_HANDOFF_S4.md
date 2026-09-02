# Cursor Ultra — GO-LS-TAURI-S4

**Model:** kimi-k3-max · ONE writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

**READ (in order):**
1. `docs/ACCEPTANCE-S4.md`
2. `~/.buzz/PLANS/LIVESPRITE_S4_LOCKS_v0.md`
3. `~/.buzz/PLANS/LIVESPRITE_AGENT_ENGAGEMENT_CHAOS_v0.md` §11.2–11.3
4. `~/.buzz/PLANS/LIVESPRITE_THIN_SLICE_LAWS_PIXEL_v0.md`
5. Existing `src-tauri/src/state.rs` + `src/routes/+page.svelte` (additive persist)

## Implement

### Persist (`state.rs` + save/load)
Additive, default-empty so old saves load:
- `kanbanCards[]` as in S4 locks
- `pointsLedger[]`
- `lastTouchedAt` on claims **or** a `claimMeta[{windowId, lastTouchedAt}]` map

### Complete
- Home-claimed rows: **Done** button + confirm
- Build card from window title + appName (titles only)
- `clinicalOrMaster` if any tag name is `clinical` or `master` (ci)
- Points: 2 if clinical/master else 1; 0 if already awarded or daily sum ≥ 12
- Unclaim live window; card stays `done`
- `sync_habitat` after so sprite disappears
- Do not AX-close the window

### Rail UX
- Done section or compact “Today” strip: card title + optional `+1`/`+2`/`cap` chip
- Daily remaining optional, quiet
- **No** score on planet sprites

### lastTouched + zombie
- On focus_window / restore_window / complete / frontmost tick: bump lastTouched
- Habitat snapshot: pass age class `fresh|fade|park|gone` so `habitat_mac` can dim / skip walkers
- 7 / 14 / 21 day thresholds from `lastTouchedAt`
- gone = omit from habitat entries (rail can still list faded)

### Habitat
Minimal: skip `gone`; dim `fade`; treat `park` like marble occupancy if easy. Do not rewrite planet physics.

### Install
`./scripts/install-stable.sh` (Local Codesign → `~/Applications/LiveSprite.app`)

## Forbidden
S5, STARS, coordinator, soul.md, auto-points, advice, PHI, Swift tree, Dock, FKT, planet zoom.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Summarize files + dogfood. **No nest STAMP.**
