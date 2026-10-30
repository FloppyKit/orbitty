//! OS write fence and per-turn revert for companion policy files.
//! The mouth child runs under sandbox-exec. Orbitty still snapshots the
//! policy set around every turn, because a socket write is not that child.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};
use tauri::Manager;

#[derive(Clone, Debug)]
pub struct PolicyRoots {
    pub home: PathBuf,
    pub plans_dir: PathBuf,
    pub app_data: PathBuf,
}

impl PolicyRoots {
    /// Live roots. `plans_dir()` may create the plans folder if it is missing;
    /// it does not write a policy file.
    pub fn real() -> Self {
        let home = crate::plans::Roots::real().home;
        let plans_dir = crate::plans::plans_dir(&crate::plans::Roots {
            home: home.clone(),
        });
        let app_data = home.join("Library/Application Support/space.orbitty.mac");
        Self {
            home,
            plans_dir,
            app_data,
        }
    }

    pub fn from_app(app: &tauri::AppHandle) -> Self {
        let mut roots = Self::real();
        if let Ok(dir) = app.path().app_data_dir() {
            roots.app_data = dir;
        }
        roots
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    File,
    Subpath,
}

#[derive(Clone, Debug)]
pub struct PolicyPath {
    pub canonical: PathBuf,
    pub kind: PathKind,
}

#[derive(Debug)]
pub struct NoSandbox;

/// Files and directories the companion may never write. Missing paths are
/// still denied, so the child cannot create them.
pub fn policy_paths(roots: &PolicyRoots) -> Vec<PolicyPath> {
    let mut out = Vec::new();
    let file = |p: PathBuf| PolicyPath {
        canonical: canonical_path(&p),
        kind: PathKind::File,
    };
    let sub = |p: PathBuf| PolicyPath {
        canonical: canonical_path(&p),
        kind: PathKind::Subpath,
    };
    out.push(file(roots.plans_dir.join("RULES.yaml")));
    out.push(sub(roots.home.join(".config/orbitty")));
    out.push(file(roots.app_data.join("control.json")));
    out.push(file(roots.app_data.join("control.token")));
    out.push(file(roots.app_data.join("flags.json")));
    out.push(file(roots.app_data.join("flags-alpha.json")));
    out.push(file(roots.app_data.join("flags-nightly.json")));
    out.push(file(roots.app_data.join("flags-stable.json")));
    out.push(file(roots.home.join("bin/go-run.sh")));
    out.push(file(roots.home.join("bin/cook-pi.sh")));
    out.push(file(roots.home.join("bin/verify-pi.sh")));
    out.push(file(roots.home.join(".pi/agent/mcp.json")));
    out.push(file(roots.home.join(".buzz/ops/needed-bus/rules.json")));
    out.push(file(roots.home.join(".config/goose/config.yaml")));
    out.push(file(roots.home.join(".config/goose/permission.yaml")));
    out
}

pub fn sandbox_profile(paths: &[PolicyPath], home: &Path) -> String {
    let home = canonical_path(home);
    let mut clauses: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for p in paths {
        let op = match p.kind {
            PathKind::File => "literal",
            PathKind::Subpath => "subpath",
        };
        let clause = format!("({op} \"{}\")", sbpl_escape(&p.canonical.display().to_string()));
        if seen.insert(clause.clone()) {
            clauses.push(clause);
        }
        let mut cur = p.canonical.parent().map(Path::to_path_buf);
        while let Some(dir) = cur {
            let dir = canonical_path(&dir);
            let clause = format!("(literal \"{}\")", sbpl_escape(&dir.display().to_string()));
            if seen.insert(clause.clone()) {
                clauses.push(clause);
            }
            if dir == home || !dir.starts_with(&home) {
                break;
            }
            cur = dir.parent().map(Path::to_path_buf);
        }
    }
    format!(
        "(version 1)(allow default)(deny file-write* {})",
        clauses.join(" ")
    )
}

fn sbpl_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn canonical_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    }
    let mut suffix = Vec::new();
    let mut cur = path;
    loop {
        if cur.exists() {
            let mut out = cur.canonicalize().unwrap_or_else(|_| cur.to_path_buf());
            for part in suffix.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (cur.file_name(), cur.parent()) {
            (Some(name), Some(parent)) if parent != cur => {
                suffix.push(name.to_os_string());
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn sandbox_exec_path() -> PathBuf {
    #[cfg(test)]
    if SANDBOX_MISSING.load(Ordering::SeqCst) {
        return PathBuf::from("/orbitty-no-sandbox-exec");
    }
    PathBuf::from("/usr/bin/sandbox-exec")
}

pub fn sandbox_ready() -> bool {
    mouth_command(Path::new("orbitty-mouth"), &spawn_roots()).is_ok()
}

pub fn mouth_command(bin: &Path, roots: &PolicyRoots) -> Result<Command, NoSandbox> {
    let sb = sandbox_exec_path();
    if !sb.is_file() {
        return Err(NoSandbox);
    }
    let paths = policy_paths(roots);
    if paths.is_empty() {
        return Err(NoSandbox);
    }
    let home = canonical_path(&roots.home);
    if !home.is_dir() {
        return Err(NoSandbox);
    }
    let profile = sandbox_profile(&paths, &home);
    let mut cmd = Command::new(sb);
    cmd.arg("-p").arg(profile).arg(bin);
    apply_bundle_env(&mut cmd);
    Ok(cmd)
}

pub fn bare_mouth(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    apply_bundle_env(&mut cmd);
    cmd
}

fn apply_bundle_env(cmd: &mut Command) {
    let bundle = crate::coordinator::bundled_orbittyctl()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let inherited = std::env::var("PATH").unwrap_or_default();
    let path = if inherited.is_empty() {
        bundle.display().to_string()
    } else {
        format!("{}:{}", bundle.display(), inherited)
    };
    cmd.env("PATH", &path);
    cmd.env("ORBITTY_CTL", bundle.join("orbittyctl"));
    cmd.env("ORBITTY_MCP", bundle.join("orbitty-mcp"));
}

/// Goose / Grok argv when the write fence cannot be built. Idempotent.
pub fn tools_off_args(args: &[String]) -> Vec<String> {
    let grok = args.iter().any(|a| {
        a == "--always-approve"
            || a == "--prompt-json"
            || a == "--output-format"
            || a.starts_with("--single=")
    });
    if grok {
        let mut out = args.to_vec();
        if !out.iter().any(|a| a == "--disallowed-tools") {
            out.push("--disallowed-tools".into());
            out.push("bash,shell,computer".into());
        }
        return out;
    }
    let goose = args.iter().any(|a| {
        a == "run" || a == "--no-profile" || a.starts_with("--with-builtin=")
    });
    if !goose {
        return args.to_vec();
    }
    let mut out: Vec<String> = args
        .iter()
        .map(|a| {
            if a.starts_with("--with-builtin=") {
                "--no-profile".to_string()
            } else {
                a.clone()
            }
        })
        .collect();
    if !out.iter().any(|a| a == "--no-profile") {
        let at = out.iter().position(|a| a == "-q").map(|i| i + 1).unwrap_or(0);
        out.insert(at, "--no-profile".into());
    }
    out
}

pub struct PreparedMouth {
    pub cmd: Command,
    pub args: Vec<String>,
    pub unsandboxed: bool,
}

pub fn prepare_mouth(bin: &Path, args: &[String]) -> PreparedMouth {
    let roots = spawn_roots();
    match mouth_command(bin, &roots) {
        Ok(cmd) => PreparedMouth {
            cmd,
            args: args.to_vec(),
            unsandboxed: false,
        },
        Err(NoSandbox) => PreparedMouth {
            cmd: bare_mouth(bin),
            args: tools_off_args(args),
            unsandboxed: true,
        },
    }
}

pub fn prepare_hermes(bin: &Path) -> Result<Command, String> {
    mouth_command(bin, &spawn_roots()).map_err(|_| {
        "Hermes tools need the write fence (sandbox-exec missing)".to_string()
    })
}

const NO_SANDBOX_NOTICE: &str =
    "Write fence unavailable (sandbox-exec missing); tools are off.";

static SAID_NO_SANDBOX: AtomicBool = AtomicBool::new(false);

pub fn note_unsandboxed(result: Result<String, String>, unsandboxed: bool) -> Result<String, String> {
    if !unsandboxed || SAID_NO_SANDBOX.swap(true, Ordering::SeqCst) {
        return result;
    }
    match result {
        Ok(s) => Ok(format!("{s}\n\n{NO_SANDBOX_NOTICE}")),
        Err(e) => Err(format!("{e}\n\n{NO_SANDBOX_NOTICE}")),
    }
}

pub fn spawn_roots() -> PolicyRoots {
    #[cfg(test)]
    if let Some(roots) = test_roots_slot().lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return roots;
    }
    PolicyRoots::real()
}

#[cfg(test)]
static SANDBOX_MISSING: AtomicBool = AtomicBool::new(false);

fn test_roots_slot() -> &'static std::sync::Mutex<Option<PolicyRoots>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<PolicyRoots>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub fn with_roots<T>(roots: PolicyRoots, f: impl FnOnce() -> T) -> T {
    with_override(Some(roots), false, f)
}

#[cfg(test)]
pub fn with_override<T>(roots: Option<PolicyRoots>, missing: bool, f: impl FnOnce() -> T) -> T {
    let _g = test_lock();
    SANDBOX_MISSING.store(missing, Ordering::SeqCst);
    *test_roots_slot().lock().unwrap_or_else(|e| e.into_inner()) = roots;
    let out = f();
    SANDBOX_MISSING.store(false, Ordering::SeqCst);
    *test_roots_slot().lock().unwrap_or_else(|e| e.into_inner()) = None;
    out
}

#[derive(Clone, Debug)]
struct SnapEntry {
    path: PathBuf,
    name: String,
    present: bool,
    bytes: Vec<u8>,
}

pub struct Snapshot {
    entries: Vec<SnapEntry>,
}

#[derive(Clone, Debug)]
pub struct Reverted {
    pub name: String,
}

impl Snapshot {
    pub fn take(roots: &PolicyRoots) -> Self {
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for (path, name) in tracked_files(roots) {
            if !seen.insert(path.clone()) {
                continue;
            }
            let (present, bytes) = match std::fs::read(&path) {
                Ok(b) => (true, b),
                Err(_) => (false, Vec::new()),
            };
            entries.push(SnapEntry {
                path,
                name,
                present,
                bytes,
            });
        }
        Self { entries }
    }

    pub fn check_and_restore(&self, roots: &PolicyRoots) -> Vec<Reverted> {
        self.check_and_restore_at(roots, &local_stamp())
    }

    pub fn check_and_restore_at(&self, roots: &PolicyRoots, stamp: &str) -> Vec<Reverted> {
        let mut reverted = Vec::new();
        let mut seen = BTreeSet::new();
        for ent in &self.entries {
            seen.insert(ent.path.clone());
            if let Some(name) = restore_one(ent, roots, stamp) {
                reverted.push(Reverted { name });
            }
        }
        let cfg = roots.home.join(".config/orbitty");
        if let Ok(rd) = std::fs::read_dir(&cfg) {
            for item in rd.flatten() {
                let path = item.path();
                if seen.contains(&path) || !path.is_file() {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "file".into());
                let ghost = SnapEntry {
                    path,
                    name,
                    present: false,
                    bytes: Vec::new(),
                };
                if let Some(name) = restore_one(&ghost, roots, stamp) {
                    reverted.push(Reverted { name });
                }
            }
        }
        reverted
    }
}

fn tracked_files(roots: &PolicyRoots) -> Vec<(PathBuf, String)> {
    let mut v = vec![
        (roots.plans_dir.join("RULES.yaml"), "RULES.yaml".into()),
        (roots.app_data.join("control.json"), "control.json".into()),
        (roots.app_data.join("control.token"), "control.token".into()),
        (roots.home.join("bin/go-run.sh"), "go-run.sh".into()),
        (roots.home.join("bin/cook-pi.sh"), "cook-pi.sh".into()),
        (roots.home.join("bin/verify-pi.sh"), "verify-pi.sh".into()),
        (roots.home.join(".pi/agent/mcp.json"), "mcp.json".into()),
        (
            roots.home.join(".buzz/ops/needed-bus/rules.json"),
            "rules.json".into(),
        ),
        (
            roots.home.join(".config/goose/config.yaml"),
            "config.yaml".into(),
        ),
        (
            roots.home.join(".config/goose/permission.yaml"),
            "permission.yaml".into(),
        ),
        (
            roots.home.join(".config/orbitty/settings.json"),
            "settings.json".into(),
        ),
        (
            roots.home.join(".config/orbitty/go-runner.json"),
            "go-runner.json".into(),
        ),
        (
            roots.home.join(".config/orbitty/mouth-sidecar.json"),
            "mouth-sidecar.json".into(),
        ),
        (
            roots.home.join(".config/orbitty/go-user.json"),
            "go-user.json".into(),
        ),
    ];
    let cfg = roots.home.join(".config/orbitty");
    if let Ok(rd) = std::fs::read_dir(&cfg) {
        for item in rd.flatten() {
            let path = item.path();
            if path.is_file() && !v.iter().any(|(p, _)| p == &path) {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "file".into());
                v.push((path, name));
            }
        }
    }
    v
}

fn restore_one(ent: &SnapEntry, roots: &PolicyRoots, stamp: &str) -> Option<String> {
    let now = std::fs::read(&ent.path).ok();
    let changed = match (&now, ent.present) {
        (Some(b), true) => b != &ent.bytes,
        (Some(_), false) => true,
        (None, true) => true,
        (None, false) => false,
    };
    if !changed {
        return None;
    }
    if is_runner_selection(&ent.path, roots) {
        if let Some(b) = &now {
            if approved_runner_json(b) {
                return None;
            }
        }
    }
    let new_bytes = now.unwrap_or_default();
    save_copy(roots, stamp, &ent.name, &new_bytes);
    if ent.present {
        let _ = write_atomic(&ent.path, &ent.bytes);
    } else if ent.path.exists() {
        let _ = std::fs::remove_file(&ent.path);
    }
    Some(ent.name.clone())
}

fn is_runner_selection(path: &Path, roots: &PolicyRoots) -> bool {
    path == roots.home.join(".config/orbitty/go-runner.json")
}

fn approved_runner_json(bytes: &[u8]) -> bool {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    let Some(obj) = v.as_object() else {
        return false;
    };
    if obj.len() != 2 || !obj.contains_key("v") || !obj.contains_key("runner") {
        return false;
    }
    if obj.get("v").and_then(|x| x.as_u64()) != Some(1) {
        return false;
    }
    let Some(id) = obj.get("runner").and_then(|x| x.as_str()) else {
        return false;
    };
    if id.trim().is_empty() {
        return false;
    }
    let settings = crate::plans::load_user_settings(&crate::plans::Roots::real());
    settings.approved_runners.iter().any(|r| r.id == id)
}

fn save_copy(roots: &PolicyRoots, stamp: &str, name: &str, bytes: &[u8]) {
    let dir = roots.plans_dir.join("_policy-guard");
    let _ = std::fs::create_dir_all(&dir);
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let mut path = dir.join(format!("{stamp}-{safe}"));
    if path.exists() {
        path = dir.join(format!("{stamp}-{safe}-{}", std::process::id()));
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    if let Ok(mut f) = opts.open(&path) {
        let _ = f.write_all(bytes);
        let _ = f.sync_all();
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("restore-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e
    })
}

fn local_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

pub fn correction_sentence(names: &[String]) -> String {
    format!(
        "Orbitty undid a change to {}. Only Matt can change Go permissions or runners, in Settings → Developer mode (a later update).",
        names.join(", ")
    )
}

pub fn needed_title(names: &[String]) -> String {
    format!(
        "Companion tried to change {}; Orbitty undid it",
        names.join(", ")
    )
}

pub const NEEDED_WHY: &str =
    "Copies in ~/Orbitty GOs/_policy-guard/. Only Matt changes Go permissions or runners.";

/// Snapshot, run `inner`, restore, and tell the sinks. The production send
/// path and the guard tests share this so a skipped restore fails the test.
pub fn apply_guard(
    roots: &PolicyRoots,
    stamp: &str,
    needed: impl FnMut(&str),
    event: impl FnMut(&serde_json::Value),
    inner: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let snap = Snapshot::take(roots);
    let result = inner();
    finish_guard_at(&snap, roots, stamp, result, needed, event)
}

pub fn finish_guard(
    snap: &Snapshot,
    roots: &PolicyRoots,
    result: Result<String, String>,
    mut needed: impl FnMut(&str),
    mut event: impl FnMut(&serde_json::Value),
) -> Result<String, String> {
    let reverted = snap.check_and_restore(roots);
    if reverted.is_empty() {
        return result;
    }
    let names: Vec<String> = reverted.into_iter().map(|r| r.name).collect();
    event(&serde_json::json!({ "files": names, "n": names.len() }));
    needed(&needed_title(&names));
    let sentence = correction_sentence(&names);
    match result {
        Ok(s) => Ok(format!("{s}\n\n{sentence}")),
        Err(e) => Err(format!("{e}\n\n{sentence}")),
    }
}

fn finish_guard_at(
    snap: &Snapshot,
    roots: &PolicyRoots,
    stamp: &str,
    result: Result<String, String>,
    mut needed: impl FnMut(&str),
    mut event: impl FnMut(&serde_json::Value),
) -> Result<String, String> {
    let reverted = snap.check_and_restore_at(roots, stamp);
    if reverted.is_empty() {
        return result;
    }
    let names: Vec<String> = reverted.into_iter().map(|r| r.name).collect();
    event(&serde_json::json!({ "files": names, "n": names.len() }));
    needed(&needed_title(&names));
    let sentence = correction_sentence(&names);
    match result {
        Ok(s) => Ok(format!("{s}\n\n{sentence}")),
        Err(e) => Err(format!("{e}\n\n{sentence}")),
    }
}

#[cfg(test)]
pub fn temp_policy_home(label: &str) -> (PathBuf, PolicyRoots) {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir().join(format!("orbitty-cnpw-{label}-{n}"));
    let home = base.join("home");
    let plans_dir = home.join("Orbitty GOs");
    let app_data = home.join("Library/Application Support/space.orbitty.mac");
    std::fs::create_dir_all(&plans_dir).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();
    std::fs::create_dir_all(home.join(".config/orbitty")).unwrap();
    (
        base,
        PolicyRoots {
            home,
            plans_dir,
            app_data,
        },
    )
}

#[cfg(test)]
fn sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[cfg(test)]
pub const RULES_FIXTURE: &str = "\
version: 1
defaults:
  harness: manual
  tools: [read]
  push_allowed: false
legacy_allow: []
rules:
  - id: orbitty-default-2026-09-29
    match: { go: \"GO-ORBITTY*\", project: orbitty }
    runner: orbitty-cursor
    harness: cursor
    model: grok-4.7-high
    runner_fallback:
      runner: herdr-probe
      harness: herdr
      model: grok
    tools: [read, edit, shell, git_commit, install_nightly, orbittyctl]
    note: \"Blank header fields use this row. The model is defaultRunner (grok-4.7-high), not a kimi profile. Fallback is Grok via herdr when Cursor cannot run.\"
";

#[cfg(test)]
pub fn d4_edit_script() -> &'static str {
    r#"#!/usr/bin/env python3
import pathlib, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text()
old = "tools: [read, edit, shell, git_commit, install_nightly, orbittyctl]"
new = "tools: [read, edit, shell, git_commit, git_push, install_nightly, orbittyctl]"
if old not in text:
    sys.stderr.write("fixture missing tools line\n")
    sys.exit(2)
text = text.replace(old, new + "\n    push_allowed: true", 1)
text = text.replace(
    'note: "Blank header fields use this row.',
    'note: "Matt 2026-10-21: allow push for Go.',
    1,
)
try:
    p.write_text(text)
except OSError as e:
    sys.stderr.write(f"Operation not permitted: {e}\n")
    sys.exit(1)
print("wrote", p)
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_exe(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(path, perm).unwrap();
    }

    #[test]
    fn fixture_matches_d4_orig() {
        assert_eq!(
            sha256(RULES_FIXTURE.as_bytes()),
            "c7c769d225fb148f70d0d37fb2f2ed59dc7c6447ca1afcfde34c72d8d52d368c"
        );
    }

    #[test]
    fn profile_paths_are_canonical() {
        let (base, roots) = temp_policy_home("profile");
        let paths = policy_paths(&roots);
        let profile = sandbox_profile(&paths, &roots.home);
        assert!(
            !profile.contains("\"/var/") && !profile.contains("\"/tmp/"),
            "{profile}"
        );
        assert!(profile.contains("/private/"), "{profile}");
        assert!(profile.contains("Orbitty GOs"), "{profile}");
        assert_eq!(sbpl_escape("a\"b\\c"), "a\\\"b\\\\c");
        let quoted = PolicyPath {
            canonical: roots.home.join("say \"hi\""),
            kind: PathKind::File,
        };
        let with_quote = sandbox_profile(&[quoted], &roots.home);
        assert!(with_quote.contains("\\\"hi\\\""), "{with_quote}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mouth_write_rules_yaml_refused() {
        let (base, roots) = temp_policy_home("mouth");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let before = sha256(&std::fs::read(&rules).unwrap());
        let script = base.join("mouth.py");
        write_exe(&script, d4_edit_script());
        let out = mouth_command(&script, &roots)
            .expect("sandbox-exec")
            .arg(&rules)
            .output()
            .expect("spawn");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "stdout {}", String::from_utf8_lossy(&out.stdout));
        assert!(
            err.to_ascii_lowercase().contains("not permitted") || err.contains("Permission"),
            "{err}"
        );
        assert_eq!(sha256(&std::fs::read(&rules).unwrap()), before);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_matrix_refused() {
        let (base, roots) = temp_policy_home("matrix");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let notes = roots.plans_dir.join("notes.md");
        std::fs::write(&notes, "keep\n").unwrap();
        let cfg = roots.home.join(".config/orbitty");
        std::fs::create_dir_all(&cfg).unwrap();
        let settings = cfg.join("settings.json");
        std::fs::write(&settings, "{}\n").unwrap();
        std::fs::create_dir_all(&roots.app_data).unwrap();
        let control = roots.app_data.join("control.json");
        std::fs::write(&control, "{}\n").unwrap();
        let outside = base.join("outside.txt");
        let before_rules = std::fs::read(&rules).unwrap();
        let rules = rules.canonicalize().unwrap();
        let plans = roots.plans_dir.canonicalize().unwrap();
        let parent = roots.home.canonicalize().unwrap();
        let cfg = cfg.canonicalize().unwrap();
        let control = control.canonicalize().unwrap();
        let notes = notes.canonicalize().unwrap();
        let script = base.join("matrix.sh");
        write_exe(
            &script,
            r#"#!/bin/sh
rules="$1"; plans="$2"; parent="$3"; cfg="$4"; control="$5"; notes="$6"; outside="$7"
try() {
  name="$1"; shift
  if "$@" >/tmp/cnpw-matrix-out 2>/tmp/cnpw-matrix-err; then
    echo "ALLOWED $name"
  else
    echo "REFUSED $name"
  fi
}
try append sh -c "printf x >> \"\$1\"" sh "$rules"
try mvover sh -c "t=\$(mktemp); printf hacked > \"\$t\"; mv \"\$t\" \"\$1\"" sh "$rules"
try rm rm "$rules"
try mvplans mv "$plans" "$plans.moved"
try mvparent mv "$parent" "$parent.moved"
try ln ln "$rules" "$plans/rules.hl"
try chmod chmod 600 "$rules"
try touch touch "$rules"
try gorunner sh -c "printf '{}' > \"\$1/go-runner.json\"" sh "$cfg"
try settings sh -c "printf '{\"x\":1}' > \"\$1\"" sh "$settings"
try control sh -c "printf '{\"x\":1}' > \"\$1\"" sh "$control"
try notes sh -c "printf 'sibling\n' >> \"\$1\"" sh "$notes"
try outside sh -c "printf 'tmp\n' > \"\$1\"" sh "$outside"
"#,
        );
        let out = mouth_command(&script, &roots)
            .expect("sandbox-exec")
            .args([
                rules.as_os_str(),
                plans.as_os_str(),
                parent.as_os_str(),
                cfg.as_os_str(),
                control.as_os_str(),
                notes.as_os_str(),
                outside.as_os_str(),
            ])
            .output()
            .expect("spawn");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "stderr {}", String::from_utf8_lossy(&out.stderr));
        for name in [
            "append", "mvover", "rm", "mvplans", "mvparent", "ln", "chmod", "touch", "gorunner",
            "settings", "control",
        ] {
            assert!(
                text.contains(&format!("REFUSED {name}")),
                "{name} in:\n{text}"
            );
        }
        assert!(text.contains("ALLOWED notes"), "{text}");
        assert!(text.contains("ALLOWED outside"), "{text}");
        assert_eq!(std::fs::read(&rules).unwrap(), before_rules);
        assert!(std::fs::read_to_string(&notes).unwrap().contains("sibling"));
        assert!(std::fs::read_to_string(&outside).unwrap().contains("tmp"));
        assert!(roots.plans_dir.is_dir());
        assert!(roots.home.is_dir());
        assert!(!cfg.join("go-runner.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn snapshot_restores_and_saves_copy() {
        let (base, roots) = temp_policy_home("snap");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let snap = Snapshot::take(&roots);
        std::fs::write(&rules, b"changed\n").unwrap();
        let reverted = snap.check_and_restore_at(&roots, "20261021-235959");
        assert_eq!(reverted.len(), 1);
        assert_eq!(reverted[0].name, "RULES.yaml");
        assert_eq!(std::fs::read(&rules).unwrap(), RULES_FIXTURE.as_bytes());
        let copy = roots.plans_dir.join("_policy-guard/20261021-235959-RULES.yaml");
        assert_eq!(std::fs::read(&copy).unwrap(), b"changed\n");
        assert_eq!(std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn runner_json_approved_kept_offlist_reverted() {
        let (base, roots) = temp_policy_home("runner");
        let path = roots.home.join(".config/orbitty/go-runner.json");
        let snap = Snapshot::take(&roots);
        std::fs::write(&path, "{\"v\":1,\"runner\":\"pi-grok\"}\n").unwrap();
        let kept = snap.check_and_restore_at(&roots, "20261021-235958");
        assert!(kept.is_empty(), "{kept:?}");
        assert!(std::fs::read_to_string(&path).unwrap().contains("pi-grok"));
        let snap2 = Snapshot::take(&roots);
        std::fs::write(&path, "{\"v\":1,\"runner\":\"nope\"}\n").unwrap();
        let reverted = snap2.check_and_restore_at(&roots, "20261021-235957");
        assert_eq!(reverted.len(), 1);
        assert!(std::fs::read_to_string(&path).unwrap().contains("pi-grok"));
        assert!(!std::fs::read_to_string(&path).unwrap().contains("nope"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn guard_reverts_unsandboxed_write() {
        let (base, roots) = temp_policy_home("guard");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let mut needed = 0usize;
        let mut events = 0usize;
        let out = apply_guard(
            &roots,
            "20261021-120000",
            |_| needed += 1,
            |v| {
                events += 1;
                assert!(v.get("files").is_some());
                assert!(v.get("n").is_some());
                assert!(v.as_object().unwrap().len() == 2);
            },
            || {
                std::fs::write(&rules, b"hacked\n").unwrap();
                Ok("Push for Go is on.".into())
            },
        )
        .unwrap();
        assert_eq!(needed, 1);
        assert_eq!(events, 1);
        assert!(out.ends_with(&correction_sentence(&["RULES.yaml".into()])));
        assert!(out.starts_with("Push for Go is on."));
        assert_eq!(std::fs::read(&rules).unwrap(), RULES_FIXTURE.as_bytes());
        assert!(roots.plans_dir.join("_policy-guard/20261021-120000-RULES.yaml").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn guard_on_err_path() {
        let (base, roots) = temp_policy_home("guarderr");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let mut needed = 0usize;
        let err = apply_guard(
            &roots,
            "20261021-120001",
            |_| needed += 1,
            |_| {},
            || {
                std::fs::write(&rules, b"hacked\n").unwrap();
                Err("Goose took longer than 300s — killed. Try again.".into())
            },
        )
        .unwrap_err();
        assert_eq!(needed, 1);
        assert!(err.contains("300s"));
        assert!(err.ends_with(&correction_sentence(&["RULES.yaml".into()])));
        assert_eq!(std::fs::read(&rules).unwrap(), RULES_FIXTURE.as_bytes());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn guard_noop_when_unchanged() {
        let (base, roots) = temp_policy_home("guardnoop");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, RULES_FIXTURE).unwrap();
        let mut needed = 0usize;
        let out = apply_guard(
            &roots,
            "20261021-120002",
            |_| needed += 1,
            |_| panic!("no event"),
            || Ok("same".into()),
        )
        .unwrap();
        assert_eq!(needed, 0);
        assert_eq!(out, "same");
        assert!(!roots.plans_dir.join("_policy-guard").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn flags_json_is_policy() {
        let (base, roots) = temp_policy_home("flagsjson");
        let paths = policy_paths(&roots);
        for name in [
            "flags.json",
            "flags-alpha.json",
            "flags-nightly.json",
            "flags-stable.json",
        ] {
            assert!(
                paths.iter().any(|p| p.canonical.ends_with(name)),
                "{name} missing from policy paths"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore]
    fn print_real_profile() {
        let roots = PolicyRoots::real();
        let paths = policy_paths(&roots);
        println!("{}", sandbox_profile(&paths, &roots.home));
    }
}

