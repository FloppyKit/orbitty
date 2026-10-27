use std::path::{Path, PathBuf};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;

    fn temp_roots(label: &str) -> Roots {
        let p = std::env::temp_dir().join(format!(
            "orbitty-plans-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Roots { home: p }
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn fresh_plans_dir_is_visible_home_folder() {
        let roots = temp_roots("a0");
        let p = plans_dir(&roots);
        assert_eq!(p, roots.home.join("Orbitty GOs"));
        #[cfg(not(feature = "alpha"))]
        assert!(p.is_dir());
        #[cfg(feature = "alpha")]
        assert!(!p.exists());
        assert!(!p.to_string_lossy().contains("Library"));
        let bus = bus_dir_in(&roots);
        assert_eq!(bus, roots.home.join(".config/orbitty/needed-bus"));
        assert!(!bus.to_string_lossy().contains("Library"));
    }

    #[test]
    fn under_buzz_is_canonical_and_case_insensitive() {
        let roots = temp_roots("underbuzz");
        let h = &roots.home;
        for p in [".buzz", ".buzz/PLANS", ".BUZZ/PLANS", ".Buzz/new/sub", "x/../.bUzZ"] {
            assert!(under_buzz(&roots, &h.join(p)), "{p} (no .buzz on disk)");
        }
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("link")).unwrap();
        for p in [".BUZZ/PLANS", ".buzz/./PLANS", "link/PLANS", "nope/../.Buzz/x"] {
            assert!(under_buzz(&roots, &h.join(p)), "{p}");
        }
        for p in [".buzzard", "buzz/PLANS", ".buzz-old", "Orbitty GOs"] {
            assert!(!under_buzz(&roots, &h.join(p)), "{p}");
        }
    }

    #[test]
    fn hand_edited_buzz_plans_dir_falls_back_to_default() {
        let roots = temp_roots("editbuzz");
        let h = roots.home.clone();
        let default = h.join("Orbitty GOs");
        let hs = h.display().to_string();
        for raw in [
            format!("{hs}/.BUZZ/PLANS"),
            "~/.buzz/PLANS".to_string(),
            "~/.Buzz/new/sub".to_string(),
        ] {
            // Hand-edited, not through set_plans_dir.
            write(&settings_path(&roots), &json!({ "plansDir": raw }).to_string());
            assert_eq!(plans_dir(&roots), default, "{raw}");
            #[cfg(not(feature = "alpha"))]
            assert!(default.is_dir());
            #[cfg(feature = "alpha")]
            assert!(!default.exists(), "{raw}");
            assert!(!h.join(".buzz").exists() && !h.join(".BUZZ").exists() && !h.join(".Buzz").exists(), "{raw}");
            assert_eq!(snapshot(&roots)["plansDir"], json!(default.display().to_string()));
        }
        // With a real ~/.buzz/PLANS: still the default, still read-only.
        write(&h.join(".buzz/PLANS/_GO_OLD_PROMPT.md"), "legacy\n");
        write(&settings_path(&roots), &json!({ "plansDir": format!("{hs}/.BUZZ/PLANS") }).to_string());
        assert_eq!(plans_dir(&roots), default);
        let kids: Vec<_> = std::fs::read_dir(h.join(".buzz/PLANS")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(kids, vec![std::ffi::OsString::from("_GO_OLD_PROMPT.md")]);
        assert_eq!(read_dirs_in(&roots), vec![default.clone(), h.join(".buzz/PLANS")]);
    }

    #[test]
    fn go_seal_refuses_a_file_inside_buzz() {
        let roots = temp_roots("sealbuzz");
        let h = roots.home.clone();
        let text = "---\nid: GO-SEALBUZZ\nstatus: ready\n---\nBODY\n";
        let inside = h.join(".buzz/PLANS/GO-SEALBUZZ.md");
        write(&inside, text);
        for p in [inside.clone(), h.join(".BUZZ/PLANS/GO-SEALBUZZ.md"), h.join("x/../.Buzz/PLANS/GO-SEALBUZZ.md")] {
            let err = seal_file_in(&roots, &p, "matt").unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{}: {err}", p.display());
            assert_eq!(std::fs::read_to_string(&inside).unwrap(), text, "{}", p.display());
        }
        // The same text outside .buzz seals, so the refusal is the guard.
        let outside = plans_dir(&roots).join("GO-SEALBUZZ.md");
        write(&outside, text);
        seal_file_in(&roots, &outside, "matt").unwrap();
        assert_ne!(std::fs::read_to_string(&outside).unwrap(), text);
    }

    /// `<home>` spelled through the /System/Volumes/Data firmlink, or None
    /// when this machine has no such firmlink for the temp dir.
    fn firmlink_home(roots: &Roots) -> Option<PathBuf> {
        let real = std::fs::canonicalize(&roots.home).ok()?;
        let fw = PathBuf::from(format!("/System/Volumes/Data{}", real.display()));
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (std::fs::metadata(&fw).ok()?, std::fs::metadata(&roots.home).ok()?);
        ((a.dev(), a.ino()) == (b.dev(), b.ino())).then_some(fw)
    }

    #[test]
    fn firmlink_spelling_of_buzz_is_refused() {
        let roots = temp_roots("fwbuzz");
        let Some(fw) = firmlink_home(&roots) else {
            eprintln!("no /System/Volumes/Data firmlink; skipped");
            return;
        };
        let h = roots.home.clone();
        // Before ~/.buzz exists.
        assert!(under_buzz(&roots, &fw.join(".buzz/PLANS")));
        assert!(!under_buzz(&roots, &fw.join("Orbitty GOs")));
        assert!(set_plans_dir_in(&roots, &fw.join(".Buzz/PLANS").display().to_string()).is_err());
        assert!(!h.join(".buzz").exists());
        // With ~/.buzz/PLANS on disk.
        let text = "---\nid: GO-FW\nstatus: ready\n---\nBODY\n";
        write(&h.join(".buzz/PLANS/GO-FW.md"), text);
        for p in [fw.join(".buzz/PLANS"), fw.join(".BUZZ/new/sub")] {
            assert!(under_buzz(&roots, &p), "{}", p.display());
            let err = set_plans_dir_in(&roots, &p.display().to_string()).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none());
        write(&settings_path(&roots), &json!({ "plansDir": fw.join(".buzz/PLANS") }).to_string());
        assert_eq!(plans_dir(&roots), h.join("Orbitty GOs"));
        let err = seal_file_in(&roots, &fw.join(".buzz/PLANS/GO-FW.md"), "matt").unwrap_err();
        assert!(err.contains("inside ~/.buzz"), "{err}");
        assert_eq!(std::fs::read_to_string(h.join(".buzz/PLANS/GO-FW.md")).unwrap(), text);
    }

    #[test]
    fn symlink_loop_and_unreadable_parent_fail_closed() {
        use std::os::unix::fs::PermissionsExt;
        struct Unlock(PathBuf);
        impl Drop for Unlock {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let roots = temp_roots("failclosed");
        let h = roots.home.clone();
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        // P3: symlink loop.
        std::os::unix::fs::symlink(h.join("loop"), h.join("loop")).unwrap();
        // P3b: a chmod 000 parent hiding a link to .buzz.
        std::fs::create_dir_all(h.join("locked")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("locked/link")).unwrap();
        std::fs::set_permissions(h.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
        let _unlock = Unlock(h.join("locked"));
        for p in [h.join("loop/PLANS"), h.join("locked/link/PLANS")] {
            assert!(under_buzz(&roots, &p), "{}", p.display());
            let err = set_plans_dir_in(&roots, &p.display().to_string()).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{err}");
            write(&settings_path(&roots), &json!({ "plansDir": p }).to_string());
            assert_eq!(plans_dir(&roots), h.join("Orbitty GOs"));
            assert!(seal_file_in(&roots, &p.join("GO.md"), "matt").unwrap_err().contains("inside ~/.buzz"));
        }
    }

    #[test]
    fn set_plans_dir_requires_an_absolute_path() {
        let roots = temp_roots("relplans");
        for p in ["Orbitty GOs", "./GOs", ".buzz/PLANS", "~nobody/GOs", "~root/.buzz/PLANS"] {
            let err = set_plans_dir_in(&roots, p).unwrap_err();
            assert!(err.contains("absolute"), "{p}: {err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none());
        set_plans_dir_in(&roots, "~/GOs").unwrap();
        assert_eq!(plans_dir(&roots), roots.home.join("GOs"));
    }

    fn tree_of(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                let ft = e.file_type().unwrap();
                if ft.is_dir() {
                    stack.push(p.clone());
                    out.push((p, Vec::new()));
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap_or_default()));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn go_seal_refuses_a_hard_link_into_buzz() {
        let roots = temp_roots("hardlink");
        let h = roots.home.clone();
        let text = "---\nid: GO-HL\nstatus: ready\n---\nBODY\n";
        let target = h.join(".buzz/PLANS/GO-HL.md");
        write(&target, text);
        let link = plans_dir(&roots).join("GO-HL.md");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::fs::hard_link(&target, &link).unwrap();
        let before = tree_of(&h.join(".buzz"));
        let err = seal_file_in(&roots, &link, "matt").unwrap_err();
        assert!(err.contains("hard link"), "{err}");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), text);
        assert_eq!(tree_of(&h.join(".buzz")), before);
    }

    #[test]
    fn go_seal_replaces_the_file_by_atomic_rename() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let roots = temp_roots("sealrename");
        let dir = plans_dir(&roots);
        let p = dir.join("GO-R.md");
        write(&p, "---\nid: GO-R\nstatus: ready\n---\nBODY\n");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
        let ino = std::fs::metadata(&p).unwrap().ino();
        seal_file_in(&roots, &p, "matt").unwrap();
        let m = std::fs::metadata(&p).unwrap();
        assert_ne!(m.ino(), ino, "sealed by writing into the old inode");
        assert_eq!(m.permissions().mode() & 0o777, 0o640);
        assert!(std::fs::read_to_string(&p).unwrap().contains("status: sealed"));
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["GO-R.md".to_string()], "no temp file left");
    }

    #[test]
    fn migrate_refuses_a_plans_subfolder_linked_into_buzz() {
        let roots = temp_roots("migbuzz");
        let h = roots.home.clone();
        write(&h.join(".buzz/PLANS/sub/a.md"), "legacy\n");
        std::fs::create_dir_all(h.join(".buzz/OTHER")).unwrap();
        set_plans_dir_in(&roots, &h.join("GOs").display().to_string()).unwrap();
        std::fs::create_dir_all(h.join("GOs")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz/OTHER"), h.join("GOs/sub")).unwrap();
        let before = tree_of(&h.join(".buzz"));
        let err = migrate_apply(&roots).unwrap_err();
        assert!(err.contains("migrate:") && err.contains("inside ~/.buzz"), "{err}");
        assert_eq!(tree_of(&h.join(".buzz")), before);
        assert!(!h.join(".buzz/OTHER/a.md").exists());
    }

    #[test]
    fn buzz_fallback_warns_once() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        let first = buzz_warning_once(&flag, "~/.BUZZ/PLANS").expect("first call warns");
        assert!(first.contains("~/.BUZZ/PLANS") && first.contains("inside ~/.buzz"), "{first}");
        assert!(buzz_warning_once(&flag, "~/.buzz/X").is_none(), "second call is silent");
        // plans_dir() goes through the warning on every fallback.
        let roots = temp_roots("warnbuzz");
        write(&settings_path(&roots), &json!({ "plansDir": "~/.BUZZ/PLANS" }).to_string());
        let before = BUZZ_FALLBACKS.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(plans_dir(&roots), roots.home.join("Orbitty GOs"));
        assert!(BUZZ_FALLBACKS.load(std::sync::atomic::Ordering::Relaxed) > before, "no warning on fallback");
    }

    #[test]
    fn dangling_symlink_into_buzz_is_refused() {
        let roots = temp_roots("dangbuzz");
        let h = roots.home.clone();
        // ~/.buzz missing: a link to ~/.buzz/PLANS.
        std::os::unix::fs::symlink(h.join(".buzz/PLANS"), h.join("dang0")).unwrap();
        assert!(under_buzz(&roots, &h.join("dang0")));
        std::fs::create_dir_all(h.join(".buzz")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz/NEWDIR"), h.join("dang")).unwrap();
        std::os::unix::fs::symlink(".BUZZ/NEW2", h.join("dangrel")).unwrap();
        std::os::unix::fs::symlink(h.join("missing/elsewhere"), h.join("dangout")).unwrap();
        let before = tree_of(&h.join(".buzz"));
        for name in ["dang0", "dang", "dang/sub", "dangrel"] {
            let p = h.join(name);
            assert!(under_buzz(&roots, &p), "{name}");
            let err = set_plans_dir_in(&roots, &p.display().to_string()).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{name}: {err}");
            write(&settings_path(&roots), &json!({ "plansDir": p }).to_string());
            assert_eq!(plans_dir(&roots), h.join("Orbitty GOs"), "{name}");
        }
        assert!(!under_buzz(&roots, &h.join("dangout")), "a dangling link elsewhere is not .buzz");
        assert_eq!(tree_of(&h.join(".buzz")), before, "nothing created under .buzz");
        assert!(!h.join(".buzz/NEWDIR").exists());
    }

    #[test]
    fn set_plans_dir_refuses_buzz_in_any_case() {
        let roots = temp_roots("setbuzz");
        let h = roots.home.clone();
        std::fs::create_dir_all(h.join(".buzz/PLANS")).unwrap();
        std::os::unix::fs::symlink(h.join(".buzz"), h.join("buzzlink")).unwrap();
        let tree = |root: &Path| -> Vec<PathBuf> {
            let mut out = Vec::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p.clone());
                    }
                    out.push(p);
                }
            }
            out.sort();
            out
        };
        let before = tree(&h.join(".buzz"));
        let hs = h.display().to_string();
        for p in [
            format!("{hs}/.buzz/PLANS"),
            format!("{hs}/.BUZZ/PLANS"),
            format!("{hs}/.Buzz"),
            "~/.BUZZ/new/sub".to_string(),
            format!("{hs}/x/../.bUzZ/PLANS"),
            format!("{hs}/buzzlink/PLANS"),
        ] {
            let err = set_plans_dir_in(&roots, &p).unwrap_err();
            assert!(err.contains("inside ~/.buzz"), "{p}: {err}");
        }
        assert!(load_settings(&roots).plans_dir.is_none(), "nothing saved");
        assert_eq!(tree(&h.join(".buzz")), before, "nothing created under .buzz");
        assert!(!h.join(".BUZZ/new").exists());
        let ok = format!("{hs}/.buzzard");
        let snap = set_plans_dir_in(&roots, &ok).unwrap();
        assert_eq!(snap["plansDir"], json!(ok));
        assert_eq!(load_settings(&roots).plans_dir.as_deref(), Some(ok.as_str()));
    }

    #[test]
    fn setting_overrides_and_buzz_is_back_compat() {
        let roots = temp_roots("set");
        let custom = roots.home.join("custom-plans");
        let mut s = Settings::default();
        s.plans_dir = Some(custom.display().to_string());
        save_settings(&roots, &s).unwrap();
        assert_eq!(plans_dir(&roots), custom);
        let buzz = roots.home.join(".buzz/PLANS");
        write(&buzz.join("_GO_ONLY_BUZZ_PROMPT.md"), "legacy\n");
        let found = legacy_prompt_in(&roots, "GO-ONLY-BUZZ");
        assert_eq!(found.unwrap(), buzz.join("_GO_ONLY_BUZZ_PROMPT.md"));
    }

    #[test]
    fn header_parse_and_resolver_order() {
        let roots = temp_roots("resolve");
        let plans = plans_dir(&roots);
        let link = plans.join("link.md");
        let by_card = plans.join("card.md");
        let by_id = plans.join("id.md");
        let legacy = plans.join("_GO_ORDER_PROMPT.md");
        write(
            &link,
            "---\nid: GO-LINK\nstatus: sealed\nsealed_at: 2026-09-29T15:40:00-05:00\nsealed_by: matt\nseal_hash: placeholder\n---\nLINK-BODY\n",
        );
        write(
            &by_card,
            "---\nid: GO-CARD\ncard_id: c-1\nstatus: ready\n---\nCARD-BODY\n",
        );
        write(&by_id, "---\nid: GO-ORDER\nstatus: ready\n---\nID-BODY\n");
        write(&legacy, "no header\n");
        // Fix the link file's hash so it is actually sealed.
        seal_file_in(&roots, &link, "matt").unwrap();
        let url = format!("file://{}", link.display());
        match gate_in(&roots, "c-1", "GO-ORDER", &[url]) {
            Gate::Launch(g) => assert!(g.body.contains("LINK-BODY"), "{}", g.body),
            Gate::Refuse { note } => panic!("link should launch: {note}"),
            Gate::Skip => panic!("missing"),
        }
        match gate_in(&roots, "c-1", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("card_id should win and be unsealed: {other:?}"),
        }
        // Drop the card_id file so the id match is next.
        std::fs::remove_file(&by_card).unwrap();
        match gate_in(&roots, "c-9", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("GO-ORDER"), "{note}"),
            other => panic!("id match: {other:?}"),
        }
        std::fs::remove_file(&by_id).unwrap();
        match gate_in(&roots, "c-9", "GO-ORDER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("legacy"), "{note}"),
            other => panic!("legacy: {other:?}"),
        }
    }

    #[test]
    fn seal_hash_blocks_a_body_edit() {
        let roots = temp_roots("seal");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-SEAL\nstatus: ready\n---\nSENTINEL-LINE\n",
        );
        let sealed = seal_file_in(&roots, &p, "matt").unwrap();
        assert!(seal_note(&roots, &sealed).is_none());
        let raw = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, raw.replace("SENTINEL-LINE", "EDITED-LINE")).unwrap();
        let again = parse_file(&p).unwrap();
        let note = seal_note(&roots, &again).unwrap();
        assert!(note.contains("changed since seal"), "{note}");
    }

    #[test]
    fn malformed_header_is_an_error() {
        let roots = temp_roots("bad");
        let p = plans_dir(&roots).join("GO-BAD.md");
        write(&p, "---\nstatus: [\n---\nbody\n");
        match gate_in(&roots, "", "GO-BAD", &[]) {
            Gate::Refuse { note } => assert!(note.contains("malformed"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ruleset_precedence_header_yaml_json_default() {
        let roots = temp_roots("rules");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\ndefaults:\n  harness: manual\n  tools: [read]\nrules:\n  - id: r1\n    match: { go: \"GO-ORBITTY*\", project: orbitty }\n    runner: from-yaml\n    harness: cursor\n    model: grok-4.7-high\n    tools: [read, edit]\n",
        );
        let header = GoHeader {
            runner: Some("from-header".into()),
            ..GoHeader::default()
        };
        let run = resolve_run_in(&roots, &header, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "from-header");
        assert_eq!(run.harness, "cursor");
        assert_eq!(run.model, "grok-4.7-high");
        assert_eq!(run.source, "header");

        let blank = GoHeader::default();
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "from-yaml");
        assert_eq!(run.source, "RULES.yaml");

        std::fs::remove_file(plans.join("RULES.yaml")).unwrap();
        let bus = roots.home.join(".buzz/ops/needed-bus");
        write(
            &bus.join("rules.json"),
            r#"{"version":1,"rules":[{"id":"r-mu3ioy6o-w7z0ky","enabled":true,"params":{"flavor":"fact","go":"GO-ORBITTY*","project":"orbitty"},"answer":"grok on this planet"}]}"#,
        );
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "herdr-probe");
        assert_eq!(run.harness, "herdr");
        assert_eq!(run.model, "grok");
        assert_eq!(run.source, "rules.json");

        std::fs::remove_file(bus.join("rules.json")).unwrap();
        let run = resolve_run_in(&roots, &blank, "GO-ORBITTY-X", "orbitty", false);
        assert_eq!(run.runner, "orbitty-cursor");
        assert_eq!(run.model, "grok-4.7-high");
        assert_eq!(run.harness, "cursor");
        assert_eq!(run.source, "defaultRunner");
    }

    #[test]
    fn fallback_names_why_and_push_rules() {
        let run = ResolvedRun {
            runner: "orbitty-cursor".into(),
            harness: "cursor".into(),
            model: "grok-4.7-high".into(),
            tools: vec!["read".into(), "git_push".into()],
            push_allowed: false,
            fallback: Some(Fallback {
                runner: "herdr-probe".into(),
                harness: "herdr".into(),
                model: "grok".into(),
            }),
            source: "header".into(),
            fell_back: false,
            fallback_why: None,
        };
        assert!(push_decision(&run).unwrap_err().contains("refused"));
        let mut no_push_tool = run.clone();
        no_push_tool.tools = vec!["read".into()];
        no_push_tool.push_allowed = true;
        assert_eq!(push_decision(&no_push_tool).unwrap(), false);
        let mut allowed = run.clone();
        allowed.push_allowed = true;
        assert_eq!(push_decision(&allowed).unwrap(), true);
        assert!(push_rule(false).contains("NOT allowed"));
        assert!(push_rule(true).contains("allowed for this run"));
        let fb = apply_fallback(&run, false);
        assert!(fb.fell_back);
        assert_eq!(fb.runner, "herdr-probe");
        assert_eq!(fb.harness, "herdr");
        assert_eq!(fb.model, "grok");
        assert!(fb.fallback_why.unwrap().contains("unavailable"));
        assert_eq!(door_harness("herdr"), "grok");
    }

    #[test]
    fn legacy_allow_and_query_cap_and_full_text() {
        let roots = temp_roots("cap");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\nlegacy_allow:\n  - \"GO-OLD*\"\nrules: []\n",
        );
        let legacy = plans.join("_GO_OLD_ONE_PROMPT.md");
        write(&legacy, "SENTINEL-OLD\n");
        let parsed = parse_file(&legacy).unwrap();
        assert!(seal_note(&roots, &parsed).is_none());
        let blocked = plans.join("_GO_NEW_ONE_PROMPT.md");
        write(&blocked, "nope\n");
        match gate_in(&roots, "", "GO-NEW-ONE", &[]) {
            Gate::Refuse { note } => assert!(note.contains("legacy"), "{note}"),
            other => panic!("{other:?}"),
        }
        let go = plans.join("full.md");
        let lock = plans.join("lock.md");
        write(&lock, "LOCK-SENTINEL\n");
        write(
            &go,
            "---\nid: GO-FULL\nstatus: ready\nlock: \"{PLANS_DIR}/lock.md\"\nworkspace: \"{REPO_ROOT}\"\n---\nBODY-SENTINEL\n",
        );
        let parsed = parse_file(&go).unwrap();
        let block = go_block_in(&roots, &parsed);
        assert!(block.contains("BODY-SENTINEL"), "{block}");
        assert!(block.contains("LOCK-SENTINEL"), "{block}");
        assert!(block.contains(&repo_root(&roots).display().to_string()));
        let huge = "x".repeat(QUERY_CAP + 1);
        assert!(enforce_cap(&huge).unwrap_err().contains("64000"));
        assert!(enforce_cap("ok").is_ok());
    }

    fn sealed_go(roots: &Roots, name: &str, header: &str, body: &str) -> PathBuf {
        let p = plans_dir(roots).join(name);
        write(&p, &format!("---\n{header}---\n{body}"));
        seal_file_in(roots, &p, "matt").unwrap();
        p
    }

    #[test]
    fn seal_covers_header_and_lock() {
        let roots = temp_roots("sealhdr");
        let plans = plans_dir(&roots);
        let lock = plans.join("lock.md");
        write(&lock, "LOCK-V1\n");
        let p = sealed_go(
            &roots,
            "go.md",
            "id: GO-HDR\nstatus: ready\npush_allowed: false\ntools: [read, edit]\nrunner: orbitty-cursor\nharness: cursor\nmodel: grok-4.7-high\nlock: \"{PLANS_DIR}/lock.md\"\n",
            "BODY\n",
        );
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        let raw = std::fs::read_to_string(&p).unwrap();
        for (from, to) in [
            ("push_allowed: false", "push_allowed: true"),
            ("tools: [read, edit]", "tools: [read, edit, git_push]"),
            ("runner: orbitty-cursor", "runner: other"),
            ("harness: cursor", "harness: grok"),
            ("model: grok-4.7-high", "model: other"),
            ("lock: \"{PLANS_DIR}/lock.md\"", "lock: \"{PLANS_DIR}/other.md\""),
        ] {
            assert!(raw.contains(from), "{from}");
            std::fs::write(&p, raw.replace(from, to)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("header edit {from:?} must break the seal"));
            assert!(
                note.contains("changed since seal") || note.contains("seal check failed"),
                "{note}"
            );
        }
        std::fs::write(&p, &raw).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        write(&lock, "LOCK-V2\n");
        let note = seal_note(&roots, &parse_file(&p).unwrap()).expect("lock edit breaks the seal");
        assert!(note.contains("changed since seal"), "{note}");
        write(&lock, "LOCK-V1\n");
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        std::fs::remove_file(&lock).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_some());
    }

    #[test]
    fn no_seal_hash_reads_unsealed() {
        let roots = temp_roots("nohash");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-NOHASH\nstatus: sealed\nsealed_at: 2026-09-29T15:40:00-05:00\nsealed_by: matt\n---\nBODY\n",
        );
        let note = seal_note(&roots, &parse_file(&p).unwrap()).unwrap();
        assert!(note.contains("unsealed"), "{note}");
        assert!(!note.contains("changed since seal"), "{note}");
    }

    #[test]
    fn seal_keeps_header_comments_and_body() {
        let roots = temp_roots("comments");
        let p = plans_dir(&roots).join("go.md");
        let body = "# Title\n\nBODY  with  spaces\n";
        write(
            &p,
            &format!(
                "---\n# top comment\nid: GO-CMT\nstatus: ready   # old status\nrunner: orbitty-cursor  # runner note\n# future option (not now)\ntools: [read]\n---\n{body}"
            ),
        );
        seal_file_in(&roots, &p, "Chuck for Matt: night").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        for keep in [
            "# top comment\n",
            "runner: orbitty-cursor  # runner note\n",
            "# future option (not now)\n",
            "tools: [read]\n",
        ] {
            assert!(raw.contains(keep), "lost {keep:?}:\n{raw}");
        }
        assert!(raw.contains("status: sealed\n"), "{raw}");
        assert!(raw.contains("sealed_by: \"Chuck for Matt: night\"\n"), "{raw}");
        assert!(raw.ends_with(&format!("---\n{body}")), "{raw}");
        let g = parse_file(&p).unwrap();
        assert_eq!(g.header.sealed_by.as_deref(), Some("Chuck for Matt: night"));
        assert!(seal_note(&roots, &g).is_none());
        // Resealing is stable: still one seal_hash line, comments intact.
        seal_file_in(&roots, &p, "matt").unwrap();
        let again = std::fs::read_to_string(&p).unwrap();
        assert_eq!(again.matches("seal_hash:").count(), 1, "{again}");
        assert!(again.contains("# future option (not now)\n"));
    }

    #[test]
    fn empty_sealed_by_refuses() {
        let roots = temp_roots("emptyby");
        let p = sealed_go(&roots, "go.md", "id: GO-BY\nstatus: ready\n", "BODY\n");
        let raw = std::fs::read_to_string(&p).unwrap();
        let line = raw.lines().find(|l| l.starts_with("sealed_by:")).unwrap().to_string();
        for blank in ["sealed_by: \"\"", "sealed_by: \"   \"", "sealed_by:"] {
            std::fs::write(&p, raw.replace(&line, blank)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("{blank:?} must not launch"));
            assert!(note.contains("sealed_by"), "{note}");
        }
    }

    #[test]
    fn card_id_match_beats_id_match() {
        let roots = temp_roots("m3");
        let plans = plans_dir(&roots);
        // The id file is sealed and sorts first; the card file is not
        // sealed. Only the card_id-before-id order refuses here.
        write(
            &plans.join("z-card.md"),
            "---\nid: GO-M-THREE\ncard_id: c-3\nstatus: ready\n---\nCARD\n",
        );
        sealed_go(&roots, "b-id.md", "id: GO-M-THREE\nstatus: ready\n", "ID\n");
        match gate_in(&roots, "c-3", "GO-M-THREE", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("card_id file must win: {other:?}"),
        }
        // Without the card match, the sealed id file launches.
        match gate_in(&roots, "c-other", "GO-M-THREE", &[]) {
            Gate::Launch(g) => assert!(g.body.contains("ID")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn yaml_defaults_beat_rules_json() {
        let roots = temp_roots("m6");
        let plans = plans_dir(&roots);
        write(
            &plans.join("RULES.yaml"),
            "version: 1\ndefaults:\n  runner: yaml-default\n  harness: manual\n  model: m-default\nrules:\n  - id: other\n    match: { go: \"GO-ELSE*\" }\n    runner: nope\n",
        );
        write(
            &roots.home.join(".buzz/ops/needed-bus/rules.json"),
            r#"{"version":1,"rules":[{"id":"r1","enabled":true,"params":{"flavor":"fact","go":"GO-ORBITTY*","project":"orbitty"},"answer":"grok on this planet"}]}"#,
        );
        let run = resolve_run_in(&roots, &GoHeader::default(), "GO-ORBITTY-X", "orbitty", true);
        assert_eq!(run.runner, "yaml-default");
        assert_eq!(run.harness, "manual");
        assert_eq!(run.model, "m-default");
        assert_eq!(run.source, "RULES.yaml");
    }

    #[test]
    fn go_token_with_no_file_refuses_instead_of_title_launch() {
        let roots = temp_roots("nofile");
        match gate_in(&roots, "c-1", "GO-ORBITTY-NOTHING", &[]) {
            Gate::Refuse { note } => assert!(note.contains("no GO file"), "{note}"),
            other => panic!("must fail closed: {other:?}"),
        }
        // A card that is not a GO still launches with its title.
        assert!(matches!(gate_in(&roots, "c-1", "Fix the drawer", &[]), Gate::Skip));
    }

    #[test]
    fn headerless_go_file_refuses() {
        let roots = temp_roots("nohdr");
        write(
            &plans_dir(&roots).join("GO-ORBITTY-NOHDR_2026-09-29.md"),
            "# GO-ORBITTY-NOHDR\nno header here\n",
        );
        match gate_in(&roots, "", "GO-ORBITTY-NOHDR", &[]) {
            Gate::Refuse { note } => assert!(note.contains("has no GO header"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lowercase_header_id_is_found_and_gated() {
        let roots = temp_roots("lower");
        let plans = plans_dir(&roots);
        write(&plans.join("draft.md"), "---\nid: go-orbitty-lower\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-LOWER", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("{other:?}"),
        }
        std::fs::remove_file(plans.join("draft.md")).unwrap();
        sealed_go(&roots, "sealed.md", "id: go-orbitty-lower\nstatus: ready\n", "SEALED\n");
        match gate_in(&roots, "", "GO-ORBITTY-LOWER", &[]) {
            Gate::Launch(g) => assert!(g.body.contains("SEALED")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn go_nested_too_deep_refuses() {
        let roots = temp_roots("deep");
        let plans = plans_dir(&roots);
        let six = plans.join("1/2/3/4/5/6");
        let seven = six.join("7");
        write(&seven.join("go.md"), "---\nid: GO-ORBITTY-DEEP\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-DEEP", &[]) {
            Gate::Refuse { note } => assert!(note.contains("folders deep"), "{note}"),
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&seven).unwrap();
        write(&six.join("go.md"), "---\nid: GO-ORBITTY-DEEP\nstatus: ready\n---\nX\n");
        match gate_in(&roots, "", "GO-ORBITTY-DEEP", &[]) {
            Gate::Refuse { note } => assert!(note.contains("ready, not sealed"), "{note}"),
            other => panic!("six deep is still searched: {other:?}"),
        }
    }

    #[test]
    fn malformed_header_without_token_in_name_refuses() {
        let roots = temp_roots("malother");
        write(
            &plans_dir(&roots).join("notes.md"),
            "---\nid: GO-ORBITTY-MAL\nstatus: [\n---\nbody\n",
        );
        match gate_in(&roots, "", "GO-ORBITTY-MAL", &[]) {
            Gate::Refuse { note } => assert!(note.contains("malformed"), "{note}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn status_query_is_the_full_launch_text() {
        let roots = temp_roots("query");
        let plans = plans_dir(&roots);
        write(&plans.join("lock.md"), "LOCK-SENTINEL {REPO_ROOT}\n");
        let p = sealed_go(
            &roots,
            "go.md",
            "id: GO-Q\nstatus: ready\nworkspace: \"{REPO_ROOT}\"\nlock: \"{PLANS_DIR}/lock.md\"\n",
            "BODY-SENTINEL {PLANS_DIR}\n",
        );
        let v = status_json(&roots, &parse_file(&p).unwrap());
        assert_eq!(v["sealed"], true, "{v}");
        let q = v["query"].as_str().expect("sealed status carries the query");
        let repo = repo_root(&roots).display().to_string();
        assert!(q.contains("BODY-SENTINEL"), "{q}");
        assert!(q.contains(&format!("LOCK-SENTINEL {repo}")), "lock appended + expanded: {q}");
        assert!(q.contains(&plans.display().to_string()), "{q}");
        assert!(!q.contains("{REPO_ROOT}") && !q.contains("{PLANS_DIR}"), "{q}");
        assert!(q.contains(push_rule(false)), "{q}");
        assert_eq!(v["workspace"], repo);
        // Unsealed: no query at all.
        let raw = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, raw.replace("BODY-SENTINEL", "EDITED")).unwrap();
        let v = status_json(&roots, &parse_file(&p).unwrap());
        assert_eq!(v["sealed"], false);
        assert!(v.get("query").is_none(), "{v}");
    }

    #[test]
    fn no_launcher_passes_a_git_push_flag() {
        let flag = ["--git", "-push"].concat();
        for (name, src) in [
            ("coordinator.rs", concat!(
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
)),
            ("herdr.rs", include_str!("../habitat/herdr.rs")),
            ("agents.rs", include_str!("../agents/agents.rs")),
        ] {
            assert!(!src.contains(&flag), "{name} still passes {flag}");
        }
    }

    #[test]
    fn migrate_dry_run_lists_and_does_not_delete() {
        let roots = temp_roots("mig");
        let buzz = roots.home.join(".buzz/PLANS");
        write(&buzz.join("a.md"), "hello\n");
        let list = migrate_list(&roots);
        assert_eq!(list.len(), 1);
        assert!(buzz.join("a.md").is_file());
        assert!(!plans_dir(&roots).join("a.md").exists());
    }

    /// Separate-lock fixture. The lock file includes `status: x` so stripping
    /// seal lines out of a separate lock changes the pinned hash. The
    /// 86441b3 bytes (lock text `LOCK-FIXED\n`, no status line) stay in
    /// `separate_lock_86441b3_bytes_still_verify`.
    fn s4_fixture(roots: &Roots) -> (PathBuf, String) {
        let plans = plans_dir(roots);
        let lock = plans.join("lock.md");
        write(&lock, "status: x\nLOCK-FIXED\n");
        let go = plans.join("go.md");
        let text = "---\nid: GO-S4\nstatus: ready\npush_allowed: false\nlock: \"{PLANS_DIR}/lock.md\"\n---\nBODY-FIXED\n";
        write(&go, text);
        (go, text.to_string())
    }

    #[test]
    fn separate_lock_hash_unchanged() {
        // Re-pinned after adding `status: x` to the lock fixture (X13).
        // Stripping seal lines from the separate lock yields the 86441b3
        // hash instead, so this pin fails closed.
        const SEPARATE: &str = "510e20c58fb99549548b90bcb24288db6a2d3e80c49cd19ad5cca8511092f46c";
        let roots = temp_roots("s4");
        let (go, _) = s4_fixture(&roots);
        let parsed = parse_file(&go).unwrap();
        let hash = seal_hash_of(&roots, &go, &parsed.yaml, &parsed.header, &parsed.body).unwrap();
        assert_eq!(hash, SEPARATE, "separate-lock pin (status: x in the lock) drifted");
        seal_file_in(&roots, &go, "matt").unwrap();
        let sealed = parse_file(&go).unwrap();
        assert_eq!(sealed.header.seal_hash.as_deref(), Some(SEPARATE));
        assert!(seal_note(&roots, &sealed).is_none());
        let lock = plans_dir(&roots).join("lock.md");
        let lock_text = std::fs::read_to_string(&lock).unwrap();
        assert!(lock_text.contains("status: x\n"), "{lock_text}");
        write(&lock, "LOCK-EDITED\n");
        let note = seal_note(&roots, &parse_file(&go).unwrap())
            .expect("lock edit breaks a separate-lock seal");
        assert!(note.contains("changed since seal"), "{note}");
        std::fs::remove_file(&lock).unwrap();
        let note = seal_note(&roots, &parse_file(&go).unwrap())
            .expect("a removed lock still refuses");
        assert!(note.contains("seal check failed") || note.contains("lock unreadable"), "{note}");
    }

    #[test]
    fn separate_lock_86441b3_bytes_still_verify() {
        // Hashes of these exact bytes at 86441b3 (separate lock / no lock).
        const SEPARATE: &str = "b226a8e61191780177b6a34fb94969e616211a428fe79f9d5d54437d0775c875";
        const NO_LOCK: &str = "dbbd6a61296ba85bea495aff6a04be68d0ee3d1350b1cae3b4faad12f4d5b6b2";
        let roots = temp_roots("s4b3");
        let plans = plans_dir(&roots);
        let lock = plans.join("lock.md");
        write(&lock, "LOCK-FIXED\n");
        let go = plans.join("go.md");
        let text = "---\nid: GO-S4\nstatus: ready\npush_allowed: false\nlock: \"{PLANS_DIR}/lock.md\"\n---\nBODY-FIXED\n";
        write(&go, text);
        let parsed = parse_file(&go).unwrap();
        let hash = seal_hash_of(&roots, &go, &parsed.yaml, &parsed.header, &parsed.body).unwrap();
        assert_eq!(hash, SEPARATE, "separate-lock hash drifted from 86441b3");
        seal_file_in(&roots, &go, "matt").unwrap();
        let sealed = parse_file(&go).unwrap();
        assert_eq!(sealed.header.seal_hash.as_deref(), Some(SEPARATE));
        assert!(seal_note(&roots, &sealed).is_none(), "86441b3 bytes must still verify");

        let go2 = plans.join("nolock.md");
        write(&go2, "---\nid: GO-S4B\nstatus: ready\n---\nBODY-FIXED\n");
        let p2 = parse_file(&go2).unwrap();
        let h2 = seal_hash_of(&roots, &go2, &p2.yaml, &p2.header, &p2.body).unwrap();
        assert_eq!(h2, NO_LOCK, "(no lock) hash drifted from 86441b3");
    }

    #[test]
    fn self_lock_seals_and_reads_sealed() {
        let roots = temp_roots("s1");
        let plans = plans_dir(&roots);
        let go = plans.join("go.md");
        write(&go, "---\nid: GO-S1\nstatus: ready\n---\nBODY\n");
        let mut cases = vec![
            ("plans-token", "{PLANS_DIR}/go.md".to_string()),
            ("absolute", go.display().to_string()),
            ("tilde", "~/Orbitty GOs/go.md".to_string()),
        ];
        // APFS is case-insensitive: a differently-cased path is the same file.
        let case_path = plans.join("GO.md");
        if case_path != go && std::fs::metadata(&case_path).is_ok() {
            cases.push(("case-fold", case_path.display().to_string()));
        }
        for (label, lock) in cases {
            let text = format!("---\nid: GO-S1\nstatus: ready\nlock: \"{lock}\"\n---\nBODY\n");
            write(&go, &text);
            seal_file_in(&roots, &go, "matt").unwrap();
            let note = seal_note(&roots, &parse_file(&go).unwrap());
            assert!(
                note.is_none(),
                "{label}: right after seal, expected sealed, got {note:?}"
            );
            let v = status_json(&roots, &parse_file(&go).unwrap());
            assert_eq!(v["sealed"], true, "{label}: {v}");
            match gate_in(&roots, "", "GO-S1", &[]) {
                Gate::Launch(g) => assert!(g.body.contains("BODY"), "{label}"),
                other => panic!("{label}: gate should agree sealed: {other:?}"),
            }
        }
        let link = plans.join("alias.md");
        std::os::unix::fs::symlink(&go, &link).unwrap();
        let text = "---\nid: GO-S1\nstatus: ready\nlock: \"{PLANS_DIR}/alias.md\"\n---\nBODY\n";
        write(&go, text);
        seal_file_in(&roots, &link, "matt").unwrap();
        let note = seal_note(&roots, &parse_file(&go).unwrap());
        assert!(note.is_none(), "symlink: right after seal, got {note:?}");
        let v = status_json(&roots, &parse_file(&go).unwrap());
        assert_eq!(v["sealed"], true, "symlink: {v}");
        match gate_in(&roots, "", "GO-S1", &[]) {
            Gate::Launch(_) => {}
            other => panic!("symlink gate: {other:?}"),
        }
    }

    #[test]
    fn self_lock_content_edit_breaks_seal() {
        let roots = temp_roots("s2");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\n# note\nid: GO-S2\nstatus: ready\npush_allowed: false\ntools: [read, edit]\nrunner: orbitty-cursor\nmodel: grok-4.7-high\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY-LINE\n",
        );
        write(&plans_dir(&roots).join("other-lock.md"), "OTHER-LOCK\n");
        seal_file_in(&roots, &p, "matt").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        let edits = [
            ("BODY-LINE", "BODY-EDITED"),
            ("push_allowed: false", "push_allowed: true"),
            ("tools: [read, edit]", "tools: [read, edit, git_push]"),
            ("runner: orbitty-cursor", "runner: other"),
            ("model: grok-4.7-high", "model: other"),
            ("# note", "# note edited"),
            ("lock: \"{PLANS_DIR}/go.md\"", "lock: \"{PLANS_DIR}/other-lock.md\""),
            ("id: GO-S2\n", "id: GO-S2\nextra: 1\n"),
        ];
        for (from, to) in edits {
            assert!(raw.contains(from), "fixture missing {from:?}");
            std::fs::write(&p, raw.replace(from, to)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("{from:?} must break the seal"));
            assert!(note.contains("changed since seal"), "{from:?}: {note}");
            std::fs::write(&p, &raw).unwrap();
            assert!(
                seal_note(&roots, &parse_file(&p).unwrap()).is_none(),
                "restore after {from:?}"
            );
        }
    }

    #[test]
    fn self_lock_seal_line_edits_keep_seal() {
        let roots = temp_roots("s3");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-S3\nstatus: ready\npush_allowed: false\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY\n",
        );
        seal_file_in(&roots, &p, "matt").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());

        let by_line = raw.lines().find(|l| l.starts_with("sealed_by:")).unwrap();
        std::fs::write(&p, raw.replace(by_line, "sealed_by: \"someone else\"")).unwrap();
        let g = parse_file(&p).unwrap();
        assert_eq!(g.header.sealed_by.as_deref(), Some("someone else"));
        assert!(seal_note(&roots, &g).is_none(), "sealed_by value");

        let at_line = raw.lines().find(|l| l.starts_with("sealed_at:")).unwrap();
        std::fs::write(&p, raw.replace(at_line, "sealed_at: \"2000-01-01T00:00:00-05:00\"")).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none(), "sealed_at value");

        std::fs::write(&p, relocate_seal_lines(&raw)).unwrap();
        let moved = std::fs::read_to_string(&p).unwrap();
        assert_ne!(moved, raw, "seal lines should have moved");
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none(), "reordered seal lines");

        std::fs::write(&p, &raw).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        seal_file_in(&roots, &p, "chuck").unwrap();
        let again = parse_file(&p).unwrap();
        assert_eq!(again.header.sealed_by.as_deref(), Some("chuck"));
        assert!(seal_note(&roots, &again).is_none(), "re-seal");
        seal_file_in(&roots, &p, "matt").unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none(), "second re-seal");

        let sealed = std::fs::read_to_string(&p).unwrap();
        let status_line = sealed.lines().find(|l| l.starts_with("status:")).unwrap();
        std::fs::write(&p, sealed.replace(status_line, "status: ready")).unwrap();
        let note = seal_note(&roots, &parse_file(&p).unwrap()).expect("status ready refuses");
        assert!(note.contains("not sealed"), "{note}");

        std::fs::write(&p, sealed.replace("sealed_by: \"matt\"", "sealed_by: \"\"")).unwrap();
        let note = seal_note(&roots, &parse_file(&p).unwrap()).expect("blank sealed_by refuses");
        assert!(note.contains("sealed_by"), "{note}");
    }

    fn relocate_seal_lines(raw: &str) -> String {
        let mut seal_lines = Vec::new();
        let mut other = Vec::new();
        let mut fences = 0usize;
        for line in raw.lines() {
            if line == "---" {
                fences += 1;
                other.push(line.to_string());
                continue;
            }
            if fences == 1 && top_key(line).is_some_and(|k| {
                k == "status" || k == "sealed_at" || k == "sealed_by" || k == "seal_hash"
            }) {
                seal_lines.push(line.to_string());
                continue;
            }
            other.push(line.to_string());
        }
        let mut out = Vec::new();
        let mut inserted = false;
        for line in other {
            out.push(line.clone());
            if !inserted && line == "---" {
                out.extend(seal_lines.iter().cloned());
                inserted = true;
            }
        }
        let mut joined = out.join("\n");
        if raw.ends_with('\n') {
            joined.push('\n');
        }
        joined
    }

    fn assert_payload_stays_off_the_launch(roots: &Roots, path: &Path, id: &str, payload: &str) {
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains(payload), "fixture missing {payload}");
        let g = parse_file(path).unwrap_or_else(|e| panic!("{payload} should parse: {e}"));
        let note = seal_note(roots, &g).unwrap_or_else(|| panic!("{payload} stayed sealed"));
        assert!(note.contains("not launching"), "{payload}: {note}");
        let v = status_json(roots, &g);
        assert_eq!(v["sealed"], false, "{payload}: {v}");
        assert!(v.get("query").is_none(), "{payload} reached the query: {v}");
        assert!(
            !v.to_string().contains(payload),
            "{payload} reached status json: {v}"
        );
        match gate_in(roots, "", id, &[]) {
            Gate::Refuse { note } => assert!(
                !note.contains(payload),
                "{payload} reached the gate note: {note}"
            ),
            other => panic!("{payload} reached the gate: {other:?}"),
        }
    }

    #[test]
    fn self_lock_continuation_payload_breaks_seal() {
        let roots = temp_roots("n1");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-N1\nstatus: ready\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY\n",
        );
        seal_file_in(&roots, &p, "matt").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        let cases = [
            (
                "sealed_by: \"matt\"\n",
                "sealed_by: |\n  BLOCK-PAYLOAD-LINE\n  BLOCK-PAYLOAD-NEXT\n",
                "BLOCK-PAYLOAD-LINE",
            ),
            (
                "sealed_by: \"matt\"\n",
                "sealed_by: >-\n  FOLDED-PAYLOAD-LINE\n",
                "FOLDED-PAYLOAD-LINE",
            ),
            (
                "sealed_by: \"matt\"\n",
                "sealed_by: \"matt\"\n  # INDENTED-UNDER-SEALED-BY\n",
                "INDENTED-UNDER-SEALED-BY",
            ),
        ];
        for (from, to, payload) in cases {
            assert!(raw.contains(from), "fixture missing {from:?}");
            std::fs::write(&p, raw.replace(from, to)).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("{payload} stayed sealed"));
            assert!(note.contains("changed since seal"), "{payload}: {note}");
            assert_payload_stays_off_the_launch(&roots, &p, "GO-N1", payload);
            std::fs::write(&p, &raw).unwrap();
            assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        }
        for (prefix, payload) in [
            ("sealed_at:", "INDENTED-UNDER-SEALED-AT"),
            ("seal_hash:", "INDENTED-UNDER-SEAL-HASH"),
        ] {
            let line = raw.lines().find(|l| l.starts_with(prefix)).unwrap();
            let edited = raw.replace(line, &format!("{line}\n  # {payload}"));
            std::fs::write(&p, edited).unwrap();
            let note = seal_note(&roots, &parse_file(&p).unwrap())
                .unwrap_or_else(|| panic!("{payload} stayed sealed"));
            assert!(note.contains("changed since seal"), "{payload}: {note}");
            assert_payload_stays_off_the_launch(&roots, &p, "GO-N1", payload);
            std::fs::write(&p, &raw).unwrap();
        }
    }

    #[test]
    fn seal_line_trailing_comment_refuses_payload() {
        let roots = temp_roots("cmt");
        let p = plans_dir(&roots).join("go.md");
        write(
            &p,
            "---\nid: GO-CMT-SEAL\nstatus: ready\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY\n",
        );
        seal_file_in(&roots, &p, "matt").unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        let cases = [
            ("status:", "status: sealed # STATUS-COMMENT-PAYLOAD", "status", "STATUS-COMMENT-PAYLOAD"),
            ("sealed_at:", "", "sealed_at", "SEALED-AT-COMMENT-PAYLOAD"),
            ("sealed_by:", "", "sealed_by", "SEALED-BY-COMMENT-PAYLOAD"),
            ("seal_hash:", "", "seal_hash", "SEAL-HASH-COMMENT-PAYLOAD"),
        ];
        for (prefix, exact, key, payload) in cases {
            let line = raw.lines().find(|l| l.starts_with(prefix)).unwrap().to_string();
            let replacement = if exact.is_empty() {
                format!("{line} # {payload}")
            } else {
                exact.to_string()
            };
            std::fs::write(&p, raw.replace(&line, &replacement)).unwrap();
            let g = parse_file(&p).unwrap();
            assert_eq!(
                g.header.status.as_deref().map(|s| s.to_ascii_lowercase()),
                Some("sealed".into()),
                "{payload} must still parse as sealed"
            );
            let note = seal_note(&roots, &g).unwrap_or_else(|| panic!("{payload} stayed sealed"));
            assert!(
                note.contains("trailing comment") && note.contains(key),
                "{payload}: {note}"
            );
            assert_payload_stays_off_the_launch(&roots, &p, "GO-CMT-SEAL", payload);
            std::fs::write(&p, &raw).unwrap();
            assert!(seal_note(&roots, &parse_file(&p).unwrap()).is_none());
        }
        // Quoted status plus a comment, the form that used to stay sealed.
        let line = raw.lines().find(|l| l.starts_with("status:")).unwrap();
        std::fs::write(
            &p,
            raw.replace(line, "status: \"sealed\"  # QUOTED-STATUS-PAYLOAD"),
        )
        .unwrap();
        let g = parse_file(&p).unwrap();
        assert_eq!(g.header.status.as_deref(), Some("sealed"));
        let note = seal_note(&roots, &g).unwrap();
        assert!(note.contains("trailing comment") && note.contains("status"), "{note}");
        assert_payload_stays_off_the_launch(&roots, &p, "GO-CMT-SEAL", "QUOTED-STATUS-PAYLOAD");
    }

    #[test]
    fn self_lock_hard_link_still_refused() {
        let roots = temp_roots("s5");
        let plans = plans_dir(&roots);
        let go = plans.join("go.md");
        let text = "---\nid: GO-S5\nstatus: ready\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY\n";
        write(&go, text);
        let other = plans.join("other.md");
        std::fs::hard_link(&go, &other).unwrap();
        let err = seal_file_in(&roots, &go, "matt").unwrap_err();
        assert!(err.contains("hard links"), "{err}");
        assert_eq!(std::fs::read_to_string(&go).unwrap(), text);
        assert_eq!(std::fs::read_to_string(&other).unwrap(), text);
    }

    fn strip_rust_comments(src: &str) -> String {
        let b = src.as_bytes();
        let mut out = String::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'"' {
                out.push('"');
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    out.push(b[i] as char);
                    if b[i] == b'\\' && i + 1 < b.len() {
                        i += 1;
                        out.push(b[i] as char);
                    }
                    i += 1;
                }
                if i < b.len() {
                    out.push('"');
                    i += 1;
                }
                continue;
            }
            if b[i] == b'\'' {
                // A lifetime (`'static`) is a quote plus an identifier.
                // A char literal is short (`'x'`, `'\n'`, `'\''`).
                if i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
                    out.push('\'');
                    i += 1;
                    continue;
                }
                out.push('\'');
                i += 1;
                if i < b.len() && b[i] == b'\\' {
                    out.push('\\');
                    i += 1;
                    if i < b.len() {
                        out.push(b[i] as char);
                        i += 1;
                    }
                } else if i < b.len() {
                    out.push(b[i] as char);
                    i += 1;
                }
                if i < b.len() && b[i] == b'\'' {
                    out.push('\'');
                    i += 1;
                }
                continue;
            }
            if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                continue;
            }
            out.push(b[i] as char);
            i += 1;
        }
        out
    }

    fn rust_fn_span(src: &str, name: &str) -> std::ops::Range<usize> {
        let needle = format!("fn {name}");
        let start = src.find(&needle).unwrap_or_else(|| panic!("missing fn {name}"));
        let rest = &src[start..];
        let open = rest.find('{').unwrap_or_else(|| panic!("fn {name} has no body"));
        let abs_open = start + open;
        let end = scan_brace_end(src, abs_open);
        start..end + 1
    }

    fn scan_brace_end(src: &str, open: usize) -> usize {
        let b = src.as_bytes();
        let mut i = open;
        let mut depth = 0i32;
        while i < b.len() {
            if b[i] == b'"' {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            if b[i] == b'\'' {
                if i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
                    i += 1;
                    continue;
                }
                i += 1;
                if i < b.len() && b[i] == b'\\' {
                    i += 2;
                } else {
                    i += 1;
                }
                if i < b.len() && b[i] == b'\'' {
                    i += 1;
                }
                continue;
            }
            match b[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("unclosed brace at {open}");
    }

    fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
        let span = rust_fn_span(src, name);
        &src[span]
    }

    fn remove_fn(src: &str, name: &str) -> String {
        let span = rust_fn_span(src, name);
        let mut out = String::new();
        out.push_str(&src[..span.start]);
        out.push_str(&src[span.end..]);
        out
    }

    fn hash_with_part(yaml: &str, body: &str, lock_name: &str, lock: &str) -> String {
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut j = serde_json::to_value(doc).unwrap();
        for k in SEAL_KEYS {
            j.as_object_mut().unwrap().remove(*k);
        }
        let head = canonical_json(&j);
        let mut h = Sha256::new();
        h.update(b"orbitty-seal-v2\n");
        for (name, part) in [("header", head.as_str()), ("body", body), (lock_name, lock)] {
            h.update(format!("{name} {}\n", part.len()).as_bytes());
            h.update(part.as_bytes());
            h.update(b"\n");
        }
        hex_encode(&h.finalize())
    }

    #[test]
    fn self_lock_hash_domain_is_lock_self() {
        let roots = temp_roots("x8");
        let go = plans_dir(&roots).join("go.md");
        let body = "BODY-X8\n";
        write(
            &go,
            "---\nid: GO-X8\nstatus: ready\nlock: \"{PLANS_DIR}/go.md\"\n---\nBODY-X8\n",
        );
        let parsed = parse_file(&go).unwrap();
        assert_eq!(parsed.body, body);
        let got = seal_hash_of(&roots, &go, &parsed.yaml, &parsed.header, &parsed.body).unwrap();
        let lock = self_lock_text(&parsed.yaml, &parsed.body);
        let expect = hash_with_part(&parsed.yaml, &parsed.body, "lock-self", &lock);
        let separate = hash_with_part(&parsed.yaml, &parsed.body, "lock", &lock);
        assert_eq!(got, expect, "self-lock hash must use the lock-self tag");
        assert_ne!(got, separate, "lock-self must not fall back to the separate-lock tag");
    }

    #[test]
    fn seal_file_in_verifies_before_write() {
        let prod = concat!(
include_str!("dirs.rs"),
include_str!("locate.rs"),
include_str!("seal.rs"),
include_str!("resolve.rs"),
include_str!("parse.rs"),
include_str!("rules.rs")
).split("mod tests").next().unwrap();
        let prod = strip_rust_comments(prod);
        let body = fn_body(&prod, "seal_file_in");
        let note_at = body.find("seal_note(").unwrap_or_else(|| {
            panic!("seal_file_in must call seal_note before writing");
        });
        let verify_at = body.find("seal did not verify").unwrap_or_else(|| {
            panic!("seal_file_in must refuse when the pre-write check fails");
        });
        let write_at = body
            .find("write_sealed(")
            .expect("seal_file_in must write through write_sealed");
        assert!(note_at < write_at, "seal_note must run before write_sealed");
        assert!(
            verify_at < write_at,
            "a failed pre-write check must return before write_sealed"
        );
    }

    #[test]
    fn seal_and_verify_share_normalization() {
        let prod = concat!(
include_str!("dirs.rs"),
include_str!("locate.rs"),
include_str!("seal.rs"),
include_str!("resolve.rs"),
include_str!("parse.rs"),
include_str!("rules.rs")
).split("mod tests").next().unwrap();
        let prod = strip_rust_comments(prod);
        assert!(
            fn_body(&prod, "seal_note").contains("seal_hash_of("),
            "seal_note must call seal_hash_of"
        );
        assert!(
            fn_body(&prod, "seal_file_in").contains("seal_hash_of("),
            "seal_file_in must call seal_hash_of"
        );
        let hash_fn = fn_body(&prod, "seal_hash_of");
        assert!(hash_fn.contains("self_lock_text("), "seal_hash_of must call self_lock_text");
        assert!(hash_fn.contains("lock_is_self("), "seal_hash_of must call lock_is_self");
        let rest = remove_fn(&prod, "seal_hash_of");
        let rest = remove_fn(&rest, "self_lock_text");
        let rest = remove_fn(&rest, "lock_is_self");
        assert!(
            !rest.contains("self_lock_text("),
            "self_lock_text( leaked outside seal_hash_of"
        );
        assert!(
            !rest.contains("lock_is_self("),
            "lock_is_self( leaked outside seal_hash_of"
        );
    }
