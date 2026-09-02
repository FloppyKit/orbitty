# Cursor Ultra — GO-LS-TAURI-AVATAR-LARGE · S2.1 dogfood

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Do not** edit Swift `~/goose artifacts/livesprite`.
**Do not** redraw `ghost.png`.

Matt dogfood 2026-08-23 on the 21:20 app:
- Window opens. Can resize + close. **Cannot drag by clicking the ghost.**
- **Kill Use my image…** — companion is not a user-upload surface.
- Chrome: **ghost only**. × in the corner. **No title. No footer.**
- Launch: **show with the app**; remember last hide/show (`companion.visible`).
- Whole ghost is the drag handle. Editor later = a text button (not this pack). Rub/react = later, not this pack.

## Fixes

### Drag
Overlay titlebar has no native drag strip. `data-tauri-drag-region="deep"` on `.stage` is **eaten by the `<img>`**.

On the spirit `<img>` and empty stage:
- `getCurrentWindow().startDragging()` on `mousedown` (primary button only)
- `draggable="false"`, `user-select: none`
- × `stopPropagation` so it still hides
- Boolean `data-tauri-drag-region` on leftover chrome if any

Verify by clicking the **ghost body** and moving the window.

### Chrome
- Remove footer, **Use my image…**, Clear, “my image” chip, the **Companion** `<h1>`.
- One ×, top-leading, padded for overlay traffic-light space (~76px) or sit in the trailing corner so it does not collide with lights.
- Spirit fills the window (`object-fit: contain`, nearest, pixelated).

### Import
Delete the companion import path from this window. Do **not** remove `packs.rs` import (planet Being editor still has Use my image). Ignore `companion.imagePackId` for drawing — always the blessed spirit. Field may remain in state so old JSON does not explode.

### Launch
Default `companion.visible = true` for first run (no prior key). After that, persist hide/show. App launch calls `show_companion` when `visible` is true **or** missing.

## Forbidden
S4 editor · talk · STARS · rub/react · new art · cargo-only install · Swift tree

## Verify

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```

Summarize files. Do not write nest STAMP_*.
