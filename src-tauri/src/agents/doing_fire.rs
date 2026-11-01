//! A `GO-*` todo→doing drag launches the assigned agent row, or the saved
//! default runner when the card has no assignment. No row and no default
//! blocks with one Needed and leaves the card in Doing. The drag never
//! marks the card done.
//! Neededs and Dogfood titles are a no-op. Never companion send.

use crate::state::{KanbanCard, PersistedState};

/// The one fact title. The standing rule matches every phrase of it, so
/// the specific GO lives in the why, never in the title.
pub const DOING_FIRE_TITLE: &str = "Who runs this doing GO?";

fn touched_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// The fire rule: `kind=todo`, was `todo` in the before snapshot, now
/// `doing`, and the title carries the bus's GO token
/// (`needed_rules::extract_go` — the same regex). A card born `doing`
/// never fired; Neededs and Dogfood cards never mint this question.
fn fire_candidates<'a>(before: &PersistedState, after: &'a PersistedState) -> Vec<(&'a KanbanCard, String)> {
    let mut out = Vec::new();
    for card in &after.kanban_cards {
        if card.kind.as_deref() != Some("todo") || card.status != "doing" {
            continue;
        }
        let was_todo = before
            .kanban_cards
            .iter()
            .any(|b| b.id == card.id && b.status == "todo");
        if !was_todo {
            continue;
        }
        if card.title.starts_with("Dogfood ") {
            continue;
        }
        let Some(go) = crate::needed_rules::extract_go(&card.title) else {
            continue;
        };
        out.push((card, go));
    }
    out
}

fn fired_edges<'a>(
    before: &PersistedState,
    after: &'a PersistedState,
    edges: &crate::board::blockers::Edges,
) -> Vec<(&'a KanbanCard, String)> {
    fire_candidates(before, after)
        .into_iter()
        .filter(|(card, _)| !crate::board::blockers::card_blocked(edges, &after.kanban_cards, &card.id))
        .collect()
}

fn fired<'a>(before: &PersistedState, after: &'a PersistedState) -> Vec<(&'a KanbanCard, String)> {
    fired_edges(before, after, &crate::board::blockers::Edges::default())
}

/// What a todo→doing GO drag should do. Pure so the tests don't need a socket.
#[derive(Debug)]
pub(crate) enum Drag {
    /// One row. Launch it. Do not ask who, and do not mark the card done.
    /// `via_default` is false when the card's assigned id picked the row.
    Launch { agent_id: String, via_default: bool },
    /// Stay in Doing. One Needed, one sentence.
    Block { why: String },
}

/// Assigned id wins. Else the saved default runner (a fresh install has
/// none — the user chooses). Two name matches block. Never a baked-in kitchen.
pub(crate) fn drag_decision(
    assigned_agent_id: Option<&str>,
    agents: &[(String, String, String)],
    chosen: Option<&crate::plans::RunnerSpec>,
) -> Drag {
    let id = assigned_agent_id.map(str::trim).filter(|s| !s.is_empty());
    if let Some(id) = id {
        let n = agents.iter().filter(|(aid, _, _)| aid == id).count();
        return if n == 1 {
            Drag::Launch {
                agent_id: id.to_string(),
                via_default: false,
            }
        } else if n == 0 {
            Drag::Block {
                why: "Assigned agent is gone. Pick another row.".into(),
            }
        } else {
            Drag::Block {
                why: "Two agent rows match this assignment.".into(),
            }
        };
    }
    let Some(spec) = chosen else {
        return Drag::Block {
            why: "Choose a default runner in Settings. A new install does not ship one.".into(),
        };
    };
    let name = spec.runner.trim();
    let harness = spec.harness.trim().to_ascii_lowercase();
    let by_name: Vec<_> = agents
        .iter()
        .filter(|(_, n, _)| !name.is_empty() && n.eq_ignore_ascii_case(name))
        .map(|(id, _, _)| id.clone())
        .collect();
    if by_name.len() > 1 {
        return Drag::Block {
            why: format!("Two agent rows match {name}."),
        };
    }
    if let Some(id) = by_name.into_iter().next() {
        return Drag::Launch {
            agent_id: id,
            via_default: true,
        };
    }
    let by_harness: Vec<_> = agents
        .iter()
        .filter(|(_, _, h)| !harness.is_empty() && h.eq_ignore_ascii_case(&harness))
        .map(|(id, _, _)| id.clone())
        .collect();
    if by_harness.len() > 1 {
        return Drag::Block {
            why: format!("Two agent rows use harness {harness}. Name the default runner."),
        };
    }
    if let Some(id) = by_harness.into_iter().next() {
        return Drag::Launch {
            agent_id: id,
            via_default: true,
        };
    }
    Drag::Block {
        why: format!("No agent row for runner {name} ({harness}). Add one, or change the default."),
    }
}

/// Cursor preset when the saved default picked the row. An assigned row
/// keeps its own model and permissions. Absent model on a default-picked
/// row is filled; permissions overwrite only on that path.
pub(crate) fn apply_default_preset(
    agent: &mut crate::state::AgentShortcut,
    spec: Option<&crate::plans::RunnerSpec>,
    via_default: bool,
) {
    if !via_default {
        return;
    }
    let Some(spec) = spec else {
        return;
    };
    if agent
        .model
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
        && !spec.model.trim().is_empty()
    {
        agent.model = Some(spec.model.trim().to_string());
    }
    if let Some(p) = spec
        .permissions
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        agent.permissions = p.to_string();
    }
}

/// Seal refusal and the runner/spend cap, before any launch.
#[derive(Debug)]
pub(crate) enum PreLaunch {
    Refuse(String),
    Capped(String),
    Go,
}

pub(crate) fn pre_launch(gate: &crate::plans::Gate, caps: Result<(), String>) -> PreLaunch {
    if let crate::plans::Gate::Refuse { note } = gate {
        return PreLaunch::Refuse(note.clone());
    }
    match caps {
        Err(why) => PreLaunch::Capped(why),
        Ok(()) => PreLaunch::Go,
    }
}

/// Plain-words why. No `key=value` token, so the standing rule cannot
/// read a block as a launch answer.
fn block_why(reason: &str, card_id: &str, go: &str) -> String {
    format!("{reason} Card: {card_id}, {go}.")
}

/// The open `Who runs this doing GO?` Needed, with `why` written over the
/// previous one. Same id. None when that Needed is not open.
fn refreshed(st: &PersistedState, why: &str) -> Option<PersistedState> {
    let idx = st.kanban_cards.iter().position(|c| {
        c.kind.as_deref() == Some("needed")
            && c.needed_state.as_deref() == Some("open")
            && c.title.trim() == DOING_FIRE_TITLE
    })?;
    let mut after = st.clone();
    let card = &mut after.kanban_cards[idx];
    card.needed_why = Some(why.to_string());
    card.last_touched_at = touched_secs();
    Some(after)
}

fn block(app: &tauri::AppHandle, card: &KanbanCard, go: &str, reason: &str) {
    let why = block_why(reason, &card.id, go);
    let _ = crate::board::kanban_note_append(app, &card.id, &why);
    if let Ok(st) = crate::state::load_state(app) {
        if let Some(after) = refreshed(&st, &why) {
            let _ = crate::board::save_with_events(app, &st, &after);
            return;
        }
    }
    let _ = crate::board::needed_open(app, DOING_FIRE_TITLE, &card.project_id, Some(&why));
}

fn launch_assigned(app: &tauri::AppHandle, card: &KanbanCard, go: &str) {
    let urls: Vec<String> = card
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|l| l.url.clone())
        .collect();
    let gate = crate::plans::gate_for_card(&card.id, &card.title, &urls);
    // A refusal returns before the cap check. `check` can open its own
    // Needed and write state; an unsealed card must not take that path.
    let caps = if matches!(gate, crate::plans::Gate::Refuse { .. }) {
        Ok(())
    } else {
        crate::gate_caps::check(app)
    };
    match pre_launch(&gate, caps) {
        PreLaunch::Refuse(note) => {
            block(app, card, go, &note);
            crate::event_log::append_best(
                app,
                "go-launch",
                None,
                Some(&card.project_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "id": go,
                    "cardId": card.id,
                    "route": "doing",
                    "launched": false,
                    "note": note,
                }),
            );
            return;
        }
        PreLaunch::Capped(why) => {
            let _ = crate::board::kanban_note_append(app, &card.id, &why);
            return;
        }
        PreLaunch::Go => {}
    }
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let rows: Vec<(String, String, String)> = st
        .agent_shortcuts
        .iter()
        .map(|a| (a.id.clone(), a.name.clone(), a.harness.clone()))
        .collect();
    let chosen = crate::plans::chosen_runner(&crate::plans::Roots::real());
    match drag_decision(card.assigned_agent_id.as_deref(), &rows, chosen.as_ref()) {
        Drag::Block { why } => block(app, card, go, &why),
        Drag::Launch {
            agent_id,
            via_default,
        } => {
            let Some(mut agent) = st
                .agent_shortcuts
                .iter()
                .find(|a| a.id == agent_id)
                .cloned()
            else {
                block(app, card, go, "Assigned agent is gone. Pick another row.");
                return;
            };
            apply_default_preset(&mut agent, chosen.as_ref(), via_default);
            let project_name = st
                .projects
                .iter()
                .find(|p| p.id == card.project_id)
                .map(|p| p.name.clone());
            let handoff = match gate {
                crate::plans::Gate::Launch(parsed) => {
                    let project = project_name.clone().unwrap_or_default();
                    let run = crate::plans::resolve_run(&parsed.header, &parsed.id(), &project);
                    Some(crate::coordinator::GoLaunch {
                        parsed,
                        run,
                        route: "doing",
                        card_id: card.id.clone(),
                        project_id: card.project_id.clone(),
                    })
                }
                _ => None,
            };
            let _ = crate::coordinator::set_companion_hack(
                app.clone(),
                true,
                Some(agent.harness.clone()),
                agent.project_id.clone(),
            );
            if let Err(e) = crate::coordinator::launch_agent(
                app.clone(),
                agent.clone(),
                project_name,
                vec![card.title.clone()],
                handoff,
            ) {
                let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
                block(app, card, go, &e);
                return;
            }
            crate::event_log::append_best(
                app,
                "doing-assign",
                None,
                Some(&card.project_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "cardId": card.id,
                    "go": go,
                    "agentId": agent.id,
                    "harness": agent.harness,
                }),
            );
        }
    }
}

/// The wire: call with the disk-before snapshot and the just-saved state
/// from each kanban-move door. Best-effort — a fire never fails the save.
pub fn fire_on_moves(app: &tauri::AppHandle, before: &PersistedState, after: &PersistedState) {
    let edges = crate::board::blockers::load_app(app);
    for (card, go) in fire_candidates(before, after) {
        if crate::board::blockers::card_blocked(&edges, &after.kanban_cards, &card.id) {
            let titles = crate::board::blockers::open_blocker_titles(&edges, &after.kanban_cards, &card.id);
            let shown = if titles.is_empty() {
                "a card".to_string()
            } else {
                titles.join(", ")
            };
            let note = format!("Blocked by {shown}; not launching.");
            let _ = crate::board::kanban_note_append(app, &card.id, &note);
            crate::event_log::append_best(
                app,
                "go-launch",
                None,
                Some(&card.project_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "id": go,
                    "cardId": card.id,
                    "route": "doing",
                    "launched": false,
                    "reason": "blocked",
                }),
            );
            continue;
        }
        if crate::plans::lock_names_live(&card.id) {
            continue;
        }
        launch_assigned(app, card, &go);
    }
}

/// Doing-fire + walker attach after the column save has returned.
/// Needed mint and cursor-go stay the same; they must not hitch the drag.
pub fn spawn_fire_and_attach(
    app: &tauri::AppHandle,
    before: PersistedState,
    after: PersistedState,
) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("orbitty-doing-fx".into())
        .spawn(move || {
            crate::state::with_actor("agent", || {
                crate::board::blockers::note_ready_on_done(&app, &before, &after);
                fire_on_moves(&app, &before, &after);
                #[cfg(unix)]
                crate::agent_runs::attach_on_moves(&app, &before, &after);
            })
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wiring guard: refusal and the runner cap each return before launch.
    #[test]
    fn doing_fire_runs_the_go_gate_before_launch() {
        let src = include_str!("doing_fire.rs");
        let pre_fn = src.find("fn pre_launch(").expect("pre_launch");
        let start = src.find("fn launch_assigned(").expect("launch_assigned");
        assert!(pre_fn < start, "pre_launch is the gate");
        assert!(
            src[pre_fn..start].contains("Gate::Refuse"),
            "a refusal is a pre-launch stop"
        );
        let body = &src[start..];
        let gate = body
            .find("gate_for_card(")
            .expect("doing drag must call the GO gate");
        let check = body.find("gate_caps::check").expect("runner cap");
        let pre = body.find("pre_launch(").expect("pre_launch");
        let launch = body.find("launch_agent(").expect("launch path");
        assert!(
            gate < check && check < launch && pre < launch,
            "the gate and the cap run before launch"
        );
        let refuse = body.find("PreLaunch::Refuse").expect("refuse arm");
        let capped = body.find("PreLaunch::Capped").expect("caps arm");
        let go_arm = body.find("PreLaunch::Go").expect("go arm");
        assert!(refuse < capped && capped < go_arm);
        assert!(
            body[refuse..capped].contains("return;"),
            "Refuse arm must return before launch"
        );
        assert!(
            body[capped..go_arm].contains("return;"),
            "caps arm must return before launch"
        );
    }

    #[test]
    fn doing_fire_skips_a_card_the_run_lock_names() {
        let src = include_str!("doing_fire.rs");
        let start = src.find("pub fn fire_on_moves(").expect("fire_on_moves");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("end");
        let body = &body[..end];
        let skip = body.find("lock_names_live(").expect("skip");
        let launch = body.find("launch_assigned(").expect("launch");
        assert!(skip < launch, "the lock skip runs before launch");
        assert!(body[skip..launch].contains("continue"));
    }

    #[test]
    fn unassigned_without_a_saved_default_blocks() {
        match drag_decision(None, &[], None) {
            Drag::Block { why } => assert!(why.contains("Choose a default runner"), "{why}"),
            Drag::Launch { .. } => panic!("must not launch"),
        }
    }

    #[test]
    fn assigned_row_launches_and_two_names_block() {
        let rows = vec![
            ("a-pi".into(), "pi".into(), "pi".into()),
            ("a-2".into(), "pi".into(), "cursor".into()),
        ];
        match drag_decision(Some("a-pi"), &rows, None) {
            Drag::Launch {
                agent_id,
                via_default,
            } => {
                assert_eq!(agent_id, "a-pi");
                assert!(!via_default);
            }
            Drag::Block { why } => panic!("{why}"),
        }
        let spec = crate::plans::RunnerSpec {
            runner: "pi".into(),
            harness: "pi".into(),
            model: String::new(),
            runner_fallback: None,
            permissions: None,
        };
        match drag_decision(None, &rows, Some(&spec)) {
            Drag::Block { why } => assert!(why.contains("Two agent rows"), "{why}"),
            Drag::Launch { .. } => panic!("ambiguous name must block"),
        }
    }

    fn spec(model: &str, permissions: Option<&str>) -> crate::plans::RunnerSpec {
        crate::plans::RunnerSpec {
            runner: "orbitty-cursor".into(),
            harness: "cursor".into(),
            model: model.into(),
            runner_fallback: None,
            permissions: permissions.map(str::to_string),
        }
    }

    fn row(permissions: &str, model: Option<&str>) -> crate::state::AgentShortcut {
        crate::state::AgentShortcut {
            id: "a-1".into(),
            name: "row".into(),
            harness: "cursor".into(),
            profile: String::new(),
            model: model.map(str::to_string),
            md_paths: Vec::new(),
            project_id: None,
            extra: None,
            permissions: permissions.into(),
        }
    }

    #[test]
    fn assigned_row_keeps_its_own_preset() {
        let spec = spec("M", Some("workspace-write"));
        let mut owned = row("read-only", Some("row-model"));
        apply_default_preset(&mut owned, Some(&spec), false);
        assert_eq!(owned.permissions, "read-only");
        assert_eq!(owned.model.as_deref(), Some("row-model"));

        let mut empty = row("read-only", None);
        apply_default_preset(&mut empty, Some(&spec), false);
        assert_eq!(empty.permissions, "read-only");
        assert!(empty.model.as_deref().unwrap_or("").is_empty());

        let mut blank = row("read-only", Some(""));
        apply_default_preset(&mut blank, Some(&spec), false);
        assert_eq!(blank.permissions, "read-only");
        assert_eq!(blank.model.as_deref(), Some(""));
    }

    #[test]
    fn default_picked_row_gets_the_preset() {
        let spec = spec("M", Some("workspace-write"));
        let mut row = row("read-only", None);
        apply_default_preset(&mut row, Some(&spec), true);
        assert_eq!(row.permissions, "workspace-write");
        assert_eq!(row.model.as_deref(), Some("M"));

        let rows = vec![("a-pi".into(), "pi".into(), "pi".into())];
        let named = crate::plans::RunnerSpec {
            runner: "pi".into(),
            harness: "pi".into(),
            model: "M".into(),
            runner_fallback: None,
            permissions: Some("workspace-write".into()),
        };
        match drag_decision(None, &rows, Some(&named)) {
            Drag::Launch {
                agent_id,
                via_default,
            } => {
                assert_eq!(agent_id, "a-pi");
                assert!(via_default);
            }
            Drag::Block { why } => panic!("{why}"),
        }
        let harness_rows = vec![("a-c".into(), "other".into(), "cursor".into())];
        let by_harness = crate::plans::RunnerSpec {
            runner: "missing-name".into(),
            harness: "cursor".into(),
            model: "M".into(),
            runner_fallback: None,
            permissions: Some("workspace-write".into()),
        };
        match drag_decision(None, &harness_rows, Some(&by_harness)) {
            Drag::Launch { via_default, .. } => assert!(via_default),
            Drag::Block { why } => panic!("{why}"),
        }
    }

    fn launched_gate() -> crate::plans::Gate {
        crate::plans::Gate::Launch(crate::plans::ParsedGo {
            path: std::path::PathBuf::new(),
            raw: String::new(),
            yaml: String::new(),
            header: crate::plans::GoHeader::default(),
            body: String::new(),
            legacy: false,
        })
    }

    #[test]
    fn pre_launch_refuse_cap_and_go() {
        let refuse = crate::plans::Gate::Refuse {
            note: "unsealed".into(),
        };
        match pre_launch(&refuse, Ok(())) {
            PreLaunch::Refuse(n) => assert_eq!(n, "unsealed"),
            PreLaunch::Capped(w) => panic!("cap {w}"),
            PreLaunch::Go => panic!("refusal must not launch"),
        }
        match pre_launch(&refuse, Err("over cap".into())) {
            PreLaunch::Refuse(n) => assert_eq!(n, "unsealed"),
            PreLaunch::Capped(w) => panic!("refuse wins before the cap {w}"),
            PreLaunch::Go => panic!("refusal must not launch"),
        }
        let go = launched_gate();
        match pre_launch(&go, Err("over cap".into())) {
            PreLaunch::Capped(w) => assert!(w.contains("over cap"), "{w}"),
            PreLaunch::Go => panic!("cap must not launch"),
            PreLaunch::Refuse(n) => panic!("refuse {n}"),
        }
        match pre_launch(&go, Ok(())) {
            PreLaunch::Go => {}
            PreLaunch::Refuse(n) => panic!("refuse {n}"),
            PreLaunch::Capped(w) => panic!("cap {w}"),
        }
    }

    #[test]
    fn two_rows_with_the_same_assigned_id_block() {
        let rows = vec![
            ("same".into(), "one".into(), "cursor".into()),
            ("same".into(), "two".into(), "cursor".into()),
        ];
        match drag_decision(Some("same"), &rows, None) {
            Drag::Block { why } => {
                assert_eq!(why, "Two agent rows match this assignment.");
            }
            Drag::Launch { .. } => panic!("two assigned rows must block"),
        }
    }

    #[test]
    fn chosen_runner_is_none_without_settings_and_some_when_saved() {
        let home = std::env::temp_dir().join(format!(
            "orbitty-cr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&home);
        let roots = crate::plans::Roots { home: home.clone() };
        assert!(crate::plans::chosen_runner(&roots).is_none());
        let dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"defaultRunner":{"runner":"pi","harness":"pi","model":"m"}}"#,
        )
        .unwrap();
        let saved = crate::plans::chosen_runner(&roots).expect("saved runner");
        assert_eq!(saved.runner, "pi");
        assert_eq!(saved.harness, "pi");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn builtin_kitchen_row_does_not_launch() {
        let rows = vec![("a-cur".into(), "orbitty-cursor".into(), "cursor".into())];
        match drag_decision(None, &rows, None) {
            Drag::Block { why } => {
                assert!(why.contains("Choose a default runner"), "{why}");
            }
            Drag::Launch { .. } => panic!("the builtin row is not a default"),
        }
    }

    #[test]
    fn block_why_names_the_card_without_a_go_token() {
        let why = block_why(
            "Assigned agent is gone. Pick another row.",
            "c-9",
            "GO-ORBITTY-X",
        );
        assert_eq!(
            why,
            "Assigned agent is gone. Pick another row. Card: c-9, GO-ORBITTY-X."
        );
        assert!(why.split_whitespace().all(|tok| !tok.contains('=')));
        assert!(crate::needed_rules::classify(DOING_FIRE_TITLE, "p-1", &why)
            .go
            .is_none());
    }

    #[test]
    fn refresh_keeps_the_id_and_replaces_the_why() {
        let mut st = PersistedState::default();
        let mut n = card("n-1", "needed", "todo", DOING_FIRE_TITLE);
        n.needed_state = Some("open".into());
        n.needed_why = Some("X".into());
        n.last_touched_at = 1.0;
        st.kanban_cards = vec![n];
        let after = refreshed(&st, "Y").expect("open needed");
        assert_eq!(after.kanban_cards.len(), 1);
        assert_eq!(after.kanban_cards[0].id, "n-1");
        assert_eq!(after.kanban_cards[0].needed_why.as_deref(), Some("Y"));
        assert!(after.kanban_cards[0].last_touched_at > 1.0);
    }

    fn card(id: &str, kind: &str, status: &str, title: &str) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: "p-1".into(),
            window_id: None,
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
    fn fire_rule() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("c-1", "todo", "todo", "GO-ORBITTY-RAIL-CLOSE"),
            card("c-2", "todo", "doing", "GO-ORBITTY-BORN-DOING"),
            card("c-3", "needed", "todo", "GO-ORBITTY-NEEDED"),
            card("c-4", "todo", "todo", "Dogfood GO-ORBITTY-X?"),
            card("c-5", "todo", "todo", "plain todo"),
            card("c-6", "todo", "doing", "GO-ORBITTY-STAYS-DOING"),
        ];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into(); // the drag — fires
        after.kanban_cards[3].status = "doing".into(); // Dogfood — no-op
        after.kanban_cards[4].status = "doing".into(); // no GO token — no-op
        let hits = fired(&before, &after);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.id, "c-1");
        assert_eq!(hits[0].1, "GO-ORBITTY-RAIL-CLOSE");
    }

    #[test]
    fn fired_skips_blocked_go_card() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("c-1", "todo", "todo", "GO-ORBITTY-CARD-BLOCKERS"),
            card("c-b", "todo", "todo", "Alpha"),
        ];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into();
        let mut edges = crate::board::blockers::Edges::default();
        edges.map.insert("c-1".into(), vec!["c-b".into()]);
        assert!(fired_edges(&before, &after, &edges).is_empty());
        after.kanban_cards[1].status = "done".into();
        let hits = fired_edges(&before, &after, &edges);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.id, "c-1");
        edges.map.insert("c-1".into(), vec!["gone".into()]);
        assert_eq!(fired_edges(&before, &after, &edges).len(), 1);

        let src = include_str!("doing_fire.rs");
        let start = src.find("pub fn fire_on_moves(").expect("fire_on_moves");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("end");
        let body = &body[..end];
        let note = body.find("Blocked by ").expect("note");
        let launch = body.find("launch_assigned(").expect("launch");
        assert!(note < launch);
        assert!(body[note..launch].contains("continue"));
        assert!(body.contains("not launching"));
        let reason = body.find("\"reason\"").expect("reason");
        assert!(body[reason..].contains("\"blocked\""));
    }
}
