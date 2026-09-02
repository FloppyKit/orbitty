# Cursor Ultra — GO-ORBITTY-RAIL-DRAWER

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_RAIL_DRAWER_LOCK_v0.md` + this handoff + `docs/ACCEPTANCE-ORBITTY-RAIL-DRAWER.md`

Chrome panes only. Invisible leash. Planets stay habitat.

## Law (post RAIL-REVIEW + BOARD-STARS)

1. **Three** drawer labels only: `companion` · `kanban` · `settings`.
2. Board and STARS are **one** 780×520 overlay (`kanban`). `open_stars` shows that same window. Do **not** `ensure_window("stars")`. If a leftover `stars` pane exists, hide it, never dock it.
3. **Not** `sky`. Cloud lives inside STARS Review. The `sky` webview stays hide-not-destroy, never a drawer.
4. **No child windows.** Follow-translate on rail Moved.
5. First show this session with no plucked bounds: slide ~240ms native `set_position` tween into the inboard slot. Reduce Motion = jump. Not a habitat clock. Not CSS.
6. Default stack inboard, top-aligned, 8pt gap: **Companion → Board → Settings**. Skip a slot that clips `visibleFrame`.
7. While docked, dragging the pane under ~48pt updates `(dx, dy)`. Crossing ~48pt **plucks**. Click is use, not pluck.
8. Settings: per-pane Re-tether + Re-tether all. User-owned. Not a proximity magnet.
9. Additive persist `drawers` on `state.json` for `kanban` / `companion` / `settings` only. Old `companion.bounds` → companion starts plucked. Other panes start docked. Missing `drawers` key = that law.
10. Hide / Quit / Pin / Show Desktop unchanged. Do not dock planets.
11. Do not restyle I-DOT / WORDMARK. Do not punch SPIRIT-FLOAT, companion-drag clamp, STARS 404, boombox.

## Files (likely)

- `src-tauri/src/state.rs` — additive `drawers`
- `src-tauri/src/lib.rs` — show path, Moved follow, drag-pluck; never spawn `stars` as a drawer
- `src/lib/SettingsPage.svelte` — Re-tether chrome

## Forbidden

`tauri dev` · `target/` · STAMP · child windows · drawn tether · habitat_mac planet ring · boombox · SPIRIT-FLOAT · companion drag clamp · second writer.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.
