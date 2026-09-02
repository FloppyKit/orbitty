# Cursor Ultra — S4.3 opacity + Wunderlist hero lists

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

Matt:
1. Opacity slider does **nothing visible**. Whole UI fill must go transparent; **icons + titles stay solid**.
2. Kanban card on **claim** (add window to project), not only on close/complete.
3. Checklist + Kanban are **heroes** — Wunderlist-like (lists + tasks, not a cramped rail board).
4. Each project can have **todos that are not windows**.

## Opacity (must demo)
- Window already `transparent: true`.
- **Every** painted chrome fill (`.rail`, `.chrome`, `.section`, `.row`, `.theme-panel`, `.claim-menu`, footer) uses theme color × `windowOpacity`.
- **Never** fade: app icons, title text, ink, buttons, SVGs (`opacity: 1` on those).
- If `color-mix` fails in WKWebView, convert `--bg` hex → `rgba(r,g,b,windowOpacity)` in JS (`applyWindowOpacity`) and set `--bg-fill`.
- Also `setBackgroundColor` on the Tauri window to transparent.
- Slider 0.2–1.0; at 0.3 desktop wallpaper must show **through the rail**, text still readable (maybe slight text-shadow).
- Live update while dragging.

## Cards on claim
- `claim(windowId, projectId)` creates/updates a Kanban card immediately: `status: doing`, title = `app — title`, `windowId`, `projectId`.
- Complete (✓) → `done` + points rules (unchanged).
- Unclaim: card stays as loose todo **or** archive — prefer keep as project todo without windowId.

## Wunderlist-shaped Kanban window (hero)
Look: 6Wunderkinder Wunderlist Mac —
- **Left sidebar:** Inbox + one **list per LiveSprite project** (folder/list). Smart-ish: Today / Done at top (simple filters, not a new product).
- **Right:** tasks in that list. Each row = checkbox + title + optional star later.
- **Add task:** bottom or top input “Add a to-do…” Enter → new card `kind: "todo"` (no window).
- Window-linked cards show tiny app icon; free todos don’t.
- Clicking list name switches list. Selected list highlighted.
- Completed: strike + check, group at bottom or “Show completed”.
- Soft, airy, large type, generous row height (~36–40px). Not our cramped rail.
- Same `state.json`. Extend card: `kind?: "window"|"todo"`, `starred?: bool`.

## Rail
- Short checklist of **current project’s open/doing** items (windows + todos) OK, or a “Lists” button that **always** opens the Wunderlist window — make that window first-class (tray or gear **Lists**).
- Do not stuff the full Wunderlist UI into the 360px rail.

## Forbidden
S5, STARS, EHR, Swift edits, planet rewrite.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
# dmg may fail — still sign+copy .app like:
npm run tauri build || true
# then codesign Local Codesign + cp to ~/Applications/LiveSprite.app
```
If `install-stable.sh` dies on dmg, sign the macos `.app` and copy anyway.
Summarize. No nest STAMP.
