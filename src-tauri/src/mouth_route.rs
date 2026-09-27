//! Hang route table. Keyword gate onto doors that already exist.
//! A miss stays chat. Not a sixth conductor verb. Never a new seat.

use serde_json::json;

/// Closed set. Keep this ≤ 15.
pub const TABLE: [&str; 8] = [
    "gate-status",
    "shift-status",
    "companion-note",
    "open-planet",
    "needed-list",
    "agent-list",
    "planet-list",
    "board-list",
];

/// Which existing tool this Hang line is, if any.
pub fn classify(line: &str) -> Option<&'static str> {
    if is_stop_word(line) {
        return None;
    }
    let l = line.trim().to_ascii_lowercase();
    if l.is_empty() {
        return None;
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

/// Run the tool. None = leave the line for chat.
pub fn run(app: &tauri::AppHandle, line: &str) -> Option<String> {
    let id = classify(line)?;
    if !TABLE.contains(&id) {
        return None;
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
        _ => return None,
    };
    log_tool(app, id);
    Some(spoken)
}

fn is_stop_word(line: &str) -> bool {
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
    let listish = l.contains("list") || l.contains("what's on") || l.contains("whats on") || l.contains("what is on");
    let boardish = l.contains("todo") || l.contains("kanban") || l.contains("board") || l.contains("card");
    listish && boardish
}

fn note_body(line: &str) -> Option<String> {
    let t = line.trim();
    let low = t.to_ascii_lowercase();
    let rest = if low.starts_with("companion note") {
        t.get(14..).unwrap_or("").trim().trim_start_matches(':').trim()
    } else if low.starts_with("note:") {
        t.get(5..).unwrap_or("").trim()
    } else {
        return None;
    };
    Some(rest.to_string())
}

fn note_line(line: &str) -> String {
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
    let Some(p) = st.projects.iter().find(|p| {
        p.id.eq_ignore_ascii_case(&want) || p.name.trim().eq_ignore_ascii_case(&want)
    }) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_small_and_closed() {
        assert!(TABLE.len() <= 15);
        assert_eq!(TABLE.len(), 8);
    }

    #[test]
    fn board_list_phrase() {
        assert_eq!(
            classify("tell me about the todo list on planet orbitty"),
            Some("board-list")
        );
        assert_eq!(classify("what's on the board"), Some("board-list"));
    }

    #[test]
    fn open_planet_is_not_the_board() {
        assert_eq!(classify("open planet orbitty"), Some("open-planet"));
        assert_eq!(classify("show planet clinic"), Some("open-planet"));
        assert_ne!(classify("open planet orbitty"), Some("board-list"));
    }

    #[test]
    fn note_gate_and_lists() {
        assert_eq!(classify("note: mouth ping"), Some("companion-note"));
        assert_eq!(classify("gate status"), Some("gate-status"));
        assert_eq!(classify("shift status"), Some("shift-status"));
        assert_eq!(classify("list planets"), Some("planet-list"));
        assert_eq!(classify("list agents"), Some("agent-list"));
        assert_eq!(classify("needed list on planet orbitty"), Some("needed-list"));
    }

    #[test]
    fn chat_and_stop_stay_out() {
        assert_eq!(classify("tell me a joke"), None);
        assert_eq!(classify("STOP"), None);
        assert_eq!(classify("stop"), None);
        assert_eq!(classify("ping"), None);
        assert_eq!(classify("how is the weather"), None);
    }

    #[test]
    fn note_body_keeps_the_words() {
        assert_eq!(note_line("note: mouth ping"), "mouth ping");
        assert_eq!(note_line("companion note: hello"), "hello");
        assert_eq!(note_line("note:"), "Note needs a line.");
    }
}
