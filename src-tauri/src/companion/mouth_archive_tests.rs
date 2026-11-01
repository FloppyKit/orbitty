use super::*;
use crate::state::{CompanionState, CompanionTurn, COMPANION_THREAD_CAP};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn on() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orbitty-mh-archive-{t}-{n}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        install_root(Some(root.clone()));
        force_flag(Some(true));
        reset_warn();
        Self { root }
    }

    fn lock_archive(&self) {
        let dir = self.root.join("archive");
        fs::create_dir_all(&dir).unwrap();
        let mut p = fs::metadata(&dir).unwrap().permissions();
        p.set_mode(0o555);
        fs::set_permissions(&dir, p).unwrap();
    }

    fn unlock_archive(&self) {
        let dir = self.root.join("archive");
        if !dir.exists() {
            return;
        }
        let mut p = fs::metadata(&dir).unwrap().permissions();
        p.set_mode(0o700);
        fs::set_permissions(&dir, p).unwrap();
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.unlock_archive();
        let _ = fs::remove_dir_all(&self.root);
        force_flag(None);
        install_root(None);
        reset_warn();
    }
}

fn lines(root: &std::path::Path) -> Vec<ArchiveLine> {
    tail(root, usize::MAX, None)
}

#[test]
fn archive_label_is_answering_lane() {
    let local = "mlx-community/gemma-4-e4b-it-mlx";
    let (lane, model) = label_from_answer("grok", GROK_CLI_MODEL, local);
    assert_eq!(lane, "grok");
    assert_eq!(model, GROK_CLI_MODEL);
    assert_ne!(model, local);
    // A grok answer must not keep the Settings local pick.
    let (lane, model) = label_from_answer("grok", local, local);
    assert_eq!(lane, "grok");
    assert_ne!(model, local);
    assert_eq!(model, GROK_CLI_MODEL);
    let (lane, model) = label_from_answer("local", local, local);
    assert_eq!(lane, "local");
    assert_eq!(model, local);

    let sb = Sandbox::on();
    note_answered("grok", GROK_CLI_MODEL);
    let (lane, model) = take_answered().expect("grok slot");
    let (lane, model) = label_from_answer(&lane, &model, local);
    let _g = TurnGuard::set(TurnMeta {
        lane,
        model,
        session: Some("5a2426e0-047f-4974-a833-38ca74f4bc2c".into()),
        src: "live".into(),
    });
    let mut st = CompanionState::default();
    st.push_turn("them", "LABEL-GROK");
    drop(_g);
    let them = lines(&sb.root)
        .into_iter()
        .find(|r| r.text == "LABEL-GROK")
        .expect("grok line");
    assert_eq!(them.lane, "grok");
    assert_eq!(them.model, GROK_CLI_MODEL);
    assert_ne!(them.model, local);

    note_answered("local", local);
    let (lane, model) = take_answered().expect("local slot");
    let (lane, model) = label_from_answer(&lane, &model, local);
    let _g = TurnGuard::set(TurnMeta {
        lane,
        model,
        session: None,
        src: "live".into(),
    });
    st.push_turn("them", "LABEL-LOCAL");
    drop(_g);
    let them = lines(&sb.root)
        .into_iter()
        .find(|r| r.text == "LABEL-LOCAL")
        .expect("local line");
    assert_eq!(them.lane, "local");
    assert_eq!(them.model, local);

    let send = include_str!("../coordinator/send.rs");
    let start = send
        .find("pub(crate) async fn companion_send_inner(")
        .expect("send");
    let rest = &send[start..];
    let end = rest
        .find("\npub(crate) async fn companion_send_berd(")
        .expect("berd");
    let body = &rest[..end];
    assert!(body.contains("pin_reply"), "reply archive must not use Settings pin");
    assert!(body.contains("note_answered"));
    let pin = send.find("fn pin_reply(").expect("pin_reply");
    let pin_body = &send[pin..pin + 500];
    assert!(pin_body.contains("take_answered"));
    assert!(pin_body.contains("label_from_answer"));
}

#[test]
fn ct_day_rolls_at_midnight() {
    // 2026-10-30 23:59:59 CDT and the next second.
    let before = chrono::DateTime::from_timestamp(1_793_422_799, 0).unwrap();
    let after = chrono::DateTime::from_timestamp(1_793_422_800, 0).unwrap();
    let (day_before, ts_before) = ct_stamp(before);
    let (day_after, ts_after) = ct_stamp(after);
    assert_eq!(day_before, "2026-10-30");
    assert!(ts_before.ends_with("-05:00"), "{ts_before}");
    assert_eq!(day_after, "2026-10-31");
    assert!(ts_after.ends_with("-05:00"), "{ts_after}");
    // January is CST.
    let jan = chrono::DateTime::from_timestamp(1_768_456_800, 0).unwrap();
    let (day, ts) = ct_stamp(jan);
    assert_eq!(day, "2026-01-15");
    assert!(ts.ends_with("-06:00"), "{ts}");
}

#[test]
fn archive_before_drain() {
    let box_ = Sandbox::on();
    let mut st = CompanionState::default();
    for i in 0..41 {
        st.push_turn("you", &format!("line {i}"));
    }
    assert_eq!(st.turns.len(), COMPANION_THREAD_CAP);
    assert_eq!(st.turns[0].text, "line 1");
    assert_eq!(st.turns[39].text, "line 40");
    let got = lines(&box_.root);
    assert_eq!(got.len(), 41, "archive keeps the drained turn");
    assert_eq!(got[0].text, "line 0");
    assert_eq!(got[40].text, "line 40");
    assert_eq!(got[0].src, "live");
    assert_eq!(got[0].v, 1);
    assert!(got[0].ts.contains("-05:00") || got[0].ts.contains("-06:00"));
    assert!(marker_exists(&box_.root));
    let mode = fs::metadata(box_.root.join("archive")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    let day = ct_stamp(chrono::Utc::now()).0;
    let file = box_.root.join("archive").join(format!("{day}.jsonl"));
    let fmode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(fmode, 0o600);
}

#[test]
fn append_fail_holds_drain() {
    let box_ = Sandbox::on();
    box_.lock_archive();
    let mut st = CompanionState::default();
    for i in 0..45 {
        st.push_turn("you", &format!("t{i}"));
    }
    assert_eq!(st.turns.len(), 45);
    assert_eq!(warn_count(), 1);
    assert!(lines(&box_.root).is_empty());
    box_.unlock_archive();
    st.push_turn("you", "after");
    assert_eq!(st.turns.len(), COMPANION_THREAD_CAP);
    let got = lines(&box_.root);
    assert!(got.len() >= 46, "held tail then the recovery push, got {}", got.len());
    assert_eq!(got[0].text, "t0");
    assert_eq!(got[44].text, "t44");
    assert_eq!(got[45].text, "after");
    assert_eq!(st.turns.last().unwrap().text, "after");
}

#[test]
fn backfill_once_idempotent() {
    let box_ = Sandbox::on();
    let mut st = CompanionState::default();
    st.mode = "spark".into();
    for i in 0..40 {
        st.turns.push(CompanionTurn {
            role: "you".into(),
            text: format!("live {i}"),
        });
    }
    let mut extra = 0usize;
    for (mode, n) in [("spark", 10), ("chat", 22), ("work", 16), ("hold", 20)] {
        let mut rows = Vec::new();
        for i in 0..n {
            rows.push(CompanionTurn {
                role: "them".into(),
                text: format!("parked {mode} {i}"),
            });
            extra += 1;
        }
        st.threads.insert(mode.into(), rows);
    }
    assert_eq!(extra, 68);
    let n = backfill_once(&box_.root, &st).unwrap();
    assert_eq!(n, 108);
    let got = lines(&box_.root);
    assert_eq!(got.len(), 108);
    assert_eq!(got[0].src, "backfill:state.turns");
    assert_eq!(got[0].text, "live 0");
    assert_eq!(got[39].src, "backfill:state.turns");
    assert_eq!(got[40].src, "backfill:state.threads.spark");
    assert_eq!(got[40].mode, "spark");
    assert!(got.iter().any(|r| r.src == "backfill:state.threads.chat"));
    assert!(got.iter().any(|r| r.src == "backfill:state.threads.work"));
    assert!(got.iter().any(|r| r.src == "backfill:state.threads.hold"));
    assert!(marker_exists(&box_.root));
    let again = backfill_once(&box_.root, &st).unwrap();
    assert_eq!(again, 0);
    assert_eq!(lines(&box_.root).len(), 108);
}

#[test]
fn no_switch_mode_overwrite_before_marker() {
    let box_ = Sandbox::on();
    box_.lock_archive();
    let mut st = CompanionState::default();
    st.mode = "spark".into();
    st.turns.push(CompanionTurn {
        role: "you".into(),
        text: "live spark".into(),
    });
    st.threads.insert(
        "chat".into(),
        vec![CompanionTurn {
            role: "them".into(),
            text: "parked chat".into(),
        }],
    );
    st.switch_mode("chat");
    assert_eq!(st.mode, "chat");
    assert_eq!(st.threads["chat"][0].text, "parked chat");
    assert!(!marker_exists(&box_.root));
    box_.unlock_archive();
    st.switch_mode("spark");
    assert!(marker_exists(&box_.root));
    let got = lines(&box_.root);
    assert!(got.iter().any(|r| r.text == "parked chat" && r.src == "backfill:state.threads.chat"));
    assert!(got.iter().any(|r| r.text == "live spark"));
}

#[test]
fn flag_off_old_behavior() {
    force_flag(Some(false));
    install_root(None);
    reset_warn();
    let mut st = CompanionState::default();
    for i in 0..41 {
        st.push_turn("you", &format!("off {i}"));
    }
    assert_eq!(st.turns.len(), COMPANION_THREAD_CAP);
    assert_eq!(st.turns[0].text, "off 1");
    assert!(!enabled());
    assert!(legacy_send_roll(false, true, 16));
    assert!(!legacy_send_roll(true, true, 16));
    assert!(planned_reason(Some("thin"), 0, 120_000).is_some());
    assert!(planned_reason(None, 16, 120_000).is_none());
    force_flag(None);
}

#[test]
fn test_build_has_no_home_root() {
    force_flag(Some(true));
    install_root(None);
    assert!(!enabled());
    assert!(archive_root().is_none());
    force_flag(None);
}

#[test]
fn state_schema_unchanged() {
    let v = serde_json::to_value(CompanionState::default()).unwrap();
    let obj = v.as_object().unwrap();
    let mut keys: Vec<_> = obj.keys().cloned().collect();
    keys.sort();
    let expect = [
        "agentProfile",
        "autoHandoff",
        "backend",
        "bounds",
        "captions",
        "cheatSheetHint",
        "conductorModel",
        "conductorOn",
        "directBaseUrl",
        "directModel",
        "expanded",
        "freeMode",
        "hackLive",
        "hackProjectId",
        "hackWho",
        "handoffBackend",
        "imagePackId",
        "lastBotLine",
        "lastUserLine",
        "mode",
        "planetsHidden",
        "replyLane",
        "replyModel",
        "speakReplies",
        "speakVolume",
        "sprite",
        "state",
        "talkSource",
        "threads",
        "turns",
        "viewCollapsed",
        "viewExpanded",
        "visible",
    ];
    assert_eq!(keys, expect);
    assert!(obj.get("holdDrain").is_none());
    assert!(obj.get("unarchived").is_none());
    let turn = serde_json::to_value(CompanionTurn {
        role: "you".into(),
        text: "hi".into(),
    })
    .unwrap();
    let mut tk: Vec<_> = turn.as_object().unwrap().keys().cloned().collect();
    tk.sort();
    assert_eq!(tk, ["role", "text"]);
}

#[test]
fn prompts_unchanged_goldens() {
    let pocket = include_str!("../coordinator/intercept.rs");
    let gist = include_str!("mouth_gist.rs");
    let local = include_str!("mouth_hop/local.rs");
    for src in [pocket, gist, local] {
        assert!(!src.contains("mouth_archive"));
        assert!(!src.contains("resets.jsonl"));
        assert!(!src.contains("archive/"));
    }
}

#[test]
fn reset_reasons_table() {
    let box_ = Sandbox::on();
    let reasons = [
        "context_dead",
        "session_gone",
        "empty_resume",
        "budget",
        "thin",
        "manual_test",
    ];
    for reason in reasons {
        log_reset(
            &box_.root,
            &ResetLine {
                ts: "2026-10-30T12:00:00-05:00".into(),
                reason: reason.into(),
                backend: "grok".into(),
                old: "old".into(),
                new: "new".into(),
                carried: Carried {
                    soul_chars: 4,
                    rules_chars: 0,
                    open_items: 0,
                    dropped_items: 1,
                    verbatim_pairs: 2,
                    archive_upto: "t-1".into(),
                },
            },
        )
        .unwrap();
    }
    let got = resets_tail(&box_.root, 20);
    assert_eq!(got.len(), reasons.len());
    for (row, reason) in got.iter().zip(reasons) {
        assert_eq!(row.reason, reason);
        assert_eq!(row.carried.rules_chars, 0);
        assert_eq!(row.backend, "grok");
        let raw = serde_json::to_value(row).unwrap();
        for key in ["ts", "reason", "backend", "old", "new", "carried"] {
            assert!(raw.get(key).is_some(), "{key}");
        }
        for key in [
            "soul_chars",
            "rules_chars",
            "open_items",
            "dropped_items",
            "verbatim_pairs",
            "archive_upto",
        ] {
            assert!(raw["carried"].get(key).is_some(), "{key}");
        }
    }
}

#[test]
fn roll_constant_gone() {
    let plug = include_str!("../coordinator/plug.rs");
    let send = include_str!("../coordinator/send.rs");
    assert!(!plug.contains("GROK_ROLL_EVERY"));
    assert!(!send.contains("GROK_ROLL_EVERY"));
    assert!(send.contains("legacy_send_roll"));
    assert!(send.contains("planned_reason"));
}

#[test]
fn handoff_dead_byte_identical() {
    use crate::mouth_roll::{handoff_from_turns, ResetReason};
    use crate::state::CompanionTurn;
    let turns = vec![
        CompanionTurn {
            role: "you".into(),
            text: "hello".into(),
        },
        CompanionTurn {
            role: "them".into(),
            text: "hi".into(),
        },
    ];
    let dead = handoff_from_turns(ResetReason::ContextDead, "spark", &turns, "USER line");
    assert_eq!(
        dead,
        handoff_from_turns(ResetReason::SessionGone, "spark", &turns, "USER line")
    );
    assert_eq!(
        dead,
        handoff_from_turns(ResetReason::EmptyResume, "spark", &turns, "USER line")
    );
    let expect = "[handoff — the prior backend session died; this is the same desk thread (mode: spark) \
on a fresh session. The transcript so far is below. Answer the USER line at the end; \
do not greet, do not summarize.]\nyou: hello\nthem: hi\n\nUSER line";
    assert_eq!(dead, expect);
    assert_eq!(
        handoff_from_turns(ResetReason::ContextDead, "spark", &[], "USER line"),
        "USER line"
    );
}

#[test]
fn handoff_planned_no_died() {
    use crate::mouth_roll::{handoff_from_turns, ResetReason};
    use crate::state::CompanionTurn;
    let turns = vec![CompanionTurn {
        role: "you".into(),
        text: "hello".into(),
    }];
    for reason in [ResetReason::Budget, ResetReason::Thin, ResetReason::ManualTest] {
        let text = handoff_from_turns(reason, "chat", &turns, "USER line");
        assert!(
            !text.to_ascii_lowercase().contains("died"),
            "{reason:?}: {text}"
        );
        assert!(text.contains(reason.as_str()), "{text}");
    }
}

#[test]
fn verbs_tooldef_arm_match() {
    let control = include_str!("../agents/control.rs");
    let mcp = include_str!("../bin/orbitty_mcp.rs");
    for name in ["companion.archive_tail", "companion.resets", "companion.roll"] {
        assert!(control.contains(&format!("\"{name}\" =>")), "{name}");
        assert!(mcp.contains(&format!("name: \"{name}\"")), "{name}");
    }
    assert!(control.contains("\"companion.archive_tail\","));
    assert!(control.contains("\"companion.resets\","));
    assert!(!control.contains("\"companion.roll\","));
}

#[test]
fn planned_budget_beats_nothing_and_thin_wins() {
    assert_eq!(planned_reason(None, 120_000, 120_000), Some("budget"));
    assert_eq!(planned_reason(Some("thin"), 120_000, 120_000), Some("thin"));
    assert_eq!(
        planned_reason(Some("manual_test"), 0, 120_000),
        Some("manual_test")
    );
    assert_eq!(planned_reason(Some("nope"), 10, 120_000), None);
}
