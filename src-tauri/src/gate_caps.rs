//! GO-ORBITTY-GATE-CAPS — spend ceiling + runner cap. Needed is the gate.
//! No toasts. Usage USD only when a runner returns parseable cost.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::state::PersistedState;

pub const RUNNER_HARD: u32 = 4;

static LIVE_ACTIONS: AtomicU32 = AtomicU32::new(0);

fn spend_cap_usd_default() -> f64 {
    20.0
}
fn spend_cap_enabled_default() -> bool {
    true
}
fn runner_cap_default() -> u32 {
    2
}

pub fn clamp_runner_cap(n: u32) -> u32 {
    n.clamp(1, RUNNER_HARD)
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn roll_day(st: &mut PersistedState) {
    let d = today();
    if st.spend_day != d {
        st.spend_day = d;
        st.spend_usd = 0.0;
    }
}

fn live_agents(st: &PersistedState) -> u32 {
    st.agent_runs
        .iter()
        .filter(|r| matches!(r.state.as_str(), "working" | "starting" | "blocked"))
        .count() as u32
}

pub fn in_flight(st: &PersistedState) -> u32 {
    live_agents(st).saturating_add(LIVE_ACTIONS.load(Ordering::SeqCst))
}

pub fn parse_usage_usd(text: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let n = v.get("total_cost").or_else(|| v.get("usd")).or_else(|| {
        v.get("usage")
            .and_then(|u| u.get("total_cost").or_else(|| u.get("usd")).or_else(|| u.get("cost")))
    })?;
    let x = n.as_f64().or_else(|| n.as_str()?.parse().ok())?;
    if x.is_finite() && x > 0.0 {
        Some(x)
    } else {
        None
    }
}

pub struct ActionGuard;

impl Drop for ActionGuard {
    fn drop(&mut self) {
        LIVE_ACTIONS.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn begin_action() -> ActionGuard {
    LIVE_ACTIONS.fetch_add(1, Ordering::SeqCst);
    ActionGuard
}

/// Refuse new launches when spend or runner caps are hit. Mints at most
/// one open Needed per title. 80% spend sets companion.state needs_human.
pub fn check(app: &tauri::AppHandle) -> Result<(), String> {
    let before = crate::state::load_state(app)?;
    let mut st = before.clone();
    heal_defaults(&mut st);
    roll_day(&mut st);

    let cap = st.spend_cap_usd;
    let enabled = st.spend_cap_enabled;
    let usd = st.spend_usd;
    if enabled && cap > 0.0 && usd >= cap * 0.8 {
        st.companion.state = "needs_human".into();
    }

    let runners = in_flight(&st);
    let rcap = st.runner_cap;
    let spend_block = enabled && cap > 0.0 && usd >= cap;
    let runner_block = runners >= rcap;

    if st.spend_day != before.spend_day
        || (st.spend_usd - before.spend_usd).abs() > f64::EPSILON
        || st.companion.state != before.companion.state
        || st.runner_cap != before.runner_cap
    {
        let _ = crate::board::save_with_events(app, &before, &st);
    }

    if spend_block {
        let why = format!("Paused new runs. ${usd:.2} of ${cap:.2} today.");
        let _ = crate::board::needed_open(app, "Spend ceiling", "", Some(&why));
        return Err(why);
    }
    if runner_block {
        let why = format!("Paused new runs. {runners} of {rcap} runners live.");
        let _ = crate::board::needed_open(app, "Runner cap", "", Some(&why));
        return Err(why);
    }
    Ok(())
}

pub fn add_spend(app: &tauri::AppHandle, usd: f64) {
    if !(usd.is_finite() && usd > 0.0) {
        return;
    }
    let Ok(before) = crate::state::load_state(app) else {
        return;
    };
    let mut st = before.clone();
    roll_day(&mut st);
    st.spend_usd += usd;
    let _ = crate::board::save_with_events(app, &before, &st);
}

pub fn heal_defaults(st: &mut PersistedState) {
    if st.runner_cap == 0 {
        st.runner_cap = runner_cap_default();
    }
    st.runner_cap = clamp_runner_cap(st.runner_cap);
    if st.spend_cap_usd <= 0.0 && st.spend_cap_enabled {
        st.spend_cap_usd = spend_cap_usd_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cost_fields() {
        assert_eq!(parse_usage_usd(r#"{"total_cost": 1.5}"#), Some(1.5));
        assert_eq!(parse_usage_usd(r#"{"usd": "0.02"}"#), Some(0.02));
        assert_eq!(
            parse_usage_usd(r#"{"usage": {"cost": 3}}"#),
            Some(3.0)
        );
        assert_eq!(parse_usage_usd("not json"), None);
        assert_eq!(parse_usage_usd(r#"{"ok": true}"#), None);
    }

    #[test]
    fn runner_clamp() {
        assert_eq!(clamp_runner_cap(0), 1);
        assert_eq!(clamp_runner_cap(2), 2);
        assert_eq!(clamp_runner_cap(9), 4);
    }
}
