# CURSOR HANDOFF — GO-ORBITTY-OVERLAY-HEAD

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_OVERLAY_HEAD_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — only.

## Do

1. Change `headShowing` so hosted no longer forces Head while expanded:

   `(viewPref === "head" || (hosted !== "off" && !expanded)) && bustOk && !missing`

   Collapsed hosted well stays Head. Overlay / idle Expand follow `viewPref` (`viewExpanded` when expanded).

2. Inside `.wellhead`, only render `spiritStage` when `!expanded`. Keep the `.wellhead` column. Keep `.hostwell` / KanbanPage / StarsPage mounted.

3. Do not rewrite `toggleView` / persist. The Full/Head button already writes `viewExpanded` when expanded.

4. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Rust. Unmount the well. Persist `hosted`. Change dest toggles / tray raise / `enterHost`. Fold action-edit / CLEAR.
