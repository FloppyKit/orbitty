# Cursor Ultra — GO-ORBITTY-WORD-SKY

**Model:** `kimi-k3-max` · **one writer**
**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-WORD-SKY`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_WORD_CONSTELLATION_LOCK_v0.md` + this handoff

**This GO is a derived curiosity map of the user’s own words.** Not STARS. Not gravity wells. Not recs. Not XP. Not a new DB.

## Why

Matt 2026-08-27: scoring = what you did + optional coarse time. Kill XP. Kinds are not planets and not a homework taxonomy — they fall out of words already written. v0 is the sky. Nearby-glow recs are a later GO.

## Law

1. **Derived view.** Do not store a `kind` object. Do not invent cluster names (“Craft”). Label = the token itself.
2. **Bag only:** Kanban **done** titles + card notes + tag **names** + planet/project name as a **weak** extra (half weight) + **user** companion `turns` (`role=user` / lastUserLine). **Never** bot replies.
3. **No scrape.** No calendar, git, window titles, dwell minutes.
4. **Bubble size = presence count**, not minutes, not points.
5. **Empty sky is valid.** Words that stop appearing **dim**. Never red. Never “you used to.”
6. **Opt-in.** Default closed. Mute list persists. Smash a word = hide it.
7. **Noun:** Sky / Words. Chrome must not say STARS. Must not look like the check-in radar or a well.
8. **No recs copy.** No “considering…”. No short-leg warning. That’s later GOs.
9. Local JSON only. **No SQLite.** No MCP/ctl verbs. No Talk/Handy. No Jacob.
10. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Shape

New webview, same SPA pattern as `settings` / `stars`:

- `tauri.conf.json` window label **`sky`**, ~420×560, transparent overlay, hiddenTitle, visible false.
- `capabilities/default.json` add `"sky"`.
- `show_labeled("sky")` + `open_sky_window`. Hide-on-close, never quit. Pin like stars.
- Rail: a small **Sky** control near STARS/Settings — **not** replacing STARS. If the rail is too tight, Settings → Appearance **Open sky** is enough for v0 (prefer a chrome button if one slot exists).
- **No** `sky.open` on orbittyctl / MCP.

Inside `SkyPage.svelte`:

- Read `load_state` once. Tokenize the bag (lowercase, split on non-letters, drop stopwords + tokens length &lt; 3).
- Each token: `{ word, count, lastAt }`. `lastAt` from `completedAt` / turn time / `lastTouchedAt`.
- Layout: word cloud / packed labels. Size ∝ count (cap). Opacity: full if seen in last ~14 local days, else dim. Never-lit tokens simply absent.
- Click a word: highlight **co-occurring** tokens (appeared in the same done or same user turn). That’s geometry, not a rec sentence.
- Right-click / small × : mute. Persist `state.sky.muted: string[]`.
- Empty: one quiet line, “Nothing in the sky yet — done titles and what you say to Spark land here.” No homework CTA.

Stopword list: tiny English set in-file (the, and, for, with, this, that, you, …). Good enough. Do not ship an NLP crate.

## Persist

Additive on `state.json`:

```json
"sky": { "muted": [] }
```

Missing = `{ muted: [] }`. Do **not** persist computed tokens. Recompute on open.

## Files (expected)

| File | Change |
|------|--------|
| `src-tauri/tauri.conf.json` | `sky` window |
| `src-tauri/capabilities/default.json` | `"sky"` |
| `src-tauri/src/lib.rs` | `open_sky_window` → `show_labeled`; glass/pin/hide loops include `sky` |
| `src-tauri/src/state.rs` | `SkyState { muted }` default empty; on `PersistedState` |
| `src/lib/SkyPage.svelte` | NEW |
| `src/routes/+page.svelte` | mount when `label===sky`; rail or Settings opener |
| `src/lib/SettingsPage.svelte` | optional Open sky if no rail slot |

Do not touch `StarsPage.svelte` except to avoid noun collision. Do not touch Talk.

## Forbidden

- Recs / “considering one of these” / opposite glow
- 1–7, streaks, points growth, minutes-as-size
- Auto cluster names / identity
- Companion bot text in the bag
- Event SQLite / LifeRPG XP
- MCP/ctl `sky.open`
- Gravity wells, marble-drift
- Calling this STARS
- `tauri dev` / `target/` dogfood

## Verify (blessed app)

1. Sky closed by default. Open → cloud of words from **your** dones + user chat. Bot lines absent.
2. Mute a word; relaunch; still gone.
3. Mark a Kanban card done with a novel word; reopen Sky; it appears.
4. Empty vault → quiet empty line, not a lecture.
5. STARS window unchanged. `orbittyctl status` unchanged. No new ctl verbs.

Acceptance: `docs/ACCEPTANCE-ORBITTY-WORD-SKY.md`

When done: `./scripts/install-stable.sh`. Summarize files + dogfood. Do **not** write STAMP_*. AX re-grant expected.
