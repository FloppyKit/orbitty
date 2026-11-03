//! Raycast-format script commands from `~/.config/orbitty/scripts/`.
//!
//! Discovery reads at most the first 4 KiB of each top-level file and
//! parses the `@raycast.*` header comments. Nothing is ever executed by
//! the scan: a script runs only when Matt picks its row. A run is a
//! direct spawn (never a shell), so the typed argument is exactly one
//! argv entry. The folder is user-owned; the app only reads it.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Header budget: Raycast scripts keep metadata in the first 4 KiB.
pub const HEADER_BYTES: u64 = 4096;
/// A huge folder must not stall the mouth.
pub const MAX_FILES: usize = 200;
/// One run, then the kill.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(30);
/// Each stream is capped at 64 KiB so a chatty script cannot flood.
pub const OUTPUT_CAP: usize = 64 * 1024;

const MODES: [&str; 4] = ["silent", "compact", "fullOutput", "inline"];

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// `~/.config/orbitty/scripts/` — read only, never created here.
pub fn scripts_dir() -> PathBuf {
    home_dir().join(".config").join("orbitty").join("scripts")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptArgument {
    /// `text` or `password`.
    pub kind: String,
    pub placeholder: Option<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCommand {
    /// Absolute path: the row id and the only thing the run command takes.
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// Emoji only in v1; file paths / URLs are ignored.
    pub icon: Option<String>,
    pub mode: String,
    pub argument1: Option<ScriptArgument>,
    pub needs_confirmation: bool,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRun {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default)]
struct Meta {
    title: String,
    package: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    mode: String,
    argument1: Option<ScriptArgument>,
    needs_confirmation: bool,
    cwd: Option<String>,
}

/// Header keys, first value wins (same table as the TS mirror).
fn header_map(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_start();
        let rest = if let Some(r) = line.strip_prefix("//") {
            r
        } else if let Some(r) = line.strip_prefix('#') {
            r
        } else {
            continue;
        };
        let body = rest.trim_start();
        let Some(meta) = body.strip_prefix("@raycast.") else {
            continue;
        };
        let mut parts = meta.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if key.is_empty() || out.iter().any(|(k, _)| k == key) {
            continue;
        }
        out.push((key.to_string(), value.to_string()));
    }
    out
}

fn truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

/// Emoji only: a non-ASCII first token with no path separators. Anything
/// that looks like a file path or URL is ignored in v1.
fn emoji_only(raw: &str) -> Option<String> {
    let token = raw.split_whitespace().next()?;
    if token.contains('/') || token.contains('\\') {
        return None;
    }
    let first = token.chars().next()?;
    if first.is_ascii() {
        return None;
    }
    Some(token.to_string())
}

fn parse_argument(raw: &str) -> Result<ScriptArgument, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("argument1 is not JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "argument1 is not an object".to_string())?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if kind != "text" && kind != "password" {
        return Err(format!("argument1 type {kind:?} is not text/password"));
    }
    Ok(ScriptArgument {
        kind,
        placeholder: obj
            .get("placeholder")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        optional: obj.get("optional").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

fn parse_meta(text: &str) -> Result<Meta, String> {
    let keys = header_map(text);
    let get = |name: &str| -> Option<&str> {
        keys.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    if get("schemaVersion") != Some("1") {
        return Err("missing @raycast.schemaVersion 1".into());
    }
    let title = get("title").unwrap_or("").trim().to_string();
    if title.is_empty() {
        return Err("missing @raycast.title".into());
    }
    let mode = get("mode").unwrap_or("").trim().to_string();
    if !MODES.contains(&mode.as_str()) {
        return Err(format!("mode {mode:?} is not silent/compact/fullOutput/inline"));
    }
    if let Some(platform) = get("platform") {
        if !platform.to_ascii_lowercase().contains("macos") {
            return Err(format!("platform {platform:?} is not macOS"));
        }
    }
    let argument1 = match get("argument1") {
        None => None,
        Some(raw) => match parse_argument(raw) {
            Ok(arg) => Some(arg),
            Err(reason) => {
                // A bad argument does not hide the row: it is listed
                // without an argument and the log says why.
                eprintln!("scripts: argument1 ignored: {reason}");
                None
            }
        },
    };
    Ok(Meta {
        title,
        package: get("packageName").map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
        description: get("description").map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
        icon: get("icon").and_then(emoji_only),
        mode,
        argument1,
        needs_confirmation: get("needsConfirmation").is_some_and(truthy),
        cwd: get("currentDirectoryPath").map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
    })
}

fn read_header(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut buf = Vec::new();
    file.take(HEADER_BYTES)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read: {e}"))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Names the scan never opens.
pub fn skip_name(name: &str) -> Option<&'static str> {
    if name.starts_with('.') {
        return Some("hidden");
    }
    if name.contains(".template.") {
        return Some("template");
    }
    None
}

fn folder_of(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Header → row. No execution, no writes.
fn read_command(path: &Path) -> Result<ScriptCommand, String> {
    let meta = parse_meta(&read_header(path)?)?;
    let folder = folder_of(path);
    let subtitle = meta
        .package
        .or(meta.description)
        .or_else(|| Some(folder).filter(|f| !f.is_empty()));
    Ok(ScriptCommand {
        id: path.to_string_lossy().into_owned(),
        title: meta.title,
        subtitle,
        icon: meta.icon,
        mode: meta.mode,
        argument1: meta.argument1,
        needs_confirmation: meta.needs_confirmation,
        cwd: meta.cwd,
    })
}

/// Every valid command directly in `dir` (no recursion), title order.
/// Malformed files are skipped with one log line each.
pub fn list_scripts_in(dir: &Path) -> Vec<ScriptCommand> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<ScriptCommand> = Vec::new();
    let mut examined = 0usize;
    for entry in entries.flatten() {
        if examined >= MAX_FILES {
            break;
        }
        examined += 1;
        // A symlink is not a regular file in the folder; skipping it here
        // keeps discovery and the run's canonical-path guard in step.
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        if skip_name(&name).is_some() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() || !is_executable(&meta) {
            continue;
        }
        match read_command(&path) {
            Ok(cmd) => out.push(cmd),
            Err(reason) => eprintln!("scripts: skip {name}: {reason}"),
        }
    }
    out.sort_by(|a, b| {
        a.title
            .to_ascii_lowercase()
            .cmp(&b.title.to_ascii_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

fn expand_home(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    if raw == "~" {
        return home_dir();
    }
    PathBuf::from(raw)
}

fn read_capped<R: Read>(mut reader: R) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let room = OUTPUT_CAP.saturating_sub(out.len());
                if room == 0 {
                    truncated = true;
                    continue;
                }
                let take = n.min(room);
                out.extend_from_slice(&buf[..take]);
                if take < n {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (out, truncated)
}

/// Run one picked script. Absolute path only, inside `dir` only; the
/// argument is one argv entry (no shell), cwd is `currentDirectoryPath`
/// or the script's own folder, the run dies at `timeout`.
pub fn run_script_with(
    dir: &Path,
    id: &str,
    args: Option<&str>,
    timeout: Duration,
) -> Result<ScriptRun, String> {
    let root = std::fs::canonicalize(dir).map_err(|_| "Scripts folder is missing.".to_string())?;
    let path = std::fs::canonicalize(id).map_err(|_| format!("Script not found: {id}"))?;
    if path.parent() != Some(root.as_path()) {
        return Err("Script is not in the scripts folder.".into());
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("script")
        .to_string();
    if skip_name(&name).is_some() {
        return Err("That file is not a script command.".into());
    }
    let stat = std::fs::metadata(&path).map_err(|e| format!("stat {name}: {e}"))?;
    if !stat.is_file() || !is_executable(&stat) {
        return Err("That file is not executable.".into());
    }
    let meta = parse_meta(&read_header(&path)?)?;
    let cwd = meta
        .cwd
        .as_deref()
        .map(expand_home)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| root.clone());

    let mut cmd = Command::new(&path);
    if let Some(arg) = args.filter(|a| !a.is_empty()) {
        // One argv entry. Never a shell string, never interpolated.
        cmd.arg(arg);
    }
    cmd.current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("launch {name}: {e}"))?;

    let (tx, rx) = std::sync::mpsc::channel::<(bool, Vec<u8>, bool)>();
    if let Some(mut out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let (buf, trunc) = read_capped(&mut out);
            let _ = tx.send((false, buf, trunc));
        });
    }
    if let Some(mut err) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let (buf, trunc) = read_capped(&mut err);
            let _ = tx.send((true, buf, trunc));
        });
    }
    drop(tx);

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wait on {name}: {e}"));
            }
        }
    };

    // A grandchild may hold a pipe open after the kill; never wait forever.
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut truncated = false;
    for _ in 0..2 {
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok((is_err, buf, trunc)) => {
                truncated |= trunc;
                let text = String::from_utf8_lossy(&buf).into_owned();
                if is_err {
                    stderr = text;
                } else {
                    stdout = text;
                }
            }
            Err(_) => break,
        }
    }
    Ok(ScriptRun {
        code: status.and_then(|s| s.code()).unwrap_or(-1),
        stdout,
        stderr,
        timed_out,
        truncated,
    })
}

/// Discovery only — async + spawn_blocking so a slow disk never freezes
/// the mouth (a sync Tauri command runs on the main thread).
#[tauri::command]
pub async fn list_script_commands() -> Result<Vec<ScriptCommand>, String> {
    tauri::async_runtime::spawn_blocking(|| list_scripts_in(&scripts_dir()))
        .await
        .map_err(|e| format!("script scan task: {e}"))
}

/// Runs only the row Matt picked. Never called by the scan.
#[tauri::command]
pub async fn run_script_command(id: String, args: Option<String>) -> Result<ScriptRun, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_script_with(&scripts_dir(), &id, args.as_deref(), RUN_TIMEOUT)
    })
    .await
    .map_err(|e| format!("script task: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tmpdir(tag: &str) -> PathBuf {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("orbitty-script-{tag}-{}-{n}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_exe(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(path, perm).unwrap();
    }

    fn write_plain(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o644);
        std::fs::set_permissions(path, perm).unwrap();
    }

    const HELLO: &str = "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Hello 27p-c\n# @raycast.mode compact\n# @raycast.icon \u{1f44b}\n# @raycast.packageName Test\necho hi\n";

    #[test]
    fn hash_header_parses() {
        let cmd = read_command(&{
            let dir = tmpdir("hash");
            let p = dir.join("hello.sh");
            write_exe(&p, HELLO);
            p
        })
        .unwrap();
        assert_eq!(cmd.title, "Hello 27p-c");
        assert_eq!(cmd.mode, "compact");
        assert_eq!(cmd.icon.as_deref(), Some("\u{1f44b}"));
        assert_eq!(cmd.subtitle.as_deref(), Some("Test"));
        assert!(!cmd.needs_confirmation);
        assert!(cmd.argument1.is_none());
    }

    #[test]
    fn slash_header_parses() {
        let meta = parse_meta(
            "// @raycast.schemaVersion 1\n// @raycast.title Two\n// @raycast.mode silent\n",
        )
        .unwrap();
        assert_eq!(meta.title, "Two");
        assert_eq!(meta.mode, "silent");
    }

    #[test]
    fn required_keys_missing_are_skipped() {
        assert!(parse_meta("# @raycast.title X\n# @raycast.mode compact\n").is_err());
        assert!(parse_meta("# @raycast.schemaVersion 1\n# @raycast.mode compact\n").is_err());
        assert!(parse_meta("# @raycast.schemaVersion 1\n# @raycast.title X\n").is_err());
        assert!(parse_meta("# @raycast.schemaVersion 2\n# @raycast.title X\n# @raycast.mode compact\n").is_err());
        assert!(parse_meta(
            "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode nope\n"
        )
        .is_err());
    }

    #[test]
    fn template_and_hidden_and_non_exec_are_skipped() {
        let dir = tmpdir("skip");
        write_exe(&dir.join("a.template.sh"), HELLO);
        write_exe(&dir.join(".hidden.sh"), HELLO);
        write_plain(&dir.join("plain.sh"), HELLO);
        write_exe(&dir.join("good.sh"), HELLO);
        std::os::unix::fs::symlink(dir.join("good.sh"), dir.join("link.sh")).unwrap();
        let listed = list_scripts_in(&dir);
        assert_eq!(listed.len(), 1);
        assert!(listed[0].id.ends_with("good.sh"));
    }

    #[test]
    fn binary_junk_and_long_header_are_skipped() {
        let dir = tmpdir("junk");
        write_exe(&dir.join("junk.sh"), "\u{0}\u{1}\u{2}not a header");
        let mut long = String::new();
        while long.len() <= HEADER_BYTES as usize {
            long.push_str("# filler filler filler filler\n");
        }
        long.push_str("# @raycast.schemaVersion 1\n# @raycast.title Late\n# @raycast.mode compact\n");
        write_exe(&dir.join("late.sh"), &long);
        assert!(list_scripts_in(&dir).is_empty());
    }

    #[test]
    fn bad_argument_json_keeps_the_row() {
        let text = "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.argument1 {nope\n";
        let meta = parse_meta(text).unwrap();
        assert!(meta.argument1.is_none());
        let text = "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.argument1 {\"type\":\"text\",\"placeholder\":\"Query\",\"optional\":true}\n";
        let meta = parse_meta(text).unwrap();
        let arg = meta.argument1.unwrap();
        assert_eq!(arg.kind, "text");
        assert_eq!(arg.placeholder.as_deref(), Some("Query"));
        assert!(arg.optional);
    }

    #[test]
    fn icon_paths_are_ignored_and_platform_is_macos_only() {
        let text = "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.icon /tmp/i.png\n";
        assert!(parse_meta(text).unwrap().icon.is_none());
        let text = "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.icon https://x/y.png\n";
        assert!(parse_meta(text).unwrap().icon.is_none());
        let text = "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.platform windows\n";
        assert!(parse_meta(text).is_err());
    }

    #[test]
    fn discovery_never_executes() {
        let dir = tmpdir("noexec");
        let marker = dir.join("ran.marker");
        let body = format!(
            "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Touch\n# @raycast.mode silent\ntouch '{}'\n",
            marker.display()
        );
        write_exe(&dir.join("touch.sh"), &body);
        let listed = list_scripts_in(&dir);
        assert_eq!(listed.len(), 1);
        assert!(!marker.exists(), "discovery ran a script");
        let id = listed[0].id.clone();
        let run = run_script_with(&dir, &id, None, Duration::from_secs(10)).unwrap();
        assert_eq!(run.code, 0);
        assert!(marker.exists(), "an explicit run did not execute");
    }

    #[test]
    fn argument_is_one_argv_entry() {
        let dir = tmpdir("arg");
        let body = "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Echo\n# @raycast.mode compact\nprintf '[%s]' \"$1\"\n";
        write_exe(&dir.join("echo.sh"), body);
        let id = dir.join("echo.sh").to_string_lossy().into_owned();
        let run = run_script_with(&dir, &id, Some("a; rm -rf /tmp/nope"), Duration::from_secs(10)).unwrap();
        assert_eq!(run.stdout, "[a; rm -rf /tmp/nope]");
    }

    #[test]
    fn timeout_kills_and_caps_output() {
        let dir = tmpdir("timeout");
        write_exe(
            &dir.join("slow.sh"),
            "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Slow\n# @raycast.mode compact\nsleep 5\n",
        );
        let id = dir.join("slow.sh").to_string_lossy().into_owned();
        let run = run_script_with(&dir, &id, None, Duration::from_millis(200)).unwrap();
        assert!(run.timed_out);

        write_exe(
            &dir.join("loud.sh"),
            "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Loud\n# @raycast.mode fullOutput\nhead -c 200000 /dev/zero | tr '\\0' 'x'\n",
        );
        let id = dir.join("loud.sh").to_string_lossy().into_owned();
        let run = run_script_with(&dir, &id, None, Duration::from_secs(20)).unwrap();
        assert_eq!(run.code, 0);
        assert!(run.truncated);
        assert_eq!(run.stdout.len(), OUTPUT_CAP);
    }

    #[test]
    fn runs_in_the_configured_directory_and_reports_failure() {
        let dir = tmpdir("cwd");
        let body = format!(
            "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Pwd\n# @raycast.mode compact\n# @raycast.currentDirectoryPath {}\npwd\nexit 3\n",
            dir.display()
        );
        write_exe(&dir.join("pwd.sh"), &body);
        let id = dir.join("pwd.sh").to_string_lossy().into_owned();
        let run = run_script_with(&dir, &id, None, Duration::from_secs(10)).unwrap();
        assert_eq!(run.code, 3);
        let expected = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(run.stdout.trim(), expected.to_string_lossy());
    }

    #[test]
    fn ids_outside_the_folder_are_refused() {
        let dir = tmpdir("outside");
        let other = tmpdir("outside-other");
        let p = other.join("x.sh");
        write_exe(&p, HELLO);
        let err = run_script_with(&dir, &p.to_string_lossy(), None, Duration::from_secs(5)).unwrap_err();
        assert!(err.contains("not in the scripts folder"), "{err}");
    }
}
