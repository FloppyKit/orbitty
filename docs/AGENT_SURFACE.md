# Orbitty agent surface

The control socket is the interface. The GUI is one driver of the same `state.json`. Grok Bot, Hermes, Goose, `orbittyctl`, and any MCP client drive a running Orbitty through that socket. Nothing here starts the app.

Parity rule: no GUI feature ships without its control tool. `DESIGN.md` should get a one-line pointer to this file later; this GO does not edit `DESIGN.md`.

## Where the other docs live

- [ORBITTY_CONTROL_PLANE_v0.md](ORBITTY_CONTROL_PLANE_v0.md) — pointer to the nest spec (`~/.buzz/PLANS/ORBITTY_CONTROL_PLANE_v0.md`).
- [ORBITTY_MCP_GOOSE_SNIPPET.md](ORBITTY_MCP_GOOSE_SNIPPET.md) — Goose `config.yaml` for the `orbitty-mcp` stdio server. Same shape works for any MCP client that launches a stdio server (Hermes, Grok Bot, a hand-rolled client).

Binary: `~/.local/bin/orbitty-mcp` after a blessed install, or `src-tauri/target/release/orbitty-mcp` from a local build. CLI: `orbittyctl` (same directory).

Orbitty must already be running. The socket is `0600` under the app support dir:

`~/Library/Application Support/space.orbitty.mac/control.sock`

Nightly's bundle id is `space.orbitty.mac.nightly`. `scripts/install-nightly.sh` symlinks that support dir to the stable one when Nightly has no folder of its own, so the default socket path still reaches Nightly. If Nightly has its own real folder, pass `--sock` or `ORBITTY_CONTROL_SOCK`.

Optional auth: `ORBITTY_CONTROL_TOKEN`, else `control.token` beside the socket. ACL file: `control.json` (`profile`: `full` | `read` | `companion`, or `allow`: an array of tool names). A name may end in `*` (`stars.*`). Missing `control.json` means full.

## Socket

One line of JSON in, one line out. Protocol `v` is 1.

```json
{"v":1,"id":"1","tool":"stars.checkin.get","args":{}}
```

```json
{"v":1,"id":"1","ok":true,"data":{"date":"2026-09-24","saved":false,"sleep":0,"terroir":0,"athleticism":0,"relationships":0,"self":0,"win1":null,"win2":null,"grateful":null}}
```

```json
{"v":1,"id":"1","ok":false,"code":1,"error":"value must be an integer 1-5"}
```

`orbittyctl --json` prints `data` on success and the whole error object on failure. Without `--json` it prints a short line, or pretty JSON when there is no special printer.

## Exit codes

`orbittyctl` maps the socket `code` onto its process status.

| Exit | Socket `code` | Meaning |
| --- | --- | --- |
| 0 | — | Ok |
| 1 | 1 | Other (bad args, unknown tool, unknown id) |
| 2 | — | Orbitty is not running (no socket). Client-side only. |
| 3 | 3 | Auth failed |
| 4 | 4 | Tool denied by `control.json` |
| 5 | 5 | Accessibility not granted |

Stars reads are allowed on the `read` profile. Stars writes need `full` or a custom `allow` entry.

## Same store as the page

Check-ins, win lines, and journal pages live on the `stars` slice of `state.json`. Trackers and timers live on `projects[].trackers`. The Cloud word bag is computed on read from done cards, tag names, planet names (half weight), and your companion lines. Bot replies never enter it. Mute list is `sky.muted`. None of this is in webview `localStorage`.

A write loads state, changes that slice, and saves through the same door as `kanban.add` (`state:changed`). The STARS page reloads. The page's in-memory draft is not a second store: these tools read and write the saved day. A draft you have not saved yet stays on screen until you save or the page reloads clean.

Spoke scores are 1–5. `0` means unset. `stars.checkin.set` accepts 1–5 only.

`stars.log.set` changes only the fields you pass. `""` or `null` clears that line. `stars.log.save` trims the day that is already saved. It does not create an empty day.

`stars.journal.append` and `stars.reflect.write` refuse a blank body. Reflect's only template is `ledger`. It fills the page locally from that day's saved check-in, the win lines, current trackers, and one fixed cloud sentence. It does not call a model. Omitted section switches default on, matching the sheet.

`stars.tracker.inc` / `dec` / `set` are for number trackers. Timers use `stars.timer.start` / `stop`. `start` on a number tracker makes it a timer. A second `start` keeps the original start time. `stop` folds elapsed seconds into `value`.

`stars.cloud.words` is read-only. `range` is `14`, `week`, `month`, `year`, or `all` (default). It limits which done cards count. Your lines always count as now. At most 80 words come back, same cap as the cover.

Dates are local `YYYY-MM-DD`. Omitted `date` is today, except `stars.journal.read`, which requires one.

## Stars tools

`stars.open` was already there (show the shared pane on STARS).

### stars.checkin.get

```json
{"v":1,"id":"1","tool":"stars.checkin.get","args":{"date":"2026-09-24"}}
```

```
orbittyctl --json stars checkin get --date 2026-09-24
```

### stars.checkin.set

```json
{"v":1,"id":"1","tool":"stars.checkin.set","args":{"axis":"sleep","value":4}}
```

```
orbittyctl --json stars checkin set --axis sleep --value 4
```

Axes: `sleep`, `terroir`, `athleticism`, `relationships`, `self`.

### stars.log.get / stars.log.set / stars.log.save

```json
{"v":1,"id":"1","tool":"stars.log.set","args":{"win1":"shipped the tool","grateful":""}}
```

```json
{"v":1,"id":"1","tool":"stars.log.save","args":{}}
```

```
orbittyctl --json stars log set --win1 "shipped the tool" --grateful ""
orbittyctl --json stars log save
```

### stars.journal.list / stars.journal.read / stars.journal.append

```json
{"v":1,"id":"1","tool":"stars.journal.list","args":{"from":"2026-09-01","to":"2026-09-24","planet":"orbitty"}}
```

```json
{"v":1,"id":"1","tool":"stars.journal.read","args":{"date":"2026-09-24"}}
```

```json
{"v":1,"id":"1","tool":"stars.journal.append","args":{"text":"a note","planet":"orbitty"}}
```

```
orbittyctl --json stars journal append --planet orbitty -- "a note"
```

`planet` is an id, a unique planet name, or `overall`. Omitted means every page on list, and `overall` on append.

### stars.reflect.templates / stars.reflect.write

```json
{"v":1,"id":"1","tool":"stars.reflect.templates","args":{}}
```

```json
{"v":1,"id":"1","tool":"stars.reflect.write","args":{"template":"ledger","answers":{"spokes":true,"wins":true,"grateful":true,"trackers":true,"cloud":false,"planet":"orbitty"}}}
```

```
orbittyctl --json stars reflect write --planet orbitty --cloud off
```

### stars.tracker.list / inc / dec / set

```json
{"v":1,"id":"1","tool":"stars.tracker.list","args":{"planet":"orbitty"}}
```

```json
{"v":1,"id":"1","tool":"stars.tracker.inc","args":{"id":"tr-…","by":1}}
```

```json
{"v":1,"id":"1","tool":"stars.tracker.set","args":{"id":"tr-…","value":3}}
```

```
orbittyctl --json stars tracker inc --id tr-… --by 1
orbittyctl --json stars tracker set --id tr-… --value 3
```

### stars.timer.start / stop / status

```json
{"v":1,"id":"1","tool":"stars.timer.start","args":{"id":"tr-…"}}
```

```json
{"v":1,"id":"1","tool":"stars.timer.status","args":{}}
```

```
orbittyctl --json stars timer start --id tr-…
orbittyctl --json stars timer status
```

### stars.cloud.words

```json
{"v":1,"id":"1","tool":"stars.cloud.words","args":{"range":"14"}}
```

```
orbittyctl --json stars cloud words --range 14
```

Each word is `{word, count, lastAt, fresh}`. `muted` is the saved mute list. `fresh` is the 14-day window the cover uses to dim a word.

## Hang route

Hang's router is a keyword gate onto doors that already exist. A line that matches the table runs that door. A miss stays chat. It is not a new seat.

The table is closed (at most 15 ids). Today:

`gate-status`, `shift-status`, `companion-note`, `open-planet`, `needed-list`, `agent-list`, `planet-list`, `board-list`.

`route.table` and `route.classify` are read-safe (`read` profile allowed). Classify only names the id, or `null` when the line stays chat (`chat: true`). It does not write state, append an event, or move a window. `route.run` is write-class (`full`, or a custom `allow` entry). It calls the same door Hang uses, including the `companion-hop` log. A miss returns `tool: null` and `spoken: null` with exit 0. The `companion` profile does not reach `route.run` (its prefix is `companion.*`). `route.run` does not push a companion turn.

`companion-note` replies with the note text and saves nothing.

### route.table

```json
{"v":1,"id":"1","tool":"route.table","args":{}}
```

```
orbittyctl --json route table
```

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route.table","arguments":{}}}
```

### route.classify

```json
{"v":1,"id":"1","tool":"route.classify","args":{"line":"list planets"}}
```

```
orbittyctl --json route classify -- "list planets"
```

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route.classify","arguments":{"line":"list planets"}}}
```

A hit is `{"tool":"planet-list","chat":false}`. `tell me a joke` and `stop` are `{"tool":null,"chat":true}`.

### route.run

```json
{"v":1,"id":"1","tool":"route.run","args":{"line":"gate status"}}
```

```
orbittyctl --json route run -- "gate status"
```

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route.run","arguments":{"line":"gate status"}}}
```

`spoken` for `gate status` starts with `Gate is`.

Nightly's socket is `~/Library/Application Support/space.orbitty.mac.nightly/control.sock` when that folder is real. Pass `--sock` (or `ORBITTY_CONTROL_SOCK` for MCP). The default socket is Stable's.

## History

Every save that changes a logged card or planet field appends history lines to `events.jsonl`, with the new values and who made the change. History is local-only and never published. Each line is tagged `t=local-only`, `e-type=history`, `d` (the card or planet id, or the checkpoint name), and `actor`.

| kind | line | content |
| --- | --- | --- |
| 423 | card add / set | `{"v":1,"op":"add","card":{…}}` or `{"v":1,"op":"set","id","fields":{…}}` |
| 424 | card delete (tombstone) | `{"v":1,"id","title"}` |
| 425 | planet add / set / rm | `{"v":1,"op","id","fields":{"name","workspaceRoot"}}` |
| 426 | checkpoint | `{"v":1,"file":"history/checkpoint-….json","sha256","cards","projects"}` |

A `set` line names only the fields that changed. A cleared field is `null`. Only these card fields are logged: `title`, `status`, `projectId`, `kind`, `notes`, `difficulty`, `subtasks`, `dueAt`, `tagIds`, `neededState`, `neededWhy`, `neededAnswer`, `assignedAgentId`, `parentId`, `rating`, `links`, `completedAt`, `starred`, `goalAligned`, `clinicalOrMaster`. Planet fields are `name` and `workspaceRoot`.

`actor` is `user` (the app's windows), `ctl` (this socket), `rule` (a Needed standing-rule answer), `agent` (pickup, doing-fire, verify close, herdr), `shift` (the shift ticker), or `app` (everything else).

`history.list` is read-safe (`read` profile allowed). It returns lines newest first, each `{id, created_at, kind, actor, content}`. `card` keeps one card's lines. `project` keeps lines that name that planet. `limit` defaults to 50, max 500. `history.checkpoint` writes the whole board to `history/checkpoint-<time>.json` next to `state.json` (mode 0600) and appends its 426 line. It returns `{file, sha256, cards, projects}`. It needs `full` or a custom `allow` entry. The app writes one checkpoint at boot when none exists.

### history.list

```json
{"v":1,"id":"1","tool":"history.list","args":{"card":"c-1","limit":12}}
```

```
orbittyctl --json history list --card c-1 --limit 12
```

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"history.list","arguments":{"card":"c-1","limit":12}}}
```

### history.checkpoint

```json
{"v":1,"id":"1","tool":"history.checkpoint","args":{}}
```

```
orbittyctl --json history checkpoint
```

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"history.checkpoint","arguments":{}}}
```

## Everything else

Windows, companion, kanban, projects, claims, actions, agents, shift, relay, and `state.get` are the same socket. Their names and ACL are the control-plane spec. `orbittyctl` with no command prints the command list. `orbitty-mcp` exposes the same names as MCP tools (`tools/list`).
