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
    explicit_planet(line).unwrap_or_else(|| "orbitty".into())
}

fn explicit_planet(line: &str) -> Option<String> {
    let l = line.to_ascii_lowercase();
    let i = l.find("planet ")?;
    let rest = line[i + 7..].trim();
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if word.len() >= 2 {
        Some(word)
    } else {
        None
    }
}

/// Planet token for a board read. An explicit `planet <name>` wins.
/// Otherwise a real planet name mentioned in the line (longest match),
/// so "what's in Kansas doing?" does not fall through to Orbitty.
pub fn project_for_line(app: &tauri::AppHandle, line: &str) -> String {
    match crate::state::load_state(app) {
        Ok(st) => project_for_names(st.projects.iter().map(|p| p.name.as_str()), line),
        Err(_) => project_for_names(std::iter::empty(), line),
    }
}

/// Pure half of [`project_for_line`]: explicit `planet <name>`, else the
/// longest real planet named in the line, else orbitty.
pub fn project_for_names<'a>(names: impl IntoIterator<Item = &'a str>, line: &str) -> String {
    if let Some(p) = explicit_planet(line) {
        return p;
    }
    mentioned_planet(names, line).unwrap_or_else(|| "orbitty".into())
}

/// Longest planet name that appears as its own word in `line`.
pub fn mentioned_planet<'a>(
    names: impl IntoIterator<Item = &'a str>,
    line: &str,
) -> Option<String> {
    // Punctuation is a word break ("what's in Kansas?").
    let words = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let padded = format!(" {} ", words(line));
    let mut best: Option<(usize, String)> = None;
    for name in names {
        let n = name.trim();
        if n.len() < 2 {
            continue;
        }
        let low = words(n);
        if low.is_empty() || !padded.contains(&format!(" {low} ")) {
            continue;
        }
        if best
            .as_ref()
            .map(|(len, _)| low.len() > *len)
            .unwrap_or(true)
        {
            best = Some((low.len(), n.to_string()));
        }
    }
    best.map(|(_, n)| n)
}

pub fn run(app: &tauri::AppHandle, name: &str, args: &Value) -> String {
    match name {
        "kanban_list" => kanban_list(app, args),
        other => format!("unknown tool {other}"),
    }
}

fn is_window_claim(kind: Option<&str>, window_id: Option<&str>) -> bool {
    if kind.is_some_and(|k| k.eq_ignore_ascii_case("window")) {
        return true;
    }
    window_id.map(str::trim).is_some_and(|s| !s.is_empty())
}

fn is_needed(id: &str, kind: Option<&str>) -> bool {
    kind.is_some_and(|k| k.eq_ignore_ascii_case("needed")) || id.starts_with("n-")
}

fn needed_still_open(needed_state: Option<&str>) -> bool {
    match needed_state
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
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

/// Open board buckets for one planet. Shared by snapshot + them-row.
#[derive(Clone, Debug)]
pub struct OpenBuckets {
    pub display: String,
    pub todo: Vec<String>,
    pub todo_n: usize,
    pub doing: Vec<String>,
    pub doing_n: usize,
    pub needed: Vec<String>,
    pub needed_n: usize,
}

/// Collect open cards for `project` (name or id). Err is a them-row string.
pub fn collect_open(app: &tauri::AppHandle, project: &str) -> Result<OpenBuckets, String> {
    let st = crate::state::load_state(app).map_err(|_| "board unreadable".to_string())?;
    open_buckets(&st, project)
}

/// Open cards on one planet. Window-claim rows are not todos.
pub fn open_buckets(
    st: &crate::state::PersistedState,
    project: &str,
) -> Result<OpenBuckets, String> {
    let want = project.trim().to_ascii_lowercase();
    let Some(proj) = st
        .projects
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(&want) || p.name.trim().eq_ignore_ascii_case(&want))
    else {
        let names: Vec<_> = st
            .projects
            .iter()
            .map(|p| p.name.trim())
            .filter(|n| !n.is_empty())
            .take(12)
            .collect();
        return Err(format!("no planet {want:?}. have: {}", names.join(", ")));
    };
    let pid = proj.id.clone();
    let display = {
        let n = proj.name.trim();
        if n.is_empty() {
            pid.clone()
        } else {
            n.to_string()
        }
    };
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
        if !status_ok("open", stt) {
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
        } else if is_window_claim(c.kind.as_deref(), c.window_id.as_deref()) {
            // A claimed window is not a todo and not a Doing card.
            continue;
        } else if stt.eq_ignore_ascii_case("doing") {
            doing_n += 1;
            push_cap(&mut doing, title);
        } else {
            todo_n += 1;
            push_cap(&mut todo, title);
        }
    }
    Ok(OpenBuckets {
        display,
        todo,
        todo_n,
        doing,
        doing_n,
        needed,
        needed_n,
    })
}

/// `Kansas — Doing (5): a; b`. Empty buckets are omitted.
/// An empty board names the planet and does not invent "Orbitty".
pub fn honest_lines(
    display: &str,
    todo: &[String],
    todo_n: usize,
    doing: &[String],
    doing_n: usize,
    needed: &[String],
    needed_n: usize,
) -> String {
    let planet = {
        let n = display.trim();
        if n.is_empty() {
            "planet"
        } else {
            n
        }
    };
    let mut lines = Vec::new();
    let push = |lines: &mut Vec<String>, bucket: &str, n: usize, titles: &[String]| {
        if n == 0 {
            return;
        }
        lines.push(format!("{planet} — {bucket} ({n}): {}", titles.join("; ")));
    };
    push(&mut lines, "Todo", todo_n, todo);
    push(&mut lines, "Doing", doing_n, doing);
    push(&mut lines, "Needed", needed_n, needed);
    if lines.is_empty() {
        format!("{planet} — no open cards.")
    } else {
        lines.join("\n")
    }
}

pub fn honest_snapshot(b: &OpenBuckets) -> String {
    honest_lines(
        &b.display, &b.todo, b.todo_n, &b.doing, b.doing_n, &b.needed, b.needed_n,
    )
}

fn kanban_list(app: &tauri::AppHandle, args: &Value) -> String {
    let want = args
        .get("project")
        .and_then(Value::as_str)
        .unwrap_or("orbitty")
        .trim();
    let status = args
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("open")
        .trim()
        .to_ascii_lowercase();
    // Snapshot path still supports status filters; them-row uses open only.
    if status == "open" {
        return match collect_open(app, want) {
            Ok(b) => honest_snapshot(&b),
            Err(e) => e,
        };
    }
    let Ok(st) = crate::state::load_state(app) else {
        return "board unreadable".into();
    };
    let want_l = want.to_ascii_lowercase();
    let found = st.projects.iter().find(|p| {
        p.id.eq_ignore_ascii_case(&want_l) || p.name.trim().eq_ignore_ascii_case(&want_l)
    });
    let Some(proj) = found else {
        let names: Vec<_> = st
            .projects
            .iter()
            .map(|p| p.name.trim())
            .filter(|n| !n.is_empty())
            .take(12)
            .collect();
        return format!("no planet {want_l:?}. have: {}", names.join(", "));
    };
    let pid = proj.id.clone();
    let display = {
        let n = proj.name.trim();
        if n.is_empty() {
            pid.clone()
        } else {
            n.to_string()
        }
    };
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
        } else if is_window_claim(c.kind.as_deref(), c.window_id.as_deref()) {
            continue;
        } else if stt.eq_ignore_ascii_case("doing") {
            doing_n += 1;
            push_cap(&mut doing, title);
        } else {
            todo_n += 1;
            push_cap(&mut todo, title);
        }
    }
    honest_lines(&display, &todo, todo_n, &doing, doing_n, &needed, needed_n)
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
        let out = honest_lines(
            "Orbitty",
            &["GO-ORBITTY-SLEEP-PRESETS".into()],
            1,
            &["Later: harness pick".into()],
            1,
            &["Who runs this doing GO?".into()],
            1,
        );
        assert!(out.contains("Orbitty — Todo (1): GO-ORBITTY-SLEEP-PRESETS"));
        assert!(out.contains("Orbitty — Doing (1): Later: harness pick"));
        assert!(out.contains("Orbitty — Needed (1): Who runs this doing GO?"));
        assert!(!out.contains("Orbitty has"));
        assert!(!out.contains("todo\t"));
    }

    #[test]
    fn board_reads_use_the_named_planet() {
        let names = ["Orbitty", "Kansas", "Kansas Lab"];
        assert_eq!(project_for_names(names, "what's in Kansas doing?"), "Kansas");
        assert_eq!(project_for_names(names, "what's in Kansas?"), "Kansas");
        assert_eq!(project_for_names(names, "cards on Kansas Lab"), "Kansas Lab");
        assert_eq!(project_for_names(names, "what's on the board"), "orbitty");
        assert_eq!(project_for_names(names, "board on planet clinic"), "clinic");
        // Wiring: both board-read doors resolve through project_for_line.
        let src = include_str!("mouth_tools.rs");
        let at = src.find("pub fn project_for_line(").unwrap();
        let body = &src[at..at + src[at..].find("\n}\n").unwrap()];
        assert!(body.contains("project_for_names("), "{body}");
        let board = include_str!("mouth_board.rs");
        assert!(board.contains("crate::mouth_tools::project_for_line(app, raw_line)"));
        assert!(!board.contains("project_from_line("));
        let hop = concat!(include_str!("mouth_hop/local.rs"), include_str!("mouth_hop/openrouter.rs"), include_str!("mouth_hop/handoff.rs"), include_str!("mouth_hop/tests.rs"));
        assert!(hop.contains("crate::mouth_tools::project_for_line(app, raw_line)"));
        assert!(hop.contains("\"project\": project, \"status\": \"open\""));
        assert!(!hop.contains("\"project\": \"orbitty\", \"status\": \"open\""));
    }

    #[test]
    fn mentioned_planet_prefers_the_named_world() {
        assert_eq!(
            mentioned_planet(["Kansas", "Orbitty"], "what's in Kansas doing?").as_deref(),
            Some("Kansas")
        );
        assert_eq!(
            mentioned_planet(["Kansas", "Orbitty"], "what's on the board"),
            None
        );
        assert_eq!(
            mentioned_planet(["Kansas", "Kansas Lab"], "cards on Kansas Lab").as_deref(),
            Some("Kansas Lab")
        );
    }

    fn card(
        id: &str,
        title: &str,
        status: &str,
        kind: &str,
        window: Option<&str>,
    ) -> crate::state::KanbanCard {
        crate::state::KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: "p-kansas".into(),
            window_id: window.map(str::to_string),
            kind: Some(kind.into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: 0.0,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: None,
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links: None,
        }
    }

    #[test]
    fn snapshot_names_kansas_doing_and_skips_window_claims() {
        let mut st = crate::state::PersistedState::default();
        st.projects.push(crate::state::Project {
            id: "p-kansas".into(),
            name: "Kansas".into(),
            trackers: None,
            workspace_root: None,
        });
        st.kanban_cards
            .push(card("c-real", "Real doing", "doing", "todo", None));
        st.kanban_cards
            .push(card("c-todo", "Real todo", "todo", "todo", None));
        for i in 0..5 {
            st.kanban_cards.push(card(
                &format!("c-win-{i}"),
                &format!("Window claim {i}"),
                "doing",
                "window",
                Some(&format!("w-{i}")),
            ));
        }
        let b = open_buckets(&st, "Kansas").expect("kansas");
        let out = honest_snapshot(&b);
        assert!(out.contains("Kansas — Doing (1): Real doing"), "{out}");
        assert!(out.contains("Kansas — Todo (1): Real todo"), "{out}");
        assert!(!out.contains("Window claim"), "{out}");
        assert!(!out.contains("Orbitty"), "{out}");
        assert!(!out.contains("5 todos"), "{out}");
        assert_eq!(b.todo_n, 1);
        assert_eq!(b.doing_n, 1);
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
