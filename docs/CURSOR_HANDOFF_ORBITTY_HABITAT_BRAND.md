# CURSOR HANDOFF — GO-ORBITTY-HABITAT-BRAND

Model: kimi-k3-max. One writer.

**Do not edit `src/lib/SkyPage.svelte`.** Cloud is the SoT and must stay byte-identical.

1. `src/themes/host.ts` — `HostTokens` gains `bloom` + `field`. Both system ids: `focus: "#6e4db0"` (deeper, less pink than `#c793e6`), `bloom: "#c793e6"`, `field: "#0b1020"`. Keep accent `#3d8f8a`, mark `#e59e3d`, onAccent `#ffffff`. Leave bg/panel/ink/dim/edge Tahoe.

2. `src/themes/apply.ts` — `HOST_VARS` includes `--bloom` `--field`. Set from `t` for system ids; Custom still `removeProperty`.

3. `src/lib/CompanionPage.svelte` — Prompts `.on` ink `--mark`, **border `--focus`**. Head/man `.on` ink `--ink`, **border `--focus`**. Speak `.on` + hover/focus `--focus`. You/Send unchanged.

4. Dark pane bodies only (not Light, not Settings, not Sky): rail `+page.svelte` main habitat surface, `StarsPage.svelte` body, `KanbanPage.svelte` body, Companion transcript/body — Cloud dust stack on `#0b1020` (copy the three radials from Sky `.field`, do not import Sky). Selected/hover outlines `--focus`. Tags/secondary chips may use `--bloom`.

5. Kanban difficulty pips stay Apple green/orange/red. STARS radar stays teal fill / ochre stroke.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: SkyPage.svelte, tauri dev, I-DOT, RPG/Winamp, caption, MH-2, drawers, second writer, navy field on Light or Settings.
