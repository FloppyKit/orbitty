# Cursor Ultra — S4.4 glass + Hive board + Habitica-rich items

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

Matt: Tauri is catching Swift. Opacity still **washes white/grey** (not desktop). Lists is a thin checklist. Need **Kanban (Hive-like)** + **checklist**, richer items like Habitica add-daily. Grok banked sprite **directories** (not zips).

## 1) Opacity — actually see the desktop
White/grey = alpha over an **opaque WKWebView / NSWindow**.

Must:
- `NSWindow.isOpaque = false`, `backgroundColor = clear` on **main + kanban**
- WKWebView `setValue(false, forKey: "drawsBackground")` (or Tauri webview background color 0,0,0,0 **and** disable default white)
- `html, body, .rail, .k-shell` `background: transparent`
- **Only** `--bg-fill` / `--panel-fill` have alpha
- Text, icons, SVG, checkboxes stay `opacity: 1`
- At 40% slider, **wallpaper/apps behind** must be visible. If still grey, dump leftover opaque CSS (section, list-head, kanban columns) onto `--bg-fill`

## 2) Lists window = two views (hero)
Toggle **List | Board** (top of Lists).

### List (Wunderlist-ish)
Keep sidebar lists. Rows show **more than title**: checkbox, title, project, difficulty pip, due if set, sub-check count `2/5`, window icon if linked.

### Board (Hive-like)
Three columns **Todo · Doing · Done**. Cards = same fields (title + meta chips). **Drag card between columns** updates `status`. Window-claimed doing cards stay honesty-linked.

Empty column: one-line hint.

## 3) Item detail (Habitica daily-edit, thin)
Click row/card (not the checkbox) → **detail panel** (right drawer or modal):

| Field | |
|-------|--|
| Title | required |
| Notes | textarea |
| Difficulty | trivial / easy / medium / hard ( Habitica ) — **metadata only**; do **not** change point formula this slice |
| Sub-checklist | add/toggle/delete lines |
| Due date | optional date |
| Tags | existing tag ids |
| Linked window | read-only if `kind=window` |

Persist additive on `kanbanCards`: `notes`, `difficulty`, `subtasks[{id,title,done}]`, `dueAt?`.

**Not this slice:** Habitica HP/streaks/AI/calendar sync/habits column. Chaos: complete still only points door (1 / 2 clinical|master / cap 12).

## 4) Sprite directories (Grok) — hook, don’t scrape
SoT (browse, not bundled):
- `~/.buzz/PLANS/LIVESPRITE_SPRITE_SOURCES_v0.md`
- `~/.buzz/PLANS/LIVESPRITE_ITCH_CC0_CHARACTER_SHORTLIST_v0.md`
- Law: `LIVESPRITE_SPRITE_PACKS_LOCK_v0.md`

Create `~/Library/Application Support/app.livesprite.mac/packs/README.txt` explaining: drop local CC0/user sheets here later (`GO-LS-TAURI-USER-PACKS`). Optional: if a `packs/*/manifest.json` exists, habitat may pick extra templates — **do not download itch**. In-tree stays drawn blobs.

## Forbidden
S5, STARS, EHR, Swift edits, itch scrape, Habitica social/AI.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign macos .app Local Codesign → ~/Applications/LiveSprite.app
```
Summarize. No nest STAMP.
