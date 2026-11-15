use super::*;
use serde_json::json;

fn seat(id: &str, state: OutcomeState, line: &str, since: u64) -> SeatOutcome {
    SeatOutcome {
        seat: id.into(),
        state,
        line: line.into(),
        since,
    }
}

fn fn_body(src: &str, name: &str) -> String {
    let start = src.find(name).unwrap_or_else(|| panic!("missing {name}"));
    let bytes = src[start..].as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    assert!(i < bytes.len(), "no brace for {name}");
    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;
    let begin = i;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return src[start + begin..=start + i].to_string();
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unclosed {name}");
}

#[test]
fn outcome_mapping_table() {
    let rows = [
        ("working", "live", true, "done", OutcomeState::Working),
        ("blocked", "live", true, "working", OutcomeState::WaitingOnYou),
        ("idle", "", true, "working", OutcomeState::Idle),
        ("error", "live", true, "working", OutcomeState::Failed),
        ("done", "finished", true, "working", OutcomeState::Done),
        ("none", "finished", true, "working", OutcomeState::Done),
        ("none", "live", true, "starting", OutcomeState::Working),
        ("none", "live", true, "working", OutcomeState::Working),
        ("none", "live", true, "blocked", OutcomeState::WaitingOnYou),
        ("none", "live", true, "done", OutcomeState::Done),
        ("none", "live", true, "other", OutcomeState::Idle),
        ("unknown", "", true, "blocked", OutcomeState::WaitingOnYou),
        ("unknown", "", false, "done", OutcomeState::Done),
        ("working", "live", false, "blocked", OutcomeState::WaitingOnYou),
        ("working", "live", false, "starting", OutcomeState::Working),
        ("working", "live", false, "working", OutcomeState::Working),
        ("working", "live", false, "done", OutcomeState::Done),
        ("working", "live", false, "", OutcomeState::Idle),
        ("paused", "live", true, "working", OutcomeState::Idle),
        ("none", "finished", false, "working", OutcomeState::Working),
    ];
    for (status, state, up, run, expect) in rows {
        assert_eq!(
            from_herdr(status, state, up, run),
            expect,
            "{status} up={up} run={run} state={state}"
        );
    }
}

#[test]
fn go_run_blocked_is_waiting_on_you() {
    assert_eq!(from_go_stage("blocked", ""), OutcomeState::WaitingOnYou);
    assert_eq!(
        from_go_stage("blocked", "BLOCKED: disk"),
        OutcomeState::WaitingOnYou
    );
    assert_eq!(
        from_go_stage("", "BLOCKED: disk"),
        OutcomeState::WaitingOnYou
    );
    assert_eq!(
        from_go_stage("", "RIG BLOCKER: busy"),
        OutcomeState::WaitingOnYou
    );
    assert_eq!(
        from_go_stage("done", "RIG BLOCKER: busy"),
        OutcomeState::WaitingOnYou
    );
    assert_eq!(from_go_stage("", "FAIL: tests"), OutcomeState::Failed);
    assert_eq!(from_go_stage("done", "FAIL: tests"), OutcomeState::Failed);
    assert_eq!(from_go_stage("done", "PASS"), OutcomeState::Done);
    assert_eq!(from_go_stage("done", ""), OutcomeState::Done);
    assert_eq!(from_go_stage("cooking", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("fixing", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("verifying", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("relaunching", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("auditing", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("installing", ""), OutcomeState::Working);
    assert_eq!(from_go_stage("nope", ""), OutcomeState::Idle);
}

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(label: &str) -> (std::path::PathBuf, Scratch) {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("seat-oc-{label}-{n}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Scratch(dir))
}

fn write_run(root: &std::path::Path, id: &str, card: &str, stage: &str, result: &str, updated: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = json!({
        "card_id": card,
        "stage": stage,
        "result_line": result,
        "updated_at": updated,
    });
    std::fs::write(dir.join("state.json"), serde_json::to_string(&doc).unwrap()).unwrap();
}

#[test]
fn go_run_without_seat_or_lock_is_not_listed() {
    let (root, _keep) = scratch("runs");
    write_run(
        &root,
        "old-run",
        "c-old",
        "blocked",
        "BLOCKED: old",
        "2020-01-01T00:00:00Z",
    );
    write_run(
        &root,
        "live-run",
        "c-live",
        "blocked",
        "BLOCKED: now",
        "2026-01-02T00:00:00Z",
    );
    write_run(
        &root,
        "lock-run",
        "c-lock",
        "cooking",
        "",
        "2026-01-03T00:00:00Z",
    );
    std::fs::create_dir_all(root.join("LOCK")).unwrap();
    std::fs::write(root.join("LOCK").join("card"), "c-lock\n").unwrap();
    let got = go_run_outcomes(&root, &["c-live"]);
    let ids: Vec<&str> = got.iter().map(|s| s.seat.as_str()).collect();
    assert!(!ids.contains(&"agent:c-old"), "{ids:?}");
    assert!(ids.contains(&"agent:c-live"), "{ids:?}");
    assert!(ids.contains(&"agent:c-lock"), "{ids:?}");
    assert_eq!(got.len(), 2, "{ids:?}");
    assert_eq!(
        got.iter().find(|s| s.seat == "agent:c-live").unwrap().state,
        OutcomeState::WaitingOnYou
    );
    assert_eq!(
        got.iter().find(|s| s.seat == "agent:c-lock").unwrap().state,
        OutcomeState::Working
    );
}

#[test]
fn outcome_keyset_no_internals() {
    let o = seat("agent:c-1", OutcomeState::WaitingOnYou, "short", 5);
    let v = serde_json::to_value(&o).unwrap();
    let obj = v.as_object().unwrap();
    let mut keys: Vec<_> = obj.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["line", "seat", "since", "state"]);
    assert_eq!(v["state"], "waiting_on_you");
    assert_eq!(v["seat"], "agent:c-1");
    assert_eq!(v["line"], "short");
    assert_eq!(v["since"], 5);
    for banned in ["terminal", "pane", "workspace", "kind", "model", "path", "source"] {
        assert!(v.get(banned).is_none(), "{banned}");
    }
}

#[test]
fn line_clamped_120_no_paths() {
    let users = format!("/{}s/x/notes", "User");
    let home = format!("/{}/x/notes", "home");
    let tilde = format!("{}{}notes", '~', "/");
    let raw = format!("keep {users}\n{home} and {tilde} tail");
    let got = clamp_line(&raw);
    assert!(!got.contains('\n'));
    assert!(got.chars().count() <= 120, "{}", got.chars().count());
    assert!(!got.contains(&users), "{got}");
    assert!(!got.contains(&home), "{got}");
    assert!(!got.contains(&tilde), "{got}");
    assert!(!got.contains("notes"), "{got}");
    assert_eq!(got, "keep and tail");
    let long = format!("{}\n{}", "z".repeat(200), "y".repeat(200));
    let cut = clamp_line(&long);
    assert_eq!(cut.chars().count(), 120);
    assert!(!cut.contains('\n'));
}

#[test]
fn merge_dedupes_newest_wins() {
    let older = seat("agent:a", OutcomeState::Working, "old", 1);
    let newer = seat("agent:a", OutcomeState::Done, "new", 5);
    let other = seat("agent:b", OutcomeState::Idle, "", 2);
    let got = merge(vec![vec![older.clone(), other.clone()], vec![newer.clone()]]);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], newer);
    assert_eq!(got[1], other);
    let tie = seat("agent:a", OutcomeState::Failed, "tie", 5);
    let got2 = merge(vec![vec![newer.clone()], vec![older, tie]]);
    assert_eq!(got2.len(), 1);
    assert_eq!(got2[0].line, "new");
    assert_eq!(got2[0].state, OutcomeState::Done);
}

struct Fake(Vec<SeatOutcome>);

impl SeatAdapter for Fake {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        self.0.clone()
    }
}

struct Other(Vec<SeatOutcome>);

impl SeatAdapter for Other {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        self.0.clone()
    }
}

#[test]
fn swap_adapter_byte_identical() {
    let seats = vec![seat("agent:c-1", OutcomeState::WaitingOnYou, "hold", 3)];
    let real = Fake(seats);
    let fake = Other(real.outcomes());
    let left = serde_json::to_vec(&door_from(true, "herdr", &[&real])).unwrap();
    let right = serde_json::to_vec(&door_from(true, "herdr", &[&fake])).unwrap();
    assert_eq!(left, right);
    let text = String::from_utf8(left).unwrap();
    assert!(text.contains("waiting_on_you"));
    assert!(text.contains("hold"));
    assert!(!text.contains("terminal"));
}

struct Boom;

impl SeatAdapter for Boom {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        panic!("flag off must not read seats");
    }
}

#[test]
fn door_flag_off_is_disabled() {
    let boom = Boom;
    let v = door_from(false, "herdr", &[&boom]);
    assert_eq!(serde_json::to_string(&v).unwrap(), "{\"enabled\":false}");
    let body = fn_body(include_str!("outcome.rs"), "pub fn all(");
    assert!(body.contains("seat_outcomes"));
    assert!(body.contains("flags::on"));
    assert!(body.contains("enabled"));
    assert!(body.contains("false"));
}

#[test]
fn flag_off_view_json_byte_equal() {
    let v = crate::herdr::peek::View {
        card: "c-1".into(),
        status: "blocked".into(),
        state: "live".into(),
        text: String::new(),
        source: "herdr".into(),
        up: true,
        outcome: String::new(),
    };
    let got = serde_json::to_string(&v).unwrap();
    let expect =
        r#"{"card":"c-1","status":"blocked","state":"live","text":"","source":"herdr","up":true}"#;
    assert_eq!(got, expect);
    assert!(!got.contains("outcome"));
}

#[test]
fn for_view_makes_no_hc_call() {
    let body = fn_body(include_str!("outcome.rs"), "pub(crate) fn for_view(");
    for name in ["peek::view", "core::terminal", "read_recent", "read_text"] {
        assert!(!body.contains(name), "{name} in for_view:\n{body}");
    }
}

#[test]
fn caption_flag_on_skips_second_peek() {
    let src = include_str!("../habitat/droid_inspect.rs");
    let on = fn_body(src, "fn capsule_outcomes(");
    assert!(on.contains("tool_line"), "{on}");
    assert!(!on.contains("capsule_line"), "{on}");
    let off = fn_body(src, "fn capsule_legacy(");
    assert!(off.contains("capsule_line"), "{off}");
    let entry = fn_body(src, "pub fn capsule(");
    assert!(entry.contains("seat_outcomes"));
    assert!(entry.contains("capsule_legacy"));
    assert!(entry.contains("capsule_outcomes"));
}
