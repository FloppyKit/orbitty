//! Needed standing rules: read / classify /
//! match / can-run against `~/.buzz/ops/needed-bus/rules.json` (the STORE
//! stage's file). Lock: `~/.buzz/PLANS/ORBITTY_NEEDED_RULES_LOCK_v0.md`.
//!
//! Rules are ALWAYS user-minted (Matt) — this module never writes one.
//! `board::needed_open` consults `decide()` before minting, so ctl, MCP,
//! and the python wrapper all ride the same door (MCP cannot bypass):
//!   0 hits / collide / cannot-run / unreadable store → `Decision::Open`
//!     (mint `open` as today — Matt gets pinged)
//!   1 hit (or many with the SAME answer) + can-run → `Decision::Apply` —
//!     the card is born `answered` with `actor: "rule"` in the same save
//!     and `needed-answer` is emitted, so the existing watch → nest clerk
//!     / writer dispatch acts as if Matt typed it. No walker, no
//!     NEXT-CARD (neededState is never `open`), no companion send.
//! Fail-open is the lock: empty answer, writer seat + missing prompt file,
//! writer seat + issue already running/shipped, unreadable JSON → ping.
//! Match is the small AND param list (unspecified = wildcard) — no model.
//! Log: append-only `rules.log` JSONL (titles only, no PHI).
//!
//! `rule_add` is the Answer chrome's Save
//! ("if this again" row). Still no writes from Rust — it shells out to
//! `orbitty-needed rule add`, so the python CLI keeps owning rules.json
//! (validation, the 48 cap, atomic save, log). This builds argv (no
//! shell) and hands back the minted rule or the CLI's refusal verbatim.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::state::Project;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn bus_dir() -> PathBuf {
    home().join(".buzz/ops/needed-bus")
}

fn rules_path() -> PathBuf {
    bus_dir().join("rules.json")
}

fn rules_log_path() -> PathBuf {
    bus_dir().join("rules.log")
}

fn issued_path() -> PathBuf {
    bus_dir().join("issued.json")
}

fn plans_dir() -> PathBuf {
    home().join(".buzz/PLANS")
}

fn utc_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The gate's answer for one `needed.open`.
pub enum Decision {
    /// Miss / collide / cannot-run / unreadable store → mint `open` (ping).
    Open,
    /// One logical hit + can-run → the standing answer; the card is born
    /// answered with `actor: "rule"`.
    Apply(String),
}

/// The would-be card, classified from its title (lock table, no model).
pub struct Classified {
    pub title: String,
    pub flavor: &'static str, // "dogfood" | "writer" | "writer-leftover" | "fact"
    pub go: Option<String>,   // GO-… token from the title, else the why's `go=`
    pub prompt_kind: Option<&'static str>, // "first" | "leftover"
    pub project: String,      // project id as passed ("" = inbox)
}

/// The bus's GO token: `\b(GO-[A-Z0-9][-A-Z0-9]*)\b` on the uppercased,
/// de-questioned text. Hand-rolled — no regex dep for one token.
pub fn extract_go(text: &str) -> Option<String> {
    let up = text.to_uppercase().replace('?', " ");
    let b = up.as_bytes();
    let mut i = 0;
    while i + 3 < b.len() {
        let boundary = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if boundary && &b[i..i + 3] == b"GO-" && b[i + 3].is_ascii_alphanumeric() {
            let mut j = i + 3;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-') {
                j += 1;
            }
            while j > i + 3 && b[j - 1] == b'-' {
                j -= 1; // a trailing '-' is not a word char — the regex's \b drops it
            }
            return Some(up[i..j].to_string());
        }
        i += 1;
    }
    None
}

/// First `key=value` token in a doing-fire why line
/// (`card=… go=GO-… planet=…`) — the fact title `Who runs this doing GO?`
/// carries no GO token, so the why is where `go` matches.
fn why_value<'a>(why: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    why.split_whitespace()
        .find_map(|tok| tok.strip_prefix(prefix.as_str()))
        .filter(|v| !v.is_empty())
}

/// Title → flavor / go / prompt_kind, per the lock table. `go` falls back
/// to the why's `go=` when the title has no token (doing-fire fact).
pub fn classify(title: &str, project: &str, why: &str) -> Classified {
    let t = title.trim().to_string();
    let low = t.to_lowercase();
    let go = extract_go(&t).or_else(|| why_value(why, "go").map(str::to_uppercase));
    let mut flavor = "fact";
    let mut prompt_kind = None;
    if t.starts_with("Dogfood ") && go.is_some() {
        flavor = "dogfood";
        if low.contains("leftover") {
            prompt_kind = Some("leftover");
        }
    } else if t.starts_with("Writer for ") && t.ends_with('?') {
        if low.contains("leftover") {
            flavor = "writer-leftover";
            prompt_kind = Some("leftover");
        } else {
            flavor = "writer";
            prompt_kind = Some("first");
        }
    }
    Classified {
        title: t,
        flavor,
        go,
        prompt_kind,
        project: project.to_string(),
    }
}

// ---------- store (read-only here; writes are the python CLI's job) ----------

#[derive(Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub params: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub when: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub answer: String,
}

fn default_enabled() -> bool {
    true
}

/// Missing file = empty store (Ok). Present but unreadable / malformed =
/// Err — the gate fails open to a ping.
fn load_rules() -> Result<Vec<Rule>, String> {
    let p = rules_path();
    if !p.is_file() {
        return Ok(Vec::new());
    }
    let raw =
        std::fs::read_to_string(&p).map_err(|e| format!("rules store unreadable: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("rules store unreadable: {e}"))?;
    if !v.is_object() || !v.get("rules").is_some_and(|r| r.is_array()) {
        return Err("rules store malformed: want {version, rules: []}".into());
    }
    #[derive(Deserialize)]
    struct Store {
        rules: Vec<Rule>,
    }
    let store: Store =
        serde_json::from_value(v).map_err(|e| format!("rules store unreadable: {e}"))?;
    Ok(store.rules)
}

/// Append-only JSONL, one object per line. Best-effort: a dead log never
/// fails an open. No PHI — titles only.
fn log_event(ev: serde_json::Value) {
    let mut rec = serde_json::Map::new();
    rec.insert("ts".into(), serde_json::Value::String(utc_now()));
    if let serde_json::Value::Object(m) = ev {
        rec.extend(m);
    }
    let line = serde_json::Value::Object(rec).to_string();
    let _ = std::fs::create_dir_all(bus_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rules_log_path())
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

// ---------- when.issued (same read as the bus `issued` check) ----------

/// `os.kill(pid, 0)` semantics without a libc dep.
fn pid_alive(pid: i64) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// shipped | running | absent for this GO + this prompt. A new leftover
/// prompt file is `absent` even if an older one on the same GO shipped.
pub fn issued_state(go: &str, prompt: &str) -> String {
    if go.is_empty() {
        return "absent".into();
    }
    let p = issued_path();
    if !p.is_file() {
        return "absent".into();
    }
    let raw = match std::fs::read_to_string(&p) {
        Ok(r) => r,
        Err(_) => return "absent".into(),
    };
    let d: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return "absent".into(),
    };
    let Some(rec) = d.get(go.to_uppercase()) else {
        return "absent".into();
    };
    if !rec.is_object() {
        return "absent".into();
    }
    let rec_prompt = rec.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    if !prompt.is_empty() && !rec_prompt.is_empty() && rec_prompt != prompt {
        return "absent".into();
    }
    if rec.get("shipped").and_then(|v| v.as_bool()).unwrap_or(false) {
        return "shipped".into();
    }
    let pid = rec.get("pid").and_then(|v| v.as_i64()).unwrap_or(0);
    if pid > 0 && pid_alive(pid) {
        return "running".into();
    }
    "absent".into()
}

// ---------- prompt resolution (mirrors the python wrapper) ----------

fn expand_home(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        return home().join(rest);
    }
    PathBuf::from(raw)
}

/// The packet's `prompt` path if it names a real file.
fn prompt_from_packet(pkt: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(pkt).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let p = v.get("prompt").and_then(|x| x.as_str()).unwrap_or("");
    if p.is_empty() {
        return None;
    }
    let p = expand_home(p);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Newest `launch-{go}-leftover*.json` prompt, else newest
/// `_{GO}_LEFTOVER*_PROMPT.md` in PLANS.
pub fn leftover_prompt_for(go: &str) -> Option<PathBuf> {
    let prefix = format!("launch-{go}-leftover");
    let mut cands: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(bus_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && name.ends_with(".json") {
                if let Some(p) = prompt_from_packet(&e.path()) {
                    let mt = e
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    cands.push((mt, p));
                }
            }
        }
    }
    cands.sort_by_key(|(mt, _)| *mt);
    if let Some((_, p)) = cands.pop() {
        return Some(p);
    }
    newest_plans_prompt(&format!("_{}_LEFTOVER", go.replace('-', "_")))
}

/// `launch-{go}.json` prompt, else `_{GO}_PROMPT.md` in PLANS.
pub fn first_prompt_for(go: &str) -> Option<PathBuf> {
    let pkt = bus_dir().join(format!("launch-{go}.json"));
    if let Some(p) = prompt_from_packet(&pkt) {
        return Some(p);
    }
    let p = plans_dir().join(format!("_{}_PROMPT.md", go.replace('-', "_")));
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Newest PLANS file starting with `slug` and ending `_PROMPT.md`.
fn newest_plans_prompt(slug: &str) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(plans_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(slug) && name.ends_with("_PROMPT.md") {
                let mt = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(bmt, _)| mt > *bmt) {
                    best = Some((mt, e.path()));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

// ---------- match ----------

/// Lock: `project` matches the planet id OR the exact name.
fn project_matches(rule_value: &str, card_project: &str, projects: &[Project]) -> bool {
    if card_project.is_empty() {
        return false;
    }
    if rule_value == card_project {
        return true;
    }
    projects
        .iter()
        .any(|p| p.id == card_project && p.name.trim() == rule_value)
}

/// `go` param: a trailing `*` is a prefix match (`GO-ORBITTY*`); without
/// the star it stays exact. Case-insensitive either way.
fn go_param_matches(rule_value: &str, card_go: &str) -> bool {
    if card_go.is_empty() {
        return false;
    }
    match rule_value.strip_suffix('*') {
        Some(prefix) => {
            !prefix.is_empty()
                && card_go.len() >= prefix.len()
                && card_go[..prefix.len()].eq_ignore_ascii_case(prefix)
        }
        None => card_go.eq_ignore_ascii_case(rule_value),
    }
}

/// The GO a rule acts on for `when.issued` / can-run prompt lookups: the
/// param, unless it is a prefix pattern — `GO-ORBITTY*` itself names no
/// prompt file, so the card's concrete token is the lookup key.
fn rule_go(rule: &Rule, card: &Classified) -> String {
    match rule.params.get("go") {
        Some(g) if !g.ends_with('*') => g.clone(),
        _ => card.go.clone().unwrap_or_default(),
    }
}

/// AND: every specified param must match; omitted = wildcard. Every
/// specified `when` must hold. Unknown keys never match (writes refuse
/// them; a hand-edit fails closed instead of silently widening).
fn rule_matches(rule: &Rule, card: &Classified, projects: &[Project]) -> bool {
    for (k, v) in &rule.params {
        let hit = match k.as_str() {
            "flavor" => card.flavor == v,
            "go" => go_param_matches(v, card.go.as_deref().unwrap_or("")),
            "prompt_kind" => card.prompt_kind == Some(v.as_str()),
            "project" => project_matches(v, &card.project, projects),
            "title_prefix" => card.title.starts_with(v.as_str()),
            "title_exact" => card.title == *v,
            _ => false,
        };
        if !hit {
            return false;
        }
    }
    if let Some(when) = &rule.when {
        if let Some(want) = when.get("issued") {
            let go = rule_go(rule, card);
            let kind = rule
                .params
                .get("prompt_kind")
                .map(String::as_str)
                .or(card.prompt_kind);
            let prompt = prompt_for(&go, kind);
            let prompt_s = prompt
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if issued_state(&go, &prompt_s) != *want {
                return false;
            }
        }
    }
    true
}

fn prompt_for(go: &str, kind: Option<&str>) -> Option<PathBuf> {
    if go.is_empty() {
        return None;
    }
    if kind == Some("leftover") {
        leftover_prompt_for(go)
    } else {
        first_prompt_for(go)
    }
}

// ---------- can-run (fail-open list from the lock) ----------

/// Same seat words as the bus's `seat_from_answer`.
fn seat_of(answer: &str) -> Option<&'static str> {
    let a = answer.trim().to_lowercase();
    if a.starts_with("cursor") || a == "c" || a == "ultra" || a == "kimi" {
        return Some("cursor");
    }
    if a.starts_with("grok") {
        return Some("grok");
    }
    if a.starts_with("wait") || a == "later" || a == "not now" {
        return Some("wait");
    }
    None
}

/// If any of these hold, the gate must NOT apply — mint `open` (ping):
/// empty answer; writer flavor + cursor/grok seat + missing prompt file;
/// writer flavor + cursor/grok seat + issue already running/shipped for
/// that prompt (a second launch would lie — checked even when the rule
/// did not set `when.issued`).
fn can_run(rule: &Rule, card: &Classified) -> Result<(), String> {
    let answer = rule.answer.trim();
    if answer.is_empty() {
        return Err("empty answer".into());
    }
    let writerish = card.flavor == "writer" || card.flavor == "writer-leftover";
    let seat = seat_of(answer);
    if writerish && matches!(seat, Some("cursor") | Some("grok")) {
        let go = rule_go(rule, card);
        let kind = rule
            .params
            .get("prompt_kind")
            .map(String::as_str)
            .or(card.prompt_kind);
        let Some(prompt) = prompt_for(&go, kind) else {
            return Err(format!(
                "writer prompt missing for {go} — a {} seat would launch blind",
                seat.unwrap_or("?")
            ));
        };
        let state = issued_state(&go, &prompt.to_string_lossy());
        if state == "running" || state == "shipped" {
            return Err(format!(
                "issued already {state} for {go} — a second launch would lie"
            ));
        }
    }
    Ok(())
}

// ---------- the gate ----------

/// Classify + match + can-run for one `needed.open`. Logs the outcome
/// (hit applied true/false, collide, miss) to `rules.log`. `why` is the
/// card's machine-parseable why line — a fact title with no GO token
/// (doing-fire) matches `go` against the why's `go=`.
pub fn decide(title: &str, project_id: &str, why: &str, projects: &[Project]) -> Decision {
    let card = classify(title, project_id, why);
    let rules = match load_rules() {
        Ok(r) => r,
        Err(e) => {
            log_event(serde_json::json!({
                "event": "miss",
                "title": card.title,
                "reason": format!("store-unreadable: {e}"),
            }));
            return Decision::Open;
        }
    };
    let hits: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.enabled && rule_matches(r, &card, projects))
        .collect();
    if hits.is_empty() {
        log_event(serde_json::json!({ "event": "miss", "title": card.title }));
        return Decision::Open;
    }
    let answers: std::collections::HashSet<&str> =
        hits.iter().map(|h| h.answer.trim()).collect();
    if answers.len() > 1 {
        // Collide — log it and ping. Never pick a winner.
        log_event(serde_json::json!({
            "event": "collide",
            "title": card.title,
            "rules": hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        }));
        return Decision::Open;
    }
    let answer = hits[0].answer.trim().to_string();
    match can_run(hits[0], &card) {
        Ok(()) => {
            for h in &hits {
                log_event(serde_json::json!({
                    "event": "hit",
                    "rule": h.id,
                    "title": card.title,
                    "applied": true,
                }));
            }
            Decision::Apply(answer)
        }
        Err(reason) => {
            for h in &hits {
                log_event(serde_json::json!({
                    "event": "hit",
                    "rule": h.id,
                    "title": card.title,
                    "applied": false,
                    "reason": reason,
                }));
            }
            Decision::Open
        }
    }
}

// ---------- The Answer chrome's Save ----------

/// `~/.local/bin/orbitty-needed` (a symlink into the scripts repo).
fn cli_path() -> PathBuf {
    home().join(".local/bin/orbitty-needed")
}

/// argv for `orbitty-needed rule add`: every param is one `--param k=v`
/// (an `issued=` pair rides the same flag — the CLI sorts it into `when`).
/// No shell, so spaces in title_prefix/title_exact/answer stay one arg.
fn rule_add_argv(
    params: &[(String, String)],
    answer: &str,
    from_needed: Option<&str>,
) -> Vec<String> {
    let mut argv = vec!["rule".to_string(), "add".to_string()];
    for (k, v) in params {
        argv.push("--param".to_string());
        argv.push(format!("{k}={v}"));
    }
    argv.push("--answer".to_string());
    argv.push(answer.to_string());
    if let Some(id) = from_needed {
        argv.push("--from-needed".to_string());
        argv.push(id.to_string());
    }
    argv
}

/// Thin door for the "if this again" row. The write itself stays the
/// python CLI's job — this never opens rules.json. Returns the minted
/// rule JSON; a CLI refusal (unknown key, cap full, issued without go,
/// empty answer) comes back verbatim as Err.
pub fn rule_add(
    params: &[(String, String)],
    answer: &str,
    from_needed: Option<&str>,
) -> Result<serde_json::Value, String> {
    let cli = cli_path();
    let out = std::process::Command::new(&cli)
        .args(rule_add_argv(params, answer, from_needed))
        .output()
        .map_err(|e| format!("rule add: cannot run {}: {e}", cli.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("rule add: exit {}", out.status)
        } else {
            err
        });
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    serde_json::from_str(&text).map_err(|e| format!("rule add: bad JSON from CLI: {e}"))
}

const PARAM_KEYS: &[&str] = &[
    "flavor",
    "go",
    "prompt_kind",
    "project",
    "title_prefix",
    "title_exact",
];

/// Leading `Rule:` / `RULE:` on a composer answer. Returns the dispatch
/// text (Rule: stripped). If the line taught a rule, it is saved here.
/// Teaching is user-minted only — callers never invent a line.
pub fn apply_composer_rule(card: &crate::state::KanbanCard, raw: &str) -> String {
    let t = raw.trim();
    let rest = match t.split_once(':') {
        Some((h, r)) if h.trim().eq_ignore_ascii_case("rule") => r.trim(),
        _ => return t.to_string(),
    };
    if rest.is_empty() {
        return String::new();
    }
    if first_tok_is_param(rest) {
        let mut lines = rest.lines();
        let first = lines.next().unwrap_or("").trim();
        let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        let mut params: Vec<(String, String)> = Vec::new();
        for tok in first.split_whitespace() {
            if let Some((k, v)) = tok.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim();
                if !k.is_empty() && !v.is_empty() {
                    params.push((k, v.to_string()));
                }
            }
        }
        let answer = if body.is_empty() {
            String::new()
        } else {
            body
        };
        if !answer.is_empty() && !params.is_empty() {
            let _ = rule_add(&params, &answer, Some(card.id.as_str()));
        }
        return answer;
    }
    teach_from_composer(card, rest);
    rest.to_string()
}

fn first_tok_is_param(s: &str) -> bool {
    let tok = s.split_whitespace().next().unwrap_or("");
    let Some((k, _)) = tok.split_once('=') else {
        return false;
    };
    let k = k.trim().to_ascii_lowercase();
    PARAM_KEYS.iter().any(|p| *p == k)
}

fn teach_from_composer(card: &crate::state::KanbanCard, rest: &str) {
    let rest = rest.trim();
    if rest.is_empty() {
        return;
    }
    let (params, answer) = if first_tok_is_param(rest) {
        let mut lines = rest.lines();
        let first = lines.next().unwrap_or("").trim();
        let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        let mut params: Vec<(String, String)> = Vec::new();
        for tok in first.split_whitespace() {
            if let Some((k, v)) = tok.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim();
                if !k.is_empty() && !v.is_empty() {
                    params.push((k, v.to_string()));
                }
            }
        }
        let answer = if body.is_empty() {
            String::new()
        } else {
            body
        };
        (params, answer)
    } else {
        let cls = classify(
            card.title.as_str(),
            card.project_id.as_str(),
            card.needed_why.as_deref().unwrap_or(""),
        );
        let mut params = vec![("flavor".into(), cls.flavor.to_string())];
        if let Some(go) = cls.go {
            params.push(("go".into(), go));
        }
        if let Some(pk) = cls.prompt_kind {
            params.push(("prompt_kind".into(), pk.to_string()));
        }
        if !cls.project.is_empty() {
            params.push(("project".into(), cls.project));
        }
        (params, rest.to_string())
    };
    if answer.trim().is_empty() || params.is_empty() {
        return;
    }
    let _ = rule_add(&params, answer.trim(), Some(card.id.as_str()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_rule_passthrough() {
        let c = crate::state::KanbanCard {
            id: "n-1".into(),
            title: "Who runs this doing GO?".into(),
            project_id: "p-1".into(),
            window_id: None,
            kind: Some("needed".into()),
            status: "todo".into(),
            created_at: 0.0,
            last_touched_at: 0.0,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: None,
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: Some("open".into()),
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
        };
        assert_eq!(apply_composer_rule(&c, "cursor on this planet"), "cursor on this planet");
        assert_eq!(apply_composer_rule(&c, "Rule:"), "");
    }

    #[test]
    fn go_token_from_title() {
        assert_eq!(
            extract_go("Writer for GO-ORBITTY-HERDING-CATS leftover?"),
            Some("GO-ORBITTY-HERDING-CATS".to_string())
        );
        assert_eq!(extract_go("Dogfood GO-X1?"), Some("GO-X1".to_string()));
        assert_eq!(extract_go("no token here"), None);
        assert_eq!(extract_go("EGO-NOPE"), None); // not a word boundary
    }

    #[test]
    fn classify_table() {
        let c = classify("Dogfood GO-ORBITTY-X?", "", "");
        assert_eq!(c.flavor, "dogfood");
        assert_eq!(c.prompt_kind, None);
        let c = classify("Dogfood GO-ORBITTY-X leftover?", "", "");
        assert_eq!(c.flavor, "dogfood");
        assert_eq!(c.prompt_kind, Some("leftover"));
        let c = classify("Writer for GO-ORBITTY-X leftover?", "", "");
        assert_eq!(c.flavor, "writer-leftover");
        assert_eq!(c.prompt_kind, Some("leftover"));
        assert_eq!(c.go.as_deref(), Some("GO-ORBITTY-X"));
        let c = classify("Writer for GO-ORBITTY-X?", "", "");
        assert_eq!(c.flavor, "writer");
        assert_eq!(c.prompt_kind, Some("first"));
        let c = classify("Rules STORE dogfood: fact ping?", "p-1", "");
        assert_eq!(c.flavor, "fact");
        assert_eq!(c.prompt_kind, None);
        assert_eq!(c.project, "p-1");
    }

    #[test]
    fn doing_fire_fact_go_from_why() {
        // The one fact title has no GO token — the why's `go=` is the match.
        let c = classify(
            "Who runs this doing GO?",
            "p-orb",
            "card=c-1 go=GO-ORBITTY-DOING-FIRE planet=orbitty",
        );
        assert_eq!(c.flavor, "fact");
        assert_eq!(c.go.as_deref(), Some("GO-ORBITTY-DOING-FIRE"));
        // A title token still wins over the why.
        let c = classify(
            "Writer for GO-ORBITTY-X?",
            "",
            "card=c-2 go=GO-OTHER planet=orbitty",
        );
        assert_eq!(c.go.as_deref(), Some("GO-ORBITTY-X"));
        // No token anywhere → no go.
        let c = classify("Who runs this doing GO?", "p-orb", "card=c-3 planet=orbitty");
        assert_eq!(c.go, None);
    }

    #[test]
    fn go_prefix_star() {
        assert!(go_param_matches("GO-ORBITTY*", "GO-ORBITTY-DOING-FIRE"));
        assert!(go_param_matches("go-orbitty*", "GO-ORBITTY-X")); // case-insensitive
        assert!(!go_param_matches("GO-ORBITTY*", "GO-RGL-X"));
        assert!(!go_param_matches("GO-ORBITTY*", "GO-ORBIT")); // shorter than prefix
        assert!(!go_param_matches("GO-ORBITTY*", "")); // no token never matches
        assert!(!go_param_matches("*", "GO-ORBITTY-X")); // bare star is not a param
        // No star = exact, as before.
        assert!(go_param_matches("GO-ORBITTY-X", "go-orbitty-x"));
        assert!(!go_param_matches("GO-ORBITTY-X", "GO-ORBITTY-XY"));
    }

    #[test]
    fn rule_star_matches_doing_fire_card() {
        let rule = Rule {
            id: "r-1".into(),
            enabled: true,
            params: [
                ("flavor".to_string(), "fact".to_string()),
                ("go".to_string(), "GO-ORBITTY*".to_string()),
                ("project".to_string(), "orbitty".to_string()),
            ]
            .into_iter()
            .collect(),
            when: None,
            answer: "cursor on this planet".into(),
        };
        let projects = vec![Project {
            id: "p-orb".into(),
            name: "orbitty".into(),
        }];
        let hit = classify(
            "Who runs this doing GO?",
            "p-orb",
            "card=c-1 go=GO-ORBITTY-DOING-FIRE planet=orbitty",
        );
        assert!(rule_matches(&rule, &hit, &projects));
        // A GO-* that is not GO-ORBITTY* misses.
        let other_go = classify(
            "Who runs this doing GO?",
            "p-orb",
            "card=c-2 go=GO-RGL-X planet=orbitty",
        );
        assert!(!rule_matches(&rule, &other_go, &projects));
        // The same fire on another planet misses.
        let other_planet = classify(
            "Who runs this doing GO?",
            "p-rgl",
            "card=c-3 go=GO-ORBITTY-X planet=rgl",
        );
        assert!(!rule_matches(&rule, &other_planet, &projects));
        // title_exact still works alongside the star param.
        let titled = Rule {
            id: "r-2".into(),
            enabled: true,
            params: [
                ("flavor".to_string(), "fact".to_string()),
                (
                    "title_exact".to_string(),
                    "Who runs this doing GO?".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            when: None,
            answer: "cursor on this planet".into(),
        };
        assert!(rule_matches(&titled, &other_planet, &projects));
    }

    #[test]
    fn seats() {
        assert_eq!(seat_of("cursor"), Some("cursor"));
        assert_eq!(seat_of("Cursor Ultra kimi-k3-max"), Some("cursor"));
        assert_eq!(seat_of("grok"), Some("grok"));
        assert_eq!(seat_of("wait"), Some("wait"));
        assert_eq!(seat_of("already shipped"), None);
        assert_eq!(seat_of("green"), None);
    }

    #[test]
    fn rule_add_argv_shape() {
        // One --param k=v per pair, issued rides --param, answer last-ish,
        // spaces never split (title_exact slurps are a CLI-side concern).
        let argv = rule_add_argv(
            &[
                ("flavor".to_string(), "fact".to_string()),
                ("go".to_string(), "GO-ORBITTY*".to_string()),
                ("issued".to_string(), "shipped".to_string()),
                (
                    "title_exact".to_string(),
                    "Who runs this doing GO?".to_string(),
                ),
            ],
            "cursor on this planet",
            Some("n-abc"),
        );
        assert_eq!(
            argv,
            vec![
                "rule",
                "add",
                "--param",
                "flavor=fact",
                "--param",
                "go=GO-ORBITTY*",
                "--param",
                "issued=shipped",
                "--param",
                "title_exact=Who runs this doing GO?",
                "--answer",
                "cursor on this planet",
                "--from-needed",
                "n-abc",
            ]
        );
        // No from-needed → the flag is absent, not empty.
        let argv = rule_add_argv(&[], "green", None);
        assert_eq!(argv, vec!["rule", "add", "--answer", "green"]);
    }
}
