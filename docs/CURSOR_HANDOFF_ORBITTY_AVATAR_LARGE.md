# Cursor Ultra — GO-LS-TAURI-AVATAR-LARGE · Stage 2 only

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Do not** edit Swift `~/goose artifacts/livesprite`.
Matt blessed the first companion: `brand/companion/spirit/ghost.png` + `ghost.gif`. Ship S2 now. Do not redraw.

Read first:
- `docs/ACCEPTANCE-ORBITTY-AVATAR-LARGE.md`
- `~/.buzz/PLANS/ORBITTY_AVATAR_LARGE_LOCK_v0.md`
- `~/.buzz/PLANS/BRIEF_ORBITTY_AVATAR_LARGE.md`

## What you build

A third webview, same trick as Lists/`kanban`.

### Window
- `src-tauri/tauri.conf.json`: add window `label: "companion"`, `visible: false`, `transparent: true`, `titleBarStyle: "Overlay"`, `hiddenTitle: true`, `title: ""`, `dragDropEnabled: false`.
- **No** `maxWidth: 360` (that is the rail). Companion is large. `minWidth` ~280, `minHeight` ~360. Default size ≈ ⅓ of a typical laptop (e.g. 520×780) — on first show, **position trailing-edge** of the main display (compute from current screen; persist after the user moves it).
- Portrait: if `screen.height > screen.width`, first show may `maximize`. Persist user choice after they un-maximize.
- CloseRequested → hide (add `"companion"` next to `"main", "kanban"` in `lib.rs`).
- `ensure_glass`: include `"companion"` (transparent + hide traffic lights). **Do not** `pin_stay_visible` on companion.

### Commands / tray / rail
- `open_companion` — show + focus (clone `open_kanban`).
- Reuse `hide_window { label: "companion" }`.
- Tray menu item **Companion** (with Lists / Show / Restore / Quit).
- Rail: obvious text button **Companion** (not a tiny glyph). Same energy as Lists.
- SPA: `+page.svelte` already branches `label === "kanban"`. Add `label === "companion"` → mount new `src/lib/CompanionPage.svelte` only (no rail poll loop).

### CompanionPage
- Full-bleed nearest-neighbor canvas/img. **8 frames @ 140 ms.** `image-rendering: pixelated`.
- Prefer `ghost.png` (2048×384 sheet, 8×256) **or** `idle_0.png`…`idle_7.png` (already sliced from that sheet). `ghost.gif` is review-only (magenta); do not ship it as the runtime loop unless you chroma-key.
- Empty OS title; optional tiny in-app “Companion” + × hide. Drag region on chrome.
- **Use my image…** — reuse `packs::import_pack_image` (or a companion-specific copy into app-data `companion/user/`). Persist `companion.imagePackId`. Clear → default spirit.
- No Hair/Clothes/Eyes. No species picker. No talk. No STARS. No score.

### Persist
Extend `state.rs` / `state.json`:

```
companion: { visible, bounds, imagePackId }
```

Restore bounds + visibility on launch (default **hidden** like Lists, unless `visible` was true).

### Art
Play the blessed files. **Do not redraw, upscale, or “improve” them.** If the path is missing, fail obvious (one line of text “spirit missing”) — do not fall back to a Bitty or `chars.rs`.

Bundle:

```json
"../brand/companion/spirit": "companion/spirit"
```

in `tauri.conf.json` `bundle.resources`.

### Urgency (thin)
If a single honest `needs_human` signal already exists on the graph/claims, a **finite** opacity pulse is allowed. If wiring that is messy, **idle only** in S2. Clinic / 2h slice → no pulse. Do not invent urgency.

## Forbidden
- S4 editor tabs / species mixer
- Talk, STARS, EHR, PHI, points on the buddy
- LPC / Habitica / itch / Kenney
- Neko click-through overlay
- Pin companion always-on
- Touching planet Bitty packs except to leave them alone
- `cargo build` as the install

## Verify

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```

If dmg fails but `src-tauri/target/release/bundle/macos/Orbitty.app` exists, the script still installs.

Summarize files + residuals. **Do not** write nest `STAMP_*.md`.
