# Companion spirit — blessed rest + thinking swap

Matt 2026-08-23: `spirit-idle-hq-v5/out_v5` is the **rest / default** idle.
Matt 2026-08-25: `spirit-idle-thinking-v9` is the **thinking / loading** swap only.

| File | What |
|------|------|
| `spirit-idle-hq-v5/out_v5/idle_0.png`…`idle_7.png` | Rest source 256×384 @ 140 ms |
| `idle_0.png`…`idle_7.png` | Rest ship path (what the .app bundles today) |
| `ghost.png` / `idle.png` | 2048×384 sheet copy of **v5 rest** |
| `spirit-idle-thinking-v9/out_v9/idle_0.png`…`idle_7.png` | Thinking source (rose→space-dust rim) |
| `thinking/think_0.png`…`think_7.png` | Thinking ship path (do not overwrite rest idle_*) |
| `thinking/think.png` | 2048×384 thinking sheet |
| `bust_0.png`…`bust_7.png` | Head closeup ship path — `bake_spirit_bust.py`, tight camera = the old top-128px closeup of v5 rest tightened to the face, hard alpha, nearest-neighbor (SPIRIT-ROLLBACK 2026-08-28; the head+neck+collar "timid" reframe is dead) |
| `thinking/bust_0.png`…`bust_7.png` | Head thinking closeup ship path (same tight camera over v9) |
| `bust_strip.png`, `thinking/bust_strip.png` | Bust review strips — review only, never bundled |
| `look/look_0.png`…`look_7.png` | GO-ORBITTY-LOOK-AT 8-dir eye cells (E NE N NW W SW S SE) from idle_0 |
| `look/bust_0.png`…`bust_7.png` | Same 8-dir on the Head bust |
| `look/overlay_*.png`, `look/overlay_bust_*.png` | Reaction overlay: blink, poke, sleepy, sparkle, delighted, dizzy |
| `look_strip.png`, `overlay_strip.png` | Look-at review strips — review only, never bundled |
| `look/look_0.png`…`look_7.png` | GO-ORBITTY-LOOK-AT 8-dir eye cells (E NE N NW W SW S SE) from idle_0 |
| `look/bust_0.png`…`bust_7.png` | Same 8-dir on the Head bust |
| `look/overlay_*.png`, `look/overlay_bust_*.png` | Reaction overlay: blink, poke, sleepy, sparkle, delighted, dizzy |
| `look_strip.png`, `overlay_strip.png` | Look-at review strips — review only, never bundled |

## Product lock (spark companion only)
- **Rest:** v5 idle loop (rose rim at rest pose).
- **While companion is waiting on a reply / test loading:** play thinking v9 loop on the **spirit itself**.
- **When the response is fully in:** snap back to v5 rest idle.
- **No loading animation inside a text bubble.** The wait mark (Orbitty ring/brick/planet) must leave the pending bubble / CC bot row — thinking lives on the spirit, not in chat chrome.
- Does **not** replace blessed rest idle v5. Preview GIFs are review only — do not ship in the .app.

Blessed still: `talk_v4_bright.png` (do not redraw).
8 frames, 256×384, 140 ms, hard alpha.

Do not redraw rest idle. Older `talk_*` / raw Imagine are research.
