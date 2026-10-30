//! Read side of herdr: one client, TUI or hc.
//! `crate::herdr::branch` and the other launch names stay the habitat
//! module's, re-exported here so existing call sites keep compiling.

pub(crate) mod client;
pub(crate) mod core;
pub(crate) mod health;
pub(crate) mod peek;
pub(crate) mod redact;
pub(crate) mod tui;

#[cfg(test)]
mod tests;

pub use crate::habitat::herdr::Branch;
pub use crate::habitat::herdr::INSTALL_HINT;
pub use crate::habitat::herdr::branch;
pub(crate) use crate::habitat::herdr::agent_alive;
pub(crate) use crate::habitat::herdr::herdr_cmd;
pub(crate) use crate::habitat::herdr::planet_cwd;
pub(crate) use crate::habitat::herdr::run_json;

pub(crate) use client::alive;
pub(crate) use client::available;
pub(crate) use client::core_on;
pub(crate) use client::ensure_server;
pub(crate) use client::locate;
pub(crate) use client::status_value;

use tauri::{AppHandle, Emitter};

pub fn present(app: &AppHandle, entry_id: &str) {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    let v = peek::view(Some(app), entry_id, peek::DEFAULT_LINES, false);
    let _ = app.emit("herdr-peek", &v);
}

pub fn status_for(app: &AppHandle) -> serde_json::Value {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    client::status_value()
}

pub fn peek_for(app: &AppHandle, card: &str, lines: u32) -> serde_json::Value {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    let v = peek::view(Some(app), card, lines, true);
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

fn flush_health(app: &AppHandle) {
    for source in health::take_unlogged() {
        crate::event_log::append_best(
            app,
            "herdr-health",
            None,
            None,
            Some("agent"),
            true,
            &serde_json::json!({ "source": source }),
        );
    }
}

pub(crate) fn capsule_line(app: &AppHandle, entry_id: &str) -> String {
    let cid = entry_id.strip_prefix("agent:").unwrap_or(entry_id);
    let spend = spend_of(app);
    if !core_on() {
        return String::new();
    }
    let v = peek::view(Some(app), cid, 0, false);
    if v.status == "unknown" {
        format!("unknown{spend}")
    } else if v.state.is_empty() {
        format!("{}{spend}", v.status)
    } else {
        format!("{} · {}{spend}", v.status, v.state)
    }
}

fn spend_of(app: &AppHandle) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return String::new();
    };
    if st.spend_usd > 0.0 {
        format!(" · ${:.2}", st.spend_usd)
    } else {
        String::new()
    }
}
