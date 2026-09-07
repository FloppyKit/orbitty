# Architecture

One Mac app. The habitat is the engine. Everything else inspects it or sits beside it.

| Piece | Where | Job |
|-------|--------|-----|
| **Habitat** | `src-tauri/src/habitat_mac/` | Project planets as peer `NSPanel`s. Claimed windows are walkers (or occupancy dots if parked). Click-through corners. Vsync tick. Done walks off. |
| **Rail** | `src/lib/RailPage.svelte` + `src-tauri/src/windows_mac.rs` | Inspector: AX window list, claim, tags, being editor. Not a second IDE. |
| **Drawers** | `src-tauri/src/drawers.rs`, `panes.rs` | Launchpad (kanban), STARS, Settings, Sky, companion chrome. Opaque overlays; the rail stays a column. |
| **Companion** | `src/lib/CompanionPage.svelte` | A seat. Spirit sheet by default. Optional 3D bodies in `Companion3D.svelte` (dynamic `three` import only then). |
| **Control plane** | `orbittyctl`, `orbitty-mcp` | Unix socket. Pure std + JSON. **Never** links Tauri, **never** opens a WebView. |

`mod.rs` / `tick.rs` / `draw.rs` / `input.rs` / `audio.rs` split the habitat. Do not put companion or kanban work in those files.

State is one `state.json` (`src-tauri/src/state.rs`, TS mirror `src/lib/persist.ts`). Every writer load → mutate → save the **whole** object. AX rows are live, not persisted.

The frontend is one SvelteKit SPA. Window label picks the page (`main` = rail, `kanban` = board/STARS, `companion`, `settings`, `sky`). Native code owns show/hide and `sharedPaneMode`.

Budget: ≤3 expanded planets, ≤12 marble panels. Sleepers stay on the disc until Done or Unclaim. Points only on explicit goal-aligned complete, hard cap.

Identifier `space.orbitty.mac`. `clinical_claim: false`.
