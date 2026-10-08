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
        let src = concat!(include_str!("mod.rs"), include_str!("write.rs"), include_str!("tests.rs"));
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
        let control = include_str!("../../agents/control.rs");
        assert!(control.contains("mouth_route::run_outcome"));
        assert!(control.contains("pendingId"));
        let ctl = include_str!("../../bin/orbittyctl.rs");
        assert!(ctl.contains("[\"route\", \"run\"]"));
        assert!(ctl.contains("pendingId"));
        let mcp = include_str!("../../bin/orbitty_mcp.rs");
        assert!(mcp.contains("name: \"route.run\""));
        assert!(mcp.contains("name: \"route.table\""));
        assert!(mcp.contains("pendingId"));
    }

    #[test]
    fn write_lane_still_reaches_the_route() {
        let src = concat!(
include_str!("../../coordinator/hermes.rs"),
include_str!("../../coordinator/allowlist.rs"),
include_str!("../../coordinator/plug.rs"),
include_str!("../../coordinator/mouth_cli.rs"),
include_str!("../../coordinator/commands.rs"),
include_str!("../../coordinator/intercept.rs"),
include_str!("../../coordinator/send.rs"),
include_str!("../../coordinator/soul_wake.rs"),
include_str!("../../coordinator/action.rs"),
include_str!("../../coordinator/paths.rs"),
include_str!("../../coordinator/cursor_perm.rs"),
include_str!("../../coordinator/launch_agent.rs"),
include_str!("../../coordinator/mac.rs"),
include_str!("../../coordinator/launch_tests.rs")
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
        let src = concat!(include_str!("mod.rs"), include_str!("write.rs"), include_str!("tests.rs"));
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
        let body = fn_body(concat!(include_str!("mod.rs"), include_str!("write.rs"), include_str!("tests.rs")), "pub fn run_outcome(");
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
        let body = fn_body(concat!(include_str!("mod.rs"), include_str!("write.rs"), include_str!("tests.rs")), "fn commit_board(");
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
        let body = fn_body(concat!(include_str!("mod.rs"), include_str!("write.rs"), include_str!("tests.rs")), "fn log_board_add(");
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
        let control = include_str!("../../agents/control.rs");
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
include_str!("../../coordinator/hermes.rs"),
include_str!("../../coordinator/allowlist.rs"),
include_str!("../../coordinator/plug.rs"),
include_str!("../../coordinator/mouth_cli.rs"),
include_str!("../../coordinator/commands.rs"),
include_str!("../../coordinator/intercept.rs"),
include_str!("../../coordinator/send.rs"),
include_str!("../../coordinator/soul_wake.rs"),
include_str!("../../coordinator/action.rs"),
include_str!("../../coordinator/paths.rs"),
include_str!("../../coordinator/cursor_perm.rs"),
include_str!("../../coordinator/launch_agent.rs"),
include_str!("../../coordinator/mac.rs"),
include_str!("../../coordinator/launch_tests.rs")
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
