# Cursor Ultra — GO-LS-TAURI-S5

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

READ:
- `docs/ACCEPTANCE-S5.md`
- `~/.buzz/PLANS/LIVESPRITE_S5_LOCKS_v0.md`
- `~/.buzz/PLANS/LIVESPRITE_THIN_SLICE_LAWS_PIXEL_v0.md` §5–6
- Pixel locks Hermes × LiveSprite (§Hermes × LiveSprite)

## Implement

### Launch Hermes
Rust command `launch_coordinator { projectId, projectName, titles: string[] }`:
1. Write `app_data/context/{projectId}.md` (titles only).
2. Try in order: focus existing “Hermes” app; `open -a Hermes`; `hermes desktop`; `hermes gui`; PATH `hermes`.
3. Do not embed an LLM. Return Err if nothing found (UI shows path hint).

Rail: **Hermes** button on each project section-head (and one on Board when a list is selected).

### soul.md
- `load_soul(projectId)` / `save_soul(projectId, body)` / `soul_path(projectId)`
- UI: small Soul panel (gear or section) — textarea + **Save** + **Open in editor** (`open` the file).
- Create template if missing.
- save_soul is the **only** writer.

### Persist
No new points. Optional `lastCoordinatorAt` skippable.

### Forbidden
No agent runtime, no STARS, no S6, no itch, no Swift tree, no PHI scrape.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# codesign Local Codesign → ~/Applications/LiveSprite.app
```
Summarize. No nest STAMP.
