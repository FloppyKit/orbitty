//! Fast-ack in front of the hop mouth. Ack is never a saved turn for
//! `chat`. Other verbs' ack is the them-row (no mouth spend). Never
//! silent-mint an Action.

use std::sync::{Mutex, MutexGuard};

use serde_json::{json, Value};
use tauri::Emitter;

/// The verb allowlist (lock). Unknown — including "think" — is `chat`.
pub const VERBS: [&str; 5] = ["chat", "do-go", "run-action", "ask-needed", "none"];

/// What the coordinator does with a conductor answer.
pub enum Step {
    /// Splash the ack over the wait mark; the mouth lands the reply.
    Chat(String),
    /// The ack IS the them-row — persist + return, no mouth spend.
    Final(String),
}

// the parked Action proposal (in-memory, never persisted)

struct PendingAction {
    name: String,
    command: String,
}

static PENDING_ACTION: Mutex<Option<PendingAction>> = Mutex::new(None);

fn lock_pending() -> MutexGuard<'static, Option<PendingAction>> {
    PENDING_ACTION.lock().unwrap_or_else(|e| e.into_inner())
}

// Ollama probe cache (the conductor runs on EVERY send)

static OLLAMA_PROBE: Mutex<Option<(bool, std::time::Instant)>> = Mutex::new(None);

/// mouth_hop probes per hop; the conductor probes per send, so a down
/// Ollama must not cost the 2s probe every line — remember the answer
/// for 15s. A just-started Ollama self-heals within that window (the
/// fallback classifier covers the gap; chat still reaches the mouth).
fn ollama_up_cached() -> bool {
    {
        let g = OLLAMA_PROBE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((up, when)) = *g {
            if when.elapsed().as_secs() < 15 {
                return up;
            }
        }
    }
    let up = crate::mouth_hop::ollama_up();
    *OLLAMA_PROBE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((up, std::time::Instant::now()));
    up
}

// the door

/// The conductor pass for one Spark send. None = conductor off (today's
/// mouth only). Runs the blocking curl on a worker thread.
pub async fn conduct(app: &tauri::AppHandle, line: &str) -> Option<Step> {
    let (on, model) = crate::state::load_state(app)
        .ok()
        .map(|s| (s.companion.conductor_on, s.companion.conductor_model))
        .unwrap_or((false, String::new()));
    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode.clone())
        .unwrap_or_else(|| "spark".into());
    // Chat/hold = join; never classify away. Spark/work = Quick ack on
    // so "hand to Cursor" does not occupy the mouth for 180s.
    if matches!(mode.as_str(), "chat" | "hold") {
        return None;
    }
    let on = on || matches!(mode.as_str(), "spark" | "work");
    if !on {
        return None;
    }
    let app2 = app.clone();
    let line = line.to_string();
    tauri::async_runtime::spawn_blocking(move || conduct_blocking(&app2, &line, &model))
        .await
        .ok()
}

fn conduct_blocking(app: &tauri::AppHandle, line: &str, model: &str) -> Step {
    // 1. A parked proposal answers first — it lived exactly one line.
    //    Yes mints through the ONE existing door; no declines; anything
    //    else drops the proposal and the line rides on as a normal send.
    if let Some(p) = lock_pending().take() {
        if is_yes(line) {
            return Step::Final(mint_action(app, &p.name, &p.command));
        }
        if is_no(line) {
            return Step::Final("Okay — not saving an Action.".into());
        }
        // Changed subject: dropped, never minted. Fall through.
    }
    // 2. Mint words? Never silent: a quoted command arms the one
    //    proposal; without one the honest how-to is the whole reply.
    match mint_scan(line) {
        Mint::Armed(ask) => {
            let them = format!(
                "Save Action \"{}\" → `{}`? Answer yes to mint it — anything else drops it.",
                ask.name, ask.command
            );
            *lock_pending() = Some(ask);
            return Step::Final(them);
        }
        Mint::Unarmed => {
            return Step::Final(
                "I never mint an Action on my own — say it like: save \"npm run build\" as action build, then answer yes."
                    .into(),
            );
        }
        Mint::No => {}
    }
    // 3. Classify — slim local first, non-LLM fallback.
    let (verb, ack) = ollama_classify(model, line).unwrap_or_else(|| fallback_classify(line));
    crate::event_log::append_best(
        app,
        "companion-ack",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "verb": verb, "n": ack.chars().count() }),
    );
    match verb {
        "chat" => Step::Chat(ack),
        // The model free-styles chat acks only — a non-chat verb gets the
        // fixed honest line, never a promise of action nothing took.
        v => Step::Final(fixed_ack(v)),
    }
}

/// The yes-mint through the existing action.add door (validation,
/// persist, combo re-sync, event log all live there). unix-only, same
/// as the door itself.
#[cfg(unix)]
fn mint_action(app: &tauri::AppHandle, name: &str, command: &str) -> String {
    match crate::actions::action_add(app, name, command, None, None) {
        Ok(a) => format!(
            "Action \"{}\" saved — `{}`. It's in Settings › Launchpad.",
            a.name, a.command
        ),
        Err(e) => format!("Action not saved: {e}"),
    }
}

#[cfg(not(unix))]
fn mint_action(_app: &tauri::AppHandle, _name: &str, _command: &str) -> String {
    "Actions mint on the mac build only.".into()
}

// yes / no (only consulted with a proposal parked)

fn bare(line: &str) -> &str {
    line.trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | '?' | ',' | '…'))
        .trim()
}

fn is_yes(line: &str) -> bool {
    let b = bare(line).to_ascii_lowercase();
    matches!(
        b.as_str(),
        "yes" | "y"
            | "yeah"
            | "yep"
            | "yup"
            | "sure"
            | "ok"
            | "okay"
            | "k"
            | "do it"
            | "save it"
            | "mint"
            | "mint it"
            | "go ahead"
            | "yes please"
    )
}

fn is_no(line: &str) -> bool {
    let b = bare(line).to_ascii_lowercase();
    matches!(
        b.as_str(),
        "no" | "n"
            | "nope"
            | "nah"
            | "cancel"
            | "drop it"
            | "never mind"
            | "nevermind"
            | "don't"
            | "dont"
            | "do not"
            | "no thanks"
    )
}

// propose-don't-save scan (deterministic — never the model)

enum Mint {
    No,
    /// Mint words but no quoted command — the honest how-to, no proposal.
    Unarmed,
    /// A command rode the line — park it and ask.
    Armed(PendingAction),
}

fn mint_scan(line: &str) -> Mint {
    let low = line.to_ascii_lowercase();
    if !low.contains("action") {
        return Mint::No;
    }
    let minty = [
        "save",
        "mint",
        "create",
        "remember",
        "new action",
        "add an action",
        "add a action",
        "add action",
        "make an action",
        "make a action",
        "make this an action",
        "make it an action",
    ]
    .iter()
    .any(|w| low.contains(w));
    if !minty {
        return Mint::No;
    }
    let Some(command) = quoted_span(line) else {
        return Mint::Unarmed;
    };
    let name = name_after(&low)
        .or_else(|| derive_name(&command))
        .unwrap_or_else(|| "spark-action".into());
    Mint::Armed(PendingAction { name, command })
}

/// The command is the first "…" or `…` quoted span — single quotes are
/// prose (don't, it's) and never delimit a command.
fn quoted_span(line: &str) -> Option<String> {
    for q in ['"', '`'] {
        if let Some(start) = line.find(q) {
            if let Some(end) = line[start + 1..].find(q) {
                let inner = line[start + 1..start + 1 + end].trim();
                if !inner.is_empty() {
                    return Some(inner.chars().take(200).collect());
                }
            }
        }
    }
    None
}

/// `as action X` / `as an action X` / `called X` / `named X` /
/// `name it X` — one token, slugged. The proposal line shows exactly
/// what yes would mint, so a rough parse is corrected by a no.
fn name_after(low: &str) -> Option<String> {
    for marker in ["as an action", "as a action", "as action", "called", "named", "name it"] {
        if let Some(i) = low.find(marker) {
            let rest = &low[i + marker.len()..];
            if let Some(tok) = rest.split_whitespace().next() {
                let s = slug(tok);
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// No name rode the line — the command's first word (basename) stands
/// in; the proposal line shows it, so a bad guess is corrected by a no.
fn derive_name(command: &str) -> Option<String> {
    let first = command.split_whitespace().next()?;
    let base = first.rsplit('/').next().unwrap_or(first);
    let s = slug(base);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if matches!(c, '-' | '_' | '.' | ' ') {
            out.push('-');
        }
        if out.len() >= 24 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

// classify: slim local first

const CONDUCTOR_SYS: &str = "You are Orbitty's conductor — a fast triage layer in front of the mouth. \
Read Matt's line and answer with EXACTLY one line, no quotes, no explanation:\n\
VERB|ack\n\
VERB is one of:\n\
chat — normal talk; the big mouth answers next (DEFAULT — when unsure, chat)\n\
none — pure noise needing no reply (ok, thanks, lol, got it)\n\
do-go — he wants a task or GO-… fired, started, or picked up\n\
run-action — he asks to run a saved Action by name\n\
ask-needed — the line needs Matt's own decision before anything moves\n\
ack: one short warm line (10 words max), first person as Orbitty, showing you heard — \
never the answer, never a promise of action. For none the ack is exactly: Heard you.\n\
Never invent rules or Actions.";

fn ollama_classify(model: &str, line: &str) -> Option<(&'static str, String)> {
    if !ollama_up_cached() {
        return None;
    }
    let model = if model.trim().is_empty() {
        crate::mouth_hop::ollama_model().ok()?
    } else {
        model.trim().to_string()
    };
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": CONDUCTOR_SYS },
            { "role": "user", "content": line },
        ],
        "stream": false,
        // Tiny + cool — the conductor answers in hundreds of ms, never
        // an essay. The 8s cap only bounds a cold model load; past it
        // the fallback classifier takes the send.
        "options": { "num_predict": 48, "temperature": 0.2 },
    });
    let (status, text) = crate::mouth_hop::curl_json(
        "POST",
        &format!("{}/api/chat", crate::mouth_hop::OLLAMA_BASE),
        None,
        Some(&body),
        8,
    )
    .ok()?;
    if !(200..=299).contains(&status) {
        return None;
    }
    let v: Value = serde_json::from_str(&text).ok()?;
    let raw = v.get("message")?.get("content")?.as_str()?;
    parse_ack(raw)
}

/// `VERB|ack` on the first non-empty line. No pipe = the model just
/// acked — verb chat. Unknown verb = chat (lock).
fn parse_ack(raw: &str) -> Option<(&'static str, String)> {
    let first = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    let (verb, ack) = match first.split_once('|') {
        Some((v, a)) => (normalize_verb(v), a.trim().trim_matches('"').trim().to_string()),
        None => ("chat", first.trim_matches('"').trim().to_string()),
    };
    if ack.is_empty() {
        return None;
    }
    Some((verb, ack.chars().take(140).collect()))
}

fn normalize_verb(v: &str) -> &'static str {
    match v.trim().to_ascii_lowercase().as_str() {
        "none" => "none",
        "do-go" | "do_go" | "dogo" | "go" => "do-go",
        "run-action" | "run_action" | "runaction" | "action" => "run-action",
        "ask-needed" | "ask_needed" | "askneeded" | "ask" => "ask-needed",
        // Lock: unknown → chat. "think" lands here too.
        _ => "chat",
    }
}

/// The non-LLM classifier — Ollama down / slow / garbage. Conservative:
/// only high-confidence shapes leave chat; everything else is chat.
fn fallback_classify(line: &str) -> (&'static str, String) {
    let low = line.trim().to_ascii_lowercase();
    let b = bare(&low).to_ascii_lowercase();
    // Pure noise — greetings stay chat (they deserve a real reply).
    if matches!(
        b.as_str(),
        "ok" | "okay"
            | "k"
            | "kk"
            | "thanks"
            | "thank you"
            | "thx"
            | "ty"
            | "lol"
            | "lmao"
            | "nice"
            | "cool"
            | "got it"
            | "yep"
            | "yeah"
            | "yup"
            | "nah"
            | "nope"
            | "bet"
            | "word"
            | "roger"
            | "copy"
            | "copy that"
            | "aight"
            | "aiight"
    ) {
        return ("none", fixed_ack("none"));
    }
    // A GO token + a fire verb in a shape the strict go: intercept
    // missed (it owns the real fire — this ack only points at it).
    if crate::needed_rules::extract_go(line).is_some()
        && ["run", "start", "fire", "launch", "pick up", "pickup", "take", "send"]
            .iter()
            .any(|w| low.contains(w))
    {
        return ("do-go", fixed_ack("do-go"));
    }
    // "run the action" — mint words were already handled above, so a
    // run verb + "action" here is a baked-Action ask that matched none.
    if low.contains("action")
        && ["run", "fire", "launch", "trigger", "execute"]
            .iter()
            .any(|w| low.contains(w))
    {
        return ("run-action", fixed_ack("run-action"));
    }
    // Hand-off to Cursor / pickup — don't occupy the mouth for 180s.
    if (low.contains("cursor") || low.contains("pickup") || low.contains("pick up"))
        && ["hand", "open", "take this", "do this", "fix this", "run this"]
            .iter()
            .any(|w| low.contains(w))
    {
        return ("do-go", fixed_ack("do-go"));
    }
    ("chat", "On it.".into())
}

/// Fixed acks for the non-chat verbs — honest about which existing door
/// owns the thing, never a promise the conductor did something it did
/// not. "Heard you." rides the hop-packet ack-noise skip list.
fn fixed_ack(verb: &str) -> String {
    match verb {
        "none" => "Heard you.".into(),
        "do-go" => "That's a go — the board fires it: drag the card to Doing, or say `GO-… cursor` here.".into(),
        "run-action" => {
            "No baked Action matched that — Settings › Launchpad lists them; say the exact name."
                .into()
        }
        "ask-needed" => "That call is yours — answer here and the line is held above.".into(),
        _ => "On it.".into(),
    }
}

// Settings surface (conductor on/off + model id)

/// Conductor toggle — the same load-modify-save + emit pattern as the
/// other companion switches. Default off = today's mouth only. Off also
/// drops any parked proposal: a switch-off never leaves a mint armed.
#[tauri::command]
pub fn set_companion_conductor(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    st.companion.conductor_on = enabled;
    crate::state::save_state(&app, &st)?;
    if !enabled {
        *lock_pending() = None;
    }
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Conductor model id (an Ollama tag). Blank = auto — the first pulled
/// model when Ollama is up; the non-LLM classifier when it isn't.
/// Local-only: a hosted id just fails the probe and lands on the
/// fallback. Never a key, never a URL.
#[tauri::command]
pub fn set_companion_conductor_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    let model = model.trim().chars().take(120).collect::<String>();
    let mut st = crate::state::load_state(&app)?;
    st.companion.conductor_model = model;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_verb_is_chat() {
        assert_eq!(normalize_verb("think"), "chat");
        assert_eq!(normalize_verb("CHAT"), "chat");
        assert_eq!(normalize_verb("bogus"), "chat");
        assert_eq!(normalize_verb("none"), "none");
        assert_eq!(normalize_verb("do-go"), "do-go");
        assert_eq!(normalize_verb("run-action"), "run-action");
        assert_eq!(normalize_verb("ask-needed"), "ask-needed");
    }

    #[test]
    fn parse_ack_splits() {
        assert_eq!(
            parse_ack("chat|On it — one sec.").unwrap(),
            ("chat", "On it — one sec.".to_string())
        );
        assert_eq!(
            parse_ack("none|Heard you.").unwrap(),
            ("none", "Heard you.".to_string())
        );
        // No pipe = the model just acked; verb chat.
        assert_eq!(
            parse_ack("Sure, one moment.").unwrap(),
            ("chat", "Sure, one moment.".to_string())
        );
        assert!(parse_ack("chat|  ").is_none());
    }

    #[test]
    fn mint_scan_arms_only_with_quoted_command() {
        assert!(matches!(mint_scan("how was your day"), Mint::No));
        // run words are not mint words.
        assert!(matches!(mint_scan("run the build action"), Mint::No));
        assert!(matches!(
            mint_scan("save this as an action"),
            Mint::Unarmed
        ));
        match mint_scan("save \"npm run build\" as action build") {
            Mint::Armed(a) => {
                assert_eq!(a.command, "npm run build");
                assert_eq!(a.name, "build");
            }
            _ => panic!("should arm"),
        }
        match mint_scan("make an action called `git status`") {
            Mint::Armed(a) => {
                assert_eq!(a.command, "git status");
                // backtick-quoted name slug: "called `git" → slug strips.
                assert!(!a.name.is_empty());
            }
            _ => panic!("should arm"),
        }
        // No name → derived from the command's first word.
        match mint_scan("save \"orbittyctl kanban open\" as an action") {
            Mint::Armed(a) => assert_eq!(a.name, "orbittyctl"),
            _ => panic!("should arm"),
        }
    }

    #[test]
    fn yes_no_only_in_context() {
        assert!(is_yes("yes"));
        assert!(is_yes("Yes!"));
        assert!(is_yes("do it"));
        assert!(is_no("no"));
        assert!(is_no("nope."));
        assert!(!is_yes("yes we can talk later about it"));
        assert!(!is_no("no idea how this works"));
    }

    #[test]
    fn fallback_is_conservative() {
        assert_eq!(fallback_classify("ok").0, "none");
        assert_eq!(fallback_classify("thanks!").0, "none");
        assert_eq!(fallback_classify("hello").0, "chat");
        assert_eq!(fallback_classify("what do you think about this?").0, "chat");
        assert_eq!(
            fallback_classify("please run GO-ORBITTY-MOUTH-ACK today").0,
            "do-go"
        );
        assert_eq!(fallback_classify("run the build action").0, "run-action");
        // A plain mention is not a fire.
        assert_eq!(fallback_classify("what about GO-ORBITTY-MOUTH-ACK?").0, "chat");
    }
}
