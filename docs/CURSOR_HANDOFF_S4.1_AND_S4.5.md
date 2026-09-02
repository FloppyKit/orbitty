# Cursor Ultra — S4.1 rail honesty + S4.5 host themes

**ONE writer** · Tree: `/Users/dude/goose artifacts/livesprite-tauri`

Matt dogfood: claim works, sprites come/go, **cannot find Done**, **no Kanban**, Stow/Restore too big. Also **GO-LS-TAURI-S4.5**.

READ:
- `~/.buzz/PLANS/LIVESPRITE_S4_LOCKS_v0.md`
- `~/.buzz/RESEARCH/LIVESPRITE_HOST_THEME_PACKS_TAHOE_LINUX_v0.md`
- skill condensed: host-theme-packs (Tahoe hex + Adwaita/Yaru/Breeze)
- `src/routes/+page.svelte`

---

## S4.1 — must be obvious

### Done
- Home-claimed rows: **Done** is a real control (min ~22px hit, 0.72rem+, not 0.58rem pill).
- Row layout: title can ellipsis; **actions stay visible** (`flex-shrink: 0`, wrap row if needed). Never clip Done off the right.
- Confirm still “Mark done?”
- Unmapped: no Done.

### Kanban (always visible)
- Persistent **Board** in chrome (toggle next to + Project) or always-on section **above** the window list:
  - Columns: **Doing** (home-claimed live) · **Done** (kanbanCards status=done)
  - Empty Done: “Mark **Done** on a claimed row — cards land here.”
  - Show points chip + daily `N/12` even at 0.
- Not a tiny strip that only appears after first complete.

### Stow / Restore
- Replace wordy **Stow** / **Restore** with compact **icon** (~22px):
  - Stow = diagonal down-right arrow (minimize)
  - Restore = diagonal up-left (or up-right) arrow
- `aria-label` + title keep “Stow / Restore” text.
- Same for Done if space is tight: keep the word **Done** (honesty door). Stow is the one that becomes an icon.

Keep S3 planets, claim, tags, window drag.

---

## S4.5 — host Light / Dark / Clear

**Law:** do not invent primary palettes.

### macOS (this machine — Tahoe)
| id | tokens |
|----|--------|
| systemLight | bg `#ECECEC` · panel `#FFFFFF` · ink `rgba(0,0,0,0.85)` · dim `rgba(0,0,0,0.50)` · edge `rgba(0,0,0,0.10)` · accent `#007AFF` |
| systemDark | bg `#1E1E1E` · panel `#2C2C2E` · ink 85% white · dim `rgba(235,235,245,0.60)` · edge `rgba(84,84,88,0.65)` · accent `#0A84FF` |
| clear | keep NSVisualEffect vibrancy; labels as dark-vibrant (readable). Reduce Transparency → solid frost `#1E1E1E`, not gray sludge / white-out |

Default still follows OS. Palette skins stay secondary.

### Linux (code stubs only — no window module)
If `cfg linux` or detect `XDG_CURRENT_DESKTOP` later: bake comment + optional hex map:
- GNOME/unknown → Adwaita `#FAFAFB` / `#222226` accent `#3584E4`
- ubuntu → Yaru accent `#E95420`
- KDE → Breeze `#EFF0F1` / `#202326` accent `#3DAEE9`

macOS build can ship a `themes/host.ts` table used now for Mac + reserved Linux keys. No KWin.

Forbidden: WhiteSur, McMojave, Liquid Glass Tinted as a new theme name.

---

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Summarize. **No nest STAMP.**
