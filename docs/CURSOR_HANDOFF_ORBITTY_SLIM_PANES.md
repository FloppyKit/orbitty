# Cursor Ultra — GO-ORBITTY-SLIM-PANES

**Model:** `kimi-k3-max` · **one writer**
**Gate:** phrase `GO-ORBITTY-SLIM-PANES` after Pixel filled REAL **and** no other cursor-agent on this tree.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SLIM_PANES_LOCK_v0.md` + this handoff

## Law

1. `tauri.conf.json` windows: **rail only**. Remove `kanban` / `companion` / `stars` / `settings` / `sky` entries.
2. `show_labeled` + `show_companion`: if the window is missing, create it (Tauri 2 `WebviewWindowBuilder`, same SPA as rail), then the existing show/focus/bounds logic. Sizes and flags match today’s conf (transparent, Overlay, hiddenTitle, no drag-drop, companion min 280×360).
3. Wire CloseRequested hide (not destroy) and companion bounds on create, same as today’s loop. Pin stay-visible on all except companion. `ensure_glass` skips missing labels.
4. Launch: if `companion.visible` persist is true, create+show companion (first-show trailing-edge if no bounds). Do not create Lists/STARS/Settings/Cloud until opened.
5. Hitch 1-2-3 KEEP. No habitat tick / DisplayLink / spirit clock edits.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

## Forbidden

`tauri dev` · `target/` leftover as blessed · destroy-on-hide · drop rail · fold drawers/spawn/RENDER/I-DOT · second writer · STAMP_*
