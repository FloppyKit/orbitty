//! Alpha mini story. Rust-owned `starterStory` plus a pure planner.
//!
//! Executors re-check their flag inside `state::update` (the write lock is
//! not reentrant, so they never run on the save path or the AppKit thread).
//! Habitat reads outpost bits from an atomic; it does no state I/O.

use std::sync::atomic::{AtomicU8, Ordering};

use tauri::Emitter;

use crate::state::{PersistedState, StarterStory};

pub const ID_COMMS: &str = "c-starter-comms";
pub const ID_STARS: &str = "c-starter-stars";
pub const ID_BOARD: &str = "c-starter-board";
pub const ID_CLAIM: &str = "c-starter-claim";

const STARTER_IDS: [&str; 4] = [ID_COMMS, ID_STARS, ID_BOARD, ID_CLAIM];

const TITLE_COMMS: &str = "Build the Comms outpost";
const TITLE_STARS: &str = "Build the Stars outpost";
const TITLE_BOARD: &str = "Move this card to Doing, then Done";
const TITLE_CLAIM: &str = "Put another window on this planet";

const NOTES_COMMS: &str = "Click the empty plot at the east edge of this planet. Your walker walks over and builds it.\nComms is where your companion lives. It says hi once Comms is up.";
const NOTES_STARS: &str = "Click the empty plot at the south edge of the grass. Your walker builds it.\nStars is your daily check-in and journal. It opens when the outpost is up.";
const NOTES_BOARD: &str = "Every todo grows a tree on its planet. Click the planet's sign to open the board and drag this card to Doing, then Done.\nDone harvests the tree and earns a point.";
const NOTES_CLAIM: &str = "Each claimed window is a walker. Click a window, then Claim in the rail (or drag it onto the planet).\nDouble-click a walker to bring its window back.";

const INTRO_LINES: [&str; 3] = [
    "Hi, I'm your companion. Comms is up, so this is where we talk.",
    "Type to me here any time.",
    "Next on your board: build the Stars outpost. Stars is where you check in on your day.",
];

/// Low 2 bits = Comms, next 2 bits = Stars. 0 Hidden, 1 Plot, 2 Built.
static OUTPOSTS: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outpost {
    Comms,
    Stars,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutpostVisual {
    Hidden = 0,
    Plot = 1,
    Built = 2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Seed(String),
    MarkBuilt(Outpost),
    Complete(String),
    Intro,
}

#[derive(Clone, Debug)]
pub struct StarterView {
    pub story: StarterStory,
    pub claim_project_id: Option<String>,
    pub comms_status: Option<String>,
    pub stars_status: Option<String>,
    pub starter_id_present: bool,
}

/// Boot arm. `v == 0` becomes 1. Any claim, project, or card is a veteran
/// (story off, both outposts built). A fresh profile hides the companion
/// so beat 4 is its first appearance.
pub fn arm(state: &mut PersistedState) {
    if state.starter_story.v != 0 {
        return;
    }
    state.starter_story.v = 1;
    let veteran = !state.claims.is_empty()
        || !state.projects.is_empty()
        || !state.kanban_cards.is_empty();
    if veteran {
        state.starter_story.skipped = true;
        state.starter_story.coach_done = true;
        state.starter_story.comms_built = true;
        state.starter_story.stars_built = true;
        state.starter_story.intro_done = true;
    } else {
        state.companion.visible = false;
    }
}

pub fn boot(app: &tauri::AppHandle) {
    let _ = crate::state::update(app, |st| {
        arm(st);
        let _ = view(st);
        Ok(())
    });
}

pub fn view(state: &PersistedState) -> StarterView {
    refresh_outposts(state);
    StarterView {
        story: state.starter_story.clone(),
        claim_project_id: claim_project_id(state),
        comms_status: card_status(state, ID_COMMS),
        stars_status: card_status(state, ID_STARS),
        starter_id_present: starter_id_present(state),
    }
}

/// Pure. `skipped` or never-evaluated yields nothing. Applying the acts
/// and planning again reaches a fixed point (Intro follows MarkBuilt on
/// the next plan, then stops).
pub fn plan(v: &StarterView) -> Vec<Act> {
    if v.story.skipped || v.story.v == 0 {
        return Vec::new();
    }
    let mut acts = Vec::new();
    if v.story.seeded_project_id.is_none() {
        if let Some(pid) = &v.claim_project_id {
            if !v.starter_id_present {
                acts.push(Act::Seed(pid.clone()));
            }
        }
    }
    push_build_act(
        &mut acts,
        Outpost::Comms,
        ID_COMMS,
        v.comms_status.as_deref(),
        v.story.comms_built,
    );
    push_build_act(
        &mut acts,
        Outpost::Stars,
        ID_STARS,
        v.stars_status.as_deref(),
        v.story.stars_built,
    );
    if v.story.comms_built && !v.story.intro_done {
        acts.push(Act::Intro);
    }
    acts
}

fn push_build_act(
    acts: &mut Vec<Act>,
    kind: Outpost,
    card_id: &str,
    status: Option<&str>,
    built: bool,
) {
    let Some(status) = status else {
        return;
    };
    if status == "done" && !built {
        acts.push(Act::MarkBuilt(kind));
    } else if built && status != "done" {
        acts.push(Act::Complete(card_id.to_string()));
    }
}

pub fn after_save(app: &tauri::AppHandle, view: StarterView) {
    let acts = plan(&view);
    if acts.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || run_acts(&app, &acts));
}

fn run_acts(app: &tauri::AppHandle, acts: &[Act]) {
    for act in acts {
        match act {
            Act::Seed(pid) => exec_seed(app, pid),
            Act::MarkBuilt(kind) => {
                if mark_built(app, *kind) {
                    open_follow_on(app, *kind);
                }
            }
            Act::Complete(id) => {
                let _ = crate::board::kanban_move(app, id, "done");
            }
            Act::Intro => {
                exec_intro(app);
            }
        }
    }
}

/// Check-and-set. A second call, or a starter id already in cards or
/// tombstones, pushes nothing.
pub fn apply_seed(state: &mut PersistedState, project_id: &str) -> bool {
    if state.starter_story.skipped || state.starter_story.v == 0 {
        return false;
    }
    if state.starter_story.seeded_project_id.is_some() || starter_id_present(state) {
        return false;
    }
    let now = now_secs();
    let seeds = [
        (ID_COMMS, TITLE_COMMS, NOTES_COMMS, 0.0),
        (ID_STARS, TITLE_STARS, NOTES_STARS, 1.0),
        (ID_BOARD, TITLE_BOARD, NOTES_BOARD, 2.0),
        (ID_CLAIM, TITLE_CLAIM, NOTES_CLAIM, 3.0),
    ];
    for (id, title, notes, offset) in seeds {
        state.kanban_cards.push(crate::board::todo_card(
            id.to_string(),
            title,
            project_id,
            "todo",
            "starter",
            Some(notes),
            now + offset,
        ));
    }
    state.starter_story.seeded_project_id = Some(project_id.to_string());
    state.starter_story.coach_done = true;
    crate::todo_trees::reconcile(state);
    true
}

fn exec_seed(app: &tauri::AppHandle, project_id: &str) {
    let added = match crate::state::update(app, |st| {
        if !apply_seed(st, project_id) {
            return Ok(None);
        }
        let _ = view(st);
        let cards: Vec<_> = st
            .kanban_cards
            .iter()
            .filter(|c| STARTER_IDS.contains(&c.id.as_str()))
            .cloned()
            .collect();
        Ok(Some(cards))
    }) {
        Ok(Some(cards)) => cards,
        _ => return,
    };
    let _ = app.emit("state:changed", ());
    for card in &added {
        crate::event_log::append_board_proof_best(
            app,
            &card.id,
            &card.status,
            None,
            Some(&card.project_id),
            Some("starter"),
            &serde_json::json!({ "cardId": card.id, "title": card.title }),
        );
    }
}

pub fn apply_mark_built(state: &mut PersistedState, kind: Outpost) -> bool {
    let flag = match kind {
        Outpost::Comms => &mut state.starter_story.comms_built,
        Outpost::Stars => &mut state.starter_story.stars_built,
    };
    if *flag {
        return false;
    }
    *flag = true;
    true
}

pub fn mark_built(app: &tauri::AppHandle, kind: Outpost) -> bool {
    let newly = match crate::state::update(app, |st| {
        let newly = apply_mark_built(st, kind);
        let _ = view(st);
        Ok(newly)
    }) {
        Ok(v) => v,
        Err(_) => false,
    };
    if newly {
        let _ = app.emit("state:changed", ());
    }
    newly
}

/// Plot arrival, or a build card completed with no walker. `newly` opens
/// the matching window once. The card goes Done through `kanban_move`
/// when it still exists.
pub fn outpost_built(app: &tauri::AppHandle, kind: Outpost) {
    let newly = mark_built(app, kind);
    let id = match kind {
        Outpost::Comms => ID_COMMS,
        Outpost::Stars => ID_STARS,
    };
    let _ = crate::board::kanban_move(app, id, "done");
    if newly {
        open_follow_on(app, kind);
    }
}

fn open_follow_on(app: &tauri::AppHandle, kind: Outpost) {
    match kind {
        Outpost::Comms => {
            exec_intro(app);
            let _ = crate::panes::show_companion(app);
        }
        Outpost::Stars => {
            let _ = crate::panes::show_shared_pane(app, "stars");
        }
    }
}

pub fn apply_intro(state: &mut PersistedState) -> bool {
    if state.starter_story.intro_done || !state.starter_story.comms_built {
        return false;
    }
    state.starter_story.intro_done = true;
    for line in INTRO_LINES {
        state.companion.push_turn("them", line);
    }
    if let Some(last) = INTRO_LINES.last() {
        state.companion.last_bot_line = (*last).to_string();
    }
    true
}

fn exec_intro(app: &tauri::AppHandle) {
    let did = crate::state::update(app, |st| {
        let did = apply_intro(st);
        if did {
            let _ = view(st);
        }
        Ok(did)
    })
    .unwrap_or(false);
    if did {
        let _ = app.emit("state:changed", ());
    }
}

pub fn apply_complete(state: &mut PersistedState, id: &str) {
    let Some(card) = state.kanban_cards.iter_mut().find(|c| c.id == id) else {
        return;
    };
    if card.status == "done" {
        return;
    }
    card.status = "done".into();
}

pub fn apply_act(state: &mut PersistedState, act: &Act) {
    match act {
        Act::Seed(pid) => {
            apply_seed(state, pid);
        }
        Act::MarkBuilt(kind) => {
            apply_mark_built(state, *kind);
        }
        Act::Complete(id) => apply_complete(state, id),
        Act::Intro => {
            apply_intro(state);
        }
    }
}

pub fn set_coach_done(app: &tauri::AppHandle) -> Result<(), String> {
    crate::state::update(app, |st| {
        st.starter_story.coach_done = true;
        let _ = view(st);
        Ok(())
    })
}

/// Habitat chore key. Never the card id: enqueue dedups on `card_id`, and
/// the build card's Plant chore would swallow a build keyed the same way.
pub fn build_chore_key(kind: Outpost) -> String {
    match kind {
        Outpost::Comms => "outpost:comms".into(),
        Outpost::Stars => "outpost:stars".into(),
    }
}

pub fn outpost_visual(story: &StarterStory, kind: Outpost) -> OutpostVisual {
    if story.skipped || built_flag(story, kind) {
        return OutpostVisual::Built;
    }
    if story.seeded_project_id.is_some() {
        return OutpostVisual::Plot;
    }
    OutpostVisual::Hidden
}

pub fn outpost_visual_now(kind: Outpost) -> OutpostVisual {
    let bits = OUTPOSTS.load(Ordering::Relaxed);
    let raw = match kind {
        Outpost::Comms => bits & 0b11,
        Outpost::Stars => (bits >> 2) & 0b11,
    };
    match raw {
        1 => OutpostVisual::Plot,
        2 => OutpostVisual::Built,
        _ => OutpostVisual::Hidden,
    }
}

fn built_flag(story: &StarterStory, kind: Outpost) -> bool {
    match kind {
        Outpost::Comms => story.comms_built,
        Outpost::Stars => story.stars_built,
    }
}

fn refresh_outposts(state: &PersistedState) {
    let comms = outpost_visual(&state.starter_story, Outpost::Comms) as u8;
    let stars = outpost_visual(&state.starter_story, Outpost::Stars) as u8;
    OUTPOSTS.store(comms | (stars << 2), Ordering::Relaxed);
}

fn starter_id_present(state: &PersistedState) -> bool {
    let hit = |id: &str| STARTER_IDS.contains(&id);
    state.kanban_cards.iter().any(|c| hit(&c.id))
        || state.card_tombstones.iter().any(|t| hit(&t.id))
}

fn card_status(state: &PersistedState, id: &str) -> Option<String> {
    state
        .kanban_cards
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.status.clone())
}

fn claim_project_id(state: &PersistedState) -> Option<String> {
    let pid = state.claims.first()?.project_id.clone();
    if pid.is_empty() {
        return None;
    }
    if state.projects.iter().any(|p| p.id == pid) {
        Some(pid)
    } else {
        None
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardTombstone, Claim, Project};

    fn project(id: &str) -> Project {
        serde_json::from_value(serde_json::json!({ "id": id, "name": id })).unwrap()
    }

    fn claim(window: &str, project_id: &str) -> Claim {
        serde_json::from_value(serde_json::json!({
            "windowId": window,
            "projectId": project_id,
        }))
        .unwrap()
    }

    fn fresh_claimed(pid: &str) -> PersistedState {
        let mut s = PersistedState::default();
        arm(&mut s);
        s.projects.push(project(pid));
        s.claims.push(claim("w", pid));
        s
    }

    fn assert_veteran(s: &PersistedState) {
        assert_eq!(s.starter_story.v, 1);
        assert!(s.starter_story.skipped);
        assert!(s.starter_story.coach_done);
        assert!(s.starter_story.comms_built);
        assert!(s.starter_story.stars_built);
        assert!(s.starter_story.intro_done);
        assert!(s.companion.visible);
    }

    #[test]
    fn s1_boot_arm_fresh_flags_and_veteran_skip() {
        let mut fresh = PersistedState::default();
        assert!(fresh.companion.visible);
        assert_eq!(fresh.starter_story.v, 0);
        arm(&mut fresh);
        assert_eq!(fresh.starter_story.v, 1);
        assert!(!fresh.starter_story.skipped);
        assert!(!fresh.starter_story.coach_done);
        assert!(fresh.starter_story.seeded_project_id.is_none());
        assert!(!fresh.starter_story.comms_built);
        assert!(!fresh.starter_story.stars_built);
        assert!(!fresh.starter_story.intro_done);
        assert!(!fresh.companion.visible);

        let mut with_claim = PersistedState::default();
        with_claim.claims.push(claim("w", "p"));
        arm(&mut with_claim);
        assert_veteran(&with_claim);

        let mut with_project = PersistedState::default();
        with_project.projects.push(project("p"));
        arm(&mut with_project);
        assert_veteran(&with_project);

        let mut with_card = PersistedState::default();
        with_card.kanban_cards.push(crate::board::todo_card(
            "c-old".into(),
            "Old",
            "p",
            "todo",
            "user",
            None,
            1.0,
        ));
        arm(&mut with_card);
        assert_veteran(&with_card);
    }

    #[test]
    fn s2_first_claim_plans_seed_and_apply_writes_four_cards() {
        let mut s = fresh_claimed("planet-a");
        s.projects.push(project("other"));
        s.claims.push(claim("w2", "other"));
        let acts = plan(&view(&s));
        assert_eq!(acts, vec![Act::Seed("planet-a".into())]);
        apply_act(&mut s, &acts[0]);
        assert_eq!(s.kanban_cards.len(), 4);
        let ids: Vec<_> = s.kanban_cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, [ID_COMMS, ID_STARS, ID_BOARD, ID_CLAIM]);
        for c in &s.kanban_cards {
            assert_eq!(c.status, "todo");
            assert_eq!(c.actor.as_deref(), Some("starter"));
            assert_eq!(c.project_id, "planet-a");
            assert_eq!(c.kind.as_deref(), Some("todo"));
        }
        assert!(s.kanban_cards[0].created_at < s.kanban_cards[1].created_at);
        assert!(s.kanban_cards[1].created_at < s.kanban_cards[2].created_at);
        assert!(s.kanban_cards[2].created_at < s.kanban_cards[3].created_at);
        assert_eq!(s.starter_story.seeded_project_id.as_deref(), Some("planet-a"));
        assert!(s.starter_story.coach_done);
        assert!(!apply_seed(&mut s, "planet-a"));
        assert_eq!(s.kanban_cards.len(), 4);
    }

    #[test]
    fn s3_seed_is_idempotent() {
        let mut s = fresh_claimed("p");
        assert!(apply_seed(&mut s, "p"));
        assert!(!apply_seed(&mut s, "p"));
        assert_eq!(s.kanban_cards.len(), 4);
        assert!(plan(&view(&s)).is_empty());
    }

    #[test]
    fn s4_starter_id_in_cards_or_tombstones_blocks_seed() {
        let mut tomb = fresh_claimed("p");
        tomb.card_tombstones.push(CardTombstone {
            id: ID_BOARD.into(),
            last_touched_at: 1.0,
        });
        assert!(plan(&view(&tomb))
            .iter()
            .all(|a| !matches!(a, Act::Seed(_))));

        let mut card = fresh_claimed("p");
        card.kanban_cards.push(crate::board::todo_card(
            ID_COMMS.into(),
            "x",
            "p",
            "todo",
            "user",
            None,
            1.0,
        ));
        assert!(plan(&view(&card))
            .iter()
            .all(|a| !matches!(a, Act::Seed(_))));
    }

    #[test]
    fn s5_no_seed_when_skipped_unevaluated_or_unclaimed() {
        let mut skipped = fresh_claimed("p");
        skipped.starter_story.skipped = true;
        assert!(plan(&view(&skipped)).is_empty());

        let mut unevaluated = fresh_claimed("p");
        unevaluated.starter_story.v = 0;
        assert!(plan(&view(&unevaluated)).is_empty());

        let mut bare = PersistedState::default();
        arm(&mut bare);
        bare.projects.push(project("p"));
        assert!(plan(&view(&bare))
            .iter()
            .all(|a| !matches!(a, Act::Seed(_))));
    }

    #[test]
    fn s6_build_reconcile_reaches_a_fixed_point() {
        let mut done_unbuilt = fresh_claimed("p");
        apply_seed(&mut done_unbuilt, "p");
        done_unbuilt.starter_story.intro_done = true;
        done_unbuilt
            .kanban_cards
            .iter_mut()
            .find(|c| c.id == ID_COMMS)
            .unwrap()
            .status = "done".into();
        let acts = plan(&view(&done_unbuilt));
        assert_eq!(acts, vec![Act::MarkBuilt(Outpost::Comms)]);
        for act in &acts {
            apply_act(&mut done_unbuilt, act);
        }
        assert!(done_unbuilt.starter_story.comms_built);
        assert!(plan(&view(&done_unbuilt)).is_empty());

        let mut built_todo = fresh_claimed("p");
        apply_seed(&mut built_todo, "p");
        built_todo.starter_story.intro_done = true;
        built_todo.starter_story.comms_built = true;
        let acts = plan(&view(&built_todo));
        assert_eq!(acts, vec![Act::Complete(ID_COMMS.into())]);
        for act in &acts {
            apply_act(&mut built_todo, act);
        }
        assert_eq!(
            built_todo
                .kanban_cards
                .iter()
                .find(|c| c.id == ID_COMMS)
                .unwrap()
                .status,
            "done"
        );
        assert!(plan(&view(&built_todo)).is_empty());

        let mut both = fresh_claimed("p");
        apply_seed(&mut both, "p");
        both.starter_story.comms_built = true;
        both.starter_story.stars_built = true;
        both.starter_story.intro_done = true;
        for c in &mut both.kanban_cards {
            if c.id == ID_COMMS || c.id == ID_STARS {
                c.status = "done".into();
            }
        }
        assert!(plan(&view(&both)).is_empty());
    }

    #[test]
    fn s7_intro_once_three_them_lines() {
        let mut s = fresh_claimed("p");
        s.starter_story.seeded_project_id = Some("p".into());
        s.starter_story.comms_built = true;
        s.companion.image_pack_id = Some("keep".into());
        let acts = plan(&view(&s));
        assert_eq!(acts, vec![Act::Intro]);
        apply_act(&mut s, &acts[0]);
        assert!(s.starter_story.intro_done);
        assert_eq!(s.companion.turns.len(), 3);
        assert!(s.companion.turns.iter().all(|t| t.role == "them"));
        assert!(s.companion.turns.iter().all(|t| t.role != "you"));
        assert_eq!(s.companion.image_pack_id.as_deref(), Some("keep"));
        assert_eq!(s.companion.last_bot_line, INTRO_LINES[2]);
        assert_eq!(s.companion.turns[0].text, INTRO_LINES[0]);
        assert_eq!(s.companion.turns[1].text, INTRO_LINES[1]);
        assert_eq!(s.companion.turns[2].text, INTRO_LINES[2]);
        assert!(plan(&view(&s)).is_empty());
        apply_intro(&mut s);
        assert_eq!(s.companion.turns.len(), 3);
    }

    #[test]
    fn s8_outpost_visual_hidden_plot_built() {
        let mut skipped = StarterStory::default();
        skipped.v = 1;
        skipped.skipped = true;
        assert_eq!(outpost_visual(&skipped, Outpost::Comms), OutpostVisual::Built);
        assert_eq!(outpost_visual(&skipped, Outpost::Stars), OutpostVisual::Built);

        let armed = StarterStory {
            v: 1,
            ..StarterStory::default()
        };
        assert_eq!(outpost_visual(&armed, Outpost::Comms), OutpostVisual::Hidden);
        assert_eq!(outpost_visual(&armed, Outpost::Stars), OutpostVisual::Hidden);

        let mut seeded = armed.clone();
        seeded.seeded_project_id = Some("p".into());
        assert_eq!(outpost_visual(&seeded, Outpost::Comms), OutpostVisual::Plot);
        assert_eq!(outpost_visual(&seeded, Outpost::Stars), OutpostVisual::Plot);

        seeded.comms_built = true;
        assert_eq!(outpost_visual(&seeded, Outpost::Comms), OutpostVisual::Built);
        assert_eq!(outpost_visual(&seeded, Outpost::Stars), OutpostVisual::Plot);
        seeded.stars_built = true;
        assert_eq!(outpost_visual(&seeded, Outpost::Stars), OutpostVisual::Built);
    }

    #[test]
    fn s9_build_chore_key_is_not_the_card_id() {
        assert_eq!(build_chore_key(Outpost::Comms), "outpost:comms");
        assert_eq!(build_chore_key(Outpost::Stars), "outpost:stars");
        let mut keys = Vec::new();
        let plant = ID_COMMS;
        if !keys.iter().any(|k| k == plant) {
            keys.push(plant.to_string());
        }
        let build = build_chore_key(Outpost::Comms);
        if !keys.iter().any(|k| k == &build) {
            keys.push(build);
        }
        assert_eq!(keys.len(), 2, "plant and build both live");
        assert!(keys.contains(&"c-starter-comms".to_string()));
        assert!(keys.contains(&"outpost:comms".to_string()));
    }
}
