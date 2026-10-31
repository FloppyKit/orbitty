use super::parse::{
    Opts, need_flag, need_id, need_int, on_off, opt, opt_pos_int, payload, put_opt, put_raw,
};
use super::usage::USAGE;
use serde_json::{json, Value};

/// Words → (tool, args). Only the v0 tool list exists.
pub(crate) fn tool_call(o: &Opts) -> Result<(&'static str, Value), String> {
    let w: Vec<&str> = o.words.iter().map(String::as_str).collect();
    match w.as_slice() {
        ["status"] => Ok(("app.status", json!({}))),
        ["rail", "state"] => Ok(("rail.state", json!({}))),
        ["windows", "list"] => Ok(("windows.list", json!({}))),
        ["windows", "focus"] => Ok(("windows.focus", json!({ "id": need_id(o)? }))),
        ["windows", "minimize"] => Ok(("windows.minimize", json!({ "id": need_id(o)? }))),
        ["windows", "restore"] => Ok(("windows.restore", json!({ "id": need_id(o)? }))),
        ["windows", "close"] => Ok(("windows.close", json!({ "id": need_id(o)? }))),
        ["companion", "open"] => Ok(("companion.open", json!({}))),
        ["companion", "send"] => {
            let mut a = json!({ "line": payload(o)? });
            if let Some(p) = opt(&o.file) {
                a["path"] = json!(p);
            }
            Ok(("companion.send", a))
        }
        ["companion", "agents"] => Ok(("companion.agents", json!({}))),
        ["companion", "set-agent"] => {
            Ok(("companion.set_agent", json!({ "profile": payload(o)? })))
        }
        ["companion", "harnesses"] => Ok(("companion.harnesses", json!({}))),
        ["companion", "set-backend"] => {
            Ok(("companion.set_backend", json!({ "id": payload(o)? })))
        }
        ["companion", "route"] => {
            Ok(("companion.route", json!({ "line": payload(o)? })))
        }
        // `companion play nod` and `companion play -- nod` both land here.
        ["companion", "play", ..] => Ok(("companion.play", json!({ "clip": payload(o)? }))),
        // The bus verbs. Fail CLOSED on an unknown
        // mode client-side — a typo never reaches the app as a silent
        // spark. The server re-validates anyway.
        ["companion", "mode"] => {
            let m = payload(o)?;
            match m.as_str() {
                "spark" | "chat" | "work" | "hold" => {
                    Ok(("companion.mode", json!({ "mode": m })))
                }
                other => Err(format!(
                    "unknown mode {other:?} — spark | chat | work | hold"
                )),
            }
        }
        // the Free Mode switch — same
        // companion.* ACL as companion.mode. Fail CLOSED on an unknown
        // verb — a typo never flips the lane router.
        ["companion", "free-mode"] => {
            let v = payload(o)?;
            match v.as_str() {
                "on" => Ok(("companion.free_mode", json!({ "enabled": true }))),
                "off" => Ok(("companion.free_mode", json!({ "enabled": false }))),
                other => Err(format!("unknown free-mode state {other:?} — on | off")),
            }
        }
        ["companion", "note"] => {
            let mut a = json!({ "line": payload(o)? });
            if let Some(m) = opt(&o.mode) {
                match m.as_str() {
                    "spark" | "chat" | "work" | "hold" => a["mode"] = json!(m),
                    other => {
                        return Err(format!(
                            "unknown mode {other:?} — spark | chat | work | hold"
                        ))
                    }
                }
            }
            Ok(("companion.note", a))
        }
        ["companion", "thin"] => Ok(("companion.thin", json!({}))),
        ["companion", "archive-tail"] => {
            let n = o
                .n
                .as_deref()
                .unwrap_or("20")
                .parse::<u64>()
                .map_err(|_| "companion archive-tail: -n must be an integer".to_string())?;
            let mut a = json!({ "n": n });
            if let Some(d) = opt(&o.date) {
                a["date"] = json!(d);
            }
            Ok(("companion.archive_tail", a))
        }
        ["companion", "resets"] => {
            let n = o
                .n
                .as_deref()
                .unwrap_or("20")
                .parse::<u64>()
                .map_err(|_| "companion resets: -n must be an integer".to_string())?;
            Ok(("companion.resets", json!({ "n": n })))
        }
        ["companion", "roll"] => {
            let reason = opt(&o.reason).unwrap_or_default();
            if reason != "manual_test" {
                return Err(
                    "companion roll: --reason must be manual_test".into(),
                );
            }
            Ok(("companion.roll", json!({ "reason": "manual_test" })))
        }
        ["companion", "recall"] => {
            let mut a = json!({ "query": payload(o)? });
            if let Some(p) = opt(&o.planet) {
                a["planet"] = json!(p);
            }
            Ok(("companion.recall", a))
        }
        ["companion", "tags-propose"] => {
            let mut a = json!({ "names": payload(o)? });
            if let Some(p) = opt(&o.planet) {
                a["planet"] = json!(p);
            }
            Ok(("companion.tags_propose", a))
        }
        ["companion", "tags-accept"] => {
            let mut a = json!({});
            if let Some(p) = opt(&o.planet) {
                a["planet"] = json!(p);
            }
            Ok(("companion.tags_accept", a))
        }
        ["companion", "now"] => {
            let p = payload(o).unwrap_or_default();
            if p.trim().is_empty() {
                Ok(("companion.now", json!({})))
            } else if p.trim().starts_with('{') {
                let v: Value = serde_json::from_str(&p)
                    .map_err(|e| format!("companion now JSON: {e}"))?;
                Ok(("companion.now", v))
            } else {
                Ok(("companion.now", json!({ "job": p })))
            }
        }
        // The dock LED flag. Fail CLOSED on an
        // unknown verb — a typo never leaves a stuck light. `--who` may
        // ride either side of `--` (after it, it's tail text we parse).
        // `--project` rides the same way — the
        // live agent's pinned planet, for the dest/planet menu glow.
        ["companion", "hack", ..] => {
            let p = payload(o)?;
            let mut toks = p.split_whitespace();
            let live = match toks.next().unwrap_or("") {
                "on" => true,
                "off" => false,
                other => return Err(format!("unknown hack state {other:?} — on | off")),
            };
            let mut who = opt(&o.who);
            let mut project = opt(&o.project);
            let mut rest = toks;
            while let Some(t) = rest.next() {
                if t == "--who" {
                    who = rest
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                } else if let Some(v) = t.strip_prefix("--who=") {
                    who = Some(v.trim().to_string()).filter(|s| !s.is_empty());
                } else if t == "--project" {
                    project = rest
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                } else if let Some(v) = t.strip_prefix("--project=") {
                    project = Some(v.trim().to_string()).filter(|s| !s.is_empty());
                }
            }
            let mut a = json!({ "live": live });
            if let Some(w) = who {
                a["who"] = json!(w);
            }
            if let Some(p) = project {
                a["projectId"] = json!(p);
            }
            Ok(("companion.hack", a))
        }
        ["kanban", "open"] => Ok(("kanban.open", json!({}))),
        // Write verbs. The server validates values
        // (status enums, unknown ids) and owns the honest errors.
        ["kanban", "add"] => {
            let mut a = json!({ "title": need_flag(&o.title, "--title")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(s) = opt(&o.status) {
                a["status"] = json!(s);
            }
            // Blank --notes = absent (none minted).
            if let Some(n) = opt(&o.notes) {
                a["notes"] = json!(n);
            }
            if !o.links.is_empty() {
                let arr: Vec<Value> = o
                    .links
                    .iter()
                    .map(|(url, label)| {
                        let mut v = json!({ "url": url });
                        if let Some(l) = label {
                            v["label"] = json!(l);
                        }
                        v
                    })
                    .collect();
                a["links"] = json!(arr);
            }
            Ok(("kanban.add", a))
        }
        ["kanban", "link"] => {
            let id = need_id(o)?;
            let modes = [o.clear, o.rm.is_some(), o.add.is_some()]
                .into_iter()
                .filter(|on| *on)
                .count();
            if modes != 1 {
                return Err(
                    "kanban link: pass exactly one of --add URL, --rm URL, or --clear".into(),
                );
            }
            if o.clear {
                Ok(("kanban.link", json!({ "id": id, "clear": true })))
            } else if let Some(url) = o.rm.clone() {
                Ok(("kanban.link", json!({ "id": id, "rm": url })))
            } else {
                let mut a = json!({ "id": id, "add": o.add.clone().unwrap_or_default() });
                if let Some(l) = opt(&o.label) {
                    a["label"] = json!(l);
                }
                Ok(("kanban.link", a))
            }
        }
        ["kanban", "open-link"] => {
            let id = need_id(o)?;
            let mut a = json!({ "id": id });
            if let Some(n) = opt(&o.n) {
                let n: u64 = n
                    .parse()
                    .map_err(|_| format!("kanban open-link: --n must be an integer, got {n:?}"))?;
                a["n"] = json!(n);
            }
            if o.run {
                a["run"] = json!(true);
            }
            Ok(("kanban.open_link", a))
        }
        // Set/clear a card's one-line summary. The
        // text is the `--` payload; `-- ""` clears (payload keeps Some("")),
        // so the clear always reaches the server as an empty string.
        ["kanban", "notes"] if o.append => Ok((
            "kanban.note_append",
            json!({ "id": need_id(o)?, "line": payload(o)? }),
        )),
        ["kanban", "notes"] => Ok((
            "kanban.notes",
            json!({ "id": need_id(o)?, "notes": payload(o)? }),
        )),
        ["kanban", "move"] => Ok((
            "kanban.move",
            json!({ "id": need_id(o)?, "status": need_flag(&o.status, "--status")? }),
        )),
        ["verify", "close"] => Ok(("verify.close", json!({ "id": need_id(o)? }))),
        // Assign a card to an agent row (id or
        // name server-side). `--agent ""` unassigns — the flag itself is
        // required so a bare `kanban assign --id` never silently clears.
        ["kanban", "assign"] => {
            let id = need_id(o)?;
            match (o.agent.clone(), o.window.clone()) {
                (Some(agent), None) => Ok(("kanban.assign", json!({ "id": id, "agent": agent }))),
                (None, Some(window)) => Ok((
                    "kanban.assign_window",
                    json!({ "id": id, "windowId": window }),
                )),
                (Some(_), Some(_)) => Err(
                    "kanban assign: pass --agent or --window, not both".into(),
                ),
                (None, None) => Err(
                    "kanban assign: need --agent <agent> or --window <windowId> (pass empty \"\" to clear)".into(),
                ),
            }
        }
        // The agent roster. The harness is
        // required on add and fails CLOSED here — a typo never reaches
        // the app (the server re-validates anyway).
        ["agent", "list"] => Ok(("agent.list", json!({}))),
        ["agent", "add"] => {
            let h = need_flag(&o.harness, "--harness")?.to_ascii_lowercase();
            match h.as_str() {
                "cursor" | "goose" | "grok" | "hermes" | "pi" => {}
                other => {
                    return Err(format!(
                        "unknown harness {other:?} — cursor | goose | grok | hermes | pi"
                    ))
                }
            }
            let mut a = json!({
                "name": need_flag(&o.name, "--name")?,
                "harness": h,
            });
            if let Some(p) = opt(&o.profile) {
                a["profile"] = json!(p);
            }
            if let Some(m) = opt(&o.model) {
                a["model"] = json!(m);
            }
            if let Some(p) = opt(&o.permissions) {
                a["permissions"] = json!(p);
            }
            let mds: Vec<String> = o
                .md
                .iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if !mds.is_empty() {
                a["mdPaths"] = json!(mds);
            }
            if let Some(x) = opt(&o.extra) {
                a["extra"] = json!(x);
            }
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            Ok(("agent.add", a))
        }
        ["agent", "rm"] => Ok(("agent.rm", json!({ "id": need_id(o)? }))),
        ["agent", "pin"] => Ok((
            "agent.pin",
            json!({ "id": need_id(o)?, "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["agent", "pickup"] => {
            let mut a = json!({});
            if let Some(id) = opt(&o.id) {
                a["id"] = json!(id);
            } else if let Some(n) = opt(&o.name) {
                a["name"] = json!(n);
            } else {
                return Err("missing --id <id> or --name <name>".to_string());
            }
            if let Some(c) = opt(&o.card) {
                a["cardId"] = json!(c);
            }
            Ok(("agent.pickup", a))
        }
        ["shift", "status"] => Ok(("shift.status", json!({}))),
        ["gate", "status"] => Ok(("gate.status", json!({}))),
        ["route", "table"] => Ok(("route.table", json!({}))),
        ["route", "classify"] => Ok(("route.classify", json!({ "line": payload(o)? }))),
        ["route", "run"] => {
            let mut a = json!({ "line": payload(o)? });
            if let Some(p) = opt(&o.pending) {
                a["pendingId"] = json!(p);
            }
            Ok(("route.run", a))
        }
        ["gate", "inject", kind] => Ok(("gate.inject", json!({ "kind": *kind }))),
        ["shift", "tick"] => Ok(("shift.tick", json!({}))),
        ["shift", "on"] => Ok(("shift.on", json!({}))),
        ["shift", "off"] => Ok(("shift.off", json!({}))),
        ["relay", "status"] => Ok(("relay.status", json!({}))),
        ["history", "list"] => {
            let mut a = json!({});
            put_opt(&mut a, "card", &o.card);
            put_opt(&mut a, "project", &o.project);
            if let Some(n) = opt_pos_int(&o.limit, "--limit")? {
                if n > 500 {
                    return Err("--limit must be 1-500".into());
                }
                a["limit"] = json!(n);
            }
            Ok(("history.list", a))
        }
        ["history", "checkpoint"] => Ok(("history.checkpoint", json!({}))),
        ["history", "drift"] => Ok(("history.drift", json!({}))),
        ["history", "rewind"] => {
            let to = opt(&o.to).ok_or("history rewind needs --to ID")?;
            let mut a = json!({ "to": to });
            put_opt(&mut a, "card", &o.card);
            if o.remove_added {
                a["removeAdded"] = json!(true);
            }
            if o.confirm {
                a["confirm"] = json!(true);
            }
            Ok(("history.rewind", a))
        }
        ["history", "repair"] => Ok(("history.repair", json!({ "confirm": o.confirm }))),
        ["history", "keep"] => Ok(("history.keep", json!({ "confirm": o.confirm }))),
        ["events", "since", n] => {
            let after: u64 = n
                .parse()
                .map_err(|_| format!("events since needs an integer, got {n:?}"))?;
            let mut a = json!({ "after": after });
            if let Some(k) = opt_pos_int(&o.limit, "--limit")? {
                if k > 500 {
                    return Err("--limit must be 1-500".into());
                }
                a["limit"] = json!(k);
            }
            Ok(("events.since", a))
        }
        ["events", "since"] => Err(format!("events since needs <n>\n\n{USAGE}")),
        ["relay", "on"] => Ok(("relay.on", json!({}))),
        ["relay", "off"] => Ok(("relay.off", json!({}))),
        ["relay", "replica"] => {
            let url = o
                .url
                .clone()
                .ok_or("missing --url (pass --url \"\" to clear)".to_string())?;
            Ok(("relay.replica", json!({ "url": url })))
        }
        // Planet control plane. need_flag trims and
        // refuses an empty --name; the server owns unknown-id errors.
        ["claim", "front"] => Ok(("claim.front", json!({}))),
        ["claim", "unclaim"] => Ok(("claim.unclaim", json!({ "windowId": need_id(o)? }))),
        ["claim", "undo"] => Ok(("claim.undo", json!({}))),
        ["claim", "to"] => Ok((
            "claim.to",
            json!({ "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["claim", "change"] => Ok((
            "claim.change",
            json!({ "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["project", "add"] => Ok((
            "project.add",
            json!({ "name": need_flag(&o.name, "--name")? }),
        )),
        ["project", "rm"] => Ok(("project.rm", json!({ "id": need_id(o)? }))),
        ["project", "rename"] => Ok((
            "project.rename",
            json!({ "id": need_id(o)?, "name": need_flag(&o.name, "--name")? }),
        )),
        ["project", "set-root"] => Ok((
            "project.set_root",
            json!({
                "id": need_id(o)?,
                "path": need_flag(&o.cwd, "--cwd")?,
            }),
        )),
        ["task", "undo"] => Ok(("task.undo", json!({ "id": need_id(o)? }))),
        // The action recipe file. The server owns
        // validation (parse, reserved chords, unknown ids) and the ACL.
        ["action", "list"] => Ok(("action.list", json!({}))),
        ["action", "add"] => {
            let mut a = json!({
                "name": need_flag(&o.name, "--name")?,
                "cmd": need_flag(&o.cmd, "--cmd")?,
            });
            if let Some(d) = opt(&o.cwd) {
                a["cwd"] = json!(d);
            }
            if let Some(k) = opt(&o.combo) {
                a["combo"] = json!(k);
            }
            Ok(("action.add", a))
        }
        ["action", "run"] => {
            if let Some(id) = opt(&o.id) {
                Ok(("action.run", json!({ "id": id })))
            } else if let Some(n) = opt(&o.name) {
                Ok(("action.run", json!({ "name": n })))
            } else {
                Err("missing --id <id> or --name <name>".to_string())
            }
        }
        ["action", "rm"] => Ok(("action.rm", json!({ "id": need_id(o)? }))),
        ["needed", "open"] => {
            let mut a = json!({ "title": need_flag(&o.title, "--title")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(w) = opt(&o.why) {
                a["why"] = json!(w);
            }
            Ok(("needed.open", a))
        }
        // Answer a Needed from the CLI —
        // the same needed-answer event path as the composer, so the watch
        // dispatches on it. The answer text is the `--` payload.
        ["needed", "answer"] => Ok((
            "needed.answer",
            json!({ "id": need_id(o)?, "answer": payload(o)? }),
        )),
        ["log", "append"] => {
            let mut a = json!({ "eType": need_flag(&o.etype, "--type")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(ac) = opt(&o.actor) {
                a["actor"] = json!(ac);
            }
            if !o.tail.is_empty() {
                let raw = o.tail.join(" ");
                let c: Value = serde_json::from_str(&raw)
                    .map_err(|e| format!("content JSON: {e}"))?;
                if !c.is_object() {
                    return Err("content must be a JSON object".to_string());
                }
                a["content"] = c;
            }
            Ok(("log.append", a))
        }
        ["stars", "open"] => Ok(("stars.open", json!({}))),
        ["stars", "checkin", "get"] => {
            let mut a = json!({});
            put_opt(&mut a, "date", &o.date);
            Ok(("stars.checkin.get", a))
        }
        ["stars", "checkin", "set"] => {
            let mut a = json!({
                "axis": need_flag(&o.axis, "--axis")?,
                "value": need_int(&o.value, "--value")?,
            });
            put_opt(&mut a, "date", &o.date);
            Ok(("stars.checkin.set", a))
        }
        ["stars", "log", "get"] => {
            let mut a = json!({});
            put_opt(&mut a, "date", &o.date);
            Ok(("stars.log.get", a))
        }
        ["stars", "log", "set"] => {
            if o.win1.is_none() && o.win2.is_none() && o.grateful.is_none() {
                return Err("need --win1, --win2, or --grateful".into());
            }
            let mut a = json!({});
            put_raw(&mut a, "win1", &o.win1);
            put_raw(&mut a, "win2", &o.win2);
            put_raw(&mut a, "grateful", &o.grateful);
            put_opt(&mut a, "date", &o.date);
            Ok(("stars.log.set", a))
        }
        ["stars", "log", "save"] => {
            let mut a = json!({});
            put_opt(&mut a, "date", &o.date);
            Ok(("stars.log.save", a))
        }
        ["stars", "journal", "list"] => {
            let mut a = json!({});
            put_opt(&mut a, "from", &o.from);
            put_opt(&mut a, "to", &o.to);
            put_opt(&mut a, "planet", &o.planet);
            Ok(("stars.journal.list", a))
        }
        ["stars", "journal", "read"] => {
            Ok(("stars.journal.read", json!({ "date": need_flag(&o.date, "--date")? })))
        }
        ["stars", "journal", "append"] => {
            let mut a = json!({ "text": payload(o)? });
            put_opt(&mut a, "date", &o.date);
            put_opt(&mut a, "planet", &o.planet);
            Ok(("stars.journal.append", a))
        }
        ["stars", "reflect", "templates"] => Ok(("stars.reflect.templates", json!({}))),
        ["stars", "reflect", "write"] => {
            let mut answers = if let Some(raw) = opt(&o.answers) {
                let v: Value = serde_json::from_str(&raw)
                    .map_err(|e| format!("answers JSON: {e}"))?;
                if !v.is_object() {
                    return Err("answers must be a JSON object".into());
                }
                v
            } else {
                json!({})
            };
            if let Some(v) = opt(&o.spokes) {
                answers["spokes"] = json!(on_off(&v, "--spokes")?);
            }
            if let Some(v) = opt(&o.wins) {
                answers["wins"] = json!(on_off(&v, "--wins")?);
            }
            if let Some(v) = opt(&o.grateful) {
                answers["grateful"] = json!(on_off(&v, "--grateful")?);
            }
            if let Some(v) = opt(&o.trackers) {
                answers["trackers"] = json!(on_off(&v, "--trackers")?);
            }
            if let Some(v) = opt(&o.cloud) {
                answers["cloud"] = json!(on_off(&v, "--cloud")?);
            }
            if let Some(p) = opt(&o.planet) {
                answers["planet"] = json!(p);
            }
            let mut a = json!({
                "template": opt(&o.template).unwrap_or_else(|| "ledger".into()),
                "answers": answers,
            });
            put_opt(&mut a, "date", &o.date);
            put_opt(&mut a, "planet", &o.planet);
            Ok(("stars.reflect.write", a))
        }
        ["stars", "tracker", "list"] => {
            let mut a = json!({});
            put_opt(&mut a, "planet", &o.planet);
            Ok(("stars.tracker.list", a))
        }
        ["stars", "tracker", "inc"] | ["stars", "tracker", "dec"] => {
            let verb = w[2];
            let mut a = json!({ "id": need_id(o)? });
            if let Some(n) = opt_pos_int(&o.by, "--by")? {
                a["by"] = json!(n);
            }
            Ok((
                if verb == "inc" {
                    "stars.tracker.inc"
                } else {
                    "stars.tracker.dec"
                },
                a,
            ))
        }
        ["stars", "tracker", "set"] => Ok((
            "stars.tracker.set",
            json!({
                "id": need_id(o)?,
                "value": need_int(&o.value, "--value")?,
            }),
        )),
        ["stars", "timer", "start"] => Ok(("stars.timer.start", json!({ "id": need_id(o)? }))),
        ["stars", "timer", "stop"] => Ok(("stars.timer.stop", json!({ "id": need_id(o)? }))),
        ["stars", "timer", "status"] => Ok(("stars.timer.status", json!({}))),
        ["stars", "cloud", "words"] => {
            let mut a = json!({});
            put_opt(&mut a, "range", &o.range);
            Ok(("stars.cloud.words", a))
        }
        ["packs", "list"] => Ok(("packs.list", json!({}))),
        ["sleeper", "wake"] => Ok(("sleeper.wake", json!({ "id": need_id(o)? }))),
        ["state", "get"] => Ok(("state.get", json!({}))),
        [] => Err(format!("no command\n\n{USAGE}")),
        ["settings", "plans-dir"] => {
            if let Some(p) = opt(&o.set) {
                Ok(("settings.plans_dir_set", json!({ "path": p })))
            } else {
                Ok(("settings.plans_dir", json!({})))
            }
        }
        ["settings", "runner"] => {
            if o.name.is_some() || o.harness.is_some() || o.model.is_some() {
                let mut a = json!({});
                put_opt(&mut a, "runner", &o.name);
                put_opt(&mut a, "harness", &o.harness);
                put_opt(&mut a, "model", &o.model);
                Ok(("settings.runner_set", a))
            } else {
                Ok(("settings.runner", json!({})))
            }
        }
        ["go", "status"] => {
            let mut a = json!({});
            put_opt(&mut a, "file", &o.file);
            put_opt(&mut a, "go", &o.go_id);
            put_opt(&mut a, "card", &o.card);
            put_opt(&mut a, "title", &o.title);
            Ok(("go.status", a))
        }
        ["go", "start"] => {
            let card = opt(&o.card).ok_or("go start needs --card")?;
            Ok(("go.start", json!({ "card": card })))
        }
        ["go", "run-status"] => Ok(("go.run_status", json!({ "run_id": need_id(o)? }))),
        ["go", "run-result"] => Ok(("go.run_result", json!({ "run_id": need_id(o)? }))),
        ["go", "runner"] => {
            if o.set.is_some() && o.reset {
                return Err("go runner: pass only one of --set or --reset".into());
            }
            if o.file.is_some() && (o.set.is_some() || o.reset) {
                return Err("go runner: --file is only for a read".into());
            }
            if o.reset {
                Ok(("go.runner_set", json!({ "reset": true })))
            } else if let Some(id) = opt(&o.set) {
                Ok(("go.runner_set", json!({ "runner": id })))
            } else {
                let mut a = json!({});
                put_opt(&mut a, "file", &o.file);
                Ok(("go.runner_get", a))
            }
        }
        ["go", "seal"] => {
            let file = opt(&o.file).ok_or("go seal needs --file")?;
            let by = opt(&o.by).ok_or("go seal needs --by")?;
            if !o.confirm {
                return Err("go seal needs --confirm".into());
            }
            Ok(("go.seal", json!({ "file": file, "by": by, "confirm": true })))
        }
        ["flags", "list"] => Ok(("flags.list", json!({}))),
        ["flags", "set", id, bit] => {
            let on = on_off(bit, "flags set")?;
            Ok(("flags.set", json!({ "id": id, "on": on })))
        }
        ["flags", "auto", bit] => {
            let on = on_off(bit, "flags auto")?;
            Ok(("flags.auto", json!({ "on": on })))
        }
        ["herdr", "status"] => Ok(("herdr.status", json!({}))),
        ["herdr", "peek", card] => {
            let mut a = json!({ "card": card });
            if let Some(n) = opt(&o.limit) {
                let lines: u64 = n.parse().unwrap_or(40);
                a["lines"] = json!(lines);
            }
            Ok(("herdr.peek", a))
        }
        ["plans", "migrate"] => {
            if o.apply && o.dry_run {
                return Err("plans migrate: pass only one of --dry-run or --apply".into());
            }
            if o.apply {
                Ok(("plans.migrate_apply", json!({})))
            } else {
                Ok(("plans.migrate", json!({ "dryRun": true })))
            }
        }
        other => Err(format!("unknown command: {}\n\n{USAGE}", other.join(" "))),
    }
}
