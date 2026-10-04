# Orbitty 1B keyword vocab v0

Features for the small router — not the sole brain.  
Soft intent first; these boost scores. Slots still fill from `state.get` / `windows.list` / planet list.  
Low confidence → escalate. Never invent tool names.

Planet / agent / window **names** are dynamic: always match against live state (case-insensitive), not this file.

---

## 1. `ack` — heard you, no tool
**Keywords / phrases:** thanks, thank you, thx, ty, ok, okay, cool, got it, nice, perfect, yes, yup, yeah, nah (alone), nm, never mind, nvm, 👍, 🙏  
**Do:** short ack / gesture only. **Don’t:** open panes, mint cards.

## 2. `refuse` — PHI / out of scope / unsafe
**Keywords:** patient, chart, PHI, diagnosis, prescribe, eRx, password, seed phrase, private key, API key (paste), “run this curl”, sudo rm, format disk  
**Do:** refuse or Needed. **Don’t:** `action.run` / shell.

## 3. `claim` — window → planet *(needs claim.* MCP when shipped)*
**Keywords:** claim, park, pin this, put this on, put on, assign window, home this, sprite this, “on &lt;planet&gt;”, “to &lt;planet&gt;”  
**Slots:** frontmost or named window; planet (last-used if omitted).  
**Near:** “unclaim”, “release”, “free this window” → unclaim when tool exists.

## 4. `focus_window` — find / restore
**Keywords:** focus, restore, bring back, show window, find window, where is, wake sleeper, wake  
**Tools:** `windows.focus` · `windows.restore` · `sleeper.wake`  
**Slots:** window id / sleeper id from list.

## 5. `window_chrome` — minimize / close
**Keywords:** minimize, minify, hide window, close window, dismiss window  
**Tools:** `windows.minimize` · `windows.close`  
**Careful:** close ≠ Done on a card.

## 6. `board_add` — mint todo
**Keywords:** todo, to-do, add card, add task, new card, remind me, “GO-…”, runner, put on the board, kanban add  
**Patterns:** `todo:` · `- [ ]` · `GO-ORBITTY-…`  
**Tools:** `kanban.add`  
**Slots:** title; optional planet (`on orbitty`, `-fkt`, project name from state).

## 7. `board_move` — column change
**Keywords:** start, doing, mark done, complete, finish, checkbox, move to todo, move to doing, move to done, park card, reopen  
**Tools:** `kanban.move`  
**Slots:** card id/title match; status todo|doing|done.

## 8. `board_notes` — annotate
**Keywords:** notes, note that, blurb, summary line, annotate  
**Tools:** `kanban.notes`

## 9. `board_assign` — agent on card
**Keywords:** assign to, give to, hand to, pickup for, “&lt;agent&gt; take”  
**Tools:** `kanban.assign` · maybe `agent.pickup`  
**Slots:** card + agent name from `agent.list`.

## 10. `needed` — ask Matt / blocker
**Keywords:** needed, need you, blocked, waiting on, question for matt, escalate, “can’t proceed”  
**Tools:** `needed.open`  
**Slots:** title + why.

## 11. `planet` — project CRUD
**Keywords:** new planet, new project, rename planet, rename project, delete planet, remove planet  
**Tools:** `project.add|rename|rm`  
**Careful:** rm is destructive — confirm or escalate if ambiguous.

## 12. `open_pane` — show UI
**Keywords:** open board, open kanban, open launchpad, open stars, open companion, show companion, open inspector, open rail, open windows  
**Tools:** `kanban.open` · `stars.open` · `companion.open`  
**Note:** rail/windows may be tray-only until MCP grows.

## 13. `companion_send` — talk path
**Keywords:** tell companion, say to spark, whisper, “send: …”  
**Tools:** `companion.send`  
**Usually escalate** to bigger mouth unless line is short/explicit.

## 14. `action_run` — named recipe only
**Keywords:** run action, run shortcut, fire, do the “&lt;action name&gt;”  
**Tools:** `action.run` after `action.list` match  
**Not:** inventing shell → that is escalate / template, not freestyle `action.add`.

## 15. `agent_ops` — list / pickup / pin (not freestyle add)
**Keywords:** list agents, pickup, pick up next, pin agent, who’s free  
**Tools:** `agent.list` · `agent.pickup` · `agent.pin`  
**`agent.add`:** only if harness+name explicitly given; else escalate.

## 16. `shift` — humming supervisor
**Keywords:** shift on, shift off, shift status, tick shift, humming  
**Tools:** `shift.on|off|status|tick`

## 17. `status` — health / state ping
**Keywords:** status, are you up, accessibility, ax grant, what’s running, state  
**Tools:** `app.status` · `app.accessibility_status` · thin `state.get`

## 18. `log` — breadcrumb
**Keywords:** log that, note in log, breadcrumb  
**Tools:** `log.append`

## 19. `escalate` — big seat
**Keywords:** figure out, design, why, architecture, refactor, write a plan, profound, “what should we”, debug this properly, draft an action that…  
**Do:** hand to Goose/Grok/Cursor / companion Work mode. **Don’t:** fake a tool call.

## 20. `color` — planet chrome *(when PLANET-COLOR ships)*
**Keywords:** recolor, change color, make &lt;planet&gt; more violet, hue, swatch  
**Do:** companion/settings path or future MCP; not freestyle RGB.

---

## Board / house verbs (always boost)
`todo` · `doing` · `done` · `Needed` · `GO-` · `claim` · `walker` · `planet` · `rail` · `board` · `stars` · `shift` · `spark` · `packet` · `leftover` · `sleeper`

## Negative / trap (usually NOT hot-path)
“delete everything” · “email the patient” · “post to x” · “pay” · “buy” · raw paste of secrets · multi-paragraph design riffs → `escalate` or `refuse`

## Slot cues
- **Planet:** `on X` · `to X` · `-x` · `#x` · after “planet/project”  
- **Card:** quoted title · GO- id · “the doing card” · “that runner”  
- **Window:** “front” · “this” · “frontmost” · app name from `windows.list`  
- **Agent:** names from `agent.list` only

## Priority if two intents fire
`refuse` > `escalate` > `needed` > `claim` / `board_*` > `focus_window` > `open_pane` > `ack`
