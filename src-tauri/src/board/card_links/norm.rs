//! Card links — file:// and https:// only, at write and at open.
//! A bare absolute `/Users/…` path is stored as a percent-encoded file URL.
//! The first click previews. Open / Run is a second click, remembered per
//! exact resolved path or full URL in `link-approvals.json` next to state
//! (local, human-readable, not part of synced state). macOS `open` is the
//! only launcher. No shell.

use std::path::PathBuf;

use crate::state::{CardLink, PersistedState};
use super::preview::LinkApproval;
use super::preview::LinkClick;
use super::preview::LinkFingerprint;
use super::preview::approvals_path;
use super::preview::clear_approvals;
use super::preview::click_link_at;
use super::preview::launch;
use super::preview::load_approvals;
use super::preview::open_link_checked;
use super::preview::preview_link;
use super::preview::remove_approval;
use super::preview::reveal;

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

pub(crate) fn file_url_to_path(url: &str) -> Option<PathBuf> {
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

pub(crate) fn encode_path(path: &str) -> String {
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

pub(crate) const EXCERPT_BYTES: usize = 64 * 1024;
pub(crate) const EXCERPT_LINES: usize = 20;
pub(crate) const THUMB_MAX: u64 = 256 * 1024;
/// Content hash cap for executables, scripts, and bundle main binaries.
/// Larger than this, the next click always previews.
pub(crate) const HASH_CAP: u64 = 64 * 1024 * 1024;

pub(crate) const EXEC_EXT: &[&str] = &[
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
pub(crate) const IMAGE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "tif", "tiff", "bmp", "ico",
];
pub(crate) const TEXT_EXT: &[&str] = &["md", "markdown", "txt", "text"];

pub(crate) const APPROVAL_NOTE: &str = "Links opened from Orbitty on this Mac. The same path or URL opens directly next time. A different path, a different URL, or a program whose contents changed asks again. Remove a row to ask again. This file is local and is not synced.";

