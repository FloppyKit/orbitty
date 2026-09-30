//! orbitty-mcp — stdio adapter to the local control socket. No TCP.

#[cfg(unix)]
mod imp {
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const PROTOCOL: &str = "2024-11-05";
const SERVER_NAME: &str = "orbitty";
const SERVER_VERSION: &str = "0.1.0";

struct ToolDef {
    name: &'static str,
    description: &'static str,
    /// "none" | "id" | "line" | "route_line" | "profile" | "clip" | "kanban_add" |
    /// "kanban_move" | "kanban_notes" | "kanban_link" | "kanban_open_link" |
    /// "kanban_assign" | "needed_open" |
    /// "log_append" | "action_add" | "action_run" | "action_rm" |
    /// "project_add" | "project_rm" | "project_rename" | "agent_add" |
    /// "agent_pin" | "agent_pickup"
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
    ToolDef { name: "companion.agents", description: "List companion agent profiles", args: "none" },
    ToolDef { name: "companion.set_agent", description: "Set the companion agent profile", args: "profile" },
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
    ToolDef { name: "kanban.open_link", description: "Open a card link with macOS open (default app, Finder, or the browser). n defaults to 0. Does not change card status.", args: "kanban_open_link" },
    ToolDef { name: "needed.open", description: "Open a Needed inbox card", args: "needed_open" },
    ToolDef { name: "log.append", description: "Append a local-only event to the Orbitty event log (never fails the caller)", args: "log_append" },
    // The planet control plane, next to the kanban
    // writes. Same ACL (full / named custom only). rm keeps cards'
    // projectId and live claims; add dedups same-name rows (newest id).
    ToolDef { name: "project.add", description: "Mint a project planet, or reuse + front the existing one on a same-name add (trim, case-insensitive; empty name refused)", args: "project_add" },
    ToolDef { name: "project.rm", description: "Remove a project planet by id; cards keep their projectId and live claims stay (walkers vanish on next habitat sync)", args: "project_rm" },
    ToolDef { name: "project.rename", description: "Rename a project planet, keeping its id (empty name refused)", args: "project_rename" },
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
    ToolDef { name: "agent.add", description: "Mint an agent row: name + harness (cursor|goose|grok|hermes, required) with optional profile / model / permissions (read-only|workspace-write|full-auto) / mdPaths / extra / projectId pin. New cursor rows default model grok-4.7-high and permissions read-only.", args: "agent_add" },
    ToolDef { name: "agent.rm", description: "Remove an agent row by id; cards keep their assignedAgentId (project.rm law)", args: "id" },
    ToolDef { name: "agent.pin", description: "Pin an agent row to a planet (project id or unique planet name)", args: "agent_pin" },
    ToolDef { name: "agent.pickup", description: "Pick the oldest assigned open Todo (the row's planet when pinned), move it doing, and launch the row's harness with the card title; no match = honest empty, no spawn", args: "agent_pickup" },
    ToolDef { name: "settings.plans_dir", description: "Read the shared plansDir (visible ~/Orbitty GOs unless set) and the default runner", args: "none" },
    ToolDef { name: "settings.plans_dir_set", description: "Set the shared plansDir used by Nightly and stable", args: "plans_dir_set" },
    ToolDef { name: "settings.runner", description: "Read defaultRunner (Cursor harness, model grok-4.7-high, herdr fallback)", args: "none" },
    ToolDef { name: "settings.runner_set", description: "Set defaultRunner runner, harness, and model", args: "runner_set" },
    ToolDef { name: "go.status", description: "Resolve a GO and report whether it is sealed", args: "go_status" },
    ToolDef { name: "go.seal", description: "Seal a GO file (profile full and confirm). Writes status, sealed_at, sealed_by, and seal_hash.", args: "go_seal" },
    ToolDef { name: "plans.migrate", description: "Dry-run: list files that would copy from the back-compat plans folder into plansDir. Never deletes.", args: "none" },
    ToolDef { name: "plans.migrate_apply", description: "Copy those files into plansDir. Never deletes. Not part of a normal GO run.", args: "none" },
    ToolDef { name: "shift.status", description: "Humming supervisor snapshot: on/off, mode, runner cap, agentRuns, next assigned pickups", args: "none" },
    ToolDef { name: "route.table", description: "Hang route table: the closed set of keyword tool ids (max 15). Read-only.", args: "none" },
    ToolDef { name: "route.classify", description: "Classify a Hang line to a route id, or null if it stays chat. Read-only, no side effects.", args: "route_line" },
    ToolDef { name: "route.run", description: "Run a Hang line through the route table (same door as Hang). A miss returns null spoken. Write-class.", args: "route_line" },
    ToolDef { name: "shift.tick", description: "One supervisor pass: park ghost working rows, Needed-escalate blocked, pickup next assigned todo if Work/Hold", args: "none" },
    ToolDef { name: "shift.on", description: "Turn the 15s shift ticker on (Work/Hold pickups; Spark/Chat never auto-pickup)", args: "none" },
    ToolDef { name: "shift.off", description: "Turn the 15s shift ticker off", args: "none" },
    ToolDef { name: "kanban.assign", description: "Assign a card to an agent row (agent id or unique name); empty agent unassigns", args: "kanban_assign" },
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
        "profile" => json!({
            "type": "object",
            "properties": { "profile": { "type": "string", "description": "Agent profile id" } },
            "required": ["profile"]
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
                "n": { "type": "integer", "minimum": 0, "description": "Link index. Default 0." }
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
                    "enum": ["cursor", "goose", "grok", "hermes"],
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

    let mut req = json!({ "v": 1, "id": "1", "tool": tool, "args": args });
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
