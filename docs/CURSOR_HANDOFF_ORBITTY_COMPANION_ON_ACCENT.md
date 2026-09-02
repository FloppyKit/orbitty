# CURSOR HANDOFF — GO-ORBITTY-COMPANION-ON-ACCENT

Model: kimi-k3-max. One writer.

Files: `src/themes/host.ts`, `src/themes/apply.ts`, `src/lib/CompanionPage.svelte`.

1. HostTokens gains `onAccent`. systemDark + systemLight = `"#ffffff"`.
2. `applyHostTokens` publishes `--on-accent` for system ids; removes it for palette ids.
3. Companion you-bubble, you-CC, `.send` → `color: var(--on-accent, #ffffff)`. No `--bg` fallback.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: tauri dev, them-bubbles, punch SVG redo, FOURCAST, caption, second writer.
