# CURSOR HANDOFF — GO-ORBITTY-COMPANION-CAPTION-WORDS

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_COMPANION_CAPTION_WORDS_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-COMPANION-CAPTION-WORDS.md`.

## Do (`CompanionPage.svelte` + `mdLite.ts`)

1. **Invert the pager gate.** Today: `if (expanded && viewPref === "full")`. Wanted: `if (!expanded)`. Head/Full (`viewPref`) must not appear in the pager path. Expanding while a page is held → `releasePager()` so the whole reply is the transcript bubble. History never re-pages on collapse.

2. **~60 word pages.** Rename `glyphPages` in spirit (`wordPages` ok). Count whitespace-split tokens, not characters. Break order stays paragraph (`\n\n`) then sentence (`.!?`) then word. Never blank. A ~100-word two-paragraph reply **must** yield ≥2 pages.

3. Keep: `.pgmore` `…` on non-final; overflow hidden while paging; Enter / Space / click-anywhere; composing wins; sound-on auto; Reduce Motion no auto; no Next; last page `releasePager`; you-bubbles never page.

4. Bind the paged bubble to **turn index** (`i === pagedTurnIdx`), not `t.text === pagedText`. `state:changed` rewriting the string must not dump the pager.

5. Comments: collapsed vs expanded. Not Full vs Head.

6. `./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Forbidden

Fit-the-stage packing. Head restyle / bustfloat. Soul. `tauri dev`. Stamp. WORDMARK. IDLE-STILL. Walker bob. Second writer.

Stop when install-stable exits 0. Log what changed. Do not stamp.
