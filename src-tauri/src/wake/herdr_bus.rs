//! Herdr first, relay if hc is missing or the stream ends.

use std::time::Duration;

use serde_json::Value;

use super::SeatId;

pub enum BusPull {
    RelayOnly,
    Missing,
    Open(Vec<Value>),
    Ended(Vec<Value>),
}

impl BusPull {
    pub fn hc_lines(&self) -> Option<&[Value]> {
        match self {
            BusPull::Open(v) | BusPull::Ended(v) => Some(v),
            _ => None,
        }
    }

    /// Relay is always the durable record. Hc lines are extra when present.
    pub fn use_relay(&self) -> bool {
        true
    }
}

pub fn pull(timeout: Duration) -> BusPull {
    match crate::herdr::events::pull_once(timeout) {
        crate::herdr::events::Pull::Missing => BusPull::Missing,
        crate::herdr::events::Pull::Open(v) => BusPull::Open(v),
        crate::herdr::events::Pull::Ended(v) => BusPull::Ended(v),
    }
}

/// Primary delivery. A miss is not fatal: the inbox line is already the record.
pub fn deliver(seat: &SeatId, summary: &str) {
    if summary.is_empty() {
        return;
    }
    let _ = crate::herdr::client::deliver_wake(&seat.as_key(), summary);
}
