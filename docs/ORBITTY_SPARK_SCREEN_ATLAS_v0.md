# Orbitty Spark screen atlas v0

Companion knowledge cards — one per seat/screen.  
Spark (1B + thin 4B) should answer “where am I / what does this do / how do I tweak it” from these, then MCP for verbs.  
Habitat still works with companion hidden.

Vocab features: `docs/ORBITTY_1B_KEYWORD_VOCAB_v0.md`  
MCP levers: live `orbitty-mcp` tool list (claim.* still a hole)

---

## CARD: Habitat (desk / planets / walkers)
**What it is:** The product. Windows become walkers only after claim. Click walker = restore. Done walks off. Marbles = idle chrome.
**User verbs:** claim, restore, Done, Put front window here, board mark on planet.
**Spark knows:** Layer 0 loop; unmapped = zero sprites; no generated pets; planet color override (swatches + constrained hue) when shipped.
**MCP:** windows.* · sleeper.wake · project.* · state.get · (claim.* when added)
**Not here:** full Needed inbox, STARS radar, companion chat as the front door.

## CARD: Tray / Orbitty Dock menu
**What it is:** Sole directory. Claim front window + Talk + Windows + Board + projects + Settings.
**Spark knows:** demo profile thin until 3 claims / first Done; no inject into Safari Dock; Hyper chords = operator Settings.
**MCP:** mostly pane opens + future claim.*; not a separate tray tool yet.

## CARD: Windows rail (Inspector)
**What it is:** Mac-Dock cousin on a non-bottom edge. Unclaimed Claim/Later + rows by planet; drag claim; click focus.
**Spark knows:** not a Kanban page; not stacked on companion dock; peer text → icons (RAIL-ICONS); collapse empty planets.
**MCP:** windows.list|focus|minimize|restore|close · sleeper.wake

## CARD: Board / Launchpad (Kanban)
**What it is:** Per-planet (and inbox) cards: todo / doing / done. Needed strip. Trackers/preview.
**Spark knows:** board mark on planet opens THAT planet only; not a peer hub to STARS/Comp; runners = GO- titles + notes.
**MCP:** kanban.open|add|move|notes|assign · needed.open

## CARD: Needed inbox
**What it is:** Questions / blockers for Matt. Standing rules can auto-answer.
**Spark knows:** escalate ambiguity here; not PHI; agent Needed ≠ patient care.
**MCP:** needed.open (+ answer path via ctl/watch)

## CARD: Companion dock + Talk
**What it is:** Mouth seat. Spark / modes / thin composer. Optional.
**Spark knows:** hideable; microharness lives on MCP+sleep, not in the bubble; 1B = quick ack + tool emit; 4B only when escalate.
**MCP:** companion.open|send|agents|set_agent|play

## CARD: STARS
**What it is:** Daily pulse (radar axes) + word bag / sky of done language. Operator depth.
**Spark knows:** Ring 1–2; one-sentence purpose TBD before grow; not first-run; not peer launcher.
**MCP:** stars.open · state.get (sky)

## CARD: Sky
**What it is:** Word/constellation field from done titles + companion sayings.
**Spark knows:** ambient memory chrome; not the board.
**MCP:** via stars/state — no sky.* tool yet.

## CARD: Settings
**What it is:** Sink. Appearance, packs, planet sleep, mouth/keys, caps, event log, cheat sheet, Herding Cats.
**Spark knows:** no peer doors required; BYOK / local / OpenRouter; spend + runner caps.
**MCP:** thin — many settings are UI-only today; packs.list · app.accessibility_status

## CARD: Cheat sheet
**What it is:** Action / chord overlay for operators.
**Spark knows:** Hyper family personal; civilian path is clicks.
**MCP:** action.list

## CARD: Actions
**What it is:** Named shell recipes + optional global chords.
**Spark knows:** 1B may action.run by name; action.add only with filled cmd / template — never freestyle shell from vibes.
**MCP:** action.list|add|run|rm

## CARD: Agents + Shift
**What it is:** Harness rows (cursor|goose|grok|hermes), pin, pickup; humming supervisor.
**Spark knows:** pickup moves assigned todo → doing + launch; Spark/Chat never auto-pickup; shift is operator.
**MCP:** agent.* · shift.* · kanban.assign

## CARD: Yard / Herding Cats
**What it is:** Optional easter overlay cats — not planet sprites. Confirm-gated.
**Spark knows:** thin-slice law; default zero; don’t teach as the product.
**MCP:** none / settings UI

## CARD: Event log + tasks
**What it is:** Append-only events; task boundary snapshots / undo when workspace root set.
**Spark knows:** build order log → shell hook → snapshots; undo-this-task on board when ready.
**MCP:** log.append · state.get · (task undo via ctl)

---

## Spark voice law (1B primary)
1. Prefer habitat truth + MCP over vibes.
2. Ack fast; tool call clean; escalate design / novel action.add / PHI.
3. Know every CARD above; if screen not in atlas, say so and open Settings or Needed.
4. Companion may hide; still answer “how does claim work” from Habitat card.
