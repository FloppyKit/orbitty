//! `orbitty.hop.v0` — packed handoff for chat hops. Code builds it.
//! The 1B does not write this JSON. Cap ~800 chars. Fail open: None
//! means today's wrap_with_pocket prose.

use serde_json::{json, Value};

pub const SCHEMA: &str = "orbitty.hop.v0";
pub const CONSUME: &str =
    "Consume orbitty.hop.v0. Answer the ask. Do not echo the JSON.";
const GIST_CAP: usize = 800;
const ASK_CAP: usize = 200;
const GOAL_CAP: usize = 80;
const HIST_N: usize = 80;
const HIST_TURNS: usize = 6;
const CONSTRAINT_CAP: usize = 6;
/// Short-chat lane: local E4B when the ask is this small and no file.
pub const SHORT_CHAT_CHARS: usize = 160;

pub fn is_short_chat(line: &str, has_attach: bool) -> bool {
    !has_attach && line.trim().chars().count() <= SHORT_CHAT_CHARS
}

pub fn is_gist(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('{') && t.contains(SCHEMA)
}

pub fn grok_user_prompt(gist: &str) -> String {
    format!("{CONSUME}\n\n{gist}")
}

pub fn hop_system(soul: &str) -> String {
    if soul.contains(CONSUME) {
        soul.to_string()
    } else if soul.trim().is_empty() {
        CONSUME.to_string()
    } else {
        format!("{}\n\n{CONSUME}", soul.trim())
    }
}

fn clip(s: &str, n: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= n {
        t
    } else {
        t.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn ack_noise(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.starts_with("noted ")
        || low.starts_with("heard you")
        || low.starts_with("on it.")
        || low.contains("companion stays the mouth")
        || (low.starts_with("dogfood ") && low.contains("is in needed"))
}

fn history(app: &tauri::AppHandle, raw_line: &str) -> Vec<Value> {
    let turns = crate::state::load_state(app)
        .map(|s| s.companion.turns)
        .unwrap_or_default();
    let start = turns.len().saturating_sub(HIST_TURNS + 1);
    let mut rows: Vec<&crate::state::CompanionTurn> = turns[start..]
        .iter()
        .filter(|t| !ack_noise(&t.text))
        .collect();
    if rows
        .last()
        .is_some_and(|t| t.role == "you" && t.text == raw_line)
    {
        rows.pop();
    }
    rows.iter()
        .rev()
        .take(HIST_TURNS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|t| {
            json!({
                "role": if t.role == "you" { "you" } else { "them" },
                "n": clip(&t.text, HIST_N),
            })
        })
        .collect()
}

fn now_bits() -> (String, Vec<String>, Vec<String>) {
    #[derive(serde::Deserialize, Default)]
    struct Now {
        #[serde(default)]
        job: String,
        #[serde(default)]
        do_not_retry: String,
        #[serde(default)]
        files: Vec<String>,
    }
    let p = crate::coordinator::companions_root().join("now.json");
    let n: Now = std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let files: Vec<String> = n
        .files
        .iter()
        .map(|f| clip(f, 48))
        .filter(|f| !f.is_empty())
        .take(4)
        .collect();
    let mut constraints = Vec::new();
    if !n.do_not_retry.trim().is_empty() {
        constraints.push(clip(&n.do_not_retry, 40));
    }
    (clip(&n.job, GOAL_CAP), files, constraints)
}

fn encode_capped(mut v: Value) -> Option<String> {
    for _ in 0..8 {
        let s = serde_json::to_string(&v).ok()?;
        if s.chars().count() <= GIST_CAP {
            return Some(s);
        }
        if let Some(arr) = v.get_mut("history").and_then(Value::as_array_mut) {
            if !arr.is_empty() {
                arr.remove(0);
                continue;
            }
        }
        let ask = v
            .pointer("/gist/ask")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if ask.chars().count() > 40 {
            v["gist"]["ask"] = json!(clip(&ask, ask.chars().count() / 2));
            continue;
        }
        break;
    }
    let s = serde_json::to_string(&v).ok()?;
    if s.chars().count() <= GIST_CAP {
        Some(s)
    } else {
        None
    }
}

/// Compact hop packet. None = use prose wrap.
pub fn build(
    app: &tauri::AppHandle,
    line: &str,
    verb: &str,
    ack: &str,
    lane: &str,
) -> Option<String> {
    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode)
        .unwrap_or_else(|| "spark".into());
    let (job, paths, mut constraints) = now_bits();
    constraints.insert(0, clip(&mode, CONSTRAINT_CAP));
    constraints.truncate(3);
    let goal = if job.is_empty() {
        clip(&mode, GOAL_CAP)
    } else {
        job
    };
    let v = json!({
        "schema": SCHEMA,
        "verb": verb,
        "ack": clip(ack, 24),
        "gist": {
            "goal": goal,
            "constraints": constraints,
            "paths": paths,
            "ask": clip(line, ASK_CAP),
        },
        "history": history(app, line),
        "lane": lane,
    });
    encode_capped(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_chat_gate() {
        assert!(is_short_chat("ping", false));
        assert!(!is_short_chat("ping", true));
        assert!(!is_short_chat(&"x".repeat(161), false));
    }

    #[test]
    fn detect_gist() {
        assert!(is_gist(r#"{"schema":"orbitty.hop.v0","verb":"chat"}"#));
        assert!(!is_gist("[now]\nUSER:\nhi"));
    }

    #[test]
    fn cap_drops_history() {
        let hist: Vec<Value> = (0..6)
            .map(|i| json!({"role":"you","n": format!("turn {i} {}", "w".repeat(70))}))
            .collect();
        let v = json!({
            "schema": SCHEMA,
            "verb": "chat",
            "ack": "On it.",
            "gist": {
                "goal": "desk",
                "constraints": ["spark"],
                "paths": [],
                "ask": "hello there",
            },
            "history": hist,
            "lane": "local",
        });
        let s = encode_capped(v).expect("gist");
        assert!(s.chars().count() <= GIST_CAP);
        assert!(s.contains(SCHEMA));
        assert!(s.contains("hello there"));
    }

    #[test]
    fn grok_prompt_has_consume() {
        let p = grok_user_prompt(r#"{"schema":"orbitty.hop.v0"}"#);
        assert!(p.starts_with(CONSUME));
        assert!(!p.contains("[pocket]"));
    }
}
