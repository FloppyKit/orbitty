# Cursor Ultra — wait-in-slot (think-v9 leftover)

**Model:** `kimi-k3-max` if listed, else `composer-2.5` · **one writer**  
**Gate:** Matt **go: wait-in-slot** via Pixel Artist 2026-08-27  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`  
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`  
**File:** `src/lib/CompanionPage.svelte` only (comments + markup + CSS). No Rust. No PNGs. No rail.

## Why
Think-v9 is eyes-on green except placement. The Orbitty solar-system wait mark currently floats **above** the transcript / under the stage (`.wait-float` sibling, not in the agent-text slot). Matt wants it **where the agent reply text will appear**, still **not** a chat bubble (or a clear / no-border shell).

Spirit thinking v9 loop stays. Do not put the mark inside `.bubble` / `.cc.bot` with fill + edge.

## Do

### Expanded
- Delete the `{#if sending}` `.wait-float` that sits **between** `.exphead` and `.transcript`.
- Inside `.transcript`, after the `{#each turns}` loop, while `sending`:
  ```svelte
  <div class="row them wait-row" aria-live="polite">{@render wait()}</div>
  ```
- Same left alignment as a `them` bubble. Last row. Scroll-to-bottom (already watches `sending`) must pin this row into view.

### Collapsed
- Delete the `{#if sending}` `.wait-float` under `.stage`.
- While `sending`, render the mark in the **bot CC slot** (same place `.ccrow.them` / `.cc.bot` lives):
  ```svelte
  <div class="ccrow them wait-row" aria-live="polite">{@render wait()}</div>
  ```
- Keep hiding the real bot CC line while `sending` (`botLine && !sending` stays).
- User CC row unchanged.

### Chrome
- Bare mark, **or** a transparent / no-border shell. **No** `background`, **no** `border` on the wait chrome.
- Keep `{#snippet wait()}` (Orbitty ring / brick / planet). Keep `.wait` / `.wmark` CSS + animation.
- Drop unused `.wait-float` CSS (or restyle it into `.wait-row`: `justify-content: flex-start`, padding matching `.row` / `.ccrow`, `pointer-events: none`).
- `role="status"` / sr-only "waiting" stay on the snippet.

### Comments
- Header S2.7–v9 blurb: wait mark lives **in the agent-text slot**, not a bubble, not floating under the stage.
- `sendLine` comment: same.

### Out of scope
- Jacob / packs / thinking frames / companion.rs
- STARS, PTT, sleepers, rail
- Do not rewrite rest or thinking PNGs
- Do not cargo-only install; use `./scripts/install-stable.sh`
- Do not stamp

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "$HOME/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Then Pixel dogfoods `~/Applications/Orbitty.app` (tray Quit first).

1. Send a line, **collapsed**: solar-system mark appears **left, in the bot-text slot** (where the reply will land). No filled bubble, no border.
2. **Expanded**: mark is the last left-aligned transcript row, same slot a `them` bubble uses.
3. Spirit / head still plays thinking v9 while `sending`.
4. Reply lands: mark gone, bot text in that same place.
5. Reduced motion: no spin; spirit may hold frame 0.

Acceptance: `docs/ACCEPTANCE-ORBITTY-AVATAR-THINK-V9.md` (wait-in-slot bullets).
