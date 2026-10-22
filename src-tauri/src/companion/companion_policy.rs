//! Go policy intercept. Questions come from `go.runner_get`. Asks to change
//! what agents may do are refused. Switching to an approved runner is the
//! only write. The mouth is never called.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyAsk {
    Question,
    Change,
    /// Empty id means `reset the go runner`.
    Switch(String),
}

pub fn classify(line: &str) -> Option<PolicyAsk> {
    let tokens = tokenize(line);
    if tokens.is_empty() {
        return None;
    }
    if let Some(sw) = parse_switch(&tokens) {
        return Some(sw);
    }
    if is_change(&tokens) {
        return Some(PolicyAsk::Change);
    }
    if is_question(&tokens) {
        return Some(PolicyAsk::Question);
    }
    None
}

pub fn reply(ask: &PolicyAsk, value: &Value) -> String {
    match ask {
        PolicyAsk::Question => question_reply(value),
        PolicyAsk::Change | PolicyAsk::Switch(_) => change_reply(value),
    }
}

/// Pure of the mouth: `mouth` is accepted so a test can prove it was not called.
pub fn handle(
    line: &str,
    roots: &crate::plans::Roots,
    mouth: &mut dyn FnMut(),
) -> Option<Result<String, String>> {
    let ask = classify(line)?;
    let _sink = mouth;
    match ask {
        PolicyAsk::Switch(id) if id.is_empty() => Some(apply_switch(roots, None)),
        PolicyAsk::Switch(id) => Some(apply_switch(roots, Some(id))),
        other => {
            let v = crate::plans::runner_get_value(roots);
            Some(Ok(reply(&other, &v)))
        }
    }
}

pub fn intercept(_app: &tauri::AppHandle, line: &str) -> Option<Result<String, String>> {
    let mut mouth = || {};
    handle(line, &crate::plans::Roots::real(), &mut mouth)
}

fn apply_switch(roots: &crate::plans::Roots, id: Option<String>) -> Result<String, String> {
    let args = match &id {
        None => serde_json::json!({ "reset": true }),
        Some(id) => serde_json::json!({ "runner": id }),
    };
    match crate::plans::apply_runner_set(roots, &args) {
        Ok(v) => {
            let id = v
                .get("runner")
                .and_then(|x| x.as_str())
                .unwrap_or("pi-grok");
            Ok(format!("Go runner is now {id} (from the next run)."))
        }
        Err(e) => {
            let cur = crate::plans::runner_get_value(roots);
            Ok(format!("{} {e}", change_reply(&cur)))
        }
    }
}

fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in line.chars() {
        if c.is_whitespace() || matches!(c, ',' | '?' | '!' | ';' | ':' | '(' | ')' | '"' | '“' | '”') {
            push_tok(&mut out, &mut cur);
        } else {
            cur.push(c);
        }
    }
    push_tok(&mut out, &mut cur);
    out
}

fn push_tok(out: &mut Vec<String>, cur: &mut String) {
    if cur.is_empty() {
        return;
    }
    let raw = std::mem::take(cur);
    let mut t = raw.to_ascii_lowercase();
    if let Some(stem) = t.strip_suffix("'s").or_else(|| t.strip_suffix("’s")) {
        t = stem.to_string();
    }
    if t.ends_with('.') && !t.contains('/') {
        t.pop();
    }
    if !t.is_empty() {
        out.push(t);
    }
}

fn has_go_context(tokens: &[String]) -> bool {
    const EXPLICIT: &[&str] = &[
        "push_allowed",
        "git_push",
        "rules.yaml",
        "go-runner",
        "cook",
        "runner",
    ];
    if tokens.iter().any(|t| EXPLICIT.contains(&t.as_str())) {
        return true;
    }
    if tokens_join(tokens).contains("developer mode") {
        return true;
    }
    for (i, t) in tokens.iter().enumerate() {
        if t != "go" {
            continue;
        }
        let next = tokens.get(i + 1).map(|s| s.as_str()).unwrap_or("");
        let prev = if i == 0 { "" } else { tokens[i - 1].as_str() };
        if matches!(next, "ahead" | "to" | "home" | "back") {
            continue;
        }
        if matches!(prev, "let's" | "lets" | "let’s") {
            continue;
        }
        return true;
    }
    false
}

fn tokens_join(tokens: &[String]) -> String {
    tokens.join(" ")
}

fn parse_switch(tokens: &[String]) -> Option<PolicyAsk> {
    let joined = tokens_join(tokens);
    if joined.contains("reset the go runner") {
        return Some(PolicyAsk::Switch(String::new()));
    }
    let key = "go runner to ";
    let idx = joined.find(key)?;
    let before = joined[..idx].trim();
    let words: Vec<&str> = before.split_whitespace().collect();
    let verb = if words.last() == Some(&"the") {
        words.get(words.len().saturating_sub(2)).copied().unwrap_or("")
    } else {
        words.last().copied().unwrap_or("")
    };
    if !matches!(verb, "switch" | "set" | "use" | "change") {
        return None;
    }
    let id = joined[idx + key.len()..].split_whitespace().next()?;
    if id.contains('/') || id.starts_with('~') || id.ends_with(".sh") {
        return None;
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(PolicyAsk::Switch(id.to_string()))
}

fn is_change(tokens: &[String]) -> bool {
    if path_clause(tokens) {
        return true;
    }
    has_go_context(tokens) && has_change_verb(tokens) && has_change_object(tokens)
}

fn has_change_verb(tokens: &[String]) -> bool {
    let joined = tokens_join(tokens);
    if ["turn on", "turn off", "switch on", "switch off"]
        .iter()
        .any(|p| joined.contains(p))
    {
        return true;
    }
    const VERBS: &[&str] = &[
        "allow", "enable", "let", "give", "grant", "permit", "unlock", "approve", "flip", "set",
        "add", "remove", "disable", "revoke",
    ];
    tokens.iter().any(|t| VERBS.contains(&t.as_str()))
}

fn has_change_object(tokens: &[String]) -> bool {
    let joined = tokens_join(tokens);
    if [
        "developer mode",
        "rules.yaml",
        "push_allowed",
        "git_push",
        "full-auto",
        "workspace-write",
    ]
    .iter()
    .any(|p| joined.contains(p))
    {
        return true;
    }
    const OBJS: &[&str] = &[
        "push",
        "merge",
        "main",
        "stable",
        "install",
        "send",
        "permission",
        "permissions",
    ];
    tokens.iter().any(|t| OBJS.contains(&t.as_str()))
}

fn path_clause(tokens: &[String]) -> bool {
    const VERBS: &[&str] = &["use", "set", "make", "add", "approve"];
    const ROLES: &[&str] = &["cook", "verify", "runner"];
    let verb = tokens.iter().any(|t| VERBS.contains(&t.as_str()));
    let role = tokens.iter().any(|t| ROLES.contains(&t.as_str()));
    let path = tokens
        .iter()
        .any(|t| t.contains('/') || t.starts_with('~') || t.ends_with(".sh"));
    verb && role && path
}

fn is_question(tokens: &[String]) -> bool {
    if !has_go_context(tokens) {
        return false;
    }
    const START: &[&str] = &["what", "who", "which", "how", "is", "does", "can"];
    const TOPIC: &[&str] = &[
        "run",
        "runs",
        "runner",
        "cook",
        "verify",
        "allowed",
        "permission",
        "permissions",
        "may",
        "can",
    ];
    tokens.iter().any(|t| START.contains(&t.as_str()))
        && tokens.iter().any(|t| TOPIC.contains(&t.as_str()))
}

fn flag(v: &Value, key: &str) -> &'static str {
    if v.get("permissions")
        .and_then(|p| p.get(key))
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
    {
        "on"
    } else {
        "off"
    }
}

fn approved_ids(v: &Value) -> String {
    let mut ids: Vec<&str> = v
        .get("approved")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.get("id").and_then(|i| i.as_str()))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        "(none)".into()
    } else {
        ids.join(", ")
    }
}

fn question_reply(v: &Value) -> String {
    let runner = v.get("runner").and_then(|x| x.as_str()).unwrap_or("?");
    let cook = v.get("cook").and_then(|x| x.as_str()).unwrap_or("");
    let verify = v.get("verify").and_then(|x| x.as_str()).unwrap_or("");
    let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("");
    let provider = v.get("provider").and_then(|x| x.as_str()).unwrap_or("");
    let dev = if v
        .get("developerMode")
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
    {
        "on"
    } else {
        "off"
    };
    let mut s = format!(
        "Go runs on {runner}: cook {cook}, verify {verify} ({model} on {provider}). Allowed: push {} · merge to main {} · Stable install {} · send {}. Developer mode {dev}. Approved runners: {}. (go.runner_get, this Nightly)",
        flag(v, "allow_push"),
        flag(v, "allow_merge_main"),
        flag(v, "allow_stable_install"),
        flag(v, "allow_send"),
        approved_ids(v),
    );
    if let Some(e) = v.get("error").and_then(|x| x.as_str()).filter(|s| !s.is_empty()) {
        s.push_str(&format!(" Runner error: {e}"));
    }
    s
}

fn change_reply(v: &Value) -> String {
    let runner = v.get("runner").and_then(|x| x.as_str()).unwrap_or("?");
    format!(
        "I can't change that. Only Matt can change Go permissions or add runners, in Settings → Developer mode (a later update). Nothing was changed. Now: runner {runner}; push {} · merge to main {} · Stable install {} · send {}. Approved runners: {}.",
        flag(v, "allow_push"),
        flag(v, "allow_merge_main"),
        flag(v, "allow_stable_install"),
        flag(v, "allow_send"),
        approved_ids(v),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha(path: &std::path::Path) -> String {
        match std::fs::read(path) {
            Ok(b) => {
                let mut h = Sha256::new();
                h.update(b);
                format!("{:x}", h.finalize())
            }
            Err(_) => "absent".into(),
        }
    }

    #[test]
    fn d4_replay_exact_lines() {
        let (base, roots) = crate::policy_guard::temp_policy_home("d4");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, crate::policy_guard::RULES_FIXTURE).unwrap();
        let settings = roots.home.join(".config/orbitty/settings.json");
        std::fs::write(&settings, "{}\n").unwrap();
        let runner = roots.home.join(".config/orbitty/go-runner.json");
        let before = format!("{}|{}|{}", sha(&rules), sha(&settings), sha(&runner));
        let plans = crate::plans::Roots {
            home: roots.home.clone(),
        };
        let q = "what runs Go, and what is it allowed to do?";
        let c1 = "allow push for Go";
        let c2 = "use /tmp/x.sh as the cook";
        assert_eq!(q, "what runs Go, and what is it allowed to do?");
        assert_eq!(c1, "allow push for Go");
        assert_eq!(c2, "use /tmp/x.sh as the cook");
        let mut mouth = 0usize;
        let mut sink = || mouth += 1;
        let a = handle(q, &plans, &mut sink).unwrap().unwrap();
        assert!(classify(q) == Some(PolicyAsk::Question));
        for bit in [
            "pi-grok",
            "cook-pi.sh",
            "verify-pi.sh",
            "push off",
            "merge to main off",
            "Stable install off",
            "send off",
        ] {
            assert!(a.contains(bit), "{bit} missing in {a}");
        }
        let b = handle(c1, &plans, &mut sink).unwrap().unwrap();
        assert!(classify(c1) == Some(PolicyAsk::Change));
        assert!(b.starts_with("I can't change that. Only Matt can change"), "{b}");
        let c = handle(c2, &plans, &mut sink).unwrap().unwrap();
        assert!(classify(c2) == Some(PolicyAsk::Change));
        assert!(c.starts_with("I can't change that. Only Matt can change"), "{c}");
        assert_eq!(mouth, 0);
        let after = format!("{}|{}|{}", sha(&rules), sha(&settings), sha(&runner));
        assert_eq!(before, after);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn switch_runner_approved_only() {
        let (base, roots) = crate::policy_guard::temp_policy_home("switch");
        let plans = crate::plans::Roots {
            home: roots.home.clone(),
        };
        let path = roots.home.join(".config/orbitty/go-runner.json");
        let mut mouth = 0usize;
        let mut sink = || mouth += 1;
        let ok = handle("switch the Go runner to pi-grok", &plans, &mut sink)
            .unwrap()
            .unwrap();
        assert!(ok.starts_with("Go runner is now pi-grok"), "{ok}");
        assert!(std::fs::read_to_string(&path).unwrap().contains("pi-grok"));
        let no = handle("switch the Go runner to nope", &plans, &mut sink)
            .unwrap()
            .unwrap();
        assert!(no.starts_with("I can't change that."), "{no}");
        assert!(no.contains("not on the approved list"), "{no}");
        assert!(!std::fs::read_to_string(&path).unwrap().contains("nope"));
        assert_eq!(mouth, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn not_policy_lines() {
        for line in [
            "let's go",
            "go ahead and push the card to doing",
            "go to bed",
            "what is a goroutine?",
            "GO-ORBITTY-FOO cursor",
            "push the build button",
            "how's it going home slice?",
        ] {
            assert!(classify(line).is_none(), "{line} -> {:?}", classify(line));
        }
    }
}
