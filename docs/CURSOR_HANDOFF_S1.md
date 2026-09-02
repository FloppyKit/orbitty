# Cursor Ultra — GO-LS-TAURI-S1 (window module)

**Model:** `kimi-k3-max` · **One writer** · Tree: `/Users/dude/goose artifacts/livesprite-tauri`

**Roadmap:** `~/.buzz/PLANS/ROADMAP_LIVESPRITE_FULL_TAURI_v0.md` §S1  
**Acceptance:** `docs/ACCEPTANCE-S1.md`  
**Swift reference (READ ONLY):** `~/goose artifacts/livesprite/Sources/LiveSprite/`  
  - `WindowCatalog.swift` — AX enumerate, frontmost, filter tiny junk, id `pid:cgWindowID`  
  - `WindowFocuser.swift` — activate app, deminiaturize, raise  
  - `PermissionGate.swift` — AXIsProcessTrusted, path + signing honesty  
  - `AXHelpers.swift` — ax copy helpers / `_AXUIElementGetWindow` if used  

## Goal
macOS window list + click-to-focus + frontmost marker + Accessibility empty-state.  
**Unmapped list only.** No claim, sprites, planets, projects.

## Implement

### Rust (`src-tauri`)
1. Module e.g. `windows_mac.rs` (cfg macos):
   - `list_windows() -> Vec<WindowInfo>`  
     fields: `id`, `pid`, `app_name`, `title`, `bundle_id` optional, `is_frontmost`, `is_minimized`
   - `focus_window(id: String) -> Result<(), String>`
   - `accessibility_status() -> { trusted: bool, bundle_path: String, signing_line: String }`
   - `open_accessibility_settings()`
   - `prompt_accessibility()` — AXIsProcessTrustedWithOptions prompt if needed
2. Enumerate via Accessibility (preferred, match Swift) and/or CGWindowList as fallback; prefer Swift approach:
   - regular apps only, skip self pid
   - real AX windows (role window, drop &lt;40px)
   - id = `pid:cgWindowID` when available else fallback
3. Frontmost: system-wide focused app + focused window
4. Focus: activate app, unminimize, raise AX window
5. Link frameworks: ApplicationServices, AppKit, CoreGraphics as needed
6. Tauri commands invoke above; poll from frontend OR rust emit events every ~0.75s when trusted

### Frontend (Svelte)
1. Replace S0 hello body with:
   - Banner **LiveSprite** + S1 badge
   - If !trusted: empty-state — short body, **large path**, signing line, Open System Settings, Retry, Copy path
   - If trusted: scrollable list of windows (app · title); frontmost left accent / bold
   - Click row → `focus_window`
   - Refresh indicator optional
2. Compact TE aesthetic (dark tokens from S0 OK). Not full theme pack (S2).

### Info.plist / entitlements
- Ensure app can request Accessibility (standard; no special sandbox that blocks AX if currently none)
- Keep identifier `app.livesprite.mac`

## Forbidden
- Claim / projects / tags  
- Stow / sprites / planets  
- Points / Kanban  
- Dual-write Swift features  
- Scraping window contents beyond title  
- Android  

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build   # or tauri dev smoke
```
Manual: grant AX if needed → see windows → click focuses → frontmost updates.

## Done
Summarize files changed, how AX works, residual (adhoc TCC, etc.). No nest STAMP (Pixel stamps).
