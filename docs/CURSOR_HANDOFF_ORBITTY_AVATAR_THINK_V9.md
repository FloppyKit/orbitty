# Cursor Ultra — AVATAR-THINK-V9 · spirit thinking swap + no bubble wait

**Model:** `kimi-k3-max` · **one writer**  
**Gate:** `GO-LS-TAURI-AVATAR-THINK-V9`  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`  
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`  
No Swift. No Imagine. Do **not** redraw PNG pixels.

Matt (via Pixel Artist): spark companion should show the **thinking spirit loop** while a reply is loading, and **never** put the loading animation in a text bubble. When the response is fully in, return to the current rest idle.

## Art already on disk (do not regenerate)

| Role | Path |
|------|------|
| Rest (blessed v5 — **unchanged**) | `brand/companion/spirit/idle_0.png`…`idle_7.png`, `ghost.png` |
| Thinking v9 ship frames | `brand/companion/spirit/thinking/think_0.png`…`think_7.png`, `think.png` |
| Pack + handoff | `brand/companion/spirit/spirit-idle-thinking-v9/` |
| Brand index | `brand/companion/spirit/INDEX.md` |

Locks:
- `~/.buzz/PLANS/LIVESPRITE_COMPANION_THINKING_V9_LOCK_20260825.md`
- Prior residual on S2.9 stamp: wait mark should leave the bubble (this GO closes that).

## Implement

### 1) Bundle thinking frames
`src-tauri/tauri.conf.json` → `bundle.resources` — add mappings:
```
../brand/companion/spirit/thinking/think_0.png → companion/spirit/thinking/think_0.png
… through think_7.png
```
(optional: `think.png` sheet as thinking fallback, mirror of `ghost.png`)

### 2) Native dual cycle — `src-tauri/src/companion.rs`
- Keep rest load of `idle_0…7` / `ghost.png` fallback.
- Also load `companion/spirit/thinking/think_0…7` the same way.
- Extend `CompanionArt` (camelCase JSON) e.g.:
  - `frames` = rest
  - `thinkingFrames` = thinking (empty vec OK if missing — fail soft)
  - `frameMs` = 140
- Rest still required; missing thinking → rest-only (no crash).

### 3) UI swap — `src/lib/CompanionPage.svelte`
- Hold `restFrames` + `thinkFrames` from `companion_frames`.
- Active draw list = `sending && thinkFrames.length ? thinkFrames : restFrames`.
- On `sending` false→true: `frameIdx = 0` on think set.
- On `sending` true→false: `frameIdx = 0` on rest set.
- **Expanded + collapsed spirit/head** both use the active list (head crop stays top 128px of current frame).
- **Remove bubble wait chrome:**
  - Expanded: delete the `{#if sending}` pending row with `{@render wait()}` inside `.bubble.pending`.
  - Collapsed: when `sending`, do **not** render `{@render wait()}` inside `.cc.bot`. Hide the them CC row while sending **or** keep last `botLine` until the new reply replaces it — **no spinner in any bubble**.
- Move a11y to the spirit stage if needed (`aria-busy` / sr-only "waiting") — not chat chrome.
- Delete unused `.wait` / `.wmark` snippet + CSS if nothing references them after.

### 4) Out of scope
- Rail, bittys, planets, Hermes path, form editor.
- Do not overwrite rest `idle_*` with v9.
- Do not manual-hack `~/Applications/Orbitty.app` resources; use install-stable only.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "$HOME/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
# open ~/Applications/Orbitty.app → companion window
```
1. Idle: rose-rim **v5** loop.  
2. Send a line: spirit (and expanded head) plays **thinking v9** (purple+rose rim).  
3. **No** ring/brick/planet spinner in transcript bubble or CC bot row.  
4. Reply lands: spirit back on v5; bot text normal.  
5. `prefers-reduced-motion`: no bubble spinner; spirit may hold frame 0 of active set.  
6. Build green.

## Done when
Pixel can dogfood the installed app. Write nothing as STAMP yourself — Pixel stamps after eyes-on.

Acceptance: `docs/ACCEPTANCE-ORBITTY-AVATAR-THINK-V9.md`
