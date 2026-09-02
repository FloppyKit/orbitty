# Cursor Ultra — S4.6 rail: no chrome checklist, circle-complete, todos in project, modern type

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

## Matt
1. **Delete the rail chrome checklist** (the Doing strip under the banner). Redundant with Lists. Do **not** leave a collapsed version.
2. **Complete = open circle at the far left** of each row (windows + todos). Click → fills (check) → row **disappears** (complete). Remove the extra ✓ / Done chip by the title.
3. **Todos live in the same project folder** as that project’s windows (rail sections).
4. **One button** to open the Kanban/Lists window (label **Board** or **Kanban**). Remove extra Lists… / Kanban… / “+N more” links. Keep **List | Board** toggle **inside** that window. Detail drawer stays.
5. **Type:** stop Wunderlist/mono. Use modern system UI: `"SF Pro Text", "SF Pro Display", system-ui, -apple-system, sans-serif` on rail **and** Lists window. Copy: “Board”, “Todo”, “Doing”, “Done” — not cute 2015 Wunderlist voice.

Keep: claim/drag, stow arrows, opacity, planets, points rules, item detail.

## Rail row (home-claimed window)
`[ ○ ] [icon] title … [stow ↘] [folder +]`

Unmapped: no complete circle (can’t complete-for-points). Just icon + title + attach.

## Todo rows in project section
Same leading ○. Title from card. Click title optional → open Board focused on that card if easy; else just complete on circle. Add todo: small “+ To-do” under the project section (or one field) → `kind: todo` on that project.

Circle complete = existing `completeWindow` / complete-todo path (confirm can be instant fill+gone; skip huge Mark done? if the fill animation is the confirm).

## Lists window
Keep List | Board + detail. Restyle font to SF Pro / system-ui.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# codesign Local Codesign + cp ~/Applications/LiveSprite.app
```
Summarize. No nest STAMP.
