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
    let mut v = peek::view(&runs, entry_id, peek::DEFAULT_LINES, false);
    if crate::flags::on(app, "seat_outcomes") {
        if let Some(o) = crate::agents::outcome::for_view(app, &v) {
            v.outcome = crate::agents::outcome::phrase_of(&o);
        }
    }
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
    let mut v = peek::view(&runs, card, lines, true);
    if crate::flags::on(app, "seat_outcomes") {
        if let Some(o) = crate::agents::outcome::for_view(app, &v) {
            v.outcome = crate::agents::outcome::phrase_of(&o);
        }
    }
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

pub fn jump_for(app: &AppHandle, card: &str, observe: Option<bool>) -> serde_json::Value {
    let v = jump::jump_for(app, card, observe);
    flush_health(app);
    v
}

pub(crate) fn flush_health(app: &AppHandle) {
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

#[cfg(test)]
mod seat_outcome_fill {
    /// `present` / `peek_for` need a live app handle and `present` only
    /// emits after the pill window shows, so this checks the source: the
    /// `v.outcome` assignment sits inside `flags::on(app, "seat_outcomes")`.
    fn fn_body(src: &str, name: &str) -> String {
        let start = src.find(name).unwrap_or_else(|| panic!("missing {name}"));
        let bytes = src[start..].as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i] != b'{' {
            i += 1;
        }
        assert!(i < bytes.len(), "no brace for {name}");
        let mut depth = 0;
        let mut in_str = false;
        let mut escape = false;
        let begin = i;
        while i < bytes.len() {
            let c = bytes[i];
            if in_str {
                if escape {
                    escape = false;
                } else if c == b'\\' {
                    escape = true;
                } else if c == b'"' {
                    in_str = false;
                }
                i += 1;
                continue;
            }
            match c {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return src[start + begin..=start + i].to_string();
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("unclosed {name}");
    }

    fn brace_end(src: &str, open: usize) -> usize {
        let bytes = src.as_bytes();
        let mut i = open;
        let mut depth = 0;
        let mut in_str = false;
        let mut escape = false;
        while i < bytes.len() {
            let c = bytes[i];
            if in_str {
                if escape {
                    escape = false;
                } else if c == b'\\' {
                    escape = true;
                } else if c == b'"' {
                    in_str = false;
                }
                i += 1;
                continue;
            }
            match c {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("unclosed brace at {open}");
    }

    fn assert_outcome_only_inside_flag(src: &str, sig: &str) {
        let body = fn_body(src, sig);
        assert!(
            !body.contains("assert_outcome_only_inside_flag"),
            "{sig} slice ran past the function into this test"
        );
        let guard = "if crate::flags::on(app, \"seat_outcomes\")";
        let at = body.find(guard).unwrap_or_else(|| {
            panic!("{sig} assigns v.outcome with no seat_outcomes flag guard\n{body}")
        });
        let after = at + guard.len();
        let rel = body[after..].find('{').unwrap_or_else(|| {
            panic!("{sig} seat_outcomes guard has no block\n{body}")
        });
        let open = after + rel;
        let close = brace_end(&body, open);
        let inside = &body[open..=close];
        assert!(
            inside.contains("v.outcome ="),
            "{sig} flag guard does not assign v.outcome\n{inside}"
        );
        assert!(
            inside.contains("phrase_of"),
            "{sig} flag guard does not fill the phrase\n{inside}"
        );
        assert!(
            !body[..open].contains("v.outcome"),
            "{sig} touches v.outcome before the seat_outcomes guard\n{body}"
        );
        assert!(
            !body[close + 1..].contains("v.outcome"),
            "{sig} touches v.outcome after the seat_outcomes guard\n{body}"
        );
    }

    #[test]
    fn present_sets_outcome_only_under_seat_outcomes() {
        let src = include_str!("mod.rs");
        let body = fn_body(src, "pub fn present(");
        assert!(body.contains("show_peek_capsule"), "{body}");
        assert_outcome_only_inside_flag(src, "pub fn present(");
    }

    #[test]
    fn peek_for_sets_outcome_only_under_seat_outcomes() {
        let src = include_str!("mod.rs");
        let body = fn_body(src, "pub fn peek_for(");
        assert!(body.contains("serde_json::to_value"), "{body}");
        assert_outcome_only_inside_flag(src, "pub fn peek_for(");
    }
}
