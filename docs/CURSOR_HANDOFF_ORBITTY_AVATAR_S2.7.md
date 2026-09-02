# Cursor Ultra — AVATAR-LARGE S2.7 · dots wait + speak replies

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift. No new sprite art.

Matt: S2.6 great except the cycling Sending/Thinking/… words feel fake. Want:
1. **Modern animated dots** for wait (iMessage typing). Both collapsed pending and expanded pending.
2. A **toggle** that reads agent replies aloud via macOS **system voice** (`say` — System Settings → Accessibility → Spoken Content / Voice). Off by default.
3. Always-listen / Hey-Siri is **not this pack** (Pixel will say so). Do not ship a dead mic.

## Wait
Remove `WAIT_WORDS` / the four-word loop. Replace with 3 pulsing dots (CSS, staggered opacity). Accessible: `aria-busy` / visually hidden “waiting”.

## Speak replies
Obvious text control: **Speak** (on/off), not a lone glyph. Persist `companion.speakReplies` (default **false**).

When a **new bot line lands** and the toggle is on:
```
say -v "?"   # do not list at users
# speak with the DEFAULT system voice:
say "$reply"
```
- Kill any in-flight `say` (`pkill -x say` or keep a child handle and kill it) before starting the next, and when toggle goes **off**.
- Do not read the user’s line. Do not read errors. Do not invent a second TTS stack (no ElevenLabs in Orbitty).
- `say` is macOS only; if it fails, one honest line — do not crash the chat.

## Forbidden
Always-on mic · wake word · fake “Hey Orbitty” · opening Hermes `/voice` as a substitute · cargo-only install

## Verify
`./scripts/install-stable.sh`
No STAMP_*.
