# CURSOR HANDOFF — GO-ORBITTY-COMPANION-CAPTION-GLYPH

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_COMPANION_CAPTION_GLYPH_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-COMPANION-CAPTION-GLYPH.md`.

## Do (`CompanionPage.svelte` almost only; `mdLite.ts` only if a tiny split helper)

1. Replace Full-stage block packing. Split the **raw** agent reply into pages of **~280 glyphs**. Prefer `\n\n`, then `.!?`, then word. Never blank.
2. Delete the `chunks.length > 1` gate in `arriveReply`. A single paragraph **must** split.
3. mdLite each page. Non-final still ends **…** (existing `.pgmore`).
4. While `pagedTurnIdx >= 0`, transcript / paged bubble **overflow hidden** — no scrollbar on the caption. Last-page release joins scrollback whole (keep `releasePager`).
5. Keep Full-only, Head/collapse/history/user never page, Enter/Space/click-anywhere, composing wins, sound-on auto, Reduce Motion no auto, no Next button, no new npm.
6. `./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden
Fit-the-stage packing. Head restyle. Soul. DQ8. `tauri dev`. Stamp. BOARD-STARS. WORDMARK. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
