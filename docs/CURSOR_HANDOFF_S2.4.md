# Cursor Ultra — S2.4 rail UX (claim chip, collapse, tags) + system tokens

**Model:** kimi-k3-max · ONE writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

## Matt dogfood
1. Big **Claim** button → compact **+** / attach icon (tooltip: “Attach to project folder”)
2. Once claimed, **do not** repeat project name on the row (folder header already shows it) — show only a small folder/attached glyph or nothing but menu
3. **Collapse** project folders (and Unmapped)
4. **Home project** (one) + **optional tags** (many) for sorting — tags ≠ home folder
5. Themes still not true macOS — tighten **systemLight / systemDark / clear** to published semantic tokens (see below). Palette stays secondary.

Keep: icons, drag-claim, sticky header, window move, focus/AX. No sprites (S3).

---

## Fix A — Claim control
- Unclaimed: small **+** button (or `↗` / folder-badge-plus), aria-label “Attach to project”
- Claimed: **no project name chip** on the row; optional tiny folder glyph that opens the same menu
- Menu still: pick home project, unclaim, new project; **also tag multi-select** (Fix C)

## Fix B — Collapsible sections
- Project sections + Unmapped: chevron toggle; collapsed hides rows, keeps header + count
- Persist `collapsedSectionIds: string[]` in state.json
- Drop-on-header still works when collapsed (claim into collapsed folder)

## Fix C — Home project + tags
Model (persist in state.json):

```ts
// projects stay {id,name} — these are folders / home targets
// tags: freeform labels {id, name, color?}
projects: Project[]
tags: Tag[]
// ONE home claim per window (folder placement) — same as today
claims: { windowId, projectId }[]   // at most one projectId per windowId
// MANY tags per window
windowTags: { windowId, tagIds: string[] }[]
```

UX:
- Row menu: **Home folder** (radio list of projects + Unclaim)
- Row menu: **Tags** (checklist multi-select + “New tag…”)
- Optional filter chips in list-head: All | tag filters (simple)
- Grouping in list remains **by home project only** (tags do not create sections in S2.4)
- Thin-slice: unmapped (no home) still zero sprites later; tags alone do not birth sprites

## Fix D — System theme tokens (macOS-faithful)

Use published-style semantic values (approx Apple / libadwaita), not invent:

### macOS Light (`systemLight`) — HIG-ish
| token | value |
|-------|--------|
| window bg | `#ececec` → prefer slightly lighter sidebar `#f5f5f7` or `#ffffff` for panel |
| panel / control | `#ffffff` |
| separator | `rgba(60,60,67,0.29)` light / `0.18` hairline |
| label | `rgba(0,0,0,0.85)` |
| secondaryLabel | `rgba(60,60,67,0.6)` |
| accent (systemBlue) | `#007AFF` |

### macOS Dark (`systemDark`)
| token | value |
|-------|--------|
| window bg | `#1e1e1e` or `#323232` elevated |
| panel | `#2c2c2e` |
| separator | `rgba(84,84,88,0.65)` |
| label | `rgba(255,255,255,0.85)` |
| secondaryLabel | `rgba(235,235,245,0.6)` |
| accent | `#0A84FF` |

### Clear
Keep NSVisualEffect vibrancy path; if still gray, boost material (sidebar/hud) + slightly stronger tint so content doesn’t look like a gray slab.

### Linux note (code comment only)
When GO-LS-TAURI-LINUX: map to Adwaita light/dark window_bg/view_bg/headerbar (libadwaita CSS variables) — not this slice.

Optional: expose `color-scheme: light|dark` (already) and prefer `accent-color: -apple-system-blue` where webview supports.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build
```
Summarize. No nest STAMP.
