//! Read side of herdr: one client, TUI or hc.
//! `crate::herdr::branch` and the other launch names stay the habitat
//! module's, re-exported here so existing call sites keep compiling.

pub(crate) mod client;
pub(crate) mod core;
pub(crate) mod events;
pub(crate) mod health;
pub(crate) mod jump;
pub(crate) mod peek;
pub(crate) mod redact;
pub(crate) mod tui;

#[cfg(test)]
mod tests;

pub use crate::habitat::herdr::Branch;
pub use crate::habitat::herdr::branch;
pub(crate) use crate::habitat::herdr::agent_alive;
pub(crate) use crate::habitat::herdr::planet_cwd;

pub(crate) use client::available;
pub(crate) use client::core_on;
pub(crate) use client::locate;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

type WalkerPointsFn = fn(&str) -> Option<crate::state::CompanionBounds>;

// habitat_mac stays a private module. It registers the walker lookup at
// habitat start so present() can place the pane without a new habitat export.
static WALKER_POINTS: Mutex<Option<WalkerPointsFn>> = Mutex::new(None);

pub(crate) fn set_walker_points_lookup(f: WalkerPointsFn) {
    if let Ok(mut slot) = WALKER_POINTS.lock() {
        *slot = Some(f);
    }
}

pub(crate) fn walker_points(entry_id: &str) -> Option<crate::state::CompanionBounds> {
    let f = WALKER_POINTS.lock().ok().and_then(|slot| *slot)?;
    f(entry_id)
}

fn runs_for(app: &AppHandle) -> Vec<crate::state::AgentRun> {
    crate::state::load_state(app)
        .map(|st| st.agent_runs)
        .unwrap_or_default()
}

pub fn present(app: &AppHandle, entry_id: &str) {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    let runs = runs_for(app);
    let v = peek::view(&runs, entry_id, peek::DEFAULT_LINES, false);
    if crate::panes::show_peek_capsule(app, &v, entry_id).is_ok() {
        let _ = app.emit_to("peek-capsule", "herdr-peek", &v);
    }
}

pub fn status_for(app: &AppHandle) -> serde_json::Value {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    client::status_value()
}

pub fn peek_for(app: &AppHandle, card: &str, lines: u32) -> serde_json::Value {
    let _ = crate::flags::on(app, "herdr_core");
    flush_health(app);
    let runs = runs_for(app);
    let v = peek::view(&runs, card, lines, true);
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

pub fn jump_for(app: &AppHandle, card: &str, observe: Option<bool>) -> serde_json::Value {
    let v = jump::jump_for(app, card, observe);
    flush_health(app);
    v
}

fn flush_health(app: &AppHandle) {
    for ev in health::take_unlogged() {
        crate::event_log::append_best(
            app,
            "herdr-health",
            None,
            None,
            Some("agent"),
            true,
            &serde_json::json!({ "source": ev.source, "kind": ev.kind }),
        );
    }
}

pub(crate) fn capsule_line(app: &AppHandle, entry_id: &str) -> String {
    let cid = entry_id.strip_prefix("agent:").unwrap_or(entry_id);
    let spend = spend_of(app);
    if !core_on() {
        return String::new();
    }
    let runs = runs_for(app);
    let v = peek::view(&runs, cid, 0, false);
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
