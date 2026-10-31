//! RAM core. Pure over `RamCtx`. Rules, user rules, soul, and pins are never cut.
//! The trim path is tokens on the trimmable section only (not `RAM_CAP`).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{read_notes_head, NOTES_RAM_CHARS};
use crate::state::CompanionTurn;

/// 21p char floor. Not read by `fit`. Token budget replaced it on the trim path.
#[allow(dead_code)]
pub(crate) const RAM_CAP: usize = 6_000;
pub(crate) const TURN_N: usize = 6;
pub(crate) const PINNED_MAX: usize = 12;
pub(crate) const ANSWERED_TTL_TURNS: usize = 6;
pub(crate) const RECALL_LAW: &str =
    "A past decision needs companion.recall in this turn and a Checked: line (terms, dates, hit count).";
const RULE_LAW: &str =
    "Rules and pins stay until the user changes them. Propose a change with companion.rule_propose; do not edit a rule yourself.";

/// Prefix, context window. First match wins. S0 ids: `reply_model` /
/// `direct_model`, pinned local `google/gemma-4-e4b`, cursor `grok-4.7-high`,
/// OpenRouter `openai/gpt-4o-mini`. Empty or `?` is unknown (32_000).
const CTX_TABLE: &[(&str, usize)] = &[
    ("grok-4", 256_000),
    ("claude", 200_000),
    ("gpt-5", 400_000),
    ("gpt-4.1", 1_000_000),
    ("gpt-4o", 128_000),
    ("openai/gpt-", 128_000),
    ("gpt-", 128_000),
    ("google/gemma-4-e4b", 8_192),
    ("google/gemma", 8_192),
    ("gemma", 8_192),
    ("ollama", 8_192),
    ("lmstudio", 32_768),
    ("goose", 32_000),
    ("hermes", 32_000),
    ("local", 8_192),
];

#[derive(Clone, Debug)]
pub(crate) struct CardIn {
    pub id: String,
    pub title: String,
    pub open: bool,
    pub active: bool,
    pub touched: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct AnsweredAsk {
    pub id: String,
    pub ask: String,
    /// Card timestamp. Hours are not the age key; turns_since is.
    #[allow(dead_code)]
    pub at: f64,
    pub turns_since: usize,
    pub answer_id: String,
    pub answer: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RamCtx {
    pub rules: String,
    pub user_rules: String,
    pub soul: String,
    pub now: String,
    pub model: String,
    pub planet_line: Option<String>,
    pub notes: Option<String>,
    pub cards: Vec<CardIn>,
    pub card_id: Option<String>,
    pub pinned_tags: Vec<String>,
    pub open_asks: Vec<(String, String)>,
    pub answered: Vec<AnsweredAsk>,
    pub turns: Vec<CompanionTurn>,
    /// Kept so callers can pass the card clock. Aging does not read it.
    #[allow(dead_code)]
    pub now_secs: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct RamCore {
    pub rules: String,
    pub user_rules: String,
    pub soul: String,
    pub now: String,
    pub model: String,
    pub planet: Option<String>,
    pub card: Option<String>,
    pub card_id: Option<String>,
    pub pinned_tags: Vec<String>,
    pub open_asks: Vec<(String, String)>,
    pub more_asks: Vec<String>,
    pub answers: Vec<(String, String, String)>,
    pub aged: Vec<(String, String)>,
    pub turns: Vec<CompanionTurn>,
}

/// `ceil(bytes / 4)`. Conservative. No tokenizer.
pub(crate) fn est_tokens(s: &str) -> usize {
    let n = s.len();
    if n == 0 {
        0
    } else {
        n.div_ceil(4)
    }
}

pub(crate) fn ctx_window(model: &str) -> usize {
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() || m == "?" {
        return 32_000;
    }
    for (prefix, n) in CTX_TABLE {
        if m.starts_with(prefix) || m.contains(prefix) {
            return *n;
        }
    }
    32_000
}

pub(crate) fn budget_for_window(ctx: usize) -> usize {
    let raw = ctx.saturating_mul(8) / 100;
    raw.clamp(1_500, 12_000)
}

pub(crate) fn budget_tokens(model: &str) -> usize {
    budget_for_window(ctx_window(model))
}

pub(crate) fn build(ctx: &RamCtx) -> RamCore {
    let mut planet = ctx.planet_line.clone().filter(|s| !s.trim().is_empty());
    if let (Some(line), Some(notes)) = (&planet, ctx.notes.as_deref()) {
        let head = cap_chars(notes, NOTES_RAM_CHARS);
        if !head.is_empty() {
            planet = Some(format!("{line}\n{head}"));
        }
    }
    let picked = ctx
        .cards
        .iter()
        .filter(|c| c.open && c.active && !c.title.trim().is_empty())
        .max_by(|a, b| a.touched.partial_cmp(&b.touched).unwrap_or(std::cmp::Ordering::Equal));
    let card = picked.map(|c| c.title.trim().to_string());
    let card_id = ctx.card_id.clone().or_else(|| {
        picked
            .map(|c| c.id.trim().to_string())
            .filter(|s| !s.is_empty())
    });
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
    let mut answers = Vec::new();
    let mut aged = Vec::new();
    for row in &ctx.answered {
        let ask = row.ask.trim();
        if row.id.trim().is_empty() || ask.is_empty() {
            continue;
        }
        if row.turns_since > ANSWERED_TTL_TURNS {
            if !row.answer_id.trim().is_empty() {
                aged.push((row.id.trim().to_string(), row.answer_id.trim().to_string()));
            }
            continue;
        }
        if !row.answer.trim().is_empty() {
            answers.push((
                row.id.trim().to_string(),
                row.answer_id.trim().to_string(),
                row.answer.trim().to_string(),
            ));
        } else {
            open_asks.push((row.id.trim().to_string(), ask.to_string()));
        }
    }
    let start = ctx.turns.len().saturating_sub(TURN_N);
    RamCore {
        rules: ctx.rules.clone(),
        user_rules: ctx.user_rules.clone(),
        soul: ctx.soul.clone(),
        now: ctx.now.clone(),
        model: ctx.model.clone(),
        planet,
        card,
        card_id,
        pinned_tags: pinned,
        open_asks,
        more_asks: Vec::new(),
        answers,
        aged,
        turns: ctx.turns[start..].to_vec(),
    }
}

/// Drop trimmable fields until that section fits the token budget.
/// Rules, user rules, soul, and pins are not in the section.
pub(crate) fn fit(core: &RamCore) -> RamCore {
    fit_to(core, budget_tokens(&core.model))
}

pub(crate) fn fit_to(core: &RamCore, budget: usize) -> RamCore {
    let mut turns = core.turns.clone();
    let mut asks = core.open_asks.clone();
    let mut answers = core.answers.clone();
    let mut aged = core.aged.clone();
    let mut more = core.more_asks.clone();
    let mut card = core.card.clone();
    let mut planet = core.planet.clone();
    loop {
        let trial = trial_of(core, &planet, &card, &asks, &more, &answers, &aged, &turns);
        if est_tokens(&trimmable_of(&trial)) <= budget {
            return trial;
        }
        if !aged.is_empty() {
            aged.remove(0);
            continue;
        }
        if !turns.is_empty() {
            turns.remove(0);
            continue;
        }
        if !answers.is_empty() {
            let (id, answer_id, _) = answers.remove(0);
            if !answer_id.is_empty() {
                aged.push((id, answer_id));
            }
            continue;
        }
        if !asks.is_empty() {
            let (id, _) = asks.remove(0);
            more.push(id);
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
        if more.len() > 1 {
            more.remove(0);
            continue;
        }
        if more.len() == 1 {
            more.clear();
            continue;
        }
        return trial;
    }
}

fn trial_of(
    core: &RamCore,
    planet: &Option<String>,
    card: &Option<String>,
    asks: &[(String, String)],
    more: &[String],
    answers: &[(String, String, String)],
    aged: &[(String, String)],
    turns: &[CompanionTurn],
) -> RamCore {
    RamCore {
        rules: core.rules.clone(),
        user_rules: core.user_rules.clone(),
        soul: core.soul.clone(),
        now: core.now.clone(),
        model: core.model.clone(),
        planet: planet.clone(),
        card: card.clone(),
        card_id: core.card_id.clone(),
        pinned_tags: core.pinned_tags.clone(),
        open_asks: asks.to_vec(),
        more_asks: more.to_vec(),
        answers: answers.to_vec(),
        aged: aged.to_vec(),
        turns: turns.to_vec(),
    }
}

pub(crate) fn render(core: &RamCore) -> String {
    render_of(&fit(core))
}

#[cfg(test)]
pub(crate) fn render_with_budget(core: &RamCore, budget: usize) -> String {
    render_of(&fit_to(core, budget))
}

#[cfg(test)]
pub(crate) fn trimmable_text(core: &RamCore) -> String {
    trimmable_of(core)
}

fn render_of(core: &RamCore) -> String {
    let mut out = String::new();
    push_block(&mut out, "rules", &core.rules);
    if !core.user_rules.trim().is_empty() {
        push_block(&mut out, "user-rules", &core.user_rules);
    }
    push_block(&mut out, "soul", &core.soul);
    push_block(&mut out, "recall", RECALL_LAW);
    push_block(&mut out, "rule-law", RULE_LAW);
    push_block(&mut out, "now", &core.now);
    if !core.pinned_tags.is_empty() {
        push_block(&mut out, "pins", &core.pinned_tags.join("\n"));
    }
    out.push_str(&trimmable_of(core));
    out
}

fn trimmable_of(core: &RamCore) -> String {
    let mut out = String::new();
    if let Some(planet) = &core.planet {
        push_block(&mut out, "planet", planet);
    }
    if let Some(card) = &core.card {
        push_block(&mut out, "card", card);
    }
    if !core.open_asks.is_empty() || !core.more_asks.is_empty() {
        let mut lines: Vec<String> = core
            .open_asks
            .iter()
            .map(|(id, ask)| format!("{id} {ask}"))
            .collect();
        if !core.more_asks.is_empty() {
            lines.push(format!("more asks: {}", core.more_asks.join(", ")));
        }
        push_block(&mut out, "asks", &lines.join("\n"));
    }
    if !core.answers.is_empty() {
        let lines: Vec<String> = core
            .answers
            .iter()
            .map(|(id, answer_id, text)| format!("{id} {answer_id} {text}"))
            .collect();
        push_block(&mut out, "answers", &lines.join("\n"));
    }
    if !core.turns.is_empty() {
        let lines: Vec<String> = core
            .turns
            .iter()
            .map(|t| format!("{}: {}", t.role, t.text))
            .collect();
        push_block(&mut out, "turns", &lines.join("\n"));
    }
    if !core.aged.is_empty() {
        let lines: Vec<String> = core
            .aged
            .iter()
            .map(|(ask, answer)| format!("aged: {ask} -> {answer}"))
            .collect();
        push_block(&mut out, "aged", &lines.join("\n"));
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

pub(crate) fn model_of(state: Option<&crate::state::PersistedState>) -> String {
    let Some(st) = state else {
        return String::new();
    };
    let reply = st.companion.reply_model.trim();
    if !reply.is_empty() && reply != "?" {
        return reply.to_string();
    }
    let direct = st.companion.direct_model.trim();
    if !direct.is_empty() && direct != "?" {
        direct.to_string()
    } else {
        String::new()
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
    let card_id = cards
        .iter()
        .filter(|c| c.open && c.active)
        .max_by(|a, b| a.touched.partial_cmp(&b.touched).unwrap_or(std::cmp::Ordering::Equal))
        .map(|c| c.id.clone());
    let turns = state
        .as_ref()
        .map(|s| s.companion.turns.clone())
        .unwrap_or_default();
    let pinned = super::tags::pinned_names(app_data.as_deref(), planet_id.as_deref());
    let user_rules = app_data
        .as_deref()
        .map(super::user_rules_for_handoff)
        .unwrap_or_default();
    let ctx = RamCtx {
        rules: load_rules(&crate::coordinator::companions_root()),
        user_rules,
        soul: crate::coordinator::companion_soul_text(app),
        now: crate::coordinator::now_block(),
        model: model_of(state.as_ref()),
        planet_line,
        notes,
        cards,
        card_id,
        pinned_tags: pinned,
        open_asks,
        answered,
        turns,
        now_secs: now_secs(),
    };
    let archive = crate::mouth_archive::archive_root()
        .unwrap_or_else(crate::coordinator::companions_root);
    match app_data.as_deref() {
        Some(dir) => super::settle(dir, &archive, &ctx),
        None => build(&ctx),
    }
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
                    turns_since: 0,
                    answer_id: String::new(),
                    answer: String::new(),
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
            id: c.id.clone(),
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
