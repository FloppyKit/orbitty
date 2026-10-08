    use super::*;

    fn fp_window(start: u32, n: u32) -> Vec<String> {
        (start..start + n)
            .map(|i| fingerprint("you", &format!("line {i}")))
            .collect()
    }

    #[test]
    fn release_gate_and_seed_follow_the_dev_flag() {
        assert!(release_gate(true).is_ok());
        assert!(release_gate(false).unwrap_err().contains("dev-only"));
        assert!(seed_thin_row(true));
        assert!(!seed_thin_row(false));
    }

    #[test]
    fn button_and_shortcut_share_one_predicate() {
        assert!(routes_to_companion_thin("thin", "anything"));
        assert!(routes_to_companion_thin(
            "Thin",
            "python3 /tmp/vivo_thin.py stamp"
        ));
        assert!(routes_to_companion_thin(
            "custom",
            "orbittyctl companion thin"
        ));
        assert!(routes_to_companion_thin("custom", "companion.thin"));
        assert!(!routes_to_companion_thin(
            "companion",
            "orbittyctl companion open"
        ));
        assert!(!routes_to_companion_thin(
            "note",
            "orbittyctl companion thinking"
        ));
        assert!(!routes_to_companion_thin(
            "kanban",
            "orbittyctl kanban open"
        ));
    }

    #[test]
    fn defaults_are_not_the_research_nest_or_homebrew() {
        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-cfg-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let cfg = resolve_config(&home, Path::new("/work/src-tauri"));
        assert_eq!(cfg.python, "python3");
        assert!(!cfg.python.contains("homebrew"));
        assert_eq!(cfg.script, Path::new("/work/scripts/thin/vivo_thin.py"));
        assert!(check_paths(&home, &cfg).is_ok(), "the default config passes check_paths");
        assert_eq!(cfg.output_dir, home.join(".config/orbitty/thin-stamps"));
        assert!(!cfg.output_dir.to_string_lossy().contains(".buzz"));
        assert!(!cfg.script.to_string_lossy().contains(".buzz"));

        let dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("thin.json"),
            r#"{"script":"~/bin/vivo_thin.py","python":"python3.12","outputDir":"~/stamps"}"#,
        )
        .unwrap();
        let cfg = resolve_config(&home, Path::new("/work/src-tauri"));
        assert_eq!(cfg.script, home.join("bin/vivo_thin.py"));
        assert_eq!(cfg.python, "python3.12");
        assert_eq!(cfg.output_dir, home.join("stamps"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn context_size_rounds_tokens_up() {
        assert_eq!(context_size(&[]), (0, 0));
        assert_eq!(context_size(&["abcd".into()]), (4, 1));
        assert_eq!(context_size(&["abcde".into(), "x".into()]), (6, 2));
    }

    #[test]
    fn turn_count_is_not_the_capped_index() {
        let now = 1_000u64;
        let window = fp_window(0, 40);
        let (meter, obs) = reconcile(&Meter::default(), "grok:s1", &window, 0, 10, now);
        assert_eq!(obs.turn_count, 40);
        assert_eq!(obs.last_reset_reason, "meter_start");
        assert_eq!(obs.secs_since_reset, 0);
        assert_eq!(obs.session_id, "grok:s1");

        // 15 oldest dropped, 15 new. The list is still 40 long.
        let slid = fp_window(15, 40);
        let (meter, obs) = reconcile(&meter, "grok:s1", &slid, 15, 25, now + 9);
        assert_eq!(obs.turn_count, 55);
        assert_ne!(obs.turn_count, slid.len() as u64);
        assert_eq!(obs.secs_since_reset, 9);
        assert_eq!(obs.last_reset_reason, "meter_start");

        // Whole window replaced; the event log saw 80 mouth turns.
        let replaced = fp_window(200, 40);
        let (_meter, obs) = reconcile(&meter, "grok:s1", &replaced, 80, 105, now + 20);
        assert_eq!(obs.turn_count, 135);
        assert!(obs.turn_count > 40);
    }

    #[test]
    fn session_change_resets_the_clock_and_reason() {
        let window = fp_window(0, 10);
        let (meter, _) = reconcile(&Meter::default(), "grok:old", &window, 0, 4, 50);
        let fresh = fp_window(90, 40);
        let (_meter, obs) = reconcile(&meter, "grok:new", &fresh, 70, 80, 90);
        assert_eq!(obs.last_reset_reason, "session_changed");
        assert_eq!(obs.turn_count, 70);
        assert_eq!(obs.secs_since_reset, 0);
        assert_eq!(obs.reset_at, 90);
        assert_eq!(obs.session_id, "grok:new");
    }

    #[test]
    fn same_stamp_twice_does_not_add_turns() {
        let window = fp_window(0, 5);
        let (meter, obs) = reconcile(&Meter::default(), "grok:s", &window, 0, 3, 10);
        assert_eq!(obs.turn_count, 5);
        let (_meter, obs) = reconcile(&meter, "grok:s", &window, 0, 3, 15);
        assert_eq!(obs.turn_count, 5);
        assert_eq!(obs.secs_since_reset, 5);
    }

    #[test]
    fn event_lines_count_only_mouth_turns_after_the_cursor() {
        let log = "\
{\"tags\":[[\"e-type\",\"companion-send\"]]}\n\
{\"tags\":[[\"e-type\",\"action-start\"]]}\n\
{\"tags\":[[\"e-type\",\"companion-reply\"]]}\n\
not json companion-send\n";
        let (n, total) = count_turn_events(log, 0);
        assert_eq!(total, 4);
        assert_eq!(n, 2);
        let (n, total) = count_turn_events(log, 2);
        assert_eq!(total, 4);
        assert_eq!(n, 1);
        let (n, _) = count_turn_events(log, 99);
        assert_eq!(n, 0);
    }

    #[test]
    fn script_writes_one_stamp_and_does_not_shell_out() {
        let src = include_str!("../../../scripts/thin/vivo_thin.py");
        assert!(!src.contains("orbittyctl"));
        assert!(!src.contains(".buzz"));
        assert!(!src.contains("/opt/homebrew"));
        assert!(!src.contains("last_error"));
        assert!(!src.contains("nod"));

        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-stamp-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let out_dir = home.join("stamps");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/thin/vivo_thin.py");
        let payload = json!({
            "session_id": "grok:test-session",
            "turn_count": 57,
            "context_chars": 11,
            "context_tokens_est": 3,
            "secs_since_reset": 12,
            "last_reset_reason": "meter_start",
            "ts": "2026-09-29T22:15:00Z",
            "mode": "spark",
            "turns": [
                {"role": "you", "text": "hello"},
                {"role": "them", "text": "there"}
            ]
        });
        let mut child = Command::new("python3")
            .arg(&script)
            .arg("stamp")
            .env("ORBITTY_THIN_OUTPUT", &out_dir)
            .env("HOME", &home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3");
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload.to_string().as_bytes())
                .unwrap();
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "stderr {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let events = std::fs::read_to_string(out_dir.join("events.jsonl")).unwrap();
        let ev: Value = serde_json::from_str(events.lines().next().unwrap()).unwrap();
        assert_eq!(ev["session_id"], "grok:test-session");
        assert_eq!(ev["turn_count"], 57);
        assert_eq!(ev["context_chars"], 11);
        assert_eq!(ev["context_tokens_est"], 3);
        assert_eq!(ev["secs_since_reset"], 12);
        assert_eq!(ev["last_reset_reason"], "meter_start");
        assert_eq!(ev["ts"], "2026-09-29T22:15:00Z");
        assert_eq!(ev["human_stamp_turn"], 57);
        let sid = ev["id"].as_str().unwrap();
        let turns = std::fs::read_to_string(out_dir.join(sid).join("turns.jsonl")).unwrap();
        let rows: Vec<Value> = turns
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(rows[0]["turn"].as_i64(), Some(56));
        assert_eq!(rows[1]["turn"].as_i64(), Some(57));
        assert!(out_dir.join(sid).join("event.json").is_file());
        assert!(!home.join(".buzz").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn script_refuses_without_an_output_dir() {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/thin/vivo_thin.py");
        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-noout-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let mut child = Command::new("python3")
            .arg(&script)
            .arg("stamp")
            .env("HOME", &home)
            .env_remove("ORBITTY_THIN_OUTPUT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Close stdin so a buggy script cannot block on a payload read
        // before it notices the missing directory.
        drop(child.stdin.take());
        let out = child.wait_with_output().unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("output dir"), "{err}");
        assert!(!home.join(".buzz").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    /// The body of `fn <name>` in `src` (up to the first line that is a lone `}`).
    pub(crate) fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
        let at = src.find(sig).unwrap_or_else(|| panic!("missing {sig}"));
        let rest = &src[at..];
        let end = rest.find("\n}\n").unwrap_or(rest.len());
        &rest[..end]
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "orbitty-thin-{tag}-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    // B1: the gate and the seeding use one predicate, and it is the dev-thin cfg.
    #[test]
    fn gate_and_seed_share_the_dev_thin_cfg() {
        assert_eq!(THIN_ENABLED, cfg!(any(debug_assertions, feature = "dev-thin")));
        let coord = concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
);
        let actions = include_str!("actions.rs");
        let thin = fn_body(coord, "pub fn companion_thin(");
        assert!(thin.contains("release_gate(crate::thin::THIN_ENABLED)"), "{thin}");
        let bake = fn_body(actions, "pub fn bake_defaults(");
        assert!(bake.contains("seed_thin_row(crate::thin::THIN_ENABLED)"), "{bake}");
        // No thin item may be gated on debug_assertions alone.
        let bare = format!("cfg!({})", "debug_assertions");
        assert!(!thin.contains(&bare) && !bake.contains(&bare));
        let src = include_str!("thin.rs");
        for needle in [
            format!("#[cfg({})]", "debug_assertions"),
            format!("#[cfg(not({}))]", "debug_assertions"),
        ] {
            assert!(!src.contains(&needle), "thin.rs still has {needle}");
        }
        let cargo = include_str!("../../Cargo.toml");
        assert!(cargo.contains("\n[features]\n") && cargo.contains("\ndev-thin = []\n"));
        let nightly = include_str!("../../../scripts/install-nightly.sh");
        assert!(nightly.contains("npm run tauri build -- --features dev-thin"));
        let stable = include_str!("../../../scripts/install-stable.sh");
        assert!(!stable.contains("dev-thin"));
    }

    // N2 (T2, T13): the gate runs first; exactly one nod per attempt.
    #[test]
    fn thin_flow_gates_first_and_nods_once() {
        use std::cell::Cell;
        let (stamps, nods, notes) = (Cell::new(0), Cell::new(0), Cell::new(0));
        let r = thin_flow(
            release_gate(false),
            || {
                stamps.set(stamps.get() + 1);
                Ok("x".into())
            },
            || nods.set(nods.get() + 1),
            |_| {
                notes.set(notes.get() + 1);
                Ok(())
            },
        );
        assert!(r.unwrap_err().contains("dev-only"));
        assert_eq!((stamps.get(), nods.get(), notes.get()), (0, 0, 0));

        let r = thin_flow(
            release_gate(true),
            || {
                stamps.set(stamps.get() + 1);
                Ok("stamped".into())
            },
            || nods.set(nods.get() + 1),
            |_| {
                notes.set(notes.get() + 1);
                Ok(())
            },
        );
        assert_eq!(r.unwrap(), "stamped");
        assert_eq!((stamps.get(), nods.get(), notes.get()), (1, 1, 0));

        let noted = std::cell::RefCell::new(String::new());
        let r = thin_flow(
            Ok(()),
            || Err(StampError::failed("boom")),
            || nods.set(nods.get() + 1),
            |l| {
                *noted.borrow_mut() = l;
                Ok(())
            },
        );
        assert_eq!(r.unwrap(), "THIN stamp failed. boom");
        assert_eq!(*noted.borrow(), "THIN stamp failed. boom");
        assert_eq!(nods.get(), 2);

        // companion_thin wires the real gate, the real stamp, and one nod.
        let body = fn_body(concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
), "pub fn companion_thin(");
        assert!(body.contains("crate::thin::thin_flow("), "{body}");
        assert!(body.contains("|| crate::thin::stamp(&app)"), "{body}");
        assert_eq!(body.matches("companion_play(").count(), 1, "{body}");
        assert_eq!(body.matches("\"nod\"").count(), 1, "{body}");
    }

    // N2 (T11, T12): no landed stamp = failure; the meter moves only after landing.
    #[test]
    fn meter_advances_only_after_a_landed_stamp() {
        let log = std::cell::RefCell::new(Vec::<&str>::new());
        let r = stamp_steps(
            || {
                log.borrow_mut().push("run");
                Ok("ok".into())
            },
            || {
                log.borrow_mut().push("landed");
                true
            },
            || log.borrow_mut().push("save"),
        );
        assert_eq!(r.ok().as_deref(), Some("ok"));
        assert_eq!(*log.borrow(), vec!["run", "landed", "save"]);

        log.borrow_mut().clear();
        let r = stamp_steps(
            || {
                log.borrow_mut().push("run");
                Ok("ok".into())
            },
            || false,
            || log.borrow_mut().push("save"),
        );
        assert!(r.err().unwrap().user_line().contains("no stamp landed"));
        assert_eq!(*log.borrow(), vec!["run"]);

        log.borrow_mut().clear();
        let r = stamp_steps(
            || Err(StampError::failed("exit 2")),
            || {
                log.borrow_mut().push("landed");
                true
            },
            || log.borrow_mut().push("save"),
        );
        assert!(r.is_err());
        assert!(log.borrow().is_empty());
    }

    // N2 (T11): the landing check really reads the last events.jsonl line.
    #[test]
    #[cfg(any(debug_assertions, feature = "dev-thin"))]
    fn landing_check_matches_the_last_event() {
        let dir = scratch("landed");
        let obs = Observation {
            session_id: "grok:s".into(),
            turn_count: 7,
            secs_since_reset: 3,
            last_reset_reason: "meter_start".into(),
            reset_at: 1,
        };
        assert!(!stamp_landed(&dir, &obs), "no events file");
        let good = r#"{"session_id":"grok:s","turn_count":7,"context_chars":1,"context_tokens_est":1,"secs_since_reset":3,"last_reset_reason":"meter_start","ts":"t"}"#;
        std::fs::write(dir.join("events.jsonl"), format!("{good}\n")).unwrap();
        assert!(stamp_landed(&dir, &obs));
        let stale = good.replace("\"turn_count\":7", "\"turn_count\":6");
        std::fs::write(dir.join("events.jsonl"), format!("{good}\n{stale}\n")).unwrap();
        assert!(!stamp_landed(&dir, &obs), "last line is an older stamp");
        let other = good.replace("grok:s", "grok:other");
        std::fs::write(dir.join("events.jsonl"), format!("{other}\n")).unwrap();
        assert!(!stamp_landed(&dir, &obs));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // N5: thin.json may not point stamps (or the script) into ~/.buzz, or be relative.
    #[test]
    fn thin_json_refuses_buzz_and_relative_paths() {
        let home = scratch("buzzguard");
        let buzz = home.join(".buzz");
        std::fs::create_dir_all(buzz.join("RESEARCH")).unwrap();
        let cfg_dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let manifest = Path::new("/work/src-tauri");
        let check = |out: &str| {
            std::fs::write(
                cfg_dir.join("thin.json"),
                format!(r#"{{"outputDir":{}}}"#, serde_json::to_string(out).unwrap()),
            )
            .unwrap();
            check_paths(&home, &resolve_config(&home, manifest))
        };
        assert!(check("~/stamps").is_ok());
        assert!(check("~/.config/orbitty/thin-stamps").is_ok());
        assert!(check("~/.buzzard/stamps").is_ok());
        for bad in [
            "~/.buzz",
            "~/.buzz/RESEARCH/stamps",
            "~/.BUZZ/research/new",
            "$HOME/.Buzz/x",
        ] {
            let e = check(bad).expect_err(bad);
            assert!(e.contains("inside ~/.buzz"), "{bad}: {e}");
        }
        // `..` is refused even when the folder it climbs out of is missing.
        let through = format!("{}/.buzz/../.buzz/y", home.display());
        for dots in [
            "~/missing/../.buzz/x",
            "~/a/b/../../.buzz/new",
            "~/missing/../stamps",
            through.as_str(),
        ] {
            let e = check(dots).expect_err(dots);
            assert!(e.contains("may not contain .."), "{dots}: {e}");
        }
        assert!(!buzz.join("x").exists() && !buzz.join("new").exists());
        for rel in ["stamps", "./stamps", "~dude/stamps", "~user"] {
            let e = check(rel).expect_err(rel);
            assert!(e.contains("absolute"), "{rel}: {e}");
        }
        // A symlink into .buzz, a dangling one to a .buzz path not made yet,
        // and the firmlink path to the same folder.
        std::os::unix::fs::symlink(buzz.join("RESEARCH"), home.join("link")).unwrap();
        assert!(check("~/link/stamps").is_err());
        std::os::unix::fs::symlink(buzz.join("not-yet"), home.join("dangle")).unwrap();
        assert!(check("~/dangle").is_err());
        let firm = PathBuf::from(format!("/System/Volumes/Data{}", home.display()));
        if firm.is_dir() && std::fs::metadata(&firm).is_ok() {
            assert!(check(&format!("{}/.buzz/RESEARCH", firm.display())).is_err());
        }
        // The legacy ~/.buzz script is refused as the scanner too.
        std::fs::write(
            cfg_dir.join("thin.json"),
            r#"{"script":"~/.buzz/RESEARCH/RGL_TESTBED/scripts/vivo_thin.py"}"#,
        )
        .unwrap();
        let e = check_paths(&home, &resolve_config(&home, manifest)).unwrap_err();
        assert!(e.contains("script") && e.contains("inside ~/.buzz"), "{e}");
        // Nothing was created under .buzz by the checks.
        let under: Vec<_> = std::fs::read_dir(&buzz).unwrap().collect();
        assert_eq!(under.len(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }

    // stamp() runs the path check before it creates the output dir.
    #[test]
    fn stamp_checks_paths_before_mkdir() {
        let body = fn_body(include_str!("thin.rs"), "pub fn stamp(app: &tauri::AppHandle)");
        let check = body.find("check_paths(&home, &cfg)").expect("check_paths call");
        let mkdir = body.find("create_dir_all(&cfg.output_dir)").expect("mkdir");
        assert!(check < mkdir);
        // The meter is saved in exactly one place, inside stamp_steps (after
        // the landing check), never on its own before the script runs.
        let steps = body.find("stamp_steps(").expect("stamp_steps call");
        assert_eq!(body.matches("save_meter(").count(), 1, "{body}");
        assert!(body.find("save_meter(").unwrap() > steps, "{body}");
    }

    // N-H: a thin.json python *path* gets the .buzz, relative and `..` checks.
    #[test]
    fn thin_json_python_path_is_checked_too() {
        let home = scratch("pyguard");
        std::fs::create_dir_all(home.join(".buzz/RESEARCH/RGL_TESTBED/.venv/bin")).unwrap();
        let cfg_dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let check = |py: &str| {
            std::fs::write(
                cfg_dir.join("thin.json"),
                format!(r#"{{"python":{}}}"#, serde_json::to_string(py).unwrap()),
            )
            .unwrap();
            check_paths(&home, &resolve_config(&home, Path::new("/work/src-tauri")))
        };
        assert!(check("python3").is_ok(), "bare name on PATH");
        assert!(check("python3.12").is_ok());
        assert!(check("/usr/bin/python3").is_ok());
        assert!(check("~/.pyenv/shims/python").is_ok());
        for bad in [
            "~/.buzz/RESEARCH/RGL_TESTBED/.venv/bin/python",
            "$HOME/.BUZZ/RESEARCH/RGL_TESTBED/.venv/bin/python",
            &format!("{}/.Buzz/x/python", home.display()),
        ] {
            let e = check(bad).expect_err(bad);
            assert!(e.contains("python") && e.contains("inside ~/.buzz"), "{bad}: {e}");
        }
        let e = check("bin/python").unwrap_err();
        assert!(e.contains("python") && e.contains("absolute"), "{e}");
        let e = check("~/missing/../.buzz/py").unwrap_err();
        assert!(e.contains("may not contain .."), "{e}");
        let _ = std::fs::remove_dir_all(&home);
    }

    // N-G: both install scripts remove the built bundle before tauri build,
    // behind a guard that needs a non-empty SRC ending in .app.
    #[test]
    fn install_scripts_clear_the_bundle_before_building() {
        for (name, src) in [
            ("install-nightly.sh", include_str!("../../../scripts/install-nightly.sh")),
            ("install-stable.sh", include_str!("../../../scripts/install-stable.sh")),
        ] {
            let guard = r#"if [[ -n "${SRC:-}" && "$SRC" == *.app && "$SRC" == src-tauri/target/release/bundle/macos/* ]]; then"#;
            let g = src.find(guard).unwrap_or_else(|| panic!("{name}: guard missing"));
            let rm = src.find("  rm -rf -- \"$SRC\"\n").unwrap_or_else(|| panic!("{name}: rm missing"));
            let build = src.find("\nnpm run tauri build").unwrap_or_else(|| panic!("{name}: build"));
            assert!(g < rm && rm < build, "{name}: rm must be guarded and before tauri build");
            assert_eq!(src.matches("rm -rf -- \"$SRC\"").count(), 1, "{name}: one bundle rm");
            let set = src.find("\nSRC=\"src-tauri/target/release/bundle/macos/").expect("SRC set");
            assert!(set < g, "{name}: SRC is set before the rm");
        }
    }
