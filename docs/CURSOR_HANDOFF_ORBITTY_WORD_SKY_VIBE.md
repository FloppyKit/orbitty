# Cursor Ultra — GO-ORBITTY-WORD-SKY-VIBE

**Model:** `kimi-k3-max` · **one writer**
**Gate:** re-issued after Pixel filled REAL. Phrase `GO-ORBITTY-WORD-SKY-VIBE`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_WORD_SKY_VIBE_LOCK_v0.md` + this handoff

Stamped Sky (`STAMP_ORBITTY_WORD_SKY.md`) stays. This GO kills the **wrap-list grid** and puts words in a **spiral cloud on a dark space field**.

## Law

1. **Do not touch the bag.** Tokenize, docs, mute, co-occur, `state.sky.muted`, empty copy, cap 80, dim ~14d — leave them.
2. **Layout:** greedy Archimedean spiral from field center, biggest first, AABB + 6px pad. Uniform scale-to-fit. No `flex-wrap`. No cloud scroll.
3. **Bounce:** CSS-only 1–3px idle drift (hash delay). Hover scale ~1.06. Not habitat. Not marbles. Not `anim_tick`.
4. **Reduce Motion stills** drift and hover scale (`prefers-reduced-motion` and existing app pref if already readable).
5. **Space field:** dark `#0b1020` (or midnight mix). CSS radial dust ochre `#e59e3d` / teal `#3d8f8a` / violet `#c793e6`. No puff PNG. Word color = hash → those three. Picked/near still `--accent`.
6. Banner chrome stays. Noun Sky. Not STARS.
7. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit. AX re-grant.

## File

Almost certainly **only** `src/lib/SkyPage.svelte` (markup of `.cloud` + script pack + CSS). Do not add rust/`state.json`/windows.

Recompute pack on `visible` change and field resize (`ResizeObserver` on the field).

## Forbidden

`tauri dev` · `target/` · STAMP_* · StarsPage · I-DOT · rail marks · clay · MCP/ctl · Talk · in-app model · TCP · pack-freeze · chroma · second writer · wrapping flex as the layout
