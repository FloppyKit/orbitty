    use super::*;
    use crate::state::{Claim, KanbanCard, PersistedState, Project};
    use std::time::UNIX_EPOCH;

    fn future() -> SystemTime {
        SystemTime::now() + Duration::from_secs(30)
    }

    fn agent(name: &str) -> ClaimOrigin {
        ClaimOrigin::Agent(name.to_string())
    }

    fn win(id: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            pid: 1,
            app_name: "App".into(),
            title: title.into(),
            bundle_id: Some("app.bundle".into()),
            stable_key: format!("app.bundle|{title}|1"),
            icon_base64: None,
            is_frontmost: true,
            is_minimized: false,
            document_path: None,
        }
    }

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            trackers: None,
            workspace_root: None,
        }
    }

    fn claim_of(window_id: &str, project_id: &str, seed: u64, pack: &str) -> Claim {
        Claim {
            window_id: window_id.into(),
            project_id: project_id.into(),
            last_touched_at: 1.0,
            bundle_id: Some("app.bundle".into()),
            app_name: Some("App".into()),
            title_at_claim: Some("notes".into()),
            title_index: Some(1),
            sprite_pack_id: Some(pack.into()),
            char_seed: Some(seed),
            char_look: None,
            sprite_scale: None,
            walker_home: None,
            file_path: None,
            hermes_profile: None,
            hermes_session: None,
            action_id: None,
            command: None,
            cwd: None,
            tab: None,
        }
    }

    fn card_of(
        id: &str,
        project_id: &str,
        title: &str,
        window_id: Option<&str>,
        status: &str,
        touched: f64,
    ) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: project_id.into(),
            window_id: window_id.map(str::to_string),
            kind: Some("todo".into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: touched,
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

    fn pending(origin: ClaimOrigin, expires: SystemTime) -> PendingUndo {
        PendingUndo {
            window_id: "W".into(),
            origin,
            expires,
            prev_claim: None,
            written_project_id: "P".into(),
            prev_last_claim_project_id: None,
            card: None,
            batch: Vec::new(),
        }
    }

    /// T1. Unrelated writes made after the claim survive a targeted undo.
    #[test]
    fn undo_keeps_unrelated_writes() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("P", "Planet"), project("Q", "Other")];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "P", &list);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());
        after.kanban_cards
            .push(card_of("c-new", "Q", "fresh card", None, "todo", 9.0));
        after.claims.push(claim_of("X", "Q", 7, "other-pack"));
        after
            .projects
            .iter_mut()
            .find(|p| p.id == "Q")
            .unwrap()
            .name = "Renamed".into();
        after.companion.last_user_line = "keep me".into();
        undo_claim_in(&mut after, &pending).unwrap();
        assert!(after.claims.iter().all(|c| c.window_id != "W"));
        assert!(after
            .claims
            .iter()
            .any(|c| c.window_id == "X" && c.project_id == "Q" && c.char_seed == Some(7)));
        assert!(after
            .kanban_cards
            .iter()
            .any(|c| c.id == "c-new" && c.title == "fresh card"));
        assert_eq!(
            after.projects.iter().find(|p| p.id == "Q").unwrap().name,
            "Renamed"
        );
        assert_eq!(after.companion.last_user_line, "keep me");
        assert_eq!(after.last_claim_project_id, None);
    }

    /// T2. Undo puts the previous claim row back, seed and pack included.
    #[test]
    fn undo_restores_previous_claim() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "Alpha"), project("B", "Beta")];
        before.claims = vec![claim_of("W", "A", 42, "pack-a")];
        before.last_claim_project_id = Some("A".into());
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let live = after.claims.iter().find(|c| c.window_id == "W").unwrap();
        assert_eq!(live.project_id, "B");
        assert_eq!(live.char_seed, Some(42));
        assert_eq!(live.sprite_pack_id.as_deref(), Some("pack-a"));
        let pending = capture_undo(&before, &after, "W", agent("mcp"), future());
        undo_claim_in(&mut after, &pending).unwrap();
        let restored = after.claims.iter().find(|c| c.window_id == "W").unwrap();
        assert_eq!(restored.project_id, "A");
        assert_eq!(restored.char_seed, Some(42));
        assert_eq!(restored.sprite_pack_id.as_deref(), Some("pack-a"));
        assert_eq!(after.last_claim_project_id.as_deref(), Some("A"));
    }

    /// T3. The card apply_claim retargeted comes back. A card moved since stays.
    #[test]
    fn undo_restores_card_retarget() {
        let w = win("W", "doc");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![
            project("A", "Alpha"),
            project("B", "Beta"),
            project("C", "Gamma"),
        ];
        before.kanban_cards = vec![card_of("c-open", "A", "", Some("W"), "doing", 1.5)];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let moved = after.kanban_cards.iter().find(|c| c.id == "c-open").unwrap();
        assert_eq!(moved.project_id, "B");
        assert_eq!(moved.title, "App — doc");
        assert_ne!(moved.last_touched_at, 1.5);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());
        undo_claim_in(&mut after, &pending).unwrap();
        let card = after.kanban_cards.iter().find(|c| c.id == "c-open").unwrap();
        assert_eq!(card.project_id, "A");
        assert_eq!(card.window_id.as_deref(), Some("W"));
        assert_eq!(card.title, "");
        assert_eq!(card.last_touched_at, 1.5);

        let mut moved_since = before.clone();
        apply_claim(&mut moved_since, &w, "B", &list);
        let pending = capture_undo(&before, &moved_since, "W", ClaimOrigin::User, future());
        {
            let card = moved_since
                .kanban_cards
                .iter_mut()
                .find(|c| c.id == "c-open")
                .unwrap();
            card.project_id = "C".into();
            card.title = "moved by hand".into();
            card.window_id = Some("elsewhere".into());
            card.last_touched_at = 99.0;
        }
        undo_claim_in(&mut moved_since, &pending).unwrap();
        let card = moved_since
            .kanban_cards
            .iter()
            .find(|c| c.id == "c-open")
            .unwrap();
        assert_eq!(card.project_id, "C");
        assert_eq!(card.title, "moved by hand");
        assert_eq!(card.window_id.as_deref(), Some("elsewhere"));
        assert_eq!(card.last_touched_at, 99.0);
    }

    /// T4. A claim re-pointed or dropped since the arm is not undone.
    #[test]
    fn undo_refuses_when_claim_changed_since() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("B", "Beta"), project("C", "Gamma")];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());

        let mut reclaimed = after.clone();
        reclaimed
            .claims
            .iter_mut()
            .find(|c| c.window_id == "W")
            .unwrap()
            .project_id = "C".into();
        let snap = serde_json::to_value(&reclaimed).unwrap();
        let err = undo_claim_in(&mut reclaimed, &pending).unwrap_err();
        assert!(err.contains("claim changed since"), "{err}");
        assert_eq!(serde_json::to_value(&reclaimed).unwrap(), snap);

        let mut gone = after.clone();
        gone.claims.retain(|c| c.window_id != "W");
        let snap = serde_json::to_value(&gone).unwrap();
        let err = undo_claim_in(&mut gone, &pending).unwrap_err();
        assert!(err.contains("claim changed since"), "{err}");
        assert_eq!(serde_json::to_value(&gone).unwrap(), snap);
    }

    /// T5. An agent undoes or changes only its own pending claim.
    #[test]
    fn agent_cannot_undo_or_change_others() {
        let exp = future();
        let now = SystemTime::now();
        for verb in ["undo", "change"] {
            let other = pending(agent("b"), exp);
            let err = check_pending(&other, &agent("a"), now, verb).unwrap_err();
            assert_eq!(err, "not your claim");

            let user_made = pending(ClaimOrigin::User, exp);
            let err = check_pending(&user_made, &agent("a"), now, verb).unwrap_err();
            assert_eq!(
                err,
                format!("claim was made from the tray/app; {verb} it there")
            );

            assert!(check_pending(&other, &ClaimOrigin::User, now, verb).is_ok());
            let mine = pending(agent("a"), exp);
            assert!(check_pending(&mine, &agent("a"), now, verb).is_ok());
        }
    }

    /// T6. change uses the same expiry window as undo.
    #[test]
    fn change_and_undo_refuse_expired() {
        let p = pending(agent("a"), UNIX_EPOCH);
        let now = SystemTime::now();
        assert_eq!(
            check_pending(&p, &agent("a"), now, "undo").unwrap_err(),
            "undo expired"
        );
        assert_eq!(
            check_pending(&p, &agent("a"), now, "change").unwrap_err(),
            "change expired"
        );
    }

    /// T7. A refused take leaves the pending slot. An accepted take clears it.
    #[test]
    fn refused_undo_keeps_pending() {
        let exp = future();
        let now = SystemTime::now();
        let mut slot = Some(pending(agent("b"), exp));
        let err = try_take(&mut slot, &agent("a"), now).unwrap_err();
        assert_eq!(err, "not your claim");
        assert!(slot.is_some());

        let mut user_slot = Some(pending(ClaimOrigin::User, exp));
        let err = try_take(&mut user_slot, &agent("a"), now).unwrap_err();
        assert!(err.contains("tray/app"), "{err}");
        assert!(user_slot.is_some());

        let mut expired = Some(pending(agent("a"), UNIX_EPOCH));
        let err = try_take(&mut expired, &agent("a"), now).unwrap_err();
        assert_eq!(err, "undo expired");
        assert!(expired.is_some());

        let taken = try_take(&mut slot, &agent("b"), now).unwrap();
        assert_eq!(taken.origin, agent("b"));
        assert!(slot.is_none());
    }

    #[test]
    fn v1_user_title_edit_on_retargeted_card_survives_undo() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "A"), project("B", "B")];
        before.claims.push(claim_of("W", "A", 5, "pack"));
        before.kanban_cards.push(card_of("c1", "A", "orig title", Some("W"), "doing", 1.0));
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let p = capture_undo(&before, &after, "W", agent("mcp:x:1"), future());
        let c = after.kanban_cards.iter_mut().find(|c| c.id == "c1").unwrap();
        c.title = "renamed by Matt".into();
        c.notes = Some("note added".into());
        undo_claim_in(&mut after, &p).unwrap();
        let c = after.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.project_id, "A", "retarget undone");
        assert_eq!(c.notes.as_deref(), Some("note added"), "note kept");
        assert_eq!(c.title, "renamed by Matt", "unrelated title edit in the window was reverted");
    }

    /// Mirror of V1: a user touch of `last_touched_at` survives; an untouched title still restores.
    #[test]
    fn v1_user_last_touched_edit_on_retargeted_card_survives_undo() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "A"), project("B", "B")];
        before
            .kanban_cards
            .push(card_of("c1", "A", "", Some("W"), "doing", 1.0));
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let p = capture_undo(&before, &after, "W", agent("mcp:x:1"), future());
        let c = after.kanban_cards.iter_mut().find(|c| c.id == "c1").unwrap();
        c.last_touched_at = 42.0;
        undo_claim_in(&mut after, &p).unwrap();
        let c = after.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.project_id, "A", "retarget undone");
        assert_eq!(c.window_id.as_deref(), Some("W"));
        assert_eq!(c.last_touched_at, 42.0, "user last_touched_at edit survives");
        assert_eq!(c.title, "", "untouched title still restores");
    }

    /// N1. An agent claim must not burn a pending tray/app undo.
    #[test]
    fn agent_claim_never_overwrites_pending_user_undo() {
        let (w1, w2) = (win("W1", "mine"), win("W2", "agent"));
        let list = vec![w1.clone(), w2.clone()];
        let mut s0 = PersistedState::default();
        s0.projects = vec![project("A", "A")];
        let mut s1 = s0.clone();
        apply_claim(&mut s1, &w1, "A", &list);
        let mut slots = PendingSlots {
            user: None,
            agent: None,
        };
        arm_into(
            &mut slots,
            capture_undo(&s0, &s1, "W1", ClaimOrigin::User, future()),
        );
        let mut s2 = s1.clone();
        apply_claim(&mut s2, &w2, "A", &list);
        arm_into(
            &mut slots,
            capture_undo(&s1, &s2, "W2", agent("mcp:x:1"), future()),
        );
        let now = SystemTime::now();
        let which = which_pending(&slots, &ClaimOrigin::User, now);
        let p = try_take(slot_mut(&mut slots, which), &ClaimOrigin::User, now).unwrap();
        assert_eq!(p.window_id, "W1");
        undo_claim_in(&mut s2, &p).unwrap();
        assert!(
            !s2.claims.iter().any(|c| c.window_id == "W1"),
            "Matt's own claim undone"
        );
        assert!(
            s2.claims.iter().any(|c| c.window_id == "W2"),
            "agent claim untouched"
        );
        let now = SystemTime::now();
        let which = which_pending(&slots, &agent("mcp:x:1"), now);
        let p2 = try_take(slot_mut(&mut slots, which), &agent("mcp:x:1"), now).unwrap();
        assert_eq!(p2.window_id, "W2");
        undo_claim_in(&mut s2, &p2).unwrap();
        assert!(!s2.claims.iter().any(|c| c.window_id == "W2"));
    }

    /// C1. Expiry is inclusive at `now == expires` (`now > expires`).
    #[test]
    fn check_pending_exact_expiry_is_inclusive() {
        let e = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let p = pending(agent("mcp:x:1"), e);
        for verb in ["change", "undo"] {
            assert!(
                check_pending(&p, &agent("mcp:x:1"), e, verb).is_ok(),
                "{verb} at expiry"
            );
            let err = check_pending(
                &p,
                &agent("mcp:x:1"),
                e + Duration::from_millis(1),
                verb,
            )
            .unwrap_err();
            assert_eq!(err, format!("{verb} expired"));
        }
    }

    /// C4. Caller compare is case-sensitive.
    #[test]
    fn agent_compare_is_case_sensitive() {
        let now = UNIX_EPOCH;
        let p = pending(agent("mcp:Cursor:1"), future());
        for other in ["MCP:CURSOR:1", "mcp:cursor:1"] {
            for verb in ["undo", "change"] {
                assert_eq!(
                    check_pending(&p, &agent(other), now, verb).unwrap_err(),
                    "not your claim",
                    "{other:?} {verb}"
                );
            }
        }
    }

    /// C5. Caller compare does not trim.
    #[test]
    fn agent_compare_does_not_trim() {
        let now = UNIX_EPOCH;
        let p = pending(agent("mcp:Cursor:1"), future());
        for verb in ["undo", "change"] {
            assert_eq!(
                check_pending(&p, &agent(" mcp:Cursor:1"), now, verb).unwrap_err(),
                "not your claim",
                "{verb}"
            );
        }
    }

    fn strip_rust_comments(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' {
                out.push('"');
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    out.push(c);
                    i += 1;
                    if c == '\\' && i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
        let marker = format!("fn {name}(");
        let start = src.find(&marker).unwrap_or_else(|| panic!("missing {name}"));
        let bytes = src.as_bytes();
        let mut i = start;
        while i < bytes.len() && bytes[i] != b'{' {
            i += 1;
        }
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escape = false;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
                i += 1;
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &src[start..=i];
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("unclosed {name}");
    }

    /// C11. change_planet checks the pending slot before it re-claims.
    #[test]
    fn change_planet_checks_pending_before_claim_window() {
        let stripped = strip_rust_comments(include_str!("claim_front.rs"));
        let body = fn_body(&stripped, "change_planet");
        let check = body
            .find("check_pending(p, caller, SystemTime::now(), \"change\")?")
            .expect("check_pending");
        let claim = body.find("claim_window(").expect("claim_window");
        assert!(
            check < claim,
            "C11 change_planet must check_pending before claim_window"
        );
    }

    /// C13. undo_last takes through try_take, never a bare slot.take().
    #[test]
    fn undo_last_uses_try_take() {
        let stripped = strip_rust_comments(include_str!("claim_front.rs"));
        let body = fn_body(&stripped, "undo_last");
        assert!(body.contains("try_take("), "C13 undo_last must call try_take");
        assert!(
            !body.contains(".take()"),
            "C13 undo_last must not bare-take the slot:\n{body}"
        );
    }

    fn tab_spec(id: &str, title: &str) -> crate::tabbity::IncomingTab {
        crate::tabbity::IncomingTab {
            id: id.into(),
            browser: "brave".into(),
            url: "https://example.test/a".into(),
            title: title.into(),
            fav_icon_url: None,
        }
    }

    /// claim_tab arms the user slot. undo_last(User) is try_take + undo_claim_in.
    #[test]
    fn claim_tab_then_undo_last_user_restores_before() {
        let id = "tab:11111111-1111-4111-8111-111111111111";
        let id2 = "tab:22222222-2222-4222-8222-222222222222";
        let mut before = PersistedState::default();
        before.projects = vec![project("P", "Planet"), project("Q", "Other")];
        before.kanban_cards.push(card_of("card", "P", "linked", Some(id), "todo", 1.0));
        let mut after = before.clone();
        apply_tab_claims(
            &mut after,
            &[tab_spec(id, "One"), tab_spec(id2, "Two")],
            "Q",
        )
        .unwrap();
        assert_eq!(after.claims.len(), 2);
        assert!(after.claims.iter().all(|c| c.tab.is_some()));
        assert!(after.claims.iter().all(|c| c.bundle_id.is_none()));
        arm_ids(
            &before,
            &after,
            &[id.to_string(), id2.to_string()],
            ClaimOrigin::User,
        );
        {
            let mut slots = PENDING.lock().unwrap();
            let now = SystemTime::now();
            let which = which_pending(&slots, &agent("mcp"), now);
            assert!(try_take(slot_mut(&mut slots, which), &agent("mcp"), now).is_err());
        }
        let pending = {
            let mut slots = PENDING.lock().unwrap();
            let now = SystemTime::now();
            let which = which_pending(&slots, &ClaimOrigin::User, now);
            try_take(slot_mut(&mut slots, which), &ClaimOrigin::User, now).unwrap()
        };
        undo_claim_in(&mut after, &pending).unwrap();
        assert!(after.claims.is_empty());
        assert_eq!(after.kanban_cards[0].window_id.as_deref(), Some(id));
        assert_eq!(after.kanban_cards[0].project_id, "P");

        let mut moved = before.clone();
        apply_tab_claims(&mut moved, &[tab_spec(id, "One")], "P").unwrap();
        let seed = moved.claims[0].char_seed;
        apply_tab_claims(&mut moved, &[tab_spec(id, "One")], "Q").unwrap();
        assert_eq!(moved.claims[0].project_id, "Q");
        assert_eq!(moved.claims[0].char_seed, seed);
        assert!(moved.claims[0].tab.is_some());

        let mut claimed = moved.clone();
        unclaim_in(&mut claimed, id).unwrap();
        assert!(claimed.claims.is_empty());
        assert!(claimed.kanban_cards[0].window_id.is_none());
    }
