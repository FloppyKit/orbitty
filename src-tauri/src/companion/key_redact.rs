//! Alpha: a key pasted into chat is not sent, and the on-disk log keeps
//! `[redacted key]` instead of the bytes.

use std::path::Path;

/// Trailing byte is outside `[A-Za-z0-9_-]`. Rodata packs these literals
/// against neighbouring words; without the break, `ghp_` plus the next
/// prefixes looks like a GitHub token to the DMG secret scan.
const PREFIX_BREAK: char = '\u{1}';

const KEY_PREFIX_STORED: &[&str] = &[
    "sk-or-v1-\u{1}",
    "sk-ant-\u{1}",
    "sk-proj-\u{1}",
    "ghp_\u{1}",
    "gho_\u{1}",
    "ghu_\u{1}",
    "ghs_\u{1}",
    "ghr_\u{1}",
    "xai-\u{1}",
    "AIza\u{1}",
    "sk-\u{1}",
];

fn stored_prefix(stored: &str) -> &str {
    stored.strip_suffix(PREFIX_BREAK).unwrap_or(stored)
}

pub const REDACTED: &str = "[redacted key]";

pub const KEY_REPLY: &str = "That looked like an API key. I removed it from your chat history. Please make a new key at your provider and delete this one: models can read old chats. To use a key, paste it in Set up a model.";

pub fn find_keys(text: &str, stored: &[String]) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut taken = vec![false; bytes.len()];
    let mut out = Vec::new();
    for stored in KEY_PREFIX_STORED {
        let prefix = stored_prefix(stored);
        let pb = prefix.as_bytes();
        let mut i = 0;
        while i + pb.len() <= bytes.len() {
            if !taken[i] && bytes[i..].starts_with(pb) {
                let mut n = 0;
                while i + pb.len() + n < bytes.len() {
                    let b = bytes[i + pb.len() + n];
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                        n += 1;
                    } else {
                        break;
                    }
                }
                if n >= 4 {
                    let end = i + pb.len() + n;
                    taken[i..end].fill(true);
                    out.push(text[i..end].to_string());
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }
    for secret in stored {
        if secret.len() < 8 {
            continue;
        }
        let mut start = 0;
        while let Some(rel) = text[start..].find(secret) {
            let abs = start + rel;
            if abs + secret.len() <= taken.len() && !taken[abs] {
                taken[abs..abs + secret.len()].fill(true);
                out.push(secret.clone());
            }
            start = abs + secret.len();
        }
    }
    out
}

pub fn redact_text(text: &str, keys: &[String]) -> String {
    let mut out = text.to_string();
    let mut keys = keys.to_vec();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    for k in keys {
        if k.is_empty() {
            continue;
        }
        out = out.replace(&k, REDACTED);
    }
    out
}

/// Rewrites `log` (earlier copies plus this message) and does not call the model.
pub fn guard_log_send(
    path: &Path,
    message: &str,
    stored: &[String],
    call_model: &mut dyn FnMut(),
) -> Option<String> {
    let keys = find_keys(message, stored);
    if keys.is_empty() {
        call_model();
        return None;
    }
    let prior = std::fs::read_to_string(path).unwrap_or_default();
    let prior = redact_text(&prior, &keys);
    let you = redact_text(message, &keys);
    let body = format!("{prior}{you}\n{KEY_REPLY}\n");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, body);
    Some(KEY_REPLY.to_string())
}

pub fn block_send(app: &tauri::AppHandle, line: &str) -> Option<String> {
    let stored = crate::companion::keystore::stored_secrets();
    let keys = find_keys(line, &stored);
    if keys.is_empty() {
        return None;
    }
    let you = redact_text(line, &keys);
    let reply = KEY_REPLY.to_string();
    let _ = crate::state::update(app, |st| {
        for turn in &mut st.companion.turns {
            turn.text = redact_text(&turn.text, &keys);
        }
        st.companion.last_user_line = redact_text(&st.companion.last_user_line, &keys);
        st.companion.last_bot_line = redact_text(&st.companion.last_bot_line, &keys);
        st.companion.push_turn("you", &you);
        st.companion.last_user_line = you.clone();
        st.companion.push_turn("them", &reply);
        st.companion.last_bot_line = reply.clone();
        Ok(())
    });
    Some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_prefixes_cover_the_list() {
        let prefixes: Vec<&str> = KEY_PREFIX_STORED.iter().copied().map(stored_prefix).collect();
        assert!(prefixes.contains(&"sk-or-v1-"));
        assert!(prefixes.contains(&"sk-ant-"));
        assert!(prefixes.contains(&"sk-proj-"));
        assert!(prefixes.contains(&"sk-"));
        assert!(prefixes.contains(&"xai-"));
        assert!(prefixes.contains(&"AIza"));
        assert!(prefixes.contains(&"ghp_"));
        assert!(prefixes.contains(&"gho_"));
        assert!(prefixes.contains(&"ghu_"));
        assert!(prefixes.contains(&"ghs_"));
        assert!(prefixes.contains(&"ghr_"));
        assert!(prefixes.iter().position(|p| *p == "sk-or-v1-") < prefixes.iter().position(|p| *p == "sk-"));
        let token = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
        for stored in KEY_PREFIX_STORED {
            let bytes = stored.as_bytes();
            assert_eq!(bytes.last().copied(), Some(1));
            assert!(!token(bytes[bytes.len() - 1]));
            assert_eq!(stored_prefix(stored).len() + 1, stored.len());
        }
    }

    #[test]
    fn chat_key_redacted_on_disk() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("orbitty-redact-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("chat.log");
        let key = "sk-or-v1-TESTfixturevalue";
        std::fs::write(&path, format!("earlier {key}\n")).unwrap();
        let mut calls = 0;
        let reply = guard_log_send(&path, &format!("paste {key} please"), &[], &mut || calls += 1);
        let disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(reply.as_deref(), Some(KEY_REPLY));
        assert_eq!(calls, 0);
        assert!(!disk.contains(key));
        assert!(!disk.contains("sk-or-v1-TEST"));
        assert!(disk.contains(REDACTED));
        assert!(disk.contains("earlier"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
