//! Loopback 1B mouth sidecar (Harsha constrained decode).
//! Not LM Studio, not Grok, not in the .app. Fail open → keywords.

use std::sync::Mutex;

use serde::Deserialize;
use serde_json::json;

const SIDECAR: &str = "http://127.0.0.1:13121";
/// Per-send ceiling. Slower than this keeps the keyword classifier.
pub const SWAP_IN_MS: f64 = 200.0;
/// Below this P, the pick is noise — keywords own it.
pub const MIN_P: f64 = 0.40;

#[derive(Debug, Clone)]
pub struct Classify {
    pub verb: &'static str,
    pub ack: String,
    pub prob: f64,
    pub elapsed_ms: f64,
    pub harness: Option<crate::mouth_gist::Harness>,
    pub harness_prob: f64,
}

static LAST_HARNESS: Mutex<Option<(crate::mouth_gist::Harness, f64)>> = Mutex::new(None);

pub fn last_harness() -> Option<(crate::mouth_gist::Harness, f64)> {
    *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn clear_harness() {
    *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[derive(Deserialize)]
struct SidecarOut {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    verb: String,
    #[serde(default)]
    ack: String,
    #[serde(default)]
    prob: f64,
    #[serde(default)]
    elapsed_ms: f64,
    #[serde(default)]
    verb_ms: f64,
    #[serde(default)]
    harness: String,
    #[serde(default)]
    harness_prob: f64,
}

pub fn splash_for(ack: &str, verb: &str) -> String {
    match ack.trim() {
        "on_it" => "On it.".into(),
        "heard" => "Heard you.".into(),
        "one_sec" => "One sec.".into(),
        "hand_off" => "Handing that off.".into(),
        "your_call" => "That call is yours.".into(),
        "got_file" => "Got the file.".into(),
        "nope" => "Not that door.".into(),
        _ => match verb {
            "none" => "Heard you.".into(),
            _ => "On it.".into(),
        },
    }
}

fn normalize_verb(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "chat" => Some("chat"),
        "do-go" | "do_go" | "dogo" => Some("do-go"),
        "run-action" | "run_action" => Some("run-action"),
        "ask-needed" | "ask_needed" => Some("ask-needed"),
        "none" => Some("none"),
        _ => None,
    }
}

/// One classify. None = down, slow, low-P, or junk — caller uses keywords.
pub fn classify(line: &str) -> Option<Classify> {
    let body = json!({ "line": line });
    let (status, text) = crate::mouth_hop::curl_json(
        "POST",
        &format!("{SIDECAR}/classify"),
        None,
        Some(&body),
        1,
    )
    .ok()?;
    if !(200..=299).contains(&status) {
        return None;
    }
    let out: SidecarOut = serde_json::from_str(&text).ok()?;
    if !out.ok {
        return None;
    }
    let verb_ms = if out.verb_ms > 0.0 {
        out.verb_ms
    } else {
        out.elapsed_ms
    };
    if verb_ms > SWAP_IN_MS {
        return None;
    }
    if out.prob < MIN_P {
        return None;
    }
    let verb = normalize_verb(&out.verb)?;
    let harness = if out.elapsed_ms <= SWAP_IN_MS {
        crate::mouth_gist::parse_harness(&out.harness)
    } else {
        None
    };
    let harness_prob = out.harness_prob;
    if let Some(h) = harness {
        *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner()) = Some((h, harness_prob));
    }
    Some(Classify {
        verb,
        ack: splash_for(&out.ack, verb),
        prob: out.prob,
        elapsed_ms: out.elapsed_ms,
        harness,
        harness_prob,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_table() {
        assert_eq!(splash_for("on_it", "chat"), "On it.");
        assert_eq!(splash_for("heard", "none"), "Heard you.");
        assert_eq!(splash_for("one_sec", "chat"), "One sec.");
        assert_eq!(splash_for("hand_off", "do-go"), "Handing that off.");
        assert_eq!(splash_for("your_call", "ask-needed"), "That call is yours.");
        assert_eq!(splash_for("got_file", "chat"), "Got the file.");
        assert_eq!(splash_for("nope", "run-action"), "Not that door.");
    }

    #[test]
    fn verb_allowlist() {
        assert_eq!(normalize_verb("CHAT"), Some("chat"));
        assert_eq!(normalize_verb("think"), None);
        assert_eq!(normalize_verb("do-go"), Some("do-go"));
    }
}
