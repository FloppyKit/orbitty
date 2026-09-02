# Cursor Ultra — GO-ORBITTY-COMPANION-RENDER

**Model:** `kimi-k3-max` · **one writer**
**Gate:** phrase `GO-ORBITTY-COMPANION-RENDER` after Pixel filled REAL.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_COMPANION_RENDER_LOCK_v0.md` + this handoff

## Law

1. Both views render the subset. Collapsed is **not** plaintext. Raw `**` / `|---` / `---` rulers must not appear as typed.
2. New `src/lib/mdLite.ts`: escape HTML first, then **bold** / *italic* / `` `code` `` / lists / simple GFM tables / line breaks. Emit only those tags. `{@html}` only from this helper.
3. CompanionPage: expanded `.bubble` and collapsed `.cc` both consume the helper. `title=` may stay raw.
4. Collapsed compact CSS: smaller type, tight lists, real `<table>` not pipes, CC bubble `max-height: 40%` of pane, overflow auto if tall. Ghost stage still visible.
5. No new npm. Wait-in-slot unchanged. HEADROOM / spark.md / Cloud / I-DOT / four-cast untouched.

## Files

- `src/lib/mdLite.ts` — new
- `src/lib/CompanionPage.svelte` — wire helper + compact CC CSS

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

## Forbidden

`tauri dev` · `target/` · STAMP_* · marked / DOMPurify / any new dependency · slice B corner · slice C ⋯ · second writer
