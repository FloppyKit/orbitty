//! orbitty-mcp — stdio adapter to the local control socket. No TCP.

#[cfg(unix)]
mod imp {
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

/// Client name from the first `initialize`. Each MCP client spawns its
/// own orbitty-mcp process, so the pid separates two sessions.
static MCP_CLIENT_NAME: OnceLock<String> = OnceLock::new();

const PROTOCOL: &str = "2024-11-05";
const SERVER_NAME: &str = "orbitty";
const SERVER_VERSION: &str = "0.1.0";

struct ToolDef {
    name: &'static str,
    description: &'static str,
    /// "none" | "id" | "line" | "route_line" | "route_run" | "profile" | "backend_id" | "clip" | "kanban_add" |
    /// "kanban_move" | "kanban_notes" | "kanban_link" | "kanban_open_link" |
    /// "kanban_assign" | "needed_open" |
    /// "log_append" | "action_add" | "action_run" | "action_rm" |
    /// "project_add" | "project_rm" | "project_rename" | "agent_add" |
    /// "agent_pin" | "agent_pickup" | "claim_project" | "claim_window"
    args: &'static str,
}

const TOOLS: &[ToolDef] = &[
    ToolDef { name: "app.status", description: "Orbitty up + Accessibility summary", args: "none" },
    ToolDef { name: "app.accessibility_status", description: "Accessibility grant status", args: "none" },
    ToolDef { name: "windows.list", description: "List windows the rail sees", args: "none" },
    ToolDef { name: "windows.focus", description: "Focus a window by id", args: "id" },
    ToolDef { name: "windows.minimize", description: "Minimize a window by id", args: "id" },
    ToolDef { name: "windows.restore", description: "Restore a window by id", args: "id" },
    ToolDef { name: "windows.close", description: "Close a window by id", args: "id" },
    ToolDef { name: "companion.open", description: "Show the companion window", args: "none" },
    ToolDef { name: "kanban.open", description: "Show the shared pane in Board mode", args: "none" },
    ToolDef { name: "stars.open", description: "Show the shared pane in STARS mode", args: "none" },
    ToolDef { name: "stars.checkin.get", description: "Read one day's saved STARS check-in (five spokes 0 or 1-5, plus win lines). Omitted date = today.", args: "stars_date" },
    ToolDef { name: "stars.checkin.set", description: "Set one spoke (sleep|terroir|athleticism|relationships|self) to 1-5 on a saved day and reload the page.", args: "stars_checkin_set" },
    ToolDef { name: "stars.log.get", description: "Read Win 1, Win 2, and Grateful for a saved day. Omitted date = today.", args: "stars_date" },
    ToolDef { name: "stars.log.set", description: "Set Win 1, Win 2, and/or Grateful on a day. Omit a field to keep it; empty string clears it.", args: "stars_log_set" },
    ToolDef { name: "stars.log.save", description: "Trim and re-save a day's check-in (the Save today door). Does not invent an empty day.", args: "stars_date" },
    ToolDef { name: "stars.journal.list", description: "List journal pages, optionally filtered by from/to dates and planet.", args: "stars_journal_list" },
    ToolDef { name: "stars.journal.read", description: "Read journal pages for one date.", args: "stars_date_req" },
    ToolDef { name: "stars.journal.append", description: "Append a journal page. Blank text is not saved.", args: "stars_journal_append" },
    ToolDef { name: "stars.reflect.templates", description: "List Reflect templates. The ledger template is local — no model call.", args: "none" },
    ToolDef { name: "stars.reflect.write", description: "Write a journal page from a template and that day's saved check-in. Blank body is not saved.", args: "stars_reflect_write" },
    ToolDef { name: "stars.tracker.list", description: "List planet trackers (numbers and timers).", args: "stars_planet" },
    ToolDef { name: "stars.tracker.inc", description: "Increase a number tracker. by defaults to 1. Timers use stars.timer.*.", args: "stars_tracker_bump" },
    ToolDef { name: "stars.tracker.dec", description: "Decrease a number tracker. by defaults to 1.", args: "stars_tracker_bump" },
    ToolDef { name: "stars.tracker.set", description: "Set a number tracker to an integer value.", args: "stars_tracker_set" },
    ToolDef { name: "stars.timer.start", description: "Start a timer tracker. A second start leaves the original start time.", args: "stars_id" },
    ToolDef { name: "stars.timer.stop", description: "Stop a timer and fold elapsed seconds into its value.", args: "stars_id" },
    ToolDef { name: "stars.timer.status", description: "List timer trackers with running state and elapsed seconds.", args: "none" },
    ToolDef { name: "stars.cloud.words", description: "Read the Cloud cover word bag (done cards, tags, planet names, your lines). Not stored. Bot replies are excluded.", args: "stars_cloud" },
    ToolDef { name: "companion.send", description: "Send a line on the companion Berd path", args: "line" },
    ToolDef { name: "companion.archive_tail", description: "Last n lines of the companion archive (default 20). Optional date is a CT day YYYY-MM-DD. Read-only.", args: "archive_tail" },
    ToolDef { name: "companion.resets", description: "Last n backend reset lines (default 20). Read-only.", args: "archive_n" },
    ToolDef { name: "companion.roll", description: "Arm a manual_test backend reset for the next Grok send. No model call. Any other reason is refused.", args: "archive_roll" },
    ToolDef { name: "companion.recall", description: "Search the companion archive, active planet notes, and tabs. Returns at most 8 dated hits. Read-only. Flag memory_ram off returns enabled false.", args: "recall" },
    ToolDef { name: "companion.tags_propose", description: "Queue up to 5 tag names and open one Approve needed. Writes no tag file until companion.tags_accept.", args: "tags_propose" },
    ToolDef { name: "companion.tags_accept", description: "Write the queued tag batch into the planet or global tag file. Refuses when nothing is queued.", args: "tags_accept" },
    ToolDef { name: "companion.agents", description: "List companion agent profiles", args: "none" },
    ToolDef { name: "companion.set_agent", description: "Set the companion agent profile", args: "profile" },
    ToolDef { name: "companion.harnesses", description: "List companion harnesses detected on this Mac, plus the current pick", args: "none" },
    ToolDef { name: "companion.set_backend", description: "Set the companion harness (goose, grok, hermes, or direct). Empty id clears. Same ACL class as companion.set_agent.", args: "backend_id" },
    ToolDef { name: "companion.route", description: "Dry-run the companion send route for a line. Returns route, backend, source, and lanes. Does not call a model or write. Keys are not set here.", args: "line" },
    ToolDef { name: "companion.play", description: "Play an allowlisted gesture clip on the 3D companion body (no-op on the spirit sheet)", args: "clip" },
    ToolDef { name: "packs.list", description: "List installed sprite packs", args: "none" },
    ToolDef { name: "sleeper.wake", description: "Wake a sleeper by id", args: "id" },
    ToolDef { name: "state.get", description: "Read persisted Orbitty state", args: "none" },
    // The four write tools. control.json ACL still
    // applies — read / companion profiles get denied (4).
    ToolDef { name: "kanban.add", description: "Mint a kanban todo as actor agent (status todo|doing; done is earned via kanban.move); optional notes = the board row's one-line summary; optional links = file:// or https:// (max 8)", args: "kanban_add" },
    ToolDef { name: "kanban.move", description: "Move a card todo|doing|done; done is the checkbox complete door (points + daily cap), no unclaim", args: "kanban_move" },
    // Set/clear the summary line. Same write ACL.
    ToolDef { name: "kanban.notes", description: "Set a card's notes (first line paints as the board-row summary; the drawer edits the rest); empty string clears", args: "kanban_notes" },
    ToolDef { name: "kanban.note_append", description: "Append one line to a card's notes, keeping the existing notes (a repeat of the same line is a no-op). Same write ACL as kanban.notes.", args: "kanban_note_append" },
    ToolDef { name: "kanban.link", description: "Add, remove, or clear a card link (file:// or https:// only, max 8). Same write ACL as kanban.notes. Does not change notes or status.", args: "kanban_link" },
    ToolDef { name: "kanban.open_link", description: "Open a card link. Documents, folders, images, and https open. An executable, script, or bundle is revealed in Finder unless run is true or that exact file was already approved in the app. Reads link-approvals.json and never writes it. n defaults to 0. Does not change card status.", args: "kanban_open_link" },
    ToolDef { name: "needed.open", description: "Open a Needed inbox card", args: "needed_open" },
    ToolDef { name: "log.append", description: "Append a local-only event to the Orbitty event log (never fails the caller)", args: "log_append" },
    // The planet control plane, next to the kanban
    // writes. Same ACL (full / named custom only). rm keeps cards'
    // projectId and live claims; add dedups same-name rows (newest id).
    ToolDef { name: "project.add", description: "Mint a project planet, or reuse + front the existing one on a same-name add (trim, case-insensitive; empty name refused)", args: "project_add" },
    ToolDef { name: "project.rm", description: "Remove a project planet by id; cards keep their projectId and live claims stay (walkers vanish on next habitat sync)", args: "project_rm" },
    ToolDef { name: "project.rename", description: "Rename a project planet, keeping its id (empty name refused)", args: "project_rename" },
    // Claim verbs. Same names and arg keys control.rs reads
    // (projectId, windowId). Forward-only; claim.* stays write-class.
    ToolDef { name: "claim.front", description: "Claim the frontmost non-Orbitty window to the last-used planet (undo within ~3s with claim.undo)", args: "none" },
    ToolDef { name: "claim.to", description: "Claim the frontmost non-Orbitty window to a planet by project id", args: "claim_project" },
    ToolDef { name: "claim.change", description: "Retarget your own pending claim (~3 s window) to another planet", args: "claim_project" },
    ToolDef { name: "claim.undo", description: "Undo your own pending claim (~3 s): reverses only that window's claim, never other changes; tray/app claims can't be undone over MCP", args: "none" },
    ToolDef { name: "claim.unclaim", description: "Drop a claim by window id (Layer-0 cleanup)", args: "claim_window" },
    // The action recipe file. list is read-safe;
    // add / run ride the companion profile (Spark mints + runs named
    // recipes, never a raw shell); rm needs full / named custom ("action.*").
    ToolDef { name: "action.list", description: "List action shortcuts (id, name, combo, command, conflict status)", args: "none" },
    ToolDef { name: "action.add", description: "Mint an action shortcut: a named /bin/zsh -lc recipe with an optional global combo like cmd+alt+m", args: "action_add" },
    ToolDef { name: "action.run", description: "Run an action by id or name (the logged 120s /bin/zsh -lc door; output clipped)", args: "action_run" },
    ToolDef { name: "action.rm", description: "Remove an action by id (full / named custom ACL only)", args: "action_rm" },
    // The agent roster + assignment. agent.list
    // is read-safe; add / rm / pin / pickup / kanban.assign are writes
    // (full / named custom only). The harness is per row, required on
    // add — unknown harness errors honestly, never a silent hermes.
    ToolDef { name: "agent.list", description: "List agent rows (id, name, harness, profile, model, md paths, extra, pinned planet)", args: "none" },
    ToolDef { name: "agent.add", description: "Mint an agent row: name + harness (cursor|goose|grok|hermes|pi, required) with optional profile / model / permissions (read-only|workspace-write|full-auto) / mdPaths / extra / projectId pin. New cursor rows default model grok-4.7-high and permissions read-only.", args: "agent_add" },
    ToolDef { name: "agent.rm", description: "Remove an agent row by id; cards keep their assignedAgentId (project.rm law)", args: "id" },
    ToolDef { name: "agent.pin", description: "Pin an agent row to a planet (project id or unique planet name)", args: "agent_pin" },
    ToolDef { name: "agent.pickup", description: "Pick the oldest assigned open Todo (the row's planet when pinned), move it doing, and launch the row's harness with the card title; no match = honest empty, no spawn", args: "agent_pickup" },
    ToolDef { name: "settings.plans_dir", description: "Read the shared plansDir (visible ~/Orbitty GOs unless set) and the default runner", args: "none" },
    ToolDef { name: "settings.plans_dir_set", description: "Set the shared plansDir used by Nightly and stable", args: "plans_dir_set" },
    ToolDef { name: "settings.runner", description: "Read defaultRunner (Cursor harness, model grok-4.7-high, herdr fallback)", args: "none" },
    ToolDef { name: "settings.runner_set", description: "Set defaultRunner runner, harness, and model", args: "runner_set" },
    ToolDef { name: "go.status", description: "Resolve a GO and report whether it is sealed", args: "go_status" },
    ToolDef { name: "go.start", description: "Start the kitchen on a sealed todo card. Returns a run id without waiting. Refuses with one card note and no spawn when the GO, base, lock, rig, disk, or runner is not ready.", args: "go_start" },
    ToolDef { name: "go.run_status", description: "Read a Go run's state.json (stage, tip, permissions, runner).", args: "go_run_id" },
    ToolDef { name: "go.run_result", description: "Read a Go run's result line, stage, and tip.", args: "go_run_id" },
    ToolDef { name: "go.runner_get", description: "Read the selected Go runner, the approved list, and the permission flags. Does not write them.", args: "none" },
    ToolDef { name: "go.runner_set", description: "Pick the Go runner id from the approved list, or reset to the default. Refuses any other key.", args: "go_runner_set" },
    ToolDef { name: "go.seal", description: "Seal a GO file (profile full and confirm). Writes status, sealed_at, sealed_by, and seal_hash.", args: "go_seal" },
    ToolDef { name: "plans.migrate", description: "Dry-run: list files that would copy from the back-compat plans folder into plansDir. Never deletes.", args: "none" },
    ToolDef { name: "plans.migrate_apply", description: "Copy those files into plansDir. Never deletes. Not part of a normal GO run.", args: "none" },
    ToolDef { name: "shift.status", description: "Humming supervisor snapshot: on/off, mode, runner cap, agentRuns, next assigned pickups", args: "none" },
    ToolDef { name: "route.table", description: "Hang route table: the closed set of keyword tool ids (max 15). Read-only.", args: "none" },
    ToolDef { name: "route.classify", description: "Classify a Hang line to a route id, or null if it stays chat. Read-only, no side effects.", args: "route_line" },
    ToolDef { name: "route.run", description: "Run a Hang line through the route table (same door as Hang). board-add and agent-spawn return pendingId and write only on a later yes that carries that pendingId. A miss returns null spoken. Write-class.", args: "route_run" },
    ToolDef { name: "shift.tick", description: "One supervisor pass: park ghost working rows, Needed-escalate blocked, pickup next assigned todo if Work/Hold", args: "none" },
    ToolDef { name: "shift.on", description: "Turn the 15s shift ticker on (Work/Hold pickups; Spark/Chat never auto-pickup)", args: "none" },
    ToolDef { name: "shift.off", description: "Turn the 15s shift ticker off", args: "none" },
    ToolDef { name: "kanban.assign", description: "Assign a card to an agent row (agent id or unique name); empty agent unassigns", args: "kanban_assign" },
    // Local-only board history (kinds 423-426), never published.
    // list is read-safe; checkpoint writes a file, so full / named custom.
    ToolDef { name: "history.list", description: "Board history lines, newest first: card add/set (423), card delete (424), planet (425), checkpoint (426). Each has id, created_at, kind, actor, content. Local-only, never published.", args: "history_list" },
    ToolDef { name: "events.since", description: "Lines in events.jsonl with seq greater than after, in file order. Flag event_seq off answers enabled false. Seq stays on this machine.", args: "events_since" },
    ToolDef { name: "history.checkpoint", description: "Write a full-board checkpoint file and its 426 line. Returns file, sha256, cards, projects.", args: "none" },
    // drift is read-safe; rewind / repair / keep are full / named custom
    // and only write with confirm: true.
    ToolDef { name: "history.drift", description: "Where the board on disk differs from its history: cards and planets whose fields differ (history and now values), missing on disk, or never in the history. Read-only.", args: "none" },
    ToolDef { name: "history.rewind", description: "Rewind the board (or one card with card) to just after history line to. Cards added since stay unless removeAdded. Live work is skipped. Without confirm: true it returns the plan and writes nothing. Undo in Settings → History.", args: "history_rewind" },
    ToolDef { name: "history.repair", description: "Put the board back to what its history says (actor repair). Without confirm: true it returns the plan and writes nothing.", args: "history_confirm" },
    ToolDef { name: "history.keep", description: "Accept the board as it is: a checkpoint with reason keep, so drift is empty. Without confirm: true it returns the drift and writes nothing.", args: "history_confirm" },
    ToolDef { name: "flags.list", description: "List runtime flags: id, on, source, stale. Read-only.", args: "none" },
    ToolDef { name: "flags.set", description: "Turn a runtime flag on or off. Write-class. Writes the channel flags file mode 0600.", args: "flags_set" },
];

fn known_tool(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

fn input_schema(kind: &str) -> Value {
    match kind {
        "id" => json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "Row id (window, sleeper, action, agent, …)" } },
            "required": ["id"]
        }),
        "line" => json!({
            "type": "object",
            "properties": { "line": { "type": "string", "description": "Text to send" } },
            "required": ["line"]
        }),
        "route_line" => json!({
            "type": "object",
            "properties": { "line": { "type": "string", "description": "Hang line" } },
            "required": ["line"]
        }),
        "route_run" => json!({
            "type": "object",
            "properties": {
                "line": { "type": "string", "description": "Hang line" },
                "pendingId": { "type": "string", "description": "Confirm / decline only: the pendingId this caller's board-add or agent-spawn returned" }
            },
            "required": ["line"]
        }),
        "profile" => json!({
            "type": "object",
            "properties": { "profile": { "type": "string", "description": "Agent profile id" } },
            "required": ["profile"]
        }),
        "backend_id" => json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "Harness id: goose, grok, hermes, or direct. Empty string clears." } },
            "required": ["id"]
        }),
        "clip" => json!({
            "type": "object",
            "properties": { "clip": {
                "type": "string",
                // LEGACY (to be replaced by the new companion pack, Matt 2026-09-29)
                "enum": ["idle", "think", "nod", "look-left", "look-right"],
                "description": "Gesture clip name (allowlist; unknown names error)"
            } },
            "required": ["clip"]
        }),
        "kanban_add" => json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Card title" },
                "projectId": { "type": "string", "description": "Project id or unique planet name (e.g. fkt, orbitty, practice_desk); omitted/empty = inbox" },
                "status": {
                    "type": "string",
                    "enum": ["todo", "doing"],
                    "description": "Initial column (default todo; done is never minted)"
                },
                "notes": { "type": "string", "description": "One-line runner summary (first line paints on the board row); omitted/empty = none" },
                "links": {
                    "type": "array",
                    "maxItems": 8,
                    "description": "Optional file:// or https:// links. A bare /Users/… path is stored as a file URL.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" },
                            "label": { "type": "string" }
                        },
                        "required": ["url"]
                    }
                }
            },
            "required": ["title"]
        }),
        "kanban_link" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "add": { "type": "string", "description": "URL to add (file://, https://, or a bare /Users/… path). Pass exactly one of add, rm, clear." },
                "label": { "type": "string", "description": "Optional label for --add" },
                "rm": { "type": "string", "description": "URL to remove" },
                "clear": { "type": "boolean", "description": "Remove every link on the card" }
            },
            "required": ["id"]
        }),
        "kanban_open_link" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "n": { "type": "integer", "minimum": 0, "description": "Link index. Default 0." },
                "run": { "type": "boolean", "description": "Run an executable. Without this, an unapproved executable is revealed in Finder. Does not record an approval." }
            },
            "required": ["id"]
        }),
        // Empty string is meaningful: clears.
        "kanban_notes" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "notes": { "type": "string", "description": "Notes text (first line is the board-row summary); empty string clears" }
            },
            "required": ["id", "notes"]
        }),
        "kanban_note_append" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "line": { "type": "string", "description": "One line to append to the notes" }
            },
            "required": ["id", "line"]
        }),
        "kanban_move" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "status": {
                    "type": "string",
                    "enum": ["todo", "doing", "done"],
                    "description": "Target column; done = complete (points + cap), no unclaim"
                }
            },
            "required": ["id", "status"]
        }),
        "needed_open" => json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "What is needed" },
                "projectId": { "type": "string", "description": "Project id; omitted/empty = inbox" },
                "why": { "type": "string", "description": "Why it is needed" }
            },
            "required": ["title"]
        }),
        "log_append" => json!({
            "type": "object",
            "properties": {
                "eType": { "type": "string", "description": "Event type tag" },
                "content": { "type": "object", "description": "Small JSON object payload" },
                "projectId": { "type": "string" },
                "actor": { "type": "string", "description": "Event actor (default agent)" }
            },
            "required": ["eType"]
        }),
        "action_add" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Action name (the list's run word)" },
                "cmd": { "type": "string", "description": "Shell recipe, run via /bin/zsh -lc (120s cap)" },
                "cwd": { "type": "string", "description": "Working directory; omitted/empty = $HOME" },
                "combo": { "type": "string", "description": "Global chord like cmd+alt+m; omitted = Spirit-only. Reserved (cmd+shift+o, cmd+space) or taken chords register nothing and show conflict" }
            },
            "required": ["name", "cmd"]
        }),
        "action_run" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Action id" },
                "name": { "type": "string", "description": "Action name (exact; unique case-insensitive fallback)" }
            }
        }),
        "action_rm" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Action id" }
            },
            "required": ["id"]
        }),
        "project_add" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Planet name (empty refused; duplicates allowed)" }
            },
            "required": ["name"]
        }),
        "project_rm" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Project id (unknown id errors)" }
            },
            "required": ["id"]
        }),
        "project_rename" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Project id" },
                "name": { "type": "string", "description": "New planet name (empty refused)" }
            },
            "required": ["id", "name"]
        }),
        // Agent roster schemas.
        "agent_add" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Agent name (empty refused)" },
                "harness": {
                    "type": "string",
                    "enum": ["cursor", "goose", "grok", "hermes", "pi"],
                    "description": "Launch door on the row — required, never a product default"
                },
                "profile": { "type": "string", "description": "Harness profile (session tweak on the row)" },
                "model": { "type": "string", "description": "Model override. Omitted on a cursor row stores grok-4.7-high." },
                "permissions": { "type": "string", "enum": ["read-only", "workspace-write", "full-auto"], "description": "Cursor CLI preset. Omitted = read-only. full-auto is the only value that passes --force and --trust." },
                "mdPaths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Readable .md files concatenated into the pickup query"
                },
                "extra": { "type": "string", "description": "Free-text extra instructions, concatenated into the pickup query after the md paths" },
                "projectId": { "type": "string", "description": "Planet pin: project id or unique planet name (e.g. orbitty); omitted = unpinned" }
            },
            "required": ["name", "harness"]
        }),
        "agent_pin" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Agent id (unknown id errors)" },
                "projectId": { "type": "string", "description": "Project id or unique planet name" }
            },
            "required": ["id", "projectId"]
        }),
        "plans_dir_set" => json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "plansDir path" }
            },
            "required": ["path"]
        }),
        "runner_set" => json!({
            "type": "object",
            "properties": {
                "runner": { "type": "string", "description": "Agent row name, e.g. orbitty-cursor" },
                "harness": { "type": "string", "description": "cursor | goose | grok | herdr | manual" },
                "model": { "type": "string", "description": "Model id, e.g. grok-4.7-high" }
            }
        }),
        "go_status" => json!({
            "type": "object",
            "properties": {
                "file": { "type": "string", "description": "GO file path" },
                "go": { "type": "string", "description": "GO id token" },
                "card": { "type": "string", "description": "Card id for header card_id match" },
                "title": { "type": "string", "description": "Card title, if it differs from the GO id" }
            }
        }),
        "go_seal" => json!({
            "type": "object",
            "properties": {
                "file": { "type": "string", "description": "GO file to seal" },
                "by": { "type": "string", "description": "Who is sealing (e.g. matt)" },
                "confirm": { "type": "boolean", "description": "Must be true" }
            },
            "required": ["file", "by", "confirm"]
        }),
        "go_start" => json!({
            "type": "object",
            "properties": {
                "card": { "type": "string", "description": "Todo card id whose sealed GO should run" }
            },
            "required": ["card"],
            "additionalProperties": false
        }),
        "go_run_id" => json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string", "description": "Run id from go.start" }
            },
            "required": ["run_id"],
            "additionalProperties": false
        }),
        "go_runner_set" => json!({
            "type": "object",
            "properties": {
                "runner": { "type": "string", "description": "Approved runner id" },
                "reset": { "type": "boolean", "description": "Clear the selection. Default runner is pi-grok." }
            },
            "additionalProperties": false
        }),
        "agent_pickup" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Agent id" },
                "name": { "type": "string", "description": "Agent name (exact; unique case-insensitive fallback)" }
            }
        }),
        "kanban_assign" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Card id" },
                "agent": { "type": "string", "description": "Agent id or unique name; empty string unassigns" }
            },
            "required": ["id", "agent"]
        }),
        "stars_date" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD. Omitted = today." }
            }
        }),
        "stars_date_req" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD" }
            },
            "required": ["date"]
        }),
        "stars_checkin_set" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD. Omitted = today." },
                "axis": {
                    "type": "string",
                    "enum": ["sleep", "terroir", "athleticism", "relationships", "self"],
                    "description": "Spoke to set"
                },
                "value": { "type": "integer", "minimum": 1, "maximum": 5, "description": "Score 1-5" }
            },
            "required": ["axis", "value"]
        }),
        "stars_log_set" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD. Omitted = today." },
                "win1": { "type": "string", "description": "Win 1. Empty string clears. Omit to keep." },
                "win2": { "type": "string", "description": "Win 2. Empty string clears. Omit to keep." },
                "grateful": { "type": "string", "description": "Grateful. Empty string clears. Omit to keep." }
            }
        }),
        "stars_journal_list" => json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "description": "First day YYYY-MM-DD, inclusive" },
                "to": { "type": "string", "description": "Last day YYYY-MM-DD, inclusive" },
                "planet": { "type": "string", "description": "Planet id, unique name, or overall" }
            }
        }),
        "stars_journal_append" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD. Omitted = today." },
                "text": { "type": "string", "description": "Page body. Blank is not saved." },
                "planet": { "type": "string", "description": "Planet id or unique name. Omitted = overall." }
            },
            "required": ["text"]
        }),
        "stars_reflect_write" => json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "Local day YYYY-MM-DD. Omitted = today. Uses the saved check-in." },
                "template": { "type": "string", "enum": ["ledger"], "description": "Template id from stars.reflect.templates" },
                "planet": { "type": "string", "description": "Scope: planet id, unique name, or overall" },
                "answers": {
                    "type": "object",
                    "description": "Section switches. A missing key is on. planet may also live here.",
                    "properties": {
                        "spokes": { "type": "boolean" },
                        "wins": { "type": "boolean" },
                        "grateful": { "type": "boolean" },
                        "trackers": { "type": "boolean" },
                        "cloud": { "type": "boolean" },
                        "planet": { "type": "string" }
                    }
                }
            },
            "required": ["template"]
        }),
        "stars_planet" => json!({
            "type": "object",
            "properties": {
                "planet": { "type": "string", "description": "Planet id, unique name, or overall. Omitted = all." }
            }
        }),
        "stars_tracker_bump" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Tracker id" },
                "by": { "type": "integer", "minimum": 1, "description": "Step size. Default 1." }
            },
            "required": ["id"]
        }),
        "stars_tracker_set" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Tracker id" },
                "value": { "type": "integer", "description": "New value" }
            },
            "required": ["id", "value"]
        }),
        "stars_id" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Timer tracker id" }
            },
            "required": ["id"]
        }),
        "stars_cloud" => json!({
            "type": "object",
            "properties": {
                "range": {
                    "type": "string",
                    "enum": ["14", "week", "month", "year", "all"],
                    "description": "How far back done cards count. Omitted = all. User lines always count as now."
                }
            }
        }),
        "claim_project" => json!({
            "type": "object",
            "properties": {
                "projectId": { "type": "string", "description": "Planet id (same key control.rs reads for claim.to and claim.change)" }
            },
            "required": ["projectId"]
        }),
        "claim_window" => json!({
            "type": "object",
            "properties": {
                "windowId": { "type": "string", "description": "Window id (same key control.rs reads for claim.unclaim)" }
            },
            "required": ["windowId"]
        }),
        "history_list" => json!({
            "type": "object",
            "properties": {
                "card": { "type": "string", "description": "Only lines for this card id" },
                "project": { "type": "string", "description": "Only lines that name this planet id" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 500, "description": "Max lines. Default 50." }
            }
        }),
        "events_since" => json!({
            "type": "object",
            "properties": {
                "after": { "type": "integer", "minimum": 0, "description": "Return lines with seq greater than this" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 500, "description": "Max lines. Default 500." }
            },
            "required": ["after"]
        }),
        "history_rewind" => json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "History line id (from history.list)" },
                "card": { "type": "string", "description": "Only this card id" },
                "removeAdded": { "type": "boolean", "description": "Also remove cards added since. Default false." },
                "confirm": { "type": "boolean", "description": "true writes. Default false: plan only." }
            },
            "required": ["to"]
        }),
        "history_confirm" => json!({
            "type": "object",
            "properties": {
                "confirm": { "type": "boolean", "description": "true writes. Default false: dry run." }
            }
        }),
        "archive_tail" => json!({
            "type": "object",
            "properties": {
                "n": { "type": "integer", "minimum": 1, "description": "How many lines. Default 20." },
                "date": { "type": "string", "description": "CT day YYYY-MM-DD. Omitted = every archive file, newest last." }
            }
        }),
        "archive_n" => json!({
            "type": "object",
            "properties": {
                "n": { "type": "integer", "minimum": 1, "description": "How many lines. Default 20." }
            }
        }),
        "archive_roll" => json!({
            "type": "object",
            "properties": {
                "reason": { "type": "string", "enum": ["manual_test"], "description": "Only manual_test is accepted." }
            },
            "required": ["reason"]
        }),
        "recall" => json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Substring or tag to find" },
                "planet": { "type": "string", "description": "Planet id. Omitted searches every planet folder." }
            },
            "required": ["query"]
        }),
        "tags_propose" => json!({
            "type": "object",
            "properties": {
                "names": { "type": "string", "description": "Comma-separated tag names, at most 5 kept" },
                "planet": { "type": "string", "description": "Planet id. Omitted queues a global batch." }
            },
            "required": ["names"]
        }),
        "tags_accept" => json!({
            "type": "object",
            "properties": {
                "planet": { "type": "string", "description": "Planet id of the queued batch. Omitted accepts a global batch." }
            }
        }),
        "flags_set" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Flag id from flags.list" },
                "on": { "type": "boolean", "description": "true on, false off" }
            },
            "required": ["id", "on"],
            "additionalProperties": false
        }),
        _ => json!({ "type": "object", "properties": {} }),
    }
}

fn tools_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": input_schema(t.args),
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn default_sock() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac/control.sock")
}

fn sock_path() -> PathBuf {
    if let Ok(p) = std::env::var("ORBITTY_CONTROL_SOCK") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    default_sock()
}

fn token(sock: &std::path::Path) -> Option<String> {
    if let Ok(t) = std::env::var("ORBITTY_CONTROL_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    sock.parent().and_then(|dir| {
        std::fs::read_to_string(dir.join("control.token"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn client_name_from(msg: &Value) -> String {
    msg.pointer("/params/clientInfo/name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn remember_client_name(msg: &Value) {
    let _ = MCP_CLIENT_NAME.set(client_name_from(msg));
}

fn mcp_caller() -> String {
    let name = MCP_CLIENT_NAME.get().map(String::as_str).unwrap_or("unknown");
    format!("mcp:{name}:{}", std::process::id())
}

/// Control-socket body. `caller` is top-level so it cannot be set from
/// tool arguments. `args` is forwarded unchanged.
fn control_request(tool: &str, args: Value) -> Value {
    json!({
        "v": 1,
        "id": "1",
        "tool": tool,
        "args": args,
        "caller": mcp_caller(),
    })
}

/// One request → one reply on the Unix control socket.
fn unix_call(tool: &str, args: Value) -> Result<Value, String> {
    let sock = sock_path();
    let mut stream = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(e) => {
            let why = match e.kind() {
                io::ErrorKind::NotFound => "no control socket".to_string(),
                _ => e.to_string(),
            };
            return Err(format!(
                "Orbitty is not running ({why}: {})",
                sock.display()
            ));
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(360)));

    let mut req = control_request(tool, args);
    if let Some(t) = token(&sock) {
        req["token"] = Value::String(t);
    }
    let mut line = serde_json::to_string(&req).unwrap_or_else(|_| "{}".into());
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("write to Orbitty: {e}"))?;

    let mut reply_line = String::new();
    let mut reader = io::BufReader::new((&stream).take(64 << 20));
    match reader.read_line(&mut reply_line) {
        Ok(0) => return Err("Orbitty closed the connection without a reply".into()),
        Ok(_) => {}
        Err(e) => return Err(format!("read from Orbitty: {e}")),
    }
    serde_json::from_str(reply_line.trim())
        .map_err(|e| format!("bad reply from Orbitty: {e}"))
}

fn rpc_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_err(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn call_result_ok(data: &Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into())
        }]
    })
}

fn call_result_err(msg: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true
    })
}

fn dispatch(msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    // Notifications have no id — no reply.
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => {
            remember_client_name(msg);
            let client_ver = msg
                .pointer("/params/protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or(PROTOCOL);
            let ver = if client_ver.is_empty() { PROTOCOL } else { client_ver };
            Some(rpc_ok(
                &id,
                json!({
                    "protocolVersion": ver,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
                }),
            ))
        }
        "ping" => Some(rpc_ok(&id, json!({}))),
        "tools/list" => Some(rpc_ok(&id, tools_list())),
        "tools/call" => {
            let name = msg
                .pointer("/params/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                return Some(rpc_err(&id, -32602, "tools/call missing params.name"));
            }
            if !known_tool(&name) {
                return Some(rpc_err(
                    &id,
                    -32602,
                    &format!("unknown tool {name:?} (v0 list only)"),
                ));
            }
            let args = msg
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or(json!({}));
            let args = if args.is_null() {
                json!({})
            } else if args.is_object() {
                args
            } else {
                json!({})
            };
            match unix_call(&name, args) {
                Err(e) => Some(rpc_err(&id, -32002, &e)),
                Ok(reply) => {
                    if reply.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                        let data = reply.get("data").cloned().unwrap_or(Value::Null);
                        Some(rpc_ok(&id, call_result_ok(&data)))
                    } else {
                        let code = reply.get("code").and_then(|v| v.as_i64()).unwrap_or(1);
                        let err = reply
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error");
                        let tagged = match code {
                            3 => format!("auth failed (3): {err}"),
                            4 => format!("denied (4): {err}"),
                            5 => format!("accessibility (5): {err}"),
                            _ => format!("orbitty error ({code}): {err}"),
                        };
                        Some(rpc_ok(&id, call_result_err(&tagged)))
                    }
                }
            }
        }
        "" => {
            // Bare JSON-RPC without method — ignore if we somehow got here.
            Some(rpc_err(&id, -32600, "missing method"))
        }
        other => Some(rpc_err(&id, -32601, &format!("method not found: {other}"))),
    }
}

/// Goose / rmcp speak MCP newline-delimited JSON. Older smoke tests used
/// LSP Content-Length. Accept both; reply in the same framing as the request.
#[derive(Clone, Copy)]
enum Framing {
    Ndjson,
    Lsp,
}

fn parse_json_bytes(buf: &[u8]) -> io::Result<Value> {
    serde_json::from_slice(buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad JSON: {e}")))
}

fn read_lsp_body(stdin: &mut impl BufRead, first_len: usize) -> io::Result<Option<Value>> {
    if first_len > 16 << 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    // Consume remaining headers until blank line.
    loop {
        let mut header = String::new();
        let n = stdin.read_line(&mut header)?;
        if n == 0 {
            return Ok(None);
        }
        if header.trim_end_matches(['\r', '\n']).is_empty() {
            break;
        }
    }
    let mut buf = vec![0u8; first_len];
    stdin.read_exact(&mut buf)?;
    Ok(Some(parse_json_bytes(&buf)?))
}

/// Read one MCP message. `framing` is set from the first request.
fn read_message(
    stdin: &mut impl BufRead,
    framing: &mut Option<Framing>,
) -> io::Result<Option<(Value, Framing)>> {
    loop {
        let mut line = String::new();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            let len = rest.trim().parse::<usize>().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Content-Length: {e}"))
            })?;
            let mode = Framing::Lsp;
            *framing = Some(mode);
            let Some(v) = read_lsp_body(stdin, len)? else {
                return Ok(None);
            };
            return Ok(Some((v, mode)));
        }
        let mode = Framing::Ndjson;
        *framing = Some(mode);
        let v = parse_json_bytes(trimmed.as_bytes())?;
        return Ok(Some((v, mode)));
    }
}

fn write_message(stdout: &mut impl Write, v: &Value, framing: Framing) -> io::Result<()> {
    let body = serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec());
    match framing {
        Framing::Ndjson => {
            stdout.write_all(&body)?;
            stdout.write_all(b"\n")?;
        }
        Framing::Lsp => {
            write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
            stdout.write_all(&body)?;
        }
    }
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> Value {
        tools_list()["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("missing tool {name}"))
            .clone()
    }

    fn required(name: &str) -> Vec<String> {
        tool(name)["inputSchema"]["required"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn claim_tools_list_schemas() {
        assert!(required("claim.front").is_empty(), "claim.front args none");
        assert!(required("claim.undo").is_empty(), "claim.undo args none");
        assert_eq!(required("claim.to"), vec!["projectId".to_string()]);
        assert_eq!(required("claim.change"), vec!["projectId".to_string()]);
        assert_eq!(required("claim.unclaim"), vec!["windowId".to_string()]);
        for name in ["claim.to", "claim.change"] {
            assert_eq!(
                tool(name)["inputSchema"]["properties"]["projectId"]["type"],
                "string",
                "{name}"
            );
        }
        assert_eq!(
            tool("claim.unclaim")["inputSchema"]["properties"]["windowId"]["type"],
            "string"
        );
    }

    /// Every `"claim.<x>" =>` arm in control.rs, and every claim.* ToolDef.
    fn control_claim_arms() -> Vec<String> {
        let src = include_str!("../agents/control.rs");
        let mut names = Vec::new();
        for line in src.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("\"claim.") else {
                continue;
            };
            let Some(end) = rest.find('"') else {
                continue;
            };
            let verb = &rest[..end];
            if verb.is_empty()
                || !verb
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                continue;
            }
            if rest[end + 1..].trim_start().starts_with("=>") {
                names.push(format!("claim.{verb}"));
            }
        }
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn archive_verbs_tooldef_arm_match() {
        let control = include_str!("../agents/control.rs");
        for name in ["companion.archive_tail", "companion.resets", "companion.roll"] {
            assert!(known_tool(name), "{name}");
            assert!(control.contains(&format!("\"{name}\" =>")), "{name}");
        }
        assert!(control.contains("\"companion.archive_tail\","));
        assert!(control.contains("\"companion.resets\","));
        assert!(!control.contains("\"companion.roll\","));
        let roll = tool("companion.roll");
        assert_eq!(roll["inputSchema"]["required"], json!(["reason"]));
        assert_eq!(
            roll["inputSchema"]["properties"]["reason"]["enum"],
            json!(["manual_test"])
        );
        assert!(required("companion.archive_tail").is_empty());
        assert!(required("companion.resets").is_empty());
    }

    #[test]
    fn recall_is_read_and_tag_writes_are_not() {
        let control = include_str!("../agents/control.rs");
        assert!(known_tool("companion.recall"));
        assert!(control.contains("\"companion.recall\","));
        assert!(control.contains("\"companion.recall\" =>"));
        assert!(control.contains("\"companion.tags_propose\" =>"));
        assert!(control.contains("\"companion.tags_accept\" =>"));
        assert!(!control.contains("\"companion.tags_propose\","));
        assert!(!control.contains("\"companion.tags_accept\","));
        let recall = tool("companion.recall");
        assert_eq!(recall["inputSchema"]["required"], json!(["query"]));
        assert_eq!(tool("companion.tags_propose")["inputSchema"]["required"], json!(["names"]));
        assert!(required("companion.tags_accept").is_empty());
    }

    #[test]
    fn claim_tooldefs_match_control_arms() {
        let arms = control_claim_arms();
        let mut tools: Vec<String> = TOOLS
            .iter()
            .filter(|t| t.name.starts_with("claim."))
            .map(|t| t.name.to_string())
            .collect();
        tools.sort();
        assert_eq!(
            tools, arms,
            "each control.rs claim.* arm needs a ToolDef, and each claim.* ToolDef needs an arm"
        );
        assert!(
            arms.contains(&"claim.change".to_string()),
            "control.rs claim.change arm missing from the parity set"
        );
    }

    #[test]
    fn history_tools_match_control_arms() {
        let control = include_str!("../agents/control.rs");
        for name in ["history.list", "history.checkpoint"] {
            assert!(known_tool(name), "{name}");
            assert!(control.contains(&format!("\"{name}\" =>")), "{name}");
        }
        assert!(required("history.list").is_empty());
        let props = &tool("history.list")["inputSchema"]["properties"];
        assert_eq!(props["card"]["type"], "string");
        assert_eq!(props["project"]["type"], "string");
        assert_eq!(props["limit"]["type"], "integer");
        assert_eq!(props["limit"]["maximum"], 500);
    }

    #[test]
    fn events_since_tool_matches_control() {
        let control = include_str!("../agents/control.rs");
        assert!(known_tool("events.since"));
        assert!(control.contains("\"events.since\" =>"));
        assert_eq!(required("events.since"), vec!["after".to_string()]);
        let props = &tool("events.since")["inputSchema"]["properties"];
        assert_eq!(props["after"]["type"], "integer");
        assert_eq!(props["limit"]["maximum"], 500);
    }

    #[test]
    fn flags_tools_match_control_arms() {
        let control = include_str!("../agents/control.rs");
        for name in ["flags.list", "flags.set"] {
            assert!(known_tool(name), "{name}");
            assert!(control.contains(&format!("\"{name}\" =>")), "{name}");
        }
        assert!(required("flags.list").is_empty());
        assert_eq!(
            required("flags.set"),
            vec!["id".to_string(), "on".to_string()]
        );
        assert!(!known_tool("flags.auto"));
    }

    #[test]
    fn go_tools_match_control_arms() {
        let control = include_str!("../agents/control.rs");
        for name in [
            "go.start",
            "go.run_status",
            "go.run_result",
            "go.runner_get",
            "go.runner_set",
        ] {
            assert!(known_tool(name), "{name}");
            assert!(control.contains(&format!("\"{name}\" =>")), "{name}");
        }
        assert_eq!(required("go.start"), vec!["card"]);
        assert_eq!(required("go.run_status"), vec!["run_id"]);
        assert_eq!(required("go.run_result"), vec!["run_id"]);
        assert!(required("go.runner_get").is_empty());
        assert!(required("go.runner_set").is_empty());
        let listed: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        for banned in ["permission", "developer", "approve", "runner_add"] {
            assert!(
                !listed.iter().any(|n| n.to_ascii_lowercase().contains(banned)),
                "{banned}"
            );
        }
        for name in ["go.permissions_set", "go.runner_add"] {
            let msg = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": name, "arguments": {} }
            });
            let reply = dispatch(&msg).expect("reply");
            assert_eq!(reply["error"]["code"], -32602, "{name}");
        }
    }

    #[test]
    fn history_rewind_tools_match_control_arms() {
        let control = include_str!("../agents/control.rs");
        assert!(control.contains("\"history.drift\" =>"));
        assert!(control.contains("\"history.rewind\" | \"history.repair\" | \"history.keep\" =>"));
        for name in ["history.drift", "history.rewind", "history.repair", "history.keep"] {
            assert!(known_tool(name), "{name}");
        }
        assert!(required("history.drift").is_empty());
        assert_eq!(required("history.rewind"), vec!["to"]);
        let props = &tool("history.rewind")["inputSchema"]["properties"];
        assert_eq!(props["card"]["type"], "string");
        assert_eq!(props["removeAdded"]["type"], "boolean");
        assert_eq!(props["confirm"]["type"], "boolean");
        for name in ["history.repair", "history.keep"] {
            assert!(required(name).is_empty(), "{name}");
            assert_eq!(tool(name)["inputSchema"]["properties"]["confirm"]["type"], "boolean");
        }
    }

    #[test]
    fn companion_harness_tools_are_listed() {
        assert!(TOOLS
            .iter()
            .any(|t| t.name == "companion.harnesses" && t.args == "none"));
        assert!(TOOLS
            .iter()
            .any(|t| t.name == "companion.set_backend" && t.args == "backend_id"));
        assert_eq!(required("companion.set_backend"), vec!["id".to_string()]);
        assert!(TOOLS.iter().any(|t| t.name == "companion.route" && t.args == "line"));
        assert_eq!(required("companion.route"), vec!["line".to_string()]);
        assert_eq!(
            tool("companion.route")["inputSchema"]["properties"]["line"]["type"],
            "string"
        );
        let args = json!({ "line": "board" });
        let req = json!({ "v": 1, "id": "1", "tool": "companion.route", "args": args });
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"args":{"line":"board"},"id":"1","tool":"companion.route","v":1}"#
        );
        let args = json!({ "line": "say \"hi\"" });
        let req = json!({ "v": 1, "id": "1", "tool": "companion.route", "args": args });
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            "{\"args\":{\"line\":\"say \\\"hi\\\"\"},\"id\":\"1\",\"tool\":\"companion.route\",\"v\":1}"
        );
    }

    #[test]
    fn unknown_claim_tool_is_rejected() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "claim.bogus", "arguments": {} }
        });
        let reply = dispatch(&msg).expect("reply");
        assert_eq!(reply["error"]["code"], -32602);
    }

    /// T9. caller is top-level on the forwarded request. args pass through.
    #[test]
    fn forwarded_request_pins_top_level_caller() {
        let init = json!({
            "params": { "clientInfo": { "name": "  cursor  " } }
        });
        assert_eq!(client_name_from(&init), "cursor");
        let blank = json!({ "params": { "clientInfo": { "name": "   " } } });
        assert_eq!(client_name_from(&blank), "unknown");

        let args = json!({ "projectId": "planet-a", "caller": "not-me" });
        let req = control_request("claim.to", args);
        let caller = req["caller"].as_str().expect("top-level caller");
        assert!(caller.starts_with("mcp:"), "{caller}");
        let pid = caller.rsplit(':').next().unwrap();
        assert!(pid.chars().all(|c| c.is_ascii_digit()), "{caller}");
        assert_eq!(req["args"]["caller"], "not-me");
        assert_eq!(req["args"]["projectId"], "planet-a");
        assert_eq!(req["tool"], "claim.to");
        assert_ne!(req["caller"], "not-me");
        assert!(tool("claim.undo")["description"]
            .as_str()
            .unwrap()
            .contains("your own pending claim"));
        assert!(tool("claim.change")["description"]
            .as_str()
            .unwrap()
            .contains("~3 s"));
    }
}

pub fn run() {
    let mut stdin = io::BufReader::new(io::stdin());
    let mut stdout = io::stdout();
    let mut framing: Option<Framing> = None;
    loop {
        let (msg, mode) = match read_message(&mut stdin, &mut framing) {
            Ok(Some(pair)) => pair,
            Ok(None) => break,
            Err(e) => {
                eprintln!("orbitty-mcp: read: {e}");
                break;
            }
        };
        // Batch arrays not required for MCP-0.
        if let Some(reply) = dispatch(&msg) {
            if let Err(e) = write_message(&mut stdout, &reply, mode) {
                eprintln!("orbitty-mcp: write: {e}");
                break;
            }
        }
    }
}

} // mod imp

#[cfg(unix)]
fn main() {
    imp::run();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("orbitty-mcp is unix-only (Orbitty control socket)");
    std::process::exit(1);
}
