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

pub fn project_from_line(line: &str) -> String {
    let l = line.to_ascii_lowercase();
    if let Some(i) = l.find("planet ") {
        let rest = line[i + 7..].trim();
        let word: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if word.len() >= 2 {
            return word;
        }
    }
    "orbitty".into()
}

pub fn run(app: &tauri::AppHandle, name: &str, args: &Value) -> String {
    match name {
        "kanban_list" => kanban_list(app, args),
        other => format!("unknown tool {other}"),
    }
}

fn is_needed(id: &str, kind: Option<&str>) -> bool {
    kind.is_some_and(|k| k.eq_ignore_ascii_case("needed")) || id.starts_with("n-")
}

fn needed_still_open(needed_state: Option<&str>) -> bool {
    match needed_state.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("answered") | Some("skipped") => false,
        _ => true,
    }
}

fn status_ok(filter: &str, stt: &str) -> bool {
    match filter {
        "open" => stt == "todo" || stt == "doing",
        s => stt.eq_ignore_ascii_case(s),
    }
}

fn clip_title(title: &str) -> Option<String> {
    let t = title.trim().replace('\n', " ");
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

const BUCKET_CAP: usize = 24;

fn push_cap(dst: &mut Vec<String>, title: String) {
    if dst.len() < BUCKET_CAP {
        dst.push(title);
    }
}

fn format_blocks(
    pid: &str,
    filter: &str,
    todo: &[String],
    todo_n: usize,
    doing: &[String],
    doing_n: usize,
    needed: &[String],
    needed_n: usize,
) -> String {
    let count = todo_n + doing_n + needed_n;
    if count == 0 {
        return format!("count=0\norbitty planet {pid}: no {filter} cards");
    }
    fn block(name: &str, n: usize, titles: &[String]) -> String {
        let mut s = format!("{name} ({n}):");
        for t in titles {
            s.push('\n');
            s.push_str(t);
        }
        s
    }
    format!(
        "count={count}\norbitty planet {pid}:\n{}\n{}\n{}",
        block("TODO", todo_n, todo),
        block("DOING", doing_n, doing),
        block("NEEDED", needed_n, needed)
    )
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
    let mut todo: Vec<String> = Vec::new();
    let mut doing: Vec<String> = Vec::new();
    let mut needed: Vec<String> = Vec::new();
    let mut todo_n = 0usize;
    let mut doing_n = 0usize;
    let mut needed_n = 0usize;
    for c in &st.kanban_cards {
        if c.project_id != pid {
            continue;
        }
        let stt = c.status.trim();
        if !status_ok(&status, stt) {
            continue;
        }
        let Some(title) = clip_title(&c.title) else {
            continue;
        };
        if is_needed(&c.id, c.kind.as_deref()) {
            if !needed_still_open(c.needed_state.as_deref()) {
                continue;
            }
            needed_n += 1;
            push_cap(&mut needed, title);
        } else if stt.eq_ignore_ascii_case("doing") {
            doing_n += 1;
            push_cap(&mut doing, title);
        } else {
            todo_n += 1;
            push_cap(&mut todo, title);
        }
    }
    format_blocks(
        &pid, &status, &todo, todo_n, &doing, doing_n, &needed, needed_n,
    )
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

    #[test]
    fn planet_from_line() {
        assert_eq!(
            project_from_line("tell me about the todo list on planet orbitty"),
            "orbitty"
        );
        assert_eq!(project_from_line("ping"), "orbitty");
    }

    #[test]
    fn needed_is_kind_or_n_prefix() {
        assert!(is_needed("n-abc", None));
        assert!(is_needed("c-abc", Some("needed")));
        assert!(is_needed("c-abc", Some("NEEDED")));
        assert!(!is_needed("c-abc", Some("todo")));
        assert!(!is_needed("c-abc", None));
    }

    #[test]
    fn snapshot_splits_needed_out_of_todo() {
        let out = format_blocks(
            "p-orbitty",
            "open",
            &["GO-ORBITTY-SLEEP-PRESETS".into()],
            1,
            &["Later: harness pick".into()],
            1,
            &["Who runs this doing GO?".into()],
            1,
        );
        assert!(out.starts_with("count=3\n"));
        assert!(out.contains("TODO (1):\nGO-ORBITTY-SLEEP-PRESETS"));
        assert!(out.contains("DOING (1):\nLater: harness pick"));
        assert!(out.contains("NEEDED (1):\nWho runs this doing GO?"));
        assert!(!out.contains("todo\t"));
    }

    #[test]
    fn answered_needed_is_not_open() {
        assert!(!needed_still_open(Some("answered")));
        assert!(!needed_still_open(Some("skipped")));
        assert!(needed_still_open(Some("open")));
        assert!(needed_still_open(Some("later")));
        assert!(needed_still_open(None));
    }
}
