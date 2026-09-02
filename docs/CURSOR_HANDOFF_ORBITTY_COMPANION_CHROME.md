# CURSOR HANDOFF — GO-ORBITTY-COMPANION-CHROME

Model: kimi-k3-max. One writer. Read the lock first.

## Files

- `src/lib/CompanionPage.svelte` — agentrow, you-bubble color, composer mic
- `src/lib/SettingsPage.svelte` — Body select under Appearance
- `src/routes/+page.svelte` — `--on-accent` on the eight `:root[data-theme]` blocks

## Do

1. `--on-accent: #ffffff` on menuCharcoal / midnight / plumNight / terminal. Light themes: `--on-accent` = that theme’s `--bg` (no visual change). You-bubbles + you-CC: `color: var(--on-accent, var(--bg))`.
2. Agent row: Agent `<select>` (four stems). Prompts = compact toggle. Speak = speaker SVG, slashed when off. View = head/body silhouette. Expand/Collapse = ↗ / ↙. No Body on this row.
3. Body `<select>` moves to Settings → Appearance, label Companion body. Same persist `companion.pack`.
4. Talk: mic SVG inside the field, far right. While `talking`: check = `talk_stop` (text stays); up-arrow = `talk_stop` then `sendLine()`. Idle Send outside the field stays for typed send.
5. Inline SVG, 16–18px, `currentColor`. No new npm.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

## Do not

FOURCAST labels. Caption pager. DQ8. I-DOT. Drawers. mdLite. Always-listen. `tauri dev`. MH-2-APP. Change System/Handy source.
