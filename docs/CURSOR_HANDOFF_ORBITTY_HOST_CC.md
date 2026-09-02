# CURSOR HANDOFF — GO-ORBITTY-HOST-CC

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_HOST_CC_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — only.

## Do

1. Delete the HOST-SPLIT line in `toggleExpanded` that calls `dismissHost()` when expanding while hosted. Expanding must not restore idle size.

2. Keep `.hostwell` mounted whenever `hosted !== "off"`, including while `expanded`. Do not unmount KanbanPage / StarsPage.

3. When `hosted !== "off"` && `expanded`: render `.transcript` as an overlay on top of `.hostwell` (absolute, inset 0 over the well). Dim the well; opaque bubbles. Composer + `.agentrow` stay below, usable. Hide the two `.ccrow`s while overlay is up.

4. When `hosted === "off"` && `expanded`: today's idle Expand (`.exphead` + `.transcript` filling the pane). Do not change.

5. Hosted collapsed CC: cap each `.cc` at ~2–3 lines (`max-height: calc(1.3em * 3 + 10px); overflow: hidden`). Idle `.cc` stays `max-height: 40vh; overflow-y: auto`. `title=` already set.

6. Escape: if Planets pop open, close it (today). Else if hosted && expanded, collapse overlay (`expanded = false` + `set_companion_expanded`). Do not hide the companion. Do not dismiss the well.

7. `enterHost` still force-collapses. Dest toggles and tray raise unchanged.

8. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Touch KanbanPage / StarsPage / panes.rs / lib.rs / tray. Persist `hosted`. Fold action-edit / CLEAR / walker-pack / planet-control. Grow/shrink the window on Expand.
