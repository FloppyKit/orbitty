use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{ Value };
use super::parse::{expand_home, load_settings, nonblank, save_settings, snapshot};


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
    pub(crate) fn blank(&self) -> bool {
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
    /// Cursor preset when this default launches a row. Absent = the row's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) plans_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) bus_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) repo_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default_runner: Option<RunnerSpec>,
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
    /// Header text between the `---` fences ("" for a headerless file).
    pub yaml: String,
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

/// The configured plans folder, else the visible `~/Orbitty GOs`. A setting
/// that points inside `~/.buzz` (e.g. a hand-edited settings.json) is
/// ignored with a warning: nothing is created or written under `.buzz`.
pub fn plans_dir(roots: &Roots) -> PathBuf {
    let s = load_settings(roots);
    if let Some(raw) = nonblank(s.plans_dir.as_deref()) {
        let p = expand_home(roots, &raw);
        if !under_buzz(roots, &p) {
            #[cfg(not(feature = "alpha"))]
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
        warn_buzz_plans_dir(&raw);
    }
    let p = roots.home.join("Orbitty GOs");
    #[cfg(not(feature = "alpha"))]
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
        permissions: None,
    }
}

/// Saved default only. A fresh install has none — the user chooses.
/// `builtin_runner` is not a product default.
pub fn chosen_runner(roots: &Roots) -> Option<RunnerSpec> {
    load_settings(roots).default_runner.and_then(|r| {
        let named = !r.runner.trim().is_empty() || !r.harness.trim().is_empty();
        named.then_some(r)
    })
}

pub fn settings_snapshot() -> Result<Value, String> {
    let roots = Roots::real();
    Ok(snapshot(&roots))
}

pub fn set_plans_dir(path: &str) -> Result<Value, String> {
    set_plans_dir_in(&Roots::real(), path)
}

pub(crate) fn set_plans_dir_in(roots: &Roots, path: &str) -> Result<Value, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("plansDir path is empty".into());
    }
    // Absolute after `~/` expansion only: a relative path would resolve
    // against the app's working directory, and `~user` is not expanded.
    let expanded = expand_home(roots, path);
    if !expanded.is_absolute() {
        return Err(format!(
            "plansDir {path} must be an absolute path (or start with ~/)"
        ));
    }
    // Nothing is written under ~/.buzz (read-only back-compat), and the
    // plans folder is written to, so it may not live there.
    if under_buzz(roots, &expanded) {
        return Err(format!(
            "plansDir {path} is inside ~/.buzz; refused (nothing is written under .buzz)"
        ));
    }
    let mut s = load_settings(roots);
    s.plans_dir = Some(path.to_string());
    save_settings(roots, &s)?;
    let _ = plans_dir(roots);
    Ok(snapshot(roots))
}

/// `Some(true)` inside, `Some(false)` not inside, `None` = an error other
/// than not-found (the caller fails closed).
pub(crate) fn under_buzz_by_identity(roots: &Roots, p: &Path) -> Option<bool> {
    under_buzz_by_identity_at(roots, p, 0)
}

pub(crate) fn under_buzz_by_identity_at(roots: &Roots, p: &Path, depth: u32) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    if depth > 32 {
        return None; // too many dangling hops: fail closed
    }
    let ident = |q: &Path| -> Result<Option<(u64, u64)>, ()> {
        match std::fs::metadata(q) {
            Ok(m) => Ok(Some((m.dev(), m.ino()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        }
    };
    let buzz = ident(&roots.home.join(".buzz")).ok()?;
    let home = ident(&roots.home).ok()?;
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(p)
    };
    let mut child: Option<String> = None;
    let mut cur = Some(abs.as_path());
    while let Some(q) = cur {
        match ident(q).ok()? {
            Some(id) => {
                if Some(id) == buzz {
                    return Some(true);
                }
                if Some(id) == home && child.as_deref().map(str::to_lowercase).as_deref() == Some(".buzz") {
                    return Some(true);
                }
            }
            None => {
                // Not found. A dangling symlink (e.g. to a .buzz folder that
                // does not exist yet) is judged by where it points.
                if let Ok(m) = std::fs::symlink_metadata(q) {
                    if m.file_type().is_symlink() {
                        let target = std::fs::read_link(q).ok()?;
                        let target = match q.parent() {
                            Some(dir) => dir.join(target),
                            None => target,
                        };
                        if under_buzz_by_identity_at(roots, &target, depth + 1)? {
                            return Some(true);
                        }
                        let t = canonical_lossy(&target).to_string_lossy().to_lowercase();
                        let b = canonical_lossy(&roots.home.join(".buzz")).to_string_lossy().to_lowercase();
                        let b = b.trim_end_matches('/');
                        if t == b || t.starts_with(&format!("{b}/")) {
                            return Some(true);
                        }
                    }
                }
            }
        }
        child = q.file_name().map(|s| s.to_string_lossy().into_owned());
        cur = q.parent();
    }
    Some(false)
}

/// How many times `plans_dir()` fell back because the setting points inside
/// ~/.buzz (tests read it).
pub(crate) static BUZZ_FALLBACKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) static BUZZ_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The warning text the first time `flag` is used, then None.
pub(crate) fn buzz_warning_once(flag: &std::sync::atomic::AtomicBool, raw: &str) -> Option<String> {
    if flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(format!(
        "orbitty: plansDir {raw} is inside ~/.buzz; using ~/Orbitty GOs instead (nothing is written under .buzz)"
    ))
}

/// Once per process: the plansDir setting points inside ~/.buzz.
pub(crate) fn warn_buzz_plans_dir(raw: &str) {
    BUZZ_FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Some(msg) = buzz_warning_once(&BUZZ_WARNED, raw) {
        eprintln!("{msg}");
    }
}

/// `p` made absolute and canonical as far as it exists (symlinks and `..`
/// resolved by the OS); the missing tail is appended with `.`/`..` applied.
pub(crate) fn canonical_lossy(p: &Path) -> PathBuf {
    use std::path::Component;
    let comps: Vec<Component> = p.components().collect();
    for i in (0..=comps.len()).rev() {
        let head: PathBuf = if i == 0 {
            PathBuf::from(".")
        } else {
            comps[..i].iter().collect()
        };
        if let Ok(mut out) = std::fs::canonicalize(&head) {
            for c in &comps[i..] {
                match c {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            return out;
        }
    }
    p.to_path_buf()
}

/// True for `~/.buzz` or anything inside it.
///
/// 1. File identity: the candidate and each existing parent are matched on
///    `(dev, ino)` against `~/.buzz` (or, before `~/.buzz` exists, against
///    HOME with a `.buzz` child in any case). Covers the
///    `/System/Volumes/Data` firmlink, symlinks and case.
/// 2. The canonical path, compared case-insensitively (APFS).
///
/// Any metadata error other than not-found (a symlink loop, a chmod 000
/// parent) counts as inside: fail closed.
pub(crate) fn under_buzz(roots: &Roots, p: &Path) -> bool {
    match under_buzz_by_identity(roots, p) {
        Some(false) => {}
        _ => return true,
    }
    let buzz = canonical_lossy(&roots.home.join(".buzz"))
        .to_string_lossy()
        .to_lowercase();
    let cand = canonical_lossy(p).to_string_lossy().to_lowercase();
    let buzz = buzz.trim_end_matches('/');
    cand == buzz || cand.starts_with(&format!("{buzz}/"))
}

pub fn set_runner(runner: &str, harness: &str, model: &str) -> Result<Value, String> {
    set_runner_choice(runner, harness, model, None)
}

pub fn set_runner_choice(
    runner: &str,
    harness: &str,
    model: &str,
    permissions: Option<&str>,
) -> Result<Value, String> {
    let roots = Roots::real();
    let mut spec = chosen_runner(&roots).unwrap_or_else(|| RunnerSpec {
        runner: String::new(),
        harness: String::new(),
        model: String::new(),
        runner_fallback: None,
        permissions: None,
    });
    if let Some(v) = nonblank(Some(runner)) {
        spec.runner = v;
    }
    if let Some(v) = nonblank(Some(harness)) {
        spec.harness = v;
    }
    if let Some(v) = nonblank(Some(model)) {
        spec.model = v;
    }
    if let Some(v) = permissions.and_then(|p| nonblank(Some(p))) {
        spec.permissions = Some(v);
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

