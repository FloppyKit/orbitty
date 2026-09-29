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
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
        warn_buzz_plans_dir(&raw);
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
    set_plans_dir_in(&Roots::real(), path)
}

fn set_plans_dir_in(roots: &Roots, path: &str) -> Result<Value, String> {
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
fn under_buzz_by_identity(roots: &Roots, p: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
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
        if let Some(id) = ident(q).ok()? {
            if Some(id) == buzz {
                return Some(true);
            }
            if Some(id) == home && child.as_deref().map(str::to_lowercase).as_deref() == Some(".buzz") {
                return Some(true);
            }
        }
        child = q.file_name().map(|s| s.to_string_lossy().into_owned());
        cur = q.parent();
    }
    Some(false)
}

/// Once per process: the plansDir setting points inside ~/.buzz.
fn warn_buzz_plans_dir(raw: &str) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!(
            "orbitty: plansDir {raw} is inside ~/.buzz; using ~/Orbitty GOs instead (nothing is written under .buzz)"
        );
    }
}

/// `p` made absolute and canonical as far as it exists (symlinks and `..`
/// resolved by the OS); the missing tail is appended with `.`/`..` applied.
fn canonical_lossy(p: &Path) -> PathBuf {
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

/// Deepest folder level searched for GO files under a plans folder. A match
/// nested deeper is refused with a note, never silently skipped.
pub const MAX_GO_DEPTH: usize = 6;
/// How far the walk looks, so a too-deep match is seen and refused.
const WALK_LIMIT: usize = 16;

/// Fail closed: once a card names a GO token (or links a file inside the
/// plans folder), the answer is a GO file or a refusal note. `Missing`
/// (launch with the title only) is only for cards that are not GOs.
fn locate(roots: &Roots, id: &str, title: &str, link_urls: &[String]) -> Locate {
    let dirs = read_dirs_in(roots);
    let mut linked = false;
    for url in link_urls {
        let Some(path) = file_url_to_path(url) else {
            continue;
        };
        if !dirs.iter().any(|d| is_inside(&path, d)) {
            continue;
        }
        linked = true;
        if !path.is_file() {
            continue;
        }
        return match parse_file(&path) {
            Ok(g) => Locate::Found(g),
            Err(note) => Locate::Bad { note },
        };
    }
    let token = crate::needed_rules::extract_go(title);
    let tok = token.as_deref();
    let slug = tok.map(|t| t.replace('-', "_"));
    let names_token = |p: &Path| -> bool {
        let (Some(t), Some(s)) = (tok, slug.as_deref()) else {
            return false;
        };
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        name.contains(t) || name.contains(s)
    };
    let mut files = Vec::new();
    for d in &dirs {
        walk_md(d, &mut files, 0);
    }
    files.sort();
    let mut by_card: Option<(ParsedGo, usize)> = None;
    let mut by_id: Option<(ParsedGo, usize)> = None;
    let mut headerless: Option<PathBuf> = None;
    let mut malformed_named: Option<String> = None;
    let mut malformed_other: Option<String> = None;
    for (path, depth) in &files {
        match parse_file(path) {
            Ok(g) if !g.legacy => {
                if !id.is_empty() && g.header.card_id.as_deref() == Some(id.trim()) {
                    if by_card.is_none() {
                        by_card = Some((g, *depth));
                    }
                    continue;
                }
                // Header ids match case-insensitively: a lowercase `id`
                // is still this GO and still goes through the seal gate.
                if let Some(t) = tok {
                    if by_id.is_none()
                        && g.header.id.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(t))
                    {
                        by_id = Some((g, *depth));
                    }
                }
            }
            Ok(_) => {
                // Headerless file named for this GO (not a legacy
                // `_X_PROMPT.md`, which has its own rule below).
                if headerless.is_none()
                    && legacy_token_from_path(path).is_none()
                    && names_token(path)
                {
                    headerless = Some(path.clone());
                }
            }
            Err(note) => {
                if names_token(path) {
                    if malformed_named.is_none() {
                        malformed_named = Some(note);
                    }
                } else if tok.is_some() && malformed_other.is_none() {
                    malformed_other = Some(format!("{}: {note}", path.display()));
                }
            }
        }
    }
    if let Some((g, depth)) = by_card.or(by_id) {
        if depth > MAX_GO_DEPTH {
            return Locate::Bad {
                note: format!(
                    "{} is nested {depth} folders deep in the plans folder (max {MAX_GO_DEPTH}); not launching.",
                    g.path.display()
                ),
            };
        }
        return Locate::Found(g);
    }
    let Some(t) = tok else {
        if linked {
            return Locate::Bad {
                note: "The card links a GO file in the plans folder that is missing; not launching."
                    .into(),
            };
        }
        return Locate::Missing;
    };
    if let Some(path) = legacy_prompt_in(roots, t) {
        return match parse_file(&path) {
            Ok(g) => Locate::Found(g),
            Err(note) => Locate::Bad { note },
        };
    }
    if let Some(p) = headerless {
        return Locate::Bad {
            note: format!("{t}: {} has no GO header; not launching.", p.display()),
        };
    }
    if let Some(note) = malformed_named {
        return Locate::Bad { note };
    }
    if let Some(note) = malformed_other {
        return Locate::Bad {
            note: format!(
                "{t}: no GO file with a matching header; a malformed header may be it ({note}); not launching."
            ),
        };
    }
    Locate::Bad {
        note: format!("{t}: no GO file with a matching header in the plans folder; not launching."),
    }
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
    let Some(want) = nonblank(go.header.seal_hash.as_deref()) else {
        return Some(format!(
            "{id} is unsealed (no seal_hash; run go seal); not launching."
        ));
    };
    match seal_hash_of(roots, &go.yaml, &go.header, &go.body) {
        Ok(got) if want.eq_ignore_ascii_case(&got) => None,
        Ok(_) => Some(format!(
            "{id} changed since seal (body, header or lock); not launching."
        )),
        Err(e) => Some(format!("{id} seal check failed ({e}); not launching.")),
    }
}

/// Header keys the seal writes, so the seal hash leaves them out.
const SEAL_KEYS: &[&str] = &["status", "sealed_at", "sealed_by", "seal_hash"];

/// The seal covers every header field except the seal fields themselves
/// (so push_allowed, tools, runner, harness, model, lock, workspace ...),
/// the body, and the lock file's text. Header comments and key order don't
/// count: the header is hashed as canonical JSON with sorted keys.
pub fn seal_hash_of(
    roots: &Roots,
    yaml: &str,
    header: &GoHeader,
    body: &str,
) -> Result<String, String> {
    let doc: serde_yaml::Value = if yaml.trim().is_empty() {
        serde_yaml::Value::Mapping(Default::default())
    } else {
        serde_yaml::from_str(yaml).map_err(|e| format!("malformed header: {e}"))?
    };
    let mut j = serde_json::to_value(&doc).map_err(|e| format!("malformed header: {e}"))?;
    let obj = j
        .as_object_mut()
        .ok_or_else(|| "malformed header: not a map".to_string())?;
    for k in SEAL_KEYS {
        obj.remove(*k);
    }
    let head = canonical_json(&j);
    let lock = match nonblank(header.lock.as_deref()) {
        None => "(no lock)".to_string(),
        Some(l) => {
            let p = lock_path(roots, &l);
            std::fs::read_to_string(&p)
                .map_err(|e| format!("lock unreadable {}: {e}", p.display()))?
        }
    };
    let mut h = Sha256::new();
    h.update(b"orbitty-seal-v2\n");
    for (name, part) in [("header", head.as_str()), ("body", body), ("lock", lock.as_str())] {
        h.update(format!("{name} {}\n", part.len()).as_bytes());
        h.update(part.as_bytes());
        h.update(b"\n");
    }
    Ok(hex_encode(&h.finalize()))
}

fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", Value::String((*k).clone()), canonical_json(&m[*k])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(canonical_json).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

fn lock_path(roots: &Roots, raw: &str) -> PathBuf {
    let plans = plans_dir(roots).display().to_string();
    let repo = repo_root(roots).display().to_string();
    expand_home(roots, &expand_text(raw, &repo, &plans))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_encode(&h.finalize())
}

pub fn seal_file(path: &Path, by: &str) -> Result<ParsedGo, String> {
    seal_file_in(&Roots::real(), path, by)
}

/// Seal in place. Only the `status`, `sealed_at`, `sealed_by` and
/// `seal_hash` lines change (added at the end of the header when absent);
/// comments, key order and the body stay byte-for-byte. The result must
/// verify before anything is written.
pub fn seal_file_in(roots: &Roots, path: &Path, by: &str) -> Result<ParsedGo, String> {
    let by = by.trim();
    if by.is_empty() {
        return Err("go seal needs --by".into());
    }
    if under_buzz(roots, path) {
        return Err(format!(
            "go seal: {} is inside ~/.buzz; refused (nothing is written under .buzz)",
            path.display()
        ));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let bom = raw.starts_with('\u{feff}');
    let text = raw.trim_start_matches('\u{feff}');
    let (yaml, body) = split_front(text)?;
    let header: GoHeader =
        serde_yaml::from_str(&yaml).map_err(|e| format!("malformed header: {e}"))?;
    let header = normalize_header(header);
    let hash = seal_hash_of(roots, &yaml, &header, &body)?;
    let nl = if text.starts_with("---\r\n") { "\r\n" } else { "\n" };
    let quote = |v: &str| serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into());
    let new_yaml = set_header_keys(
        &yaml,
        &[
            ("status", "sealed".to_string()),
            ("sealed_at", quote(&seal_timestamp())),
            ("sealed_by", quote(by)),
            ("seal_hash", hash),
        ],
        nl,
    );
    let out = format!(
        "{}---{nl}{new_yaml}---{nl}{body}",
        if bom { "\u{feff}" } else { "" }
    );
    let parsed = parse_text(path, out.clone())?;
    if let Some(note) = seal_note(roots, &parsed) {
        return Err(format!("seal did not verify: {note}"));
    }
    std::fs::write(path, &out).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(parsed)
}

/// Top-level YAML key on this line, if it is a plain `key: ...` line.
fn top_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (k, _) = line.split_once(':')?;
    let k = k.trim().trim_matches(['"', '\'']);
    if k.is_empty() || k.contains(' ') {
        None
    } else {
        Some(k)
    }
}

/// Replace (or append) top-level keys in header text, keeping every other
/// line, comments included. A replaced key's indented continuation lines go
/// with it; a repeated key keeps only the first, rewritten line.
fn set_header_keys(yaml: &str, pairs: &[(&str, String)], nl: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut done = vec![false; pairs.len()];
    let mut skipping = false;
    for line in yaml.lines() {
        if skipping {
            if line.starts_with([' ', '\t']) {
                continue;
            }
            skipping = false;
        }
        if let Some(i) = top_key(line).and_then(|k| pairs.iter().position(|(p, _)| *p == k)) {
            if !done[i] {
                out.push(format!("{}: {}", pairs[i].0, pairs[i].1));
                done[i] = true;
            }
            skipping = true;
            continue;
        }
        out.push(line.to_string());
    }
    for (i, (k, v)) in pairs.iter().enumerate() {
        if !done[i] {
            out.push(format!("{k}: {v}"));
        }
    }
    let mut joined = out.join(nl);
    joined.push_str(nl);
    joined
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

/// `query` (sealed only) is the exact text a launch hands the agent: the
/// GO with paths expanded, the lock appended, and the launcher rules. The
/// orbitty-needed clerk / cursor-go routes use it, so every route gets the
/// same text. `launchNote` set = sealed but still not launchable.
fn status_json(roots: &Roots, go: &ParsedGo) -> Value {
    let note = seal_note(roots, go);
    let mut v = json!({
        "sealed": note.is_none(),
        "found": true,
        "id": go.id(),
        "file": go.path.display().to_string(),
        "status": go.header.status,
        "legacy": go.legacy,
        "sha256": sha256_hex(go.raw.as_bytes()),
        "workspace": workspace_in(roots, go).display().to_string(),
        "note": note.clone().unwrap_or_default(),
    });
    if note.is_none() {
        let run = resolve_run_in(roots, &go.header, &go.id(), "", herdr_bin_present());
        match push_decision(&run) {
            Err(e) => v["launchNote"] = json!(format!("{} {e}; not launching.", go.id())),
            Ok(push) => {
                let q = launch_text_in(roots, go, push);
                match enforce_cap(&q) {
                    Err(e) => v["launchNote"] = json!(e),
                    Ok(()) => {
                        v["queryChars"] = json!(q.chars().count());
                        v["query"] = json!(q);
                    }
                }
            }
        }
    }
    v
}

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
    parse_text(path, raw)
}

fn parse_text(path: &Path, raw: String) -> Result<ParsedGo, String> {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return Ok(ParsedGo {
            path: path.to_path_buf(),
            yaml: String::new(),
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
        yaml,
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

fn walk_md(dir: &Path, out: &mut Vec<(PathBuf, usize)>, depth: usize) {
    if depth > WALK_LIMIT {
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
            out.push((p, depth));
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
    fn under_buzz_is_canonical_and_case_insensitive() {
        let roots = temp_roots("underbuzz");
        let h = &roots.home;
        for p in [".buzz", ".buzz/PLANS", ".BUZZ/PLANS", ".Buzz/new/sub", "x/../.bUzZ"] {
            assert!(under_buzz(&roots, &h.join(p)), "{p} (no .buzz on disk)");
        }
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("link")).unwrap();
        for p in [".BUZZ/PLANS", ".buzz/./PLANS", "link/PLANS", "nope/../.Buzz/x"] {
            assert!(under_buzz(&roots, &h.join(p)), "{p}");
        }
        for p in [".buzzard", "buzz/PLANS", ".buzz-old", "Orbitty GOs"] {
            assert!(!under_buzz(&roots, &h.join(p)), "{p}");
        }
    }

    #[test]
    fn hand_edited_buzz_plans_dir_falls_back_to_default() {
        let roots = temp_roots("editbuzz");
        let h = roots.home.clone();
        let default = h.join("Orbitty GOs");
        let hs = h.display().to_string();
        for raw in [
            format!("{hs}/.BUZZ/PLANS"),
            "~/.buzz/PLANS".to_string(),
            "~/.Buzz/new/sub".to_string(),
        ] {
            // Hand-edited, not through set_plans_dir.
            write(&settings_path(&roots), &json!({ "plansDir": raw }).to_string());
            assert_eq!(plans_dir(&roots), default, "{raw}");
            assert!(default.is_dir());
            assert!(!h.join(".buzz").exists() && !h.join(".BUZZ").exists() && !h.join(".Buzz").exists(), "{raw}");
            assert_eq!(snapshot(&roots)["plansDir"], json!(default.display().to_string()));
        }
        // With a real ~/.buzz/PLANS: still the default, still read-only.
        write(&h.join(".buzz/PLANS/_GO_OLD_PROMPT.md"), "legacy\n");
        write(&settings_path(&roots), &json!({ "plansDir": format!("{hs}/.BUZZ/PLANS") }).to_string());
        assert_eq!(plans_dir(&roots), default);
        let kids: Vec<_> = std::fs::read_dir(h.join(".buzz/PLANS")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(kids, vec![std::ffi::OsString::from("_GO_OLD_PROMPT.md")]);
        assert_eq!(read_dirs_in(&roots), vec![default.clone(), h.join(".buzz/PLANS")]);
    }

    #[test]
    fn go_seal_refuses_a_file_inside_buzz() {
        let roots = temp_roots("sealbuzz");
        let h = roots.home.clone();
        let text = "---\nid: GO-SEALBUZZ\nstatus: ready\n---\nBODY\n";
        let inside = h.join(".buzz/PLANS/GO-SEALBUZZ.md");
        write(&inside, text);
        for p in [inside.clone(), h.join(".BUZZ/PLANS/GO-SEALBUZZ.md"), h.join("x/../.Buzz/PLANS/GO-SEALBUZZ.md")] {
            let err = seal_file_in(&roots, &p, "matt").unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{}: {err}", p.display());
            assert_eq!(std::fs::read_to_string(&inside).unwrap(), text, "{}", p.display());
        }
        // The same text outside .buzz seals, so the refusal is the guard.
        let outside = plans_dir(&roots).join("GO-SEALBUZZ.md");
        write(&outside, text);
        seal_file_in(&roots, &outside, "matt").unwrap();
        assert_ne!(std::fs::read_to_string(&outside).unwrap(), text);
    }

    /// `<home>` spelled through the /System/Volumes/Data firmlink, or None
    /// when this machine has no such firmlink for the temp dir.
    fn firmlink_home(roots: &Roots) -> Option<PathBuf> {
        let real = std::fs::canonicalize(&roots.home).ok()?;
        let fw = PathBuf::from(format!("/System/Volumes/Data{}", real.display()));
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (std::fs::metadata(&fw).ok()?, std::fs::metadata(&roots.home).ok()?);
        ((a.dev(), a.ino()) == (b.dev(), b.ino())).then_some(fw)
    }

    #[test]
    fn firmlink_spelling_of_buzz_is_refused() {
        let roots = temp_roots("fwbuzz");
        let Some(fw) = firmlink_home(&roots) else {
            eprintln!("no /System/Volumes/Data firmlink; skipped");
            return;
        };
        let h = roots.home.clone();
        // Before ~/.buzz exists.
        assert!(under_buzz(&roots, &fw.join(".buzz/PLANS")));
        assert!(!under_buzz(&roots, &fw.join("Orbitty GOs")));
        assert!(set_plans_dir_in(&roots, &fw.join(".Buzz/PLANS").display().to_string()).is_err());
        assert!(!h.join(".buzz").exists());
        // With ~/.buzz/PLANS on disk.
        let text = "---\nid: GO-FW\nstatus: ready\n---\nBODY\n";
        write(&h.join(".buzz/PLANS/GO-FW.md"), text);
        for p in [fw.join(".buzz/PLANS"), fw.join(".BUZZ/new/sub")] {
            assert!(under_buzz(&roots, &p), "{}", p.display());
            let err = set_plans_dir_in(&roots, &p.display().to_string()).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none());
        write(&settings_path(&roots), &json!({ "plansDir": fw.join(".buzz/PLANS") }).to_string());
        assert_eq!(plans_dir(&roots), h.join("Orbitty GOs"));
        let err = seal_file_in(&roots, &fw.join(".buzz/PLANS/GO-FW.md"), "matt").unwrap_err();
        assert!(err.contains("inside ~/.buzz"), "{err}");
        assert_eq!(std::fs::read_to_string(h.join(".buzz/PLANS/GO-FW.md")).unwrap(), text);
    }

    #[test]
    fn symlink_loop_and_unreadable_parent_fail_closed() {
        use std::os::unix::fs::PermissionsExt;
        struct Unlock(PathBuf);
        impl Drop for Unlock {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let roots = temp_roots("failclosed");
        let h = roots.home.clone();
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        // P3: symlink loop.
        std::os::unix::fs::symlink(h.join("loop"), h.join("loop")).unwrap();
        // P3b: a chmod 000 parent hiding a link to .buzz.
        std::fs::create_dir_all(h.join("locked")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("locked/link")).unwrap();
        std::fs::set_permissions(h.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
        let _unlock = Unlock(h.join("locked"));
        for p in [h.join("loop/PLANS"), h.join("locked/link/PLANS")] {
            assert!(under_buzz(&roots, &p), "{}", p.display());
            let err = set_plans_dir_in(&roots, &p.display().to_string()).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{err}");
            write(&settings_path(&roots), &json!({ "plansDir": p }).to_string());
            assert_eq!(plans_dir(&roots), h.join("Orbitty GOs"));
            assert!(seal_file_in(&roots, &p.join("GO.md"), "matt").unwrap_err().contains("inside ~/.buzz"));
        }
    }

    #[test]
    fn set_plans_dir_requires_an_absolute_path() {
        let roots = temp_roots("relplans");
        for p in ["Orbitty GOs", "./GOs", ".buzz/PLANS", "~nobody/GOs", "~root/.buzz/PLANS"] {
            let err = set_plans_dir_in(&roots, p).unwrap_err();
            assert!(err.contains("absolute"), "{p}: {err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none());
        set_plans_dir_in(&roots, "~/GOs").unwrap();
        assert_eq!(plans_dir(&roots), roots.home.join("GOs"));
    }

    #[test]
    fn set_plans_dir_refuses_buzz_in_any_case() {
        let roots = temp_roots("setbuzz");
        let h = roots.home.clone();
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("buzzlink")).unwrap();
        let tree = |root: &Path| -> Vec<PathBuf> {
            let mut out = Vec::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p.clone());
                    }
                    out.push(p);
                }
            }
            out.sort();
            out
        };
        let before = tree(&h.join(".buzz"));
        let hs = h.display().to_string();
        for p in [
            format!("{hs}/.buzz/PLANS"),
            format!("{hs}/.BUZZ/PLANS"),
            format!("{hs}/.Buzz"),
            "~/.BUZZ/new/sub".to_string(),
            format!("{hs}/x/../.bUzZ/PLANS"),
            format!("{hs}/buzzlink/PLANS"),
        ] {
            let err = set_plans_dir_in(&roots, &p).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{p}: {err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none(), "nothing saved");
        assert_eq!(tree(&h.join(".buzz")), before, "nothing created under .buzz");
        assert!(!h.join(".BUZZ/new").exists());
        let ok = format!("{hs}/.buzzard");
        let snap = set_plans_dir_in(&roots, &ok).unwrap();
        assert_eq!(snap["plansDir"], json!(ok));
        assert_eq!(load_settings(&roots).plans_dir.as_deref(), Some(ok.as_str()));
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
        seal_file_in(&roots, &link, "matt").unwrap();
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
        let sealed = seal_file_in(&roots, &p, "matt").unwrap();
        assert!(seal_note(&roots, &sealed).is_none());
        let raw = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, raw.replace("SENTINEL-LINE", "EDITED-LINE")).unwrap();
        let again = parse_file(&p).unwrap();
        let note = seal_note(&roots, &again).unwrap();
        assert!(note.contains("changed since seal"), "{note}");
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
        let mut allowed = run.clone();
        allowed.push_allowed = true;
        assert_eq!(push_decision(&allowed).unwrap(), true);
        assert!(push_rule(false).contains("NOT allowed"));
        assert!(push_rule(true).contains("allowed for this run"));
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

    fn sealed_go(roots: &Roots, name: &str, header: &str, body: &str) -> PathBuf {
        let p = plans_dir(roots).join(name);
        write(&p, &format!("---\n{header}---\n{body}"));
        seal_file_in(roots, &p, "matt").unwrap();
        p
    }

    #[test]
    fn seal_covers_header_and_lock() {
        let roots = temp_roots("sealhdr");
        let plans = plans_dir(&roots);
        let lock = plans.join("lock.md");
        write(&lock, "LOCK-V1\n");
        let p = sealed_go(
            &roots,
            "go.md",
            "id: GO-HDR\nstatus: ready\npush_allowed: false\ntools: [read, edit]\nrunner: orbitty-cursor\nharness: cursor\nmodel: grok-4.7-high\nlock: \"{PLANS_DIR}/lock.md\"\n",
            "BODY\n",
        );
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        let raw = std::fs::read_to_string(&p).unwrap();
        for (from, to) in [
            ("push_allowed: false", "push_allowed: true"),
            ("tools: [read, edit]", "tools: [read, edit, git_push]"),
            ("runner: orbitty-cursor", "runner: other"),
            ("harness: cursor", "harness: grok"),
            ("model: grok-4.7-high", "model: other"),
            ("lock: \"{PLANS_DIR}/lock.md\"", "lock: \"{PLANS_DIR}/other.md\""),
        ] {
            assert!(raw.contains(from), "{from}");
            std::fs::write(&p, raw.replace(from, to)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("header edit {from:?} must break the seal"));
            assert!(
                note.contains("changed since seal") || note.contains("seal check failed"),
                "{note}"
            );
        }
        std::fs::write(&p, &raw).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        write(&lock, "LOCK-V2\n");
        let note = seal_note(&roots, &parse_file(&p).unwrap()).expect("lock edit breaks the seal");
        assert!(note.contains("changed since seal"), "{note}");
        write(&lock, "LOCK-V1\n");
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        std::fs::remove_file(&lock).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_some());
    }

    #[test]
    fn no_seal_hash_reads_unsealed() {
        let roots = temp_roots("nohash");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-NOHASH\nstatus: sealed\nsealed_at: 2026-09-29T15:40:00-05:00\nsealed_by: matt\n---\nBODY\n",
        );
        let note = seal_note(&roots, &parse_file(&p).unwrap()).unwrap();
        assert!(note.contains("unsealed"), "{note}");
        assert!(!note.contains("changed since seal"), "{note}");
    }

    #[test]
    fn seal_keeps_header_comments_and_body() {
        let roots = temp_roots("comments");
        let p = plans_dir(&roots).join("go.md");
        let body = "# Title\n\nBODY  with  spaces\n";
        write(
            &p,
            &format!(
                "---\n# top comment\nid: GO-CMT\nstatus: ready   # old status\nrunner: orbitty-cursor  # runner note\n# future option (not now)\ntools: [read]\n---\n{body}"
            ),
        );
        seal_file_in(&roots, &p, "Chuck for Matt: night").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        for keep in [
            "# top comment\n",
            "runner: orbitty-cursor  # runner note\n",
            "# future option (not now)\n",
            "tools: [read]\n",
        ] {
            assert!(raw.contains(keep), "lost {keep:?}:\n{raw}");
        }
        assert!(raw.contains("status: sealed\n"), "{raw}");
        assert!(raw.contains("sealed_by: \"Chuck for Matt: night\"\n"), "{raw}");
        assert!(raw.ends_with(&format!("---\n{body}")), "{raw}");
        let g = parse_file(&p).unwrap();
        assert_eq!(g.header.sealed_by.as_deref(), Some("Chuck for Matt: night"));
        assert!(seal_note(&roots, &g).is_none());
        // Resealing is stable: still one seal_hash line, comments intact.
        seal_file_in(&roots, &p, "matt").unwrap();
        let again = std::fs::read_to_string(&p).unwrap();
        assert_eq!(again.matches("seal_hash:").count(), 1, "{again}");
        assert!(again.contains("# future option (not now)\n"));
    }

    #[test]
    fn empty_sealed_by_refuses() {
        let roots = temp_roots("emptyby");
        let p = sealed_go(&roots, "go.md", "id: GO-BY\nstatus: ready\n", "BODY\n");
        let raw = std::fs::read_to_string(&p).unwrap();
        let line = raw.lines().find(|l| l.starts_with("sealed_by:")).unwrap().to_string();
        for blank in ["sealed_by: \"\"", "sealed_by: \"   \"", "sealed_by:"] {
            std::fs::write(&p, raw.replace(&line, blank)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("{blank:?} must not launch"));
            assert!(note.contains("sealed_by"), "{note}");
        }
    }

    #[test]
    fn card_id_match_beats_id_match() {
        let roots = temp_roots("m3");
        let plans = plans_dir(&roots);
        // The id file is sealed and sorts first; the card file is not
        // sealed. Only the card_id-before-id order refuses here.
        write(
            &plans.join("z-card.md"),
            "---\nid: GO-M-THREE\ncard_id: c-3\nstatus: ready\n---\nCARD\n",
        );
        sealed_go(&roots, "b-id.md", "id: GO-M-THREE\nstatus: ready\n", "ID\n");
        match gate_in(&roots, "c-3", "GO-M-THREE", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("card_id file must win: {other:?}"),
        }
        // Without the card match, the sealed id file launches.
        match gate_in(&roots, "c-other", "GO-M-THREE", &[]) {
            Gate::Launch(g) => assert!(g.body.contains("ID")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn yaml_defaults_beat_rules_json() {
        let roots = temp_roots("m6");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\ndefaults:\n  runner: yaml-default\n  harness: manual\n  model: m-default\nrules:\n  - id: other\n    match: { go: \"GO-ELSE*\" }\n    runner: nope\n",
        );
        write(
            &roots.home.join(".buzz/ops/needed-bus/rules.json"),
            r#"{"version":1,"rules":[{"id":"r1","enabled":true,"params":{"flavor":"fact","go":"GO-ORBITTY*","project":"orbitty"},"answer":"grok on this planet"}]}"#,
        );
        let run = resolve_run_in(&roots, &GoHeader::default(), "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "yaml-default");
        assert_eq!(run.harness, "manual");
        assert_eq!(run.model, "m-default");
        assert_eq!(run.source, "RULES.yaml");
    }

    #[test]
    fn go_token_with_no_file_refuses_instead_of_title_launch() {
        let roots = temp_roots("nofile");
        match gate_in(&roots, "c-1", "GO-ORBITTY-NOTHING", &[]) {
            Gate::Refuse { note } => assert!(note.contains("no GO file"), "{note}"),
            other => panic!("must fail closed: {other:?}"),
        }
        // A card that is not a GO still launches with its title.
        assert!(matches!(gate_in(&roots, "c-1", "Fix the drawer", &[]), Gate::Skip));
    }

    #[test]
    fn headerless_go_file_refuses() {
        let roots = temp_roots("nohdr");
        write(
            &plans_dir(&roots).join("GO-ORBITTY-NOHDR_2026-09-29.md"),
            "# GO-ORBITTY-NOHDR\nno header here\n",
        );
        match gate_in(&roots, "", "GO-ORBITTY-NOHDR", &[]) {
            Gate::Refuse { note } => assert!(note.contains("has no GO header"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lowercase_header_id_is_found_and_gated() {
        let roots = temp_roots("lower");
        let plans = plans_dir(&roots);
        write(&plans.join("draft.md"), "---\nid: go-orbitty-lower\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-LOWER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("{other:?}"),
        }
        std::fs::remove_file(plans.join("draft.md")).unwrap();
        sealed_go(&roots, "sealed.md", "id: go-orbitty-lower\nstatus: ready\n", "SEALED\n");
        match gate_in(&roots, "", "GO-ORBITTY-LOWER", &[]) {
            Gate::Launch(g) => assert!(g.body.contains("SEALED")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn go_nested_too_deep_refuses() {
        let roots = temp_roots("deep");
        let plans = plans_dir(&roots);
        let six = plans.join("1/2/3/4/5/6");
        let seven = six.join("7");
        write(&seven.join("go.md"), "---\nid: GO-ORBITTY-DEEP\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-DEEP", &[]) {
            Gate::Refuse { note } => assert!(note.contains("folders deep"), "{note}"),
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&seven).unwrap();
        write(&six.join("go.md"), "---\nid: GO-ORBITTY-DEEP\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-DEEP", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("six deep is still searched: {other:?}"),
        }
    }

    #[test]
    fn malformed_header_without_token_in_name_refuses() {
        let roots = temp_roots("malother");
        write(
            &plans_dir(&roots).join("notes.md"),
            "---\nid: GO-ORBITTY-MAL\nstatus: [\n---\nbody\n",
        );
        match gate_in(&roots, "", "GO-ORBITTY-MAL", &[]) {
            Gate::Refuse { note } => assert!(note.contains("malformed"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn status_query_is_the_full_launch_text() {
        let roots = temp_roots("query");
        let plans = plans_dir(&roots);
        write(&plans.join("lock.md"), "LOCK-SENTINEL {REPO_ROOT}\n");
        let p = sealed_go(
            &roots,
            "go.md",
            "id: GO-Q\nstatus: ready\nworkspace: \"{REPO_ROOT}\"\nlock: \"{PLANS_DIR}/lock.md\"\n",
            "BODY-SENTINEL {PLANS_DIR}\n",
        );
        let v = status_json(&roots, &parse_file(&p).unwrap());
        assert_eq!(v["sealed"], true, "{v}");
        let q = v["query"].as_str().expect("sealed status carries the query");
        let repo = repo_root(&roots).display().to_string();
        assert!(q.contains("BODY-SENTINEL"), "{q}");
        assert!(q.contains(&format!("LOCK-SENTINEL {repo}")), "lock appended + expanded: {q}");
        assert!(q.contains(&plans.display().to_string()), "{q}");
        assert!(!q.contains("{REPO_ROOT}") && !q.contains("{PLANS_DIR}"), "{q}");
        assert!(q.contains(push_rule(false)), "{q}");
        assert_eq!(v["workspace"], repo);
        // Unsealed: no query at all.
        let raw = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, raw.replace("BODY-SENTINEL", "EDITED")).unwrap();
        let v = status_json(&roots, &parse_file(&p).unwrap());
        assert_eq!(v["sealed"], false);
        assert!(v.get("query").is_none(), "{v}");
    }

    #[test]
    fn no_launcher_passes_a_git_push_flag() {
        let flag = ["--git", "-push"].concat();
        for (name, src) in [
            ("coordinator.rs", include_str!("coordinator.rs")),
            ("herdr.rs", include_str!("herdr.rs")),
            ("agents.rs", include_str!("agents.rs")),
        ] {
            assert!(!src.contains(&flag), "{name} still passes {flag}");
        }
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
