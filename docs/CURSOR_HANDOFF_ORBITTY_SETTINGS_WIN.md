# Cursor Ultra — GO-ORBITTY-SETTINGS-WIN

**Gate:** Matt **GO-ORBITTY-SETTINGS-WIN** via Pixel Artist **2026-08-27** (applied)

**This GO pops Settings out of the rail.** Gear ⚙ opens its own window (same pattern as Board / Companion / STARS). Rail inline `theme-panel` is gone. Talk source lives here.

## Applied

- Window label `settings` (~420×560, hidden at launch, hide on ×)
- `src/lib/SettingsPage.svelte` — tabs Appearance · Talk · Packs
- `talkSource` on companion state: `system` (default) | `hotkey`
- `talk_start` only holds Handy when source is `hotkey`; system is honest until `GO-ORBITTY-PTT-DICTATION`
- No `settings.open` on orbittyctl / MCP v0
- Speak / Prompts stay on companion; AX grant card stays on rail

Acceptance: `docs/ACCEPTANCE-ORBITTY-SETTINGS-WIN.md`
