Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_GLASS_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-GLASS.md`.

`GO-ORBITTY-PANE-MATCH` CODED 23:24 leftover this. Do not re-issue MATCH / SPA / PAINT / HOLD / SHOW / RAISE / SHEET-* / CLAMP. Do not stamp.

Matt: black native banner gone. Transparency still not like the rail. Settings 0.75 looks solid on Companion/Board.

Cause: PAINT leftover `html[data-drawer], html[data-drawer] body { background: var(--bg-fill, var(--bg, #1E1E1E)); }` stacks two extra fills on top of `<main>`. Three 0.75 layers ≈ 0.98. Rail html/body are transparent — one fill.

Do (`+page.svelte` only):

A. Delete the `html[data-drawer]` / `html[data-drawer] body` background rule. Keep `html, body { background: transparent }`. Page fills stay on `<main>` via `--bg-fill`. One layer.

B. Keep MATCH Overlay+transparent, SPA never-hide, HOLD never-hide, companion pin-while-shown, COVER, STARS-shares-kanban, `?mode=` ban.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
