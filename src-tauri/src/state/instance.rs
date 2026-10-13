//! One Orbitty at a time. Stable, Nightly and dev builds share one state
//! folder (the compiled identifier), so a second copy would overwrite the
//! first one's saves and take over its control socket.
//!
//! `orbitty.instance.lock` is held with an OS file lock for the whole
//! process. The OS drops it on exit or crash: there is no pid file to clean
//! up and no stale lock. Only `run()` takes it; ctl, mcp and relay never do.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub(crate) const LOCK_FILE: &str = "orbitty.instance.lock";
pub(crate) const STABLE_ID: &str = "space.orbitty.mac";
pub(crate) const NIGHTLY_ID: &str = "space.orbitty.mac.nightly";

/// Kept open until the process ends. Closing it would release the lock.
#[cfg(not(test))]
static HELD: std::sync::OnceLock<File> = std::sync::OnceLock::new();

pub(crate) enum Acquire {
    Held(File),
    /// The holder's JSON line, best-effort (may be empty).
    Busy(String),
}

/// The folder Tauri's `app_data_dir()` resolves on macOS:
/// `$HOME/Library/Application Support/<identifier>`, canonicalized.
pub(crate) fn support_dir(identifier: &str) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or("HOME unset")?;
    let dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(identifier);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    dir.canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", dir.display()))
}

/// Try the lock in `dir`. Never truncates before the lock is ours, so a
/// busy result still reads the holder's line.
pub(crate) fn acquire_in(dir: &Path, bundle_id: Option<&str>) -> Result<Acquire, String> {
    let path = dir.join(LOCK_FILE);
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    match f.try_lock() {
        Ok(()) => {
            let line = holder_line(bundle_id);
            let _ = f.set_len(0);
            let _ = f.rewind();
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
            Ok(Acquire::Held(f))
        }
        Err(TryLockError::WouldBlock) => {
            let mut text = String::new();
            let _ = f.read_to_string(&mut text);
            Ok(Acquire::Busy(text))
        }
        Err(TryLockError::Error(e)) => Err(format!("lock {}: {e}", path.display())),
    }
}

fn holder_line(bundle_id: Option<&str>) -> String {
    let started_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut line = serde_json::json!({
        "pid": std::process::id(),
        "bundleId": bundle_id,
        "exe": exe,
        "startedAt": started_at,
    })
    .to_string();
    line.push('\n');
    line
}

/// `Orbitty` / `Orbitty Nightly` from the holder's bundle id, else
/// `Another Orbitty` (a dev binary has no bundle id).
pub(crate) fn holder_name(bundle_id: Option<&str>) -> &'static str {
    match bundle_id {
        Some(STABLE_ID) => "Orbitty",
        Some(NIGHTLY_ID) => "Orbitty Nightly",
        _ => "Another Orbitty",
    }
}

/// Alert title and text for a busy lock.
pub(crate) fn busy_message(holder: &str) -> (&'static str, String) {
    let v: serde_json::Value = serde_json::from_str(holder.trim()).unwrap_or_default();
    let name = holder_name(v.get("bundleId").and_then(|b| b.as_str()));
    let who = match v.get("pid").and_then(|p| p.as_u64()) {
        Some(pid) => format!("{name} (pid {pid})"),
        None => name.to_string(),
    };
    (
        "Orbitty is already running",
        format!("{who} has your board open. Quit it first. Two copies would overwrite each other's saves."),
    )
}

/// First thing in `run()`. Busy: one alert, then exit 0 before anything
/// binds, spawns, registers or writes. Any other error fails open.
#[cfg(not(test))]
pub fn guard(identifier: &str) {
    let dir = match support_dir(identifier) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("orbitty: instance lock skipped: {e}");
            return;
        }
    };
    match acquire_in(&dir, bundle_id().as_deref()) {
        Ok(Acquire::Held(f)) => {
            let _ = HELD.set(f);
        }
        Ok(Acquire::Busy(holder)) => {
            let (title, text) = busy_message(&holder);
            eprintln!("orbitty: {title}. {text}");
            alert(title, &text);
            std::process::exit(0);
        }
        Err(e) => eprintln!("orbitty: instance lock skipped: {e}"),
    }
}

/// The running bundle's own id (the installed plist), not the compiled
/// identifier, which Stable and Nightly share.
#[cfg(all(not(test), target_os = "macos"))]
fn bundle_id() -> Option<String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return None;
        }
        let id: *mut Object = msg_send![bundle, bundleIdentifier];
        if id.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![id, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

#[cfg(all(not(test), not(target_os = "macos")))]
fn bundle_id() -> Option<String> {
    None
}

/// Runs before Tauri creates NSApplication; the process exits right after.
#[cfg(all(not(test), target_os = "macos"))]
fn alert(title: &str, text: &str) {
    use objc::runtime::{Object, YES};
    use objc::{class, msg_send, sel, sel_impl};
    let (Ok(title), Ok(text), Ok(ok)) = (
        std::ffi::CString::new(title),
        std::ffi::CString::new(text),
        std::ffi::CString::new("OK"),
    ) else {
        return;
    };
    unsafe {
        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if !ns_app.is_null() {
            let _: () = msg_send![ns_app, activateIgnoringOtherApps: YES];
        }
        let alert: *mut Object = msg_send![class!(NSAlert), new];
        if alert.is_null() {
            return;
        }
        let ns_title: *mut Object = msg_send![class!(NSString), stringWithUTF8String: title.as_ptr()];
        let ns_text: *mut Object = msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()];
        let ns_ok: *mut Object = msg_send![class!(NSString), stringWithUTF8String: ok.as_ptr()];
        let _: () = msg_send![alert, setMessageText: ns_title];
        let _: () = msg_send![alert, setInformativeText: ns_text];
        let _: () = msg_send![alert, addButtonWithTitle: ns_ok];
        let _: isize = msg_send![alert, runModal];
        let _: () = msg_send![alert, release];
    }
}

#[cfg(all(not(test), not(target_os = "macos")))]
fn alert(_title: &str, _text: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "orbitty-instance-{tag}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn second_holder_is_busy_and_reads_the_first_line() {
        let dir = temp_dir("busy");
        let first = match acquire_in(&dir, Some(NIGHTLY_ID)).unwrap() {
            Acquire::Held(f) => f,
            Acquire::Busy(_) => panic!("first acquire must hold"),
        };
        let holder = match acquire_in(&dir, Some(STABLE_ID)).unwrap() {
            Acquire::Busy(line) => line,
            Acquire::Held(_) => panic!("second acquire must be busy"),
        };
        let v: serde_json::Value = serde_json::from_str(holder.trim()).unwrap();
        assert_eq!(v["pid"].as_u64(), Some(std::process::id() as u64));
        assert_eq!(v["bundleId"], NIGHTLY_ID);
        assert!(v["startedAt"].as_u64().is_some());
        assert!(v["exe"].is_string());
        let (title, text) = busy_message(&holder);
        assert_eq!(title, "Orbitty is already running");
        assert_eq!(
            text,
            format!(
                "Orbitty Nightly (pid {}) has your board open. Quit it first. Two copies would overwrite each other's saves.",
                std::process::id()
            )
        );
        drop(first);
        assert!(
            matches!(acquire_in(&dir, Some(STABLE_ID)).unwrap(), Acquire::Held(_)),
            "a closed holder (exit or crash) leaves no stale lock"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lock_file_is_0600_and_rewritten_by_each_holder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("mode");
        std::fs::write(dir.join(LOCK_FILE), "x".repeat(400)).unwrap();
        let held = acquire_in(&dir, None).unwrap();
        assert!(matches!(held, Acquire::Held(_)));
        let text = std::fs::read_to_string(dir.join(LOCK_FILE)).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.ends_with('\n'));
        assert!(serde_json::from_str::<serde_json::Value>(text.trim()).is_ok());
        let fresh = temp_dir("mode-new");
        let _held = acquire_in(&fresh, None).unwrap();
        let mode = std::fs::metadata(fresh.join(LOCK_FILE)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    #[test]
    fn holder_names() {
        assert_eq!(holder_name(Some(STABLE_ID)), "Orbitty");
        assert_eq!(holder_name(Some(NIGHTLY_ID)), "Orbitty Nightly");
        assert_eq!(holder_name(None), "Another Orbitty");
        assert_eq!(holder_name(Some("space.orbitty.mac.dev")), "Another Orbitty");
        let (_, text) = busy_message("not json");
        assert!(text.starts_with("Another Orbitty has your board open."), "{text}");
        let (_, text) = busy_message(r#"{"pid":42,"bundleId":"space.orbitty.mac"}"#);
        assert!(text.starts_with("Orbitty (pid 42) has your board open."), "{text}");
    }

    /// The guard is first in `run()`, before the builder, and test builds
    /// never compile it.
    #[test]
    fn guard_is_first_in_run_and_not_in_tests() {
        let lib = include_str!("../lib.rs");
        let start = lib.find("pub fn run() {").expect("run");
        let body = &lib[start..];
        let guard = body.find("state::instance::guard(").expect("guard call");
        let builder = body.find("tauri::Builder::default()").expect("builder");
        assert!(guard < builder);
        let before = &body[..guard];
        assert!(before.contains("#[cfg(not(test))]"), "{before}");
        for later in [".plugin(", "control::start", "shift::boot", "TrayIconBuilder"] {
            assert!(!before.contains(later), "{later} before the guard");
        }
        let src = include_str!("instance.rs");
        let at = src.find("pub fn guard(").unwrap();
        assert!(src[..at].trim_end().ends_with("#[cfg(not(test))]"));
        assert!(!src.contains(concat!("remove_", "file")), "no pid-file cleanup");
    }
}
