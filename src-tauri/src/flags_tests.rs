use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(label: &str) -> (PathBuf, Scratch) {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("orbitty-flags-{label}-{n}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Scratch(dir))
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn demo(id: &'static str, go: &'static str, auto: bool) -> FlagDef {
    FlagDef {
        id,
        default: false,
        what: "demo",
        go,
        added: "2026-10-28",
        remove_after: "2026-12-27",
        auto_on_verify: auto,
    }
}

const EXPECT_MANUAL: &[&str] = &[
    "adapter_cursor",
    "adapter_grokbot",
    "beads_truth",
    "board_rules_webhook",
    "cold_tree",
    "coordinator_launch",
    "coordinator_propose",
    "coordinator_triage",
    "cost_view",
    "net_guard",
    "outcome_lanes",
    "promote_schedule",
    "proof_required",
    "remote_guardrails",
    "remote_net",
];

#[test]
fn auto_sweep_flips_on_pass() {
    let (dir, _keep) = scratch("sweep");
    let sha = "abcd1234";
    fs::write(
        dir.join(format!("VERIFY_GO-ORBITTY-DEMO_{sha}.md")),
        "noise\nRESULT: PASS\n",
    )
    .unwrap();
    fs::write(
        dir.join(format!("VERIFY_GO-ORBITTY-OTHER_{sha}.md")),
        "RESULT: FAIL\n",
    )
    .unwrap();
    fs::write(
        dir.join("VERIFY_GO-ORBITTY-DEMO_ffffffff.md"),
        "RESULT: PASS\n",
    )
    .unwrap();
    let reports = load_reports(&dir, sha);
    let row = demo("demo_auto", "GO-ORBITTY-DEMO", true);
    let mut file = FileFlags::empty();
    let flipped = sweep_defs(&[row], Channel::Nightly, sha, &reports, &mut file);
    assert_eq!(flipped, vec!["demo_auto".to_string()]);
    assert_eq!(file.on.get("demo_auto"), Some(&true));
    assert_eq!(
        file.src.get("demo_auto").map(String::as_str),
        Some("auto-verify")
    );
    let (on, source) = resolve(&row, None, &file);
    assert!(on);
    assert_eq!(source, Source::AutoVerify);

    let other = demo("demo_fail", "GO-ORBITTY-OTHER", true);
    let mut file2 = FileFlags::empty();
    let flipped2 = sweep_defs(&[other], Channel::Nightly, sha, &reports, &mut file2);
    assert!(flipped2.is_empty());
    assert!(file2.on.get("demo_fail").is_none());

    let mut held = FileFlags::empty();
    held.auto = false;
    let flipped3 = sweep_defs(&[row], Channel::Nightly, sha, &reports, &mut held);
    assert!(flipped3.is_empty(), "flags auto off stops the sweep");
}

/// Shape copied from VERIFY_GO-ORBITTY-ALPHA-STORY-FIX-2_79486ae7.md:
/// a `## Result` heading, the verdict line, then `PASS tip:`.
const ALPHA_STORY_PASS: &str = "\
# VERIFY GO-ORBITTY-ALPHA-STORY-FIX-2

## Left running / on disk

- Nothing left running. No Orbitty launch, no cargo process, no data-dir writes.

## Result
PASS-with-notes
PASS tip: 79486ae7f698e0a5cf89e22c625e633feac8f27d
Fails:
DOGFOOD items: fresh-launch rail edge + hide + no late snap
Fixes for Ship:
";

const HEADING_FAIL: &str = "\
## Result
FAIL
PASS tip: cb14e32f92cf9395ee77e0aafb996c90247986c8
Fails: F4 plant/harvest chore aborts on the Board-stage sign target
";

#[test]
fn auto_sweep_reads_real_verify_heading() {
    assert_eq!(
        result_of(ALPHA_STORY_PASS).as_deref(),
        Some("PASS-with-notes")
    );
    assert_eq!(result_of(HEADING_FAIL).as_deref(), Some("FAIL"));
    assert_eq!(
        result_of("## Result\n\nRIG BLOCKER\nPASS tip: abcd1234\n").as_deref(),
        Some("RIG BLOCKER")
    );
    assert_eq!(result_of("noise\nRESULT: PASS\n").as_deref(), Some("PASS"));
    assert_eq!(
        result_of("RESULT: PASS-with-notes abcd1234\n").as_deref(),
        Some("PASS-with-notes abcd1234")
    );

    let (dir, _keep) = scratch("heading");
    let sha = "79486ae7";
    let go = "GO-ORBITTY-ALPHA-STORY-FIX-2";
    fs::write(
        dir.join(format!("VERIFY_{go}_{sha}.md")),
        ALPHA_STORY_PASS,
    )
    .unwrap();
    let reports = load_reports(&dir, sha);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].result, "PASS-with-notes");
    let row = demo("story_auto", go, true);
    let mut file = FileFlags::empty();
    let flipped = sweep_defs(&[row], Channel::Nightly, sha, &reports, &mut file);
    assert_eq!(flipped, vec!["story_auto".to_string()]);
    assert_eq!(
        file.src.get("story_auto").map(String::as_str),
        Some("auto-verify")
    );

    let (fail_dir, _keep_fail) = scratch("heading-fail");
    fs::write(fail_dir.join(format!("VERIFY_{go}_{sha}.md")), HEADING_FAIL).unwrap();
    let fail_reports = load_reports(&fail_dir, sha);
    assert_eq!(fail_reports[0].result, "FAIL");
    let mut file_fail = FileFlags::empty();
    let flipped_fail = sweep_defs(&[row], Channel::Nightly, sha, &fail_reports, &mut file_fail);
    assert!(flipped_fail.is_empty());
    assert!(file_fail.on.get("story_auto").is_none());

    let blocker = "\
## Result
RIG BLOCKER
PASS tip: 79486ae7f698e0a5cf89e22c625e633feac8f27d
";
    fs::write(fail_dir.join(format!("VERIFY_{go}_{sha}.md")), blocker).unwrap();
    let blocker_reports = load_reports(&fail_dir, sha);
    let mut file_block = FileFlags::empty();
    let flipped_block = sweep_defs(
        &[row],
        Channel::Nightly,
        sha,
        &blocker_reports,
        &mut file_block,
    );
    assert!(flipped_block.is_empty());
}

#[test]
fn manual_off_sticky() {
    let (dir, _keep) = scratch("sticky");
    let sha = "abcd1234";
    fs::write(
        dir.join(format!("VERIFY_GO-ORBITTY-DEMO_{sha}.md")),
        "RESULT: PASS-with-notes\n",
    )
    .unwrap();
    let reports = load_reports(&dir, sha);
    let row = demo("demo_auto", "GO-ORBITTY-DEMO", true);
    let mut file = FileFlags::empty();
    mark_set(&mut file, "demo_auto", false);
    let flipped = sweep_defs(&[row], Channel::Nightly, sha, &reports, &mut file);
    assert!(flipped.is_empty());
    assert_eq!(file.on.get("demo_auto"), Some(&false));
    assert!(file.sticky_off.contains("demo_auto"));

    mark_set(&mut file, "demo_auto", true);
    assert!(!file.sticky_off.contains("demo_auto"));
    file.on.insert("demo_auto".into(), false);
    file.src.insert("demo_auto".into(), "file".into());
    let flipped = sweep_defs(&[row], Channel::Nightly, sha, &reports, &mut file);
    assert_eq!(flipped, vec!["demo_auto".to_string()]);
}

#[test]
fn manual_only_rows_never_auto() {
    assert_eq!(MANUAL_ONLY, EXPECT_MANUAL);
    for id in MANUAL_ONLY {
        assert!(id_ok(id), "{id}");
        if let Some(def) = def_by_id(id) {
            assert!(!def.auto_on_verify, "{id}");
        }
    }
    for def in FLAGS {
        if MANUAL_ONLY.contains(&def.id) {
            assert!(!def.auto_on_verify, "{}", def.id);
        }
    }
    let row = demo("remote_net", "GO-ORBITTY-REMOTE", true);
    let mut file = FileFlags::empty();
    let reports = [SweepReport {
        go_id: "GO-ORBITTY-REMOTE".into(),
        sha8: "abcd1234".into(),
        result: "PASS".into(),
    }];
    let flipped = sweep_defs(&[row], Channel::Nightly, "abcd1234", &reports, &mut file);
    assert!(flipped.is_empty());
    assert!(file.on.get("remote_net").is_none());
}

#[test]
fn stable_never_auto() {
    let row = demo("demo_auto", "GO-ORBITTY-DEMO", true);
    let reports = [SweepReport {
        go_id: "GO-ORBITTY-DEMO".into(),
        sha8: "abcd1234".into(),
        result: "PASS".into(),
    }];
    for channel in [Channel::Stable, Channel::Alpha] {
        let mut file = FileFlags::empty();
        let flipped = sweep_defs(&[row], channel, "abcd1234", &reports, &mut file);
        assert!(flipped.is_empty(), "{channel:?}");
        assert!(file.on.is_empty());
    }
}

#[test]
fn env_beats_file_beats_default() {
    let def = demo("demo", "GO-ORBITTY-DEMO", false);
    let mut file = FileFlags::empty();
    file.on.insert("demo".into(), true);
    assert_eq!(resolve(&def, Some("0"), &file), (false, Source::Env));
    assert_eq!(resolve(&def, Some("off"), &file), (false, Source::Env));
    assert_eq!(resolve(&def, None, &file), (true, Source::File));
    assert_eq!(
        resolve(&def, None, &FileFlags::empty()),
        (false, Source::Default)
    );
    assert_eq!(resolve(&def, Some("nope"), &file), (true, Source::File));
}

#[test]
fn corrupt_file_reads_empty_with_one_warning() {
    let (file, warn) = parse_file(b"{nope");
    assert!(file.on.is_empty());
    assert!(file.auto);
    let warn = warn.expect("one warning");
    assert!(warn.contains("corrupt"), "{warn}");
    assert!(!warn.contains('\n'));
}

#[test]
fn write_is_0600_and_atomic() {
    let (dir, _keep) = scratch("mode");
    let path = dir.join("flags-stable.json");
    let mut file = FileFlags::empty();
    apply_set(&mut file, "go_start", false).unwrap();
    write_atomic(&path, &serialize_file(&file)).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(!dir.join("flags-stable.json.tmp").exists());
    assert!(!path.with_extension("json.tmp").exists());
    let (parsed, warn) = parse_file(&fs::read(&path).unwrap());
    assert!(warn.is_none());
    assert_eq!(parsed.on.get("go_start"), Some(&false));
    assert!(parsed.sticky_off.contains("go_start"));
    assert_eq!(parsed.src.get("go_start").map(String::as_str), Some("file"));
}

#[test]
fn unknown_id_errs() {
    let err = get("nope").unwrap_err();
    assert_eq!(err, "unknown flag nope");
    assert!(apply_set(&mut FileFlags::empty(), "nope", true).is_err());
    assert!(!on_loaded("nope", &FileFlags::empty()));
}

#[test]
fn policy_names_forbidden() {
    let banned = [
        "push",
        "merge",
        "stable",
        "send",
        "permission",
        "developer",
        "runner",
        "seal",
    ];
    for def in FLAGS {
        for word in banned {
            assert!(!def.id.contains(word), "{} contains {word}", def.id);
        }
    }
}

#[test]
fn go_permissions_never_reads_flags() {
    let src = include_str!("plans/go_runner.rs");
    assert!(!src.contains("flags::"));
}

#[test]
fn stale_marks_past_remove_after() {
    assert!(is_stale("2026-01-01", "2026-10-28"));
    assert!(!is_stale("2026-12-27", "2026-10-28"));
    assert!(!is_stale("2026-10-28", "2026-10-28"));
    let def = FlagDef {
        id: "old_row",
        default: false,
        what: "old",
        go: "GO-OLD",
        added: "2025-11-01",
        remove_after: "2026-01-01",
        auto_on_verify: false,
    };
    assert!(is_stale(def.remove_after, "2026-10-28"));
}

#[test]
fn ids_are_snake_and_unique() {
    let mut seen = BTreeSet::new();
    assert!(!FLAGS.is_empty());
    for def in FLAGS {
        assert!(id_ok(def.id), "{}", def.id);
        assert!(seen.insert(def.id), "duplicate {}", def.id);
        assert!(!def.what.is_empty(), "{}", def.id);
        assert!(def.go.starts_with("GO-"), "{}", def.id);
        assert_eq!(def.added.len(), 10, "{}", def.id);
        assert_eq!(def.remove_after.len(), 10, "{}", def.id);
    }
    assert!(def_by_id("tabbity_bridge").unwrap().default);
    assert!(def_by_id("go_start").unwrap().default);
}

#[test]
fn go_start_off_refuses() {
    let def = def_by_id("go_start").unwrap();
    let (on, _) = resolve(def, Some("0"), &FileFlags::empty());
    assert!(!on);
    let msg = refusal("go.start", "go_start");
    assert_eq!(
        msg,
        "go.start is off (flag go_start). orbittyctl flags set go_start on"
    );
    let (on, source) = resolve(def, None, &FileFlags::empty());
    assert!(on);
    assert_eq!(source, Source::Default);
    let src = include_str!("agents/control.rs");
    let start = src.find("\"go.start\" =>").expect("arm");
    let body = &src[start..];
    let prep = body.find("prepare_start").expect("prepare");
    let gate = body.find("blocked(").expect("flag gate");
    assert!(gate < prep, "flag check sits above prepare_start");
    let ui = include_str!("lib.rs");
    let fn_at = ui.find("fn go_start(").expect("fn");
    let fn_body = &ui[fn_at..];
    let fn_end = fn_body[1..]
        .find("\nfn ")
        .map(|n| n + 1)
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..fn_end];
    let prep = fn_body.find("prepare_start").expect("prepare");
    let gate = fn_body.find("blocked(").expect("flag gate");
    assert!(gate < prep);
}

#[test]
fn print_stale() {
    let today = today_ct();
    for def in FLAGS {
        if is_stale(def.remove_after, &today) {
            println!("NOTE stale flag {}", def.id);
        }
    }
}

#[test]
fn nightly_flip_leaves_stable() {
    let (dir, _keep) = scratch("channel");
    let nightly = flags_path(&dir, Channel::Nightly);
    let stable = flags_path(&dir, Channel::Stable);
    assert_ne!(nightly, stable);
    let mut file = FileFlags::empty();
    apply_set(&mut file, "go_start", false).unwrap();
    write_atomic(&nightly, &serialize_file(&file)).unwrap();
    let (night, warn) = read_disk(&nightly);
    assert!(warn.is_none());
    assert_eq!(night.on.get("go_start"), Some(&false));
    let (day, warn) = read_disk(&stable);
    assert!(warn.is_none());
    assert!(day.on.get("go_start").is_none());
    let def = def_by_id("go_start").unwrap();
    let (on, source) = resolve(def, None, &day);
    assert!(on);
    assert_eq!(source, Source::Default);
    let io = include_str!("state/io.rs");
    assert!(io.contains("dir.join(\"state.json\")"));
    assert!(!io.contains("flags-nightly"));
    assert!(!io.contains("flags-stable"));
    assert_eq!(channel_of("space.orbitty.mac.nightly"), Channel::Nightly);
    assert_eq!(channel_of("space.orbitty.mac"), Channel::Stable);
    assert_eq!(channel_of("space.orbitty.alpha"), Channel::Alpha);
}

#[test]
fn herdr_core_default_on_env_and_file_still_win() {
    let def = def_by_id("herdr_core").expect("herdr_core row");
    assert!(def.default, "herdr_core default on");
    assert!(!def.auto_on_verify, "auto_on_verify stays false");
    assert_eq!(def.what, "run seats in herdr-core");
    let (on, source) = resolve(def, None, &FileFlags::empty());
    assert!(on);
    assert_eq!(source, Source::Default);
    let (off, source) = resolve(def, Some("0"), &FileFlags::empty());
    assert!(!off);
    assert_eq!(source, Source::Env);
    let (on_env, source) = resolve(def, Some("on"), &FileFlags::empty());
    assert!(on_env);
    assert_eq!(source, Source::Env);
    let mut file = FileFlags::empty();
    file.on.insert("herdr_core".into(), false);
    file.src.insert("herdr_core".into(), "file".into());
    let (off_file, source) = resolve(def, None, &file);
    assert!(!off_file, "flag file beats the new default");
    assert_eq!(source, Source::File);
    let (env_wins, source) = resolve(def, Some("1"), &file);
    assert!(env_wins, "env beats the flag file");
    assert_eq!(source, Source::Env);
    let (bad_env, source) = resolve(def, Some("nope"), &file);
    assert!(!bad_env, "invalid env falls through to the file");
    assert_eq!(source, Source::File);
}

#[test]
fn plist_tip_is_sha8() {
    let text = r#"<key>OrbittyGitCommit</key>
<string>8E017A86D46A1E9E92DC90E006A8DD5B89027163</string>"#;
    assert_eq!(tip_sha8_from_plist(text).as_deref(), Some("8e017a86"));
    assert!(tip_sha8_from_plist("<key>Other</key>").is_none());
}
