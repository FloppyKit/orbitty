//! Unused expanded planets collapse to marbles after a published idle.
//! 30s cadence. Never preset never collapses. Companion untouched.

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::sync::Mutex;
use std::time::Instant;
use tauri::Emitter;

use super::{app_handle, lock, now_epoch};

const CADENCE: std::time::Duration = std::time::Duration::from_secs(30);

fn last_pass() -> &'static Mutex<Option<Instant>> {
    static T: std::sync::OnceLock<Mutex<Option<Instant>>> = std::sync::OnceLock::new();
    T.get_or_init(|| Mutex::new(None))
}

/// None = Never (clock still stamps elsewhere; this pass no-ops).
pub(crate) fn idle_secs(preset: &str) -> Option<f64> {
    match preset.trim().to_ascii_lowercase().as_str() {
        "1h" => Some(3600.0),
        "12h" => Some(12.0 * 3600.0),
        "never" => None,
        _ => Some(4.0 * 3600.0), // missing / unknown / "4h"
    }
}

pub(crate) fn maybe_pass() {
    let now_i = Instant::now();
    {
        let mut last = last_pass().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *last {
            if now_i.saturating_duration_since(t) < CADENCE {
                return;
            }
        }
        *last = Some(now_i);
    }
    pass();
}

fn pass() {
    let Some(app) = app_handle() else {
        return;
    };
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let Some(limit) = idle_secs(&st.planet_idle) else {
        return;
    };
    let now = now_epoch();
    let in_flight: std::collections::HashSet<String> = st
        .agent_runs
        .iter()
        .filter(|r| r.state != "done")
        .map(|r| r.planet_id.clone())
        .collect();

    let mut victims: Vec<String> = Vec::new();
    {
        let h = lock();
        let mut focused: Option<String> = None;
        let mut focused_used = f64::NEG_INFINITY;
        for (pid, planet) in &h.planets {
            if planet.is_marble {
                continue;
            }
            if panel_is_key(planet.panel) {
                focused = Some(pid.clone());
                break;
            }
            let used = h.worlds.get(pid).map(|w| w.last_used).unwrap_or(0.0);
            if used >= focused_used {
                focused_used = used;
                focused = Some(pid.clone());
            }
        }
        let focused = focused;
        for (pid, planet) in &h.planets {
            if planet.is_marble {
                continue;
            }
            if focused.as_deref() == Some(pid.as_str()) {
                continue;
            }
            if in_flight.contains(pid) {
                continue;
            }
            let used = h.worlds.get(pid).map(|w| w.last_used).unwrap_or(0.0);
            if now - used >= limit {
                victims.push(pid.clone());
            }
        }
    }
    for pid in victims {
        let _ = app.emit_to("main", "habitat:collapse", pid);
    }
}

fn panel_is_key(panel: usize) -> bool {
    if panel == 0 {
        return false;
    }
    unsafe {
        let nsw = panel as *mut Object;
        let key: bool = msg_send![nsw, isKeyWindow];
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_secs_presets() {
        assert_eq!(idle_secs("1h"), Some(3600.0));
        assert_eq!(idle_secs("4h"), Some(14400.0));
        assert_eq!(idle_secs("12h"), Some(43200.0));
        assert_eq!(idle_secs("never"), None);
        assert_eq!(idle_secs(""), Some(14400.0));
        assert_eq!(idle_secs("nope"), Some(14400.0));
    }

    #[test]
    fn unused_past_limit_is_idle() {
        let last = 1000.0;
        let now = 1000.0 + 3600.0;
        let limit = idle_secs("1h").unwrap();
        assert!(now - last >= limit);
        assert!(now - last < idle_secs("4h").unwrap());
    }
}
