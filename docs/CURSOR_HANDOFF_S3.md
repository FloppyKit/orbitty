# Cursor Ultra — GO-LS-TAURI-S3

**Model:** kimi-k3-max · ONE writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

**READ:**
- `~/.buzz/PLANS/LIVESPRITE_S3_LOCKS_MATT_20260821.md`
- `~/.buzz/PLANS/ROADMAP_LIVESPRITE_FULL_TAURI_v0.md` §S3
- `docs/ACCEPTANCE-S3.md`
- Swift **reference only** (do not edit):  
  `~/goose artifacts/livesprite/Sources/LiveSprite/StowController.swift`  
  `OverlayController.swift` (per-planet NSPanel @ `.normal`, orderFront on down)  
  `HabitatViews.swift`

## Goal
Stow / OS-minimize of a **home-claimed** window → idle pixel sprite on that project's planet. Click restore. Collapsed folder → marble. Blessed install script.

## Implement

### Stow (Rust + AX)
- `minimize_window(id)` / `restore_window(id)` via AX minimized attribute (Swift StowController)
- List already has `isMinimized`
- Rail: compact **Stow** / **Restore** on home-claimed rows (not unmapped)
- Observe minimize: if `isMinimized && hasHome` → sprite; no extra journal required beyond AX truth
- Optional light persist: planet positions `{projectId, x, y, screenId}`

### Claim-first nudge
- Stow or detect minimize on unmapped:
  - if windowId not in `nudgeSuppressedWindowIds` → rail banner: pick/create home or dismiss
  - persist suppress after one show
- Never auto-claim

### Habitat (planets)
- **One borderless NSPanel per project that has a home claim** (or all projects with claims), `level = normal`
- Click/drag planet → orderFront; stay peer (not desktopIcon+1 always-behind)
- Expanded rail section → large planet (~72pt); **collapsed** → **marble** (~18pt)
- Minimized home windows → simple pixel sprite walking surface (idle frames OK; reduce-motion = static)
- Open home windows → small occupancy dot (optional, Swift had this)
- Click sprite → restore+focus
- Unmapped / tags-only → no sprite
- Exclude LiveSprite’s own windows

### Install C (required)
`scripts/install-stable.sh`:
```
npm run tauri build
sign ~/Applications/LiveSprite.app with "LiveSprite Local Codesign"
rm old dest, cp -R bundle app
print: quit LS, open only ~/Applications/LiveSprite.app, AX that path
```
Identity exists on this Mac. Fallback ad-hoc if identity missing, but prefer Local Codesign.

### Thin-slice / forbidden
No points, Kanban complete, coordinator, soul.md, Swift edits, Android, planet zoom/decor/music.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Summarize files + how to dogfood planets. No nest STAMP.
