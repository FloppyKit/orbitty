# Cursor Ultra — GO-LS-STARS-STATUS + companion PTT

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift tree. No LifeRPG. No always-listen. No companion art redraw.

**Lock:** `~/.buzz/PLANS/ORBITTY_STARS_LOCK_v0.md`  
**Acceptance:** `docs/ACCEPTANCE-ORBITTY-STARS.md`

## Rail — STARS pane
- Chrome text button **STARS** next to Companion.
- Panel: heading “STARS — Spiritual / Self is the fifth spoke” + five obvious 0–4 controls labeled Sleep, Terroir, Athleticism, Relationships, **Self**.
- SVG/canvas radar (chunky, teal fill / ochre stroke). Even = calm; one short spoke, no red alarm.
- **Save today** writes `state.stars.checkins[local YYYY-MM-DD]`.
- `stars.promptsOptIn` default **false**. Toggle **Prompts** in this pane (and mirrored on companion).

## Persist (`state.rs` additive)
```
stars: { promptsOptIn: bool, checkins: { day: { sleep, terroir, athleticism, relationships, self } } }
```
Old saves load. No PHI fields.

## Companion
- **Talk** hold next to Send (text, not a lone glyph). `mousedown`/`touchstart` start, `mouseup`/`mouseleave`/`touchend` stop.
  - `webkitSpeechRecognition` / `SpeechRecognition` if present. Interim optional. On end: put transcript in the composer field (do **not** auto-send).
  - Missing API: one honest line. Keep Send/type working.
- **Prompts: Off/On** next to Speak. When On **and** today’s check-in exists: one strip naming the **lowest** spoke + chips **Rest** · **Water** · **Smallest todo**.
  - Rest / Water: ack and hide strip until next check-in / next day.
  - Smallest todo: first `!done` subtask on a `doing` card, else that card’s title, else honest “nothing open on the Board”. May `open` Lists.
- Never draw text on the spirit. No timer nag. No second LLM.

## Forbidden
Wearables · dwell score · window-title scoring · always-on mic · wake word · cycle “back to work” loop · cargo-only install · STAMP_*

## Verify
`./scripts/install-stable.sh`
