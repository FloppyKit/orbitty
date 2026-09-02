# CURSOR HANDOFF — GO-ORBITTY-RAIL-COVER

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_RAIL_COVER_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-RAIL-COVER.md`.

Desk-blocking leftover of RAIL-DRAWER + PANE-404. Click one rail item → blank overlay sits on the rail and eats every click.

## Do (`src-tauri/src/lib.rs` + maybe `src/routes/+page.svelte`)

1. **Never show a drawer at the rail origin.** First-show slide start is the inboard slot, or fully off `visibleFrame` (one pane-width past the rail), **never** `geo.pos.x`. Prefer skip the on-rail tuck entirely: show already at the inboard slot (Reduce Motion already jumps).
2. **Do not `show()` the kanban overlay until `paneMode` is board|stars.** Native already persists + emits mode before show. Frontend: keep null = render nothing, but native must not show a hit-testing empty frame over the rail. Lean: seed `paneMode` synchronously from `load_state` / tiny invoke before first paint, default `"board"` — if null remains, `ignoreCursorEvents(true)` until board|stars.
3. **If nav-guard cancels or document is non-2xx: hide AND `ignoreCursorEvents(true)`.** Never leave a pinned shown blank overlay.
4. Keep `?mode=` banned. Keep one 780×520 kanban overlay. Do not spawn `stars`.

## Forbidden

Companion CLAMP. SPIRIT-FLOAT. Boombox. Caption. Drawers redesign. `tauri dev`. Stamp. Second writer.

`./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Log what changed. Do not stamp.
