# Cursor Ultra — STARS S1.1 dogfood

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift. No art redraw. No LifeRPG.

Matt dogfood:
1. **Titles missing / some missing** on the rail. Fix.
2. Chrome **does not fit** one row. Stack it.
3. **STARS** must **pop out** like Board (own window), not an inline pane.

## Titles
- Unless **Hide titles** is on, every row shows a title.
- `titleFor`: override if non-empty, else live AX title, else `appName`. Never blank, never a lonely “—”.
- `.wtitle` cannot be crushed to 0px by tags (keep `flex: 1; min-width: 4.5em; overflow: hidden; ellipsis; nowrap`).
- Do not persist `hideTitles: true` as a default. If STARS save accidentally wrote it, stop.

## Stacked chrome (rail maxWidth is 360)
After the banner, a **vertical** stack, one text button per line, order **exactly**:

1. Home  
2. STARS  
3. Board  
4. Companion  
5. Agents  
6. Actions  
7. Projects  

Then the list for the selected lane (Projects default). Kill the horizontal `.chrome-bar` + `.lane-tabs` row. Sticky: banner + this stack stay fixed; list scrolls.

Home / Board / Companion / STARS = same jobs as today (front planets / open window). Agents / Actions / Projects = lane.

## STARS window
Clone the Lists pattern:
- `tauri.conf.json` window `label: "stars"`, hidden at launch, overlay titlebar, empty/hidden OS title, ~420×560, transparent.
- `open_stars` show+focus. Close / × = **hide**, not destroy. `capabilities/default.json` add `"stars"`.
- Same SPA: `label === "stars"` mounts a `StarsPage.svelte` (move the radar + 0–4 + Save today + Prompts out of `+page.svelte`).
- `ensure_glass` / hide traffic lights like kanban. Persist bounds if easy; don’t block.
- Rail **STARS** calls `open_stars`. No inline `stars-panel` on the rail.

Companion Prompts strip can stay; it already reads `state.stars`.

## Verify
`./scripts/install-stable.sh`
No STAMP_*.
