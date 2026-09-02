# Cursor Ultra — AVATAR-LARGE S2.4 · real send + agent + CC + expand

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift tree. No redraw of `ghost.png`. **No LLM inside Orbitty** — the chosen Hermes **profile** is the agent.

Matt dogfood:
- × left is good.
- Send only **focused** Hermes and showed `Focused the running Hermes. Context: …/companion.md`. **No chat landed.** That banner must die.
- User picks **which agent** (Hermes profile) for the companion.
- See **back-and-forth in this window** (CC collapsed; full transcript when expanded).
- **Expand** covers most of the ghost; a **small head** stays top-L or top-R.

## Why send failed
`companion_send` wrote a file and called `focus_running_hermes()`. The desktop app does not ingest that file. Stop opening Hermes.app on send.

## Send (this is the product)
Use the CLI that already exists:

```bash
hermes -p <profile> chat -q "<line>" \
  --continue "orbitty-companion" --create-if-missing \
  -Q --source orbitty
```

- `<profile>` = persisted `companion.agentProfile` (see picker).
- `--continue "orbitty-companion" --create-if-missing` = **one named thread**, not a new session every send.
- `-Q` = final reply + session info only. Parse stdout as the **bot** line. Persist both turns.
- Do **not** `--create-if-missing` without `--continue <name>` (that was the old last-resort and mints junk).
- Empty line = no-op.
- Run off the UI thread (async). While in flight: Send disabled / “…” on the bot CC row. On fail: honest one-line error (no filesystem path dump). Keep the draft.
- Prefer `--query-file` if the line has quotes/`$()`.

## Agent picker
List **Hermes profiles on disk**: `~/.hermes/profiles/*` directory names, plus a row for the **default** profile (`hermes` with no `-p`).
This is the same roster as Bot Mode / the Agents lock (`hermes -p <profile>`). The rail Agents lane today is only per-project Hermes+Soul — **do not** invent a second list. Companion picker reads the profile dirs.

- Obvious control: **Agent** + name, cycle or menu (text, not a tiny glyph).
- Persist `companion.agentProfile`. Changing agent starts/continues **that** profile’s `orbitty-companion` thread (name is per-profile home).
- Default if unset: `grok` if that dir exists, else default profile.

## Collapsed (default)
```
[ × ]
[        big ghost (drag)        ]
[ you: last user line            ]  ← hide if empty
[ them: last bot line            ]  ← hide if empty; "…" while sending
[ Agent: grok          ] [Expand]
[ [ type…                   ] Send ]
```

True CC: **only** latest user + latest bot. Not a scroller.

## Expanded
A text **Expand** / **Collapse** button (not a lone glyph).

```
[ × ]              [ small head ]
[ you: …                       ]
[ them: …                      ]
[ you: …                       ]
[ them: …                      ]
[ Agent: grok        ] [Collapse]
[ [ type…                   ] Send ]
```

- Ghost **head**: crop the **top 128px** of `idle_0` / current frame, draw ~96–128px near **top-right** (× stays top-left). Nearest, pixelated. No new art.
- Transcript: scroll the saved turns for this agent thread (user/bot). Same persist as CC.
- Window may grow taller on expand if easy; otherwise reuse current bounds. Persist `companion.expanded`.
- Stage/ghost remainder is **not** the drag handle when expanded if it would fight scrolling — drag on the head + a thin top strip.

## Persist (`companion`)
`agentProfile`, `expanded`, `lastUserLine`, `lastBotLine`, and a small `turns: [{role, text}]` cap (e.g. last 40) for expand. `visible` / `bounds` unchanged.

## Forbidden
Opening Hermes.app on send · showing raw context paths · second LLM in-process · minting untitled sessions · Unity · STARS · editor · rub/react · PHI · cargo-only install

## Verify
`./scripts/install-stable.sh`

Do not write nest STAMP_*.
