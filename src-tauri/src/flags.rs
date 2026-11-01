//! One tiny runtime flag table. Later GOs add a row and an `if`.
//! Effective value: env `ORBITTY_FLAG_<ID>`, else the channel file, else the
//! table default. A flag that is not in the table is an error, never silently on.
//!
//! Nightly and Stable share an app-data directory. Flag files are per channel
//! (`flags-nightly.json`, `flags-stable.json`, `flags-alpha.json`). `state.json`
//! stays the shared path.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use tauri::Manager;

pub const MANUAL_ONLY: &[&str] = &[
    "adapter_cursor",
    "adapter_grokbot",
    "beads_truth",
    "board_rules_webhook",
    "cold_tree",
    "coordinator_launch",
    "coordinator_propose",
    "coordinator_triage",
    "cost_view",
    "net_guard",
    "outcome_lanes",
    "promote_schedule",
    "proof_required",
    "remote_guardrails",
    "remote_net",
];

#[derive(Clone, Copy, Debug)]
pub struct FlagDef {
    pub id: &'static str,
    pub default: bool,
    pub what: &'static str,
    pub go: &'static str,
    pub added: &'static str,
    pub remove_after: &'static str,
    pub auto_on_verify: bool,
}

pub const FLAGS: &[FlagDef] = &[
    FlagDef {
        id: "tabbity_bridge",
        default: true,
        what: "tab claim door",
        go: "GO-ORBITTY-TABBITY-BRIDGE",
        added: "2026-10-28",
        remove_after: "2026-12-27",
        auto_on_verify: false,
    },
    FlagDef {
        id: "go_start",
        default: true,
        what: "go.start verb",
        go: "GO-ORBITTY-MCP-GO",
        added: "2026-10-28",
        remove_after: "2026-12-27",
        auto_on_verify: false,
    },
    FlagDef {
        id: "event_seq",
        default: false,
        what: "monotonic seq on events.jsonl",
        go: "GO-ORBITTY-EVENT-SEQ",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "ax_safe",
        default: true,
        what: "AX timeout and off-main frontmost read",
        go: "GO-ORBITTY-AX-SAFE",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: false,
    },
    FlagDef {
        id: "microtags",
        default: false,
        what: "namespaced t tags on board 420-422",
        go: "GO-ORBITTY-NOSTR-MICROTAGS",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "mh_archive",
        default: false,
        what: "archive companion turns before the 40-turn drain",
        go: "GO-ORBITTY-MH-ARCHIVE-STEP2",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "shim_ledger",
        default: false,
        what: "count model-misbehaviour shims",
        go: "GO-ORBITTY-SHIM-LEDGER",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "memory_ram",
        default: false,
        what: "companion RAM core in the prompt",
        go: "GO-ORBITTY-COMPANION-MEMORY-RAM",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "herdr_core",
        default: true,
        what: "run seats in herdr-core",
        go: "GO-ORBITTY-HERDR-CORE-READ",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        // Kill switch stays manual. Env and the flag file still win over this default.
        auto_on_verify: false,
    },
    FlagDef {
        id: "wake_core",
        default: false,
        what: "subscribe and wake inbox",
        go: "GO-ORBITTY-SUBSCRIBE-WAKE-CORE",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "herdr_primary",
        default: false,
        what: "wake reads hc events first",
        go: "GO-ORBITTY-SUBSCRIBE-WAKE-CORE",
        added: "2026-10-30",
        remove_after: "2026-12-29",
        auto_on_verify: true,
    },
    FlagDef {
        id: "card_blockers",
        default: false,
        what: "card blocker edges and Doing doors",
        go: "GO-ORBITTY-CARD-BLOCKERS",
        added: "2026-11-01",
        remove_after: "2026-12-31",
        auto_on_verify: true,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Env,
    File,
    Default,
    AutoVerify,
}

#[derive(Clone, Debug, Serialize)]
pub struct Effective {
    pub id: &'static str,
    pub on: bool,
    pub source: Source,
    pub stale: bool,
    pub what: &'static str,
    pub added: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Alpha,
    Nightly,
    Stable,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Alpha => "alpha",
            Channel::Nightly => "nightly",
            Channel::Stable => "stable",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileFlags {
    pub on: BTreeMap<String, bool>,
    pub src: BTreeMap<String, String>,
    pub sticky_off: BTreeSet<String>,
    /// `false` after `orbittyctl flags auto off`. Missing in the file means on.
    pub auto: bool,
}

impl FileFlags {
    pub(crate) fn empty() -> Self {
        Self {
            on: BTreeMap::new(),
            src: BTreeMap::new(),
            sticky_off: BTreeSet::new(),
            auto: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SweepReport {
    pub go_id: String,
    pub sha8: String,
    pub result: String,
}

struct Loaded {
    path: PathBuf,
    file: FileFlags,
}

fn cache() -> &'static RwLock<Option<Loaded>> {
    static CACHE: OnceLock<RwLock<Option<Loaded>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

fn read_cache() -> Option<(PathBuf, FileFlags)> {
    let g = cache().read().unwrap_or_else(|e| e.into_inner());
    g.as_ref().map(|l| (l.path.clone(), l.file.clone()))
}

fn write_cache(path: PathBuf, file: FileFlags) {
    let mut g = cache().write().unwrap_or_else(|e| e.into_inner());
    *g = Some(Loaded { path, file });
}

fn cache_file() -> FileFlags {
    read_cache()
        .map(|(_, file)| file)
        .unwrap_or_else(FileFlags::empty)
}

pub fn channel_of(bundle_id: &str) -> Channel {
    if bundle_id == crate::state::instance::NIGHTLY_ID || bundle_id.ends_with(".nightly") {
        Channel::Nightly
    } else if bundle_id == "space.orbitty.alpha" || bundle_id.contains(".alpha") {
        Channel::Alpha
    } else {
        Channel::Stable
    }
}

pub fn flags_path(app_data: &Path, channel: Channel) -> PathBuf {
    app_data.join(format!("flags-{}.json", channel.as_str()))
}

pub fn def_by_id(id: &str) -> Option<&'static FlagDef> {
    FLAGS.iter().find(|d| d.id == id)
}

pub fn env_key(id: &str) -> String {
    format!("ORBITTY_FLAG_{}", id.to_ascii_uppercase())
}

pub(crate) fn parse_env_bit(raw: &str) -> Option<bool> {
    match raw.trim() {
        "1" | "on" | "true" => Some(true),
        "0" | "off" | "false" => Some(false),
        _ => None,
    }
}

fn env_raw(id: &str) -> Option<String> {
    std::env::var(env_key(id))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn today_ct() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn is_stale(remove_after: &str, today_ct: &str) -> bool {
    !remove_after.is_empty() && remove_after < today_ct
}

pub fn id_ok(id: &str) -> bool {
    let n = id.len();
    (2..=40).contains(&n)
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn refusal(feature: &str, id: &str) -> String {
    format!("{feature} is off (flag {id}). orbittyctl flags set {id} on")
}

/// Pure. Invalid env is not a value: the caller warns and this falls through.
pub(crate) fn resolve(def: &FlagDef, env: Option<&str>, file: &FileFlags) -> (bool, Source) {
    if let Some(raw) = env {
        if let Some(on) = parse_env_bit(raw) {
            return (on, Source::Env);
        }
    }
    if let Some(on) = file.on.get(def.id) {
        let auto = file.src.get(def.id).map(String::as_str) == Some("auto-verify");
        return (
            *on,
            if auto {
                Source::AutoVerify
            } else {
                Source::File
            },
        );
    }
    (def.default, Source::Default)
}

pub(crate) fn parse_file(bytes: &[u8]) -> (FileFlags, Option<String>) {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            return (
                FileFlags::empty(),
                Some(format!("flags.json: corrupt ({e})")),
            )
        }
    };
    let Some(obj) = value.as_object() else {
        return (
            FileFlags::empty(),
            Some("flags.json: corrupt (not an object)".into()),
        );
    };
    if obj.get("v").and_then(Value::as_u64) != Some(1) {
        return (FileFlags::empty(), Some("flags.json: corrupt (v)".into()));
    }
    let Some(on_v) = obj.get("on") else {
        return (FileFlags::empty(), Some("flags.json: corrupt (on)".into()));
    };
    let Some(on_obj) = on_v.as_object() else {
        return (FileFlags::empty(), Some("flags.json: corrupt (on)".into()));
    };
    let mut file = FileFlags::empty();
    for (k, v) in on_obj {
        let Some(bit) = v.as_bool() else {
            return (
                FileFlags::empty(),
                Some(format!("flags.json: corrupt (on.{k})")),
            );
        };
        file.on.insert(k.clone(), bit);
    }
    if let Some(src_v) = obj.get("src") {
        let Some(src_obj) = src_v.as_object() else {
            return (FileFlags::empty(), Some("flags.json: corrupt (src)".into()));
        };
        for (k, v) in src_obj {
            let Some(s) = v.as_str() else {
                return (
                    FileFlags::empty(),
                    Some(format!("flags.json: corrupt (src.{k})")),
                );
            };
            file.src.insert(k.clone(), s.to_string());
        }
    }
    if let Some(sticky_v) = obj.get("sticky_off") {
        let Some(arr) = sticky_v.as_array() else {
            return (
                FileFlags::empty(),
                Some("flags.json: corrupt (sticky_off)".into()),
            );
        };
        for v in arr {
            let Some(s) = v.as_str() else {
                return (
                    FileFlags::empty(),
                    Some("flags.json: corrupt (sticky_off)".into()),
                );
            };
            file.sticky_off.insert(s.to_string());
        }
    }
    if let Some(auto_v) = obj.get("auto") {
        let Some(bit) = auto_v.as_bool() else {
            return (
                FileFlags::empty(),
                Some("flags.json: corrupt (auto)".into()),
            );
        };
        file.auto = bit;
    }
    (file, None)
}

pub(crate) fn serialize_file(file: &FileFlags) -> Vec<u8> {
    let sticky: Vec<&String> = file.sticky_off.iter().collect();
    let value = json!({
        "v": 1,
        "on": file.on,
        "src": file.src,
        "sticky_off": sticky,
        "auto": file.auto,
    });
    let mut bytes = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    bytes
}

pub(crate) fn mark_set(file: &mut FileFlags, id: &str, on: bool) {
    file.on.insert(id.to_string(), on);
    file.src.insert(id.to_string(), "file".into());
    if on {
        file.sticky_off.remove(id);
    } else {
        file.sticky_off.insert(id.to_string());
    }
}

pub(crate) fn apply_set(file: &mut FileFlags, id: &str, on: bool) -> Result<(), String> {
    if !id_ok(id) || def_by_id(id).is_none() {
        return Err(format!("unknown flag {id}"));
    }
    mark_set(file, id, on);
    Ok(())
}

fn row(def: &FlagDef, file: &FileFlags, today: &str) -> Effective {
    let env = env_raw(def.id);
    if let Some(ref raw) = env {
        if parse_env_bit(raw).is_none() {
            eprintln!("orbitty: ignoring bad {}={raw}", env_key(def.id));
        }
    }
    let (on, source) = resolve(def, env.as_deref(), file);
    Effective {
        id: def.id,
        on,
        source,
        stale: is_stale(def.remove_after, today),
        what: def.what,
        added: def.added,
    }
}

/// Same decision `on` uses. Unknown id → false and one warning. Never panics.
pub(crate) fn on_loaded(id: &str, file: &FileFlags) -> bool {
    let Some(def) = def_by_id(id) else {
        eprintln!("orbitty: unknown flag {id}");
        return false;
    };
    row(def, file, &today_ct()).on
}

fn flags_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    let channel = channel_of(app.config().identifier.as_str());
    Ok(flags_path(&dir, channel))
}

fn read_disk(path: &Path) -> (FileFlags, Option<String>) {
    match fs::read(path) {
        Ok(bytes) => parse_file(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (FileFlags::empty(), None),
        Err(e) => (
            FileFlags::empty(),
            Some(format!("flags.json: read {}: {e}", path.display())),
        ),
    }
}

fn load(app: &tauri::AppHandle) -> Result<(PathBuf, FileFlags), String> {
    if let Some(hit) = read_cache() {
        return Ok(hit);
    }
    let path = flags_file(app)?;
    let (file, warn) = read_disk(&path);
    if let Some(w) = warn {
        eprintln!("orbitty: {w}");
    }
    write_cache(path.clone(), file.clone());
    Ok((path, file))
}

fn file_of(app: &tauri::AppHandle) -> FileFlags {
    match load(app) {
        Ok((_, file)) => file,
        Err(e) => {
            eprintln!("orbitty: flags: {e}");
            FileFlags::empty()
        }
    }
}

/// Env, then the process cache `on` filled, else the channel file for the
/// running bundle. Default off when none of those are present. `push_turn`
/// has no app handle; startup `auto_sweep` warms the cache before a send.
pub(crate) fn on_ambient(id: &str) -> bool {
    if let Some(raw) = env_raw(id) {
        if let Some(bit) = parse_env_bit(&raw) {
            return bit;
        }
        eprintln!("orbitty: ignoring bad {}={raw}", env_key(id));
    }
    if read_cache().is_some() {
        return on_loaded(id, &cache_file());
    }
    #[cfg(all(target_os = "macos", not(test)))]
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        let bundle =
            macos_bundle_id().unwrap_or_else(|| crate::state::instance::STABLE_ID.to_string());
        let dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(bundle);
        if dir.is_dir() {
            return on_in_dir(&dir, id);
        }
    }
    on_loaded(id, &FileFlags::empty())
}

pub fn on(app: &tauri::AppHandle, id: &str) -> bool {
    if def_by_id(id).is_none() {
        return on_loaded(id, &FileFlags::empty());
    }
    on_loaded(id, &file_of(app))
}

// Tests pin ax_safe per thread. None follows the process cache below.
#[cfg(test)]
thread_local! {
    static AX_SAFE_FORCE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Previous value, so a test can restore the thread when it finishes.
#[cfg(test)]
pub(crate) fn force_ax_safe(on: Option<bool>) -> Option<bool> {
    AX_SAFE_FORCE.with(|c| c.replace(on))
}

/// `ax_safe`, read once per process. The frontmost tracker ticks every
/// 400 ms and must not hit the flag file on each tick. Env, then the
/// channel file beside the app-data dir `flags::on` uses, else the
/// table default (on).
pub(crate) fn ax_safe_on() -> bool {
    #[cfg(test)]
    if let Some(v) = AX_SAFE_FORCE.with(|c| c.get()) {
        return v;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(ax_safe_from_disk)
}

fn ax_safe_from_disk() -> bool {
    let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
        return get("ax_safe").map(|e| e.on).unwrap_or(true);
    };
    let dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(ax_safe_support_id());
    on_in_dir(&dir, "ax_safe")
}

/// Tauri's compiled identifier. Nightly rewrites the installed plist to
/// `space.orbitty.mac.nightly` but keeps this id (and this support dir),
/// which is what `flags::on` reads. Alpha is its own identifier.
fn ax_safe_support_id() -> String {
    #[cfg(all(target_os = "macos", not(test)))]
    if let Some(id) = macos_bundle_id() {
        if id == "space.orbitty.alpha" || id.contains(".alpha") {
            return id;
        }
    }
    crate::state::instance::STABLE_ID.to_string()
}

#[cfg(all(target_os = "macos", not(test)))]
fn macos_bundle_id() -> Option<String> {
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
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// Flag bit for a directory that holds `events.jsonl`. Env wins, then the
/// process cache when it was loaded for this directory, else the channel
/// file beside the log. The directory name is the bundle id
/// (`space.orbitty.mac.nightly` → `flags-nightly.json`).
pub(crate) fn on_in_dir(dir: &Path, id: &str) -> bool {
    if let Some((path, file)) = read_cache() {
        if path.parent() == Some(dir) {
            return on_loaded(id, &file);
        }
    }
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let (file, _) = read_disk(&flags_path(dir, channel_of(name)));
    on_loaded(id, &file)
}

pub fn get(id: &str) -> Result<Effective, String> {
    let def = def_by_id(id).ok_or_else(|| format!("unknown flag {id}"))?;
    Ok(row(def, &cache_file(), &today_ct()))
}

pub fn list(today_ct: &str) -> Vec<Effective> {
    let file = cache_file();
    FLAGS.iter().map(|def| row(def, &file, today_ct)).collect()
}

pub fn list_app(app: &tauri::AppHandle) -> Value {
    let _ = file_of(app);
    let rows = list(&today_ct());
    Value::Array(
        rows.into_iter()
            .map(|eff| serde_json::to_value(eff).unwrap_or(Value::Null))
            .collect(),
    )
}

pub fn get_app(app: &tauri::AppHandle, id: &str) -> Result<Value, String> {
    let _ = file_of(app);
    let eff = get(id)?;
    Ok(serde_json::to_value(eff).unwrap_or(Value::Null))
}

#[tauri::command]
pub fn flags_get(app: tauri::AppHandle, id: String) -> Result<Value, String> {
    get_app(&app, &id)
}

#[tauri::command]
pub fn flags_list(app: tauri::AppHandle) -> Value {
    list_app(&app)
}

#[tauri::command]
pub fn flags_set(app: tauri::AppHandle, id: String, on: bool) -> Result<Value, String> {
    set(&app, &id, on)
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let wrote = (|| -> Result<(), String> {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all()
            .map_err(|e| format!("sync {}: {e}", tmp.display()))?;
        drop(f);
        fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("mode {}: {e}", path.display()))?;
        }
        Ok(())
    })();
    if wrote.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    wrote
}

fn store_at(path: &Path, file: &FileFlags) -> Result<(), String> {
    write_atomic(path, &serialize_file(file))?;
    write_cache(path.to_path_buf(), file.clone());
    Ok(())
}

pub fn set(app: &tauri::AppHandle, id: &str, on_bit: bool) -> Result<Value, String> {
    let (path, mut file) = load(app)?;
    apply_set(&mut file, id, on_bit)?;
    store_at(&path, &file)?;
    let def = def_by_id(id).ok_or_else(|| format!("unknown flag {id}"))?;
    Ok(serde_json::to_value(row(def, &cache_file(), &today_ct())).unwrap_or(Value::Null))
}

pub fn set_auto(app: &tauri::AppHandle, on_bit: bool) -> Result<Value, String> {
    let (path, mut file) = load(app)?;
    file.auto = on_bit;
    store_at(&path, &file)?;
    Ok(json!({ "auto": on_bit }))
}

pub fn blocked(app: &tauri::AppHandle, id: &str, feature: &str) -> Option<String> {
    if on(app, id) {
        None
    } else {
        Some(refusal(feature, id))
    }
}

pub fn note_refusal(
    app: &tauri::AppHandle,
    card: &str,
    msg: String,
) -> Result<serde_json::Value, String> {
    let _ = crate::board::kanban_note_append(app, card, &msg);
    Err(msg)
}

pub(crate) fn report_is_pass(result: &str) -> bool {
    let r = result.trim();
    r == "PASS" || r.starts_with("PASS-with-notes")
}

fn verdict_line(t: &str) -> bool {
    matches!(t, "PASS" | "PASS-with-notes" | "FAIL" | "RIG BLOCKER")
}

/// Kitchen reports put the verdict on the first non-empty line under
/// `## Result` (`PASS`, `PASS-with-notes`, `FAIL`, `RIG BLOCKER`), then a
/// `PASS tip:` line. Older notes use a `RESULT:` line. Either counts.
pub(crate) fn result_of(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i];
        if let Some(rest) = t.strip_prefix("RESULT:") {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        } else if let Some(rest) = t.strip_prefix("RESULT ") {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        } else if t == "## Result" {
            let mut j = i + 1;
            while j < lines.len() && lines[j].is_empty() {
                j += 1;
            }
            if j < lines.len() && verdict_line(lines[j]) {
                return Some(lines[j].to_string());
            }
        }
        i += 1;
    }
    None
}

pub(crate) fn parse_verify_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("VERIFY_")?.strip_suffix(".md")?;
    let (go, sha) = rest.rsplit_once('_')?;
    if sha.len() == 8 && sha.chars().all(|c| c.is_ascii_hexdigit()) && !go.is_empty() {
        Some((go.to_string(), sha.to_ascii_lowercase()))
    } else {
        None
    }
}

pub(crate) fn load_reports(dir: &Path, sha8: &str) -> Vec<SweepReport> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((go_id, sha)) = parse_verify_name(name) else {
            continue;
        };
        if sha != sha8 {
            continue;
        }
        let Ok(text) = fs::read_to_string(ent.path()) else {
            continue;
        };
        let Some(result) = result_of(&text) else {
            continue;
        };
        out.push(SweepReport {
            go_id,
            sha8: sha,
            result,
        });
    }
    out
}

pub(crate) fn sweep_defs(
    defs: &[FlagDef],
    channel: Channel,
    tip_sha8: &str,
    reports: &[SweepReport],
    file: &mut FileFlags,
) -> Vec<String> {
    if channel != Channel::Nightly || !file.auto {
        return Vec::new();
    }
    let mut flipped = Vec::new();
    for def in defs {
        if !def.auto_on_verify || MANUAL_ONLY.contains(&def.id) {
            continue;
        }
        if file.sticky_off.contains(def.id) {
            continue;
        }
        let pass = reports
            .iter()
            .any(|r| r.sha8 == tip_sha8 && r.go_id == def.go && report_is_pass(&r.result));
        if !pass {
            continue;
        }
        let already = file.on.get(def.id) == Some(&true)
            && file.src.get(def.id).map(String::as_str) == Some("auto-verify");
        if already {
            continue;
        }
        file.on.insert(def.id.to_string(), true);
        file.src.insert(def.id.to_string(), "auto-verify".into());
        file.sticky_off.remove(def.id);
        flipped.push(def.id.to_string());
    }
    flipped
}

pub(crate) fn tip_sha8_from_plist(text: &str) -> Option<String> {
    let key = "<key>OrbittyGitCommit</key>";
    let i = text.find(key)?;
    let rest = &text[i + key.len()..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    let commit = rest[start..start + end].trim();
    if commit.len() >= 8 && commit.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        Some(commit[..8].to_ascii_lowercase())
    } else {
        None
    }
}

fn installed_tip_sha8() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let plist = exe.parent()?.parent()?.join("Info.plist");
    let text = fs::read_to_string(plist).ok()?;
    tip_sha8_from_plist(&text)
}

fn append_flags_log(dir: &Path, line: &str) -> Result<(), String> {
    let path = dir.join("flags.log");
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| format!("flags.log: {e}"))?;
    writeln!(f, "{line}").map_err(|e| format!("flags.log: {e}"))?;
    Ok(())
}

/// Nightly start. Stable never flips. A manual off stays off. No-ops when
/// this build has no stamped tip (dev `cargo run` has no Info.plist commit).
pub fn auto_sweep(app: &tauri::AppHandle) {
    let channel = channel_of(app.config().identifier.as_str());
    if channel != Channel::Nightly {
        return;
    }
    let Some(sha8) = installed_tip_sha8() else {
        return;
    };
    let reports_dir = crate::plans::plans_dir(&crate::plans::Roots::real());
    let reports = load_reports(&reports_dir, &sha8);
    let Ok((path, mut file)) = load(app) else {
        return;
    };
    let flipped = sweep_defs(FLAGS, channel, &sha8, &reports, &mut file);
    if flipped.is_empty() {
        return;
    }
    if store_at(&path, &file).is_err() {
        return;
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    for id in &flipped {
        let line = format!("{stamp} auto-verify {id} on tip {sha8}");
        if let Some(dir) = path.parent() {
            let _ = append_flags_log(dir, &line);
        }
        let _ = crate::event_log::append(
            app,
            "flag-auto",
            None,
            None,
            Some("flags"),
            true,
            &json!({
                "id": id,
                "on": true,
                "source": "auto-verify",
                "tip": sha8,
            }),
        );
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
