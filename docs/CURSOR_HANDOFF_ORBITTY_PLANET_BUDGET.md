# CURSOR HANDOFF — GO-ORBITTY-PLANET-BUDGET

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_PLANET_BUDGET_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-PLANET-BUDGET.md`.

## Do (`habitat_mac.rs` almost only)

1. **Still-planet.** In `anim_tick`, expanded planets: `setNeedsDisplay` only if roam stepped, bob changed, locate stroke, or boombox caption changed this tick. Marbles already `continue`. Sleepers must not keep the view dirty.

2. **Lazy packs.** Do not decode a sprite pack until an **expanded** planet wears it. Marble never loads a pack. Collapsed layout already empty — keep that. Drop the bitmap when the last expanded wearer collapses (if cheap). Generated 12×12 beings unchanged.

3. **Cap 3 expanded.** `habitat:expand` / click-to-expand always succeeds. If 3 already expanded, collapse the least-recently focused (else stable worlds order) to marble, persist existing `collapsed` flag. On sync, if worlds have >3 expanded, collapse extras.

4. **Cap 12 marbles.** At most 12 collapsed NSPanels. 13th+ projects stay on the rail list with **no panel**. Slot frees when one expands or is unclaimed. On load, create ≤12 marble panels.

5. `./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Keep
DisplayLink, 16ms, marble drift, hitch 1-2-3 breathing, overlay, one panel per planet under the cap, sleepers on expanded planets, PANES keep-warm, companion own window.

## Forbidden
Skip hop if Δ < 1px. One marble CALayer surface. Caps in Settings. Destroy-on-hide. Spirit clock. `tauri dev`. Second writer. Stamp.

Stop when install-stable exits 0. Log what changed. Do not stamp.
