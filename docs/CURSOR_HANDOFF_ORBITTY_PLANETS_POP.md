# CURSOR HANDOFF — GO-ORBITTY-PLANETS-POP

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_PLANETS_POP_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — Planets popup; load `projects` from `load_state` (CompanionPage `PersistedState` does not declare projects today)
- `src-tauri/src/habitat_mac/mod.rs` — `bring_planet_front` next to `bring_all_front`
- `src-tauri/src/habitat.rs` — add to `pub use habitat_mac::{ ... }`
- `src-tauri/src/lib.rs` — thin command + `generate_handler!`

## Do

1. Planets dest (the Planets text button in `.agentrow` between spark and speaker — **not** a `.destrow`) **toggles** a popup **over the well / stage / CC**, never over `.composer`. Not a new window. Not a slide-under-field. Hosted: overlay `.hostwell`. Do not dismiss the host.
2. First row **All planets** → `bring_all_front`. Then one row per `projects[].name`. Click → `bring_planet_front(project_id)` = `orderFrontRegardless` on **that** panel only. Unknown id = no-op (do not fall back to all).
3. Close on pick / Escape / click-outside / second click on Planets. `aria-expanded` on Planets.
4. No projects: "No planets yet." Omit All (Pixel lean).
5. Load `{ id, name }[]` in `loadState` + `state:changed`. Same shape as RailPage `interface Project`.
6. `./scripts/install-stable.sh`; stop.

## Do not

Rail section expand. Actions in the popup. Cover the type field. Dismiss host well. Fold HOST-CC / BOARD-ACTIONS. `tauri dev`. Stamp.
