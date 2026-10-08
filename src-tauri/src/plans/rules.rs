#![allow(unused_imports)]
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use super::dirs::*;
#[allow(unused_imports)]
use super::locate::*;
#[allow(unused_imports)]
use super::seal::*;
#[allow(unused_imports)]
use super::resolve::*;
#[allow(unused_imports)]
use super::parse::*;

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct RulesFile {
    #[serde(default)]
    pub(crate) defaults: RuleBody,
    #[serde(default)]
    pub(crate) rules: Vec<RuleEntry>,
    #[serde(default)]
    pub(crate) legacy_allow: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct RuleEntry {
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) id: String,
    #[serde(rename = "match", default)]
    pub(crate) when: RuleMatch,
    #[serde(flatten)]
    pub(crate) body: RuleBody,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct RuleMatch {
    #[serde(default)]
    pub(crate) go: String,
    #[serde(default)]
    pub(crate) project: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct RuleBody {
    #[serde(default)]
    pub(crate) runner: Option<String>,
    #[serde(default)]
    pub(crate) harness: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) runner_fallback: Option<Fallback>,
    #[serde(default)]
    pub(crate) tools: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) push_allowed: Option<bool>,
}

pub(crate) struct FactRun {
    pub(crate) runner: String,
    pub(crate) harness: String,
    pub(crate) model: String,
    pub(crate) tools: Vec<String>,
    pub(crate) push_allowed: bool,
}

pub(crate) fn load_rules_yaml(roots: &Roots) -> Option<RulesFile> {
    let p = plans_dir(roots).join("RULES.yaml");
    let raw = std::fs::read_to_string(p).ok()?;
    serde_yaml::from_str(&raw).ok()
}

pub(crate) fn first_rule(file: &RulesFile, go_id: &str, project: &str) -> Option<RuleBody> {
    for rule in &file.rules {
        if rule.when.go.trim().is_empty() {
            continue;
        }
        if !go_glob(&rule.when.go, go_id) {
            continue;
        }
        let proj = rule.when.project.trim();
        if !proj.is_empty() && !proj.eq_ignore_ascii_case(project) {
            continue;
        }
        return Some(rule.body.clone());
    }
    None
}

pub(crate) fn legacy_allowed(roots: &Roots, go_id: &str) -> bool {
    let Some(file) = load_rules_yaml(roots) else {
        return false;
    };
    file.legacy_allow.iter().any(|pat| go_glob(pat, go_id))
}

pub(crate) fn go_glob(pattern: &str, go_id: &str) -> bool {
    if go_id.is_empty() {
        return false;
    }
    match pattern.trim().strip_suffix('*') {
        Some(prefix) => {
            !prefix.is_empty()
                && go_id.len() >= prefix.len()
                && go_id[..prefix.len()].eq_ignore_ascii_case(prefix)
        }
        None => go_id.eq_ignore_ascii_case(pattern.trim()),
    }
}

pub(crate) fn matching_fact(roots: &Roots, go_id: &str, project: &str, herdr_up: bool) -> Option<FactRun> {
    let p = bus_dir_in(roots).join("rules.json");
    let raw = std::fs::read_to_string(p).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let rules = v.get("rules")?.as_array()?;
    for rule in rules {
        if rule.get("enabled").and_then(|x| x.as_bool()) == Some(false) {
            continue;
        }
        let params = rule.get("params").cloned().unwrap_or(json!({}));
        let flavor = params.get("flavor").and_then(|x| x.as_str()).unwrap_or("");
        if flavor != "fact" {
            continue;
        }
        let go_pat = params.get("go").and_then(|x| x.as_str()).unwrap_or("");
        let proj = params.get("project").and_then(|x| x.as_str()).unwrap_or("");
        if !go_glob(go_pat, go_id) {
            continue;
        }
        if !proj.is_empty() && !proj.eq_ignore_ascii_case(project) {
            continue;
        }
        let answer = rule.get("answer").and_then(|x| x.as_str()).unwrap_or("");
        let Some(word) = answer_harness(answer) else {
            continue;
        };
        return Some(map_fact(word, herdr_up));
    }
    None
}

pub(crate) fn answer_harness(answer: &str) -> Option<&str> {
    let a = answer.trim();
    let lower = a.to_ascii_lowercase();
    let rest = lower.strip_suffix(" on this planet")?;
    let word = rest.trim();
    if word.is_empty() || word.contains(' ') {
        return None;
    }
    // Return the original-cased slice by splitting the trimmed answer.
    let n = word.len();
    Some(a[..n].trim())
}

pub(crate) fn map_fact(word: &str, herdr_up: bool) -> FactRun {
    let tools = vec![
        "read".into(),
        "edit".into(),
        "shell".into(),
        "git_commit".into(),
        "install_nightly".into(),
        "orbittyctl".into(),
    ];
    if word.eq_ignore_ascii_case("grok") {
        FactRun {
            runner: "herdr-probe".into(),
            harness: if herdr_up { "herdr" } else { "grok" }.into(),
            model: "grok".into(),
            tools,
            push_allowed: false,
        }
    } else {
        FactRun {
            runner: word.to_string(),
            harness: word.to_ascii_lowercase(),
            model: String::new(),
            tools,
            push_allowed: false,
        }
    }
}
