//! Local `todo:` intercept — mint a Launchpad card without pinging the
//! companion model. Flags: `-planet` (unique name or id), `--agent name`.
//! Unknown planet/agent = honest caption, no card.

use crate::agents;
use crate::board;

pub struct TodoLine {
    pub title: String,
    pub project: String,
    pub agent: Option<String>,
}

/// `Some` only when the line starts with `todo:` (case-insensitive).
pub fn parse_todo_line(line: &str) -> Option<Result<TodoLine, String>> {
    let t = line.trim();
    let rest = strip_prefix_ci(t, "todo:")?;
    Some(parse_flags(rest))
}

fn strip_prefix_ci<'a>(s: &'a str, pfx: &str) -> Option<&'a str> {
    if s.len() >= pfx.len() && s[..pfx.len()].eq_ignore_ascii_case(pfx) {
        Some(s[pfx.len()..].trim())
    } else {
        None
    }
}

fn parse_flags(rest: &str) -> Result<TodoLine, String> {
    let mut title: Vec<&str> = Vec::new();
    let mut project = String::new();
    let mut agent: Option<String> = None;
    let mut tokens = rest.split_whitespace().peekable();
    while let Some(tok) = tokens.next() {
        if let Some(val) = tok.strip_prefix("--agent=") {
            if val.is_empty() {
                return Err("todo: --agent needs a name".into());
            }
            agent = Some(val.to_string());
            continue;
        }
        if tok.eq_ignore_ascii_case("--agent") {
            let val = tokens
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "todo: --agent needs a name".to_string())?;
            agent = Some(val.to_string());
            continue;
        }
        if tok == "-" || tok == "--" {
            title.push(tok);
            continue;
        }
        if let Some(name) = tok.strip_prefix('-').filter(|s| !s.starts_with('-')) {
            if name.is_empty() {
                title.push(tok);
            } else {
                project = name.to_string();
            }
            continue;
        }
        title.push(tok);
    }
    let title = title.join(" ");
    if title.is_empty() {
        return Err("todo: need a title (todo: buy milk -fkt)".into());
    }
    Ok(TodoLine {
        title,
        project,
        agent,
    })
}

/// Mint the card. Honest errors stay strings for the caption — never a
/// send failure, never a model call.
pub fn intercept(app: &tauri::AppHandle, line: &str) -> Option<Result<String, String>> {
    let parsed = match parse_todo_line(line)? {
        Ok(p) => p,
        Err(e) => return Some(Err(e)),
    };
    Some(run(app, parsed))
}

fn run(app: &tauri::AppHandle, parsed: TodoLine) -> Result<String, String> {
    let card = board::kanban_add_as(app, &parsed.title, &parsed.project, "todo", "user", None)?;
    if let Some(agent) = parsed.agent.as_deref() {
        agents::kanban_assign(app, &card.id, agent).map_err(|e| format!("todo: {e}"))?;
    }
    let planet = if card.project_id.is_empty() {
        "Inbox".to_string()
    } else {
        crate::state::load_state(app)
            .ok()
            .and_then(|st| {
                st.projects
                    .iter()
                    .find(|p| p.id == card.project_id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| card.project_id.clone())
    };
    let mut msg = format!("Todo: {} → {}", card.title, planet);
    if let Some(agent) = parsed.agent {
        msg.push_str(" · ");
        msg.push_str(&agent);
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_only() {
        assert!(parse_todo_line("hello").is_none());
        assert!(parse_todo_line("todo").is_none());
        assert!(matches!(parse_todo_line("todo:"), Some(Err(_))));
    }

    #[test]
    fn flags() {
        let p = parse_todo_line("todo: buy milk -fkt --agent spark")
            .unwrap()
            .unwrap();
        assert_eq!(p.title, "buy milk");
        assert_eq!(p.project, "fkt");
        assert_eq!(p.agent.as_deref(), Some("spark"));
    }

    #[test]
    fn agent_eq() {
        let p = parse_todo_line("TODO:  -orbitty  restock --agent=grok")
            .unwrap()
            .unwrap();
        assert_eq!(p.title, "restock");
        assert_eq!(p.project, "orbitty");
        assert_eq!(p.agent.as_deref(), Some("grok"));
    }
}
