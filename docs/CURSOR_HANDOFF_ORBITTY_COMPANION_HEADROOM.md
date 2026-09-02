# Cursor Ultra — GO-ORBITTY-COMPANION-HEADROOM

**Model:** `kimi-k3-max` · **one writer**
**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-COMPANION-HEADROOM`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_COMPANION_HEADROOM_LOCK_v0.md`

## Why

Expanded today clips `idle_N` to **top 128px** (`.head { height: 128px }`). Spirit → glow blob. Jacob → neck shear. That is not a floating bust.

Matt: default non-expanded = **full**; default expanded = **head**. Toggle so they can run a really big head on the stage. Bust camera. Float. When it reacts, a little rotate.

## Law

1. Same companion window, same packs, same 8-frame idle + think-v9. No new character.
2. Persist:
   - `companion.viewCollapsed`: `"full" | "head"` default `"full"`
   - `companion.viewExpanded`: `"full" | "head"` default `"head"`
3. Chrome label **Full / Head** (not Figure, not Max, not mannequin). One control; writes the **current** surface only.
4. **Head** = bust sheet (head + neck + collar hint), 256×384, hard alpha, nearest-neighbor. Not a CSS crop of the full frame.
5. Float: few px drift while Head is showing. Think/send: small yaw/roll then settle. `reduce_motion` → sit, no rotate.
6. Spirit remains default `imagePackId`. Do not auto-switch to Jacob.
7. No Talk/PTT/MCP/STARS/marbles/wells/recast/S4 mixer/live 3D/`tauri dev`/`STAMP_*`.
8. Dogfood only `~/Applications/Orbitty.app` after tray Quit.

## Ship

**Chrome (`CompanionPage.svelte` + persist in `lib.rs` / companion state):**

- Kill the 128px `.head` crop as the expanded object.
- If current surface view is `full`: existing `.stage` full frame.
- If `head`: draw the **bust** frames (same tick as idle/think). Hard alpha, float.
- Toggle Full/Head in the existing companion chrome (near Expand). Writes `viewCollapsed` or `viewExpanded` matching `expanded`.
- Load/save with `state.json` companion slice. Missing keys → defaults above.

**Bust frames:**

- Spirit: camera push-in / second render of the existing idle+think (same cream/teal/ochre wisp). Output `bust_0…7` (+ think bust if think-v9 has a matching camera). Fail soft: if bust files missing, do **not** fall back to the 128px crop — sit on full and keep Head disabled or show full with a quiet miss. Never shear.
- Jacob temp: second camera in `bake_jacob_idle.py` (or sibling) → `temp_bodies/companion-jacob/bust_*.png`. Proof only. Do not make Jacob default. Do not ship as CoS.
- Pack contract: a companion pack **may** include `bust_0…7` next to `idle_0…7`. `companion_frames` returns both. Head view requires bust; no crop fallback.

**Motion:** CSS/transform on the bust img is enough (translate + small rotate). No CADisplayLink. No n-body. Think clip already swaps the sheet — rotate is extra, not a new clip.

## Files

- `src/lib/CompanionPage.svelte` — views, toggle, float, kill 128px crop
- `src-tauri/src/lib.rs` (or companion persist) — `viewCollapsed` / `viewExpanded`
- `brand/companion/spirit/` — bust frames if you bake them this GO
- `brand/companion/jacob_idle_bake/` — bust camera proof only
- `habitat_stub` / marbles / Talk — do not touch

## Forbidden

128px crop as Head. One global view bool. Auto-replace spirit. Scanlines as v0. Live Three.js. Puppet MCP. Four-cast allowlist rewrite. Halo/marble. `STAMP_*`.

When issued and done: `./scripts/install-stable.sh`. Summarize. Do **not** write `STAMP_*`.
