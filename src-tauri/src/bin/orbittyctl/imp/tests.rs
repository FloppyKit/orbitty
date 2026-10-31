    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn stars_checkin_set_parses() {
        let o = parse(&argv(&[
            "--json",
            "stars",
            "checkin",
            "set",
            "--axis",
            "sleep",
            "--value",
            "4",
            "--date",
            "2099-12-31",
        ]))
        .unwrap();
        assert!(o.json);
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "stars.checkin.set");
        assert_eq!(args["axis"], "sleep");
        assert_eq!(args["value"], 4);
        assert_eq!(args["date"], "2099-12-31");
    }

    #[test]
    fn history_verbs_parse() {
        let o = parse(&argv(&[
            "--json", "history", "list", "--card", "c-1", "--project", "p-1", "--limit", "12",
        ]))
        .unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "history.list");
        assert_eq!(args, serde_json::json!({ "card": "c-1", "project": "p-1", "limit": 12 }));
        let (tool, args) = tool_call(&parse(&argv(&["history", "list"])).unwrap()).unwrap();
        assert_eq!(tool, "history.list");
        assert_eq!(args, serde_json::json!({}));
        for bad in ["0", "501", "x"] {
            let o = parse(&argv(&["history", "list", "--limit", bad])).unwrap();
            assert!(tool_call(&o).is_err(), "{bad}");
        }
        let (tool, args) = tool_call(&parse(&argv(&["history", "checkpoint"])).unwrap()).unwrap();
        assert_eq!(tool, "history.checkpoint");
        assert_eq!(args, serde_json::json!({}));
        assert!(USAGE.contains("history list [--card ID] [--project ID] [--limit N]"));
        assert!(USAGE.contains("history checkpoint"));
    }

    #[test]
    fn history_rewind_verbs_parse() {
        let call = |a: &[&str]| tool_call(&parse(&argv(a)).unwrap());
        assert_eq!(call(&["history", "drift"]).unwrap(), ("history.drift", serde_json::json!({})));
        let (tool, args) = call(&["history", "rewind", "--to", "h-1"]).unwrap();
        assert_eq!((tool, args), ("history.rewind", serde_json::json!({ "to": "h-1" })));
        let (_, args) = call(&[
            "history", "rewind", "--to", "h-1", "--card", "c-1", "--remove-added", "--confirm",
        ])
        .unwrap();
        assert_eq!(
            args,
            serde_json::json!({ "to": "h-1", "card": "c-1", "removeAdded": true, "confirm": true })
        );
        assert!(call(&["history", "rewind"]).is_err(), "needs --to");
        for verb in ["repair", "keep"] {
            let (tool, args) = call(&["history", verb]).unwrap();
            assert_eq!(tool, format!("history.{verb}"));
            assert_eq!(args, serde_json::json!({ "confirm": false }));
            let (_, args) = call(&["history", verb, "--confirm"]).unwrap();
            assert_eq!(args, serde_json::json!({ "confirm": true }));
        }
        for line in [
            "history drift",
            "history rewind --to ID [--card C] [--remove-added] [--confirm]",
            "history repair [--confirm]",
            "history keep [--confirm]",
        ] {
            assert!(USAGE.contains(line), "{line}");
        }
    }

    #[test]
    fn stars_log_set_keeps_an_empty_clear() {
        let o = parse(&argv(&[
            "stars", "log", "set", "--win1", "", "--date", "2099-12-31",
        ]))
        .unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "stars.log.set");
        assert_eq!(args["win1"], "");
        assert!(args.get("win2").is_none());
    }

    #[test]
    fn stars_journal_append_and_reflect_flags() {
        let o = parse(&argv(&[
            "stars",
            "journal",
            "append",
            "--date",
            "2099-12-31",
            "--planet",
            "orbitty",
            "--",
            "a note",
        ]))
        .unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "stars.journal.append");
        assert_eq!(args["text"], "a note");
        assert_eq!(args["planet"], "orbitty");

        let o = parse(&argv(&[
            "stars",
            "reflect",
            "write",
            "--date",
            "2099-12-31",
            "--cloud",
            "off",
            "--answers",
            "{\"spokes\":true}",
        ]))
        .unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "stars.reflect.write");
        assert_eq!(args["template"], "ledger");
        assert_eq!(args["answers"]["cloud"], false);
        assert_eq!(args["answers"]["spokes"], true);
    }

    #[test]
    fn companion_harness_verbs_parse() {
        let o = parse(&argv(&["companion", "harnesses"])).unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "companion.harnesses");
        assert!(args.as_object().unwrap().is_empty());

        let o = parse(&argv(&["companion", "set-backend", "--", "hermes"])).unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "companion.set_backend");
        assert_eq!(args["id"], "hermes");

        let o = parse(&argv(&["companion", "set-backend", "--", ""])).unwrap();
        let (_, args) = tool_call(&o).unwrap();
        assert_eq!(args["id"], "");
    }

    #[test]
    fn companion_route_request_matches_mcp() {
        let o = parse(&argv(&["companion", "route", "--", "board"])).unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        assert_eq!(tool, "companion.route");
        let req = json!({ "v": 1, "id": "1", "tool": tool, "args": args });
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"args":{"line":"board"},"id":"1","tool":"companion.route","v":1}"#
        );
        let o = parse(&argv(&["companion", "route", "--", "say \"hi\""])).unwrap();
        let (tool, args) = tool_call(&o).unwrap();
        let req = json!({ "v": 1, "id": "1", "tool": tool, "args": args });
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            "{\"args\":{\"line\":\"say \\\"hi\\\"\"},\"id\":\"1\",\"tool\":\"companion.route\",\"v\":1}"
        );
    }

    #[test]
    fn go_runner_verbs_parse_and_refuse_permission_flags() {
        let (tool, args) = tool_call(&parse(&argv(&["go", "runner"])).unwrap()).unwrap();
        assert_eq!(tool, "go.runner_get");
        assert_eq!(args, serde_json::json!({}));
        let (tool, args) =
            tool_call(&parse(&argv(&["go", "runner", "--file", "/tmp/GO.md"])).unwrap()).unwrap();
        assert_eq!(tool, "go.runner_get");
        assert_eq!(args["file"], "/tmp/GO.md");
        assert!(tool_call(
            &parse(&argv(&["go", "runner", "--file", "/tmp/GO.md", "--reset"])).unwrap()
        )
        .is_err());
        let (tool, args) =
            tool_call(&parse(&argv(&["go", "runner", "--set", "pi-grok"])).unwrap()).unwrap();
        assert_eq!(tool, "go.runner_set");
        assert_eq!(args, serde_json::json!({"runner": "pi-grok"}));
        let (tool, args) = tool_call(&parse(&argv(&["go", "runner", "--reset"])).unwrap()).unwrap();
        assert_eq!(tool, "go.runner_set");
        assert_eq!(args, serde_json::json!({"reset": true}));
        assert!(tool_call(
            &parse(&argv(&["go", "runner", "--set", "pi-grok", "--reset"])).unwrap()
        )
        .is_err());
        let (tool, args) =
            tool_call(&parse(&argv(&["go", "start", "--card", "c-1"])).unwrap()).unwrap();
        assert_eq!(tool, "go.start");
        assert_eq!(args, serde_json::json!({"card": "c-1"}));
        let (_, args) =
            tool_call(&parse(&argv(&["go", "run-status", "--id", "r-1"])).unwrap()).unwrap();
        assert_eq!(args["run_id"], "r-1");
        let (tool, args) =
            tool_call(&parse(&argv(&["go", "run-result", "--id", "r-1"])).unwrap()).unwrap();
        assert_eq!(tool, "go.run_result");
        assert_eq!(args["run_id"], "r-1");
        let (tool, args) = tool_call(&parse(&argv(&["flags", "list"])).unwrap()).unwrap();
        assert_eq!(tool, "flags.list");
        assert_eq!(args, serde_json::json!({}));
        let (tool, args) =
            tool_call(&parse(&argv(&["flags", "set", "go_start", "off"])).unwrap()).unwrap();
        assert_eq!(tool, "flags.set");
        assert_eq!(args, serde_json::json!({"id": "go_start", "on": false}));
        let (tool, args) =
            tool_call(&parse(&argv(&["flags", "set", "tabbity_bridge", "on"])).unwrap()).unwrap();
        assert_eq!(tool, "flags.set");
        assert_eq!(args["on"], true);
        assert!(tool_call(&parse(&argv(&["flags", "set", "go_start", "maybe"])).unwrap()).is_err());
        let (tool, args) = tool_call(&parse(&argv(&["flags", "auto", "off"])).unwrap()).unwrap();
        assert_eq!(tool, "flags.auto");
        assert_eq!(args, serde_json::json!({"on": false}));
        let (tool, args) = tool_call(&parse(&argv(&["herdr", "status"])).unwrap()).unwrap();
        assert_eq!(tool, "herdr.status");
        assert_eq!(args, serde_json::json!({}));
        let (tool, args) =
            tool_call(&parse(&argv(&["herdr", "peek", "c-1", "--limit", "40"])).unwrap()).unwrap();
        assert_eq!(tool, "herdr.peek");
        assert_eq!(args["card"], "c-1");
        assert_eq!(args["lines"], 40);
        for flag in ["--cook", "--allow-push"] {
            let err = match parse(&argv(&["go", "runner", flag])) {
                Err(e) => e,
                Ok(_) => panic!("{flag} should be a usage error"),
            };
            assert!(err.contains("unknown flag"), "{flag}: {err}");
        }
        assert!(USAGE.contains("go runner [--set ID | --reset | --file PATH]"));
        assert!(!USAGE.contains("--allow-push"));
        assert!(!USAGE.contains("--cook"));
    }

    #[test]
    fn events_since_parses() {
        let (tool, args) = tool_call(&parse(&argv(&["events", "since", "0"])).unwrap()).unwrap();
        assert_eq!(tool, "events.since");
        assert_eq!(args, serde_json::json!({ "after": 0 }));
        let (_, args) =
            tool_call(&parse(&argv(&["events", "since", "4", "--limit", "2"])).unwrap()).unwrap();
        assert_eq!(args, serde_json::json!({ "after": 4, "limit": 2 }));
        assert!(tool_call(&parse(&argv(&["events", "since"])).unwrap()).is_err());
        assert!(parse(&argv(&["events", "since", "-1"])).is_err());
        assert!(
            tool_call(&parse(&argv(&["events", "since", "0", "--limit", "501"])).unwrap()).is_err()
        );
        assert!(
            tool_call(&parse(&argv(&["events", "since", "0", "--limit", "0"])).unwrap()).is_err()
        );
        assert!(USAGE.contains("events since <n> [--limit N]"));
    }

    #[test]
    fn archive_verbs_parse() {
        let (tool, args) =
            tool_call(&parse(&argv(&["companion", "archive-tail", "-n", "5"])).unwrap()).unwrap();
        assert_eq!(tool, "companion.archive_tail");
        assert_eq!(args["n"], 5);
        let (tool, args) = tool_call(
            &parse(&argv(&["companion", "resets", "--n", "3", "--date", "2026-10-30"])).unwrap(),
        )
        .unwrap();
        assert_eq!(tool, "companion.resets");
        assert_eq!(args["n"], 3);
        let (tool, args) = tool_call(
            &parse(&argv(&["companion", "roll", "--reason", "manual_test"])).unwrap(),
        )
        .unwrap();
        assert_eq!(tool, "companion.roll");
        assert_eq!(args["reason"], "manual_test");
        assert!(
            tool_call(&parse(&argv(&["companion", "roll", "--reason", "budget"])).unwrap())
                .is_err()
        );
        assert!(USAGE.contains("companion archive-tail"));
        assert!(USAGE.contains("companion roll --reason manual_test"));
    }

    #[test]
    fn memory_verbs_parse() {
        let (tool, args) = tool_call(
            &parse(&argv(&["companion", "recall", "--planet", "p1", "--", "decided"])).unwrap(),
        )
        .unwrap();
        assert_eq!(tool, "companion.recall");
        assert_eq!(args["query"], "decided");
        assert_eq!(args["planet"], "p1");
        let (tool, args) = tool_call(
            &parse(&argv(&["companion", "tags-propose", "--", "Hello, World"])).unwrap(),
        )
        .unwrap();
        assert_eq!(tool, "companion.tags_propose");
        assert_eq!(args["names"], "Hello, World");
        let (tool, args) =
            tool_call(&parse(&argv(&["companion", "tags-accept", "--planet", "p1"])).unwrap())
                .unwrap();
        assert_eq!(tool, "companion.tags_accept");
        assert_eq!(args["planet"], "p1");
        assert!(USAGE.contains("companion recall"));
        assert!(USAGE.contains("companion tags-accept"));
    }

    #[test]
    fn export_out_outside_home_refused() {
        let home = std::env::temp_dir().join(format!(
            "orbitty-ctl-home-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let err = super::ledger::export_with(
            &home,
            &home,
            Some("/tmp/orbitty-shim-ledger-out.tsv"),
            "2026-10-30",
        )
        .unwrap_err();
        assert!(err.contains("HOME"), "{err}");
        let body = super::ledger::export_with(&home, &home, Some("~/ok.tsv"), "2026-10-30").unwrap();
        assert!(body.starts_with("shim_id\t"));
        assert!(std::fs::read_to_string(home.join("ok.tsv")).unwrap().starts_with("shim_id\t"));
        let o = parse(&argv(&["ledger", "export", "--out", "~/ok.tsv"])).unwrap();
        assert_eq!(o.out.as_deref(), Some("~/ok.tsv"));
        assert!(super::ledger::run(&parse(&argv(&["ledger", "nope"])).unwrap()).is_err());
        assert!(USAGE.contains("ledger export [--out PATH]"));
        let _ = std::fs::remove_dir_all(&home);
    }
