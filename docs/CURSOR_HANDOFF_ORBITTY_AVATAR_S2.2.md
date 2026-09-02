# Cursor Ultra — AVATAR-LARGE S2.2 · composer under ghost

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Do not** edit Swift `livesprite`. **Do not** redraw `ghost.png`.
**Do not** put an LLM inside Orbitty.

Law: `~/.buzz/PLANS/ORBITTY_COMPANION_PLAN_v0.md`  
Talk goes to an **existing** Hermes session. Companion = face + captions + composer. Not a second harness.

Matt: put the **chat bar under the avatar**. × was too centered — pin it while you are here.

## Layout
```
[ × ]                    ← top-**right** (traffic lights own the left)
[     ghost (drag)     ]
[ last user line       ]  ← 2-line CC, hide a line if empty
[ last bot line        ]
[ [  type…        ] ➤ ]  ← composer: field + Send. Not a drag region.
```

- Ghost still fills the **upper** area; `object-fit: contain`; nearest; drag via `startDragging` on the stage/img only.
- × `position: absolute; top: 8px; right: 10px;` — **not** centered, **not** under traffic lights.
- Composer: obvious text field + **Send** button (not a lone glyph). Enter sends. Mic = later (PTT), do not ship a dead mic.
- No title. No Use my image.

## Send (honest)
Reuse `coordinator::launch_coordinator` / focus Hermes. Pass the typed line as the query (or write it into the existing context file and focus the running Hermes app).

- Orbitty does **not** call an LLM.
- Do **not** mint a new session (`--create-if-missing` only if that is already how launch_coordinator works — prefer focus running Hermes).
- After send: show the line as the latest **user** CC row; clear the field; keep the ghost looping.
- If Hermes is missing: one obvious line of text (same honesty as the rail), do not swallow the error.

No transcript scroller in this pack. No bot reply ingest unless a read path already exists (it does not — leave the bot CC line blank).

## Persist
Remember last user CC line in `companion.lastUserLine` if easy; else memory of the window session is enough.

## Verify
`./scripts/install-stable.sh` → `~/Applications/Orbitty.app`

Forbidden: Unity, second model loop, STARS, editor, rub/react, PHI, cargo-only install.
Do not write nest STAMP_*.
