//! GO-ORBITTY-MOUTH-TODO — desk wrap for the companion mouth.
//!
//! The model (never the saved transcript) sees a short `[desk]` prepend:
//! one board-law line (Orbitty Todos = tool `kanban.add`; graph-sync /
//! Hive is not this board) plus the live `projects[]` as `name → id`, so
//! "add a todo on fkt: …" can mint on the fkt disc instead of drifting
//! to graph-sync. Capped (~1k chars). Fail OPEN: state unreadable → the
//! prompt goes out unchanged; a ping is never blocked. Transcript,
//! captions, and the event log keep the raw line.

use tauri::AppHandle;

use crate::state::PersistedState;

/// Chars, whole desk block — the mouth prompt stays small.
const DESK_CAP: usize = 1024;

const LAW: &str = "[desk]\nOrbitty Todos = tool kanban.add (projectId = planet id or name); graph-sync / Hive is not this board.\n";

/// True when the raw user line is a board mint, not small talk.
pub(crate) fn line_needs_desk(line: &str) -> bool {
    let l = line.trim().to_ascii_lowercase();
    l.starts_with("todo:")
        || l.contains("kanban")
        || l.contains("add a todo")
        || l.contains("on planet")
}

/// Prepend the desk block to an already pocket-wrapped prompt. One job:
/// law line + planet list. Called next to `wrap_with_pocket` in
/// `companion_send_inner`.
pub(crate) fn wrap_with_desk(app: &AppHandle, prompt: &str) -> String {
    let st = match crate::state::load_state(app) {
        Ok(s) => s,
        Err(_) => return prompt.to_string(),
    };
    format!("{}\n{prompt}", desk_block(&st))
}

/// Law line always (inbox adds still count); `name → id` per project,
/// budgeted — planet lines give way first (dropped from the end), the
/// law line never shrinks.
fn desk_block(st: &PersistedState) -> String {
    let mut out = String::from(LAW);
    if st.projects.is_empty() {
        return out;
    }
    out.push_str("planets:\n");
    for p in &st.projects {
        let line = format!("- {} → {}\n", p.name.trim().replace('\n', " "), p.id);
        if out.chars().count() + line.chars().count() > DESK_CAP {
            break;
        }
        out.push_str(&line);
    }
    out
}
