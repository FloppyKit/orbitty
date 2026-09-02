# CURSOR HANDOFF — GO-ORBITTY-HABITAT-CLEAR

Model: kimi-k3-max. One writer.

**Do not edit `src/lib/SkyPage.svelte`.**

Strip every copied Cloud-dust rule. Do not replace with a new color.

1. `src/lib/StarsPage.svelte` — drop `habitat` from `.s-body`. Delete the `:root[data-theme="systemDark"] .habitat` navy+radial block. Body uses the pre-HABITAT fill (`--bg-fill` / existing `.s-body` background).

2. `src/lib/CompanionPage.svelte` — delete the dark `.composer` radial-gradient block. Composer is padding + flex only.

3. `src/lib/KanbanPage.svelte` — delete dark `.k-side` / `.b-col` radial rules. Those surfaces stay `var(--panel-fill, var(--panel))` only.

Leave `+page.svelte` (already clear). Leave `host.ts` / `apply.ts`. Leave `--focus` `#6e4db0`.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: SkyPage.svelte, tauri dev, re-applying dust, I-DOT, RPG, caption, MH-2, drawers, second writer.
