use super::parse::Opts;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) fn default_sock() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac/control.sock")
}

pub(crate) fn sock_path(o: &Opts) -> PathBuf {
    if let Some(p) = &o.sock {
        return p.clone();
    }
    if let Ok(p) = std::env::var("ORBITTY_CONTROL_SOCK") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    default_sock()
}

/// Token precedence: --token flag, then env, then control.token next to
/// the socket in use.
pub(crate) fn token(o: &Opts, sock: &Path) -> Option<String> {
    if let Some(t) = &o.token {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("ORBITTY_CONTROL_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    sock.parent().and_then(|dir| {
        std::fs::read_to_string(dir.join("control.token"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

pub(crate) fn print_gate(data: &Value) {
    let armed = data["armed"].as_bool().unwrap_or(false);
    let frozen = data["frozen"].as_bool().unwrap_or(false);
    println!(
        "gate {} · freeze {} · chatter {} · stop {}",
        if armed { "armed" } else { "disarmed" },
        if frozen { "on" } else { "off" },
        data["chatterHeld"].as_u64().unwrap_or(0),
        data["stopHonored"].as_u64().unwrap_or(0)
    );
}

pub(crate) fn print_human(tool: &str, data: &Value) {
    match tool {
        "app.status" => {
            let trusted = data["ax"]["trusted"].as_bool().unwrap_or(false);
            println!(
                "Orbitty up · socket ok · AX {}",
                if trusted { "trusted" } else { "NOT trusted" }
            );
            if !trusted {
                println!("  grant: System Settings → Privacy & Security → Accessibility");
            }
        }
        "app.accessibility_status" => {
            println!("trusted:  {}", data["trusted"].as_bool().unwrap_or(false));
            println!("bundle:   {}", data["bundlePath"].as_str().unwrap_or(""));
            let s = data["signingLine"].as_str().unwrap_or("");
            if !s.is_empty() {
                println!("signing:  {s}");
            }
        }
        "windows.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no windows)");
            }
            for w in arr {
                let id = w["id"].as_str().unwrap_or("?");
                let app = w["appName"].as_str().unwrap_or("?");
                let title = w["title"].as_str().unwrap_or("");
                let mut marks = String::new();
                if w["isFrontmost"].as_bool().unwrap_or(false) {
                    marks.push_str(" [frontmost]");
                }
                if w["isMinimized"].as_bool().unwrap_or(false) {
                    marks.push_str(" [minimized]");
                }
                if title.is_empty() {
                    println!("{id}  {app}{marks}");
                } else {
                    println!("{id}  {app} — {title}{marks}");
                }
            }
        }
        "windows.focus" => println!("focused"),
        "windows.minimize" => println!("minimized"),
        "windows.restore" => println!("restored"),
        "windows.close" => println!("close pressed"),
        "companion.open" => println!("companion open"),
        "kanban.open" => println!("kanban open"),
        "stars.open" => println!("stars open"),
        "companion.send" => println!("{}", data["reply"].as_str().unwrap_or("")),
        "companion.agents" => {
            let sel = data["selected"].as_str().unwrap_or("");
            if let Some(arr) = data["agents"].as_array() {
                for a in arr {
                    let n = a.as_str().unwrap_or("");
                    println!("{} {}", if n == sel { "*" } else { " " }, n);
                }
            }
        }
        "companion.set_agent" => println!("agent: {}", data.as_str().unwrap_or("")),
        "companion.harnesses" => {
            let id = data["id"].as_str().unwrap_or("");
            let source = data["source"].as_str().unwrap_or("");
            if id.is_empty() {
                println!("harness: (unset)");
            } else {
                println!("harness: {id} ({source})");
            }
            if data["envOverride"].as_bool() == Some(true) {
                println!("set by ORBITTY_COMPANION_BACKEND");
            }
            if let Some(arr) = data["choices"].as_array() {
                for c in arr {
                    let cid = c["id"].as_str().unwrap_or("");
                    let mark = if c["present"].as_bool().unwrap_or(false) {
                        "present"
                    } else {
                        "absent"
                    };
                    println!("  {cid}  {mark}");
                }
            }
        }
        "companion.set_backend" => {
            let msg = data["message"].as_str().unwrap_or("");
            if msg.is_empty() {
                println!("harness: {}", data["backend"].as_str().unwrap_or(""));
            } else {
                println!("{msg}");
            }
        }
        "companion.route" => {
            println!("route: {}", data["route"].as_str().unwrap_or(""));
            println!(
                "backend: {} ({})",
                data["backend"].as_str().unwrap_or(""),
                data["source"].as_str().unwrap_or("")
            );
            if let Some(arr) = data["lanes"].as_array() {
                let lanes: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                println!("lanes: {}", lanes.join(", "));
            }
        }
        "companion.play" => println!("playing: {}", data["played"].as_str().unwrap_or("")),
        "companion.mode" => println!("mode: {}", data.as_str().unwrap_or("")),
        "companion.free_mode" => println!(
            "free mode: {}",
            if data["freeMode"].as_bool() == Some(true) {
                "on"
            } else {
                "off"
            }
        ),
        "companion.note" => println!("noted ({})", data.as_str().unwrap_or("")),
        "companion.thin" => println!("{}", data.as_str().unwrap_or("thin")),
        "companion.now" => {
            if data.is_string() {
                println!("{}", data.as_str().unwrap_or(""));
            } else {
                println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
            }
        }
        "companion.hack" => {
            let live = data["live"].as_bool().unwrap_or(false);
            let who = data["who"].as_str().unwrap_or("");
            match (live, who.is_empty()) {
                (true, false) => println!("hack: on ({who})"),
                (true, true) => println!("hack: on"),
                _ => println!("hack: off"),
            }
        }
        "packs.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no packs)");
            }
            for p in arr {
                let id = p["id"].as_str().unwrap_or("?");
                let name = p["name"].as_str().unwrap_or("");
                let frames = p["frameCount"].as_u64().unwrap_or(0);
                let warn = if p["warnLicense"].as_bool().unwrap_or(false) {
                    "  [no license]"
                } else {
                    ""
                };
                println!("{id}  {name} ({frames} frames){warn}");
            }
        }
        "sleeper.wake" => println!("{}", data.as_str().unwrap_or("")),
        "kanban.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "kanban.move" => {
            println!(
                "{} → {}",
                data["id"].as_str().unwrap_or("?"),
                data["status"].as_str().unwrap_or("?")
            );
            if let Some(ids) = data.get("readyIds").and_then(|v| v.as_array()) {
                let list: Vec<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
                println!("readyIds: {}", list.join(" "));
            }
        }
        // The reply is the card; notes
        // absent = cleared back to title-only.
        "kanban.notes" | "kanban.note_append" => {
            let id = data["id"].as_str().unwrap_or("?");
            match data["notes"].as_str().filter(|n| !n.is_empty()) {
                Some(n) => println!("notes {id} — {}", n.lines().next().unwrap_or("")),
                None => println!("notes cleared {id}"),
            }
        }
        "kanban.link" => {
            let id = data["id"].as_str().unwrap_or("?");
            let n = data["links"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("links {id} — {n}");
        }
        "kanban.open_link" => {
            println!(
                "opened {} n={} {}",
                data["id"].as_str().unwrap_or("?"),
                data["n"].as_u64().unwrap_or(0),
                data["url"].as_str().unwrap_or("")
            );
        }
        // Agent roster printers.
        "kanban.assign" => {
            let id = data["id"].as_str().unwrap_or("?");
            let title = data["title"].as_str().unwrap_or("");
            match data["agent"].as_str() {
                Some(a) => println!("assigned {id} — {title} → {a}"),
                None => println!("unassigned {id} — {title}"),
            }
        }
        "agent.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no agents)");
            }
            for a in arr {
                let id = a["id"].as_str().unwrap_or("?");
                let name = a["name"].as_str().unwrap_or("?");
                let harness = a["harness"].as_str().unwrap_or("?");
                let profile = a["profile"].as_str().unwrap_or("");
                let project = a["project"]
                    .as_str()
                    .or_else(|| a["projectId"].as_str())
                    .unwrap_or("—");
                let mut marks = String::new();
                if let Some(m) = a["model"].as_str().filter(|m| !m.is_empty()) {
                    marks.push_str(&format!(" model={m}"));
                }
                let md_n = a["mdPaths"].as_array().map(|x| x.len()).unwrap_or(0);
                if md_n > 0 {
                    marks.push_str(&format!(" md={md_n}"));
                }
                if a["extra"].as_str().is_some_and(|x| !x.is_empty()) {
                    marks.push_str(" +extra");
                }
                let prof = if profile.is_empty() {
                    String::new()
                } else {
                    format!("  {profile}")
                };
                println!("{id}  {name}  {harness}{prof}  → {project}{marks}");
            }
        }
        "agent.add" => {
            let id = data["id"].as_str().unwrap_or("?");
            let name = data["name"].as_str().unwrap_or("");
            match data["project"].as_str() {
                Some(p) => println!("added {id} — {name} on {p}"),
                None => println!("added {id} — {name}"),
            }
        }
        "agent.rm" => println!(
            "removed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "agent.pin" => println!(
            "pinned {} — {} → {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or(""),
            data["project"].as_str().unwrap_or("?")
        ),
        "agent.pickup" => {
            if data["picked"].as_bool() == Some(true) {
                println!(
                    "picked {} — {} → doing",
                    data["cardId"].as_str().unwrap_or("?"),
                    data["title"].as_str().unwrap_or("")
                );
                if let Some(l) = data["launch"].as_str() {
                    println!("{l}");
                }
            } else {
                println!(
                    "nothing assigned to {} — no spawn",
                    data["agent"].as_str().unwrap_or("?")
                );
            }
        }
        "gate.status" => print_gate(data),
        "gate.inject" => {
            let kind = data["kind"].as_str().unwrap_or("?");
            let verdict = data["verdict"].as_str().unwrap_or("?");
            println!("{kind} → {verdict}");
            print_gate(&data["status"]);
        }
        "shift.status" => {
            let on = data["on"].as_bool().unwrap_or(false);
            let mode = data["mode"].as_str().unwrap_or("?");
            let live = data["runners"]["live"].as_u64().unwrap_or(0);
            let cap = data["runners"]["cap"].as_u64().unwrap_or(0);
            let ax = data["axTrusted"].as_bool().unwrap_or(false);
            println!(
                "shift {} · mode {mode} · runners {live}/{cap} · AX {}",
                if on { "on" } else { "off" },
                if ax { "trusted" } else { "NOT trusted" }
            );
            if let Some(runs) = data["runs"].as_array() {
                for r in runs {
                    let ghost = if r["ghost"].as_bool().unwrap_or(false) {
                        " ghost"
                    } else {
                        ""
                    };
                    println!(
                        "  {}  {}  {}{ghost}",
                        r["state"].as_str().unwrap_or("?"),
                        r["cardId"].as_str().unwrap_or("?"),
                        r["title"].as_str().unwrap_or("")
                    );
                }
            }
            if let Some(next) = data["next"].as_array() {
                for n in next {
                    println!(
                        "  next {} — {} ({})",
                        n["agent"].as_str().unwrap_or("?"),
                        n["title"].as_str().unwrap_or(""),
                        n["cardId"].as_str().unwrap_or("?")
                    );
                }
            }
        }
        "shift.tick" => {
            let parked = data["parked"].as_array().map(|a| a.len()).unwrap_or(0);
            let escalated = data["escalated"].as_array().map(|a| a.len()).unwrap_or(0);
            let picked = data["picked"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("tick · parked {parked} · stuck {escalated} · picked {picked}");
            if let Some(notes) = data["notes"].as_array() {
                for n in notes {
                    if let Some(s) = n.as_str() {
                        println!("  {s}");
                    }
                }
            }
        }
        "shift.on" => println!(
            "shift on{}",
            if data["tick"].is_object() {
                " · ticked"
            } else {
                ""
            }
        ),
        "shift.off" => println!("shift off"),
        "relay.status" => {
            let on = data["on"].as_bool().unwrap_or(false);
            let url = data["url"].as_str().unwrap_or("").trim();
            let replica = if url.is_empty() {
                "local only"
            } else {
                url
            };
            println!(
                "private relay {} · replica {replica}",
                if on { "on" } else { "off" }
            );
        }
        "relay.replica" => {
            let url = data["url"].as_str().unwrap_or("").trim();
            if url.is_empty() {
                println!("replica cleared — local only");
            } else {
                println!("replica {url}");
            }
        }
        "project.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "project.rm" => println!(
            "removed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "project.rename" => println!(
            "renamed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "action.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no actions)");
            }
            for a in arr {
                let id = a["id"].as_str().unwrap_or("?");
                let name = a["name"].as_str().unwrap_or("?");
                let combo = a["combo"].as_str().unwrap_or("—");
                let cmd = a["command"].as_str().unwrap_or("");
                let mark = if a["status"].as_str() == Some("conflict") {
                    "  [conflict]"
                } else {
                    ""
                };
                println!("{id}  {name}  {combo}  {cmd}{mark}");
            }
        }
        "action.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "action.rm" => println!("removed {}", data["id"].as_str().unwrap_or("?")),
        "action.run" => {
            let ok = data["ok"].as_bool().unwrap_or(false);
            let head = if ok {
                "done".to_string()
            } else {
                format!(
                    "exit {}",
                    data["code"]
                        .as_i64()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".into())
                )
            };
            println!("{head}");
            for s in ["stdout", "stderr"]
                .into_iter()
                .filter_map(|k| data[k].as_str())
                .filter(|s| !s.is_empty())
            {
                println!("{s}");
            }
        }
        "needed.open" => println!(
            "needed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "needed.answer" => println!(
            "answered {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "log.append" => {
            if data["skipped"].as_bool() == Some(true) {
                println!("log append skipped (local log unavailable — not an error)");
            } else {
                println!("logged {}", data["id"].as_str().unwrap_or("?"));
            }
        }
        "flags.list" => {
            let Some(arr) = data.as_array() else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into())
                );
                return;
            };
            for row in arr {
                let id = row["id"].as_str().unwrap_or("?");
                let on = if row["on"].as_bool().unwrap_or(false) {
                    "on"
                } else {
                    "off"
                };
                let source = row["source"].as_str().unwrap_or("");
                let stale = if row["stale"].as_bool().unwrap_or(false) {
                    " stale"
                } else {
                    ""
                };
                println!("{id}  {on}  {source}{stale}");
            }
        }
        "flags.set" => {
            let id = data["id"].as_str().unwrap_or("?");
            let on = if data["on"].as_bool().unwrap_or(false) {
                "on"
            } else {
                "off"
            };
            let source = data["source"].as_str().unwrap_or("");
            println!("{id}  {on}  {source}");
        }
        "flags.auto" => {
            let on = if data["auto"].as_bool().unwrap_or(false) {
                "on"
            } else {
                "off"
            };
            println!("flags auto {on}");
        }
        "wake.list" | "wake.inbox" | "wake.resume" => {
            let lines = data.get("lines").or_else(|| data.get("subs")).and_then(|v| v.as_array());
            let n = lines.map(|a| a.len()).unwrap_or(0);
            println!("{n}");
        }
        "wake.status" | "wake.alarm" => {
            let active = data["active"].as_bool().unwrap_or(false);
            let at = data["at"].as_str().unwrap_or("null");
            let unacked = data["unacked"].as_u64().unwrap_or(0);
            println!("active={active} at={at} unacked={unacked}");
            if data["seq_ok"].as_bool() == Some(false) {
                if let Some(w) = data["seq_warn"].as_str().filter(|s| !s.is_empty()) {
                    println!("{w}");
                }
            }
        }
        "wake.subscribe" => {
            println!(
                "subscribed {} {}",
                data["id"].as_str().unwrap_or("?"),
                data["seat"].as_str().unwrap_or("")
            );
        }
        "wake.ack" | "wake.unsubscribe" | "wake.emit" => {
            println!("{}", serde_json::to_string(data).unwrap_or_else(|_| "ok".into()));
        }
        "herdr.status" => {
            if let Some(line) = data["line"].as_str().filter(|s| !s.is_empty()) {
                println!("{line}");
            } else {
                let source = data["source"].as_str().unwrap_or("fallback");
                let version = data["version"].as_str().unwrap_or("");
                if data["up"].as_bool() == Some(true) {
                    println!("up/source={source} {version}");
                } else {
                    println!("unknown/source={source} {version}");
                }
            }
        }
        "herdr.peek" => {
            let status = data["status"].as_str().unwrap_or("unknown");
            let text = data["text"].as_str().unwrap_or("");
            println!("{status}");
            if !text.is_empty() {
                println!("{text}");
            }
        }
        "herdr.jump" => {
            if data["ok"].as_bool() == Some(true) {
                println!("Opened the terminal.");
            } else {
                println!(
                    "{}",
                    data["reason"].as_str().unwrap_or("Couldn't open the terminal.")
                );
            }
        }
        // state.get and anything else: pretty JSON is the honest human view.
        _ => println!(
            "{}",
            serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into())
        ),
    }
}
