    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn touch_card(id: &str, touch: f64, notes: Option<&str>, links: Option<Vec<CardLink>>) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: id.into(),
            project_id: "p".into(),
            window_id: None,
            kind: Some("todo".into()),
            status: "todo".into(),
            created_at: 1.0,
            last_touched_at: touch,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: notes.map(str::to_string),
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: Some("agent-1".into()),
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links,
        }
    }

    fn run_row(card_id: &str) -> AgentRun {
        AgentRun {
            card_id: card_id.into(),
            planet_id: "p".into(),
            agent_shortcut_id: "agent-1".into(),
            state: "working".into(),
            herdr: None,
        }
    }

    fn temp_state() -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("orbitty-state-lock-{ms}-{n}-{}.json", std::process::id()))
    }

    fn update_file<T>(
        path: &std::path::Path,
        f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
    ) -> Result<T, String> {
        let _g = write_lock();
        update_at(path, f)
    }

    /// N4 coverage: every `PersistedState` / `CompanionState` field must be
    /// named in the writer-delta lists, or a native writer's change to it
    /// is silently dropped under the lock. Reads this file's own source.
    fn struct_fields(src: &str, header: &str) -> Vec<String> {
        let start = src.find(header).unwrap_or_else(|| panic!("no {header}"));
        let body = &src[start + header.len()..];
        let end = body.find("\n}").unwrap();
        body[..end]
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                let rest = t.strip_prefix("pub ")?;
                let (name, _) = rest.split_once(':')?;
                (!name.contains(' ') && !name.contains('(')).then(|| name.to_string())
            })
            .collect()
    }

    fn fn_body<'a>(src: &'a str, header: &str) -> &'a str {
        let start = src.find(header).unwrap_or_else(|| panic!("no {header}"));
        let body = &src[start..];
        let end = body.find("\n}\n").unwrap();
        &body[..end]
    }

    /// `live.<field>` as a whole identifier — `live.private_relay_url` must
    /// not count as naming `private_relay`.
    fn names_field(body: &str, field: &str) -> bool {
        let needle = format!("live.{field}");
        body.match_indices(&needle).any(|(i, _)| {
            body[i + needle.len()..]
                .chars()
                .next()
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'))
        })
    }

    #[test]
    fn names_field_is_word_bounded() {
        let body = "take_changed(&mut live.private_relay_url, &b, &a);";
        assert!(names_field(body, "private_relay_url"));
        assert!(!names_field(body, "private_relay"));
    }

    /// R3: a window holding a stale copy makes a real later edit (newer
    /// lastTouchedAt). Its edit wins; disk's native taskId/taskReason stay.
    #[test]
    fn newer_window_edit_keeps_disk_task_fields() {
        let mut disk = PersistedState::default();
        let mut d = touch_card("c1", 10.0, Some("old notes"), None);
        d.task_id = Some("t-abc".into());
        d.task_reason = Some("done".into());
        disk.kanban_cards.push(d);
        let mut incoming = PersistedState::default();
        // Window copy predates the stamp: no task fields, but a newer edit.
        incoming.kanban_cards.push(touch_card("c1", 20.0, Some("new notes"), None));
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.notes.as_deref(), Some("new notes"));
        assert_eq!(c.last_touched_at, 20.0);
        assert_eq!(c.task_id.as_deref(), Some("t-abc"));
        assert_eq!(c.task_reason.as_deref(), Some("done"));

        // A window can't clear or forge them either.
        let mut forged = incoming.clone();
        forged.kanban_cards[0].task_id = Some("t-forged".into());
        let (out, _) = merge_window_save(&disk, &forged);
        assert_eq!(out.kanban_cards[0].task_id.as_deref(), Some("t-abc"));
    }

    /// F2: a window card that is not on disk must not keep a stale taskId.
    #[test]
    fn window_card_not_on_disk_clears_task_fields() {
        let disk = PersistedState::default();
        let mut incoming = PersistedState::default();
        let mut c = touch_card("c-new", 5.0, None, None);
        c.task_id = Some("t-stale".into());
        c.task_reason = Some("done".into());
        incoming.kanban_cards.push(c);
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c-new").unwrap();
        assert!(c.task_id.is_none());
        assert!(c.task_reason.is_none());

        // Newer window edit resurrects a card Rust deleted. The stale
        // task fields on that echo are dropped.
        let mut disk = PersistedState::default();
        disk.card_tombstones.push(CardTombstone {
            id: "c-old".into(),
            last_touched_at: 3.0,
        });
        let mut incoming = PersistedState::default();
        let mut c = touch_card("c-old", 9.0, Some("back"), None);
        c.task_id = Some("t-ghost".into());
        c.task_reason = Some("done".into());
        incoming.kanban_cards.push(c);
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c-old").unwrap();
        assert!(c.task_id.is_none());
        assert!(c.task_reason.is_none());
        assert_eq!(c.notes.as_deref(), Some("back"));
    }

    #[test]
    fn agent_terminal_explicit_window_wins_omitted_keeps_disk() {
        let mut disk = PersistedState::default();
        disk.agent_terminal = "terminal".into();
        let mut incoming = PersistedState::default();
        incoming.agent_terminal = "ghostty".into();
        let (out, _) = merge_window_save(&disk, &incoming);
        assert_eq!(out.agent_terminal, "ghostty");

        let mut omitted = PersistedState::default();
        omitted.agent_terminal.clear();
        let (out, _) = merge_window_save(&disk, &omitted);
        assert_eq!(out.agent_terminal, "terminal");

        let before = PersistedState::default();
        let mut after = before.clone();
        after.agent_terminal = "terminal".into();
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.agent_terminal, "terminal");
    }

    #[test]
    fn writer_delta_lists_cover_every_field() {
        let src = concat!(
include_str!("mod.rs"),
include_str!("rail_geom.rs"),
include_str!("models.rs"),
include_str!("models_companion.rs"),
include_str!("models_board.rs"),
include_str!("io.rs"),
include_str!("merge.rs")
);
        let top = struct_fields(src, "pub struct PersistedState {");
        let comp = struct_fields(src, "pub struct CompanionState {");
        assert!(top.len() >= 40 && comp.len() >= 20, "parser found {} / {}", top.len(), comp.len());
        let writer = fn_body(src, "pub fn apply_writer_delta(");
        let companion = fn_body(src, "fn apply_companion_delta(");
        let miss_top: Vec<_> = top
            .iter()
            .filter(|f| !names_field(writer, f))
            .collect();
        let miss_comp: Vec<_> = comp
            .iter()
            .filter(|f| !names_field(companion, f))
            .collect();
        assert!(
            miss_top.is_empty() && miss_comp.is_empty(),
            "add to apply_writer_delta: {miss_top:?}; to apply_companion_delta: {miss_comp:?}"
        );
    }

    /// Old saves that picked the removed 3D body still load and land on
    /// the Spirit; other picks and unknown old fields are untouched/ignored.
    #[test]
    fn old_save_with_retired_body_loads_as_spirit() {
        let path = temp_state();
        std::fs::write(
            &path,
            r#"{"themeId":"x","companion":{"imagePackId":"tang","lookAtTarget":{"x":1},"lookAt":true},"kanbanCards":[]}"#,
        )
        .unwrap();
        let st = load_path(&path).expect("old save loads");
        assert_eq!(st.companion.image_pack_id, None);
        std::fs::write(&path, r#"{"companion":{"imagePackId":"female"}}"#).unwrap();
        assert_eq!(load_path(&path).unwrap().companion.image_pack_id.as_deref(), Some("female"));
        std::fs::write(&path, r#"{"companion":{"imagePackId":"my-pack"}}"#).unwrap();
        assert_eq!(load_path(&path).unwrap().companion.image_pack_id.as_deref(), Some("my-pack"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn eight_threads_keep_every_write() {
        let path = temp_state();
        let _ = std::fs::remove_file(&path);
        std::thread::scope(|s| {
            for i in 0..8 {
                let path = path.clone();
                s.spawn(move || {
                    update_file(&path, |st| {
                        st.kanban_cards.push(touch_card(&format!("c{i}"), i as f64, None, None));
                        Ok(())
                    })
                    .unwrap();
                });
            }
        });
        let st = load_path(&path).unwrap();
        let mut ids: Vec<_> = st.kanban_cards.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7"]);
        let _ = std::fs::remove_file(&path);
    }

    /// H2: a second holder of `<path>.lock` (another process in real life)
    /// blocks the save until it lets go.
    #[test]
    fn update_waits_for_a_second_holder_of_the_file_lock() {
        let path = temp_state();
        let _ = std::fs::remove_file(&path);
        let other = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(lock_path(&path))
            .unwrap();
        other.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            let out = update_at(&writer_path, |st| {
                st.kanban_cards.push(touch_card("c-locked", 1.0, None, None));
                Ok(())
            });
            let _ = tx.send(out);
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(200)).is_err(),
            "update_at returned while another holder had the file lock"
        );
        assert!(load_path(&path).unwrap().kanban_cards.is_empty());
        other.unlock().unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .expect("update_at finishes once the lock is free")
            .unwrap();
        writer.join().unwrap();
        let st = load_path(&path).unwrap();
        assert!(st.kanban_cards.iter().any(|c| c.id == "c-locked"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(lock_path(&path));
    }

    #[test]
    fn update_releases_the_file_lock_on_err() {
        let path = temp_state();
        let err: Result<(), String> = update_at(&path, |_| Err("nope".into()));
        assert_eq!(err.unwrap_err(), "nope");
        let probe = std::fs::OpenOptions::new()
            .write(true)
            .open(lock_path(&path))
            .unwrap();
        probe.try_lock().expect("the Err path released the lock");
        let _ = std::fs::remove_file(lock_path(&path));
    }

    #[test]
    fn update_takes_mutex_then_file_lock() {
        let src = include_str!("io.rs");
        let body = fn_body(src, "pub fn update<T>(");
        let mutex = body.find("write_lock()").expect("mutex");
        let file = body.find("lock_state_file(").expect("file lock");
        let load = body.find("load_state(").expect("load");
        assert!(mutex < file && file < load, "{body}");
        let at = fn_body(src, "pub(crate) fn update_at<T>(");
        assert!(at.find("lock_state_file(").unwrap() < at.find("load_path(").unwrap());
    }

    #[test]
    fn pickup_race_keeps_run_row_200() {
        let path = temp_state();
        for i in 0..200 {
            save_path(
                &path,
                &PersistedState {
                    kanban_cards: vec![touch_card("c1", 1.0, None, None)],
                    ..PersistedState::default()
                },
            )
            .unwrap();
            std::thread::scope(|s| {
                let path_run = path.clone();
                s.spawn(move || {
                    update_file(&path_run, |st| {
                        st.agent_runs.push(run_row("c1"));
                        Ok(())
                    })
                    .unwrap();
                });
                let path_hack = path.clone();
                s.spawn(move || {
                    update_file(&path_hack, |st| {
                        st.companion.hack_live = true;
                        st.companion.hack_who = "cursor".into();
                        st.companion.hack_project_id = Some("p".into());
                        Ok(())
                    })
                    .unwrap();
                });
            });
            let st = load_path(&path).unwrap();
            assert!(
                st.agent_runs.iter().any(|r| r.card_id == "c1"),
                "run dropped on iter {i}"
            );
            assert!(st.companion.hack_live, "hack cleared on iter {i}");
            assert_eq!(st.companion.hack_who, "cursor");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stale_hack_snapshot_does_not_drop_run() {
        let mut live = PersistedState::default();
        live.agent_runs.push(run_row("c1"));
        live.kanban_cards.push(touch_card("c1", 5.0, Some("disk"), None));
        let before = PersistedState {
            kanban_cards: vec![touch_card("c1", 5.0, Some("disk"), None)],
            ..PersistedState::default()
        };
        let mut after = before.clone();
        after.companion.hack_live = true;
        after.companion.hack_who = "cursor".into();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.agent_runs.len(), 1);
        assert!(live.companion.hack_live);
        assert_eq!(live.kanban_cards[0].notes.as_deref(), Some("disk"));
    }

    #[test]
    fn writer_run_delete_still_drops_the_row() {
        let mut live = PersistedState::default();
        live.agent_runs.push(run_row("c1"));
        let before = PersistedState {
            agent_runs: vec![run_row("c1")],
            ..PersistedState::default()
        };
        let after = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert!(live.agent_runs.is_empty());
    }

    #[test]
    fn fresh_handoff_is_empty_and_off() {
        let c = CompanionState::default();
        assert!(c.handoff_backend.is_empty());
        assert!(!c.auto_handoff);
        let partial: CompanionState =
            serde_json::from_str(r#"{"visible":true,"freeMode":true}"#).unwrap();
        assert!(partial.handoff_backend.is_empty());
        assert!(!partial.auto_handoff);
        assert!(partial.free_mode);
        let saved = serde_json::to_value(&c).unwrap();
        assert_eq!(saved["handoffBackend"], "");
        assert_eq!(saved["autoHandoff"], false);
    }

    #[test]
    fn stale_window_cannot_strip_handoff() {
        let mut disk = PersistedState::default();
        disk.companion.handoff_backend = "goose".into();
        disk.companion.auto_handoff = true;
        let mut incoming = disk.clone();
        incoming.companion.handoff_backend.clear();
        incoming.companion.auto_handoff = false;
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.companion.handoff_backend, "goose");
        assert!(merged.companion.auto_handoff);

        let before = PersistedState::default();
        let mut after = before.clone();
        after.companion.reply_lane = "local".into();
        let mut live = PersistedState::default();
        live.companion.handoff_backend = "grok".into();
        live.companion.auto_handoff = true;
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.companion.handoff_backend, "grok");
        assert!(live.companion.auto_handoff);
        assert_eq!(live.companion.reply_lane, "local");
    }

    #[test]
    fn fresh_backend_is_empty_and_stale_window_cannot_strip_it() {
        let c = CompanionState::default();
        assert!(c.backend.is_empty());
        let partial: CompanionState = serde_json::from_str(r#"{"visible":true}"#).unwrap();
        assert!(partial.backend.is_empty());
        let saved = serde_json::to_value(&c).unwrap();
        assert_eq!(saved["backend"], "");

        let mut disk = PersistedState::default();
        disk.companion.backend = "hermes".into();
        let mut incoming = disk.clone();
        incoming.companion.backend.clear();
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.companion.backend, "hermes");

        let before = PersistedState::default();
        let after = before.clone();
        let mut live = PersistedState::default();
        live.companion.backend = "hermes".into();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.companion.backend, "hermes");

        let mut changed = before.clone();
        changed.companion.backend = "grok".into();
        apply_writer_delta(&mut live, &before, &changed);
        assert_eq!(live.companion.backend, "grok");
    }

    #[test]
    fn direct_endpoint_fields_are_covered_and_the_key_is_not() {
        let src = concat!(
include_str!("mod.rs"),
include_str!("rail_geom.rs"),
include_str!("models.rs"),
include_str!("models_companion.rs"),
include_str!("models_board.rs"),
include_str!("io.rs"),
include_str!("merge.rs")
);
        let comp = struct_fields(src, "pub struct CompanionState {");
        assert!(comp.iter().any(|f| f == "direct_base_url"));
        assert!(comp.iter().any(|f| f == "direct_model"));
        assert!(comp.iter().all(|f| !f.to_ascii_lowercase().contains("key")));
        let companion = fn_body(src, "fn apply_companion_delta(");
        assert!(names_field(companion, "direct_base_url"));
        assert!(names_field(companion, "direct_model"));
        assert!(!companion.contains("direct_key"));
        let c = CompanionState::default();
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["directBaseUrl"], "");
        assert_eq!(v["directModel"], "");
        assert!(v.get("directKey").is_none());
    }

    #[test]
    fn window_merge_stale_card_loses_and_rust_slices_stay() {
        let mut disk = PersistedState::default();
        disk.kanban_cards.push(touch_card(
            "c1",
            10.0,
            Some("newer notes"),
            Some(vec![CardLink {
                url: "https://example.com/a".into(),
                label: None,
            }]),
        ));
        disk.agent_runs.push(run_row("c1"));
        disk.companion.hack_live = true;
        disk.companion.hack_who = "cursor".into();
        disk.companion.hack_project_id = Some("p".into());
        disk.companion.free_mode = true;
        disk.companion.conductor_on = true;
        disk.companion.conductor_model = "built-in".into();
        disk.companion.reply_lane = "free".into();
        disk.companion.reply_model = "e4b".into();
        disk.companion.handoff_backend = "goose".into();
        disk.companion.auto_handoff = true;
        disk.companion.backend = "hermes".into();
        disk.companion.direct_base_url = "https://example.test/v1".into();
        disk.companion.direct_model = "m".into();
        disk.shift_on = true;
        disk.peer_chrome = true;
        disk.last_claim_project_id = Some("planet".into());

        let mut incoming = disk.clone();
        incoming.kanban_cards[0] = touch_card("c1", 4.0, Some("stale"), None);
        incoming.agent_runs.clear();
        incoming.companion.hack_live = false;
        incoming.companion.hack_who.clear();
        incoming.companion.hack_project_id = None;
        incoming.companion.free_mode = false;
        incoming.companion.conductor_on = false;
        incoming.companion.conductor_model.clear();
        incoming.companion.reply_lane.clear();
        incoming.companion.reply_model.clear();
        incoming.companion.handoff_backend.clear();
        incoming.companion.auto_handoff = false;
        incoming.companion.backend.clear();
        incoming.companion.direct_base_url.clear();
        incoming.companion.direct_model.clear();
        incoming.shift_on = false;
        incoming.peer_chrome = false;
        incoming.last_claim_project_id = None;

        let (merged, stats) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.kanban_cards[0].notes.as_deref(), Some("newer notes"));
        assert_eq!(
            merged.kanban_cards[0].links.as_ref().map(|l| l.len()),
            Some(1)
        );
        assert_eq!(merged.agent_runs.len(), 1);
        assert!(merged.companion.hack_live);
        assert_eq!(merged.companion.hack_who, "cursor");
        assert_eq!(merged.companion.hack_project_id.as_deref(), Some("p"));
        assert!(merged.companion.free_mode);
        assert!(merged.companion.conductor_on);
        assert_eq!(merged.companion.conductor_model, "built-in");
        assert_eq!(merged.companion.reply_lane, "free");
        assert_eq!(merged.companion.reply_model, "e4b");
        assert_eq!(merged.companion.handoff_backend, "goose");
        assert!(merged.companion.auto_handoff);
        assert_eq!(merged.companion.backend, "hermes");
        assert_eq!(merged.companion.direct_base_url, "https://example.test/v1");
        assert_eq!(merged.companion.direct_model, "m");
        assert!(merged.shift_on);
        assert!(merged.peer_chrome);
        assert_eq!(merged.last_claim_project_id.as_deref(), Some("planet"));
        assert_eq!(stats.kept_disk, 1);
        assert_eq!(stats.took_incoming, 0);
    }

    #[test]
    fn window_merge_newer_delete_and_link_remove() {
        let mut disk = PersistedState::default();
        disk.kanban_cards.push(touch_card(
            "keep",
            3.0,
            None,
            Some(vec![CardLink {
                url: "https://example.com/old".into(),
                label: None,
            }]),
        ));
        disk.kanban_cards.push(touch_card("gone", 2.0, None, None));
        disk.kanban_cards.push(touch_card("stay", 8.0, Some("disk"), None));

        let mut incoming = PersistedState::default();
        // Link remove bumps lastTouchedAt, so the incoming card is newer.
        let mut edited = touch_card("keep", 9.0, None, None);
        edited.links = None;
        incoming.kanban_cards.push(edited);
        incoming.kanban_cards.push(touch_card("stay", 1.0, Some("stale rail"), None));
        incoming.card_tombstones.push(CardTombstone {
            id: "gone".into(),
            last_touched_at: 4.0,
        });
        // A card the window never saw must survive an omit with no tombstone.
        // `stay` is present but older — disk wins. Add `fresh` only on disk
        // by not listing it here; it is on disk below.
        disk.kanban_cards.push(touch_card("fresh", 6.0, Some("born on disk"), None));

        let (merged, stats) = merge_window_save(&disk, &incoming);
        let ids: Vec<_> = merged.kanban_cards.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"keep"), "{ids:?}");
        assert!(!ids.contains(&"gone"), "delete should win: {ids:?}");
        assert!(ids.contains(&"fresh"), "omit without tombstone resurrected by absence: {ids:?}");
        let keep = merged.kanban_cards.iter().find(|c| c.id == "keep").unwrap();
        assert!(keep.links.is_none(), "newer link remove should stick");
        let stay = merged.kanban_cards.iter().find(|c| c.id == "stay").unwrap();
        assert_eq!(stay.notes.as_deref(), Some("disk"));
        assert!(stats.took_incoming >= 1);
        assert!(stats.kept_disk >= 1);
        assert!(merged.card_tombstones.iter().any(|t| t.id == "gone"));
    }

    #[test]
    fn window_merge_old_tombstone_does_not_block_newer_card() {
        let mut disk = PersistedState::default();
        disk.card_tombstones.push(CardTombstone {
            id: "c1".into(),
            last_touched_at: 2.0,
        });
        let mut incoming = PersistedState::default();
        incoming.kanban_cards.push(touch_card("c1", 5.0, Some("back"), None));
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.kanban_cards.len(), 1);
        assert!(merged.card_tombstones.iter().all(|t| t.id != "c1"));
    }

    #[test]
    fn rail_board_cols_default_and_delta() {
        let missing: PersistedState = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.rail_board_cols, vec!["todo".to_string()]);

        let round: PersistedState =
            serde_json::from_str(r#"{"railBoardCols":["todo","done"]}"#).unwrap();
        assert_eq!(
            round.rail_board_cols,
            vec!["todo".to_string(), "done".to_string()]
        );
        let encoded = serde_json::to_value(&round).unwrap();
        assert_eq!(encoded["railBoardCols"], serde_json::json!(["todo", "done"]));

        let mut before = PersistedState::default();
        before.rail_board_cols = vec!["doing".into()];
        let mut after = before.clone();
        after.rail_board_cols = vec!["done".into()];
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.rail_board_cols, vec!["done".to_string()]);

        let mut kept = PersistedState::default();
        kept.rail_board_cols = vec!["keep".into()];
        apply_writer_delta(&mut kept, &after, &after);
        assert_eq!(kept.rail_board_cols, vec!["keep".to_string()]);
    }

    #[test]
    fn rail_stage_prefs_default_left_and_survive_window_save() {
        let missing: PersistedState = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.rail_edge, "left");
        assert!(missing.rail_auto_hide_on_blur);
        assert!(missing.autohide_row_chrome);
        assert_eq!(normalize_rail_edge("nope"), "left");
        assert_eq!(normalize_rail_edge("right"), "right");

        let mut disk = PersistedState::default();
        disk.rail_edge = "bottom".into();
        disk.rail_auto_hide_on_blur = false;
        disk.autohide_row_chrome = true;
        let mut incoming = PersistedState::default();
        incoming.rail_edge = "left".into();
        incoming.rail_auto_hide_on_blur = true;
        incoming.autohide_row_chrome = false;
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.rail_edge, "bottom");
        assert!(!merged.rail_auto_hide_on_blur);
        assert!(!merged.autohide_row_chrome);

        let (x, y, w, h) = rail_stage_frame("left", false, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!((x, y, w, h), (-348.0, 240.0, 360.0, 420.0));
        let (x, _, _, _) = rail_stage_frame("left", true, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(x, 8.0);
        let (x, _, _, _) = rail_stage_frame("right", false, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(x, 1388.0);
        let (_, y, _, _) = rail_stage_frame("top", false, 0.0, 20.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(y, 20.0 - 420.0 + 12.0);
        let (_, y, _, _) = rail_stage_frame("bottom", true, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(y, 900.0 - 420.0 - 8.0);

        let before = PersistedState::default();
        let mut after = before.clone();
        after.rail_edge = "top".into();
        after.rail_auto_hide_on_blur = false;
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.rail_edge, "top");
        assert!(!live.rail_auto_hide_on_blur);
    }

    #[test]
    fn rail_wheel_size_hugs_wheel_and_board() {
        assert_eq!(rail_wheel_size("left", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("right", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("top", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("bottom", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("left", true, 1000.0, 700.0), (992.0, 700.0));
        assert_eq!(rail_wheel_size("top", true, 1000.0, 700.0), (1000.0, 692.0));
        assert_eq!(rail_wheel_size("left", false, 1440.0, 875.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("right", false, 100.0, 100.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("top", false, 1440.0, 875.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("bottom", false, 8.0, 8.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("nope", true, 1440.0, 875.0), (1040.0, 760.0));
        let (x, y, w, h) = rail_stage_frame("left", true, 0.0, 0.0, 1440.0, 875.0, 1040.0, 760.0);
        assert_eq!((x, y, w, h), (8.0, 57.5, 1040.0, 760.0));
    }

    #[test]
    fn starter_story_is_rust_owned_and_survives_window_save() {
        let mut disk = PersistedState::default();
        disk.starter_story.v = 1;
        disk.starter_story.seeded_project_id = Some("p1".into());
        disk.starter_story.comms_built = true;
        disk.starter_story.stars_built = true;
        disk.starter_story.coach_done = true;
        disk.starter_story.intro_done = false;
        let incoming = PersistedState::default();
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.starter_story.v, 1);
        assert_eq!(merged.starter_story.seeded_project_id.as_deref(), Some("p1"));
        assert!(merged.starter_story.comms_built);
        assert!(merged.starter_story.stars_built);
        assert!(merged.starter_story.coach_done);
        assert!(!merged.starter_story.intro_done);
        assert!(!merged.starter_story.skipped);
    }

    #[test]
    fn starter_story_serializes_to_the_camel_case_key_contract() {
        let story = StarterStory {
            v: 1,
            skipped: true,
            coach_done: true,
            seeded_project_id: Some("p".into()),
            comms_built: true,
            stars_built: true,
            intro_done: true,
        };
        let value = serde_json::to_value(&story).unwrap();
        let obj = value.as_object().unwrap();
        let keys: std::collections::BTreeSet<_> = obj.keys().cloned().collect();
        let expect: std::collections::BTreeSet<_> = [
            "v",
            "skipped",
            "coachDone",
            "seededProjectId",
            "commsBuilt",
            "starsBuilt",
            "introDone",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(keys, expect);
        assert_eq!(obj.get("v").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(obj.get("skipped").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("coachDone").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            obj.get("seededProjectId").and_then(|v| v.as_str()),
            Some("p")
        );
        assert_eq!(obj.get("commsBuilt").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("starsBuilt").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("introDone").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn ledger_page_day_key_end_round_trip() {
        let page = LedgerPage {
            id: "led-span".into(),
            created_at: 1_700_000_000_000.0,
            day_key: Some("2026-09-28".into()),
            day_key_end: Some("2026-10-04".into()),
            scope: Some("overall".into()),
            body: Some("a week".into()),
            metric_flags: None,
        };
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("dayKeyEnd"), "{json}");
        let back: LedgerPage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.day_key.as_deref(), Some("2026-09-28"));
        assert_eq!(back.day_key_end.as_deref(), Some("2026-10-04"));
        assert_eq!(back.scope.as_deref(), Some("overall"));
    }

    #[test]
    fn ledger_page_without_end_loads() {
        let json = r#"{"id":"led-old","createdAt":10,"dayKey":"2026-09-01","scope":"overall","body":"old"}"#;
        let page: LedgerPage = serde_json::from_str(json).unwrap();
        assert!(page.day_key_end.is_none());
        assert_eq!(page.day_key.as_deref(), Some("2026-09-01"));
        assert_eq!(page.body.as_deref(), Some("old"));
        let out = serde_json::to_string(&page).unwrap();
        assert!(!out.contains("dayKeyEnd"), "{out}");
    }

    #[test]
    fn tracker_history_round_trip() {
        let mut days = std::collections::BTreeMap::new();
        days.insert("2026-10-07".to_string(), 12);
        let tracker = Tracker {
            id: "tr1".into(),
            label: "Steps".into(),
            value: 12,
            kind: Some("number".into()),
            running_since: None,
            history: Some(TrackerHistory {
                days: Some(days),
                sessions: Some(TrackerSessions {
                    day: "2026-10-07".into(),
                    spans: vec![(1_700_000_000_000, 1_700_000_060_000)],
                }),
            }),
        };
        let json = serde_json::to_string(&tracker).unwrap();
        assert!(json.contains("history"), "{json}");
        assert!(json.contains("2026-10-07"), "{json}");
        assert!(json.contains("spans"), "{json}");
        let back: Tracker = serde_json::from_str(&json).unwrap();
        let hist = back.history.expect("history");
        assert_eq!(hist.days.expect("days").get("2026-10-07"), Some(&12));
        let sessions = hist.sessions.expect("sessions");
        assert_eq!(sessions.day, "2026-10-07");
        assert_eq!(sessions.spans, vec![(1_700_000_000_000, 1_700_000_060_000)]);
        assert!(back.running_since.is_none());
    }

    #[test]
    fn tracker_without_history_loads() {
        let json = r#"{"id":"tr-old","label":"Old","value":3}"#;
        let tracker: Tracker = serde_json::from_str(json).unwrap();
        assert!(tracker.history.is_none());
        assert_eq!(tracker.value, 3);
        assert_eq!(tracker.label, "Old");
        let out = serde_json::to_string(&tracker).unwrap();
        assert!(!out.contains("history"), "{out}");
        assert!(!out.contains("runningSince"), "{out}");
    }

    /// Synthetic board shaped like a heavy desk: 626 cards, about 650 KB
    /// on disk. Never built from a real state.json.
    pub(crate) fn fixture_626() -> PersistedState {
        let mut st = PersistedState::default();
        for p in 0..12 {
            st.projects.push(Project {
                id: format!("p-{p}"),
                name: format!("Planet {p}"),
                trackers: None,
                workspace_root: None,
            });
        }
        let note = "Synthetic note line for the timing fixture. ".repeat(9);
        for i in 0..626 {
            let mut c = touch_card(&format!("c-{i:04}"), i as f64, Some(&note), None);
            c.title = format!("Synthetic card number {i} with a realistic title");
            c.project_id = format!("p-{}", i % 12);
            c.status = ["todo", "doing", "done"][i % 3].into();
            c.tag_ids = Some(vec!["t-1".into(), "t-2".into()]);
            c.subtasks = Some(vec![Subtask {
                id: format!("s-{i}"),
                title: "Synthetic subtask".into(),
                done: i % 2 == 0,
            }]);
            st.kanban_cards.push(c);
        }
        for i in 0..900 {
            st.points_ledger.push(PointsEntry {
                id: format!("pt-{i}"),
                card_id: format!("c-{:04}", i % 626),
                amount: 1,
                day: "2026-10-01".into(),
                at: i as f64,
                actor: None,
            });
        }
        st
    }

    fn median_p95(mut ms: Vec<f64>) -> (f64, f64) {
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = ms[((ms.len() as f64) * 0.95).ceil() as usize - 1];
        (ms[ms.len() / 2], p95)
    }

    /// Phase-0 / after timing of the save door on the synthetic fixture.
    /// `cargo test --lib update_timing_626 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn update_timing_626_cards() {
        let path = temp_state();
        save_path(&path, &fixture_626()).unwrap();
        let bytes = std::fs::metadata(&path).unwrap().len();
        let runs = 60;
        let mut noop = Vec::new();
        for _ in 0..runs {
            let t = std::time::Instant::now();
            update_file(&path, |_| Ok(())).unwrap();
            noop.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut title = Vec::new();
        for i in 0..runs {
            let t = std::time::Instant::now();
            update_file(&path, |st| {
                st.kanban_cards[7].title = format!("Renamed {i}");
                Ok(())
            })
            .unwrap();
            title.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let (nm, np) = median_p95(noop);
        let (tm, tp) = median_p95(title);
        println!("fixture bytes={bytes} cards=626 runs={runs}");
        println!("noop  median={nm:.2}ms p95={np:.2}ms");
        println!("title median={tm:.2}ms p95={tp:.2}ms");
        let _ = std::fs::remove_file(&path);
    }
