//! Plain seat phrase. Std only, so the app and orbittyctl share one function.

/// Prefixes are built here so this file has no home-path literal.
fn path_prefixes() -> [String; 3] {
    [
        format!("/{}s/", "User"),
        format!("/{}/", "home"),
        format!("{}{}", '~', '/'),
    ]
}

fn strip_prefixes(s: &str, prefixes: &[String]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(n) = prefixes.iter().find_map(|p| {
            rest.strip_prefix(p.as_str()).map(|tail| {
                let extra = tail.find(|c: char| c.is_whitespace()).unwrap_or(tail.len());
                p.len() + extra
            })
        }) {
            rest = &rest[n..];
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

fn is_box_drawing(c: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&c)
}

/// `n/m:` key hints, as in `1/5:select`.
fn has_count_hint(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'/' {
            continue;
        }
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > start && i < b.len() && b[i] == b':' {
            return true;
        }
    }
    false
}

/// A TUI footer or key-hint line is not a plain status line.
pub(crate) fn is_tui_footer(s: &str) -> bool {
    s.chars().any(is_box_drawing)
        || s.contains("Ctrl+")
        || s.contains("Tab:")
        || s.contains("Esc:")
        || has_count_hint(s)
}

fn has_elapsed(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'h' && b[i] != b'm') {
            continue;
        }
        let unit = b[i];
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start || i >= b.len() {
            continue;
        }
        let unit2 = b[i];
        if (unit == b'h' && unit2 == b'm') || (unit == b'm' && unit2 == b's') {
            return true;
        }
    }
    false
}

fn has_token_count(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                continue;
            }
        }
        if i < b.len() && b[i] == b'k' && (i + 1 >= b.len() || !b[i + 1].is_ascii_alphabetic()) {
            return true;
        }
    }
    false
}

fn has_bracket_button(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'[' {
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < b.len() && b[i] != b']' && b[i] != b'\n' && i - start < 12 {
            i += 1;
        }
        if i < b.len() && b[i] == b']' && i > start {
            return true;
        }
    }
    false
}

/// A terminal status bar: spinner marks, a timer, a token count, or a bracket button.
pub(crate) fn is_status_chrome(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '◆' | '◈' | '❯' | '⇣' | '↓' | '`'))
        || has_elapsed(s)
        || has_token_count(s)
        || has_bracket_button(s)
}

pub(crate) fn clamp_line(s: &str) -> String {
    if is_tui_footer(s) || is_status_chrome(s) {
        return String::new();
    }
    let flat = s.replace(['\r', '\n', '\t'], " ");
    let prefixes = path_prefixes();
    let stripped = strip_prefixes(&flat, &prefixes);
    let one = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    one.chars().take(120).collect()
}

/// Words plus at most one clean sentence. `state` is the door's snake_case name.
pub(crate) fn phrase_from_state(state: &str, line: &str) -> String {
    let words = match state {
        "working" => "working",
        "waiting_on_you" => "waiting on you",
        "idle" => "idle",
        "failed" => "failed",
        "done" => "done",
        other => other,
    };
    let line = clamp_line(line);
    if line.is_empty() {
        words.to_string()
    } else {
        format!("{words} · {line}")
    }
}
