//! Hang route table. Keyword gate onto doors that already exist.
//! A miss stays chat. Not a sixth conductor verb. Never a new seat.

use serde_json::json;

mod write;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(crate) use write::*;

/// Closed set. Keep this ≤ 15.
pub const TABLE: [&str; 10] = [
    "gate-status",
    "shift-status",
    "companion-note",
    "open-planet",
    "needed-list",
    "agent-list",
    "planet-list",
    "board-list",
    "board-add",
    "agent-spawn",
];

/// Confirm window. Anything older is dropped and does not write.
pub const CONFIRM_MS: u64 = 60_000;

/// Which existing tool this Hang line is, if any.
/// `yes` is not a route; it confirms a pending board write inside `run`.
pub fn classify(line: &str) -> Option<&'static str> {
    if is_stop_word(line) {
        return None;
    }
    let l = line.trim().to_ascii_lowercase();
    if l.is_empty() {
        return None;
    }
    if let Some(parsed) = parse_board_write(line) {
        return Some(parsed.route);
    }
    if is_gate(&l) {
        return Some("gate-status");
    }
    if is_shift(&l) {
        return Some("shift-status");
    }
    if note_body(line).is_some() {
        return Some("companion-note");
    }
    if open_planet_name(line).is_some() {
        return Some("open-planet");
    }
    if is_needed(&l) {
        return Some("needed-list");
    }
    if is_agents(&l) {
        return Some("agent-list");
    }
    if is_planets(&l) {
        return Some("planet-list");
    }
    if is_board(&l) {
        return Some("board-list");
    }
    None
}

/// What `route.run` returns on every surface (socket, orbittyctl, MCP).
#[derive(Clone, Debug)]
pub struct RouteOutcome {
    pub tool: Option<String>,
    pub spoken: Option<String>,
    pub pending_id: Option<String>,
}

/// Which surface sent the line. A pending board write confirms only on
/// the surface that proposed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The app composer (Tauri `companion_send`). A bare yes confirms a
    /// chat-made pending only.
    Chat,
    /// Socket / orbittyctl / MCP `route.run`. The yes must carry the
    /// matching `pendingId`.
    Socket,
    /// Socket `companion.send`. Never proposes or confirms a board write
    /// (the companion profile is denied kanban.add / agent.pickup).
    Companion,
}

/// Run the tool. None spoken = leave the line for chat.
pub fn run(app: &tauri::AppHandle, line: &str, origin: Origin) -> Option<String> {
    run_outcome(app, line, origin, None).spoken
}

/// Same door as [`run`], plus the pending confirm id. `confirm_id` is the
/// `pendingId` a socket caller got back from its own proposal.
pub fn run_outcome(
    app: &tauri::AppHandle,
    line: &str,
    origin: Origin,
    confirm_id: Option<&str>,
) -> RouteOutcome {
    let now = std::time::Instant::now();
    let id = new_pending_id();
    let decision = {
        let mut slot = pending_slot().lock().unwrap_or_else(|e| e.into_inner());
        if is_stop_word(line) {
            step(&mut slot, line, &[], &[], now, &id, origin, confirm_id)
        } else {
            match crate::state::load_state(app) {
                Ok(st) => {
                    let planets = planet_rows(&st);
                    step(
                        &mut slot,
                        line,
                        &planets,
                        &st.agent_shortcuts,
                        now,
                        &id,
                        origin,
                        confirm_id,
                    )
                }
                Err(_) => unreadable(&mut slot, line, now, origin, confirm_id),
            }
        }
    };
    match decision {
        Decision::Miss => {
            // Base path: write-lane lines skip the read doors (as before
            // the board-add route) and hop to the write lane.
            if crate::mouth_gist::needs_write_lane(line) {
                return RouteOutcome {
                    tool: None,
                    spoken: None,
                    pending_id: None,
                };
            }
            run_read(app, line)
        }
        Decision::Confirm { pending, spoken } => {
            let tool = pending.resolved.route.to_string();
            // `step` already stored it in the slot.
            let pending_id = pending.id.clone();
            log_tool(app, &tool);
            RouteOutcome {
                tool: Some(tool),
                spoken: Some(spoken),
                pending_id: Some(pending_id),
            }
        }
        Decision::Ask { route, spoken } => {
            log_tool(app, route);
            RouteOutcome {
                tool: Some(route.to_string()),
                spoken: Some(spoken),
                pending_id: None,
            }
        }
        Decision::Execute(p) => {
            let tool = p.resolved.route.to_string();
            let spoken = commit(app, &p.resolved);
            log_tool(app, &tool);
            RouteOutcome {
                tool: Some(tool),
                spoken: Some(spoken),
                pending_id: None,
            }
        }
        Decision::Decline(spoken) => RouteOutcome {
            tool: None,
            spoken: Some(spoken),
            pending_id: None,
        },
    }
}

/// Board unreadable: a board write (or the owner's reply to its pending)
/// gets an honest ask and never executes.
fn unreadable(
    slot: &mut Option<PendingWrite>,
    line: &str,
    now: std::time::Instant,
    origin: Origin,
    confirm_id: Option<&str>,
) -> Decision {
    let mine = slot
        .as_ref()
        .is_some_and(|p| fresh(p, now) && owns(p, origin, confirm_id));
    let write = origin != Origin::Companion && parse_board_write(line).is_some();
    if !mine && !write {
        return Decision::Miss;
    }
    if mine {
        *slot = None;
    }
    Decision::Ask {
        route: parse_board_write(line)
            .map(|p| p.route)
            .unwrap_or("board-add"),
        spoken: "Board unreadable.".into(),
    }
}

fn run_read(app: &tauri::AppHandle, line: &str) -> RouteOutcome {
    let Some(id) = classify(line) else {
        return RouteOutcome {
            tool: None,
            spoken: None,
            pending_id: None,
        };
    };
    if !TABLE.contains(&id) || id == "board-add" || id == "agent-spawn" {
        return RouteOutcome {
            tool: None,
            spoken: None,
            pending_id: None,
        };
    }
    let spoken = match id {
        "gate-status" => gate_line(),
        "shift-status" => shift_line(app),
        "companion-note" => note_line(line),
        "open-planet" => open_planet(app, line),
        "needed-list" => needed_line(app, line),
        "agent-list" => agent_line(app),
        "planet-list" => planet_line(app),
        "board-list" => crate::mouth_board::speak(app, line),
        _ => {
            return RouteOutcome {
                tool: None,
                spoken: None,
                pending_id: None,
            };
        }
    };
    log_tool(app, id);
    RouteOutcome {
        tool: Some(id.to_string()),
        spoken: Some(spoken),
        pending_id: None,
    }
}

pub(crate) fn is_stop_word(line: &str) -> bool {
    let b = line
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?'))
        .to_ascii_lowercase();
    matches!(b.as_str(), "stop" | "abort" | "chatter")
}

fn is_gate(l: &str) -> bool {
    l.contains("gate status")
        || l.contains("interrupt gate")
        || l.contains("is the gate")
        || l.contains("hang armed")
        || l.contains("is hang armed")
}

fn is_shift(l: &str) -> bool {
    l.contains("shift status") || l.contains("is shift on") || l.contains("is shift off")
}

fn is_needed(l: &str) -> bool {
    l.contains("needed list")
        || l.contains("open needed")
        || l.contains("list needed")
        || l.contains("what's needed")
        || l.contains("whats needed")
}

fn is_agents(l: &str) -> bool {
    l.contains("list agents")
        || l.contains("which agents")
        || l.contains("what agents")
        || l.contains("who is seated")
        || l.contains("who's seated")
}

fn is_planets(l: &str) -> bool {
    l.contains("list planets") || l.contains("which planets") || l.contains("what planets")
}

fn is_board(l: &str) -> bool {
    if l.contains("todo list") || l.contains("board snapshot") || l.contains("open cards") {
        return true;
    }
    let listish = l.contains("list")
        || l.contains("what's on")
        || l.contains("whats on")
        || l.contains("what is on")
        || l.contains("what's in")
        || l.contains("whats in")
        || l.contains("what is in");
    let boardish = l.contains("todo")
        || l.contains("kanban")
        || l.contains("board")
        || l.contains("card")
        || l.contains("doing");
    listish && boardish
}

fn note_body(line: &str) -> Option<String> {
    let t = line.trim();
    let low = t.to_ascii_lowercase();
    let rest = if low.starts_with("companion note") {
        t.get(14..)
            .unwrap_or("")
            .trim()
            .trim_start_matches(':')
            .trim()
    } else if low.starts_with("note:") {
        t.get(5..).unwrap_or("").trim()
    } else {
        return None;
    };
    Some(rest.to_string())
}

pub(crate) fn note_line(line: &str) -> String {
    match note_body(line).as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "Note needs a line.".into(),
    }
}

fn open_planet_name(line: &str) -> Option<String> {
    let l = line.to_ascii_lowercase();
    for marker in ["open planet ", "show planet ", "raise planet "] {
        if let Some(i) = l.find(marker) {
            let rest = line[i + marker.len()..].trim();
            let word: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if word.len() >= 2 {
                return Some(word);
            }
        }
    }
    None
}

fn gate_line() -> String {
    let s = crate::interrupt_gate::interrupt_gate_status();
    format!(
        "Gate is {}. Freeze is {}.",
        if s.armed { "armed" } else { "disarmed" },
        if s.frozen { "on" } else { "off" }
    )
}

fn shift_line(app: &tauri::AppHandle) -> String {
    #[cfg(unix)]
    {
        match crate::shift::status(app) {
            Ok(v) => {
                let on = v.get("on").and_then(|x| x.as_bool()).unwrap_or(false);
                let live = v
                    .get("runners")
                    .and_then(|r| r.get("live"))
                    .and_then(|n| n.as_u64())
                    .unwrap_or(0);
                format!(
                    "Shift is {}. Live runners: {live}.",
                    if on { "on" } else { "off" }
                )
            }
            Err(e) => format!("Shift status failed: {e}"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = app;
        "Shift status is on the Mac build.".into()
    }
}

fn find_project(app: &tauri::AppHandle, name: &str) -> Result<(String, String), String> {
    let st = crate::state::load_state(app).map_err(|_| "board unreadable".to_string())?;
    let want = name.trim().to_ascii_lowercase();
    let Some(p) = st
        .projects
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(&want) || p.name.trim().eq_ignore_ascii_case(&want))
    else {
        return Err(format!("No planet {want}."));
    };
    let display = {
        let n = p.name.trim();
        if n.is_empty() {
            want.clone()
        } else {
            n.to_string()
        }
    };
    Ok((p.id.clone(), display))
}

fn open_planet(app: &tauri::AppHandle, line: &str) -> String {
    let Some(name) = open_planet_name(line) else {
        return "Say which planet to open.".into();
    };
    match find_project(app, &name) {
        Ok((id, display)) => {
            crate::habitat::bring_planet_front(&id);
            let _ = crate::habitat::locate_planet(app.clone(), id);
            format!("Opened {display}.")
        }
        Err(e) => e,
    }
}

fn needed_line(app: &tauri::AppHandle, line: &str) -> String {
    let project = crate::mouth_tools::project_from_line(line);
    match crate::mouth_tools::collect_open(app, &project) {
        Ok(b) if b.needed_n == 0 => format!("No open Needed on {}.", b.display),
        Ok(b) => {
            let mut s = format!("Needed on {}: {}.", b.display, b.needed_n);
            for t in &b.needed {
                s.push('\n');
                s.push_str("- ");
                s.push_str(t);
            }
            s
        }
        Err(e) => e,
    }
}

fn agent_line(app: &tauri::AppHandle) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return "Agents unreadable.".into();
    };
    if st.agent_shortcuts.is_empty() {
        return "No agents.".into();
    }
    let parts: Vec<String> = st
        .agent_shortcuts
        .iter()
        .map(|a| format!("{} ({})", a.name.trim(), a.harness.trim()))
        .collect();
    format!("Agents: {}.", parts.join(", "))
}

fn planet_line(app: &tauri::AppHandle) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return "Planets unreadable.".into();
    };
    let names: Vec<&str> = st
        .projects
        .iter()
        .map(|p| p.name.trim())
        .filter(|n| !n.is_empty())
        .take(24)
        .collect();
    if names.is_empty() {
        "No planets.".into()
    } else {
        format!("Planets: {}.", names.join(", "))
    }
}

fn log_tool(app: &tauri::AppHandle, tool: &str) {
    crate::event_log::append_best(
        app,
        "companion-hop",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "lane": "code", "tool": tool }),
    );
}

