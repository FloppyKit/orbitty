//! Path A GATE — Proof-3 CHATTER vs STOP for companion focus (product CP).
//!
//! clinical_claim: false. Reuses MiniHack Proof-3 soft/hard distinction only —
//! no MiniHack, no residual theory, no growth of parked `coordinator.rs`.
//! Hang-first = armed while the companion window has OS focus.
//!
//! Brain-matrix seat: Fast gate (`GO-ORBITTY-LOCAL-BRAIN-MATRIX`).

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// Soft vs hard interrupt (Proof-3 `InterruptKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterruptKind {
    Chatter,
    Stop,
}

/// Gate decision for one interrupt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// Soft: continue current plan / mouth; never treat as STOP.
    Continue,
    /// Hard: honor within one decision (freeze shift pickup; abort runners).
    Abort,
    /// Companion not focused — gate does not claim the interrupt.
    Disarmed,
}

#[derive(Debug, Default, Clone, Serialize)]
struct Ledger {
    chatter_held: u64,
    stop_honored: u64,
    /// Aborted on CHATTER (merged soft into hard) — must stay 0.
    false_hard: u64,
    /// Continued after STOP (merged hard into soft) — must stay 0.
    false_soft: u64,
}

static ARMED: AtomicBool = AtomicBool::new(false);
static FROZEN: AtomicBool = AtomicBool::new(false);

fn ledger() -> &'static Mutex<Ledger> {
    static L: OnceLock<Mutex<Ledger>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(Ledger::default()))
}

pub fn is_armed() -> bool {
    ARMED.load(Ordering::SeqCst)
}

pub fn is_frozen() -> bool {
    FROZEN.load(Ordering::SeqCst)
}

pub fn set_armed(on: bool) {
    ARMED.store(on, Ordering::SeqCst);
}

pub fn clear_freeze() {
    FROZEN.store(false, Ordering::SeqCst);
}

fn parse_kind(raw: &str) -> Result<InterruptKind, String> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "CHATTER" | "SOFT" | "DEFER" => Ok(InterruptKind::Chatter),
        "STOP" | "HARD" | "ABORT" => Ok(InterruptKind::Stop),
        other => Err(format!(
            "unknown interrupt kind {other:?} (want CHATTER|STOP)"
        )),
    }
}

/// Classify an interrupt under the Proof-3 gate policy.
/// CHATTER → Continue (hold); STOP → Abort + freeze. Never merge kinds.
pub fn decide(kind: InterruptKind) -> Verdict {
    if !is_armed() {
        return Verdict::Disarmed;
    }
    match kind {
        InterruptKind::Chatter => {
            if let Ok(mut g) = ledger().lock() {
                g.chatter_held = g.chatter_held.saturating_add(1);
            }
            // Soft must never freeze / abort.
            Verdict::Continue
        }
        InterruptKind::Stop => {
            FROZEN.store(true, Ordering::SeqCst);
            if let Ok(mut g) = ledger().lock() {
                g.stop_honored = g.stop_honored.saturating_add(1);
            }
            Verdict::Abort
        }
    }
}

/// Honor STOP on the product control plane: freeze shift pickup + stop
/// live runners for seated doing cards (best-effort).
pub fn honor_stop(app: &tauri::AppHandle) {
    FROZEN.store(true, Ordering::SeqCst);
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let drops: Vec<String> = st
        .agent_runs
        .iter()
        .filter(|r| matches!(r.state.as_str(), "working" | "starting" | "blocked"))
        .map(|r| r.card_id.clone())
        .collect();
    if !drops.is_empty() {
        crate::agent_stop::stop_resets(app, &st, &drops);
    }
    crate::event_log::append_best(
        app,
        "interrupt-gate",
        None,
        None,
        Some("control"),
        true,
        &serde_json::json!({
            "kind": "STOP",
            "verdict": "ABORT",
            "frozen": true,
            "drops": drops,
        }),
    );
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateStatus {
    pub armed: bool,
    pub frozen: bool,
    pub clinical_claim: bool,
    pub chatter_held: u64,
    pub stop_honored: u64,
    pub false_hard: u64,
    pub false_soft: u64,
}

#[tauri::command]
pub fn interrupt_gate_set_armed(armed: bool) -> GateStatus {
    set_armed(armed);
    // Losing focus clears freeze so the next Hang session starts clean.
    if !armed {
        clear_freeze();
    }
    status_inner()
}

#[tauri::command]
pub fn interrupt_gate_status() -> GateStatus {
    status_inner()
}

#[tauri::command]
pub fn interrupt_gate_clear_freeze() -> GateStatus {
    clear_freeze();
    status_inner()
}

/// Inject + classify (dogfood / orbittyctl). CHATTER never aborts; STOP does.
#[tauri::command]
pub fn interrupt_gate_inject(
    app: tauri::AppHandle,
    kind: String,
) -> Result<serde_json::Value, String> {
    let k = parse_kind(&kind)?;
    let verdict = decide(k);
    if verdict == Verdict::Abort {
        honor_stop(&app);
    }
    Ok(serde_json::json!({
        "kind": k,
        "verdict": verdict,
        "status": status_inner(),
    }))
}

fn status_inner() -> GateStatus {
    let g = ledger().lock().ok();
    GateStatus {
        armed: is_armed(),
        frozen: is_frozen(),
        clinical_claim: false,
        chatter_held: g.as_ref().map(|x| x.chatter_held).unwrap_or(0),
        stop_honored: g.as_ref().map(|x| x.stop_honored).unwrap_or(0),
        false_hard: g.as_ref().map(|x| x.false_hard).unwrap_or(0),
        false_soft: g.as_ref().map(|x| x.false_soft).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Statics are process-global — serialize unit tests.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static M: Mutex<()> = Mutex::new(());
        M.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn gate_policy_chatter_stop_disarmed() {
        let _g = lock();
        set_armed(true);
        clear_freeze();
        assert_eq!(decide(InterruptKind::Chatter), Verdict::Continue);
        assert!(!is_frozen());

        assert_eq!(decide(InterruptKind::Stop), Verdict::Abort);
        assert!(is_frozen());
        clear_freeze();

        set_armed(false);
        assert_eq!(decide(InterruptKind::Stop), Verdict::Disarmed);
        assert!(!is_frozen());
    }
}
