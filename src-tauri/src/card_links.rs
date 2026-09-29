//! Card links — file:// and https:// only, at write and at open.
//! A bare absolute `/Users/…` path is stored as a percent-encoded file URL.
//! macOS `open` is the only launcher. No shell.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::state::{CardLink, PersistedState};

pub const MAX_LINKS: usize = 8;

const MAX_URL: usize = 2048;
const MAX_LABEL: usize = 80;

/// Normalize one link for storage. Idempotent on its own output.
pub fn normalize_link(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("link must be file:// or https://".into());
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
                return Err(format!("link rejected: {other}: is not allowed"));
            }
        }
    }
    if s.starts_with('/') {
        return finish_file_path(&percent_decode(s)?);
    }
    Err("link must be file:// or https://".into())
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

/// Open a link with macOS `open`. Missing files return "file not found"
/// and do not spawn.
pub fn open_url(raw: &str) -> Result<(), String> {
    let url = normalize_link(raw)?;
    if let Some(path) = file_url_to_path(&url) {
        if !path.exists() {
            return Err("file not found".into());
        }
        spawn_open(&path.to_string_lossy())
    } else {
        spawn_open(&url)
    }
}

#[tauri::command]
pub fn open_card_link(
    app: tauri::AppHandle,
    card_id: String,
    index: Option<u32>,
) -> Result<(), String> {
    let st = crate::state::load_state(&app)?;
    let id = card_id.trim();
    let card = st
        .kanban_cards
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("unknown card id {id:?}"))?;
    let i = index.unwrap_or(0) as usize;
    let links = card.links.as_deref().unwrap_or(&[]);
    let link = links
        .get(i)
        .ok_or_else(|| format!("no link at {i}"))?;
    open_url(&link.url)
}

#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    open_url(&url)
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
        return Err("file link must be an absolute path".into());
    } else {
        let (host, path) = rest
            .split_once('/')
            .ok_or_else(|| "file link must be an absolute path".to_string())?;
        if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            return Err("file link must be an absolute path".into());
        }
        percent_decode(&format!("/{path}"))?
    };
    finish_file_path(&path)
}

fn finish_file_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("file link must be an absolute path".into());
    }
    if path.chars().any(|c| c.is_control()) {
        return Err("link rejected".into());
    }
    for seg in path.split('/') {
        if seg == ".." {
            return Err("file link must be an absolute path".into());
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
    let i = raw.find(':').ok_or_else(|| "link must be file:// or https://".to_string())?;
    if !raw[..i].eq_ignore_ascii_case(scheme) {
        return Err("link must be file:// or https://".into());
    }
    let rest = &raw[i + 1..];
    let rest = rest
        .strip_prefix("//")
        .ok_or_else(|| "link must be file:// or https://".to_string())?;
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

fn spawn_open(arg: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(arg)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("open: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arg;
        Err("open is wired for macOS only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                err.contains("not allowed") || err.contains("link must be"),
                "{raw} -> {err}"
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
}
