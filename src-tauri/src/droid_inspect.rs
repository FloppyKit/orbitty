//! Click a droid: status capsule + focus the herdr pane. Not xterm.

use std::process::Stdio;

use tauri::AppHandle;

use crate::coordinator::find_on_path;
use crate::herdr::{herdr_cmd, run_json};

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
    let Some(bin) = find_on_path("herdr") else {
        return format!("{status}{spend}");
    };
    let target = run
        .and_then(|r| r.herdr.as_ref())
        .map(|h| h.agent_name.as_str())
        .unwrap_or(cid);
    let tool = last_tool_line(&bin, target);
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
    let Some(bin) = find_on_path("herdr") else {
        return;
    };
    let _ = run_json(&bin, &["workspace", "focus", &h.workspace], 8_000);
    let _ = run_json(&bin, &["agent", "focus", &h.agent_name], 8_000);
    let _ = herdr_cmd(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn last_tool_line(bin: &std::path::Path, target: &str) -> Option<String> {
    let out = herdr_cmd(bin)
        .args(["agent", "read", target, "--source", "recent", "--lines", "16"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let raw = String::from_utf8_lossy(&out.stdout);
    let text = if raw.trim().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(raw.trim())
            .ok()
            .and_then(|v| {
                v["result"]["text"]
                    .as_str()
                    .or_else(|| v["result"]["output"].as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default()
    } else {
        raw.into_owned()
    };
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