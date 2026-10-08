use serde_json::json;
use super::CONFIRM_MS;
use super::Origin;
use super::is_stop_word;

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
pub(crate) fn new_pending_id() -> String {
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

pub(crate) fn pending_slot() -> &'static std::sync::Mutex<Option<PendingWrite>> {
    static PENDING: std::sync::Mutex<Option<PendingWrite>> = std::sync::Mutex::new(None);
    &PENDING
}

pub(crate) fn fresh(p: &PendingWrite, now: std::time::Instant) -> bool {
    now.saturating_duration_since(p.at) < std::time::Duration::from_millis(CONFIRM_MS)
}

/// Only the proposing surface confirms. Chat: any chat line. Socket: the
/// line must carry the pendingId it got back. companion.send: never.
pub(crate) fn owns(p: &PendingWrite, origin: Origin, confirm_id: Option<&str>) -> bool {
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

pub(crate) fn planet_rows(st: &crate::state::PersistedState) -> Vec<PlanetRow> {
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
pub fn blank_card(id: &str, r: &Resolved, status: &str) -> crate::state::KanbanCard {
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
pub(crate) fn commit_with(
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
pub(crate) fn commit_board(app: &tauri::AppHandle, r: &Resolved) -> String {
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
pub(crate) fn board_add_payload(r: &Resolved, card_id: &str) -> serde_json::Value {
    json!({
        "planet": r.planet_name,
        "bucket": r.bucket,
        "agent": r.agent_id,
        "card": card_id,
    })
}

#[cfg(unix)]
pub(crate) fn log_board_add(app: &tauri::AppHandle, r: &Resolved, card_id: &str) {
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

pub(crate) fn commit(app: &tauri::AppHandle, r: &Resolved) -> String {
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

