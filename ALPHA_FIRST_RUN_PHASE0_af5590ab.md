# ALPHA-FIRST-RUN Phase 0

Base `af5590ab`. No code in this commit. Each beat is the path a fresh profile actually takes.

## B0 first launch, no Accessibility — PASS

- `src/lib/RailPage.svelte:1944-1954` loads `accessibility_status` on mount, then `tick` every 800ms.
- `src/lib/rail/pageDrag.svelte.ts:217-218` stores that status. Untrusted skips `list_windows`.
- `src/lib/rail/List.svelte:696-715` paints "Accessibility access required", Open System Settings, and Retry. The stage rail is the other branch (`:717`).
- `src/lib/rail/pageDrag.svelte.ts:387-391` Retry calls `prompt_accessibility` then `tick`.
- `src-tauri/src/windows/windows_mac.rs:777` `trusted` is `AXIsProcessTrusted`. `:931-936` `prompt_accessibility` is the system prompt.

## B1 fresh profile, zero claims — PASS

- `src-tauri/src/lib.rs:949` `starter::boot` runs in setup, before the window reads state.
- `src-tauri/src/agents/starter.rs:86-102` `v == 0` becomes 1. Zero claims, projects, and cards is fresh: not skipped, companion hidden. Any claim, project, or card is veteran (`skipped`, both outposts built). A veteran does not re-arm.
- `src/lib/rail/coach.ts:36-48` coach line when trusted, state loaded, zero claims, `v == 1`, not coach-done, not skipped.
- `src/lib/rail/coach.ts:52-58` `emptyDeskShowsWheel` is true only for zero windows, zero projects, armed, not skipped.
- `src/lib/rail/List.svelte:525-541` and `:784` use that so an empty fresh desk does not die on "No windows found".
- `src/lib/rail/List.svelte:860-862` renders `COACH_LINE`.
- `src/lib/rail/stageLook.ts:245-252` the wheel always ends with `rnewproj` ("+ Project", `List.svelte:412`).

## B2 first claim seeds exactly four todos once — PASS

- `src-tauri/src/agents/starter.rs:15-20` ids `c-starter-comms`, `c-starter-stars`, `c-starter-board`, `c-starter-claim`.
- `src-tauri/src/agents/starter.rs:126-141` `plan` emits `Seed` once, when a claim has a real project and no starter id is present.
- `src-tauri/src/agents/starter.rs:220-256` `apply_seed` writes those four cards and refuses a second call.
- `src-tauri/src/lib.rs:738-742` window `save_state` calls `after_save`. `src-tauri/src/board/board.rs:80` the Rust save door does too. Origin does not gate the seed.
- Notes (`starter.rs:28`) still say click the planet's sign. That path still opens the board (`habitat_mac/mouse_up.rs:132-140`). The rail board is B5, not a broken seed.

## B3 click Comms plot, walker walks, icon appears — GAP

Happy path is present:

- `src-tauri/src/habitat/habitat_mac/input.rs:99-118` a Plot hit returns Comms or Stars.
- `src-tauri/src/habitat/habitat_mac/mouse.rs:141` click calls `enqueue_build`.
- `src-tauri/src/habitat/habitat_mac/todo_tree.rs:293-318` keys the chore `outpost:comms` (not the card id), picks an awake walker, walks to the station.
- `todo_tree.rs:397-404` arrival calls `starter::outpost_built`.
- `src-tauri/src/agents/starter.rs:322-336` `outpost_built` → `mark_built` → `view` → `refresh_outposts` (`starter.rs:449-452`) stores Built in `OUTPOSTS`.

GAP: `todo_tree.rs:385-387` if that walker's `roam` entry is gone, the chore is skipped and left in the list. `enqueue_build` (`:296-298`) dedupes on the same key, so a second click does not retry. The icon never appears. Same hole for Stars.

## B4 Comms built, spirit and intro once — PASS

- `starter.rs:101` fresh arm sets `companion.visible = false`.
- `src-tauri/src/lib.rs:1123-1126` setup shows the companion only when that flag is true, and boot already ran.
- `starter.rs:333-339` Comms follow-on runs `exec_intro` then `show_companion`.
- `starter.rs:345-356` `apply_intro` writes `INTRO_LINES` (`:36-40`) once (`intro_done`).
- `src-tauri/src/windows/panes.rs:985` show sets `visible = true`.
- Default art is the spirit sheet. No face picker in `src/`.

Not a beat failure: tray, hotkey, and `companion.open` can still call `show_companion` before Comms (`lib.rs:1254`, `rail/rail_toggle.rs:48`). `panes.rs` and the tray arm are outside this GO's command-registration fence, so this walk does not call that a fixable GAP.

## B5 Stars, top rail stop, planet board beside the wheel — PASS

- Same plot → `enqueue_build` → `outpost_built` path as B3, kind Stars (`starter.rs:340-341` opens the stars pane via `panes.rs:432-437`).
- `src/lib/rail/stageLook.ts:245-246` `rstars` is the first wheel stop, then Settings, then planets.
- `src/lib/rail/List.svelte:451-454` a planet click `archEnter`s that planet.
- `src/lib/rail/stagePlace.ts:171-174` layer 2 opens centred on Board.
- `src/lib/rail/List.svelte:1041-1054` that centred board is the kanban pane beside the wheel.

## M model path — GAP

Chat log writer: `src-tauri/src/state/models_companion.rs:248-257` `push_turn` appends `companion.turns` and the `threads` shadow. That struct is `state.json`. No second companion chat file. Callers that persist a send: `coordinator/send.rs:369-371` and `:428-430`, `coordinator/intercept.rs:100` and `:130`.

| Read or write | Where | Today | Mark |
|---|---|---|---|
| Empty reply lane | `mouth_hop/handoff.rs:159-168` | empty → `"grok"` (or `"free"` if `free_mode`) | GAP |
| Lane normalize | `handoff.rs:149-157` | unknown, including empty string, → `"grok"` | GAP (alpha must keep `"none"`) |
| Lane save | `handoff.rs:448-451` `set_companion_reply_lane` writes `state.json` `companion.reply_lane` | file in app data, not a key | PASS for the lane field |
| Send uses lane | `coordinator/send.rs:192` then hops / CLI spawn | no `"none"` short-circuit | GAP |
| OpenRouter key | `mouth_hop/openrouter.rs:15-31` | `OPENROUTER_API_KEY`, then `companions_root()/openrouter.key` | GAP |
| Key save | `handoff.rs:690-714` `set_openrouter_key` | creates that file, mode 0600 | GAP |
| Key status | `handoff.rs:657-674` | env, then file | GAP |
| Model env | `openrouter.rs:40-47` `ORBITTY_OPENROUTER_MODEL` | read for the free model id | GAP |
| Compat key | `agents/direct_lane.rs:238-239` and `:312-318` | `companions_root()/direct.key` | GAP |
| Harness env | `companion/companion_backend.rs:126` | `ORBITTY_COMPANION_BACKEND` wins over state | GAP |
| Sidecar URL | `mouth_sidecar.rs:142` and `:130` | `~/.config/orbitty/mouth-sidecar.json` and `ORBITTY_MOUTH_SIDECAR_URL` | GAP for a mouth that is not this app's state |
| Kitchen settings | `plans/parse.rs:24` `~/.config/orbitty/settings.json`; `plans/go_runner.rs:88` `go-runner.json` | runner/model for GOs, not the companion reply lane | not the reply lane; kitchen create is the GAP below |
| Kitchen folder | `plans/dirs.rs:153-166` `plans_dir` | `create_dir_all(~/Orbitty GOs)` | GAP |
| Local probe | `mouth_scan.rs:10-13` | `127.0.0.1:11434`, `:1234`, and `:8080` | GAP: helper must hit only the first two |
| CLI spawn | `coordinator/launch_agent.rs:29-32` and `send.rs` grok/goose/hermes arms | PATH CLIs, no "No model" line | GAP until the helper offers them only after a pick |
| Keychain | no crate, no `security` call | keys are files | GAP |
| Chat redaction | `push_turn` stores the raw string | a pasted key is kept and can be sent | GAP |

No `alpha` feature in `src-tauri/Cargo.toml`. No Set up a model surface. No fixed No-model line.
