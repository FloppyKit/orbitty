# CURSOR HANDOFF — GO-ORBITTY-HABITAT-MIST

Model: kimi-k3-max. One writer.

**Do not edit `src/lib/SkyPage.svelte`.**

Solid `#0b1020` is **only** legal on STARS `.s-body`. Everywhere else, radials sit on `--bg-fill` / `--panel-fill` — never a navy slab.

1. `src/lib/CompanionPage.svelte`
   - `<main class="companion">` — drop `habitat`.
   - Delete `:root[data-theme="systemDark"] .habitat { … #0b1020 }`.
   - Dark `.composer` only:
     ```
     background:
       radial-gradient(620px 440px at 85% 100%, rgb(199 147 230 / 0.18), transparent 70%),
       radial-gradient(400px 280px at 20% 120%, rgb(61 143 138 / 0.08), transparent 70%),
       radial-gradient(360px 220px at 70% 0%, rgb(229 158 61 / 0.06), transparent 70%);
     ```
     No `#0b1020`. Light unchanged.

2. `src/lib/KanbanPage.svelte`
   - `.k-body` — drop `habitat`. Delete the body navy rule.
   - Dark `.k-side` and `.b-col`: Cloud’s three radials **then** `var(--panel-fill, var(--panel))`. No `#0b1020`.

3. `src/routes/+page.svelte`
   - `.scroll` — drop `habitat`. Delete the rail navy rule. Agents / Actions / Projects list stay `--bg-fill`.

4. `src/lib/StarsPage.svelte` — **keep** `.s-body.habitat` and the full stack including `#0b1020`.

Do not touch `host.ts` / `apply.ts`. When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: SkyPage.svelte, tauri dev, navy on companion/board/rail, mist on Agents/Actions, I-DOT, RPG, caption, MH-2, drawers, second writer.
