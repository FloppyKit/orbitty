//! Go runner selection and the permission model.
//!
//! This GO ships the built-in default only. The signed store (Keychain +
//! HMAC) is the follow-up. Callers use `load_user_settings`. Nothing in
//! this file writes permissions or the approved list. Selection is a
//! separate file (`~/.config/orbitty/go-runner.json`) and may only name
//! an id from the approved list.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::dirs::Roots;
use super::parse::expand_home;

pub const BUILTIN_ID: &str = "pi-grok";

/// 20.5 GiB, in KiB. Same line the cook seat uses before a cold build.
pub const DISK_MIN_KIB: u64 = 21_495_808;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoPermissions {
    pub allow_push: bool,
    pub allow_merge_main: bool,
    pub allow_stable_install: bool,
    pub allow_send: bool,
}

impl Default for GoPermissions {
    fn default() -> Self {
        Self {
            allow_push: false,
            allow_merge_main: false,
            allow_stable_install: false,
            allow_send: false,
        }
    }
}

impl GoPermissions {
    pub fn all_off() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovedRunner {
    pub id: String,
    pub cook: String,
    pub verify: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserGoSettings {
    pub developer_mode: bool,
    pub permissions: GoPermissions,
    pub approved_runners: Vec<ApprovedRunner>,
}

pub fn builtin_user_settings() -> UserGoSettings {
    UserGoSettings {
        developer_mode: false,
        permissions: GoPermissions::all_off(),
        approved_runners: vec![ApprovedRunner {
            id: BUILTIN_ID.into(),
            cook: "~/bin/cook-pi.sh".into(),
            verify: "~/bin/verify-pi.sh".into(),
            model: Some("grok".into()),
            provider: Some("pi".into()),
        }],
    }
}

/// Loader seam. The follow-up reads the signed store here. This GO never
/// opens `go-user.json` and never writes permissions or approved runners.
pub fn load_user_settings(_roots: &Roots) -> UserGoSettings {
    builtin_user_settings()
}

pub fn selection_path(roots: &Roots) -> PathBuf {
    roots.home.join(".config/orbitty/go-runner.json")
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub runner: String,
    pub error: Option<String>,
}

/// Missing file means `pi-grok`. A bad file is an error and is not rewritten.
pub fn read_selection(roots: &Roots) -> Selection {
    let path = selection_path(roots);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Selection {
            runner: BUILTIN_ID.into(),
            error: None,
        };
    };
    match serde_json::from_str::<SelectionFile>(&raw) {
        Ok(f) if f.v == 1 && !f.runner.trim().is_empty() => Selection {
            runner: f.runner.trim().to_string(),
            error: None,
        },
        Ok(_) => Selection {
            runner: BUILTIN_ID.into(),
            error: Some("go-runner.json is not {\"v\":1,\"runner\":\"…\"}".into()),
        },
        Err(e) => Selection {
            runner: BUILTIN_ID.into(),
            error: Some(format!("go-runner.json is malformed ({e})")),
        },
    }
}

#[derive(Deserialize)]
struct SelectionFile {
    v: u32,
    runner: String,
}

fn approved_ids(settings: &UserGoSettings) -> String {
    let mut ids: Vec<&str> = settings.approved_runners.iter().map(|r| r.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        "(none)".into()
    } else {
        ids.join(", ")
    }
}

fn find_runner<'a>(settings: &'a UserGoSettings, id: &str) -> Option<&'a ApprovedRunner> {
    settings.approved_runners.iter().find(|r| r.id == id)
}

/// `go.runner_set`. Accepts only `runner` or `reset`. Refusals do not write.
pub fn apply_runner_set(roots: &Roots, args: &Value) -> Result<Value, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "go.runner_set: need an object".to_string())?;
    for key in obj.keys() {
        if key != "runner" && key != "reset" {
            return Err(format!(
                "go.runner_set: refused key {key:?} (only runner or reset)"
            ));
        }
    }
    let reset = obj.get("reset").and_then(Value::as_bool).unwrap_or(false);
    let runner = obj.get("runner").and_then(Value::as_str).map(str::trim);
    if reset && runner.is_some() {
        return Err("go.runner_set: pass only one of runner or reset".into());
    }
    if reset {
        let path = selection_path(roots);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("go.runner_set: {e}"))?;
        }
        return Ok(json!({ "ok": true, "runner": BUILTIN_ID, "reset": true }));
    }
    let Some(id) = runner.filter(|s| !s.is_empty()) else {
        return Err("go.runner_set: need runner or reset".into());
    };
    let settings = load_user_settings(roots);
    if find_runner(&settings, id).is_none() {
        return Err(format!(
            "runner \"{id}\" is not on the approved list (approved: {})",
            approved_ids(&settings)
        ));
    }
    write_selection(roots, id)?;
    Ok(json!({ "ok": true, "runner": id }))
}

fn write_selection(roots: &Roots, id: &str) -> Result<(), String> {
    let path = selection_path(roots);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let body = format!("{{\"v\":1,\"runner\":{}}}\n", serde_json::to_string(id).unwrap_or_default());
    let tmp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("rename {}: {e}", path.display())
    })
}

pub fn runner_get_value(roots: &Roots) -> Value {
    let settings = load_user_settings(roots);
    let sel = read_selection(roots);
    let entry = find_runner(&settings, &sel.runner);
    let mut error = sel.error.clone();
    if error.is_none() && entry.is_none() {
        error = Some(format!(
            "runner \"{}\" is not on the approved list (approved: {})",
            sel.runner,
            approved_ids(&settings)
        ));
    }
    let (cook, verify, model, provider) = match entry {
        Some(r) => (
            expand_home(roots, &r.cook).display().to_string(),
            expand_home(roots, &r.verify).display().to_string(),
            r.model.clone(),
            r.provider.clone(),
        ),
        None => (String::new(), String::new(), None, None),
    };
    json!({
        "runner": sel.runner,
        "cook": cook,
        "verify": verify,
        "model": model,
        "provider": provider,
        "approved": settings.approved_runners,
        "permissions": settings.permissions,
        "developerMode": settings.developer_mode,
        "error": error,
    })
}

/// One path, `~/` expanded, no args. Regular file, executable, owned by
/// the user, not group or world writable. Symlinks are refused.
pub fn validate_program(roots: &Roots, raw: &str, label: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(format!("runner config: {label} is empty"));
    }
    if raw.contains(char::is_whitespace) || raw.contains('\0') {
        return Err(format!("runner config: {label} is not a single path"));
    }
    if raw.starts_with('-') {
        return Err(format!("runner config: {label} is not a path"));
    }
    let path = expand_home(roots, raw);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| {
        format!("runner config: {label} is not a regular file ({})", path.display())
    })?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return Err(format!(
            "runner config: {label} is not a regular file ({})",
            path.display()
        ));
    }
    let mode = meta.mode();
    if mode & 0o111 == 0 {
        return Err(format!("runner config: {label} is not executable"));
    }
    let uid = unsafe { getuid() };
    if meta.uid() != uid {
        return Err(format!("runner config: {label} is not owned by the user"));
    }
    if mode & 0o022 != 0 {
        return Err(format!(
            "runner config: {label} is group or world writable"
        ));
    }
    Ok(path)
}

extern "C" {
    fn getuid() -> u32;
}

pub fn validate_selected(roots: &Roots) -> Result<(String, PathBuf, PathBuf, GoPermissions), String> {
    let settings = load_user_settings(roots);
    let sel = read_selection(roots);
    if let Some(e) = sel.error {
        return Err(format!("runner config: {e}"));
    }
    let Some(entry) = find_runner(&settings, &sel.runner) else {
        return Err(format!(
            "runner config: runner \"{}\" is not on the approved list (approved: {})",
            sel.runner,
            approved_ids(&settings)
        ));
    };
    let cook = validate_program(roots, &entry.cook, "cook")?;
    let verify = validate_program(roots, &entry.verify, "verify")?;
    Ok((entry.id.clone(), cook, verify, settings.permissions.clone()))
}

/// Top-level `key: value` in a GO header. Quoted scalars are unwrapped.
/// Does not touch `GoHeader`, so the seal hash path stays as it is.
pub fn yaml_top_scalar(yaml: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in yaml.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with([' ', '\t', '#']) {
            continue;
        }
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() || rest.starts_with('|') || rest.starts_with('>') {
            return None;
        }
        let rest = rest.split(" #").next().unwrap_or(rest).trim();
        let unquoted =
            if rest.len() >= 2 && ((rest.starts_with('\'') && rest.ends_with('\'')) || (rest.starts_with('"') && rest.ends_with('"')))
            {
                &rest[1..rest.len() - 1]
            } else {
                rest
            };
        let unquoted = unquoted.trim();
        if unquoted.is_empty() {
            return None;
        }
        return Some(unquoted.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn roots(home: &Path) -> Roots {
        Roots {
            home: home.to_path_buf(),
        }
    }

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "orbitty-go-runner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn defaults_are_all_off_and_pi_grok_only() {
        let s = builtin_user_settings();
        assert!(!s.developer_mode);
        assert_eq!(s.permissions, GoPermissions::all_off());
        assert_eq!(s.approved_runners.len(), 1);
        assert_eq!(s.approved_runners[0].id, "pi-grok");
        assert_eq!(s.approved_runners[0].cook, "~/bin/cook-pi.sh");
        assert_eq!(s.approved_runners[0].verify, "~/bin/verify-pi.sh");
        let home = tmp();
        let g = runner_get_value(&roots(&home));
        assert_eq!(g["runner"], "pi-grok");
        assert_eq!(g["developerMode"], false);
        assert_eq!(g["permissions"]["allow_push"], false);
        assert_eq!(g["permissions"]["allow_merge_main"], false);
        assert_eq!(g["permissions"]["allow_stable_install"], false);
        assert_eq!(g["permissions"]["allow_send"], false);
        assert!(g["error"].is_null());
        assert!(!selection_path(&roots(&home)).exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn valid_set_writes_only_the_id() {
        let home = tmp();
        let r = roots(&home);
        let got = apply_runner_set(&r, &json!({"runner": "pi-grok"})).unwrap();
        assert_eq!(got["runner"], "pi-grok");
        let raw = std::fs::read_to_string(selection_path(&r)).unwrap();
        assert_eq!(raw, "{\"v\":1,\"runner\":\"pi-grok\"}\n");
        assert_eq!(read_selection(&r).runner, "pi-grok");
        assert!(read_selection(&r).error.is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn off_list_id_is_refused_with_no_write() {
        let home = tmp();
        let r = roots(&home);
        let err = apply_runner_set(&r, &json!({"runner": "nope"})).unwrap_err();
        assert_eq!(
            err,
            "runner \"nope\" is not on the approved list (approved: pi-grok)"
        );
        assert!(!selection_path(&r).exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn extra_keys_are_refused() {
        let home = tmp();
        let r = roots(&home);
        std::fs::write(selection_path(&r).with_file_name("keep"), b"x").ok();
        for args in [
            json!({"runner": "pi-grok", "cook": "/tmp/x.sh"}),
            json!({"runner": "pi-grok", "verify": "/tmp/x.sh"}),
            json!({"permissions": {"allow_push": true}}),
            json!({"approved": []}),
            json!({"cook": "/tmp/x.sh"}),
        ] {
            let err = apply_runner_set(&r, &args).unwrap_err();
            assert!(err.contains("refused key"), "{err}");
            assert!(!selection_path(&r).exists(), "{args}");
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn malformed_selection_is_not_replaced() {
        let home = tmp();
        let r = roots(&home);
        let path = selection_path(&r);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{not json\n").unwrap();
        let before = std::fs::read(&path).unwrap();
        let err = apply_runner_set(&r, &json!({"runner": "nope"})).unwrap_err();
        assert!(err.contains("not on the approved list"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let got = runner_get_value(&r);
        assert!(got["error"].as_str().unwrap().contains("malformed"), "{got}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn no_code_path_writes_permissions_or_approved_runners() {
        let src = include_str!("go_runner.rs");
        assert!(!src.contains("go-user.json"), "must not touch the signed store");
        assert!(!src.contains("save_settings"));
        assert!(!src.contains("allow_push ="));
        assert!(!src.contains("approved_runners ="));
        let prod = src.split("mod tests").next().unwrap();
        assert_eq!(prod.matches("std::fs::write").count(), 0);
        assert_eq!(prod.matches("File::create").count(), 1, "only the selection temp");
        assert!(prod.contains("only runner or reset"));
    }

    #[test]
    fn program_must_be_a_user_owned_executable() {
        let home = tmp();
        let r = roots(&home);
        let bin = home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let ok = bin.join("cook-pi.sh");
        std::fs::write(&ok, b"#!/bin/zsh\n").unwrap();
        let mut perms = std::fs::metadata(&ok).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&ok, perms).unwrap();
        assert!(validate_program(&r, &ok.display().to_string(), "cook").is_ok());
        let spaced = format!("{} --flag", ok.display());
        assert!(validate_program(&r, &spaced, "cook")
            .unwrap_err()
            .contains("not a single path"));
        perms.set_mode(0o775);
        std::fs::set_permissions(&ok, perms).unwrap();
        assert!(validate_program(&r, &ok.display().to_string(), "cook")
            .unwrap_err()
            .contains("group or world writable"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn yaml_top_scalar_reads_base_without_a_header_field() {
        let yaml = "id: GO-X\nbase: '56ace40f'\nlock:\ncard_id: c-1\n";
        assert_eq!(yaml_top_scalar(yaml, "base").as_deref(), Some("56ace40f"));
        assert_eq!(yaml_top_scalar(yaml, "id").as_deref(), Some("GO-X"));
        assert!(yaml_top_scalar(yaml, "lock").is_none());
        assert_eq!(yaml_top_scalar(yaml, "card_id").as_deref(), Some("c-1"));
    }
}
