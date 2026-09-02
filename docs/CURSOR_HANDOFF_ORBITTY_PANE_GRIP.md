# CURSOR HANDOFF — GO-ORBITTY-PANE-GRIP

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_PANE_GRIP_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-PANE-GRIP.md`.

STAY leftover: click right-of-center on a drawer title and the window runs away; left-of-center generally grabs. Cause: Overlay+hidden_title native ~28px drag strip PLUS `data-tauri-drag-region="deep"` on Kanban/Settings banners (Companion also JS `startDragging`). Two drag sources. List/Board tabs have no stopPropagation so Overlay eats the click.

Do (`KanbanPage.svelte` + `CompanionPage.svelte` + `SettingsPage.svelte` only):

A. Remove `data-tauri-drag-region` from drawer banners. No JS `startDragging` on those banners. Overlay native drag is the one source. Companion `startDragging` only on collapsed ghost / corner portrait below the native strip. Keep × stopPropagation. If List/Board tabs still don't click, pad the banner so tabs sit below ~28px.

B. Do not touch the rail banner.

Keep STAY setFrame / park / focus-only, WARM, MATCH Overlay+transparent, SPA, HOLD, GLASS, COVER, STARS-shares-kanban, `?mode=` ban, hide-not-destroy.

Forbidden: persist-law / PANE-SLOT, hide after show, Visible titlebar, `ensure_opaque`, screen-center, undo dock, `slide_into_slot`, stamp, `tauri dev`, `state.json`, kill Orbitty, same-breath SPLIT-PANES.

`./scripts/install-stable.sh` (NOT tauri dev). Stop when exit 0. Do not stamp.
