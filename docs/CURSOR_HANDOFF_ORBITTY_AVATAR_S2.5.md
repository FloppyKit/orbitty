# Cursor Ultra — AVATAR-LARGE S2.5 · SMS bubbles + instant you + v5 art

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift. Do **not** redraw pixels.

Matt: S2.4 looks really great. Now:
1. Expanded transcript = **SMS / iMessage bubbles** (you right, them left).
2. On Send, **your line appears immediately** in the chat — do not wait for the agent, do not only show it in the tiny CC after the reply.
3. New default art is already on the ship path (Pixel copied v5). Just bundle/play it. Do not invent frames.

## Art
`brand/companion/spirit/idle_0.png`…`idle_7.png` and `ghost.png` are now **v5** (256×384, 8 @ 140 ms, hard alpha). `tauri.conf.json` already lists those files — `install-stable` picks them up. Head crop still top 128px of the current frame.

## Optimistic you
On Send (before `companion_send`):
- Append `{role:"you", text}` to `turns`
- Set `userLine`
- Clear the field
- Scroll expanded transcript to **bottom**
- Show a pending **them** bubble / CC `…`

`companion_send` today appends **both** turns after the reply → would **duplicate** you. Change persist: if the last saved turn is already this same `you` line, **only** append `them`. Otherwise append both (old clients). Failed send: keep the you bubble; do not remove it; show the honest error; keep draft only if you never appended (you already did — so just error).

## SMS expanded
- You: right-aligned, filled bubble (use `--accent` / a warm fill, white or ink text that contrasts).
- Them: left-aligned, muted bubble (`--bg` / `--edge`).
- No `you:` / `them:` prefixes **in expanded**.
- Tight padding, 12–16px radius, max-width ~78% of the transcript.
- Pending `…` is a left bubble.
- Collapsed CC can stay the 2-line `you:` / `them:` captions (or small bubbles — expanded is the must).

## Verify
`./scripts/install-stable.sh`

No STAMP_*.
