//! Alpha: a key pasted into chat is not sent, and the on-disk log keeps
//! `[redacted key]` instead of the bytes.

use std::path::Path;

pub const KEY_PREFIXES: &[&str] = &[
    "sk-or-v1-",
    "sk-ant-",
    "sk-proj-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "xai-",
    "AIza",
    "sk-",
];

pub const REDACTED: &str = "[redacted key]";

pub const KEY_REPLY: &str = "That looked like an API key. I removed it from your chat history. Please make a new key at your provider and delete this one: models can read old chats. To use a key, paste it in Set up a model.";

pub fn find_keys(text: &str, stored: &[String]) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut taken = vec![false; bytes.len()];
    let mut out = Vec::new();
    for prefix in KEY_PREFIXES {
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
        assert!(KEY_PREFIXES.contains(&"sk-or-v1-"));
        assert!(KEY_PREFIXES.contains(&"sk-ant-"));
        assert!(KEY_PREFIXES.contains(&"sk-proj-"));
        assert!(KEY_PREFIXES.contains(&"sk-"));
        assert!(KEY_PREFIXES.contains(&"xai-"));
        assert!(KEY_PREFIXES.contains(&"AIza"));
        assert!(KEY_PREFIXES.contains(&"ghp_"));
        assert!(KEY_PREFIXES.contains(&"gho_"));
        assert!(KEY_PREFIXES.contains(&"ghu_"));
        assert!(KEY_PREFIXES.contains(&"ghs_"));
        assert!(KEY_PREFIXES.contains(&"ghr_"));
        assert!(KEY_PREFIXES.iter().position(|p| *p == "sk-or-v1-") < KEY_PREFIXES.iter().position(|p| *p == "sk-"));
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
