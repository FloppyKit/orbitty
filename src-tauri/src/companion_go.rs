//! Mouth GO intercept — mint Writer + seat without pinging the model.
//! `GO-ORBITTY-FOO cursor` / `run GO-… with cursor on planet orbitty`.
//! Chat that merely mentions a GO does not match.

use crate::board;

pub struct GoLine {
    pub go: String,
    pub seat: Option<String>,
    pub project: String,
}

/// `Some` when the line is a GO run, not a question.
pub fn parse_go_line(line: &str) -> Option<GoLine> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("dogfood ") || lower.starts_with("what") || lower.starts_with("why") {
        return None;
    }
    let tokens: Vec<&str> = t.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let go = extract_go_token(tokens[0])
        .or_else(|| {
            let head = tokens[0].to_ascii_lowercase();
            if matches!(head.as_str(), "run" | "start" | "go" | "launch") && tokens.len() > 1 {
                extract_go_token(tokens[1])
            } else {
                None
            }
        })?;
    let mut seat = None;
    let mut project = String::new();
    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i];
        let tl = tok.to_ascii_lowercase();
        if matches!(tl.as_str(), "cursor" | "grok" | "wait") {
            seat = Some(tl.clone());
        }
        if tl == "planet" && i + 1 < tokens.len() {
            let n = tokens[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            if !n.is_empty() && extract_go_token(n).is_none() {
                project = n.to_string();
            }
        }
        if let Some(name) = tok.strip_prefix('-').filter(|s| !s.starts_with('-') && s.len() > 1) {
            if extract_go_token(name).is_none() {
                project = name.to_string();
            }
        }
        i += 1;
    }
    Some(GoLine { go, seat, project })
}

fn extract_go_token(tok: &str) -> Option<String> {
    let t = tok.trim_matches(|c: char| matches!(c, '.' | '?' | '!' | ',' | ':' | ';'));
    let up = t.to_ascii_uppercase();
    if up.starts_with("GO-") && up.len() >= 6 && up.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        Some(up)
    } else {
        None
    }
}

pub fn intercept(app: &tauri::AppHandle, line: &str) -> Option<Result<String, String>> {
    let parsed = parse_go_line(line)?;
    Some(run(app, parsed))
}

fn run(app: &tauri::AppHandle, parsed: GoLine) -> Result<String, String> {
    let st = crate::state::load_state(app)?;
    let project = if parsed.project.is_empty() {
        String::new()
    } else {
        board::resolve_project(&st, &parsed.project, "go")?
    };
    let title = format!("Writer for {}?", parsed.go);
    let why = format!(
        "Mouth. Answer cursor / grok / wait.{}",
        parsed
            .seat
            .as_deref()
            .map(|s| format!(" Seat in line: {s}."))
            .unwrap_or_default()
    );
    let card = board::needed_open(app, &title, &project, Some(why.as_str()))?;
    if let Some(seat) = parsed.seat.as_deref() {
        let _ = board::needed_answer(app, &card.id, seat)?;
        return Ok(format!(
            "{seat} has {}. Writer {} — bus launches; Spark does not chew.",
            parsed.go, card.id
        ));
    }
    Ok(format!(
        "Writer for {}? is {}. Answer cursor / grok / wait.",
        parsed.go, card.id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_mentions_skip() {
        assert!(parse_go_line("what about GO-ORBITTY-RAIL-CLOSE").is_none());
        assert!(parse_go_line("Dogfood GO-ORBITTY-BOOMBOX?").is_none());
    }

    #[test]
    fn run_cursor_planet() {
        let p = parse_go_line("GO-ORBITTY-MOUTH-LIVE run with cursor on planet orbitty").unwrap();
        assert_eq!(p.go, "GO-ORBITTY-MOUTH-LIVE");
        assert_eq!(p.seat.as_deref(), Some("cursor"));
        assert_eq!(p.project, "orbitty");
    }

    #[test]
    fn run_prefix() {
        let p = parse_go_line("run GO-ORBITTY-FOO with grok").unwrap();
        assert_eq!(p.go, "GO-ORBITTY-FOO");
        assert_eq!(p.seat.as_deref(), Some("grok"));
    }
}
