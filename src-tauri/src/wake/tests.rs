use super::route::{self, Caller, Clock, EventView, Perm, Sub, Trigger};
use super::store;
use super::*;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn clock(unix: u64) -> Clock {
    Clock { unix }
}

fn sub(id: &str, seat: &str, dedupe: u32, cap: u32, e_type: &str) -> Sub {
    Sub {
        v: 1,
        id: id.into(),
        seat: seat.into(),
        trigger: Trigger::events(if e_type.is_empty() {
            vec![]
        } else {
            vec![e_type.into()]
        }),
        dedupe_s: dedupe,
        max_per_hour: cap,
        created_by: "user".into(),
        created_at: "2026-10-30T12:00:00-05:00".into(),
        expires_at: None,
    }
}

fn ev(seq: u64, e_type: &str, key: &str) -> EventView {
    EventView {
        seq,
        id: format!("e{seq}"),
        kind: 421,
        e_type: e_type.into(),
        project: "planet".into(),
        card: "card1".into(),
        key: key.into(),
        summary: format!("m{seq}"),
        source: "events".into(),
    }
}

fn sys() -> Perm {
    Perm {
        caller: Caller::System,
    }
}

fn append_log(dir: &Path, seqs: &[u64]) {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("events.jsonl"))
        .unwrap();
    for seq in seqs {
        let line = format!(
            r#"{{"seq":{seq},"id":"e{seq}","pubkey":"","created_at":1,"kind":421,"tags":[["t","orbitty"],["e-type","board-move"],["project","planet"],["d","card1"]],"content":"{{\"key\":\"k{seq}\",\"summary\":\"m{seq}\"}}","sig":""}}"#
        );
        writeln!(f, "{line}").unwrap();
    }
}

fn inbox_summaries(dir: &Path, seat: &str) -> Vec<String> {
    let seat = SeatId::parse(seat).unwrap();
    store::read_inbox(dir, &seat)
        .iter()
        .map(|l| l["summary"].as_str().unwrap_or("").to_string())
        .collect()
}

fn stub(dir: &Path, mode: &str) -> PathBuf {
    let path = dir.join("hc-stub");
    let script = r#"#!/usr/bin/env python3
import os, sys, time
args = sys.argv[1:]
log = os.environ.get("WAKE_HC_LOG", "")
if log:
    with open(log, "a", encoding="utf-8") as f:
        f.write(" ".join(args) + "\n")
cmd = args[0] if args else ""
mode = os.environ.get("WAKE_HC_MODE", "open")
if cmd == "server":
    sys.stdout.write('{"version":"stub"}\n')
    sys.exit(0)
if cmd == "events":
    if mode == "crash":
        sys.stdout.write('{"seq":1,"id":"e1","e_type":"board-move","summary":"one","kind":421,"project":"planet","card":"card1","key":"k1"}\n')
        sys.stdout.flush()
        sys.exit(0)
    if mode == "map":
        sys.stdout.write('{"seq":1,"id":"h1","e_type":"board-move","summary":"from-hc","project":"p","card":"c","key":"k","kind":421}\n')
        sys.stdout.write('{"seq":2,"id":"h2","e_type":"note","summary":"two","project":"p","card":"c","key":"k2","kind":31200}\n')
        sys.stdout.flush()
        sys.exit(0)
    sys.stdout.write('{"ok":true}\n')
    sys.stdout.flush()
    time.sleep(30)
    sys.exit(0)
if cmd == "send":
    sys.stdout.write('{"ok":true}\n')
    sys.exit(0)
sys.exit(0)
"#;
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&path).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&path, p).unwrap();
    }
    crate::herdr::core::set_bin_override(Some(path.clone()));
    crate::herdr::client::set_child_env(vec![
        ("WAKE_HC_MODE".into(), mode.into()),
        ("WAKE_HC_LOG".into(), dir.join("hc.log").display().to_string()),
    ]);
    path
}

fn herdr_guard() -> std::sync::MutexGuard<'static, ()> {
    let g = crate::herdr::client::TEST_GATE.lock().unwrap_or_else(|e| e.into_inner());
    crate::herdr::client::reset_overrides();
    crate::herdr::health::reset();
    g
}

#[test]
fn seat_id_parse_table() {
    let ok = [
        "primary",
        "agent:card1",
        "claim:win1",
        "claim:tab:uuid",
        "watch:w1",
        "remote:r1",
    ];
    assert_eq!(ok.len(), 6);
    for raw in ok {
        assert!(SeatId::parse(raw).is_ok(), "{raw}");
    }
    assert!(SeatId::parse("foo:1").is_err());
    assert!(SeatId::parse("").is_err());
    assert!(SeatId::parse("agent:").is_err());
}

#[test]
fn route_match_table() {
    let mut base = ev(1, "board-move", "alpha");
    let hit = sub("s", "primary", 0, 6, "board-move");
    assert!(route::matches(&base, &hit));
    base.e_type = "note".into();
    assert!(!route::matches(&base, &hit));
    base.e_type = "board-move".into();
    let mut by_kind = hit.clone();
    by_kind.trigger.e_types.clear();
    by_kind.trigger.kinds = vec![421];
    assert!(route::matches(&base, &by_kind));
    base.kind = 31200;
    assert!(!route::matches(&base, &by_kind));
    base.kind = 421;
    let mut by_project = hit.clone();
    by_project.trigger.project = Some("other".into());
    assert!(!route::matches(&base, &by_project));
    by_project.trigger.project = Some("planet".into());
    assert!(route::matches(&base, &by_project));
    let mut by_card = hit.clone();
    by_card.trigger.card = Some("nope".into());
    assert!(!route::matches(&base, &by_card));
    by_card.trigger.card = Some("card1".into());
    assert!(route::matches(&base, &by_card));
    let mut by_prefix = hit.clone();
    by_prefix.trigger.key_prefix = Some("zz".into());
    assert!(!route::matches(&base, &by_prefix));
    by_prefix.trigger.key_prefix = Some("alp".into());
    assert!(route::matches(&base, &by_prefix));
}

#[test]
fn perm_matrix() {
    let any = Trigger::events(vec!["board-move".into()]);
    let seats = [
        SeatId::Primary,
        SeatId::Agent("c1".into()),
        SeatId::Claim("w".into()),
        SeatId::Watch("w".into()),
        SeatId::Remote("r".into()),
    ];
    for seat in &seats {
        assert!(route::permit(&Caller::User, seat, &any).is_ok(), "{seat:?}");
    }
    let agent = Caller::Agent {
        card: "c1".into(),
        planet: Some("p1".into()),
    };
    let own = Trigger {
        card: Some("c1".into()),
        project: Some("p1".into()),
        ..Trigger::events(vec!["board-move".into()])
    };
    assert!(route::permit(&agent, &SeatId::Agent("c1".into()), &own).is_ok());
    assert!(route::permit(&agent, &SeatId::Agent("c2".into()), &own).is_err());
    assert!(route::permit(&agent, &SeatId::Primary, &own).is_err());
    let other_card = Trigger {
        card: Some("c2".into()),
        ..own.clone()
    };
    assert!(route::permit(&agent, &SeatId::Agent("c1".into()), &other_card).is_err());
    let other_planet = Trigger {
        project: Some("p2".into()),
        ..own.clone()
    };
    assert!(route::permit(&agent, &SeatId::Agent("c1".into()), &other_planet).is_err());
    assert!(route::permit(&Caller::Companion, &SeatId::Primary, &any).is_ok());
    assert!(route::permit(&Caller::Companion, &SeatId::Agent("c1".into()), &any).is_err());
}

#[test]
fn dedupe_window() {
    let subs = vec![sub("s", "primary", 300, 6, "board-move")];
    let mut state = route::RouteState::default();
    let c = clock(1_700_000_000);
    let mut n = 0;
    for i in 1..=5 {
        let d = route::route(&ev(i, "board-move", "same"), &subs, &sys(), &c, &mut state);
        n += d.iter().filter(|d| d.deliver).count();
    }
    assert_eq!(n, 1);
}

#[test]
fn noise_cap_folds() {
    let subs = vec![sub("s", "primary", 0, 6, "board-move")];
    let mut state = route::RouteState::default();
    let c = clock(1_700_000_000);
    let mut lines = 0;
    for i in 1..=7 {
        let d = route::route(
            &ev(i, "board-move", &format!("k{i}")),
            &subs,
            &sys(),
            &c,
            &mut state,
        );
        lines += d.iter().filter(|d| d.deliver).count();
    }
    assert_eq!(lines, 6);
    let later = clock(1_700_000_000 + 3600);
    let folded = route::roll_noise(&subs, &later, &mut state);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].line["summary"], "1 more");
}

#[test]
fn loop_guard_ignores_wake_etypes() {
    let subs = vec![sub("s", "primary", 0, 6, "")];
    let mut state = route::RouteState::default();
    let c = clock(1_700_000_000);
    for e in ["wake", "wake-drop", "wake-alarm"] {
        let d = route::route(&ev(1, e, "k"), &subs, &sys(), &c, &mut state);
        assert!(d.iter().all(|x| !x.deliver), "{e}");
    }
}

#[test]
fn cursor_survives_restart() {
    let dir = scratch_dir("cursor");
    arm(&dir, true, false);
    put_sub(&dir, sub("s", "primary", 0, 20, "board-move")).unwrap();
    append_log(&dir, &(1..=5).collect::<Vec<_>>());
    run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert_eq!(store::load_cursor(&dir), 5);
    append_log(&dir, &(6..=10).collect::<Vec<_>>());
    run_drain(&dir, &clock(1_700_000_100)).unwrap();
    let seat = SeatId::parse("primary").unwrap();
    let lines = store::read_inbox(&dir, &seat);
    let mut seqs: Vec<u64> = lines.iter().filter_map(|l| l["seq"].as_u64()).collect();
    seqs.sort();
    seqs.dedup();
    assert_eq!(seqs, (1..=10).collect::<Vec<_>>());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn replay_no_dupes() {
    let dir = scratch_dir("replay");
    arm(&dir, true, false);
    put_sub(&dir, sub("s", "primary", 0, 20, "board-move")).unwrap();
    append_log(&dir, &[1, 2, 3]);
    run_drain(&dir, &clock(1_700_000_000)).unwrap();
    let seat = SeatId::parse("primary").unwrap();
    let before = store::read_inbox(&dir, &seat).len();
    store::save_cursor(&dir, 0).unwrap();
    let mut state = store::load_state(&dir);
    state.seen.clear();
    state.logical.clear();
    store::save_state(&dir, &state).unwrap();
    run_drain(&dir, &clock(1_700_000_500)).unwrap();
    assert_eq!(store::read_inbox(&dir, &seat).len(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn append_never_blocks() {
    let _lock = worker::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = scratch_dir("block");
    arm(&dir, true, false);
    worker::stall_for_test(true);
    worker::ensure_started(dir.clone());
    assert!(worker::wait_parked(Duration::from_secs(5)));
    let ping = Instant::now();
    for _ in 0..1000 {
        worker::notify();
    }
    assert!(
        ping.elapsed() < Duration::from_millis(200),
        "notify blocked on the stalled worker"
    );
    let path = dir.join("events.jsonl");
    let start = Instant::now();
    for i in 0..1000 {
        crate::event_log::write_event(
            &path,
            i,
            31200,
            vec![["t".into(), "local-only".into()], ["e-type".into(), "note".into()]],
            "{}".into(),
            (format!("i{i}"), String::new(), String::new()),
        )
        .unwrap();
    }
    // A blocking send never returns while the worker is stalled. 30s only
    // covers a busy log lock; the notify loop above is the real check.
    assert!(start.elapsed() < Duration::from_secs(30), "appends blocked");
    worker::stall_for_test(false);
    worker::stop();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn soak_p99_under_1ms() {
    let _lock = worker::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = scratch_dir("soak");
    arm(&dir, true, false);
    worker::stall_for_test(true);
    worker::ensure_started(dir.clone());
    assert!(worker::wait_parked(Duration::from_secs(2)));
    let path = dir.join("events.jsonl");
    let mut samples = Vec::with_capacity(1000);
    for i in 0..1000 {
        let t = Instant::now();
        crate::event_log::write_event(
            &path,
            i,
            31200,
            vec![["t".into(), "local-only".into()]],
            "{}".into(),
            (format!("s{i}"), String::new(), String::new()),
        )
        .unwrap();
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = samples[989];
    eprintln!("wake soak p99 ms {p99}");
    assert!(p99 < 1.0, "p99 {p99} ms");
    worker::stall_for_test(false);
    worker::stop();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn status_active_at() {
    let dir = scratch_dir("status");
    arm(&dir, true, false);
    let seat = SeatId::parse("primary").unwrap();
    let c = clock(1_700_000_000);
    let st = status_json(&dir, &seat, &c);
    assert_eq!(st["active"], false);
    assert!(st["at"].is_null());
    let at = alarm::parse_at("+2m", c.unix).unwrap();
    alarm::set(&dir, &seat, &at).unwrap();
    let st = status_json(&dir, &seat, &c);
    assert_eq!(st["active"], false);
    assert_eq!(st["at"].as_str().unwrap(), at);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn one_alarm_per_seat() {
    let dir = scratch_dir("one-alarm");
    arm(&dir, true, false);
    let seat = SeatId::parse("primary").unwrap();
    alarm::set(&dir, &seat, "2026-10-30T12:00:00-05:00").unwrap();
    alarm::set(&dir, &seat, "2026-10-30T13:00:00-05:00").unwrap();
    let alarms = store::load_alarms(&dir);
    assert_eq!(alarms.len(), 1);
    assert_eq!(
        alarms.get("primary").map(String::as_str),
        Some("2026-10-30T13:00:00-05:00")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn alarm_fires_once_writes_line_only() {
    let dir = scratch_dir("alarm-fire");
    arm(&dir, true, false);
    let seat = SeatId::parse("primary").unwrap();
    let at = "2026-10-30T12:00:00-05:00";
    alarm::set(&dir, &seat, at).unwrap();
    let c = clock(alarm::unix_of(at).unwrap() + 1);
    let fired = alarm::fire_due(&dir, &c).unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].1["e_type"], "wake-alarm");
    let again = alarm::fire_due(&dir, &c).unwrap();
    assert!(again.is_empty());
    assert_eq!(store::read_inbox(&dir, &seat).len(), 1);
    let logs = store::summary_lines(&dir);
    assert!(logs.iter().any(|v| store::tag(v, "e-type") == "wake-alarm" && store::tag(v, "t") == "local-only"));
    assert!(logs.iter().all(|v| v["kind"].as_u64() == Some(31200)));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resume_idempotent_after_ack() {
    let dir = scratch_dir("resume");
    arm(&dir, true, false);
    let seat = SeatId::parse("primary").unwrap();
    for seq in 1..=3 {
        store::append_inbox(
            &dir,
            &seat,
            &json!({"seq": seq, "summary": format!("m{seq}"), "dedupe_key": format!("k{seq}")}),
        )
        .unwrap();
    }
    let a = resume(&dir, &seat, 50);
    let b = resume(&dir, &seat, 50);
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
    ack(&dir, &seat, 2).unwrap();
    let left = resume(&dir, &seat, 50);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["seq"], 3);
    let again = resume(&dir, &seat, 50);
    assert_eq!(left, again);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ack_monotonic() {
    let dir = scratch_dir("ack");
    arm(&dir, true, false);
    let seat = SeatId::parse("primary").unwrap();
    ack(&dir, &seat, 5).unwrap();
    ack(&dir, &seat, 3).unwrap();
    assert_eq!(store::load_ack(&dir, &seat), 5);
    ack(&dir, &seat, 9).unwrap();
    assert_eq!(store::load_ack(&dir, &seat), 9);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn flag_off_no_files_verbs_refuse() {
    let dir = scratch_dir("off");
    arm(&dir, false, false);
    for tool in [
        "wake.list",
        "wake.inbox",
        "wake.status",
        "wake.subscribe",
        "wake.ack",
        "wake.emit",
        "wake.alarm",
        "wake.resume",
    ] {
        let err = dispatch(&dir, tool, &json!({"seat":"primary","source":"x","at":"+1m","upto_seq":1}), "ctl:1")
            .unwrap_err();
        assert!(err.contains("wake_core"), "{tool} {err}");
        assert!(!err.contains('\n'), "{err}");
    }
    assert!(!files_exist(&dir));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn wake_source_never_acts() {
    let src = format!(
        "{}{}{}{}{}{}",
        include_str!("mod.rs"),
        include_str!("store.rs"),
        include_str!("route.rs"),
        include_str!("worker.rs"),
        include_str!("alarm.rs"),
        include_str!("herdr_bus.rs"),
    );
    for banned in [
        "launch_agent",
        "go_run",
        "kanban",
        "send_route",
        "companion_send",
    ] {
        assert!(!src.contains(banned), "{banned}");
    }
}

#[test]
fn wake_via_hc_send_primary() {
    let _g = herdr_guard();
    let dir = scratch_dir("hc-send");
    arm(&dir, true, true);
    put_sub(&dir, sub("s", "primary", 0, 6, "board-move")).unwrap();
    append_log(&dir, &[1]);
    stub(&dir, "open");
    let out = run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert_eq!(out.bus, "herdr");
    let log = fs::read_to_string(dir.join("hc.log")).unwrap_or_default();
    assert!(log.contains("send"), "{log}");
    assert!(!inbox_summaries(&dir, "primary").is_empty());
    worker::stop();
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hc_events_maps_to_wake() {
    let _g = herdr_guard();
    let dir = scratch_dir("hc-map");
    arm(&dir, true, true);
    put_sub(&dir, sub("s", "primary", 0, 6, "")).unwrap();
    stub(&dir, "map");
    run_drain(&dir, &clock(1_700_000_000)).unwrap();
    let sums = inbox_summaries(&dir, "primary");
    assert!(sums.iter().any(|s| s == "from-hc"), "{sums:?}");
    assert!(sums.iter().any(|s| s == "two"), "{sums:?}");
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn herdr_missing_falls_back_to_relay() {
    let _g = herdr_guard();
    let dir = scratch_dir("hc-miss");
    arm(&dir, true, true);
    put_sub(&dir, sub("s", "primary", 0, 6, "board-move")).unwrap();
    append_log(&dir, &[1, 2]);
    crate::herdr::core::set_bin_override(None);
    let out = run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert_eq!(out.bus, "fallback");
    assert_eq!(inbox_summaries(&dir, "primary").len(), 2);
    let switches = switch_events(&dir);
    assert_eq!(switches.len(), 1, "{switches:?}");
    assert_eq!(store::json_body(&switches[0])["source"], "fallback");
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn herdr_crash_midstream_falls_back() {
    let _g = herdr_guard();
    let dir = scratch_dir("hc-crash");
    arm(&dir, true, true);
    put_sub(&dir, sub("s", "primary", 0, 20, "board-move")).unwrap();
    append_log(&dir, &[1, 2, 3]);
    stub(&dir, "crash");
    let start = Instant::now();
    let out = run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert!(start.elapsed() < Duration::from_secs(2), "fallback slower than 2s");
    assert_eq!(out.bus, "fallback");
    let seat = SeatId::parse("primary").unwrap();
    let mut seqs: Vec<u64> = store::read_inbox(&dir, &seat)
        .iter()
        .filter_map(|l| l["seq"].as_u64())
        .collect();
    seqs.sort();
    seqs.dedup();
    assert_eq!(seqs, vec![1, 2, 3], "lost or doubled wake");
    let switches = switch_events(&dir);
    assert_eq!(switches.len(), 1);
    assert_eq!(store::json_body(&switches[0])["source"], "fallback");
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn herdr_recovers_returns_primary() {
    let _g = herdr_guard();
    let dir = scratch_dir("hc-back");
    arm(&dir, true, true);
    put_sub(&dir, sub("s", "primary", 0, 6, "board-move")).unwrap();
    crate::herdr::core::set_bin_override(None);
    let first = run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert_eq!(first.bus, "fallback");
    stub(&dir, "open");
    let mid = run_drain(&dir, &clock(1_700_000_030)).unwrap();
    assert_eq!(mid.bus, "fallback", "one healthy sample is not enough after fallback");
    let back = run_drain(&dir, &clock(1_700_000_060)).unwrap();
    assert_eq!(back.bus, "herdr");
    let switches = switch_events(&dir);
    assert!(switches.iter().any(|v| store::json_body(v)["source"] == "herdr"), "{switches:?}");
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn flag_off_relay_only() {
    let _g = herdr_guard();
    let dir = scratch_dir("relay-only");
    arm(&dir, true, false);
    put_sub(&dir, sub("s", "primary", 0, 6, "board-move")).unwrap();
    append_log(&dir, &[4]);
    stub(&dir, "open");
    let out = run_drain(&dir, &clock(1_700_000_000)).unwrap();
    assert_eq!(out.bus, "relay");
    assert_eq!(inbox_summaries(&dir, "primary").len(), 1);
    let log = fs::read_to_string(dir.join("hc.log")).unwrap_or_default();
    assert!(!log.contains("send"), "{log}");
    assert!(switch_events(&dir).is_empty());
    crate::herdr::client::reset_overrides();
    let _ = fs::remove_dir_all(&dir);
}
