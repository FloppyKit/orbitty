//! Card links — file:// and https:// only, at write and at open.
//! A bare absolute `/Users/…` path is stored as a percent-encoded file URL.
//! The first click previews. Open / Run is a second click, remembered per
//! exact resolved path or full URL in `link-approvals.json` next to state
//! (local, human-readable, not part of synced state). macOS `open` is the
//! only launcher. No shell.

#[cfg(test)]
use std::cell::RefCell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::state::{CardLink, PersistedState};

pub const MAX_LINKS: usize = 8;

const MAX_URL: usize = 2048;

/// Said on every rejection so the fix is obvious.
pub const ACCEPTED: &str = "use https://…, file:///…, /absolute/path or ~/path";

/// `file://Users/x` (two slashes) is a common typo for `file:///Users/x`:
/// the first path segment lands in the host slot. Only well-known macOS
/// root folders are re-read as a path — `file://relative` stays rejected.
const FILE_HOST_ROOTS: &[&str] = &[
    "Users", "Volumes", "Applications", "Library", "System", "private", "tmp", "opt",
];

fn must_be() -> String {
    format!("link rejected — {ACCEPTED}")
}

fn not_absolute() -> String {
    format!("file link must be an absolute path — {ACCEPTED}")
}

/// `~` / `~/…` → $HOME/…  (`~user` is not expanded — rejected).
fn expand_tilde(s: &str) -> Option<String> {
    let rest = s.strip_prefix('~')?;
    if !(rest.is_empty() || rest.starts_with('/')) {
        return None;
    }
    let home = std::env::var("HOME").ok().filter(|h| h.starts_with('/'))?;
    Some(format!("{}{rest}", home.trim_end_matches('/')))
}
const MAX_LABEL: usize = 80;

/// Normalize one link for storage. Idempotent on its own output.
pub fn normalize_link(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(must_be());
    }
    if s.len() > MAX_URL {
        return Err("link is too long".into());
    }
    if s.chars().any(|c| c.is_control()) {
        return Err("link rejected".into());
    }
    if let Some(scheme) = scheme_of(s) {
        let lower = scheme.to_ascii_lowercase();
        match lower.as_str() {
            "https" => return normalize_https(s),
            "file" => return normalize_file_url(s),
            other => {
                return Err(format!("link rejected: {other}: is not allowed — {ACCEPTED}"));
            }
        }
    }
    if s.starts_with('/') {
        return finish_file_path(&percent_decode(s)?);
    }
    if s.starts_with('~') {
        let p = expand_tilde(s).ok_or_else(|| format!("only ~/ (your home) expands — {ACCEPTED}"))?;
        return finish_file_path(&percent_decode(&p)?);
    }
    Err(must_be())
}

pub fn clean_label(label: Option<String>) -> Result<Option<String>, String> {
    let Some(l) = label else {
        return Ok(None);
    };
    let t = l.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.chars().any(|c| c.is_control()) {
        return Err("link label rejected".into());
    }
    if t.chars().count() > MAX_LABEL {
        return Err("link label is too long".into());
    }
    Ok(Some(t.to_string()))
}

/// Normalize every card's links. Empty becomes absent (`None`), never null.
pub fn canonicalize_links(links: Option<Vec<CardLink>>) -> Result<Option<Vec<CardLink>>, String> {
    let Some(links) = links else {
        return Ok(None);
    };
    if links.is_empty() {
        return Ok(None);
    }
    let mut out: Vec<CardLink> = Vec::with_capacity(links.len());
    for link in links {
        let url = normalize_link(&link.url)?;
        let label = clean_label(link.label)?;
        if let Some(prev) = out.iter_mut().find(|l| l.url == url) {
            prev.label = label;
        } else {
            out.push(CardLink { url, label });
        }
    }
    if out.len() > MAX_LINKS {
        return Err(format!("a card holds at most {MAX_LINKS} links"));
    }
    Ok(Some(out))
}

pub fn canonicalize_state_links(state: &mut PersistedState) -> Result<(), String> {
    for card in &mut state.kanban_cards {
        card.links = canonicalize_links(card.links.take())?;
    }
    Ok(())
}

/// Launch a link. Missing files return "file not found" and do not spawn.
/// This is the confirmed launcher (Open / Run). Unconfirmed clicks go
/// through [`click_link_at`], which previews first. Callers that show a
/// preview first use [`open_link_checked`] so a swapped target is refused.
#[cfg_attr(not(test), allow(dead_code))]
pub fn open_url(raw: &str) -> Result<(), String> {
    let preview = preview_link(raw)?;
    launch(&preview)
}

#[tauri::command]
pub fn open_card_link(
    app: tauri::AppHandle,
    card_id: String,
    index: Option<u32>,
) -> Result<LinkClick, String> {
    let st = crate::state::load_state(&app)?;
    let id = card_id.trim();
    let card = st
        .kanban_cards
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("unknown card id {id:?}"))?;
    let i = index.unwrap_or(0) as usize;
    let links = card.links.as_deref().unwrap_or(&[]);
    let link = links.get(i).ok_or_else(|| format!("no link at {i}"))?;
    click_link_at(&link.url, &approvals_path(&app)?)
}

/// First click. An exact remembered approval opens immediately. Anything
/// else returns a preview and opens nothing.
#[tauri::command]
pub fn click_link(app: tauri::AppHandle, url: String) -> Result<LinkClick, String> {
    click_link_at(&url, &approvals_path(&app)?)
}

/// Open / Run from the preview. Records the approval. Refuses when the
/// file no longer matches the kind and fingerprint the preview showed.
#[tauri::command]
pub fn open_link(
    app: tauri::AppHandle,
    url: String,
    kind: String,
    size: Option<u64>,
    modified_secs: Option<u64>,
    content_sha256: Option<String>,
    plist_sha256: Option<String>,
) -> Result<LinkClick, String> {
    let expect = LinkFingerprint {
        kind,
        size,
        modified_secs,
        content_sha256,
        plist_sha256,
    };
    open_link_checked(&url, &approvals_path(&app)?, &expect)
}

/// Reveal in Finder (`open -R`). Does not approve and does not run.
#[tauri::command]
pub fn reveal_link(url: String) -> Result<(), String> {
    let preview = preview_link(&url)?;
    reveal(&preview)
}

#[tauri::command]
pub fn list_link_approvals(app: tauri::AppHandle) -> Result<Vec<LinkApproval>, String> {
    Ok(load_approvals(&approvals_path(&app)?)?.approvals)
}

#[tauri::command]
pub fn remove_link_approval(app: tauri::AppHandle, target: String) -> Result<(), String> {
    remove_approval(&approvals_path(&app)?, &target)
}

#[tauri::command]
pub fn clear_link_approvals(app: tauri::AppHandle) -> Result<(), String> {
    clear_approvals(&approvals_path(&app)?)
}

fn scheme_of(s: &str) -> Option<&str> {
    let i = s.find(':')?;
    let sch = &s[..i];
    if !sch.is_empty() && sch.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(sch)
    } else {
        None
    }
}

fn normalize_https(raw: &str) -> Result<String, String> {
    let rest = strip_scheme(raw, "https")?;
    if rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#') {
        return Err("https link needs a host".into());
    }
    if rest.contains('@') || rest.contains('\\') || rest.contains('"') || rest.contains('\'')
        || rest.contains('<') || rest.contains('>') || rest.contains(' ')
    {
        return Err("link rejected".into());
    }
    let (hostport, tail) = split_host(rest)?;
    if !valid_hostport(hostport) {
        return Err("https link needs a host".into());
    }
    if !valid_https_tail(tail) {
        return Err("link rejected".into());
    }
    Ok(format!("https://{hostport}{tail}"))
}

fn split_host(rest: &str) -> Result<(&str, &str), String> {
    let cut = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let hostport = &rest[..cut];
    let tail = &rest[cut..];
    if hostport.is_empty() {
        return Err("https link needs a host".into());
    }
    Ok((hostport, tail))
}

fn valid_hostport(hp: &str) -> bool {
    let host = if let Some((h, p)) = hp.rsplit_once(':') {
        if h.is_empty() || p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        let n: u32 = match p.parse() {
            Ok(n) => n,
            Err(_) => return false,
        };
        if n == 0 || n > 65535 {
            return false;
        }
        h
    } else {
        hp
    };
    if host.is_empty() || host.len() > 253 || host.starts_with('.') || host.ends_with('.') {
        return false;
    }
    host.split('.').all(|lab| {
        !lab.is_empty()
            && lab.len() <= 63
            && lab.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !lab.starts_with('-')
            && !lab.ends_with('-')
    })
}

fn valid_https_tail(tail: &str) -> bool {
    if tail.is_empty() {
        return true;
    }
    if !tail.starts_with('/') && !tail.starts_with('?') && !tail.starts_with('#') {
        return false;
    }
    tail.bytes().all(|b| {
        matches!(
            b,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-' | b'.' | b'_' | b'~' | b'/' | b'?' | b'#'
                | b'[' | b']' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')'
                | b'*' | b'+' | b',' | b';' | b'=' | b'%'
        )
    })
}

fn normalize_file_url(raw: &str) -> Result<String, String> {
    let rest = strip_scheme(raw, "file")?;
    let path = if let Some(path) = rest.strip_prefix('/') {
        // file:///absolute — `rest` includes the leading slash.
        percent_decode(rest).or_else(|_| percent_decode(&format!("/{path}")))?
    } else if rest.is_empty() {
        return Err(not_absolute());
    } else if let Some(home_rel) = rest.strip_prefix('~') {
        // file://~/x — expand like a bare ~/x.
        expand_tilde(&format!("~{home_rel}"))
            .ok_or_else(not_absolute)
            .and_then(|p| percent_decode(&p))?
    } else {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if FILE_HOST_ROOTS.iter().any(|r| r.eq_ignore_ascii_case(host)) {
            // file://Users/dude/x → /Users/dude/x
            percent_decode(&format!("/{rest}"))?
        } else if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
            if path.is_empty() && !rest.contains('/') {
                return Err(not_absolute());
            }
            percent_decode(&format!("/{path}"))?
        } else {
            return Err(not_absolute());
        }
    };
    finish_file_path(&path)
}

fn finish_file_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err(not_absolute());
    }
    if path.chars().any(|c| c.is_control()) {
        return Err("link rejected".into());
    }
    for seg in path.split('/') {
        if seg == ".." {
            return Err(format!("file link may not use .. — {ACCEPTED}"));
        }
    }
    let encoded = encode_path(path);
    let url = format!("file://{encoded}");
    if url.len() > MAX_URL {
        return Err("link is too long".into());
    }
    Ok(url)
}

fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let path = percent_decode(rest).ok()?;
    if path.starts_with('/') {
        Some(PathBuf::from(path))
    } else {
        None
    }
}

fn strip_scheme<'a>(raw: &'a str, scheme: &str) -> Result<&'a str, String> {
    let i = raw.find(':').ok_or_else(must_be)?;
    if !raw[..i].eq_ignore_ascii_case(scheme) {
        return Err(must_be());
    }
    let rest = &raw[i + 1..];
    let rest = rest.strip_prefix("//").ok_or_else(must_be)?;
    Ok(rest)
}

fn percent_decode(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return Err("link rejected".into());
            }
            let h = hex_val(b[i + 1]).ok_or_else(|| "link rejected".to_string())?;
            let l = hex_val(b[i + 2]).ok_or_else(|| "link rejected".to_string())?;
            let byte = (h << 4) | l;
            if byte < 0x20 || byte == 0x7f {
                return Err("link rejected".into());
            }
            out.push(byte);
            i += 3;
        } else if b[i] < 0x20 || b[i] == 0x7f {
            return Err("link rejected".into());
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "link rejected".to_string())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Shown on an executable preview. The button reads Run.
pub const RUN_WARNING: &str = "Open will run a program.";

const EXCERPT_BYTES: usize = 64 * 1024;
const EXCERPT_LINES: usize = 20;
const THUMB_MAX: u64 = 256 * 1024;
/// Content hash cap for executables, scripts, and bundle main binaries.
/// Larger than this, the next click always previews.
const HASH_CAP: u64 = 64 * 1024 * 1024;

const EXEC_EXT: &[&str] = &[
    "app",
    "command",
    "sh",
    "zsh",
    "bash",
    "tool",
    "workflow",
    "terminal",
    "pkg",
    "dmg",
    "scpt",
    "applescript",
    "fileloc",
    "inetloc",
    "webloc",
    "jar",
    "mpkg",
    "shortcut",
    "mobileconfig",
    "osax",
    "action",
    "csh",
    "py",
];
const IMAGE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "tif", "tiff", "bmp", "ico",
];
const TEXT_EXT: &[&str] = &["md", "markdown", "txt", "text"];

const APPROVAL_NOTE: &str = "Links opened from Orbitty on this Mac. The same path or URL opens directly next time. A different path, a different URL, or a program whose contents changed asks again. Remove a row to ask again. This file is local and is not synced.";

#[cfg(test)]
thread_local! {
    static OPEN_LOG: RefCell<Option<Vec<Vec<String>>>> = const { RefCell::new(None) };
}

/// Test hook. While armed, macOS `open` is not spawned; calls are recorded.
#[cfg(test)]
pub struct OpenGuard;
#[cfg(test)]
impl OpenGuard {
    pub fn arm() -> Self {
        OPEN_LOG.with(|l| *l.borrow_mut() = Some(Vec::new()));
        OpenGuard
    }
    pub fn log() -> Vec<Vec<String>> {
        OPEN_LOG.with(|l| l.borrow().clone().unwrap_or_default())
    }
    pub fn clear() {
        OPEN_LOG.with(|l| {
            if let Some(v) = l.borrow_mut().as_mut() {
                v.clear();
            }
        });
    }
}
#[cfg(test)]
impl Drop for OpenGuard {
    fn drop(&mut self) {
        OPEN_LOG.with(|l| *l.borrow_mut() = None);
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    pub kind: String,
    pub target: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// sha256 of the executable, script, or bundle main binary. Absent when
    /// the file is over [`HASH_CAP`] or is not an executable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    /// sha256 of `Contents/Info.plist` for a bundle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plist_sha256: Option<String>,
}

/// What the preview showed. Open / Run refuses when the live target differs.
pub(crate) struct LinkFingerprint {
    kind: String,
    size: Option<u64>,
    modified_secs: Option<u64>,
    content_sha256: Option<String>,
    plist_sha256: Option<String>,
}

impl LinkFingerprint {
    #[cfg_attr(not(test), allow(dead_code))]
    fn from_preview(p: &LinkPreview) -> Self {
        Self {
            kind: p.kind.clone(),
            size: p.size,
            modified_secs: p.modified_secs,
            content_sha256: p.content_sha256.clone(),
            plist_sha256: p.plist_sha256.clone(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkClick {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<LinkPreview>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkApproval {
    pub target: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plist_sha256: Option<String>,
    pub approved_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkApprovalFile {
    pub note: String,
    pub approvals: Vec<LinkApproval>,
}

impl Default for LinkApprovalFile {
    fn default() -> Self {
        Self {
            note: APPROVAL_NOTE.to_string(),
            approvals: Vec::new(),
        }
    }
}

/// `link-approvals.json` beside `state.json`. Not part of the synced state.
pub fn approvals_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("link-approvals.json"))
}

/// One preview for a link. No network. Does not launch anything.
pub fn preview_link(raw: &str) -> Result<LinkPreview, String> {
    let url = normalize_link(raw)?;
    if let Some(path) = file_url_to_path(&url) {
        Ok(preview_path(&path))
    } else {
        let host = url
            .strip_prefix("https://")
            .unwrap_or(&url)
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .to_string();
        Ok(LinkPreview {
            kind: "https".into(),
            name: host.clone(),
            target: url,
            host: Some(host),
            size: None,
            modified: None,
            modified_secs: None,
            excerpt: None,
            warning: None,
            thumbnail: None,
            content_sha256: None,
            plist_sha256: None,
        })
    }
}

pub fn click_link_at(raw: &str, approvals: &Path) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    if preview.kind != "missing" {
        let file = load_approvals(approvals)?;
        if approval_matches(&preview, &file) {
            launch(&preview)?;
            return Ok(LinkClick {
                action: "opened".into(),
                preview: None,
            });
        }
    }
    Ok(LinkClick {
        action: "preview".into(),
        preview: Some(preview),
    })
}

/// Open / Run against the fingerprint taken just now. Tests use this for
/// "Matt clicked Run on the preview he is looking at."
#[cfg_attr(not(test), allow(dead_code))]
pub fn open_link_at(raw: &str, approvals: &Path) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    let expect = LinkFingerprint::from_preview(&preview);
    open_link_checked(raw, approvals, &expect)
}

/// Open / Run only when `expect` still matches the live target. A mismatch
/// opens nothing and returns the new preview. Only this path records an
/// approval — ctl and MCP never do.
pub(crate) fn open_link_checked(
    raw: &str,
    approvals: &Path,
    expect: &LinkFingerprint,
) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    if preview.kind == "missing" || !fingerprint_eq(&preview, expect) {
        return Ok(LinkClick {
            action: "preview".into(),
            preview: Some(preview),
        });
    }
    launch(&preview)?;
    remember(&preview, approvals)?;
    Ok(LinkClick {
        action: "opened".into(),
        preview: None,
    })
}

fn fingerprint_eq(preview: &LinkPreview, expect: &LinkFingerprint) -> bool {
    preview.kind == expect.kind
        && preview.size == expect.size
        && preview.modified_secs == expect.modified_secs
        && preview.content_sha256 == expect.content_sha256
        && preview.plist_sha256 == expect.plist_sha256
}

/// orbittyctl and MCP `kanban open-link`. Documents, folders, images, and
/// https open. An executable is revealed unless `run` or an exact approval
/// still matches. Reads the approvals list and never writes it.
pub fn ctl_open_at(raw: &str, run: bool, approvals: &Path) -> Result<String, String> {
    let preview = preview_link(raw)?;
    if preview.kind == "missing" {
        return Err("file not found".into());
    }
    let file = load_approvals(approvals)?;
    let do_open = preview.kind != "executable" || run || approval_matches(&preview, &file);
    if do_open {
        launch(&preview)?;
        Ok("open".into())
    } else {
        reveal(&preview)?;
        Ok("reveal".into())
    }
}

pub fn approval_matches(preview: &LinkPreview, file: &LinkApprovalFile) -> bool {
    let Some(row) = file.approvals.iter().find(|a| a.target == preview.target) else {
        return false;
    };
    if preview.kind == "executable" {
        return row.kind == "executable"
            && row.size.is_some()
            && row.size == preview.size
            && row.modified_secs == preview.modified_secs
            && preview.content_sha256.is_some()
            && row.content_sha256 == preview.content_sha256
            && row.plist_sha256 == preview.plist_sha256;
    }
    true
}

pub fn load_approvals(path: &Path) -> Result<LinkApprovalFile, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(LinkApprovalFile::default()),
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LinkApprovalFile::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

pub fn remove_approval(path: &Path, target: &str) -> Result<(), String> {
    let mut file = load_approvals(path)?;
    let target = resolve_approval_key(target)?;
    file.approvals.retain(|a| a.target != target);
    save_approvals(path, &file)
}

pub fn clear_approvals(path: &Path) -> Result<(), String> {
    save_approvals(path, &LinkApprovalFile::default())
}

fn resolve_approval_key(raw: &str) -> Result<String, String> {
    let preview = preview_link(raw).unwrap_or(LinkPreview {
        kind: "document".into(),
        target: raw.to_string(),
        name: raw.to_string(),
        host: None,
        size: None,
        modified: None,
        modified_secs: None,
        excerpt: None,
        warning: None,
        thumbnail: None,
        content_sha256: None,
        plist_sha256: None,
    });
    if preview.target.is_empty() {
        Ok(raw.to_string())
    } else {
        Ok(preview.target)
    }
}

fn remember(preview: &LinkPreview, path: &Path) -> Result<(), String> {
    let mut file = load_approvals(path)?;
    file.note = APPROVAL_NOTE.to_string();
    file.approvals.retain(|a| a.target != preview.target);
    file.approvals.push(LinkApproval {
        target: preview.target.clone(),
        kind: preview.kind.clone(),
        size: preview.size,
        modified_secs: preview.modified_secs,
        content_sha256: preview.content_sha256.clone(),
        plist_sha256: preview.plist_sha256.clone(),
        approved_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    });
    save_approvals(path, &file)
}

fn save_approvals(path: &Path, file: &LinkApprovalFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

fn preview_path(path: &Path) -> LinkPreview {
    if !path.exists() {
        return bare_preview("missing", path, None);
    }
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let meta = match std::fs::metadata(&resolved) {
        Ok(m) => m,
        Err(_) => return bare_preview("missing", &resolved, None),
    };
    if resolved.is_dir() {
        if is_bundle_dir(&resolved) {
            return bundle_preview(&resolved);
        }
        let (modified_secs, modified) = modified_pair(&meta);
        return file_preview(
            "folder",
            &resolved,
            Some(meta.len()),
            modified,
            modified_secs,
            None,
            None,
        );
    }
    let (modified_secs, modified) = modified_pair(&meta);
    let size = Some(meta.len());
    if ext_in(&resolved, EXEC_EXT) || has_exec_bit(&meta) {
        let mut preview = file_preview(
            "executable",
            &resolved,
            size,
            modified,
            modified_secs,
            Some(RUN_WARNING.to_string()),
            None,
        );
        preview.content_sha256 = hash_file_capped(&resolved, meta.len());
        return preview;
    }
    if ext_in(&resolved, IMAGE_EXT) {
        let thumb = thumbnail_data_url(&resolved, meta.len());
        return file_preview(
            "image",
            &resolved,
            size,
            modified,
            modified_secs,
            None,
            thumb,
        );
    }
    let excerpt = if ext_in(&resolved, TEXT_EXT) {
        read_excerpt(&resolved)
    } else {
        None
    };
    file_preview(
        "document",
        &resolved,
        size,
        modified,
        modified_secs,
        None,
        excerpt,
    )
}

fn modified_pair(meta: &std::fs::Metadata) -> (Option<u64>, Option<String>) {
    match meta.modified().ok() {
        Some(t) => {
            let secs = t
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let shown = chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M")
                .to_string();
            (Some(secs), Some(shown))
        }
        None => (None, None),
    }
}

/// Bundle fingerprint is the main executable plus Info.plist, never the
/// bundle directory's own size or mtime.
fn bundle_preview(bundle: &Path) -> LinkPreview {
    let plist_path = bundle.join("Contents").join("Info.plist");
    let plist_sha256 = hash_plist(&plist_path);
    let exe = bundle_main_executable(bundle, &plist_path);
    let (size, modified, modified_secs, content_sha256) = match exe {
        Some(path) => match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() => {
                let (modified_secs, modified) = modified_pair(&meta);
                let sha = hash_file_capped(&path, meta.len());
                (Some(meta.len()), modified, modified_secs, sha)
            }
            _ => (None, None, None, None),
        },
        None => (None, None, None, None),
    };
    let mut preview = file_preview(
        "executable",
        bundle,
        size,
        modified,
        modified_secs,
        Some(RUN_WARNING.to_string()),
        None,
    );
    // No main-binary hash, or an Info.plist we could not hash, means the
    // next click always previews.
    preview.content_sha256 = content_sha256;
    preview.plist_sha256 = plist_sha256;
    if plist_path.is_file() && preview.plist_sha256.is_none() {
        preview.content_sha256 = None;
    }
    if preview.content_sha256.is_none() {
        preview.plist_sha256 = None;
    }
    preview
}

fn hash_plist(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    hash_file_capped(path, meta.len())
}

fn bundle_main_executable(bundle: &Path, plist_path: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(plist_path).ok()?;
    let name = cfbundle_executable(&bytes)?;
    if !exe_name_ok(&name) {
        return None;
    }
    Some(bundle.join("Contents").join("MacOS").join(name))
}

fn exe_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name != "."
        && name != ".."
}

fn cfbundle_executable(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"bplist00") {
        return bplist_dict_string(bytes, "CFBundleExecutable");
    }
    xml_cfbundle_executable(bytes)
}

fn xml_cfbundle_executable(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let key_at = text.find("CFBundleExecutable")?;
    let rest = &text[key_at..];
    let tag = "<string>";
    let start = rest.find(tag)? + tag.len();
    let end = start + rest[start..].find("</string>")?;
    let name = decode_xml_basic(rest[start..end].trim());
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn decode_xml_basic(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn bplist_dict_string(data: &[u8], want: &str) -> Option<String> {
    if data.len() < 40 || &data[..8] != b"bplist00" {
        return None;
    }
    let t = data.len() - 32;
    let offset_size = data[t + 6] as usize;
    let ref_size = data[t + 7] as usize;
    if !(1..=8).contains(&offset_size) || !(1..=8).contains(&ref_size) {
        return None;
    }
    let num_objects = be_int(&data[t + 8..t + 16])?;
    let top = be_int(&data[t + 16..t + 24])?;
    let table_off = be_int(&data[t + 24..t + 32])?;
    if num_objects == 0 || num_objects > 100_000 || top >= num_objects {
        return None;
    }
    let mut offsets = Vec::with_capacity(num_objects);
    for i in 0..num_objects {
        let start = table_off.checked_add(i.checked_mul(offset_size)?)?;
        let end = start.checked_add(offset_size)?;
        if end > data.len() {
            return None;
        }
        offsets.push(be_int(&data[start..end])?);
    }
    let off = *offsets.get(top)?;
    if off >= data.len() || data[off] >> 4 != 0xD {
        return None;
    }
    let mut i = off + 1;
    let n = read_count(data, &mut i, data[off] & 0x0f)?;
    if n > 10_000 {
        return None;
    }
    let mut keys = Vec::with_capacity(n);
    let mut vals = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(read_ref(data, &mut i, ref_size)?);
    }
    for _ in 0..n {
        vals.push(read_ref(data, &mut i, ref_size)?);
    }
    for (k, v) in keys.into_iter().zip(vals) {
        let koff = *offsets.get(k)?;
        let voff = *offsets.get(v)?;
        let Some(ks) = parse_bplist_string(data, koff) else {
            continue;
        };
        if ks == want {
            return parse_bplist_string(data, voff);
        }
    }
    None
}

fn read_count(data: &[u8], i: &mut usize, low: u8) -> Option<usize> {
    if low != 0x0f {
        return Some(low as usize);
    }
    if *i >= data.len() {
        return None;
    }
    let marker = data[*i];
    *i += 1;
    if marker >> 4 != 0x1 {
        return None;
    }
    let nbytes = 1usize << (marker & 0x0f);
    if nbytes == 0 || nbytes > 8 || *i + nbytes > data.len() {
        return None;
    }
    let n = be_int(&data[*i..*i + nbytes])?;
    *i += nbytes;
    Some(n)
}

fn read_ref(data: &[u8], i: &mut usize, ref_size: usize) -> Option<usize> {
    if *i + ref_size > data.len() {
        return None;
    }
    let n = be_int(&data[*i..*i + ref_size])?;
    *i += ref_size;
    Some(n)
}

fn parse_bplist_string(data: &[u8], off: usize) -> Option<String> {
    if off >= data.len() {
        return None;
    }
    let marker = data[off];
    let mut i = off + 1;
    let n = read_count(data, &mut i, marker & 0x0f)?;
    match marker >> 4 {
        0x5 => {
            if i + n > data.len() {
                return None;
            }
            String::from_utf8(data[i..i + n].to_vec()).ok()
        }
        0x6 => {
            let bytes = n.checked_mul(2)?;
            if i + bytes > data.len() {
                return None;
            }
            let units: Vec<u16> = data[i..i + bytes]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => None,
    }
}

fn be_int(b: &[u8]) -> Option<usize> {
    if b.is_empty() || b.len() > 8 {
        return None;
    }
    let mut n: u64 = 0;
    for byte in b {
        n = n.checked_shl(8)?.checked_add(u64::from(*byte))?;
    }
    usize::try_from(n).ok()
}

fn hash_file_capped(path: &Path, len: u64) -> Option<String> {
    if len > HASH_CAP {
        return None;
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut left = len;
    while left > 0 {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        let n = n.min(left as usize);
        hasher.update(&buf[..n]);
        left -= n as u64;
    }
    if left != 0 {
        return None;
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn bare_preview(kind: &str, path: &Path, excerpt: Option<String>) -> LinkPreview {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.display().to_string());
    LinkPreview {
        kind: kind.into(),
        target: path.display().to_string(),
        name,
        host: None,
        size: None,
        modified: None,
        modified_secs: None,
        excerpt,
        warning: None,
        thumbnail: None,
        content_sha256: None,
        plist_sha256: None,
    }
}

fn file_preview(
    kind: &str,
    path: &Path,
    size: Option<u64>,
    modified: Option<String>,
    modified_secs: Option<u64>,
    warning: Option<String>,
    extra: Option<String>,
) -> LinkPreview {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let (excerpt, thumbnail) = if kind == "image" {
        (None, extra)
    } else {
        (extra, None)
    };
    let warning = if kind == "executable" {
        Some(warning.unwrap_or_else(|| RUN_WARNING.to_string()))
    } else {
        warning
    };
    LinkPreview {
        kind: kind.into(),
        target: path.display().to_string(),
        name,
        host: None,
        size,
        modified,
        modified_secs,
        excerpt,
        warning,
        thumbnail,
        content_sha256: None,
        plist_sha256: None,
    }
}

fn is_bundle_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    if ext_in(path, EXEC_EXT) {
        return true;
    }
    path.join("Contents").join("Info.plist").is_file()
}

fn ext_in(path: &Path, list: &[&str]) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    list.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

fn has_exec_bit(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

fn read_excerpt(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; EXCERPT_BYTES];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let lines: Vec<&str> = text.lines().take(EXCERPT_LINES).collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

fn thumbnail_data_url(path: &Path, size: u64) -> Option<String> {
    if size == 0 || size > THUMB_MAX {
        return None;
    }
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", b64(&bytes)))
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let mut n = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn launch(preview: &LinkPreview) -> Result<(), String> {
    if preview.kind == "missing"
        || (preview.kind != "https" && !Path::new(&preview.target).exists())
    {
        return Err("file not found".into());
    }
    spawn_open(&[preview.target.as_str()])
}

fn reveal(preview: &LinkPreview) -> Result<(), String> {
    if preview.kind == "https" {
        return Err("reveal is for files".into());
    }
    if preview.kind == "missing" || !Path::new(&preview.target).exists() {
        return Err("file not found".into());
    }
    spawn_open(&["-R", preview.target.as_str()])
}

fn spawn_open(args: &[&str]) -> Result<(), String> {
    #[cfg(test)]
    {
        let hooked = OPEN_LOG.with(|log| {
            let mut slot = log.borrow_mut();
            if let Some(v) = slot.as_mut() {
                v.push(args.iter().map(|s| (*s).to_string()).collect());
                true
            } else {
                false
            }
        });
        if hooked {
            return Ok(());
        }
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("open: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("open is wired for macOS only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn rejects_hostile_schemes() {
        for raw in [
            "javascript:alert(1)",
            "JAVASCRIPT:alert(1)",
            "data:text/html,hi",
            "http://example.com",
            "HTTP://example.com",
            "vbscript:msg",
            "blob:https://example.com/x",
        ] {
            let err = normalize_link(raw).unwrap_err();
            assert!(
                err.contains("not allowed") || err.contains("link rejected"),
                "{raw} -> {err}"
            );
            assert!(
                err.contains(ACCEPTED),
                "{raw} error should say what is accepted: {err}"
            );
            let scheme = raw.split(':').next().unwrap().to_ascii_lowercase();
            assert!(
                err.contains(&scheme),
                "{raw} error {err} should name {scheme}"
            );
        }
    }

    #[test]
    fn allows_https_and_file() {
        assert_eq!(
            normalize_link("https://example.com/a?b=1").unwrap(),
            "https://example.com/a?b=1"
        );
        assert_eq!(
            normalize_link("HTTPS://Example.com/Path").unwrap(),
            "https://Example.com/Path"
        );
        let go = "/Users/dude/Orbitty GOs/GO-ORBITTY-CARD-LINKS_2026-09-25.md";
        let want = "file:///Users/dude/Orbitty%20GOs/GO-ORBITTY-CARD-LINKS_2026-09-25.md";
        assert_eq!(normalize_link(go).unwrap(), want);
        assert_eq!(normalize_link(want).unwrap(), want);
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty GOs/x.md").unwrap(),
            "file:///Users/dude/Orbitty%20GOs/x.md"
        );
    }

    #[test]
    fn rejects_relative_and_dotdot() {
        assert!(normalize_link("notes.md").is_err());
        assert!(normalize_link("file://relative").is_err());
        assert!(normalize_link("file:///Users/dude/../../etc/passwd").is_err());
        assert!(normalize_link("https://").is_err());
        assert!(normalize_link("https://user@example.com").is_err());
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = std::env::var("HOME").unwrap();
        let want = format!(
            "file://{}",
            encode_path(&format!("{home}/Orbitty GOs/x.md"))
        );
        assert_eq!(normalize_link("~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("  ~/Orbitty GOs/x.md ").unwrap(), want);
        assert_eq!(normalize_link("file://~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(
            normalize_link("~").unwrap(),
            format!("file://{}", encode_path(&home))
        );
        // ~user is not expanded, and the message says what works.
        let err = normalize_link("~root/x").unwrap_err();
        assert!(err.contains(ACCEPTED), "{err}");
        assert!(normalize_link("~/../../etc/passwd").is_err());
    }

    #[test]
    fn file_url_forms_normalize() {
        let want = "file:///Users/dude/Orbitty%20GOs/x.md";
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty%20GOs/x.md").unwrap(),
            want
        );
        // Two-slash typo: the root folder landed in the host slot.
        assert_eq!(
            normalize_link("file://Users/dude/Orbitty GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("FILE://Users/dude/Orbitty%20GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("file://Volumes/Disk/a b.pdf").unwrap(),
            "file:///Volumes/Disk/a%20b.pdf"
        );
        assert_eq!(
            normalize_link("file://localhost/tmp/a.md").unwrap(),
            "file:///tmp/a.md"
        );
        assert_eq!(
            normalize_link("/Users/dude/x.md").unwrap(),
            "file:///Users/dude/x.md"
        );
        // Idempotent on its own output.
        let once = normalize_link("file://Users/dude/Orbitty GOs/x.md").unwrap();
        assert_eq!(normalize_link(&once).unwrap(), once);
        // Still rejected: unknown host, relative, dot-dot via the typo form.
        assert!(normalize_link("file://example.com/x").is_err());
        assert!(normalize_link("file://relative").is_err());
        assert!(normalize_link("file://Users/dude/../../etc/passwd").is_err());
    }

    #[test]
    fn rejections_say_what_is_accepted() {
        for raw in ["notes.md", "file://relative", "ftp://x.org/a", ""] {
            let err = normalize_link(raw).unwrap_err();
            assert!(err.contains(ACCEPTED), "{raw:?} -> {err}");
        }
    }

    #[test]
    fn empty_links_stay_absent() {
        assert_eq!(canonicalize_links(None).unwrap(), None);
        assert_eq!(canonicalize_links(Some(vec![])).unwrap(), None);
    }

    #[test]
    fn old_card_json_omits_links() {
        let raw = r#"{"id":"c1","title":"T","projectId":"p","status":"todo","createdAt":1.0,"lastTouchedAt":1.0,"goalAligned":true,"clinicalOrMaster":false,"pointsAwarded":0}"#;
        let card: crate::state::KanbanCard = serde_json::from_str(raw).unwrap();
        assert!(card.links.is_none());
        let v = serde_json::to_value(&card).unwrap();
        assert!(v.get("links").is_none());
    }

    #[test]
    fn missing_file_is_a_clear_error() {
        let err = open_url("file:///tmp/orbitty-card-links-no-such-file-zz.md").unwrap_err();
        assert_eq!(err, "file not found");
    }

    #[test]
    fn cap_is_eight() {
        let links = (0..9)
            .map(|i| CardLink {
                url: format!("https://example.com/{i}"),
                label: None,
            })
            .collect();
        let err = canonicalize_links(Some(links)).unwrap_err();
        assert!(err.contains("8"));
    }

    fn temp_case(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "orbitty-link-safe-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn set_mode(path: &Path, mode: u32) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(mode);
            std::fs::set_permissions(path, perms).unwrap();
        }
        let _ = (path, mode);
    }

    #[test]
    fn preview_kind_table() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("kinds");
        let md = dir.join("note.md");
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("line {i}\n"));
        }
        std::fs::write(&md, &body).unwrap();
        let folder = dir.join("papers");
        std::fs::create_dir(&folder).unwrap();
        let png = dir.join("dot.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nnot-really").unwrap();
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho hi\n").unwrap();
        set_mode(&cmd, 0o755);
        let cmd_nox = dir.join("plain.command");
        std::fs::write(&cmd_nox, "echo hi\n").unwrap();
        set_mode(&cmd_nox, 0o644);
        let sh = dir.join("run.sh");
        std::fs::write(&sh, "echo hi\n").unwrap();
        set_mode(&sh, 0o644);
        let plus = dir.join("plain.txt");
        std::fs::write(&plus, "text\n").unwrap();
        set_mode(&plus, 0o755);
        let app = dir.join("Demo.app");
        std::fs::create_dir(&app).unwrap();
        let bundle = dir.join("Widget");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), b"<plist/>").unwrap();
        let pdf = dir.join("a.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let big = dir.join("big.md");
        let mut big_body = vec![b'a'; EXCERPT_BYTES + 100];
        big_body.push(b'\n');
        big_body.extend_from_slice(b"TAIL\n");
        std::fs::write(&big, &big_body).unwrap();
        let mut run_exts = Vec::new();
        for ext in [
            "fileloc",
            "inetloc",
            "webloc",
            "jar",
            "mpkg",
            "shortcut",
            "mobileconfig",
            "osax",
            "action",
            "csh",
            "py",
        ] {
            let p = dir.join(format!("sample.{ext}"));
            std::fs::write(&p, b"not a document\n").unwrap();
            set_mode(&p, 0o644);
            run_exts.push(p);
        }

        let mut cases = vec![
            (md.to_string_lossy().to_string(), "document"),
            (folder.to_string_lossy().to_string(), "folder"),
            (png.to_string_lossy().to_string(), "image"),
            (cmd.to_string_lossy().to_string(), "executable"),
            (cmd_nox.to_string_lossy().to_string(), "executable"),
            (sh.to_string_lossy().to_string(), "executable"),
            (plus.to_string_lossy().to_string(), "executable"),
            (app.to_string_lossy().to_string(), "executable"),
            (bundle.to_string_lossy().to_string(), "executable"),
            (pdf.to_string_lossy().to_string(), "document"),
            (
                "/tmp/orbitty-link-safe-no-such-file-zz.md".into(),
                "missing",
            ),
            ("https://example.com/a?b=1".into(), "https"),
        ];
        for p in &run_exts {
            cases.push((p.to_string_lossy().to_string(), "executable"));
        }
        for (raw, kind) in cases {
            let p = preview_link(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(p.kind, kind, "{raw}");
            assert!(
                OpenGuard::log().is_empty(),
                "preview spawned open for {raw}"
            );
        }

        let doc = preview_link(&md.to_string_lossy()).unwrap();
        let excerpt = doc.excerpt.clone().unwrap();
        let lines: Vec<_> = excerpt.lines().collect();
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[0], "line 0");
        assert_eq!(doc.target, md.canonicalize().unwrap().to_string_lossy());
        assert!(doc.size.is_some() && doc.modified.is_some());

        let img = preview_link(&png.to_string_lossy()).unwrap();
        assert!(img.thumbnail.unwrap().starts_with("data:image/png;base64,"));

        let exe = preview_link(&cmd.to_string_lossy()).unwrap();
        assert_eq!(exe.warning.as_deref(), Some(RUN_WARNING));
        assert!(exe.excerpt.is_none());
        assert!(exe.content_sha256.is_some());
        let nox = preview_link(&cmd_nox.to_string_lossy()).unwrap();
        assert_eq!(nox.kind, "executable");
        assert_eq!(nox.warning.as_deref(), Some(RUN_WARNING));
        for p in &run_exts {
            let row = preview_link(&p.to_string_lossy()).unwrap();
            assert_eq!(row.warning.as_deref(), Some(RUN_WARNING), "{}", p.display());
        }

        let web = preview_link("https://example.com/a?b=1").unwrap();
        assert_eq!(web.target, "https://example.com/a?b=1");
        assert_eq!(web.host.as_deref(), Some("example.com"));
        assert!(web.size.is_none() && web.excerpt.is_none());

        let capped = preview_link(&big.to_string_lossy()).unwrap();
        let excerpt = capped.excerpt.unwrap();
        assert!(!excerpt.contains("TAIL"));
        assert!(excerpt.len() <= EXCERPT_BYTES);

        let pdf_p = preview_link(&pdf.to_string_lossy()).unwrap();
        assert!(pdf_p.excerpt.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn click_previews_and_run_does_not_execute_fixture() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("run");
        let approvals = dir.join("link-approvals.json");
        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let md = dir.join("note.md");
        std::fs::write(&md, "hello\nsecond\n").unwrap();
        let folder = dir.join("dir");
        std::fs::create_dir(&folder).unwrap();

        let first = click_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        assert_eq!(first.action, "preview");
        assert_eq!(first.preview.unwrap().kind, "executable");
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        let md_click = click_link_at(&md.to_string_lossy(), &approvals).unwrap();
        assert_eq!(md_click.action, "preview");
        let web = click_link_at("https://example.com/docs", &approvals).unwrap();
        assert_eq!(web.action, "preview");
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        let canon = md.canonicalize().unwrap().to_string_lossy().to_string();
        assert_eq!(OpenGuard::log(), vec![vec![canon.clone()]]);
        OpenGuard::clear();
        reveal(&preview_link(&folder.to_string_lossy()).unwrap()).unwrap();
        assert_eq!(
            OpenGuard::log(),
            vec![vec![
                "-R".into(),
                folder.canonicalize().unwrap().to_string_lossy().to_string()
            ]]
        );
        OpenGuard::clear();
        open_link_at("https://example.com/docs", &approvals).unwrap();
        assert_eq!(
            OpenGuard::log(),
            vec![vec!["https://example.com/docs".to_string()]]
        );

        OpenGuard::clear();
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let cmd_canon = cmd.canonicalize().unwrap().to_string_lossy().to_string();
        assert_eq!(OpenGuard::log(), vec![vec![cmd_canon]]);
        assert!(!sentinel.exists(), "fixture .command must not execute");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remembered_approval_is_exact_and_stale_executable_asks_again() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("memory");
        let approvals = dir.join("link-approvals.json");
        let md = dir.join("note.md");
        std::fs::write(&md, "one\n").unwrap();
        let other = dir.join("other.md");
        std::fs::write(&other, "two\n").unwrap();
        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );
        assert_eq!(OpenGuard::log().len(), 1);

        OpenGuard::clear();
        assert_eq!(
            click_link_at(&other.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        let https = "https://example.com/a?b=1";
        open_link_at(https, &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(https, &approvals).unwrap().action, "opened");
        OpenGuard::clear();
        assert_eq!(
            click_link_at("https://example.com/a?b=2", &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        #[cfg(unix)]
        {
            let same = dir.join("same.md");
            std::os::unix::fs::symlink(&md, &same).unwrap();
            OpenGuard::clear();
            assert_eq!(
                click_link_at(&same.to_string_lossy(), &approvals)
                    .unwrap()
                    .action,
                "opened"
            );
            let elsewhere = dir.join("else.md");
            std::os::unix::fs::symlink(&other, &elsewhere).unwrap();
            OpenGuard::clear();
            assert_eq!(
                click_link_at(&elsewhere.to_string_lossy(), &approvals)
                    .unwrap()
                    .action,
                "preview"
            );
            assert!(OpenGuard::log().is_empty());
        }

        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );
        assert!(!sentinel.exists());

        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran-more > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let f = std::fs::File::options().write(true).open(&cmd).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(180))
            .unwrap();
        drop(f);
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        let canon = md.canonicalize().unwrap().to_string_lossy().to_string();
        remove_approval(&approvals, &canon).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        clear_approvals(&approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        let file = load_approvals(&approvals).unwrap();
        assert!(file.approvals.is_empty());
        assert!(file.note.contains("not synced"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ctl_reveals_executables_unless_run_or_approved() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("ctl");
        let approvals = dir.join("link-approvals.json");
        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let md = dir.join("note.md");
        std::fs::write(&md, "doc\n").unwrap();

        assert_eq!(
            ctl_open_at(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            OpenGuard::log()[0],
            vec![md.canonicalize().unwrap().to_string_lossy().to_string()]
        );

        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "reveal"
        );
        assert_eq!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());

        let fresh = dir.join("fresh.json");
        let fresh_before = snap(&fresh);
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), true, &fresh).unwrap(),
            "open"
        );
        assert_ne!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());
        assert_eq!(snap(&fresh), fresh_before, "--run must not write approvals");
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &fresh).unwrap(),
            "reveal"
        );
        assert_eq!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());

        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let approved = snap(&approvals);
        assert!(!approved.is_empty());
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            snap(&approvals),
            approved,
            "honoring an approval must not rewrite it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn snap(path: &Path) -> Vec<u8> {
        match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => panic!("read {}: {e}", path.display()),
        }
    }

    fn plist_xml(exe: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <plist version=\"1.0\"><dict>\n\
             <key>CFBundleExecutable</key><string>{exe}</string>\n\
             </dict></plist>\n"
        )
    }

    fn write_app(dir: &Path, body: &[u8]) -> PathBuf {
        let app = dir.join("Demo.app");
        let macos = app.join("Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::write(app.join("Contents/Info.plist"), plist_xml("Demo")).unwrap();
        let bin = macos.join("Demo");
        std::fs::write(&bin, body).unwrap();
        set_mode(&bin, 0o755);
        app
    }

    #[test]
    fn open_card_link_previews_not_opens() {
        let src = include_str!("card_links.rs");
        let start = src.find("pub fn open_card_link(").expect("open_card_link");
        let rest = &src[start..];
        let end = rest
            .find("\npub fn click_link(")
            .expect("click_link follows");
        let body = &rest[..end];
        assert!(body.contains("click_link_at("), "{body}");
        assert!(!body.contains("open_link_at("), "{body}");
        assert!(!body.contains("open_url("), "{body}");
    }

    #[test]
    fn ctl_and_mcp_open_leave_approvals_byte_identical() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("bytes");
        let approvals = dir.join("link-approvals.json");
        let seeded = b"{\n  \"note\": \"seed\",\n  \"approvals\": []\n}\n";
        std::fs::write(&approvals, seeded).unwrap();
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho hi\n").unwrap();
        set_mode(&cmd, 0o644);
        let md = dir.join("note.md");
        std::fs::write(&md, "doc\n").unwrap();

        let before = snap(&approvals);
        assert_eq!(
            ctl_open_at(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), true, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&cmd.to_string_lossy(), true, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "reveal"
        );
        assert_eq!(snap(&approvals), before);

        let click = click_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        assert_eq!(click.action, "preview");
        assert_eq!(click.preview.unwrap().kind, "executable");
        assert_eq!(snap(&approvals), before);
        assert!(OpenGuard::log()
            .iter()
            .any(|c| c.first().map(String::as_str) == Some("-R")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundle_fingerprint_is_main_executable_and_plist() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("bundle");
        let approvals = dir.join("link-approvals.json");
        let app = write_app(&dir, b"#!/bin/sh\necho one\n");
        let bin = app.join("Contents/MacOS/Demo");
        let plist = app.join("Contents/Info.plist");
        let preview = preview_link(&app.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_some());
        assert!(preview.plist_sha256.is_some());
        assert_eq!(preview.size, Some(std::fs::metadata(&bin).unwrap().len()));

        open_link_at(&app.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        let folder = std::fs::File::open(&app).unwrap();
        folder
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(900))
            .unwrap();
        drop(folder);
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );

        let old_mtime = std::fs::metadata(&bin).unwrap().modified().unwrap();
        let old_len = std::fs::metadata(&bin).unwrap().len();
        let mut swapped = std::fs::read(&bin).unwrap();
        assert!(!swapped.is_empty());
        swapped[0] ^= 0x01;
        assert_eq!(swapped.len() as u64, old_len);
        std::fs::write(&bin, &swapped).unwrap();
        set_mode(&bin, 0o755);
        let f = std::fs::File::options().write(true).open(&bin).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        let meta = std::fs::metadata(&bin).unwrap();
        assert_eq!(meta.len(), old_len);
        assert_eq!(
            meta.modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            old_mtime
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        std::fs::write(&bin, b"#!/bin/sh\necho one\n").unwrap();
        set_mode(&bin, 0o755);
        let f = std::fs::File::options().write(true).open(&bin).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        open_link_at(&app.to_string_lossy(), &approvals).unwrap();
        std::fs::write(&plist, format!("{}<!-- edited -->\n", plist_xml("Demo"))).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_plist_names_the_main_executable() {
        let dir = temp_case("bplist");
        let app = write_app(&dir, b"#!/bin/sh\necho bin\n");
        let plist = app.join("Contents/Info.plist");
        let status = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "binary1"])
            .arg(&plist)
            .status()
            .unwrap();
        assert!(status.success());
        let bytes = std::fs::read(&plist).unwrap();
        assert!(bytes.starts_with(b"bplist00"));
        assert_eq!(cfbundle_executable(&bytes).as_deref(), Some("Demo"));
        let preview = preview_link(&app.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_some());
        assert!(preview.plist_sha256.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_size_rewrite_with_mtime_restored_asks_again() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("hash");
        let approvals = dir.join("link-approvals.json");
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho one\n").unwrap();
        set_mode(&cmd, 0o644);
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let row = load_approvals(&approvals)
            .unwrap()
            .approvals
            .into_iter()
            .find(|a| a.kind == "executable")
            .unwrap();
        let old_mtime = std::fs::metadata(&cmd).unwrap().modified().unwrap();
        let mut body = std::fs::read(&cmd).unwrap();
        body[8] ^= 0x01;
        std::fs::write(&cmd, &body).unwrap();
        set_mode(&cmd, 0o644);
        let f = std::fs::File::options().write(true).open(&cmd).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        let meta = std::fs::metadata(&cmd).unwrap();
        assert_eq!(Some(meta.len()), row.size);
        assert_eq!(
            Some(
                meta.modified()
                    .unwrap()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            row.modified_secs
        );
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn over_cap_executable_always_previews() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("cap");
        let approvals = dir.join("link-approvals.json");
        let big = dir.join("big.command");
        let f = std::fs::File::create(&big).unwrap();
        f.set_len(HASH_CAP + 1).unwrap();
        drop(f);
        set_mode(&big, 0o644);
        let preview = preview_link(&big.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_none());
        open_link_at(&big.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&big.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_link_refuses_when_target_changed_since_preview() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("stale");
        let approvals = dir.join("link-approvals.json");
        std::fs::write(&approvals, b"{\"note\":\"n\",\"approvals\":[]}\n").unwrap();
        let md = dir.join("note.md");
        std::fs::write(&md, "hello\n").unwrap();
        let shown = preview_link(&md.to_string_lossy()).unwrap();
        let expect = LinkFingerprint::from_preview(&shown);
        std::fs::write(&md, "hello\nchanged\n").unwrap();
        let before = snap(&approvals);
        let res = open_link_checked(&md.to_string_lossy(), &approvals, &expect).unwrap();
        assert_eq!(res.action, "preview");
        assert_eq!(res.preview.unwrap().kind, "document");
        assert!(OpenGuard::log().is_empty());
        assert_eq!(snap(&approvals), before);

        let shown = preview_link(&md.to_string_lossy()).unwrap();
        let expect = LinkFingerprint::from_preview(&shown);
        set_mode(&md, 0o755);
        let res = open_link_checked(&md.to_string_lossy(), &approvals, &expect).unwrap();
        assert_eq!(res.action, "preview");
        assert_eq!(res.preview.as_ref().unwrap().kind, "executable");
        assert!(OpenGuard::log().is_empty());
        assert_eq!(snap(&approvals), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
