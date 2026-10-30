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
        for flag in ["--cook", "--allow-push"] {
            let err = match parse(&argv(&["go", "runner", flag])) {
                Err(e) => e,
                Ok(_) => panic!("{flag} should be a usage error"),
            };
            assert!(err.contains("unknown flag"), "{flag}: {err}");
        }
        assert!(USAGE.contains("go runner [--set ID | --reset]"));
        assert!(!USAGE.contains("--allow-push"));
        assert!(!USAGE.contains("--cook"));
    }
