# CURSOR HANDOFF — GO-ORBITTY-COMPANION-BRAND

Model: kimi-k3-max. One writer.

Brand triad (Sky): teal `#3d8f8a` · ochre `#e59e3d` · violet `#c793e6`.

## Files

1. `src/themes/host.ts` — `HostTokens` gains `mark` + `focus`. Both systemDark and systemLight: `accent="#3d8f8a"`, `onAccent="#ffffff"`, `mark="#e59e3d"`, `focus="#c793e6"`. Leave bg/panel/ink/dim/edge.
2. `src/themes/apply.ts` — `HOST_VARS` includes `--mark` and `--focus` (keep `--on-accent`). Set from `t` for system ids; `removeProperty` all HOST_VARS for Custom ids.
3. `src/lib/CompanionPage.svelte` — you/Send stay `var(--accent)` + `var(--on-accent, #ffffff)`. Prompts `.on` → `--mark`. Head `.on` → `--ink`. Speak `.on` + hover/focus → `--focus`. Full-body SVG = restroom-man (head + shoulders + two legs); delete the fat-i rounded rect.
4. `src/lib/SettingsPage.svelte` — block `"Custom"` not `"Palette"`. THEMES = Light, Dark, Terminal. Delete the other seven. Dead saved ids → OS default. Swatch `.c-ac` for system pair = `#3d8f8a`.
5. `src/routes/+page.svelte` — same THEMES / BLOCKS / migrateThemeId. Delete the seven `:root[data-theme]` CSS blocks. Keep Terminal.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: tauri dev, them-bubbles, I-DOT, RPG/Winamp, caption, FOURCAST, Linux DE packs, second writer.
