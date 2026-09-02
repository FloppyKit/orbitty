# Cursor Ultra — AVATAR-LARGE S2.8 · ochre-square wait

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift. No Imagine. CSS/SVG only.

Matt: pulsing dots are better than the word-loop but still generic. Wait should feel like **this** companion — the **ochre/orange square** from the spirit (Orbitty mark). Loop + spin. DNA-helix energy is OK if it stays ochre + teal, not a science stock gif.

## Wait mark (collapsed pending + expanded pending)
Replace the 3 CSS dots with a **16–20px** loader:

- Center: **rounded ochre square** (`#C8A45C` / `#E59E3D` — same family as the brick in the ghost). Slow rotate (one turn ~1.8s, linear).
- Orbit: a **teal** (`#2A8F8A`–`#3DB8B0`) 4px dot circles the square (opposite period ~1.1s) — planet around the brick.
- Optional second cream speck, very dim — do not clutter.
- `prefers-reduced-motion: reduce` → static square, no spin.
- `role="status"` + visually hidden “waiting”.
- No “Sending/Thinking” copy. No lone `…`.

Keep Speak toggle and everything else.

## Not this pack
Always-listen / PTT / wake word. Pixel is locking those as **Off | PTT | Always** (Off default). Do not ship a dead mic.

## Verify
`./scripts/install-stable.sh`
No STAMP_*.
