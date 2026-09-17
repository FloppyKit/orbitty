//! `orbitty.hop.v0` — packed handoff for chat hops. Code builds it.
//! The 1B does not write this JSON. Cap ~800 chars. Fail open: None
//! means today's wrap_with_pocket prose.

use serde_json::{json, Value};

pub const SCHEMA: &str = "orbitty.hop.v0";
pub const CONSUME: &str =
    "Consume orbitty.hop.v0. Answer the ask. Do not echo the JSON. If a Board snapshot is present: one kind lead sentence with the counts, then copy TODO / DOING / NEEDED titles. Do not invent status, Open tab, or what a GO means.";
pub const BOARD_LIST: &str =
    "Board snapshot is the truth. Open with one kind sentence that names the counts (Okay, Orbitty has N todos right now:). Then copy the titles under TODO, DOING, and NEEDED as given. Skip empty buckets. Do not invent status, Open tab, or extra numbered lines.";
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

/// Short is not enough. E4B has no kanban/ctl — a board/planet/todo
/// ask must hop Grok (Spark tools). Code lane pick, not a 1B harness.
/// Mint/move/delete — E4B must not write the board. Hop Grok.
pub fn needs_write_lane(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    l.contains("add a todo")
        || l.starts_with("todo:")
        || l.contains("kanban.add")
        || l.contains("move to doing")
        || l.contains("mark done")
        || l.contains("delete the card")
        || l.contains("mint a")
}

pub fn needs_tools_lane(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    l.contains("todo")
        || l.contains("kanban")
        || l.contains("board")
        || l.contains("planet")
        || l.contains("needed")
        || l.contains("orbittyctl")
        || l.contains("leftover")
        || l.contains("tell me")
        || l.contains("list the")
        || l.contains("what's on")
        || l.contains("whats on")
        || crate::companion_desk::line_needs_desk(line)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harness {
    Skip,
    Job,
    Pocket,
    Tight,
}

impl Harness {
    pub fn as_str(self) -> &'static str {
        match self {
            Harness::Skip => "skip",
            Harness::Job => "job",
            Harness::Pocket => "pocket",
            Harness::Tight => "tight",
        }
    }
}

pub fn parse_harness(s: &str) -> Option<Harness> {
    match s.trim().to_ascii_lowercase().as_str() {
        "skip" => Some(Harness::Skip),
        "job" => Some(Harness::Job),
        "pocket" => Some(Harness::Pocket),
        "tight" => Some(Harness::Tight),
        _ => None,
    }
}

fn leftover_open() -> bool {
    #[derive(serde::Deserialize, Default)]
    struct Map {
        #[serde(default)]
        items: Vec<Item>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Item {
        #[serde(default)]
        text: String,
        #[serde(default)]
        tag: String,
    }
    let p = crate::coordinator::companions_root().join("leftovers.json");
    let map: Map = std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    map.items.iter().any(|it| {
        let t = it.tag.trim().to_ascii_lowercase();
        (t == "open" || t == "keep") && !it.text.trim().is_empty()
    })
}

fn now_job_set() -> bool {
    !now_bits().0.is_empty()
}

/// Code gates. 1B may only break Job/Pocket/Tight ties on spark/work chat.
pub fn code_harness(
    mode: &str,
    verb: &str,
    line: &str,
    now_job: bool,
    pocket: bool,
    short: bool,
) -> Harness {
    if matches!(mode, "chat" | "hold") && !needs_tools_lane(line) {
        return Harness::Skip;
    }
    if verb != "chat" {
        return Harness::Skip;
    }
    if needs_write_lane(line) || needs_tools_lane(line) {
        return Harness::Tight;
    }
    if !now_job && !pocket {
        return Harness::Skip;
    }
    if !short {
        return Harness::Tight;
    }
    if now_job {
        return Harness::Job;
    }
    if pocket {
        return Harness::Pocket;
    }
    Harness::Skip
}

pub fn merge_harness(code: Harness, oneb: Option<Harness>, p: f64) -> Harness {
    if matches!(code, Harness::Skip | Harness::Tight) {
        return code;
    }
    match oneb {
        Some(h) if p >= 0.40 && matches!(h, Harness::Job | Harness::Pocket | Harness::Tight) => h,
        _ => code,
    }
}

pub fn pick_harness(
    app: &tauri::AppHandle,
    line: &str,
    verb: &str,
    oneb: Option<Harness>,
    p: f64,
    short: bool,
) -> Harness {
    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode)
        .unwrap_or_else(|| "spark".into());
    let code = code_harness(&mode, verb, line, now_job_set(), leftover_open(), short);
    merge_harness(code, oneb, p)
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
    history_on: bool,
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
    let hist = if history_on {
        history(app, line)
    } else {
        Vec::new()
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
        "history": hist,
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
    fn board_ask_needs_tools() {
        assert!(needs_tools_lane(
            "ok how about now? tell me about the todo list on planet orbitty"
        ));
        assert!(needs_tools_lane("add a todo on fkt: milk"));
        assert!(!needs_tools_lane("ok how about now?"));
        assert!(!needs_tools_lane("ping"));
        assert!(needs_write_lane("add a todo on fkt: milk"));
        assert!(!needs_write_lane(
            "tell me about the todo list on planet orbitty"
        ));
    }

    #[test]
    fn harness_code_gates() {
        assert_eq!(
            code_harness("hold", "chat", "hi", false, false, true),
            Harness::Skip
        );
        assert_eq!(
            code_harness(
                "spark",
                "chat",
                "tell me about the todo list on planet orbitty",
                true,
                false,
                true
            ),
            Harness::Tight
        );
        assert_eq!(
            code_harness("spark", "chat", "ping", true, false, true),
            Harness::Job
        );
        assert_eq!(
            merge_harness(Harness::Job, Some(Harness::Pocket), 0.7),
            Harness::Pocket
        );
        assert_eq!(
            merge_harness(Harness::Tight, Some(Harness::Skip), 0.9),
            Harness::Tight
        );
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

    #[test]
    fn board_list_asks_kind_lead() {
        assert!(BOARD_LIST.contains("kind sentence"));
        assert!(BOARD_LIST.contains("TODO"));
        assert!(!BOARD_LIST.contains("No extra story"));
        assert!(CONSUME.contains("kind lead"));
        assert!(CONSUME.contains("TODO"));
    }
}
