# CURSOR HANDOFF — GO-ORBITTY-INSPECTOR

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_INSPECTOR_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/RailPage.svelte` — drop `.chrome-stack`, banner `.gear`, Actions lane tab + `lane === "actions"` branch. Keep `actionShortcuts` on `persist()`. Agents lane stays. Projects lane stays.
- `src-tauri/src/lib.rs` — insert tray item **Inspector** above Restore → `show_rail`. Do not touch left-click, Launchpad/STARS host-well, Reopen, PNG swap.
- `README.md` — one tray / ⌘⇧O sentence. Do not rewrite the README.

## Do

1. Remove `<nav class="chrome-stack">` (Companion / Board / STARS). Destinations are companion dests + tray.
2. Remove banner ⚙ / `openSettingsWindow` from the banner. Settings = companion ⚙ / tray Settings.
3. Drop Actions lane-tab. Drop `lane === "actions"` UI. Default lane `"projects"`. If lane was `"actions"`, fall back `"projects"`. Keep `actionShortcuts` state + persist key.
4. Tray menu after Settings, before Restore: **Inspector** → `show_rail`. Ids: do not reuse `"show"` (that was Show Orbitty). New id `"inspector"`.
5. README: ⌘⇧O = companion; tray left = companion; right = Show Companion · Launchpad · STARS · Settings · Inspector · Restore · Quit.
6. `./scripts/install-stable.sh`; stop.

## Do not

Delete the rail window. Delete Agents lane. Wipe `actionShortcuts` from persist. Companion dest for Inspector. Change TRAY-REMOTE left-click or Launchpad `enter-host`. Fold CLEAR / HOST-CC / action-edit. `tauri dev`. Stamp.
