# CURSOR HANDOFF — GO-ORBITTY-SPLIT-RAIL

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_RAIL_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-RAIL.md`

Extract the rail from `src/routes/+page.svelte` into `src/lib/RailPage.svelte`. Behavior identical. Overlay windows load the same SPA — `:global` host rules stay in `+page.svelte`.

PANES already shipped.

## Do

1. New `src/lib/RailPage.svelte`:
   - rail brain (today's rail `onMount`, habitat listeners including **`habitat:walker-home`**, tick/claims/being/ctx/packs, `occupancyOf` / `setWalkerHome`)
   - the `{:else}` tree (wordmark Home with `rb` / ochre `i` / accent `ty`, stack Companion·Board·STARS, lanes, window list, being editor, ctx menu)
   - rail `<style>` (`.rail` `.chrome` `.banner` `.wordmark` `.o` `.idot` `.pop` `.row` `.ctx-menu` `.being-editor` …). Optional `src/lib/rail.css`.
2. `+page.svelte` leftover:
   - window-label dispatch → spa-slab / CompanionPage / Kanban+Stars / Settings / Sky / `<RailPage />`
   - `dataset.drawer` tag
   - `paneMode` seed `"board"` + kanban `$effect` `setIgnoreCursorEvents`
   - kanban `onMount` `orbitty:shared-pane-mode` + `load_state` reconcile
   - `:global(html)`, `:global(body)`, `:global(:root[data-theme="terminal"])`
   - SkyPage frozen — import only.
3. Host tokens stay `themes/`. List-row left-click stays locate-only. Rail Assign stays claim-move. Walker occupancy does not rewrite `projectId`.
4. Leave rail banner dual-drag (`data-tauri-drag-region` + JS) as-is.
5. `svelte-check` + `./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`.

## Must not

Restyle wordmark. Change stack order. Change wordmark Home = `bringAllFront`. Touch Rust / habitat / SkyPage. Grow Being editor. Strip Image chips.

## Forbidden

Same-breath habitat. `tauri dev`. Stamp. Second writer.

Stop when install-stable exits 0. Log what moved. Do not stamp. Do not kill Orbitty.
