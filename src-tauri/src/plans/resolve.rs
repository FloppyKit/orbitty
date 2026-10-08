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
use super::parse::*;
#[allow(unused_imports)]
use super::rules::*;

/// Header `workspace` with `{REPO_ROOT}` / `{PLANS_DIR}` / `~` expanded,
/// else the repo root setting.
pub fn workspace_in(roots: &Roots, go: &ParsedGo) -> PathBuf {
    match nonblank(go.header.workspace.as_deref()) {
        Some(w) => lock_path(roots, &w),
        None => repo_root(roots),
    }
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
        if under_buzz(roots, &to) {
            return Err(format!(
                "migrate: {} is inside ~/.buzz; refused (nothing is written under .buzz)",
                to.display()
            ));
        }
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

/// No harness has a push flag (cursor-agent, goose and grok have none), so
/// push_allowed is enforced only here: this rule rides every GO prompt.
/// `push_decision` still refuses `git_push` without `push_allowed: true`.
pub fn push_rule(push_ok: bool) -> &'static str {
    if push_ok {
        "git push: allowed for this run (push_allowed: true and git_push in tools)."
    } else {
        "git push: NOT allowed for this run. Do not push to any remote; commit locally only."
    }
}

/// The text every route hands the agent: GO block + launcher rules.
pub fn launch_text(go: &ParsedGo, push_ok: bool) -> String {
    launch_text_in(&Roots::real(), go, push_ok)
}

pub fn launch_text_in(roots: &Roots, go: &ParsedGo, push_ok: bool) -> String {
    format!(
        "{}\n\n## Launcher rules\n\n- {}\n",
        go_block_in(roots, go).trim_end(),
        push_rule(push_ok)
    )
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
        let lock_path = lock_path(roots, &lock).display().to_string();
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

