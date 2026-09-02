# Cursor Ultra — GO-ORBITTY-CHARS

**Model:** `kimi-k3-max` · one writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`  
**Do not** edit Swift `~/goose artifacts/livesprite`.

Read first:
- `~/.buzz/PLANS/ORBITTY_CHARS_DESIGN_v0.md`
- `docs/ACCEPTANCE-ORBITTY-CHARS.md`

## Build
1. **Procedural people** from current built-in pixel critter language. Layers: body, clothes, eyes, mouth, hat/hair, colors. Persist `claims[].charSeed` + layer picks (or equivalent). Empty pack id = generated being.
2. **Being editor** on habitat **single-click** walker (change `habitat_mac` click matrix: single = editor event to frontend or native popover; **double = restore** stays).
3. Editor fields: look, reroll, per-claim scale, local file import (reuse `packs.rs` file picker — **delete/hide itch URL download** from Settings), tags, home project / unclaim.
4. Habitat must draw generated frames (rasterize layers to idle/walk images, or draw layers in `drawRect`). Don’t require an itch zip.
5. install-stable → Orbitty.app.

## Forbidden
Habitica/Kenney/itch fetch · companion avatar · STARS · EHR · points rewrite.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
# prefer install-stable if present; else tauri build
```
Summarize files + residuals. Do not write nest STAMP_*.
