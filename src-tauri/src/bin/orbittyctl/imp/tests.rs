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
