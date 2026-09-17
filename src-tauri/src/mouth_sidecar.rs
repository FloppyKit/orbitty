//! Loopback 1B mouth sidecar (Harsha constrained decode).
//! Not LM Studio, not Grok, not in the .app. Fail open → keywords.

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
}

pub fn splash_for(ack: &str, verb: &str) -> String {
    match ack.trim() {
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
    if out.elapsed_ms > SWAP_IN_MS {
        return None;
    }
    if out.prob < MIN_P {
        return None;
    }
    let verb = normalize_verb(&out.verb)?;
    Some(Classify {
        verb,
        ack: splash_for(&out.ack, verb),
        prob: out.prob,
        elapsed_ms: out.elapsed_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_table() {
        assert_eq!(splash_for("heard", "none"), "Heard you.");
        assert_eq!(splash_for("on_it", "chat"), "On it.");
        assert_eq!(splash_for("hand_off", "do-go"), "Handing that off.");
    }

    #[test]
    fn verb_allowlist() {
        assert_eq!(normalize_verb("CHAT"), Some("chat"));
        assert_eq!(normalize_verb("think"), None);
        assert_eq!(normalize_verb("do-go"), Some("do-go"));
    }
}
