# ACCEPTANCE — GO-ORBITTY-ASK-ACTION

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- Typing an existing Action **name** (or `run <name>` / `/run <name>` / `action <name>`) runs it without calling the companion agent. Them-line reports `name: done` or `name: exit N`.
- Unknown lines still go to the companion agent.
- Two similar names: exact wins; still two → `which action? use the Launchpad well.`, do not fire.
- `please run Build` still goes to the agent.

## Must not

- New Actions dest on companion
- Model inventing a shell command
- NLP / fuzzy match
- TRAY-REMOTE / HOST-CC in this binary
