//! Tiny OpenAI-tool allowlist for the local hop (E4B). Reads only.
//! Writes stay Grok. Not bash. Not a second soul.

use serde_json::{json, Value};

pub fn schemas() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "kanban_list",
                "description": "List cards on an Orbitty planet. Default project is orbitty. status: todo, doing, done, or open (todo+doing).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "status": { "type": "string" }
                    }
                }
            }
        }
    ])
}

pub fn run(app: &tauri::AppHandle, name: &str, args: &Value) -> String {
    match name {
        "kanban_list" => kanban_list(app, args),
        other => format!("unknown tool {other}"),
    }
}

fn kanban_list(app: &tauri::AppHandle, args: &Value) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return "board unreadable".into();
    };
    let want = args
        .get("project")
        .and_then(Value::as_str)
        .unwrap_or("orbitty")
        .trim()
        .to_ascii_lowercase();
    let pid = st
        .projects
        .iter()
        .find(|p| {
            p.id.eq_ignore_ascii_case(&want) || p.name.trim().eq_ignore_ascii_case(&want)
        })
        .map(|p| p.id.clone());
    let Some(pid) = pid else {
        let names: Vec<_> = st
            .projects
            .iter()
            .map(|p| p.name.trim())
            .filter(|n| !n.is_empty())
            .take(12)
            .collect();
        return format!("no planet {want:?}. have: {}", names.join(", "));
    };
    let status = args
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("open")
        .trim()
        .to_ascii_lowercase();
    let mut lines: Vec<String> = Vec::new();
    for c in &st.kanban_cards {
        if c.project_id != pid {
            continue;
        }
        let stt = c.status.trim();
        let ok = match status.as_str() {
            "open" => stt == "todo" || stt == "doing",
            s => stt.eq_ignore_ascii_case(s),
        };
        if !ok {
            continue;
        }
        let title = c.title.trim().replace('\n', " ");
        if title.is_empty() {
            continue;
        }
        lines.push(format!("{stt}\t{title}"));
        if lines.len() >= 24 {
            break;
        }
    }
    if lines.is_empty() {
        format!("orbitty planet {pid}: no {status} cards")
    } else {
        format!("orbitty planet {pid} ({status}):\n{}", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_kanban_list_only() {
        let s = schemas();
        let arr = s.as_array().expect("arr");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["function"]["name"], "kanban_list");
    }
}
