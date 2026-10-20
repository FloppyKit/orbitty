pub(crate) const EXIT_OTHER: i32 = 1;
pub(crate) const EXIT_NOT_RUNNING: i32 = 2;

// LEGACY (to be replaced by the new companion pack, Matt 2026-09-29): the `companion play` gesture list
// (idle · think · nod · look-left · look-right) inside USAGE below.
pub(crate) const USAGE: &str = "orbittyctl — drive a running Orbitty over its local control socket

usage: orbittyctl [--json] [--token T] [--sock PATH] <command>

commands:
  status                          app health + AX summary
  windows list
  windows focus|minimize|restore|close --id <id>
  companion open
  companion send [--file PATH] -- \"line\"
  companion agents
  companion set-agent -- <profile>
  companion harnesses
  companion set-backend -- <id>     goose | grok | hermes | direct; \"\" clears
  companion route -- \"line\"         dry run: route, backend, source, lanes
  companion play -- <clip>         idle · think · nod · look-left · look-right
  companion mode -- spark|chat|work|hold
  companion free-mode -- on|off
  companion note [--mode spark|chat|work|hold] -- \"line\"
  companion thin                    label this chat thin (mouth keeps going)
  companion now [-- '{\"job\":\"…\"}']   current-job packet (now.json)
  companion hack -- on|off [--who cursor] [--project P]
  kanban open
  kanban add --title \"t\" [--project P] [--status todo|doing] [--notes \"n\"]
            [--link URL [--link-label L]]...
  kanban move --id <id> --status todo|doing|done
  verify close --id <card>          local Verify Bot after a writer; model is the saved default
  kanban notes --id <card> -- \"text\"      empty \"\" clears
  kanban notes --id <card> --append -- \"line\"   adds one line, keeps the rest
  kanban link --id <card> --add URL [--label L] | --rm URL | --clear
  kanban open-link --id <card> [--n 0] [--run]
  kanban assign --id <card> --agent <agent>   empty --agent \"\" unassigns
  agent list
  agent add --name N --harness cursor|goose|grok|hermes|pi [--profile P]
            [--model M] [--permissions read-only|workspace-write|full-auto]
            [--md PATH]... [--extra TEXT] [--project P]
  agent rm --id <id>
  agent pin --id <id> --project P
  agent pickup --id <id> | --name <name> [--card <card>]
  shift status
  shift tick
  shift on|off
  gate status
  gate inject CHATTER|STOP
  route table
  route classify -- \"line\"
  route run [--pending ID] -- \"line\"
  relay status
  relay replica --url ws://100.x:13120   empty --url \"\" = local only
  history list [--card ID] [--project ID] [--limit N]   newest first, default 50, max 500
  history checkpoint                  write a full-board checkpoint (426)
  history drift                       where the board differs from its history
  history rewind --to ID [--card C] [--remove-added] [--confirm]   back to just after line ID
  history repair [--confirm]          put the board back to its history
  history keep [--confirm]            accept the board as it is (checkpoint, reason keep)
  project add --name N
  claim front                         claim frontmost non-Orbitty window
  claim unclaim --id <windowId>        drop a claim (Layer-0 cleanup)
  claim undo                          undo last claim (~3s window)
  claim to --project <id>             claim front window to a planet
  claim change --project <id>         retarget pending claim
  project rm --id <id>
  project rename --id <id> --name <new>
  project set-root --id <id> --cwd PATH
  task undo --id <card>
  action list
  action add --name N --cmd \"C\" [--cwd D] [--combo cmd+alt+m]
  action run --id <id> | --name <name>
  action rm --id <id>
  needed open --title \"t\" [--project P] [--why \"w\"]
  needed answer --id <id> -- \"text\"
  log append --type T [--project P] [--actor agent] -- '{\"k\":1}'
  stars open
  stars checkin get [--date YYYY-MM-DD]
  stars checkin set --axis sleep|terroir|athleticism|relationships|self --value 1-5 [--date D]
  stars log get [--date D]
  stars log set [--date D] [--win1 T] [--win2 T] [--grateful T]
  stars log save [--date D]
  stars journal list [--from D] [--to D] [--planet P]
  stars journal read --date D
  stars journal append [--date D] [--planet P] -- \"text\"
  stars reflect templates
  stars reflect write [--date D] [--template ledger] [--planet P]
            [--spokes on|off] [--wins on|off] [--grateful on|off]
            [--trackers on|off] [--cloud on|off] [--answers '{...}']
  stars tracker list [--planet P]
  stars tracker inc|dec --id ID [--by N]
  stars tracker set --id ID --value N
  stars timer start|stop --id ID
  stars timer status
  stars cloud words [--range 14|week|month|year|all]
  packs list
  sleeper wake --id <id>
  state get
  settings plans-dir [--set PATH]
  settings runner [--name N --harness H --model M]
  go status --file PATH | --go GO-ID [--card ID]
  go start --card ID
  go run-status --id RUN
  go run-result --id RUN
  go runner [--set ID | --reset]
  rail state                          level, alpha, frame, appActive, occludedBy (read-only)
  go seal --file PATH --by NAME --confirm
  plans migrate [--dry-run | --apply]

flags:
  --json        machine JSON on stdout (data, or the full error object)
  --token T     auth token (else $ORBITTY_CONTROL_TOKEN, else control.token)
  --sock PATH   socket override (else $ORBITTY_CONTROL_SOCK, else
                ~/Library/Application Support/space.orbitty.mac/control.sock)

The four write verbs (kanban add/move, needed open,
log append) mutate through the app's one save door — the UI live-updates.
ACL: control.json profile full, or a named custom allowlist entry; read /
companion are denied (exit 4).

kanban add takes --notes (the board row's one-line
summary; first line paints, the drawer edits the rest) and kanban notes
sets/clears it on an existing card — same save door, same write ACL.
kanban link and kanban open-link use that same ACL. --link is repeatable
(max 8). Only file:// and https:// are accepted; a bare /Users/… path
is stored as a file URL. open-link opens documents, folders, images, and
https. An executable, script, or bundle is revealed in Finder unless
--run is passed or that exact file is already in the approvals list.
open-link reads that list and never writes it. An approval is recorded
only when you click Open or Run in the Orbitty preview.

project add / rm / rename mint, kill, and rename
planets through the same save door (habitat paints whatever projects[]
holds). Empty --name refused; duplicate display names allowed (ids are
unique). project rm keeps cards' projectId and live claims — walkers
vanish with the disc on the next habitat sync. Same ACL as kanban.add.

action list is read-safe; the companion profile
may action add / action run (Spark mints named recipes — never a raw
shell); action rm needs full or a custom entry naming it (or \"action.*\").
Combos register globally on persist; reserved (cmd+shift+o, cmd+space) or
taken chords show [conflict] in action list.

agent add mints a row (--harness required —
cursor|goose|grok|hermes|pi, never a silent default). A cursor row with no
--model stores grok-4.7-high. --permissions defaults to read-only;
full-auto is the only preset that passes the CLI force/trust flags.
agent pin sets its
planet, kanban assign writes the card's assignedAgentId (empty --agent
unassigns), and agent pickup moves the oldest assigned todo to doing and
launches the row's harness with that title (every harness lights the dock
LED while the run is live). agent list is read-safe; the rest are writes
(full / named custom).

shift status is read-safe. shift tick parks ghost working
rows (they eat the runner cap), Needed-escalates blocked, and pickups
the next assigned todo while Work/Hold is on. shift on starts a 15s
ticker; off stops it. Spark/Chat never auto-pickup.

gate status is read-safe. Hang is companion focus: click the
companion and status reports armed; click anywhere else and it reports
disarmed, and a freeze clears. gate inject CHATTER holds (no freeze).
gate inject STOP aborts, freezes shift pickup, and stops live working
runs — write-class ACL.

route table and route classify are read-safe. classify only names
which existing door a Hang line would hit, or that the line stays chat.
It writes nothing. route run calls that same door (including the
companion-hop log). board-add and agent-spawn return pendingId and do
not write until a later `route run --pending ID -- yes` (or ok / do
it) with the pendingId that proposal returned. A yes from another
surface or without the id confirms nothing. no (with the id) or a
60s timeout drops the pending write. A miss returns tool null and
spoken null, exit 0. route run is write-class (full / named custom).
The companion profile does not reach it.

relay status is read-safe. relay replica --url sets the
optional one-user VPS URL (Tailscale 100.64/10:13120 or
ws://127.0.0.1:port for SSH -L). Empty URL = local sidecar only.
Never 0.0.0.0. Write-class ACL (full / named custom).

history list reads the board's local-only history from events.jsonl:
423 card add/set, 424 card delete, 425 planet, 426 checkpoint. Each line
carries the new values and the actor (user, ctl, rule, agent, shift, app,
rewind, repair). --card and --project filter; read-safe. history
checkpoint writes the whole board to history/checkpoint-*.json and logs a
426 line; it needs full or a custom entry. History is never published.

history drift is read-safe. history rewind, repair and keep need full or
a custom entry, and without --confirm they print the plan and write
nothing. rewind --card restores one card (a deleted card comes back
whole); without --card the whole board goes back and cards added since
stay unless --remove-added. Live work (an agent run, a running task, a
claimed window) is skipped. Each rewind or repair first writes a
checkpoint, so the Settings → History \"Undo last rewind\" can undo it.

stars checkin / log / journal / reflect / tracker / timer / cloud
read and write the STARS page through the same state.json save door as
the GUI (`state:changed` reloads the page). Reads are on the read
profile. Writes need full or a custom allow entry (`stars.*` covers
them). Omitted --date is today. log set leaves out a field to keep it;
`--win1 \"\"` clears that line. log save trims the saved day and does not
invent an empty one. journal append refuses a blank body. reflect write
uses the local ledger template (no model call).

needed answer --id n-… -- \"text\" applies an
answer through the same needed-answer event path as the composer (the
watch dispatches on it) — scripts never need the UI. needed open itself
now consults the user-minted standing rules first: a can-run hit is born
answered (actor \"rule\"); a miss/collide still pings.

exit codes: 0 ok · 2 Orbitty not running · 3 auth · 4 denied · 5 AX · 1 other";

