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

/// Which surface sent the line. A pending board write confirms only on
/// the surface that proposed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The app composer (Tauri `companion_send`). A bare yes confirms a
    /// chat-made pending only.
    Chat,
    /// Socket / orbittyctl / MCP `route.run`. The yes must carry the
    /// matching `pendingId`.
    Socket,
    /// Socket `companion.send`. Never proposes or confirms a board write
    /// (the companion profile is denied kanban.add / agent.pickup).
    Companion,
}

/// Run the tool. None spoken = leave the line for chat.
pub fn run(app: &tauri::AppHandle, line: &str, origin: Origin) -> Option<String> {
    run_outcome(app, line, origin, None).spoken
}

/// Same door as [`run`], plus the pending confirm id. `confirm_id` is the
/// `pendingId` a socket caller got back from its own proposal.
pub fn run_outcome(
    app: &tauri::AppHandle,
    line: &str,
    origin: Origin,
    confirm_id: Option<&str>,
) -> RouteOutcome {
    let now = std::time::Instant::now();
    let id = new_pending_id();
    let decision = {
        let mut slot = pending_slot().lock().unwrap_or_else(|e| e.into_inner());
        if is_stop_word(line) {
            step(&mut slot, line, &[], &[], now, &id, origin, confirm_id)
        } else {
            match crate::state::load_state(app) {
                Ok(st) => {
                    let planets = planet_rows(&st);
                    step(
                        &mut slot,
                        line,
                        &planets,
                        &st.agent_shortcuts,
                        now,
                        &id,
                        origin,
                        confirm_id,
                    )
                }
                Err(_) => unreadable(&mut slot, line, now, origin, confirm_id),
            }
        }
    };
    match decision {
        Decision::Miss => {
            // Base path: write-lane lines skip the read doors (as before
            // the board-add route) and hop to the write lane.
            if crate::mouth_gist::needs_write_lane(line) {
                return RouteOutcome {
                    tool: None,
                    spoken: None,
                    pending_id: None,
                };
            }
            run_read(app, line)
        }
        Decision::Confirm { pending, spoken } => {
            let tool = pending.resolved.route.to_string();
            // `step` already stored it in the slot.
            let pending_id = pending.id.clone();
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

/// Board unreadable: a board write (or the owner's reply to its pending)
/// gets an honest ask and never executes.
fn unreadable(
    slot: &mut Option<PendingWrite>,
    line: &str,
    now: std::time::Instant,
    origin: Origin,
    confirm_id: Option<&str>,
) -> Decision {
    let mine = slot
        .as_ref()
        .is_some_and(|p| fresh(p, now) && owns(p, origin, confirm_id));
    let write = origin != Origin::Companion && parse_board_write(line).is_some();
    if !mine && !write {
        return Decision::Miss;
    }
    if mine {
        *slot = None;
    }
    Decision::Ask {
        route: parse_board_write(line)
            .map(|p| p.route)
            .unwrap_or("board-add"),
        spoken: "Board unreadable.".into(),
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
    /// Monotonic: a wall-clock step cannot keep a pending alive.
    pub at: std::time::Instant,
    /// The surface that proposed it. Only that surface can confirm.
    pub origin: Origin,
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

/// 128 random bits (urandom) so a socket confirm cannot guess another
/// caller's pendingId. Time + counter only if urandom is unreadable.
fn new_pending_id() -> String {
    use std::io::Read;
    let mut buf = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok();
    if ok && buf.iter().any(|b| *b != 0) {
        let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
        return format!("mba-{hex}");
    }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("mba-{t:x}-{n:x}")
}

fn pending_slot() -> &'static std::sync::Mutex<Option<PendingWrite>> {
    static PENDING: std::sync::Mutex<Option<PendingWrite>> = std::sync::Mutex::new(None);
    &PENDING
}

fn fresh(p: &PendingWrite, now: std::time::Instant) -> bool {
    now.saturating_duration_since(p.at) < std::time::Duration::from_millis(CONFIRM_MS)
}

/// Only the proposing surface confirms. Chat: any chat line. Socket: the
/// line must carry the pendingId it got back. companion.send: never.
fn owns(p: &PendingWrite, origin: Origin, confirm_id: Option<&str>) -> bool {
    p.origin == origin
        && match origin {
            Origin::Chat => true,
            Origin::Socket => confirm_id == Some(p.id.as_str()),
            Origin::Companion => false,
        }
}

/// One line against the single pending slot. Stop clears it. The owner's
/// line takes it (so one yes executes once); another surface's line
/// leaves it alone. A new proposal replaces it.
#[allow(clippy::too_many_arguments)]
pub fn step(
    slot: &mut Option<PendingWrite>,
    line: &str,
    planets: &[PlanetRow],
    agents: &[crate::state::AgentShortcut],
    now: std::time::Instant,
    next_id: &str,
    origin: Origin,
    confirm_id: Option<&str>,
) -> Decision {
    if is_stop_word(line) {
        *slot = None;
        return Decision::Miss;
    }
    if slot.as_ref().is_some_and(|p| !fresh(p, now)) {
        *slot = None;
    }
    let pending = if slot
        .as_ref()
        .is_some_and(|p| owns(p, origin, confirm_id))
    {
        slot.take()
    } else {
        None
    };
    let d = decide(
        pending, line, planets, agents, now, next_id, origin, confirm_id,
    );
    if let Decision::Confirm { pending, .. } = &d {
        *slot = Some(pending.clone());
    }
    d
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

/// Longest real name that `text` starts with (word boundary).
fn longest_name_at(text: &str, names: &[&str]) -> Option<String> {
    let rest = text.trim_start().to_ascii_lowercase();
    let mut best: Option<&str> = None;
    for n in names {
        let n = n.trim();
        if n.len() < 2 {
            continue;
        }
        let nl = n.to_ascii_lowercase();
        if !rest.starts_with(&nl) {
            continue;
        }
        if rest[nl.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric())
        {
            continue;
        }
        if best.map(|b| n.len() > b.len()).unwrap_or(true) {
            best = Some(n);
        }
    }
    best.map(str::to_string)
}

fn marker_starts(text: &str, first: &[&str], all: &[&str]) -> Vec<usize> {
    let low = text.to_ascii_lowercase();
    let mut v = Vec::new();
    for m in first {
        if let Some(i) = low.find(m) {
            v.push(i + m.len());
        }
    }
    for m in all {
        v.extend(low.match_indices(m).map(|(i, _)| i + m.len()));
    }
    v
}

/// Multi-word planet and agent names: the longest real name after the
/// same markers `planet_token` / `agent_token` read wins, so "Kansas Lab"
/// is not "Kansas" and "with Kansas Agent" is not "Kansas".
fn refine(
    mut parsed: ParsedWrite,
    line: &str,
    planets: &[PlanetRow],
    agents: &[crate::state::AgentShortcut],
) -> ParsedWrite {
    let raw = line
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?'));
    let (quoted, without) = take_quote(raw);
    let planet_names: Vec<&str> = planets.iter().map(|p| p.name.as_str()).collect();
    let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    let planet = marker_starts(&without, &["planet "], &[" to ", " on ", " for "])
        .into_iter()
        .find_map(|i| longest_name_at(&without[i..], &planet_names));
    let agent = marker_starts(&without, &[], &["named ", "with ", "using "])
        .into_iter()
        .find_map(|i| longest_name_at(&without[i..], &agent_names));
    let mut changed = false;
    if let Some(p) = planet {
        if parsed.planet.as_deref() != Some(p.as_str()) {
            parsed.planet = Some(p);
            changed = true;
        }
    }
    if let Some(a) = agent {
        if parsed.agent.as_deref() != Some(a.as_str()) {
            parsed.agent = Some(a);
            changed = true;
        }
    }
    if changed && quoted.is_none() {
        let t = remainder_title(&without, parsed.planet.as_deref(), parsed.agent.as_deref());
        parsed.title = if t.is_empty() { None } else { Some(t) };
    }
    parsed
}

/// Pure confirm machine. A confirm does not touch `PersistedState`.
/// Timeout, "no", and another surface's reply return without
/// [`Decision::Execute`]. A planet counts as named only on an exact
/// case-insensitive name or id. Anything else, unless the line says the
/// word "planet", is a Miss and keeps its base path (chat or write lane).
#[allow(clippy::too_many_arguments)]
pub fn decide(
    pending: Option<PendingWrite>,
    line: &str,
    planets: &[PlanetRow],
    agents: &[crate::state::AgentShortcut],
    now: std::time::Instant,
    next_id: &str,
    origin: Origin,
    confirm_id: Option<&str>,
) -> Decision {
    let pending = pending.filter(|p| fresh(p, now) && owns(p, origin, confirm_id));
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
    if origin == Origin::Companion {
        return Decision::Miss;
    }
    let Some(parsed) = parse_board_write(line) else {
        return Decision::Miss;
    };
    let parsed = refine(parsed, line, planets, agents);
    let named = parsed.planet.as_deref().is_some_and(|t| {
        planets.iter().any(|p| {
            p.name.trim().eq_ignore_ascii_case(t.trim()) || p.id.eq_ignore_ascii_case(t.trim())
        })
    });
    if !named && !has_word(&line.to_ascii_lowercase(), "planet") {
        return Decision::Miss;
    }
    match resolve(&parsed, planets, agents) {
        Resolve::Ask { route, spoken } => Decision::Ask { route, spoken },
        Resolve::Ready(r) => {
            let spoken = confirm_line(&r);
            Decision::Confirm {
                pending: PendingWrite {
                    id: next_id.to_string(),
                    at: now,
                    origin,
                    resolved: r,
                },
                spoken,
            }
        }
    }
}

#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
pub fn state_hash(st: &crate::state::PersistedState) -> String {
    serde_json::to_string(&(&st.kanban_cards, &st.agent_runs)).unwrap_or_default()
}

/// The commit plan with the doors injected (tests see every call).
/// Always adds as todo (pickup is what moves a card to doing), assigns,
/// and picks up only for doing. An add error stops before assign.
#[cfg(any(unix, test))]
fn commit_with(
    r: &Resolved,
    add: impl FnOnce(&str, &str, &str) -> Result<String, String>,
    assign: impl FnOnce(&str, &str) -> Result<(), String>,
    pickup: impl FnOnce(&str, &str) -> Result<(), String>,
    log: impl FnOnce(&str),
) -> String {
    let card_id = match add(&r.title, &r.planet_id, "todo") {
        Ok(id) => id,
        Err(e) => return e,
    };
    if let Err(e) = assign(&card_id, &r.agent_id) {
        return format!(
            "Added {} on {} but assign failed: {e}",
            card_id, r.planet_name
        );
    }
    if r.bucket == "doing" {
        if let Err(e) = pickup(&r.agent_id, &card_id) {
            log(&card_id);
            return format!(
                "Added {} on {} but pickup failed: {e}",
                card_id, r.planet_name
            );
        }
    }
    log(&card_id);
    format!("Added {} on {}.", card_id, r.planet_name)
}

/// Live door. Adds as todo, assigns, and pickups only for doing.
/// Each door saves through `state::update`. No write happens before yes.
#[cfg(unix)]
fn commit_board(app: &tauri::AppHandle, r: &Resolved) -> String {
    commit_with(
        r,
        |title, planet, status| {
            crate::board::kanban_add(app, title, planet, status, None, Vec::new()).map(|c| c.id)
        },
        |card, agent| crate::agents::kanban_assign(app, card, agent).map(|_| ()),
        |agent, card| {
            crate::agents::agent_pickup(app, Some(agent), None, Some(card)).map(|_| ())
        },
        |card| log_board_add(app, r, card),
    )
}

/// Log payload: planet, bucket, agent id, card id. Never the title or line.
#[cfg(any(unix, test))]
fn board_add_payload(r: &Resolved, card_id: &str) -> serde_json::Value {
    json!({
        "planet": r.planet_name,
        "bucket": r.bucket,
        "agent": r.agent_id,
        "card": card_id,
    })
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
        &board_add_payload(r, card_id),
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
        st.projects.push(crate::state::Project {
            id: "p-clinic".into(),
            name: "Clinic".into(),
            trackers: None,
            workspace_root: None,
        });
        // Matt-like names. Everyday words must not count as these planets
        // unless the token is the exact name or id. Fixture only.
        for (id, name) in [
            ("p-prints", "prints"),
            ("p-stars", "stars"),
            ("p-dreamachine", "dreamachine"),
            ("p-tanks", "tanks"),
            ("p-jungle", "Jungle"),
            ("p-work", "Work"),
        ] {
            st.projects.push(crate::state::Project {
                id: id.into(),
                name: name.into(),
                trackers: None,
                workspace_root: None,
            });
        }
        let lab = crate::state::AgentShortcut {
            id: "a-lab".into(),
            name: "Lab Agent".into(),
            harness: "cursor".into(),
            profile: String::new(),
            model: None,
            md_paths: Vec::new(),
            project_id: Some("p-lab".into()),
            extra: None,
            permissions: "read-only".into(),
        };
        st.agent_shortcuts.push(agent);
        st.agent_shortcuts.push(kansas);
        st.agent_shortcuts.push(lab);
        let planets = planet_rows(&st);
        let agents = st.agent_shortcuts.clone();
        (st, planets, agents)
    }

    #[test]
    fn no_and_timeout_leave_the_fixture_hash() {
        let (st, planets, agents) = fixture();
        let hash = state_hash(&st);
        let now = std::time::Instant::now();
        let d = chat_decide(
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

        let d = chat_decide(Some(pending.clone()), "no", &planets, &agents, now, "mba-2");
        assert!(matches!(d, Decision::Decline(_)));
        assert_eq!(state_hash(&st), hash, "no must not write");

        let later = now + std::time::Duration::from_millis(CONFIRM_MS + 1);
        let d = chat_decide(Some(pending), "yes", &planets, &agents, later, "mba-3");
        assert!(matches!(d, Decision::Miss), "timeout must not execute");
        assert_eq!(state_hash(&st), hash, "timeout must not write");
    }

    #[test]
    fn yes_applies_fixture_add_and_spawn_only_in_memory() {
        let (mut st, planets, agents) = fixture();
        let hash = state_hash(&st);
        let now = std::time::Instant::now();
        let d = chat_decide(
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
        let d = chat_decide(Some(pending), "yes", &planets, &agents, now, "mba-x");
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

        let d = chat_decide(
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
        let d = chat_decide(Some(pending), "do it", &planets, &agents, now, "mba-y");
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
        let d = chat_decide(
            None,
            "add a card milk to planet kan todo",
            &planets,
            &agents,
            std::time::Instant::now(),
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
        assert!(log_body.contains("board_add_payload("), "{log_body}");
        let apply = src.find("fn apply_confirmed(").unwrap();
        let apply_body = &src[apply..apply + src[apply..].find("\n}\n").unwrap()];
        assert!(!apply_body.contains("agent_pickup"));
        assert!(!apply_body.contains("launch_agent"));
        assert!(apply_body.contains("agent_runs"));
    }

    #[test]
    fn route_run_is_one_door_on_three_surfaces() {
        let control = include_str!("../agents/control.rs");
        assert!(control.contains("mouth_route::run_outcome"));
        assert!(control.contains("pendingId"));
        let ctl = include_str!("../bin/orbittyctl.rs");
        assert!(ctl.contains("[\"route\", \"run\"]"));
        assert!(ctl.contains("pendingId"));
        let mcp = include_str!("../bin/orbitty_mcp.rs");
        assert!(mcp.contains("name: \"route.run\""));
        assert!(mcp.contains("name: \"route.table\""));
        assert!(mcp.contains("pendingId"));
    }

    #[test]
    fn write_lane_still_reaches_the_route() {
        let src = concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
);
        let start = src.find("Board add / agent spawn").expect("comment");
        let slice = &src[start..start + 400];
        assert!(slice.contains("mouth_route::run"));
        assert!(!slice.contains("needs_write_lane"));
    }

    // --- verify 4df2f22 fixes (B1, N1–N7, surviving mutations) ---------

    #[allow(clippy::too_many_arguments)]
    fn chat_decide(
        pending: Option<PendingWrite>,
        line: &str,
        planets: &[PlanetRow],
        agents: &[crate::state::AgentShortcut],
        now: std::time::Instant,
        id: &str,
    ) -> Decision {
        decide(pending, line, planets, agents, now, id, Origin::Chat, None)
    }

    fn ms(n: u64) -> std::time::Duration {
        std::time::Duration::from_millis(n)
    }

    const ADD: &str = "add a card test-add to planet fixture todo";

    fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
        let at = src.find(sig).unwrap_or_else(|| panic!("{sig}"));
        &src[at..at + src[at..].find("\n}\n").expect("end")]
    }

    #[test]
    fn b1_lines_without_a_real_planet_miss() {
        let (_st, planets, agents) = fixture();
        let now = std::time::Instant::now();
        for line in [
            "add a todo buy milk",
            "please add a todo: call mom",
            "can you make a card game for my kids?",
            "add some cards to my deck",
            "make a todo app in rust",
            "put the card down",
            "I want to start an agent someday, doing laundry now",
            "add a todo to print the report",
            "make a card for star wars fans",
            "make a todo for dream journal",
            "make a card on tank battles",
            "put a card for jung quotes",
        ] {
            match chat_decide(None, line, &planets, &agents, now, "mba-b1") {
                Decision::Miss => {}
                Decision::Ask { spoken, .. } => {
                    for name in ["prints", "stars", "dreamachine", "tanks", "Jungle"] {
                        assert!(
                            !spoken.to_ascii_lowercase().contains(&name.to_ascii_lowercase()),
                            "{line} asked about {name}: {spoken}"
                        );
                    }
                    panic!("{line} asked: {spoken}");
                }
                other => panic!("{line}: {other:?}"),
            }
        }
        // Prefix without the word "planet" is not a named planet.
        let d = chat_decide(
            None,
            "add a card X to kan todo",
            &planets,
            &agents,
            now,
            "mba-kan",
        );
        assert!(matches!(d, Decision::Miss), "to kan todo: {d:?}");
        // Literal "planet" still asks when the token is only a prefix.
        match chat_decide(
            None,
            "add a card X to planet kan todo",
            &planets,
            &agents,
            now,
            "mba-x3",
        ) {
            Decision::Ask { spoken, .. } => {
                assert_eq!(spoken, "Which planet — Kansas or Kansas Lab?");
            }
            other => panic!("X3 ask, got {other:?}"),
        }
        // "Work" is an exact planet name, so this still asks.
        match chat_decide(
            None,
            "add a card to work on later",
            &planets,
            &agents,
            now,
            "mba-work",
        ) {
            Decision::Ask { spoken, .. } => assert!(spoken.contains("Work"), "{spoken}"),
            other => panic!("exact Work should ask, got {other:?}"),
        }
        // It keeps its base path: the write lane.
        assert!(crate::mouth_gist::needs_write_lane("add a todo buy milk"));
        // Matt's line still confirms.
        let d = chat_decide(
            None,
            "add an agent in doing to planet Kansas",
            &planets,
            &agents,
            now,
            "mba-k",
        );
        assert!(matches!(d, Decision::Confirm { .. }), "{d:?}");
        // Asks stay for: a literal "planet" with no name, an unknown named
        // planet, a missing title, a missing agent.
        for (line, want) in [
            ("add a card milk to planet", "Which planet?"),
            ("add a card milk to planet Mars todo", "No planet Mars."),
            ("add a card to planet Kansas", "What title?"),
            ("add a card milk to planet Clinic todo", "Which agent on Clinic?"),
        ] {
            match chat_decide(None, line, &planets, &agents, now, "mba-ask") {
                Decision::Ask { spoken, .. } => assert_eq!(spoken, want, "{line}"),
                other => panic!("{line}: ask, got {other:?}"),
            }
        }
    }

    #[test]
    fn miss_lines_stay_plain_chat() {
        // Prod Grok/auto: empty reply_lane with free mode off, and any
        // unknown lane, both become "grok" (effective_reply_lane /
        // normalize_reply_lane). That string hits hop_lane_for's catch-all.
        let settings = "grok";
        let (_st, planets, agents) = fixture();
        let now = std::time::Instant::now();
        for line in [
            "make a todo app in rust",
            "make a todo app in rust for fun",
            "Make a todo app in Rust.",
            "make me a todo list app",
            "can you make a card game for my kids?",
            "make a card game for my kids",
            "add some cards to my deck",
            "put the card down",
            "I want to start an agent someday, doing laundry now",
            "add an agent",
            "start an agent",
        ] {
            assert!(!crate::mouth_gist::needs_write_lane(line), "{line}");
            assert_eq!(
                crate::mouth_hop::hop_lane_for(settings, line, false, true),
                "local",
                "{line} local up"
            );
            assert_eq!(
                crate::mouth_hop::hop_lane_for(settings, line, false, false),
                "grok",
                "{line} local down"
            );
            let d = chat_decide(None, line, &planets, &agents, now, "mba-miss");
            assert!(matches!(d, Decision::Miss), "{line}: {d:?}");
        }
        assert!(crate::mouth_gist::needs_write_lane("add a todo buy milk"));
    }

    #[test]
    fn confirm_window_is_sixty_seconds_on_a_monotonic_clock() {
        assert_eq!(CONFIRM_MS, 60_000);
        let (_st, planets, agents) = fixture();
        let t0 = std::time::Instant::now();
        let Decision::Confirm { pending, .. } = chat_decide(None, ADD, &planets, &agents, t0, "t")
        else {
            panic!("confirm");
        };
        let d = chat_decide(
            Some(pending.clone()),
            "yes",
            &planets,
            &agents,
            t0 + ms(59_999),
            "x",
        );
        assert!(matches!(d, Decision::Execute(_)), "{d:?}");
        let d = chat_decide(Some(pending), "yes", &planets, &agents, t0 + ms(60_000), "x");
        assert!(matches!(d, Decision::Miss), "{d:?}");
        let src = include_str!("mouth_route.rs");
        assert!(src.contains("pub at: std::time::Instant,"));
    }

    #[test]
    fn one_yes_consumes_the_pending() {
        let (_st, planets, agents) = fixture();
        let t0 = std::time::Instant::now();
        let mut slot = None;
        let d = step(&mut slot, ADD, &planets, &agents, t0, "mba-1", Origin::Chat, None);
        assert!(matches!(d, Decision::Confirm { .. }), "{d:?}");
        assert!(slot.is_some());
        let d = step(&mut slot, "yes", &planets, &agents, t0 + ms(5), "mba-2", Origin::Chat, None);
        assert!(matches!(d, Decision::Execute(_)), "{d:?}");
        assert!(slot.is_none(), "yes must consume the pending");
        let d = step(&mut slot, "yes", &planets, &agents, t0 + ms(9), "mba-3", Origin::Chat, None);
        assert!(matches!(d, Decision::Miss), "second yes must not write twice: {d:?}");
        // The live door runs every line through `step` on the one slot.
        let body = fn_body(include_str!("mouth_route.rs"), "pub fn run_outcome(");
        assert!(body.contains("step(\n"), "{body}");
        assert!(body.contains("pending_slot().lock()"));
    }

    #[test]
    fn stop_clears_the_pending() {
        let (_st, planets, agents) = fixture();
        let t0 = std::time::Instant::now();
        for (stop, origin) in [
            ("stop", Origin::Chat),
            ("STOP.", Origin::Chat),
            ("stop", Origin::Socket),
        ] {
            let mut slot = None;
            let _ = step(&mut slot, ADD, &planets, &agents, t0, "mba-s", Origin::Chat, None);
            assert!(slot.is_some());
            let d = step(&mut slot, stop, &planets, &agents, t0, "mba-t", origin, None);
            assert!(matches!(d, Decision::Miss));
            assert!(slot.is_none(), "{stop} must clear the pending");
            let d = step(&mut slot, "yes", &planets, &agents, t0, "mba-u", Origin::Chat, None);
            assert!(matches!(d, Decision::Miss), "yes after stop: {d:?}");
        }
    }

    fn resolved(bucket: &'static str) -> Resolved {
        Resolved {
            route: if bucket == "doing" { "agent-spawn" } else { "board-add" },
            planet_id: "p-kansas".into(),
            planet_name: "Kansas".into(),
            bucket,
            title: "secret milk".into(),
            agent_id: "a-kansas".into(),
            agent_name: "Kansas Agent".into(),
        }
    }

    fn commit_calls(r: &Resolved, add_ok: bool) -> (String, Vec<String>) {
        let calls = std::cell::RefCell::new(Vec::<String>::new());
        let out = commit_with(
            r,
            |t, p, s| {
                calls.borrow_mut().push(format!("add {t}|{p}|{s}"));
                if add_ok {
                    Ok("c-1".into())
                } else {
                    Err("boom".into())
                }
            },
            |c, a| {
                calls.borrow_mut().push(format!("assign {c}|{a}"));
                Ok(())
            },
            |a, c| {
                calls.borrow_mut().push(format!("pickup {a}|{c}"));
                Ok(())
            },
            |c| calls.borrow_mut().push(format!("log {c}")),
        );
        (out, calls.into_inner())
    }

    #[test]
    fn commit_picks_up_only_on_doing_and_never_adds_as_doing() {
        let (out, calls) = commit_calls(&resolved("todo"), true);
        assert_eq!(
            calls,
            ["add secret milk|p-kansas|todo", "assign c-1|a-kansas", "log c-1"],
            "todo add: no pickup"
        );
        assert_eq!(out, "Added c-1 on Kansas.");
        let (_, calls) = commit_calls(&resolved("doing"), true);
        assert_eq!(
            calls,
            [
                "add secret milk|p-kansas|todo",
                "assign c-1|a-kansas",
                "pickup a-kansas|c-1",
                "log c-1"
            ],
            "doing: added as todo, then pickup with that card id"
        );
        let (out, calls) = commit_calls(&resolved("doing"), false);
        assert_eq!(out, "boom");
        assert_eq!(calls, ["add secret milk|p-kansas|todo"], "add error: no assign");
        // The live door is this plan.
        let body = fn_body(include_str!("mouth_route.rs"), "fn commit_board(");
        assert!(body.contains("commit_with("), "{body}");
    }

    #[test]
    fn board_add_log_has_no_title_or_line() {
        let r = resolved("doing");
        let v = board_add_payload(&r, "c-9");
        let mut keys: Vec<String> = v.as_object().expect("obj").keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["agent", "bucket", "card", "planet"]);
        assert!(!v.to_string().contains("secret milk"), "{v}");
        let body = fn_body(include_str!("mouth_route.rs"), "fn log_board_add(");
        assert!(body.contains("&board_add_payload(r, card_id)"), "{body}");
    }

    #[test]
    fn longest_planet_and_agent_names_win() {
        let (_st, planets, agents) = fixture();
        let now = std::time::Instant::now();
        let d = chat_decide(
            None,
            "add a card Fix login for Kansas Lab todo",
            &planets,
            &agents,
            now,
            "mba-lab",
        );
        let Decision::Confirm { pending, spoken } = d else {
            panic!("confirm lab: {d:?}");
        };
        assert_eq!(pending.resolved.planet_id, "p-lab");
        assert_eq!(pending.resolved.title, "Fix login");
        assert_eq!(spoken, "Add 'Fix login' to Kansas Lab todo with Lab Agent? (yes / no)");
        let d = chat_decide(
            None,
            "add a card Fix login to planet Kansas Lab with Kansas Agent",
            &planets,
            &agents,
            now,
            "mba-lab2",
        );
        let Decision::Confirm { pending, .. } = d else {
            panic!("confirm named agent: {d:?}");
        };
        assert_eq!(pending.resolved.planet_id, "p-lab");
        assert_eq!(pending.resolved.agent_id, "a-kansas");
        assert_eq!(pending.resolved.title, "Fix login");
        let d = chat_decide(
            None,
            "add a card Fix login to planet Kansas todo",
            &planets,
            &agents,
            now,
            "mba-k",
        );
        let Decision::Confirm { pending, .. } = d else {
            panic!("confirm kansas: {d:?}");
        };
        assert_eq!(pending.resolved.planet_id, "p-kansas");
    }

    #[test]
    fn confirm_is_bound_to_its_surface_and_pending_id() {
        let (_st, planets, agents) = fixture();
        let t0 = std::time::Instant::now();
        let mut slot = None;
        let sock = |slot: &mut Option<PendingWrite>, line: &str, id: Option<&str>| {
            step(slot, line, &planets, &agents, t0, "mba-new", Origin::Socket, id)
        };
        // MCP / ctl proposes.
        let d = sock(&mut slot, ADD, None);
        let Decision::Confirm { pending, .. } = d else {
            panic!("socket confirm: {d:?}");
        };
        assert_eq!(pending.origin, Origin::Socket);
        let pid = pending.id.clone();
        // Matt's unrelated chat "ok" does not confirm it and leaves it.
        let d = step(&mut slot, "ok", &planets, &agents, t0, "x", Origin::Chat, None);
        assert!(matches!(d, Decision::Miss), "{d:?}");
        assert!(slot.is_some());
        // companion.send cannot confirm it.
        let d = step(&mut slot, "yes", &planets, &agents, t0, "x", Origin::Companion, None);
        assert!(matches!(d, Decision::Miss), "{d:?}");
        // A socket yes without the id, or with the wrong id, does nothing.
        assert!(matches!(sock(&mut slot, "yes", None), Decision::Miss));
        assert!(matches!(sock(&mut slot, "yes", Some("mba-wrong")), Decision::Miss));
        assert!(slot.is_some());
        // The matching pendingId confirms, once.
        assert!(matches!(sock(&mut slot, "yes", Some(pid.as_str())), Decision::Execute(_)));
        assert!(slot.is_none());

        // A chat pending: a socket yes (even with its id) and companion.send
        // cannot confirm it. Chat can.
        let d = step(&mut slot, ADD, &planets, &agents, t0, "mba-chat", Origin::Chat, None);
        assert!(matches!(d, Decision::Confirm { .. }));
        assert!(matches!(sock(&mut slot, "yes", Some("mba-chat")), Decision::Miss));
        let d = step(&mut slot, "yes", &planets, &agents, t0, "x", Origin::Companion, None);
        assert!(matches!(d, Decision::Miss));
        let d = step(&mut slot, "yes", &planets, &agents, t0, "x", Origin::Chat, None);
        assert!(matches!(d, Decision::Execute(_)), "{d:?}");

        // Two companion.send calls never write: the add proposes nothing.
        let d = step(&mut slot, ADD, &planets, &agents, t0, "x", Origin::Companion, None);
        assert!(matches!(d, Decision::Miss), "{d:?}");
        assert!(slot.is_none());
        let d = step(&mut slot, "yes", &planets, &agents, t0, "x", Origin::Companion, None);
        assert!(matches!(d, Decision::Miss));

        // Wiring: each surface names its origin; the socket passes pendingId.
        let control = include_str!("../agents/control.rs");
        let send = &control[control.find("\"companion.send\" =>").unwrap()..];
        let send = &send[..send.find("\"companion.agents\"").unwrap()];
        assert!(
            send.contains(
                "arg_opt(args, \"path\"),\n                crate::mouth_route::Origin::Companion,"
            ),
            "{send}"
        );
        let run = &control[control.find("\"route.run\" =>").unwrap()..];
        let run = &run[..run.find("\"gate.inject\"").unwrap()];
        assert!(run.contains("Origin::Socket"), "{run}");
        assert!(run.contains("arg_opt(args, \"pendingId\")"), "{run}");
        assert!(run.contains("confirm.as_deref(),"), "{run}");
        let coord = concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
);
        let cmd = fn_body(coord, "pub async fn companion_send(");
        assert!(cmd.contains("Origin::Chat"), "{cmd}");
        assert!(coord.contains("mouth_route::run(&app, &line, origin)"));
    }

    #[test]
    fn pending_ids_are_random_and_distinct() {
        let a = new_pending_id();
        let b = new_pending_id();
        assert_ne!(a, b);
        assert!(a.starts_with("mba-") && a.len() == 36, "{a}");
        assert!(a[4..].bytes().all(|c| c.is_ascii_hexdigit()), "{a}");
    }
}
