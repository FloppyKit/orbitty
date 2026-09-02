# Cursor Ultra — GO-LS-TAURI-S2

**Model:** `kimi-k3-max` · **One writer**  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

**Roadmap:** `~/.buzz/PLANS/ROADMAP_LIVESPRITE_FULL_TAURI_v0.md` §S2  
**Acceptance:** `docs/ACCEPTANCE-S2.md`  
**Theme tokens (read):** `~/.buzz/PLANS/LIVESPRITE_DOGFOOD_UI_DESIGN_v0.md`  
**Swift reference (READ ONLY):** `~/goose artifacts/livesprite` Theme.swift / RailView — do not edit Swift.

## Goal
Rail chrome + 9 themes + projects + **explicit claim** persistence + replace red-square tray/app icon with a **rail glyph**. Keep S1 window list/focus/AX.

## Implement

### Frontend (Svelte 5)
1. Compact banner: **LiveSprite** left, **gear** far right.
2. Gear → panel/popover: **Theme** picker only (9 named themes).
3. Window list grouped: **Unmapped** + one section per project.
4. Claim UX: pick a window → assign to a project (or create project). Unclaim back to unmapped.
5. Persist: `projects`, `claims` (window id → project id), `themeId` to local file via Tauri command (JSON under app data dir).
6. No sprites, no planet canvas, no complete/points.

### Rust
1. `load_state` / `save_state` for `{ themeId, projects: [{id,name}], claims: [{windowId, projectId}] }`.
2. Keep existing window commands.

### Icon
Replace `src-tauri/icons/` placeholders (many are 70-byte dummies). Rail glyph: simple high-contrast vertical rail / stacked-bar mark, template-friendly for menu bar (monochrome). Generate png + icns/ico as Tauri expects. Update tray to use it.

### Themes
Port the 9 names/tokens from dogfood design. Liquid Glass: best-effort CSS backdrop-filter / transparent window; document residual if webview can’t match AppKit hudWindow.

## Forbidden
Sprites/planets/stow, points/Kanban, soul.md, Swift edits, Android.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build
```
Or `tauri dev` smoke if build is heavy. Summarize files + residuals. Do **not** write nest STAMP_* (Pixel/CoS stamps).
