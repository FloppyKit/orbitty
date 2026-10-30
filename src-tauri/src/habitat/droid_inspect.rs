//! Click a droid: status capsule + focus the herdr pane. Not xterm.
//! A core run has no confirmed Herdr.app deep link (TO CONFIRM); focus
//! is a no-op and the peek capsule is the surface. The last tool line
//! is `hc read` for a core run and `herdr agent read` for a tui run.

use tauri::AppHandle;

pub fn card_id_from_entry(entry_id: &str) -> Option<&str> {
    entry_id.strip_prefix("agent:")
}

pub fn capsule(app: &AppHandle, entry_id: &str) -> String {
    let Some(cid) = card_id_from_entry(entry_id) else {
        return "agent".into();
    };
    let Ok(st) = crate::state::load_state(app) else {
        return format!("{cid} · agent");
    };
    let run = st.agent_runs.iter().find(|r| r.card_id == cid);
    let status = run
        .map(|r| r.state.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("agent");
    let spend = if st.spend_usd > 0.0 {
        format!(" · ${:.2}", st.spend_usd)
    } else {
        String::new()
    };
    if let Some(h) = run.and_then(|r| r.herdr.as_ref()).filter(|h| h.backend == "core") {
        let tool = tool_line(h).unwrap_or_default();
        if crate::herdr::core_on() {
            let peek = crate::herdr::capsule_line(app, entry_id);
            if peek.is_empty() {
                return if tool.is_empty() {
                    format!("{status}{spend}")
                } else {
                    format!("{status} · {tool}{spend}")
                };
            }
            return if tool.is_empty() {
                peek
            } else {
                format!("{peek} · {tool}")
            };
        }
        return if tool.is_empty() {
            format!("{status}{spend}")
        } else {
            format!("{status} · {tool}{spend}")
        };
    }
    if crate::herdr::core_on() {
        let line = crate::herdr::capsule_line(app, entry_id);
        return if line.is_empty() {
            format!("{status}{spend}")
        } else {
            format!("{line}")
        };
    }
    let Some(bin) = crate::herdr::tui::locate() else {
        return format!("{status}{spend}");
    };
    let target = run
        .and_then(|r| r.herdr.as_ref())
        .map(|h| h.agent_name.as_str())
        .unwrap_or(cid);
    let tool = crate::herdr::tui::agent_read(&bin, target)
        .ok()
        .and_then(|text| pick_line(&text));
    match tool {
        Some(t) if !t.is_empty() => format!("{status} · {t}{spend}"),
        _ => format!("{status}{spend}"),
    }
}

pub fn focus(app: &AppHandle, entry_id: &str) {
    let Some(cid) = card_id_from_entry(entry_id) else {
        return;
    };
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let Some(run) = st.agent_runs.iter().find(|r| r.card_id == cid) else {
        return;
    };
    let Some(h) = run.herdr.as_ref() else {
        return;
    };
    crate::herdr::client::focus(h);
}

/// Last tool line for this run's own backend.
pub(crate) fn tool_line(h: &crate::state::AgentRunHerdr) -> Option<String> {
    let text = crate::herdr::client::read_recent(h).ok()?;
    pick_line(&text)
}

fn pick_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('╭')
                && !l.starts_with('│')
                && !l.starts_with('╰')
                && !l.starts_with("Update:")
        })
        .last()
        .map(|s| {
            let s = s.trim_start_matches(['❯', '•', '-', '*']).trim();
            if s.chars().count() > 48 {
                format!("{}…", s.chars().take(47).collect::<String>())
            } else {
                s.to_string()
            }
        })
        .filter(|s| !s.is_empty())
}