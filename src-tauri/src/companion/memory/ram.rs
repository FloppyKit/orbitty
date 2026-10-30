//! RAM core. Pure over `RamCtx`. Rules and soul are never cut.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{read_notes_head, NOTES_RAM_CHARS};
use crate::state::CompanionTurn;

pub(crate) const RAM_CAP: usize = 6_000;
pub(crate) const TURN_N: usize = 6;
pub(crate) const PINNED_MAX: usize = 12;
pub(crate) const ANSWERED_TTL_SECS: f64 = 24.0 * 60.0 * 60.0;
pub(crate) const RECALL_LAW: &str =
    "A past decision needs companion.recall in this turn and a Checked: line (terms, dates, hit count).";

#[derive(Clone, Debug)]
pub(crate) struct CardIn {
    pub title: String,
    pub open: bool,
    pub active: bool,
    pub touched: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct AnsweredAsk {
    pub id: String,
    pub ask: String,
    pub at: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct RamCtx {
    pub rules: String,
    pub soul: String,
    pub now: String,
    pub planet_line: Option<String>,
    pub notes: Option<String>,
    pub cards: Vec<CardIn>,
    pub pinned_tags: Vec<String>,
    pub open_asks: Vec<(String, String)>,
    pub answered: Vec<AnsweredAsk>,
    pub turns: Vec<CompanionTurn>,
    pub now_secs: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct RamCore {
    pub rules: String,
    pub soul: String,
    pub now: String,
    pub planet: Option<String>,
    pub card: Option<String>,
    pub pinned_tags: Vec<String>,
    pub open_asks: Vec<(String, String)>,
    pub turns: Vec<CompanionTurn>,
}

pub(crate) fn build(ctx: &RamCtx) -> RamCore {
    let mut planet = ctx.planet_line.clone().filter(|s| !s.trim().is_empty());
    if let (Some(line), Some(notes)) = (&planet, ctx.notes.as_deref()) {
        let head = cap_chars(notes, NOTES_RAM_CHARS);
        if !head.is_empty() {
            planet = Some(format!("{line}\n{head}"));
        }
    }
    let card = ctx
        .cards
        .iter()
        .filter(|c| c.open && c.active && !c.title.trim().is_empty())
        .max_by(|a, b| a.touched.partial_cmp(&b.touched).unwrap_or(std::cmp::Ordering::Equal))
        .map(|c| c.title.trim().to_string());
    let mut pinned = Vec::new();
    for tag in &ctx.pinned_tags {
        let tag = tag.trim();
        if tag.is_empty() || pinned.iter().any(|t: &String| t == tag) {
            continue;
        }
        pinned.push(tag.to_string());
        if pinned.len() == PINNED_MAX {
            break;
        }
    }
    let mut open_asks = Vec::new();
    for (id, ask) in &ctx.open_asks {
        let ask = ask.trim();
        if id.trim().is_empty() || ask.is_empty() {
            continue;
        }
        open_asks.push((id.trim().to_string(), ask.to_string()));
    }
    for row in &ctx.answered {
        let age = ctx.now_secs - row.at;
        if age >= ANSWERED_TTL_SECS {
            continue;
        }
        let ask = row.ask.trim();
        if row.id.trim().is_empty() || ask.is_empty() {
            continue;
        }
        open_asks.push((row.id.trim().to_string(), ask.to_string()));
    }
    let start = ctx.turns.len().saturating_sub(TURN_N);
    RamCore {
        rules: ctx.rules.clone(),
        soul: ctx.soul.clone(),
        now: ctx.now.clone(),
        planet,
        card,
        pinned_tags: pinned,
        open_asks,
        turns: ctx.turns[start..].to_vec(),
    }
}

/// Drop volatile fields until the render fits. Rules and soul stay whole.
pub(crate) fn fit(core: &RamCore) -> RamCore {
    let mut turns = core.turns.clone();
    let mut asks = core.open_asks.clone();
    let mut tags = core.pinned_tags.clone();
    let mut card = core.card.clone();
    let mut planet = core.planet.clone();
    loop {
        let trial = RamCore {
            rules: core.rules.clone(),
            soul: core.soul.clone(),
            now: core.now.clone(),
            planet: planet.clone(),
            card: card.clone(),
            pinned_tags: tags.clone(),
            open_asks: asks.clone(),
            turns: turns.clone(),
        };
        if render_of(&trial).chars().count() <= RAM_CAP {
            return trial;
        }
        if !turns.is_empty() {
            turns.remove(0);
            continue;
        }
        if !asks.is_empty() {
            asks.remove(0);
            continue;
        }
        if !tags.is_empty() {
            tags.remove(0);
            continue;
        }
        if card.is_some() {
            card = None;
            continue;
        }
        if planet.is_some() {
            planet = None;
            continue;
        }
        return trial;
    }
}

pub(crate) fn render(core: &RamCore) -> String {
    render_of(&fit(core))
}

fn render_of(core: &RamCore) -> String {
    let mut out = String::new();
    push_block(&mut out, "rules", &core.rules);
    push_block(&mut out, "soul", &core.soul);
    push_block(&mut out, "recall", RECALL_LAW);
    push_block(&mut out, "now", &core.now);
    if let Some(planet) = &core.planet {
        push_block(&mut out, "planet", planet);
    }
    if let Some(card) = &core.card {
        push_block(&mut out, "card", card);
    }
    if !core.pinned_tags.is_empty() {
        push_block(&mut out, "tags", &core.pinned_tags.join("\n"));
    }
    if !core.open_asks.is_empty() {
        let lines: Vec<String> = core
            .open_asks
            .iter()
            .map(|(id, ask)| format!("{id} {ask}"))
            .collect();
        push_block(&mut out, "asks", &lines.join("\n"));
    }
    if !core.turns.is_empty() {
        let lines: Vec<String> = core
            .turns
            .iter()
            .map(|t| format!("{}: {}", t.role, t.text))
            .collect();
        push_block(&mut out, "turns", &lines.join("\n"));
    }
    out
}

fn push_block(out: &mut String, name: &str, body: &str) {
    out.push('[');
    out.push_str(name);
    out.push_str("]\n");
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
}

fn cap_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect()
    }
}

pub(crate) fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub(crate) fn load_rules(dir: &Path) -> String {
    let mode = std::fs::read_to_string(dir.join("MODE_LAW.md")).unwrap_or_default();
    let thin = std::fs::read_to_string(dir.join("THIN_LAW.md")).unwrap_or_default();
    match (mode.is_empty(), thin.is_empty()) {
        (true, true) => String::new(),
        (false, true) => mode,
        (true, false) => thin,
        (false, false) => format!("{mode}\n{thin}"),
    }
}

pub(crate) fn core_for_app(app: &tauri::AppHandle) -> RamCore {
    let state = crate::state::load_state(app).ok();
    let planet_id = state.as_ref().and_then(|s| {
        s.last_claim_project_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    let app_data = crate::coordinator::app_data(app).ok();
    let notes = match (&app_data, &planet_id) {
        (Some(dir), Some(id)) => {
            let head = read_notes_head(dir, id);
            if head.is_empty() {
                None
            } else {
                Some(head)
            }
        }
        _ => None,
    };
    let planet_line = match (&state, &planet_id) {
        (Some(st), Some(id)) => {
            let name = st
                .projects
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            if name.is_empty() {
                Some(id.clone())
            } else {
                Some(format!("{id} {name}"))
            }
        }
        _ => None,
    };
    let (cards, open_asks, answered) = match &state {
        Some(st) => board_parts(st, planet_id.as_deref()),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let turns = state
        .as_ref()
        .map(|s| s.companion.turns.clone())
        .unwrap_or_default();
    let pinned = super::tags::pinned_names(app_data.as_deref(), planet_id.as_deref());
    build(&RamCtx {
        rules: load_rules(&crate::coordinator::companions_root()),
        soul: crate::coordinator::companion_soul_text(app),
        now: crate::coordinator::now_block(),
        planet_line,
        notes,
        cards,
        pinned_tags: pinned,
        open_asks,
        answered,
        turns,
        now_secs: now_secs(),
    })
}

fn board_parts(
    state: &crate::state::PersistedState,
    planet: Option<&str>,
) -> (Vec<CardIn>, Vec<(String, String)>, Vec<AnsweredAsk>) {
    let on_planet = |project: &str| planet.map(|p| project == p).unwrap_or(true);
    let mut cards = Vec::new();
    let mut open_asks = Vec::new();
    let mut answered = Vec::new();
    let mut newest_open: Option<(usize, f64)> = None;
    for c in &state.kanban_cards {
        if !on_planet(&c.project_id) {
            continue;
        }
        if c.kind.as_deref() == Some("needed") {
            match c.needed_state.as_deref() {
                Some("open") => open_asks.push((c.id.clone(), c.title.clone())),
                Some("answered") => answered.push(AnsweredAsk {
                    id: c.id.clone(),
                    ask: c.title.clone(),
                    at: c.last_touched_at,
                }),
                _ => {}
            }
            continue;
        }
        let open = c.status != "done";
        let active = open && c.status == "doing";
        if open && !active && newest_open.map(|(_, t)| c.last_touched_at >= t).unwrap_or(true) {
            newest_open = Some((cards.len(), c.last_touched_at));
        }
        cards.push(CardIn {
            title: c.title.clone(),
            open,
            active,
            touched: c.last_touched_at,
        });
    }
    if !cards.iter().any(|c| c.open && c.active) {
        if let Some((idx, _)) = newest_open {
            cards[idx].active = true;
        }
    }
    (cards, open_asks, answered)
}
