//! Hang route table. Keyword gate onto doors that already exist.
//! A miss stays chat. Not a sixth conductor verb. Never a new seat.

use serde_json::json;

/// Closed set. Keep this ≤ 15.
pub const TABLE: [&str; 10] = [
    "gate-status",
    "shift-status",
    "companion-note",
    "open-planet",
    "needed-list",
    "agent-list",
    "planet-list",
    "board-list",
    "board-add",
    "agent-spawn",
];

/// Confirm window. Anything older is dropped and does not write.
pub const CONFIRM_MS: u64 = 60_000;

/// Which existing tool this Hang line is, if any.
/// `yes` is not a route; it confirms a pending board write inside `run`.
pub fn classify(line: &str) -> Option<&'static str> {
    if is_stop_word(line) {
        return None;
    }
    let l = line.trim().to_ascii_lowercase();
    if l.is_empty() {
        return None;
    }
    if let Some(parsed) = parse_board_write(line) {
        return Some(parsed.route);
    }
    if is_gate(&l) {
        return Some("gate-status");
    }
    if is_shift(&l) {
        return Some("shift-status");
    }
    if note_body(line).is_some() {
        return Some("companion-note");
    }
    if open_planet_name(line).is_some() {
        return Some("open-planet");
    }
    if is_needed(&l) {
        return Some("needed-list");
    }
    if is_agents(&l) {
        return Some("agent-list");
    }
    if is_planets(&l) {
        return Some("planet-list");
    }
    if is_board(&l) {
        return Some("board-list");
    }
    None
}

/// What `route.run` returns on every surface (socket, orbittyctl, MCP).
#[derive(Clone, Debug)]
pub struct RouteOutcome {
    pub tool: Option<String>,
    pub spoken: Option<String>,
    pub pending_id: Option<String>,
}

/// Run the tool. None spoken = leave the line for chat.
pub fn run(app: &tauri::AppHandle, line: &str) -> Option<String> {
    run_outcome(app, line).spoken
}

/// Same door as [`run`], plus the pending confirm id.
pub fn run_outcome(app: &tauri::AppHandle, line: &str) -> RouteOutcome {
    if is_stop_word(line) {
        clear_pending();
        return RouteOutcome {
            tool: None,
            spoken: None,
            pending_id: None,
        };
    }
    let now = now_ms();
    let pending = take_pending();
    let loaded = crate::state::load_state(app).ok();
    let id = new_pending_id();
    let decision = match &loaded {
        Some(st) => {
            let planets = planet_rows(st);
            decide(pending, line, &planets, &st.agent_shortcuts, now, &id)
        }
        None => {
            if parse_board_write(line).is_some() || pending.is_some() {
                Decision::Ask {
                    route: parse_board_write(line)
                        .map(|p| p.route)
                        .unwrap_or("board-add"),
                    spoken: "Board unreadable.".into(),
                }
            } else {
                Decision::Miss
            }
        }
    };
    match decision {
        Decision::Miss => run_read(app, line),
        Decision::Confirm { pending, spoken } => {
            let tool = pending.resolved.route.to_string();
            let pending_id = pending.id.clone();
            store_pending(pending);
            log_tool(app, &tool);
            RouteOutcome {
                tool: Some(tool),
                spoken: Some(spoken),
                pending_id: Some(pending_id),
            }
        }
        Decision::Ask { route, spoken } => {
            log_tool(app, route);
            RouteOutcome {
                tool: Some(route.to_string()),
                spoken: Some(spoken),
                pending_id: None,
            }
        }
        Decision::Execute(p) => {
            let tool = p.resolved.route.to_string();
            let spoken = commit(app, &p.resolved);
            log_tool(app, &tool);
            RouteOutcome {
                tool: Some(tool),
                spoken: Some(spoken),
                pending_id: None,
            }
        }
        Decision::Decline(spoken) => RouteOutcome {
            tool: None,
            spoken: Some(spoken),
            pending_id: None,
        },
    }
}

fn run_read(app: &tauri::AppHandle, line: &str) -> RouteOutcome {
    let Some(id) = classify(line) else {
        return RouteOutcome {
            tool: None,
            spoken: None,
            pending_id: None,
        };
    };
    if !TABLE.contains(&id) || id == "board-add" || id == "agent-spawn" {
        return RouteOutcome {
            tool: None,
            spoken: None,
            pending_id: None,
        };
    }
    let spoken = match id {
        "gate-status" => gate_line(),
        "shift-status" => shift_line(app),
        "companion-note" => note_line(line),
        "open-planet" => open_planet(app, line),
        "needed-list" => needed_line(app, line),
        "agent-list" => agent_line(app),
        "planet-list" => planet_line(app),
        "board-list" => crate::mouth_board::speak(app, line),
        _ => {
            return RouteOutcome {
                tool: None,
                spoken: None,
                pending_id: None,
            };
        }
    };
    log_tool(app, id);
    RouteOutcome {
        tool: Some(id.to_string()),
        spoken: Some(spoken),
        pending_id: None,
    }
}

fn is_stop_word(line: &str) -> bool {
    let b = line
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?'))
        .to_ascii_lowercase();
    matches!(b.as_str(), "stop" | "abort" | "chatter")
}

fn is_gate(l: &str) -> bool {
    l.contains("gate status")
        || l.contains("interrupt gate")
        || l.contains("is the gate")
        || l.contains("hang armed")
        || l.contains("is hang armed")
}

fn is_shift(l: &str) -> bool {
    l.contains("shift status") || l.contains("is shift on") || l.contains("is shift off")
}

fn is_needed(l: &str) -> bool {
    l.contains("needed list")
        || l.contains("open needed")
        || l.contains("list needed")
        || l.contains("what's needed")
        || l.contains("whats needed")
}

fn is_agents(l: &str) -> bool {
    l.contains("list agents")
        || l.contains("which agents")
        || l.contains("what agents")
        || l.contains("who is seated")
        || l.contains("who's seated")
}

fn is_planets(l: &str) -> bool {
    l.contains("list planets") || l.contains("which planets") || l.contains("what planets")
}

fn is_board(l: &str) -> bool {
    if l.contains("todo list") || l.contains("board snapshot") || l.contains("open cards") {
        return true;
    }
    let listish = l.contains("list")
        || l.contains("what's on")
        || l.contains("whats on")
        || l.contains("what is on")
        || l.contains("what's in")
        || l.contains("whats in")
        || l.contains("what is in");
    let boardish = l.contains("todo")
        || l.contains("kanban")
        || l.contains("board")
        || l.contains("card")
        || l.contains("doing");
    listish && boardish
}

fn note_body(line: &str) -> Option<String> {
    let t = line.trim();
    let low = t.to_ascii_lowercase();
    let rest = if low.starts_with("companion note") {
        t.get(14..)
            .unwrap_or("")
            .trim()
            .trim_start_matches(':')
            .trim()
    } else if low.starts_with("note:") {
        t.get(5..).unwrap_or("").trim()
    } else {
        return None;
    };
    Some(rest.to_string())
}

fn note_line(line: &str) -> String {
    match note_body(line).as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "Note needs a line.".into(),
    }
}

fn open_planet_name(line: &str) -> Option<String> {
    let l = line.to_ascii_lowercase();
    for marker in ["open planet ", "show planet ", "raise planet "] {
        if let Some(i) = l.find(marker) {
            let rest = line[i + marker.len()..].trim();
            let word: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if word.len() >= 2 {
                return Some(word);
            }
        }
    }
    None
}

fn gate_line() -> String {
    let s = crate::interrupt_gate::interrupt_gate_status();
    format!(
        "Gate is {}. Freeze is {}.",
        if s.armed { "armed" } else { "disarmed" },
        if s.frozen { "on" } else { "off" }
    )
}

fn shift_line(app: &tauri::AppHandle) -> String {
    #[cfg(unix)]
    {
        match crate::shift::status(app) {
            Ok(v) => {
                let on = v.get("on").and_then(|x| x.as_bool()).unwrap_or(false);
                let live = v
                    .get("runners")
                    .and_then(|r| r.get("live"))
                    .and_then(|n| n.as_u64())
                    .unwrap_or(0);
                format!(
                    "Shift is {}. Live runners: {live}.",
                    if on { "on" } else { "off" }
                )
            }
            Err(e) => format!("Shift status failed: {e}"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = app;
        "Shift status is on the Mac build.".into()
    }
}

fn find_project(app: &tauri::AppHandle, name: &str) -> Result<(String, String), String> {
    let st = crate::state::load_state(app).map_err(|_| "board unreadable".to_string())?;
    let want = name.trim().to_ascii_lowercase();
    let Some(p) = st
        .projects
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(&want) || p.name.trim().eq_ignore_ascii_case(&want))
    else {
        return Err(format!("No planet {want}."));
    };
    let display = {
        let n = p.name.trim();
        if n.is_empty() {
            want.clone()
        } else {
            n.to_string()
        }
    };
    Ok((p.id.clone(), display))
}

fn open_planet(app: &tauri::AppHandle, line: &str) -> String {
    let Some(name) = open_planet_name(line) else {
        return "Say which planet to open.".into();
    };
    match find_project(app, &name) {
        Ok((id, display)) => {
            crate::habitat::bring_planet_front(&id);
            let _ = crate::habitat::locate_planet(app.clone(), id);
            format!("Opened {display}.")
        }
        Err(e) => e,
    }
}

fn needed_line(app: &tauri::AppHandle, line: &str) -> String {
    let project = crate::mouth_tools::project_from_line(line);
    match crate::mouth_tools::collect_open(app, &project) {
        Ok(b) if b.needed_n == 0 => format!("No open Needed on {}.", b.display),
        Ok(b) => {
            let mut s = format!("Needed on {}: {}.", b.display, b.needed_n);
            for t in &b.needed {
                s.push('\n');
                s.push_str("- ");
                s.push_str(t);
            }
            s
        }
        Err(e) => e,
    }
}

fn agent_line(app: &tauri::AppHandle) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return "Agents unreadable.".into();
    };
    if st.agent_shortcuts.is_empty() {
        return "No agents.".into();
    }
    let parts: Vec<String> = st
        .agent_shortcuts
        .iter()
        .map(|a| format!("{} ({})", a.name.trim(), a.harness.trim()))
        .collect();
    format!("Agents: {}.", parts.join(", "))
}

fn planet_line(app: &tauri::AppHandle) -> String {
    let Ok(st) = crate::state::load_state(app) else {
        return "Planets unreadable.".into();
    };
    let names: Vec<&str> = st
        .projects
        .iter()
        .map(|p| p.name.trim())
        .filter(|n| !n.is_empty())
        .take(24)
        .collect();
    if names.is_empty() {
        "No planets.".into()
    } else {
        format!("Planets: {}.", names.join(", "))
    }
}

fn log_tool(app: &tauri::AppHandle, tool: &str) {
    crate::event_log::append_best(
        app,
        "companion-hop",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "lane": "code", "tool": tool }),
    );
}

// --- board-add / agent-spawn -------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedWrite {
    pub route: &'static str,
    pub planet: Option<String>,
    pub bucket: &'static str,
    pub title: Option<String>,
    pub agent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanetRow {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub route: &'static str,
    pub planet_id: String,
    pub planet_name: String,
    pub bucket: &'static str,
    pub title: String,
    pub agent_id: String,
    pub agent_name: String,
}

#[derive(Clone, Debug)]
pub struct PendingWrite {
    pub id: String,
    pub at_ms: u64,
    pub resolved: Resolved,
}

pub enum Hit<T> {
    One(T),
    None,
    Ambiguous(Vec<String>),
}

pub enum Resolve {
    Ready(Resolved),
    Ask { route: &'static str, spoken: String },
}

#[derive(Debug)]
pub enum Decision {
    Confirm {
        pending: PendingWrite,
        spoken: String,
    },
    Ask {
        route: &'static str,
        spoken: String,
    },
    Execute(PendingWrite),
    Decline(String),
    Miss,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn new_pending_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("mba-{:x}-{n:x}", now_ms())
}

fn pending_slot() -> &'static std::sync::Mutex<Option<PendingWrite>> {
    static PENDING: std::sync::Mutex<Option<PendingWrite>> = std::sync::Mutex::new(None);
    &PENDING
}

fn take_pending() -> Option<PendingWrite> {
    pending_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

fn store_pending(p: PendingWrite) {
    *pending_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
}

fn clear_pending() {
    let _ = take_pending();
}

fn planet_rows(st: &crate::state::PersistedState) -> Vec<PlanetRow> {
    st.projects
        .iter()
        .map(|p| PlanetRow {
            id: p.id.clone(),
            name: p.name.clone(),
        })
        .collect()
}

fn has_word(l: &str, w: &str) -> bool {
    l.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        .any(|t| t == w)
}

fn is_particle(w: &str) -> bool {
    matches!(
        w,
        "todo"
            | "todos"
            | "doing"
            | "card"
            | "cards"
            | "agent"
            | "agents"
            | "a"
            | "an"
            | "the"
            | "planet"
            | "please"
            | "add"
            | "put"
            | "make"
            | "start"
            | "spawn"
            | "in"
            | "on"
            | "to"
            | "for"
            | "with"
            | "using"
            | "named"
    )
}

fn word_at(s: &str) -> Option<String> {
    let w: String = s
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if w.len() >= 2 {
        Some(w)
    } else {
        None
    }
}

fn take_quote(raw: &str) -> (Option<String>, String) {
    let mut chars = raw.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '"' || c == '\'' {
            let rest = &raw[i + c.len_utf8()..];
            if let Some(rel) = rest.find(c) {
                let title = rest[..rel].trim().to_string();
                let after = &rest[rel + c.len_utf8()..];
                let without = format!("{} {}", &raw[..i], after);
                let title = if title.is_empty() { None } else { Some(title) };
                return (title, without);
            }
        }
    }
    (None, raw.to_string())
}

fn planet_token(raw: &str) -> Option<String> {
    let low = raw.to_ascii_lowercase();
    if let Some(i) = low.find("planet ") {
        if let Some(w) = word_at(&raw[i + "planet ".len()..]) {
            return Some(w);
        }
    }
    for marker in [" to ", " on ", " for "] {
        let mut search = low.as_str();
        let mut base = 0usize;
        while let Some(rel) = search.find(marker) {
            let abs = base + rel + marker.len();
            if let Some(w) = word_at(&raw[abs..]) {
                if !is_particle(&w.to_ascii_lowercase()) {
                    return Some(w);
                }
            }
            let next = rel + marker.len();
            base += next;
            search = &search[next..];
        }
    }
    None
}

fn agent_token(raw: &str) -> Option<String> {
    let low = raw.to_ascii_lowercase();
    for marker in ["named ", "with ", "using "] {
        if let Some(i) = low.find(marker) {
            if let Some(w) = word_at(&raw[i + marker.len()..]) {
                if !is_particle(&w.to_ascii_lowercase()) {
                    return Some(w);
                }
            }
        }
    }
    None
}

fn strip_prefix_ci(s: &str, prefix: &str) -> Option<String> {
    let low = s.to_ascii_lowercase();
    let p = prefix.to_ascii_lowercase();
    if low.starts_with(&p) {
        Some(s[p.len()..].to_string())
    } else {
        None
    }
}

fn strip_leading_verb(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if let Some(rest) = strip_prefix_ci(s.trim(), "please ") {
        s = rest.trim().to_string();
    }
    let low = s.to_ascii_lowercase();
    let verb = ["add", "put", "make", "start", "spawn"]
        .into_iter()
        .find(|v| {
            low.starts_with(v)
                && low
                    .as_bytes()
                    .get(v.len())
                    .map(|b| b.is_ascii_whitespace())
                    .unwrap_or(true)
        });
    let Some(verb) = verb else {
        return s;
    };
    let mut rest = s[verb.len()..].trim().to_string();
    let low_rest = rest.to_ascii_lowercase();
    for art in ["an ", "a ", "the "] {
        if low_rest.starts_with(art) {
            rest = rest[art.len()..].trim().to_string();
            break;
        }
    }
    let low_rest = rest.to_ascii_lowercase();
    for obj in [
        "cards ", "card ", "todos ", "todo ", "agents ", "agent ", "cards", "card", "todos",
        "todo", "agents", "agent",
    ] {
        if !low_rest.starts_with(obj) {
            continue;
        }
        let n = obj.len();
        let boundary = obj.ends_with(' ')
            || low_rest.len() == n
            || low_rest
                .as_bytes()
                .get(n)
                .map(|b| !b.is_ascii_alphanumeric())
                .unwrap_or(true);
        if boundary {
            rest = rest[n..].trim().to_string();
            break;
        }
    }
    rest
}

fn strip_phrase_ci(s: &str, phrase: &str) -> String {
    let low = s.to_ascii_lowercase();
    let p = phrase.to_ascii_lowercase();
    if let Some(i) = low.find(&p) {
        let mut out = String::new();
        out.push_str(&s[..i]);
        out.push(' ');
        out.push_str(&s[i + p.len()..]);
        out
    } else {
        s.to_string()
    }
}

fn cleanup_title(s: &str) -> String {
    s.split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, ':' | ',' | ';' | '.')))
        .filter(|w| !w.is_empty())
        .filter(|w| {
            let l = w.to_ascii_lowercase();
            !matches!(
                l.as_str(),
                "doing"
                    | "todo"
                    | "todos"
                    | "in"
                    | "to"
                    | "on"
                    | "for"
                    | "planet"
                    | "with"
                    | "using"
                    | "named"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn remainder_title(raw: &str, planet: Option<&str>, agent: Option<&str>) -> String {
    let mut s = strip_leading_verb(raw);
    if let Some(p) = planet {
        s = strip_phrase_ci(&s, &format!("planet {p}"));
        s = strip_phrase_ci(&s, &format!("to {p}"));
        s = strip_phrase_ci(&s, &format!("on {p}"));
        s = strip_phrase_ci(&s, &format!("for {p}"));
    }
    if let Some(a) = agent {
        s = strip_phrase_ci(&s, &format!("named {a}"));
        s = strip_phrase_ci(&s, &format!("with {a}"));
        s = strip_phrase_ci(&s, &format!("using {a}"));
    }
    cleanup_title(&s)
}

/// Keyword parse. Planet/agent names are resolved later against real rows.
pub fn parse_board_write(line: &str) -> Option<ParsedWrite> {
    let raw = line
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?'));
    if raw.is_empty() || is_stop_word(raw) {
        return None;
    }
    // Quoted text is the title, not a route keyword ("Card title").
    let (quoted, without) = take_quote(raw);
    let low = without.to_ascii_lowercase();
    let verb = ["add", "put", "make", "start", "spawn"]
        .iter()
        .any(|w| has_word(&low, w));
    if !verb {
        return None;
    }
    let has_agent = has_word(&low, "agent") || has_word(&low, "agents");
    let has_card = has_word(&low, "card")
        || has_word(&low, "cards")
        || has_word(&low, "todo")
        || has_word(&low, "todos");
    let has_doing = has_word(&low, "doing");
    let has_start = has_word(&low, "start") || has_word(&low, "spawn");
    let route = if has_agent && !has_card && (has_doing || has_start) {
        "agent-spawn"
    } else if has_card && (has_word(&low, "add") || has_word(&low, "put") || has_word(&low, "make"))
    {
        "board-add"
    } else {
        return None;
    };
    let planet = planet_token(&without);
    let agent = agent_token(&without);
    let bucket = if route == "agent-spawn" || has_doing {
        "doing"
    } else {
        "todo"
    };
    let title = if let Some(q) = quoted {
        Some(q)
    } else {
        let t = remainder_title(&without, planet.as_deref(), agent.as_deref());
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    };
    Some(ParsedWrite {
        route,
        planet,
        bucket,
        title,
        agent,
    })
}

fn display_name(row: &PlanetRow) -> String {
    let n = row.name.trim();
    if n.is_empty() {
        row.id.clone()
    } else {
        n.to_string()
    }
}

pub fn match_planet(token: &str, rows: &[PlanetRow]) -> Hit<(String, String)> {
    let want = token.trim().to_ascii_lowercase();
    if want.is_empty() {
        return Hit::None;
    }
    let exact_id: Vec<&PlanetRow> = rows
        .iter()
        .filter(|p| p.id.eq_ignore_ascii_case(&want))
        .collect();
    if exact_id.len() == 1 {
        return Hit::One((exact_id[0].id.clone(), display_name(exact_id[0])));
    }
    let exact_name: Vec<&PlanetRow> = rows
        .iter()
        .filter(|p| p.name.trim().eq_ignore_ascii_case(&want))
        .collect();
    if exact_name.len() == 1 {
        return Hit::One((exact_name[0].id.clone(), display_name(exact_name[0])));
    }
    if exact_name.len() > 1 {
        return Hit::Ambiguous(exact_name.iter().map(|p| display_name(p)).collect());
    }
    if want.len() >= 3 {
        let prefs: Vec<&PlanetRow> = rows
            .iter()
            .filter(|p| p.name.trim().to_ascii_lowercase().starts_with(&want))
            .collect();
        if prefs.len() == 1 {
            return Hit::One((prefs[0].id.clone(), display_name(prefs[0])));
        }
        if prefs.len() > 1 {
            return Hit::Ambiguous(prefs.iter().map(|p| display_name(p)).collect());
        }
        let cont: Vec<&PlanetRow> = rows
            .iter()
            .filter(|p| p.name.trim().to_ascii_lowercase().contains(&want))
            .collect();
        if cont.len() == 1 {
            return Hit::One((cont[0].id.clone(), display_name(cont[0])));
        }
        if cont.len() > 1 {
            return Hit::Ambiguous(cont.iter().map(|p| display_name(p)).collect());
        }
    }
    Hit::None
}

fn match_agent(
    named: Option<&str>,
    planet_id: &str,
    agents: &[crate::state::AgentShortcut],
) -> Hit<(String, String)> {
    if let Some(raw) = named.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(a) = agents.iter().find(|a| a.id == raw) {
            return Hit::One((a.id.clone(), a.name.clone()));
        }
        let exact: Vec<&crate::state::AgentShortcut> =
            agents.iter().filter(|a| a.name == raw).collect();
        if exact.len() == 1 {
            return Hit::One((exact[0].id.clone(), exact[0].name.clone()));
        }
        let lower = raw.to_ascii_lowercase();
        let ci: Vec<&crate::state::AgentShortcut> = agents
            .iter()
            .filter(|a| a.name.to_ascii_lowercase() == lower)
            .collect();
        return match ci.len() {
            1 => Hit::One((ci[0].id.clone(), ci[0].name.clone())),
            0 => Hit::None,
            _ => Hit::Ambiguous(ci.iter().map(|a| a.name.clone()).collect()),
        };
    }
    let pinned: Vec<&crate::state::AgentShortcut> = agents
        .iter()
        .filter(|a| a.project_id.as_deref() == Some(planet_id))
        .collect();
    match pinned.len() {
        1 => Hit::One((pinned[0].id.clone(), pinned[0].name.clone())),
        0 => Hit::None,
        _ => Hit::Ambiguous(pinned.iter().map(|a| a.name.clone()).collect()),
    }
}

pub fn resolve(
    parsed: &ParsedWrite,
    planets: &[PlanetRow],
    agents: &[crate::state::AgentShortcut],
) -> Resolve {
    let Some(token) = parsed.planet.as_deref() else {
        return Resolve::Ask {
            route: parsed.route,
            spoken: "Which planet?".into(),
        };
    };
    let (planet_id, planet_name) = match match_planet(token, planets) {
        Hit::One(pair) => pair,
        Hit::None => {
            return Resolve::Ask {
                route: parsed.route,
                spoken: format!("No planet {token}."),
            };
        }
        Hit::Ambiguous(names) => {
            return Resolve::Ask {
                route: parsed.route,
                spoken: format!("Which planet — {}?", names.join(" or ")),
            };
        }
    };
    let title = if let Some(t) = parsed
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        t.to_string()
    } else if parsed.route == "agent-spawn" {
        "agent".into()
    } else {
        return Resolve::Ask {
            route: parsed.route,
            spoken: "What title?".into(),
        };
    };
    let agent = match match_agent(parsed.agent.as_deref(), &planet_id, agents) {
        Hit::One(pair) => pair,
        Hit::None if parsed.bucket == "doing" => {
            return Resolve::Ask {
                route: parsed.route,
                spoken: format!("Doing needs an agent. Which agent on {planet_name}?"),
            };
        }
        Hit::None => {
            return Resolve::Ask {
                route: parsed.route,
                spoken: format!("Which agent on {planet_name}?"),
            };
        }
        Hit::Ambiguous(names) => {
            return Resolve::Ask {
                route: parsed.route,
                spoken: format!("Which agent — {}?", names.join(" or ")),
            };
        }
    };
    Resolve::Ready(Resolved {
        route: parsed.route,
        planet_id,
        planet_name,
        bucket: parsed.bucket,
        title,
        agent_id: agent.0,
        agent_name: agent.1,
    })
}

pub fn confirm_line(r: &Resolved) -> String {
    format!(
        "Add '{}' to {} {} with {}? (yes / no)",
        r.title, r.planet_name, r.bucket, r.agent_name
    )
}

fn norm_reply(line: &str) -> String {
    line.trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?' | ','))
        .trim()
        .to_ascii_lowercase()
}

fn is_yes(line: &str) -> bool {
    matches!(norm_reply(line).as_str(), "yes" | "ok" | "okay" | "do it")
}

fn is_no(line: &str) -> bool {
    matches!(
        norm_reply(line).as_str(),
        "no" | "nope" | "nah" | "cancel" | "don't" | "do not"
    )
}

/// Pure confirm machine. A confirm does not touch `PersistedState`.
/// Timeout and "no" return without [`Decision::Execute`].
pub fn decide(
    pending: Option<PendingWrite>,
    line: &str,
    planets: &[PlanetRow],
    agents: &[crate::state::AgentShortcut],
    now: u64,
    next_id: &str,
) -> Decision {
    let pending = pending.filter(|p| now.saturating_sub(p.at_ms) < CONFIRM_MS);
    if let Some(p) = pending {
        if is_yes(line) {
            return Decision::Execute(p);
        }
        if is_no(line) {
            return Decision::Decline("Okay, nothing added.".into());
        }
        // Anything else drops the pending write, then the line is read fresh.
    }
    if is_yes(line) || is_no(line) {
        return Decision::Miss;
    }
    let Some(parsed) = parse_board_write(line) else {
        return Decision::Miss;
    };
    match resolve(&parsed, planets, agents) {
        Resolve::Ask { route, spoken } => Decision::Ask { route, spoken },
        Resolve::Ready(r) => {
            let spoken = confirm_line(&r);
            Decision::Confirm {
                pending: PendingWrite {
                    id: next_id.to_string(),
                    at_ms: now,
                    resolved: r,
                },
                spoken,
            }
        }
    }
}

fn blank_card(id: &str, r: &Resolved, status: &str) -> crate::state::KanbanCard {
    crate::state::KanbanCard {
        id: id.to_string(),
        title: r.title.clone(),
        project_id: r.planet_id.clone(),
        window_id: None,
        kind: Some("todo".into()),
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
        actor: Some("agent".into()),
        assigned_agent_id: Some(r.agent_id.clone()),
        parent_id: None,
        rating: None,
        trackers: None,
        task_id: None,
        task_reason: None,
        links: None,
    }
}

/// Fixture mirror of kanban_add → assign → (doing) the droid row.
/// Does not call `agent_pickup` and does not launch a harness.
pub fn apply_confirmed(
    st: &mut crate::state::PersistedState,
    r: &Resolved,
) -> Result<String, String> {
    if !st.projects.iter().any(|p| p.id == r.planet_id) {
        return Err(format!("no planet {}", r.planet_name));
    }
    let id = format!("c-fixture-{}", st.kanban_cards.len() + 1);
    let status = if r.bucket == "doing" { "doing" } else { "todo" };
    let card = blank_card(&id, r, status);
    if r.bucket == "doing" {
        st.agent_runs.push(crate::state::AgentRun {
            card_id: card.id.clone(),
            planet_id: r.planet_id.clone(),
            agent_shortcut_id: r.agent_id.clone(),
            state: "working".into(),
            herdr: None,
        });
    }
    let card_id = card.id.clone();
    st.kanban_cards.push(card);
    Ok(card_id)
}

pub fn state_hash(st: &crate::state::PersistedState) -> String {
    serde_json::to_string(&(&st.kanban_cards, &st.agent_runs)).unwrap_or_default()
}

/// Live door. Adds as todo, assigns, and pickups only for doing.
/// Each door saves through `state::update`. No write happens before yes.
#[cfg(unix)]
fn commit_board(app: &tauri::AppHandle, r: &Resolved) -> String {
    let card = match crate::board::kanban_add(app, &r.title, &r.planet_id, "todo", None, Vec::new())
    {
        Ok(c) => c,
        Err(e) => return e,
    };
    if let Err(e) = crate::agents::kanban_assign(app, &card.id, &r.agent_id) {
        return format!(
            "Added {} on {} but assign failed: {e}",
            card.id, r.planet_name
        );
    }
    if r.bucket == "doing" {
        if let Err(e) = crate::agents::agent_pickup(app, Some(&r.agent_id), None, Some(&card.id)) {
            log_board_add(app, r, &card.id);
            return format!(
                "Added {} on {} but pickup failed: {e}",
                card.id, r.planet_name
            );
        }
    }
    log_board_add(app, r, &card.id);
    format!("Added {} on {}.", card.id, r.planet_name)
}

#[cfg(unix)]
fn log_board_add(app: &tauri::AppHandle, r: &Resolved, card_id: &str) {
    crate::event_log::append_best(
        app,
        "mouth-board-add",
        None,
        Some(&r.planet_id),
        Some("agent"),
        true,
        &json!({
            "planet": r.planet_name,
            "bucket": r.bucket,
            "agent": r.agent_id,
            "card": card_id,
        }),
    );
}

fn commit(app: &tauri::AppHandle, r: &Resolved) -> String {
    #[cfg(unix)]
    {
        commit_board(app, r)
    }
    #[cfg(not(unix))]
    {
        let _ = (app, r);
        "Board add is on the Mac build.".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_small_and_closed() {
        assert!(TABLE.len() <= 15);
        assert_eq!(TABLE.len(), 10);
        assert!(TABLE.contains(&"board-add"));
        assert!(TABLE.contains(&"agent-spawn"));
    }

    #[test]
    fn board_list_phrase() {
        assert_eq!(
            classify("tell me about the todo list on planet orbitty"),
            Some("board-list")
        );
        assert_eq!(classify("what's on the board"), Some("board-list"));
    }

    #[test]
    fn open_planet_is_not_the_board() {
        assert_eq!(classify("open planet orbitty"), Some("open-planet"));
        assert_eq!(classify("show planet clinic"), Some("open-planet"));
        assert_ne!(classify("open planet orbitty"), Some("board-list"));
    }

    #[test]
    fn note_gate_and_lists() {
        assert_eq!(classify("note: mouth ping"), Some("companion-note"));
        assert_eq!(classify("gate status"), Some("gate-status"));
        assert_eq!(classify("shift status"), Some("shift-status"));
        assert_eq!(classify("list planets"), Some("planet-list"));
        assert_eq!(classify("list agents"), Some("agent-list"));
        assert_eq!(
            classify("needed list on planet orbitty"),
            Some("needed-list")
        );
    }

    #[test]
    fn chat_and_stop_stay_out() {
        assert_eq!(classify("tell me a joke"), None);
        assert_eq!(classify("STOP"), None);
        assert_eq!(classify("stop"), None);
        assert_eq!(classify("ping"), None);
        assert_eq!(classify("how is the weather"), None);
    }

    #[test]
    fn note_body_keeps_the_words() {
        assert_eq!(note_line("note: mouth ping"), "mouth ping");
        assert_eq!(note_line("companion note: hello"), "hello");
        assert_eq!(note_line("note:"), "Note needs a line.");
    }

    fn expect_add(line: &str, planet: &str, bucket: &str, title: Option<&str>) {
        let p = parse_board_write(line).unwrap_or_else(|| panic!("parse {line}"));
        assert_eq!(p.route, "board-add", "{line}");
        assert_eq!(p.planet.as_deref(), Some(planet), "{line}");
        assert_eq!(p.bucket, bucket, "{line}");
        assert_eq!(p.title.as_deref(), title, "{line}");
        assert_eq!(classify(line), Some("board-add"), "{line}");
    }

    fn expect_spawn(line: &str, planet: &str, title: Option<&str>) {
        let p = parse_board_write(line).unwrap_or_else(|| panic!("parse {line}"));
        assert_eq!(p.route, "agent-spawn", "{line}");
        assert_eq!(p.planet.as_deref(), Some(planet), "{line}");
        assert_eq!(p.bucket, "doing", "{line}");
        assert_eq!(p.title.as_deref(), title, "{line}");
        assert_eq!(classify(line), Some("agent-spawn"), "{line}");
    }

    #[test]
    fn board_add_phrasings() {
        expect_add(
            "add a card Buy milk to planet Kansas todo",
            "Kansas",
            "todo",
            Some("Buy milk"),
        );
        expect_add(
            "add a card \"Buy milk\" to planet Kansas",
            "Kansas",
            "todo",
            Some("Buy milk"),
        );
        expect_add(
            "put a todo Fix the gate on planet orbitty",
            "orbitty",
            "todo",
            Some("Fix the gate"),
        );
        expect_add(
            "make a card Review notes to kansas todo",
            "kansas",
            "todo",
            Some("Review notes"),
        );
        expect_add(
            "add card \"Ship alpha\" to planet clinic",
            "clinic",
            "todo",
            Some("Ship alpha"),
        );
        expect_add(
            "put card hello world on planet orbitty todo",
            "orbitty",
            "todo",
            Some("hello world"),
        );
        expect_add(
            "make a todo \"Call lab\" to planet kansas",
            "kansas",
            "todo",
            Some("Call lab"),
        );
        expect_add(
            "add a todo Write the note to planet orbitty",
            "orbitty",
            "todo",
            Some("Write the note"),
        );
        expect_add(
            "add a card test-add to planet fixture todo",
            "fixture",
            "todo",
            Some("test-add"),
        );
        expect_add(
            "please add a card \"Standup\" to planet kansas todo",
            "kansas",
            "todo",
            Some("Standup"),
        );
        expect_add(
            "make todo \"Inbox zero\" to planet clinic todo",
            "clinic",
            "todo",
            Some("Inbox zero"),
        );
        expect_add(
            "add a card Check mail to planet orbitty",
            "orbitty",
            "todo",
            Some("Check mail"),
        );
    }

    #[test]
    fn agent_spawn_phrasings() {
        expect_spawn("add an agent in doing to planet Kansas", "Kansas", None);
        expect_spawn(
            "start an agent on kansas doing \"Fix the leak\"",
            "kansas",
            Some("Fix the leak"),
        );
        expect_spawn(
            "spawn an agent on planet orbitty doing Write tests",
            "orbitty",
            Some("Write tests"),
        );
        expect_spawn(
            "add an agent doing \"Review PR\" on planet clinic",
            "clinic",
            Some("Review PR"),
        );
        expect_spawn(
            "start agent on planet kansas doing Ship it",
            "kansas",
            Some("Ship it"),
        );
        expect_spawn(
            "spawn agent doing hello on planet orbitty",
            "orbitty",
            Some("hello"),
        );
        expect_spawn(
            "add agent in doing to planet fixture \"test spawn\"",
            "fixture",
            Some("test spawn"),
        );
        expect_spawn(
            "start an agent on planet kansas doing: check the board",
            "kansas",
            Some("check the board"),
        );
        expect_spawn(
            "please spawn an agent on orbitty doing \"Night run\"",
            "orbitty",
            Some("Night run"),
        );
        expect_spawn(
            "add an agent to planet kansas doing the dishes",
            "kansas",
            Some("the dishes"),
        );
        expect_spawn(
            "start an agent doing \"Card title\" to planet kansas",
            "kansas",
            Some("Card title"),
        );
        expect_spawn(
            "spawn an agent in doing on planet kansas \"Walk the list\"",
            "kansas",
            Some("Walk the list"),
        );
    }

    #[test]
    fn matts_line_is_agent_spawn_on_kansas_doing() {
        let line = "add an agent in doing to planet Kansas";
        let p = parse_board_write(line).expect("parse");
        assert_eq!(classify(line), Some("agent-spawn"));
        assert_eq!(p.planet.as_deref(), Some("Kansas"));
        assert_eq!(p.bucket, "doing");
        assert!(p.title.is_none());
    }

    #[test]
    fn kansas_doing_question_stays_a_board_read() {
        assert_eq!(classify("what's in Kansas doing?"), Some("board-list"));
        assert_ne!(classify("what's in Kansas doing?"), Some("agent-spawn"));
    }

    fn fixture() -> (
        crate::state::PersistedState,
        Vec<PlanetRow>,
        Vec<crate::state::AgentShortcut>,
    ) {
        let mut st = crate::state::PersistedState::default();
        st.projects.push(crate::state::Project {
            id: "p-fixture".into(),
            name: "Fixture".into(),
            trackers: None,
            workspace_root: None,
        });
        st.projects.push(crate::state::Project {
            id: "p-kansas".into(),
            name: "Kansas".into(),
            trackers: None,
            workspace_root: None,
        });
        st.projects.push(crate::state::Project {
            id: "p-lab".into(),
            name: "Kansas Lab".into(),
            trackers: None,
            workspace_root: None,
        });
        let agent = crate::state::AgentShortcut {
            id: "a-fixture".into(),
            name: "Fixture Agent".into(),
            harness: "cursor".into(),
            profile: String::new(),
            model: None,
            md_paths: Vec::new(),
            project_id: Some("p-fixture".into()),
            extra: None,
            permissions: "read-only".into(),
        };
        let kansas = crate::state::AgentShortcut {
            id: "a-kansas".into(),
            name: "Kansas Agent".into(),
            harness: "cursor".into(),
            profile: String::new(),
            model: None,
            md_paths: Vec::new(),
            project_id: Some("p-kansas".into()),
            extra: None,
            permissions: "read-only".into(),
        };
        st.agent_shortcuts.push(agent);
        st.agent_shortcuts.push(kansas);
        let planets = planet_rows(&st);
        let agents = st.agent_shortcuts.clone();
        (st, planets, agents)
    }

    #[test]
    fn no_and_timeout_leave_the_fixture_hash() {
        let (st, planets, agents) = fixture();
        let hash = state_hash(&st);
        let now = 1_000_000u64;
        let d = decide(
            None,
            "add a card test-add to planet fixture todo",
            &planets,
            &agents,
            now,
            "mba-test",
        );
        let Decision::Confirm { pending, spoken } = d else {
            panic!("expected confirm");
        };
        assert!(spoken.contains("test-add"));
        assert!(spoken.contains("Fixture"));
        assert!(spoken.contains("todo"));
        assert!(spoken.contains("Fixture Agent"));
        assert!(spoken.contains("yes / no"));
        assert_eq!(pending.id, "mba-test");
        assert_eq!(state_hash(&st), hash, "confirm must not write");

        let d = decide(Some(pending.clone()), "no", &planets, &agents, now, "mba-2");
        assert!(matches!(d, Decision::Decline(_)));
        assert_eq!(state_hash(&st), hash, "no must not write");

        let mut stale = pending;
        stale.at_ms = now.saturating_sub(CONFIRM_MS + 1);
        let d = decide(Some(stale), "yes", &planets, &agents, now, "mba-3");
        assert!(matches!(d, Decision::Miss), "timeout must not execute");
        assert_eq!(state_hash(&st), hash, "timeout must not write");
    }

    #[test]
    fn yes_applies_fixture_add_and_spawn_only_in_memory() {
        let (mut st, planets, agents) = fixture();
        let hash = state_hash(&st);
        let now = 2_000_000u64;
        let d = decide(
            None,
            "add a card test-add to planet fixture todo",
            &planets,
            &agents,
            now,
            "mba-add",
        );
        let Decision::Confirm { pending, .. } = d else {
            panic!("confirm add");
        };
        assert_eq!(state_hash(&st), hash);
        let d = decide(Some(pending), "yes", &planets, &agents, now, "mba-x");
        let Decision::Execute(p) = d else {
            panic!("execute add");
        };
        let id = apply_confirmed(&mut st, &p.resolved).expect("apply add");
        assert_eq!(id, "c-fixture-1");
        let card = st.kanban_cards.iter().find(|c| c.id == id).expect("card");
        assert_eq!(card.title, "test-add");
        assert_eq!(card.project_id, "p-fixture");
        assert_eq!(card.status, "todo");
        assert_eq!(card.assigned_agent_id.as_deref(), Some("a-fixture"));
        assert!(
            st.agent_runs.is_empty(),
            "todo assign does not spawn a droid"
        );

        let d = decide(
            None,
            "start an agent on planet fixture doing \"walk\"",
            &planets,
            &agents,
            now,
            "mba-spawn",
        );
        let Decision::Confirm { pending, spoken } = d else {
            panic!("confirm spawn");
        };
        assert!(spoken.starts_with("Add 'walk' to Fixture doing with Fixture Agent?"));
        let d = decide(Some(pending), "do it", &planets, &agents, now, "mba-y");
        let Decision::Execute(p) = d else {
            panic!("execute spawn");
        };
        assert_eq!(p.resolved.bucket, "doing");
        let id = apply_confirmed(&mut st, &p.resolved).expect("apply spawn");
        let card = st
            .kanban_cards
            .iter()
            .find(|c| c.id == id)
            .expect("spawn card");
        assert_eq!(card.status, "doing");
        assert_eq!(card.project_id, "p-fixture");
        assert!(
            st.agent_runs
                .iter()
                .any(|r| { r.card_id == id && r.planet_id == "p-fixture" && r.state == "working" }),
            "droid row"
        );
        assert!(
            !st.projects.iter().any(|p| p.name == "Kansas"
                && st.kanban_cards.iter().any(|c| c.project_id == "p-kansas")),
            "fixture writes must not land on Kansas"
        );
    }

    #[test]
    fn matts_line_confirms_against_a_pinned_agent() {
        let (_st, planets, agents) = fixture();
        let parsed = parse_board_write("add an agent in doing to planet Kansas").unwrap();
        let Resolve::Ready(r) = resolve(&parsed, &planets, &agents) else {
            panic!("ready");
        };
        assert_eq!(r.route, "agent-spawn");
        assert_eq!(r.planet_name, "Kansas");
        assert_eq!(r.planet_id, "p-kansas");
        assert_eq!(r.bucket, "doing");
        assert_eq!(r.title, "agent");
        assert_eq!(r.agent_name, "Kansas Agent");
        assert_eq!(
            confirm_line(&r),
            "Add 'agent' to Kansas doing with Kansas Agent? (yes / no)"
        );
    }

    #[test]
    fn ambiguous_planet_asks_and_does_not_confirm() {
        let (st, planets, agents) = fixture();
        let hash = state_hash(&st);
        let d = decide(
            None,
            "add a card milk to planet kan todo",
            &planets,
            &agents,
            10,
            "mba-amb",
        );
        match d {
            Decision::Ask { spoken, .. } => assert!(spoken.contains("Which planet"), "{spoken}"),
            other => panic!("ask, got confirm? {other:?}"),
        }
        assert_eq!(state_hash(&st), hash);
    }

    #[test]
    fn commit_uses_the_board_doors_in_order() {
        let src = include_str!("mouth_route.rs");
        let start = src.find("fn commit_board(").expect("commit_board");
        let body = &src[start..start + src[start..].find("\n}\n").unwrap()];
        let add = body.find("kanban_add(").expect("kanban_add");
        let assign = body.find("kanban_assign(").expect("assign");
        let pickup = body.find("agent_pickup(").expect("pickup");
        assert!(add < assign && assign < pickup, "{body}");
        let log_at = src.find("fn log_board_add(").expect("log");
        let log_body = &src[log_at..log_at + src[log_at..].find("\n}\n").unwrap()];
        assert!(body.contains("log_board_add("));
        assert!(log_body.contains("mouth-board-add"), "{log_body}");
        assert!(log_body.contains("\"planet\""));
        assert!(log_body.contains("\"bucket\""));
        assert!(log_body.contains("\"card\""));
        let apply = src.find("fn apply_confirmed(").unwrap();
        let apply_body = &src[apply..apply + src[apply..].find("\n}\n").unwrap()];
        assert!(!apply_body.contains("agent_pickup"));
        assert!(!apply_body.contains("launch_agent"));
        assert!(apply_body.contains("agent_runs"));
    }

    #[test]
    fn route_run_is_one_door_on_three_surfaces() {
        let control = include_str!("control.rs");
        assert!(control.contains("mouth_route::run_outcome"));
        assert!(control.contains("pendingId"));
        let ctl = include_str!("bin/orbittyctl.rs");
        assert!(ctl.contains("[\"route\", \"run\"]"));
        assert!(ctl.contains("pendingId"));
        let mcp = include_str!("bin/orbitty_mcp.rs");
        assert!(mcp.contains("name: \"route.run\""));
        assert!(mcp.contains("name: \"route.table\""));
        assert!(mcp.contains("pendingId"));
    }

    #[test]
    fn write_lane_still_reaches_the_route() {
        let src = include_str!("coordinator.rs");
        let start = src.find("Board add / agent spawn").expect("comment");
        let slice = &src[start..start + 400];
        assert!(slice.contains("mouth_route::run"));
        assert!(!slice.contains("needs_write_lane"));
    }
}
