//! MarkItDown sidecar for office chips. Detect-only — never prompt-install.
//! Fail convert → path-only, honest note. Already-text and images skip.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::attach::AttachInfo;

const OFFICE: &[&str] = &["pdf", "docx", "pptx", "xlsx", "xls", "html", "htm"];
const TEXT_SKIP: &[&str] = &["md", "txt", "rs", "json", "csv", "toml", "py", "ts", "js", "svelte", "css"];
const MD_CAP: usize = 400_000;

pub fn is_office(path: &str) -> bool {
    ext(path).is_some_and(|e| OFFICE.contains(&e.as_str()))
}

pub fn is_already_text(path: &str) -> bool {
    ext(path).is_some_and(|e| TEXT_SKIP.contains(&e.as_str()))
}

fn ext(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn markitdown_bin() -> Option<std::path::PathBuf> {
    crate::coordinator::find_on_path("markitdown")
}

/// Convert an office chip. `None` = skip (not office, or binary missing).
/// `Err` = ran and failed — caller still sends the original path.
pub fn convert(info: &AttachInfo) -> Result<Option<String>, String> {
    if !is_office(&info.path) || is_already_text(&info.path) {
        return Ok(None);
    }
    let Some(bin) = markitdown_bin() else {
        return Ok(None);
    };
    let out = Command::new(&bin)
        .arg(&info.path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("markitdown: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(if err.is_empty() {
            "markitdown failed".into()
        } else {
            format!("markitdown: {}", err.chars().take(160).collect::<String>())
        });
    }
    let mut md = String::from_utf8_lossy(&out.stdout).into_owned();
    if md.trim().is_empty() {
        return Err("markitdown returned empty".into());
    }
    if md.len() > MD_CAP {
        md.truncate(MD_CAP);
        md.push_str("\n\n[truncated]");
    }
    Ok(Some(md))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_vs_text() {
        assert!(is_office("/tmp/a.pdf"));
        assert!(is_office("/tmp/a.DOCX"));
        assert!(!is_office("/tmp/a.rs"));
        assert!(is_already_text("/tmp/a.rs"));
        assert!(is_already_text("/tmp/note.md"));
        assert!(!is_office("/tmp/pic.png"));
    }
}
