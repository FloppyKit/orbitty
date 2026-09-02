# Cursor Ultra — S4.2 rail + opacity (Matt dogfood)

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

## Matt
1. **Ditch Clear** as a named theme. Add an **opacity slider** for window **background** (Light/Dark/Palette still apply; slider fades `--bg` / rail fill).
2. **Kill the big Doing/Done board tile** in the rail. Rail may have a **short checklist** only. Click a checklist item → open a **separate Kanban** (own window/panel or gear dropdown). Full board does **not** live in the rail column.
3. **Done** = **checkmark icon** (not a big Done word button). aria-label “Mark done”. Confirm can stay tiny.
4. **Remove `min` pill** — minimized rows already gray.

Keep: claim, planets, stow arrows, focus, tags, points rules, window drag.

---

### 1 — Themes / opacity
- Remove **Clear** from System picker. Migrate saved `clear` / `liquidGlass` → OS Light/Dark + keep opacity.
- Persist `windowOpacity` 0.35–1.0 (default 1.0) in `state.json`.
- Slider in Theme panel: “Background opacity”.
- Implementation: paint rail/body bg as `color-mix` or rgba of theme `--bg` at slider alpha. Webview window **transparent** when opacity < 1; call `set_vibrancy(false)` always (no Clear material).
- At 1.0: fully opaque (current Light/Dark).
- Do **not** fade text/icons — only background fill.
- Desktop shows through at low opacity. Readable labels always.

### 2 — Checklist + separate Kanban
- Remove chrome **Board** two-column tile entirely.
- Optional compact **checklist** in chrome or under gear: ≤5 lines (Doing titles + check to complete). Empty: omit or one line “No claimed work”.
- Click checklist **title** (not the check) → **open Kanban** in a **separate** Tauri webview window (`kanban`, ~640×420) **or** a panel launched from gear “Kanban…”.
- Kanban window: Doing | Done columns, cards, `N/12`. Same `state.json`. Not stuffed into the rail.
- Prefer second window if easy; else gear dropdown that opens it.

### 3 — Checkmark Done
- Replace Done text button with ✓ icon (~22px hit). Title/aria “Mark done”.
- Confirm: small ✓? or keep “Mark done?” as pop, not a huge chip.

### 4 — No min pill
- Delete `{#if w.isMinimized}<span class="tag">min</span>{/if}`
- Keep gray `.min` row style.

Forbidden: S5, STARS, new sprites, EHR.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Summarize. No nest STAMP.
