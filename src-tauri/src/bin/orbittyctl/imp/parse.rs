use super::usage::USAGE;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::exit;

pub(crate) struct Opts {
    pub(crate) json: bool,
    pub(crate) token: Option<String>,
    pub(crate) sock: Option<PathBuf>,
    pub(crate) id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) notes: Option<String>,
    pub(crate) why: Option<String>,
    pub(crate) etype: Option<String>,
    pub(crate) actor: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) cmd: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) combo: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) who: Option<String>,
    // Agent roster flags. `md` is repeatable.
    pub(crate) harness: Option<String>,
    pub(crate) profile: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) permissions: Option<String>,
    pub(crate) md: Vec<String>,
    pub(crate) extra: Option<String>,
    pub(crate) agent: Option<String>,
    /// kanban assign --window (empty clears).
    pub(crate) window: Option<String>,
    /// kanban add --link URL (repeatable), optional --link-label on the last.
    pub(crate) links: Vec<(String, Option<String>)>,
    /// kanban link --add URL
    pub(crate) add: Option<String>,
    /// kanban link --rm URL
    pub(crate) rm: Option<String>,
    /// kanban link --label
    pub(crate) label: Option<String>,
    /// kanban link --clear
    pub(crate) clear: bool,
    /// kanban open-link --n
    pub(crate) n: Option<String>,
    /// kanban open-link --run (executables; otherwise they are revealed).
    /// Does not record an approval.
    pub(crate) run: bool,
    pub(crate) card: Option<String>,
    pub(crate) file: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) date: Option<String>,
    pub(crate) axis: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) planet: Option<String>,
    pub(crate) win1: Option<String>,
    pub(crate) win2: Option<String>,
    pub(crate) grateful: Option<String>,
    pub(crate) template: Option<String>,
    pub(crate) answers: Option<String>,
    pub(crate) by: Option<String>,
    pub(crate) range: Option<String>,
    pub(crate) spokes: Option<String>,
    pub(crate) wins: Option<String>,
    pub(crate) trackers: Option<String>,
    pub(crate) cloud: Option<String>,
    pub(crate) words: Vec<String>,
    pub(crate) tail: Vec<String>,
    /// settings plans-dir --set PATH
    pub(crate) set: Option<String>,
    pub(crate) confirm: bool,
    /// kanban notes --append
    pub(crate) append: bool,
    pub(crate) dry_run: bool,
    pub(crate) apply: bool,
    /// go status --go GO-ID
    pub(crate) go_id: Option<String>,
    /// route run --pending ID (the pendingId a board-add returned)
    pub(crate) pending: Option<String>,
    /// history list --limit N
    pub(crate) limit: Option<String>,
    /// history rewind --remove-added
    pub(crate) remove_added: bool,
}

pub(crate) fn parse(argv: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        json: false,
        token: None,
        sock: None,
        id: None,
        title: None,
        project: None,
        status: None,
        notes: None,
        why: None,
        etype: None,
        actor: None,
        name: None,
        cmd: None,
        cwd: None,
        combo: None,
        mode: None,
        who: None,
        harness: None,
        profile: None,
        model: None,
        permissions: None,
        md: Vec::new(),
        extra: None,
        agent: None,
        window: None,
        links: Vec::new(),
        add: None,
        rm: None,
        label: None,
        clear: false,
        n: None,
        run: false,
        card: None,
        file: None,
        url: None,
        date: None,
        axis: None,
        value: None,
        from: None,
        to: None,
        planet: None,
        win1: None,
        win2: None,
        grateful: None,
        template: None,
        answers: None,
        by: None,
        range: None,
        spokes: None,
        wins: None,
        trackers: None,
        cloud: None,
        words: Vec::new(),
        tail: Vec::new(),
        set: None,
        confirm: false,
        append: false,
        dry_run: false,
        apply: false,
        go_id: None,
        pending: None,
        limit: None,
        remove_added: false,
    };
    let mut it = argv.iter();
    let mut after_ddash = false;
    while let Some(a) = it.next() {
        if after_ddash {
            o.tail.push(a.clone());
            continue;
        }
        match a.as_str() {
            "--" => after_ddash = true,
            "--json" => o.json = true,
            "--token" => {
                o.token = Some(it.next().ok_or("--token needs a value")?.clone())
            }
            "--sock" => {
                o.sock = Some(PathBuf::from(it.next().ok_or("--sock needs a path")?))
            }
            "--id" => o.id = Some(it.next().ok_or("--id needs a value")?.clone()),
            "--title" => o.title = Some(it.next().ok_or("--title needs a value")?.clone()),
            "--project" => {
                o.project = Some(it.next().ok_or("--project needs a value")?.clone())
            }
            "--status" => {
                o.status = Some(it.next().ok_or("--status needs a value")?.clone())
            }
            "--notes" => o.notes = Some(it.next().ok_or("--notes needs a value")?.clone()),
            "--why" => o.why = Some(it.next().ok_or("--why needs a value")?.clone()),
            "--type" => o.etype = Some(it.next().ok_or("--type needs a value")?.clone()),
            "--actor" => o.actor = Some(it.next().ok_or("--actor needs a value")?.clone()),
            "--name" => o.name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "--cmd" => o.cmd = Some(it.next().ok_or("--cmd needs a value")?.clone()),
            "--cwd" => o.cwd = Some(it.next().ok_or("--cwd needs a value")?.clone()),
            "--combo" => {
                o.combo = Some(it.next().ok_or("--combo needs a value")?.clone())
            }
            "--mode" => o.mode = Some(it.next().ok_or("--mode needs a value")?.clone()),
            "--who" => o.who = Some(it.next().ok_or("--who needs a value")?.clone()),
            "--harness" => {
                o.harness = Some(it.next().ok_or("--harness needs a value")?.clone())
            }
            "--profile" => {
                o.profile = Some(it.next().ok_or("--profile needs a value")?.clone())
            }
            "--model" => o.model = Some(it.next().ok_or("--model needs a value")?.clone()),
            "--permissions" => {
                o.permissions = Some(it.next().ok_or("--permissions needs a value")?.clone())
            }
            "--md" => o.md.push(it.next().ok_or("--md needs a path")?.clone()),
            "--extra" => o.extra = Some(it.next().ok_or("--extra needs a value")?.clone()),
            // Kept as Some("") when passed empty — kanban assign unassigns.
            "--agent" => o.agent = Some(it.next().ok_or("--agent needs a value")?.clone()),
            "--window" => o.window = Some(it.next().ok_or("--window needs a value")?.clone()),
            "--link" => {
                let url = it.next().ok_or("--link needs a URL")?.clone();
                o.links.push((url, None));
            }
            "--link-label" => {
                let label = it.next().ok_or("--link-label needs a value")?.clone();
                let last = o
                    .links
                    .last_mut()
                    .ok_or("--link-label needs a preceding --link")?;
                if last.1.is_some() {
                    return Err("one --link-label per --link".into());
                }
                last.1 = Some(label);
            }
            "--add" => o.add = Some(it.next().ok_or("--add needs a URL")?.clone()),
            "--rm" => o.rm = Some(it.next().ok_or("--rm needs a URL")?.clone()),
            "--label" => o.label = Some(it.next().ok_or("--label needs a value")?.clone()),
            "--clear" => o.clear = true,
            "--n" => o.n = Some(it.next().ok_or("--n needs a number")?.clone()),
            "--run" => o.run = true,
            "--card" => o.card = Some(it.next().ok_or("--card needs a value")?.clone()),
            "--pending" => {
                o.pending = Some(it.next().ok_or("--pending needs a pendingId")?.clone())
            }
            "--file" => o.file = Some(it.next().ok_or("--file needs a path")?.clone()),
            // Kept as Some("") when passed empty — relay replica clears.
            "--url" => o.url = Some(it.next().ok_or("--url needs a value")?.clone()),
            "--date" => o.date = Some(it.next().ok_or("--date needs a value")?.clone()),
            "--axis" => o.axis = Some(it.next().ok_or("--axis needs a value")?.clone()),
            "--value" => o.value = Some(it.next().ok_or("--value needs a value")?.clone()),
            "--from" => o.from = Some(it.next().ok_or("--from needs a value")?.clone()),
            "--to" => o.to = Some(it.next().ok_or("--to needs a value")?.clone()),
            "--planet" => o.planet = Some(it.next().ok_or("--planet needs a value")?.clone()),
            // Empty string is meaningful: clears that log line.
            "--win1" => o.win1 = Some(it.next().ok_or("--win1 needs a value")?.clone()),
            "--win2" => o.win2 = Some(it.next().ok_or("--win2 needs a value")?.clone()),
            "--grateful" => {
                o.grateful = Some(it.next().ok_or("--grateful needs a value")?.clone())
            }
            "--template" => {
                o.template = Some(it.next().ok_or("--template needs a value")?.clone())
            }
            "--answers" => {
                o.answers = Some(it.next().ok_or("--answers needs a value")?.clone())
            }
            "--by" => o.by = Some(it.next().ok_or("--by needs a value")?.clone()),
            "--limit" => o.limit = Some(it.next().ok_or("--limit needs a number")?.clone()),
            "--set" => o.set = Some(it.next().ok_or("--set needs a value")?.clone()),
            "--confirm" => o.confirm = true,
            "--remove-added" => o.remove_added = true,
            "--append" => o.append = true,
            "--dry-run" => o.dry_run = true,
            "--apply" => o.apply = true,
            "--go" => o.go_id = Some(it.next().ok_or("--go needs a value")?.clone()),
            "--range" => o.range = Some(it.next().ok_or("--range needs a value")?.clone()),
            "--spokes" => o.spokes = Some(it.next().ok_or("--spokes needs a value")?.clone()),
            "--wins" => o.wins = Some(it.next().ok_or("--wins needs a value")?.clone()),
            "--trackers" => {
                o.trackers = Some(it.next().ok_or("--trackers needs a value")?.clone())
            }
            "--cloud" => o.cloud = Some(it.next().ok_or("--cloud needs a value")?.clone()),
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0);
            }
            s if s.starts_with("--token=") => o.token = Some(s["--token=".len()..].to_string()),
            s if s.starts_with("--sock=") => o.sock = Some(PathBuf::from(&s["--sock=".len()..])),
            s if s.starts_with("--id=") => o.id = Some(s["--id=".len()..].to_string()),
            s if s.starts_with("--title=") => o.title = Some(s["--title=".len()..].to_string()),
            s if s.starts_with("--project=") => {
                o.project = Some(s["--project=".len()..].to_string())
            }
            s if s.starts_with("--status=") => o.status = Some(s["--status=".len()..].to_string()),
            s if s.starts_with("--notes=") => o.notes = Some(s["--notes=".len()..].to_string()),
            s if s.starts_with("--why=") => o.why = Some(s["--why=".len()..].to_string()),
            s if s.starts_with("--type=") => o.etype = Some(s["--type=".len()..].to_string()),
            s if s.starts_with("--actor=") => o.actor = Some(s["--actor=".len()..].to_string()),
            s if s.starts_with("--name=") => o.name = Some(s["--name=".len()..].to_string()),
            s if s.starts_with("--cmd=") => o.cmd = Some(s["--cmd=".len()..].to_string()),
            s if s.starts_with("--cwd=") => o.cwd = Some(s["--cwd=".len()..].to_string()),
            s if s.starts_with("--combo=") => {
                o.combo = Some(s["--combo=".len()..].to_string())
            }
            s if s.starts_with("--mode=") => o.mode = Some(s["--mode=".len()..].to_string()),
            s if s.starts_with("--who=") => o.who = Some(s["--who=".len()..].to_string()),
            s if s.starts_with("--harness=") => {
                o.harness = Some(s["--harness=".len()..].to_string())
            }
            s if s.starts_with("--profile=") => {
                o.profile = Some(s["--profile=".len()..].to_string())
            }
            s if s.starts_with("--model=") => o.model = Some(s["--model=".len()..].to_string()),
            s if s.starts_with("--permissions=") => {
                o.permissions = Some(s["--permissions=".len()..].to_string())
            }
            s if s.starts_with("--file=") => o.file = Some(s["--file=".len()..].to_string()),
            s if s.starts_with("--md=") => o.md.push(s["--md=".len()..].to_string()),
            s if s.starts_with("--extra=") => o.extra = Some(s["--extra=".len()..].to_string()),
            s if s.starts_with("--agent=") => o.agent = Some(s["--agent=".len()..].to_string()),
            s if s.starts_with("--link=") => o.links.push((s["--link=".len()..].to_string(), None)),
            s if s.starts_with("--link-label=") => {
                let label = s["--link-label=".len()..].to_string();
                let last = o
                    .links
                    .last_mut()
                    .ok_or("--link-label needs a preceding --link")?;
                if last.1.is_some() {
                    return Err("one --link-label per --link".into());
                }
                last.1 = Some(label);
            }
            s if s.starts_with("--add=") => o.add = Some(s["--add=".len()..].to_string()),
            s if s.starts_with("--rm=") => o.rm = Some(s["--rm=".len()..].to_string()),
            s if s.starts_with("--label=") => o.label = Some(s["--label=".len()..].to_string()),
            s if s.starts_with("--n=") => o.n = Some(s["--n=".len()..].to_string()),
            s if s.starts_with("--url=") => o.url = Some(s["--url=".len()..].to_string()),
            s if s.starts_with("--date=") => o.date = Some(s["--date=".len()..].to_string()),
            s if s.starts_with("--axis=") => o.axis = Some(s["--axis=".len()..].to_string()),
            s if s.starts_with("--value=") => o.value = Some(s["--value=".len()..].to_string()),
            s if s.starts_with("--from=") => o.from = Some(s["--from=".len()..].to_string()),
            s if s.starts_with("--to=") => o.to = Some(s["--to=".len()..].to_string()),
            s if s.starts_with("--planet=") => o.planet = Some(s["--planet=".len()..].to_string()),
            s if s.starts_with("--win1=") => o.win1 = Some(s["--win1=".len()..].to_string()),
            s if s.starts_with("--win2=") => o.win2 = Some(s["--win2=".len()..].to_string()),
            s if s.starts_with("--grateful=") => {
                o.grateful = Some(s["--grateful=".len()..].to_string())
            }
            s if s.starts_with("--template=") => {
                o.template = Some(s["--template=".len()..].to_string())
            }
            s if s.starts_with("--answers=") => {
                o.answers = Some(s["--answers=".len()..].to_string())
            }
            s if s.starts_with("--by=") => o.by = Some(s["--by=".len()..].to_string()),
            s if s.starts_with("--set=") => o.set = Some(s["--set=".len()..].to_string()),
            s if s.starts_with("--go=") => o.go_id = Some(s["--go=".len()..].to_string()),
            s if s.starts_with("--range=") => o.range = Some(s["--range=".len()..].to_string()),
            s if s.starts_with("--spokes=") => o.spokes = Some(s["--spokes=".len()..].to_string()),
            s if s.starts_with("--wins=") => o.wins = Some(s["--wins=".len()..].to_string()),
            s if s.starts_with("--trackers=") => {
                o.trackers = Some(s["--trackers=".len()..].to_string())
            }
            s if s.starts_with("--cloud=") => o.cloud = Some(s["--cloud=".len()..].to_string()),
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("unknown flag {s:?}\n\n{USAGE}"))
            }
            s => o.words.push(s.to_string()),
        }
    }
    Ok(o)
}

/// Payload for send / set-agent / play: everything after `--`, else
/// trailing words.
pub(crate) fn payload(o: &Opts) -> Result<String, String> {
    if !o.tail.is_empty() {
        return Ok(o.tail.join(" "));
    }
    if o.words.len() > 2 {
        return Ok(o.words[2..].join(" "));
    }
    Err("missing payload — e.g. orbittyctl companion send -- \"line\"".into())
}

pub(crate) fn need_id(o: &Opts) -> Result<String, String> {
    o.id
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing --id <id>".to_string())
}

/// Optional flag value: blank-after-trim = absent.
pub(crate) fn opt(v: &Option<String>) -> Option<String> {
    v.clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) fn need_flag(v: &Option<String>, flag: &str) -> Result<String, String> {
    opt(v).ok_or_else(|| format!("missing {flag} <value>"))
}

pub(crate) fn need_int(v: &Option<String>, flag: &str) -> Result<i64, String> {
    let s = need_flag(v, flag)?;
    s.parse::<i64>()
        .map_err(|_| format!("{flag} must be an integer"))
}

pub(crate) fn opt_pos_int(v: &Option<String>, flag: &str) -> Result<Option<i64>, String> {
    let Some(s) = opt(v) else {
        return Ok(None);
    };
    let n: i64 = s
        .parse()
        .map_err(|_| format!("{flag} must be an integer"))?;
    if n <= 0 {
        return Err(format!("{flag} must be a positive integer"));
    }
    Ok(Some(n))
}

pub(crate) fn on_off(v: &str, flag: &str) -> Result<bool, String> {
    match v.trim() {
        "on" | "true" | "1" => Ok(true),
        "off" | "false" | "0" => Ok(false),
        other => Err(format!("{flag} must be on or off, got {other:?}")),
    }
}

pub(crate) fn put_opt(a: &mut Value, key: &str, v: &Option<String>) {
    if let Some(s) = opt(v) {
        a[key] = json!(s);
    }
}

/// Keep an explicit empty string (log lines clear that way).
pub(crate) fn put_raw(a: &mut Value, key: &str, v: &Option<String>) {
    if let Some(s) = v {
        a[key] = json!(s);
    }
}

