//! Plans folder, GO header, seal gate, and ruleset.
//!
//! One shared settings file (`~/.config/orbitty/settings.json`) for Nightly
//! and stable. `plansDir` defaults to the visible `$HOME/Orbitty GOs`.
//! `~/.buzz/PLANS` is read-only back-compat. No absolute home path in code.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const QUERY_CAP: usize = 64_000;

#[derive(Clone, Debug)]
pub struct Roots {
    pub home: PathBuf,
}

impl Roots {
    pub fn real() -> Self {
        Self {
            home: PathBuf::from(std::env::var("HOME").unwrap_or_default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    #[serde(default)]
    pub runner: String,
    #[serde(default)]
    pub harness: String,
    #[serde(default)]
    pub model: String,
}

impl Fallback {
    fn blank(&self) -> bool {
        self.runner.trim().is_empty()
            && self.harness.trim().is_empty()
            && self.model.trim().is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerSpec {
    pub runner: String,
    pub harness: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_fallback: Option<Fallback>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plans_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bus_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repo_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_runner: Option<RunnerSpec>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GoHeader {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sealed_at: Option<String>,
    #[serde(default)]
    pub sealed_by: Option<String>,
    #[serde(default)]
    pub seal_hash: Option<String>,
    #[serde(default)]
    pub runner: Option<String>,
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub runner_fallback: Option<Fallback>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub plans_dir: Option<String>,
    #[serde(default)]
    pub push_allowed: Option<bool>,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub lock: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ParsedGo {
    pub path: PathBuf,
    pub raw: String,
    pub header: GoHeader,
    pub body: String,
    pub legacy: bool,
}

impl ParsedGo {
    pub fn id(&self) -> String {
        nonblank(self.header.id.as_deref())
            .unwrap_or_else(|| "GO".into())
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedRun {
    pub runner: String,
    pub harness: String,
    pub model: String,
    pub tools: Vec<String>,
    pub push_allowed: bool,
    pub fallback: Option<Fallback>,
    pub source: String,
    pub fell_back: bool,
    pub fallback_why: Option<String>,
}

#[derive(Debug)]
pub enum Gate {
    Launch(ParsedGo),
    Refuse { note: String },
    Skip,
}

pub fn dir() -> PathBuf {
    plans_dir(&Roots::real())
}

pub fn bus_dir() -> PathBuf {
    bus_dir_in(&Roots::real())
}

pub fn plans_dir(roots: &Roots) -> PathBuf {
    let s = load_settings(roots);
    if let Some(p) = nonblank(s.plans_dir.as_deref()) {
        let p = expand_home(roots, &p);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    let p = roots.home.join("Orbitty GOs");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn bus_dir_in(roots: &Roots) -> PathBuf {
    let s = load_settings(roots);
    if let Some(p) = nonblank(s.bus_dir.as_deref()) {
        let p = expand_home(roots, &p);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    let buzz = roots.home.join(".buzz/ops/needed-bus");
    if buzz.is_dir() {
        return buzz;
    }
    let native = roots.home.join(".config/orbitty/needed-bus");
    let _ = std::fs::create_dir_all(&native);
    native
}

/// Plans folder first, then `~/.buzz/PLANS` when that directory exists.
pub fn read_dirs() -> Vec<PathBuf> {
    read_dirs_in(&Roots::real())
}

pub fn read_dirs_in(roots: &Roots) -> Vec<PathBuf> {
    let primary = plans_dir(roots);
    let mut out = vec![primary.clone()];
    let buzz = roots.home.join(".buzz/PLANS");
    if buzz.is_dir() && buzz != primary {
        out.push(buzz);
    }
    out
}

pub fn repo_root(roots: &Roots) -> PathBuf {
    let s = load_settings(roots);
    if let Some(p) = nonblank(s.repo_root.as_deref()) {
        return expand_home(roots, &p);
    }
    roots.home.join("orbitty")
}

pub fn effective_runner(roots: &Roots) -> RunnerSpec {
    let s = load_settings(roots);
    s.default_runner.unwrap_or_else(builtin_runner)
}

pub fn builtin_runner() -> RunnerSpec {
    RunnerSpec {
        runner: "orbitty-cursor".into(),
        harness: "cursor".into(),
        model: "grok-4.7-high".into(),
        runner_fallback: Some(Fallback {
            runner: "herdr-probe".into(),
            harness: "herdr".into(),
            model: "grok".into(),
        }),
    }
}

pub fn settings_snapshot() -> Result<Value, String> {
    let roots = Roots::real();
    Ok(snapshot(&roots))
}

pub fn set_plans_dir(path: &str) -> Result<Value, String> {
    let roots = Roots::real();
    let path = path.trim();
    if path.is_empty() {
        return Err("plansDir path is empty".into());
    }
    let mut s = load_settings(&roots);
    s.plans_dir = Some(path.to_string());
    save_settings(&roots, &s)?;
    let _ = plans_dir(&roots);
    Ok(snapshot(&roots))
}

pub fn set_runner(runner: &str, harness: &str, model: &str) -> Result<Value, String> {
    let roots = Roots::real();
    let mut spec = effective_runner(&roots);
    if let Some(v) = nonblank(Some(runner)) {
        spec.runner = v;
    }
    if let Some(v) = nonblank(Some(harness)) {
        spec.harness = v;
    }
    if let Some(v) = nonblank(Some(model)) {
        spec.model = v;
    }
    let mut s = load_settings(&roots);
    s.default_runner = Some(spec);
    save_settings(&roots, &s)?;
    Ok(snapshot(&roots))
}

pub fn reveal_plans_dir() -> Result<(), String> {
    let p = dir();
    let _ = std::fs::create_dir_all(&p);
    let st = std::process::Command::new("open")
        .arg("-R")
        .arg(&p)
        .status()
        .map_err(|e| format!("open: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("Reveal in Finder failed ({})", st.code().unwrap_or(1)))
    }
}

pub fn legacy_prompt_file(go: &str) -> Option<PathBuf> {
    legacy_prompt_in(&Roots::real(), go)
}

fn legacy_prompt_in(roots: &Roots, go: &str) -> Option<PathBuf> {
    let name = format!("_{}_PROMPT.md", go.replace('-', "_"));
    for d in read_dirs_in(roots) {
        let p = d.join(&name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn gate_for_card(id: &str, title: &str, link_urls: &[String]) -> Gate {
    gate_in(&Roots::real(), id, title, link_urls)
}

fn gate_in(roots: &Roots, id: &str, title: &str, link_urls: &[String]) -> Gate {
    match locate(roots, id, title, link_urls) {
        Locate::Missing => Gate::Skip,
        Locate::Bad { note } => Gate::Refuse { note },
        Locate::Found(go) => match seal_note(roots, &go) {
            None => Gate::Launch(go),
            Some(note) => Gate::Refuse { note },
        },
    }
}

enum Locate {
    Found(ParsedGo),
    Bad { note: String },
    Missing,
}

fn locate(roots: &Roots, id: &str, title: &str, link_urls: &[String]) -> Locate {
    let dirs = read_dirs_in(roots);
    for url in link_urls {
        let Some(path) = file_url_to_path(url) else {
            continue;
        };
        if !dirs.iter().any(|d| is_inside(&path, d)) {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        return match parse_file(&path) {
            Ok(g) => Locate::Found(g),
            Err(note) => Locate::Bad { note },
        };
    }
    let token = crate::needed_rules::extract_go(title);
    let mut files = Vec::new();
    for d in &dirs {
        walk_md(d, &mut files, 0);
    }
    files.sort();
    let mut by_card: Option<ParsedGo> = None;
    let mut by_id: Option<ParsedGo> = None;
    let mut malformed_name: Option<String> = None;
    for path in &files {
        match parse_file(path) {
            Ok(g) if !g.legacy => {
                if !id.is_empty() && nonblank(g.header.card_id.as_deref()).as_deref() == Some(id) {
                    by_card.get_or_insert(g);
                    continue;
                }
                if let Some(tok) = token.as_deref() {
                    if nonblank(g.header.id.as_deref()).as_deref() == Some(tok) && by_id.is_none() {
                        by_id = Some(g);
                    }
                }
            }
            Err(note) => {
                if malformed_name.is_none() {
                    if let Some(tok) = token.as_deref() {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let slug = tok.replace('-', "_");
                        if name.contains(tok) || name.contains(&slug) {
                            malformed_name = Some(note);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(g) = by_card.or(by_id) {
        return Locate::Found(g);
    }
    if let Some(tok) = token.as_deref() {
        if let Some(path) = legacy_prompt_in(roots, tok) {
            return match parse_file(&path) {
                Ok(g) => Locate::Found(g),
                Err(note) => Locate::Bad { note },
            };
        }
        if let Some(note) = malformed_name {
            return Locate::Bad { note };
        }
    }
    Locate::Missing
}

/// `None` means sealed and launchable.
pub fn seal_note(roots: &Roots, go: &ParsedGo) -> Option<String> {
    let id = if go.legacy {
        legacy_token_from_path(&go.path).unwrap_or_else(|| go.id())
    } else {
        go.id()
    };
    if go.legacy {
        if legacy_allowed(roots, &id) {
            return None;
        }
        return Some(format!("{id} is unsealed (legacy prompt); not launching."));
    }
    let status = nonblank(go.header.status.as_deref())
        .unwrap_or_else(|| "draft".into())
        .to_ascii_lowercase();
    if status != "sealed" {
        return Some(format!("{id} is {status}, not sealed; not launching."));
    }
    if nonblank(go.header.sealed_at.as_deref()).is_none()
        || nonblank(go.header.sealed_by.as_deref()).is_none()
    {
        return Some(format!(
            "{id} is sealed but missing sealed_at or sealed_by; not launching."
        ));
    }
    let want = nonblank(go.header.seal_hash.as_deref()).unwrap_or_default();
    let got = sha256_hex(go.body.as_bytes());
    if want.is_empty() || !want.eq_ignore_ascii_case(&got) {
        return Some(format!("{id} body changed since seal."));
    }
    None
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_encode(&h.finalize())
}

pub fn seal_file(path: &Path, by: &str) -> Result<ParsedGo, String> {
    let by = by.trim();
    if by.is_empty() {
        return Err("go seal needs --by".into());
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let (yaml, body) = split_front(&raw)?;
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml).map_err(|e| format!("malformed header: {e}"))?;
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| "malformed header: not a map".to_string())?;
    let hash = sha256_hex(body.as_bytes());
    let now = seal_timestamp();
    map.insert(
        serde_yaml::Value::String("status".into()),
        serde_yaml::Value::String("sealed".into()),
    );
    map.insert(
        serde_yaml::Value::String("sealed_at".into()),
        serde_yaml::Value::String(now),
    );
    map.insert(
        serde_yaml::Value::String("sealed_by".into()),
        serde_yaml::Value::String(by.into()),
    );
    map.insert(
        serde_yaml::Value::String("seal_hash".into()),
        serde_yaml::Value::String(hash),
    );
    let dumped = serde_yaml::to_string(&doc).map_err(|e| e.to_string())?;
    let mut dumped = dumped
        .trim_start_matches("---")
        .trim_start_matches(['\n', '\r'])
        .to_string();
    if !dumped.ends_with('\n') {
        dumped.push('\n');
    }
    let out = format!("---\n{dumped}---\n{body}");
    std::fs::write(path, &out).map_err(|e| format!("write {}: {e}", path.display()))?;
    parse_file(path)
}

pub fn status_value(
    file: Option<&str>,
    go: Option<&str>,
    card: Option<&str>,
    title: Option<&str>,
) -> Result<Value, String> {
    let roots = Roots::real();
    if let Some(f) = nonblank(file) {
        let path = PathBuf::from(&f);
        let parsed = parse_file(&path)?;
        return Ok(status_json(&roots, &parsed));
    }
    let go_id = nonblank(go).unwrap_or_default();
    let card_id = nonblank(card).unwrap_or_default();
    let title = if let Some(t) = nonblank(title) {
        t
    } else if go_id.is_empty() {
        return Err("go status needs --file or --go".into());
    } else {
        go_id.clone()
    };
    match locate(&roots, &card_id, &title, &[]) {
        Locate::Missing => Ok(json!({
            "sealed": false,
            "found": false,
            "id": go_id,
            "note": format!("{go_id} is not sealed; not launching."),
        })),
        Locate::Bad { note } => Ok(json!({
            "sealed": false,
            "found": true,
            "id": go_id,
            "note": note,
        })),
        Locate::Found(parsed) => Ok(status_json(&roots, &parsed)),
    }
}

fn status_json(roots: &Roots, go: &ParsedGo) -> Value {
    let note = seal_note(roots, go);
    json!({
        "sealed": note.is_none(),
        "found": true,
        "id": go.id(),
        "file": go.path.display().to_string(),
        "status": go.header.status,
        "legacy": go.legacy,
        "sha256": sha256_hex(go.raw.as_bytes()),
        "note": note.unwrap_or_default(),
    })
}

pub fn migrate_list(roots: &Roots) -> Vec<Value> {
    let src = roots.home.join(".buzz/PLANS");
    let dst = plans_dir(roots);
    let mut rows = Vec::new();
    if !src.is_dir() || src == dst {
        return rows;
    }
    let mut files = Vec::new();
    walk_any(&src, &mut files, 0);
    for from in files {
        let Ok(rel) = from.strip_prefix(&src) else {
            continue;
        };
        let to = dst.join(rel);
        if to.exists() {
            continue;
        }
        rows.push(json!({
            "from": from.display().to_string(),
            "to": to.display().to_string(),
        }));
    }
    rows
}

pub fn migrate_apply(roots: &Roots) -> Result<Value, String> {
    let list = migrate_list(roots);
    let mut copied = 0u64;
    for row in &list {
        let from = PathBuf::from(row["from"].as_str().unwrap_or(""));
        let to = PathBuf::from(row["to"].as_str().unwrap_or(""));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        if to.exists() {
            continue;
        }
        std::fs::copy(&from, &to)
            .map_err(|e| format!("copy {} → {}: {e}", from.display(), to.display()))?;
        copied += 1;
    }
    Ok(json!({ "copied": copied, "files": list }))
}

pub fn resolve_run(header: &GoHeader, go_id: &str, project: &str) -> ResolvedRun {
    resolve_run_in(&Roots::real(), header, go_id, project, herdr_bin_present())
}

pub fn resolve_run_in(
    roots: &Roots,
    header: &GoHeader,
    go_id: &str,
    project: &str,
    herdr_up: bool,
) -> ResolvedRun {
    let builtin = effective_runner(roots);
    let yaml = load_rules_yaml(roots);
    let (rule, defaults, yaml_present) = match &yaml {
        Some(f) => (first_rule(f, go_id, project), f.defaults.clone(), true),
        None => (None, RuleBody::default(), false),
    };
    let fact = if yaml_present {
        None
    } else {
        matching_fact(roots, go_id, project, herdr_up)
    };
    let pick = |h: Option<&str>, r: Option<&str>, d: Option<&str>, f: Option<&str>, b: &str| {
        nonblank(h)
            .or_else(|| nonblank(r))
            .or_else(|| nonblank(d))
            .or_else(|| nonblank(f))
            .unwrap_or_else(|| b.to_string())
    };
    let rule_b = rule.as_ref();
    let fact_b = fact.as_ref();
    let runner = pick(
        header.runner.as_deref(),
        rule_b.and_then(|r| r.runner.as_deref()),
        defaults.runner.as_deref(),
        fact_b.map(|f| f.runner.as_str()),
        &builtin.runner,
    );
    let harness = pick(
        header.harness.as_deref(),
        rule_b.and_then(|r| r.harness.as_deref()),
        defaults.harness.as_deref(),
        fact_b.map(|f| f.harness.as_str()),
        &builtin.harness,
    );
    let model = pick(
        header.model.as_deref(),
        rule_b.and_then(|r| r.model.as_deref()),
        defaults.model.as_deref(),
        fact_b.map(|f| f.model.as_str()),
        &builtin.model,
    );
    let tools = header
        .tools
        .as_ref()
        .filter(|t| !t.is_empty())
        .cloned()
        .or_else(|| rule_b.and_then(|r| r.tools.clone()).filter(|t| !t.is_empty()))
        .or_else(|| defaults.tools.clone().filter(|t| !t.is_empty()))
        .or_else(|| fact_b.map(|f| f.tools.clone()))
        .unwrap_or_else(|| {
            vec![
                "read".into(),
                "edit".into(),
                "shell".into(),
                "git_commit".into(),
                "install_nightly".into(),
                "orbittyctl".into(),
            ]
        });
    let push_allowed = header
        .push_allowed
        .or(rule_b.and_then(|r| r.push_allowed))
        .or(defaults.push_allowed)
        .or(fact_b.map(|f| f.push_allowed))
        .unwrap_or(false);
    let fallback = header
        .runner_fallback
        .clone()
        .filter(|f| !f.blank())
        .or_else(|| rule_b.and_then(|r| r.runner_fallback.clone()).filter(|f| !f.blank()))
        .or_else(|| defaults.runner_fallback.clone().filter(|f| !f.blank()))
        .or_else(|| builtin.runner_fallback.clone().filter(|f| !f.blank()));
    let source = if nonblank(header.runner.as_deref()).is_some()
        || nonblank(header.harness.as_deref()).is_some()
        || nonblank(header.model.as_deref()).is_some()
    {
        "header"
    } else if rule_b.is_some() {
        "RULES.yaml"
    } else if fact_b.is_some() {
        "rules.json"
    } else if yaml_present {
        "RULES.yaml"
    } else {
        "defaultRunner"
    };
    ResolvedRun {
        runner,
        harness,
        model,
        tools,
        push_allowed,
        fallback,
        source: source.into(),
        fell_back: false,
        fallback_why: None,
    }
}

/// `Ok(true)` = pass a push flag. `Ok(false)` = no flag. `Err` = refused.
pub fn push_decision(run: &ResolvedRun) -> Result<bool, String> {
    let wants = run.tools.iter().any(|t| t == "git_push");
    if wants && !run.push_allowed {
        return Err("git_push refused: push_allowed is false".into());
    }
    Ok(wants && run.push_allowed)
}

pub fn apply_fallback(run: &ResolvedRun, primary_ok: bool) -> ResolvedRun {
    if primary_ok {
        return run.clone();
    }
    let Some(fb) = run.fallback.as_ref().filter(|f| !f.blank()) else {
        return run.clone();
    };
    let why = format!(
        "fallback: {} unavailable (binary missing or harness down)",
        if run.harness.is_empty() {
            run.runner.as_str()
        } else {
            run.harness.as_str()
        }
    );
    ResolvedRun {
        runner: if fb.runner.trim().is_empty() {
            run.runner.clone()
        } else {
            fb.runner.clone()
        },
        harness: if fb.harness.trim().is_empty() {
            run.harness.clone()
        } else {
            fb.harness.clone()
        },
        model: if fb.model.trim().is_empty() {
            run.model.clone()
        } else {
            fb.model.clone()
        },
        tools: run.tools.clone(),
        push_allowed: run.push_allowed,
        fallback: None,
        source: run.source.clone(),
        fell_back: true,
        fallback_why: Some(why),
    }
}

pub fn door_harness(harness: &str) -> String {
    match harness.trim().to_ascii_lowercase().as_str() {
        "herdr" => "grok".into(),
        other => other.to_string(),
    }
}

pub fn exec_with_push(line: &str, push: bool) -> String {
    if !push {
        return line.to_string();
    }
    if line.contains("--git-push") {
        return line.to_string();
    }
    if let Some(i) = line.rfind("\"$prompt\"") {
        let mut s = line.to_string();
        s.insert_str(i, "--git-push ");
        return s;
    }
    format!("{line} --git-push")
}

pub fn unsupported_tools(harness: &str, tools: &[String]) -> Vec<String> {
    let door = door_harness(harness);
    let door = door.as_str();
    tools
        .iter()
        .filter(|t| {
            let t = t.as_str();
            t.starts_with("mcp:")
                || t == "browser"
                || !matches!(
                    t,
                    "read"
                        | "edit"
                        | "shell"
                        | "git_commit"
                        | "git_push"
                        | "network"
                        | "install_nightly"
                        | "orbittyctl"
                )
                || (door != "cursor" && door != "goose" && (t == "install_nightly" || t == "orbittyctl"))
        })
        .cloned()
        .collect()
}

pub fn go_block(go: &ParsedGo) -> String {
    let roots = Roots::real();
    go_block_in(&roots, go)
}

pub fn go_block_in(roots: &Roots, go: &ParsedGo) -> String {
    let plans = plans_dir(roots).display().to_string();
    let repo = repo_root(roots).display().to_string();
    let mut text = expand_text(&go.raw, &repo, &plans);
    if let Some(stated) = nonblank(go.header.plans_dir.as_deref()) {
        if stated != plans {
            text = text.replace(&stated, &plans);
        }
    }
    if let Some(lock) = nonblank(go.header.lock.as_deref()) {
        let lock_path = expand_text(&lock, &repo, &plans);
        match std::fs::read_to_string(&lock_path) {
            Ok(body) => {
                let body = expand_text(&body, &repo, &plans);
                text.push_str(&format!("\n\n--- lock {lock_path} ---\n\n{body}"));
            }
            Err(_) => {
                text.push_str(&format!("\n\n--- lock {lock_path} ---\n\n(unreadable)\n"));
            }
        }
    }
    text
}

pub fn enforce_cap(body: &str) -> Result<(), String> {
    let n = body.chars().count();
    if n > QUERY_CAP {
        Err(format!(
            "GO query is {n} chars; cap is {QUERY_CAP}. Not launching."
        ))
    } else {
        Ok(())
    }
}

pub fn join_note(existing: Option<&str>, line: &str) -> String {
    let line = line.trim();
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        None => line.to_string(),
        Some(e) if e.contains(line) => e.to_string(),
        Some(e) => format!("{e}\n{line}"),
    }
}

fn snapshot(roots: &Roots) -> Value {
    let run = effective_runner(roots);
    json!({
        "plansDir": plans_dir(roots).display().to_string(),
        "busDir": bus_dir_in(roots).display().to_string(),
        "repoRoot": repo_root(roots).display().to_string(),
        "runner": run.runner,
        "harness": run.harness,
        "model": run.model,
        "runnerFallback": run.runner_fallback,
    })
}

fn settings_path(roots: &Roots) -> PathBuf {
    roots.home.join(".config/orbitty/settings.json")
}

fn load_settings(roots: &Roots) -> Settings {
    let p = settings_path(roots);
    let Ok(raw) = std::fs::read_to_string(&p) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_settings(roots: &Roots, s: &Settings) -> Result<(), String> {
    let p = settings_path(roots);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(&p, raw + "\n").map_err(|e| format!("write {}: {e}", p.display()))
}

fn expand_home(roots: &Roots, raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        return roots.home.join(rest);
    }
    if raw == "~" {
        return roots.home.clone();
    }
    PathBuf::from(raw)
}

fn expand_text(text: &str, repo: &str, plans: &str) -> String {
    text.replace("{REPO_ROOT}", repo)
        .replace("{PLANS_DIR}", plans)
}

fn nonblank(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn seal_timestamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn parse_file(path: &Path) -> Result<ParsedGo, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("GO unreadable {}: {e}", path.display()))?;
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return Ok(ParsedGo {
            path: path.to_path_buf(),
            header: GoHeader::default(),
            body: raw.clone(),
            raw,
            legacy: true,
        });
    }
    let (yaml, body) = split_front(trimmed)?;
    let header: GoHeader =
        serde_yaml::from_str(&yaml).map_err(|e| format!("malformed header: {e}"))?;
    let header = normalize_header(header);
    Ok(ParsedGo {
        path: path.to_path_buf(),
        raw,
        header,
        body,
        legacy: false,
    })
}

fn normalize_header(mut h: GoHeader) -> GoHeader {
    h.id = nonblank(h.id.as_deref());
    h.title = nonblank(h.title.as_deref());
    h.status = nonblank(h.status.as_deref()).map(|s| s.to_ascii_lowercase());
    h.sealed_at = nonblank(h.sealed_at.as_deref());
    h.sealed_by = nonblank(h.sealed_by.as_deref());
    h.seal_hash = nonblank(h.seal_hash.as_deref());
    h.runner = nonblank(h.runner.as_deref());
    h.harness = nonblank(h.harness.as_deref());
    h.model = nonblank(h.model.as_deref());
    h.workspace = nonblank(h.workspace.as_deref());
    h.plans_dir = nonblank(h.plans_dir.as_deref());
    h.card_id = nonblank(h.card_id.as_deref());
    h.lock = nonblank(h.lock.as_deref());
    if h.runner_fallback.as_ref().is_some_and(|f| f.blank()) {
        h.runner_fallback = None;
    }
    if h.tools.as_ref().is_some_and(|t| t.is_empty()) {
        h.tools = None;
    }
    h
}

fn split_front(raw: &str) -> Result<(String, String), String> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if !raw.starts_with("---") {
        return Err("malformed header: no opening ---".into());
    }
    let after = &raw[3..];
    let after = after.strip_prefix('\r').unwrap_or(after);
    let after = after
        .strip_prefix('\n')
        .ok_or_else(|| "malformed header: opening ---".to_string())?;
    let mut line_start = 0usize;
    while line_start <= after.len() {
        let rest = &after[line_start..];
        let nl = rest.find('\n').map(|i| line_start + i);
        let line_end = nl.unwrap_or(after.len());
        let line = after[line_start..line_end].trim_end_matches('\r');
        if line == "---" {
            let body_at = nl.map(|i| i + 1).unwrap_or(after.len());
            return Ok((after[..line_start].to_string(), after[body_at..].to_string()));
        }
        let Some(i) = nl else { break };
        line_start = i + 1;
    }
    Err("malformed header: no closing ---".into())
}

fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let u = url.trim();
    let rest = u
        .strip_prefix("file://")
        .or_else(|| u.strip_prefix("FILE://"))?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    if decoded.is_empty() || !decoded.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(decoded))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (from_hex(b[i + 1]), from_hex(b[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn is_inside(path: &Path, dir: &Path) -> bool {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    path.starts_with(dir)
}

fn walk_md(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk_md(&p, out, depth + 1);
        } else if name.ends_with(".md") {
            out.push(p);
        }
    }
}

fn walk_any(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk_any(&p, out, depth + 1);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

fn herdr_bin_present() -> bool {
    which("herdr")
}

fn which(bin: &str) -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        if Path::new(dir).join(bin).is_file() {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RulesFile {
    #[serde(default)]
    defaults: RuleBody,
    #[serde(default)]
    rules: Vec<RuleEntry>,
    #[serde(default)]
    legacy_allow: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RuleEntry {
    #[serde(default)]
    #[allow(dead_code)]
    id: String,
    #[serde(rename = "match", default)]
    when: RuleMatch,
    #[serde(flatten)]
    body: RuleBody,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RuleMatch {
    #[serde(default)]
    go: String,
    #[serde(default)]
    project: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RuleBody {
    #[serde(default)]
    runner: Option<String>,
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    runner_fallback: Option<Fallback>,
    #[serde(default)]
    tools: Option<Vec<String>>,
    #[serde(default)]
    push_allowed: Option<bool>,
}

struct FactRun {
    runner: String,
    harness: String,
    model: String,
    tools: Vec<String>,
    push_allowed: bool,
}

fn load_rules_yaml(roots: &Roots) -> Option<RulesFile> {
    let p = plans_dir(roots).join("RULES.yaml");
    let raw = std::fs::read_to_string(p).ok()?;
    serde_yaml::from_str(&raw).ok()
}

fn first_rule(file: &RulesFile, go_id: &str, project: &str) -> Option<RuleBody> {
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

fn legacy_allowed(roots: &Roots, go_id: &str) -> bool {
    let Some(file) = load_rules_yaml(roots) else {
        return false;
    };
    file.legacy_allow.iter().any(|pat| go_glob(pat, go_id))
}

fn go_glob(pattern: &str, go_id: &str) -> bool {
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

fn matching_fact(roots: &Roots, go_id: &str, project: &str, herdr_up: bool) -> Option<FactRun> {
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

fn answer_harness(answer: &str) -> Option<&str> {
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

fn map_fact(word: &str, herdr_up: bool) -> FactRun {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_roots(label: &str) -> Roots {
        let p = std::env::temp_dir().join(format!(
            "orbitty-plans-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Roots { home: p }
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn fresh_plans_dir_is_visible_home_folder() {
        let roots = temp_roots("a0");
        let p = plans_dir(&roots);
        assert_eq!(p, roots.home.join("Orbitty GOs"));
        assert!(p.is_dir());
        assert!(!p.to_string_lossy().contains("Library"));
        let bus = bus_dir_in(&roots);
        assert_eq!(bus, roots.home.join(".config/orbitty/needed-bus"));
        assert!(!bus.to_string_lossy().contains("Library"));
    }

    #[test]
    fn setting_overrides_and_buzz_is_back_compat() {
        let roots = temp_roots("set");
        let custom = roots.home.join("custom-plans");
        let mut s = Settings::default();
        s.plans_dir = Some(custom.display().to_string());
        save_settings(&roots, &s).unwrap();
        assert_eq!(plans_dir(&roots), custom);
        let buzz = roots.home.join(".buzz/PLANS");
        write(&buzz.join("_GO_ONLY_BUZZ_PROMPT.md"), "legacy\n");
        let found = legacy_prompt_in(&roots, "GO-ONLY-BUZZ");
        assert_eq!(found.unwrap(), buzz.join("_GO_ONLY_BUZZ_PROMPT.md"));
    }

    #[test]
    fn header_parse_and_resolver_order() {
        let roots = temp_roots("resolve");
        let plans = plans_dir(&roots);
        let link = plans.join("link.md");
        let by_card = plans.join("card.md");
        let by_id = plans.join("id.md");
        let legacy = plans.join("_GO_ORDER_PROMPT.md");
        write(
            &link,
            "---\nid: GO-LINK\nstatus: sealed\nsealed_at: 2026-09-29T15:40:00-05:00\nsealed_by: matt\nseal_hash: placeholder\n---\nLINK-BODY\n",
        );
        write(
            &by_card,
            "---\nid: GO-CARD\ncard_id: c-1\nstatus: ready\n---\nCARD-BODY\n",
        );
        write(&by_id, "---\nid: GO-ORDER\nstatus: ready\n---\nID-BODY\n");
        write(&legacy, "no header\n");
        // Fix the link file's hash so it is actually sealed.
        seal_file(&link, "matt").unwrap();
        let url = format!("file://{}", link.display());
        match gate_in(&roots, "c-1", "GO-ORDER", &[url]) {
            Gate::Launch(g) => assert!(g.body.contains("LINK-BODY"), "{}", g.body),
            Gate::Refuse { note } => panic!("link should launch: {note}"),
            Gate::Skip => panic!("missing"),
        }
        match gate_in(&roots, "c-1", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("card_id should win and be unsealed: {other:?}"),
        }
        // Drop the card_id file so the id match is next.
        std::fs::remove_file(&by_card).unwrap();
        match gate_in(&roots, "c-9", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("GO-ORDER"), "{note}"),
            other => panic!("id match: {other:?}"),
        }
        std::fs::remove_file(&by_id).unwrap();
        match gate_in(&roots, "c-9", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("legacy"), "{note}"),
            other => panic!("legacy: {other:?}"),
        }
    }

    #[test]
    fn seal_hash_blocks_a_body_edit() {
        let roots = temp_roots("seal");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-SEAL\nstatus: ready\n---\nSENTINEL-LINE\n",
        );
        let sealed = seal_file(&p, "matt").unwrap();
        assert!(seal_note(&roots, &sealed).is_none());
        let raw = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, raw.replace("SENTINEL-LINE", "EDITED-LINE")).unwrap();
        let again = parse_file(&p).unwrap();
        let note = seal_note(&roots, &again).unwrap();
        assert!(note.contains("body changed since seal"), "{note}");
    }

    #[test]
    fn malformed_header_is_an_error() {
        let roots = temp_roots("bad");
        let p = plans_dir(&roots).join("GO-BAD.md");
        write(&p, "---\nstatus: [\n---\nbody\n");
        match gate_in(&roots, "", "GO-BAD", &[]) {
            Gate::Refuse { note } => assert!(note.contains("malformed"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ruleset_precedence_header_yaml_json_default() {
        let roots = temp_roots("rules");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\ndefaults:\n  harness: manual\n  tools: [read]\nrules:\n  - id: r1\n    match: { go: \"GO-ORBITTY*\", project: orbitty }\n    runner: from-yaml\n    harness: cursor\n    model: grok-4.7-high\n    tools: [read, edit]\n",
        );
        let header = GoHeader {
            runner: Some("from-header".into()),
            ..GoHeader::default()
        };
        let run = resolve_run_in(&roots, &header, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "from-header");
        assert_eq!(run.harness, "cursor");
        assert_eq!(run.model, "grok-4.7-high");
        assert_eq!(run.source, "header");

        let blank = GoHeader::default();
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "from-yaml");
        assert_eq!(run.source, "RULES.yaml");

        std::fs::remove_file(plans.join("RULES.yaml")).unwrap();
        let bus = roots.home.join(".buzz/ops/needed-bus");
        write(
            &bus.join("rules.json"),
            r#"{"version":1,"rules":[{"id":"r-mu3ioy6o-w7z0ky","enabled":true,"params":{"flavor":"fact","go":"GO-ORBITTY*","project":"orbitty"},"answer":"grok on this planet"}]}"#,
        );
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "herdr-probe");
        assert_eq!(run.harness, "herdr");
        assert_eq!(run.model, "grok");
        assert_eq!(run.source, "rules.json");

        std::fs::remove_file(bus.join("rules.json")).unwrap();
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", false);
        assert_eq!(run.runner, "orbitty-cursor");
        assert_eq!(run.model, "grok-4.7-high");
        assert_eq!(run.harness, "cursor");
        assert_eq!(run.source, "defaultRunner");
    }

    #[test]
    fn fallback_names_why_and_push_rules() {
        let run = ResolvedRun {
            runner: "orbitty-cursor".into(),
            harness: "cursor".into(),
            model: "grok-4.7-high".into(),
            tools: vec!["read".into(), "git_push".into()],
            push_allowed: false,
            fallback: Some(Fallback {
                runner: "herdr-probe".into(),
                harness: "herdr".into(),
                model: "grok".into(),
            }),
            source: "header".into(),
            fell_back: false,
            fallback_why: None,
        };
        assert!(push_decision(&run).unwrap_err().contains("refused"));
        let mut no_push_tool = run.clone();
        no_push_tool.tools = vec!["read".into()];
        no_push_tool.push_allowed = true;
        assert_eq!(push_decision(&no_push_tool).unwrap(), false);
        assert!(!exec_with_push("grok \"$prompt\"", false).contains("--git-push"));
        let mut allowed = run.clone();
        allowed.push_allowed = true;
        assert_eq!(push_decision(&allowed).unwrap(), true);
        assert!(exec_with_push("cursor \"$prompt\"", true).contains("--git-push"));
        let fb = apply_fallback(&run, false);
        assert!(fb.fell_back);
        assert_eq!(fb.runner, "herdr-probe");
        assert_eq!(fb.harness, "herdr");
        assert_eq!(fb.model, "grok");
        assert!(fb.fallback_why.unwrap().contains("unavailable"));
        assert_eq!(door_harness("herdr"), "grok");
    }

    #[test]
    fn legacy_allow_and_query_cap_and_full_text() {
        let roots = temp_roots("cap");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\nlegacy_allow:\n  - \"GO-OLD*\"\nrules: []\n",
        );
        let legacy = plans.join("_GO_OLD_ONE_PROMPT.md");
        write(&legacy, "SENTINEL-OLD\n");
        let parsed = parse_file(&legacy).unwrap();
        assert!(seal_note(&roots, &parsed).is_none());
        let blocked = plans.join("_GO_NEW_ONE_PROMPT.md");
        write(&blocked, "nope\n");
        match gate_in(&roots, "", "GO-NEW-ONE", &[]) {
            Gate::Refuse { note } => assert!(note.contains("legacy"), "{note}"),
            other => panic!("{other:?}"),
        }
        let go = plans.join("full.md");
        let lock = plans.join("lock.md");
        write(&lock, "LOCK-SENTINEL\n");
        write(
            &go,
            "---\nid: GO-FULL\nstatus: ready\nlock: \"{PLANS_DIR}/lock.md\"\nworkspace: \"{REPO_ROOT}\"\n---\nBODY-SENTINEL\n",
        );
        let parsed = parse_file(&go).unwrap();
        let block = go_block_in(&roots, &parsed);
        assert!(block.contains("BODY-SENTINEL"), "{block}");
        assert!(block.contains("LOCK-SENTINEL"), "{block}");
        assert!(block.contains(&repo_root(&roots).display().to_string()));
        let huge = "x".repeat(QUERY_CAP + 1);
        assert!(enforce_cap(&huge).unwrap_err().contains("64000"));
        assert!(enforce_cap("ok").is_ok());
    }

    #[test]
    fn migrate_dry_run_lists_and_does_not_delete() {
        let roots = temp_roots("mig");
        let buzz = roots.home.join(".buzz/PLANS");
        write(&buzz.join("a.md"), "hello\n");
        let list = migrate_list(&roots);
        assert_eq!(list.len(), 1);
        assert!(buzz.join("a.md").is_file());
        assert!(!plans_dir(&roots).join("a.md").exists());
    }
}

fn legacy_token_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix('_')?.strip_suffix("_PROMPT.md")?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.replace('_', "-"))
}
