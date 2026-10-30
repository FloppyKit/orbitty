//! Peek redaction. Runs before any terminal text leaves Rust.
//! Marker is `[redacted]`. Reuses companion key prefixes where they exist.

use crate::companion::key_redact::find_keys;

pub const MARK: &str = "[redacted]";

pub fn redact(input: &str) -> String {
    let mut out = mask_pem(input);
    let keys = find_keys(&out, &[]);
    let mut keys = keys;
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    for k in keys {
        if k.is_empty() {
            continue;
        }
        out = out.replace(&k, MARK);
    }
    out = mask_token(&out, "github_pat_");
    out = mask_slack(&out);
    out = mask_akia(&out);
    out = mask_bearer(&out);
    out = mask_assign(&out);
    mask_entropy(&out)
}

fn mask_pem(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let begin = b"-----BEGIN ";
    let end_mark = b"-----END ";
    let key = b"PRIVATE KEY-----";
    while i < bytes.len() {
        if bytes[i..].starts_with(begin) {
            if let Some(rel) = find_sub(bytes, i + begin.len(), key) {
                let after_begin = rel + key.len();
                if let Some(end_at) = find_sub(bytes, after_begin, end_mark) {
                    if let Some(end_key) = find_sub(bytes, end_at + end_mark.len(), key) {
                        let end = end_key + key.len();
                        out.push_str(MARK);
                        i = end;
                        continue;
                    }
                }
            }
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn find_sub(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

fn mask_token(input: &str, prefix: &str) -> String {
    let bytes = input.as_bytes();
    let pb = prefix.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(pb) {
            let mut n = 0;
            while i + pb.len() + n < bytes.len() {
                let b = bytes[i + pb.len() + n];
                if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                    n += 1;
                } else {
                    break;
                }
            }
            if n >= 8 {
                out.push_str(MARK);
                i += pb.len() + n;
                continue;
            }
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn mask_slack(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"xox")
            && i + 4 < bytes.len()
            && matches!(bytes[i + 3], b'a' | b'b' | b'p')
            && bytes.get(i + 4) == Some(&b'-')
        {
            let mut n = 5;
            while i + n < bytes.len() {
                let b = bytes[i + n];
                if b.is_ascii_alphanumeric() || b == b'-' {
                    n += 1;
                } else {
                    break;
                }
            }
            if n >= 16 {
                out.push_str(MARK);
                i += n;
                continue;
            }
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn mask_akia(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"AKIA") && i + 20 <= bytes.len() {
            let rest = &bytes[i + 4..i + 20];
            if rest.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
                out.push_str(MARK);
                i += 20;
                continue;
            }
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn mask_bearer(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if lb[i..].starts_with(b"bearer ") {
            let mut n = 7;
            let mut tok = 0;
            while i + n < bytes.len() {
                let b = bytes[i + n];
                if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b'/' | b'=')
                {
                    tok += 1;
                    n += 1;
                } else {
                    break;
                }
            }
            if tok >= 8 {
                out.push_str(MARK);
                i += n;
                continue;
            }
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn mask_assign(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(n) = assign_at(bytes, i) {
            out.push_str(MARK);
            i += n;
            continue;
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn assign_at(bytes: &[u8], i: usize) -> Option<usize> {
    let rest = &bytes[i..];
    let lower: Vec<u8> = rest.iter().take(24).map(|b| b.to_ascii_lowercase()).collect();
    let key_len = if lower.starts_with(b"api_key") || lower.starts_with(b"api-key") {
        7
    } else if lower.starts_with(b"apikey") {
        6
    } else if lower.starts_with(b"password") {
        8
    } else if lower.starts_with(b"secret") {
        6
    } else if lower.starts_with(b"token") {
        5
    } else {
        return None;
    };
    if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
        return None;
    }
    let mut j = key_len;
    while j < rest.len() && rest[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= rest.len() || (rest[j] != b'=' && rest[j] != b':') {
        return None;
    }
    j += 1;
    while j < rest.len() && rest[j].is_ascii_whitespace() {
        j += 1;
    }
    let start_val = j;
    while j < rest.len() && !rest[j].is_ascii_whitespace() {
        j += 1;
    }
    if j == start_val {
        return None;
    }
    Some(j)
}

fn mask_entropy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_b64(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_b64(bytes[i]) {
                i += 1;
            }
            let run = &bytes[start..i];
            if run.len() >= 32 && distinct(run) >= 8 {
                out.push_str(MARK);
            } else {
                out.push_str(&input[start..i]);
            }
            continue;
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn is_b64(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'='
}

fn distinct(run: &[u8]) -> usize {
    let mut seen = [false; 256];
    let mut n = 0;
    for &b in run {
        if !seen[b as usize] {
            seen[b as usize] = true;
            n += 1;
        }
    }
    n
}
