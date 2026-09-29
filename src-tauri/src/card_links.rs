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
/// through [`click_link_at`], which previews first.
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

/// Open / Run from the preview. Records the exact resolved path or URL.
#[tauri::command]
pub fn open_link(app: tauri::AppHandle, url: String) -> Result<(), String> {
    open_link_at(&url, &approvals_path(&app)?)
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

const EXEC_EXT: &[&str] = &[
    "app", "command", "sh", "zsh", "bash", "tool", "workflow", "terminal", "pkg", "dmg", "scpt",
    "applescript",
];
const IMAGE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "tif", "tiff", "bmp", "ico",
];
const TEXT_EXT: &[&str] = &["md", "markdown", "txt", "text"];

const APPROVAL_NOTE: &str = "Links opened from Orbitty on this Mac. The same path or URL opens directly next time. A different path, a different URL, or an executable whose size or modified time changed asks again. Remove a row to ask again. This file is local and is not synced.";

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
        })
    }
}

pub fn click_link_at(raw: &str, approvals: &Path) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    if preview.kind != "missing" {
        let file = load_approvals(approvals)?;
        if approval_matches(&preview, &file) {
            launch(&preview)?;
            remember(&preview, approvals)?;
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

pub fn open_link_at(raw: &str, approvals: &Path) -> Result<(), String> {
    let preview = preview_link(raw)?;
    open_url(raw)?;
    remember(&preview, approvals)?;
    Ok(())
}

/// orbittyctl `kanban open-link`. Documents, folders, images, and https
/// open. An executable is revealed unless `run` or an exact approval
/// (size + mtime still match). Returns "open" or "reveal".
pub fn ctl_open_at(raw: &str, run: bool, approvals: &Path) -> Result<String, String> {
    let preview = preview_link(raw)?;
    if preview.kind == "missing" {
        return Err("file not found".into());
    }
    let file = load_approvals(approvals)?;
    let do_open = preview.kind != "executable" || run || approval_matches(&preview, &file);
    if do_open {
        launch(&preview)?;
        remember(&preview, approvals)?;
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
            && row.modified_secs == preview.modified_secs;
    }
    true
}

pub fn load_approvals(path: &Path) -> Result<LinkApprovalFile, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(LinkApprovalFile::default()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display())),
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
    let (modified_secs, modified) = match meta.modified().ok() {
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
    };
    let size = Some(meta.len());
    if resolved.is_dir() {
        if is_bundle_dir(&resolved) {
            return file_preview("executable", &resolved, size, modified, modified_secs, None, None);
        }
        return file_preview("folder", &resolved, size, modified, modified_secs, None, None);
    }
    if ext_in(&resolved, EXEC_EXT) || has_exec_bit(&meta) {
        return file_preview(
            "executable",
            &resolved,
            size,
            modified,
            modified_secs,
            Some(RUN_WARNING.to_string()),
            None,
        );
    }
    if ext_in(&resolved, IMAGE_EXT) {
        let thumb = thumbnail_data_url(&resolved, meta.len());
        return file_preview("image", &resolved, size, modified, modified_secs, None, thumb);
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
    let mime = match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
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
    if preview.kind == "missing" || (preview.kind != "https" && !Path::new(&preview.target).exists()) {
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
            assert!(err.contains(ACCEPTED), "{raw} error should say what is accepted: {err}");
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
        let want = format!("file://{}", encode_path(&format!("{home}/Orbitty GOs/x.md")));
        assert_eq!(normalize_link("~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("  ~/Orbitty GOs/x.md ").unwrap(), want);
        assert_eq!(normalize_link("file://~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("~").unwrap(), format!("file://{}", encode_path(&home)));
        // ~user is not expanded, and the message says what works.
        let err = normalize_link("~root/x").unwrap_err();
        assert!(err.contains(ACCEPTED), "{err}");
        assert!(normalize_link("~/../../etc/passwd").is_err());
    }

    #[test]
    fn file_url_forms_normalize() {
        let want = "file:///Users/dude/Orbitty%20GOs/x.md";
        assert_eq!(normalize_link("file:///Users/dude/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("file:///Users/dude/Orbitty%20GOs/x.md").unwrap(), want);
        // Two-slash typo: the root folder landed in the host slot.
        assert_eq!(normalize_link("file://Users/dude/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("FILE://Users/dude/Orbitty%20GOs/x.md").unwrap(), want);
        assert_eq!(
            normalize_link("file://Volumes/Disk/a b.pdf").unwrap(),
            "file:///Volumes/Disk/a%20b.pdf"
        );
        assert_eq!(normalize_link("file://localhost/tmp/a.md").unwrap(), "file:///tmp/a.md");
        assert_eq!(normalize_link("/Users/dude/x.md").unwrap(), "file:///Users/dude/x.md");
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
        let p = std::env::temp_dir().join(format!("orbitty-link-safe-{}-{}-{}", std::process::id(), name, n));
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

        let cases = [
            (md.to_string_lossy().to_string(), "document"),
            (folder.to_string_lossy().to_string(), "folder"),
            (png.to_string_lossy().to_string(), "image"),
            (cmd.to_string_lossy().to_string(), "executable"),
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
        for (raw, kind) in cases {
            let p = preview_link(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(p.kind, kind, "{raw}");
            assert!(OpenGuard::log().is_empty(), "preview spawned open for {raw}");
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
        assert_eq!(click_link_at(&md.to_string_lossy(), &approvals).unwrap().action, "opened");
        assert_eq!(OpenGuard::log().len(), 1);

        OpenGuard::clear();
        assert_eq!(click_link_at(&other.to_string_lossy(), &approvals).unwrap().action, "preview");
        assert!(OpenGuard::log().is_empty());

        let https = "https://example.com/a?b=1";
        open_link_at(https, &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(https, &approvals).unwrap().action, "opened");
        OpenGuard::clear();
        assert_eq!(
            click_link_at("https://example.com/a?b=2", &approvals).unwrap().action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        #[cfg(unix)]
        {
            let same = dir.join("same.md");
            std::os::unix::fs::symlink(&md, &same).unwrap();
            OpenGuard::clear();
            assert_eq!(click_link_at(&same.to_string_lossy(), &approvals).unwrap().action, "opened");
            let elsewhere = dir.join("else.md");
            std::os::unix::fs::symlink(&other, &elsewhere).unwrap();
            OpenGuard::clear();
            assert_eq!(
                click_link_at(&elsewhere.to_string_lossy(), &approvals).unwrap().action,
                "preview"
            );
            assert!(OpenGuard::log().is_empty());
        }

        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, format!("#!/bin/sh\necho ran > {}\n", sentinel.display())).unwrap();
        set_mode(&cmd, 0o755);
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(&cmd.to_string_lossy(), &approvals).unwrap().action, "opened");
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
        assert_eq!(click_link_at(&cmd.to_string_lossy(), &approvals).unwrap().action, "preview");
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        let canon = md.canonicalize().unwrap().to_string_lossy().to_string();
        remove_approval(&approvals, &canon).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(&md.to_string_lossy(), &approvals).unwrap().action, "preview");
        assert!(OpenGuard::log().is_empty());
        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        clear_approvals(&approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(&md.to_string_lossy(), &approvals).unwrap().action, "preview");
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
        std::fs::write(&cmd, format!("#!/bin/sh\necho ran > {}\n", sentinel.display())).unwrap();
        set_mode(&cmd, 0o755);
        let md = dir.join("note.md");
        std::fs::write(&md, "doc\n").unwrap();

        assert_eq!(ctl_open_at(&md.to_string_lossy(), false, &approvals).unwrap(), "open");
        assert_eq!(
            OpenGuard::log()[0],
            vec![md.canonicalize().unwrap().to_string_lossy().to_string()]
        );

        OpenGuard::clear();
        assert_eq!(ctl_open_at(&cmd.to_string_lossy(), false, &approvals).unwrap(), "reveal");
        assert_eq!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());

        let fresh = dir.join("fresh.json");
        OpenGuard::clear();
        assert_eq!(ctl_open_at(&cmd.to_string_lossy(), true, &fresh).unwrap(), "open");
        assert_ne!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());
        OpenGuard::clear();
        assert_eq!(ctl_open_at(&cmd.to_string_lossy(), false, &fresh).unwrap(), "open");
        assert_ne!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
