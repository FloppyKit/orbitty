use std::path::{ PathBuf };
use tauri::{ Emitter };
use super::allowlist::{companions_root};

// A typed line can name a Launchpad Action

/// `Run` carries the shortcut plus `keep_you`: prefix/command chrome never
/// persists a you-line; a bare name typed as a sentence keeps it.
pub(crate) enum ActionIntercept {
    Run(crate::state::ActionShortcut, bool),
    Ambiguous,
}

/// Local match only, in law order: exact name (case-insensitive), whole
/// line == command, then `run|/run|action <name>`. No fuzzy, no NLP —
/// `please run Build` is a chat line. Two equal candidates in one tier →
/// Ambiguous, which never fires.
pub(crate) fn match_action_line(
    shortcuts: &[crate::state::ActionShortcut],
    line: &str,
) -> Option<ActionIntercept> {
    if shortcuts.is_empty() {
        return None;
    }

    let by_name: Vec<_> = shortcuts
        .iter()
        .filter(|a| a.name.trim().eq_ignore_ascii_case(line))
        .collect();
    if by_name.len() == 1 {
        return Some(ActionIntercept::Run(by_name[0].clone(), true));
    }
    if by_name.len() > 1 {
        return Some(ActionIntercept::Ambiguous);
    }

    let by_command: Vec<_> = shortcuts
        .iter()
        .filter(|a| a.command.trim() == line)
        .collect();
    if by_command.len() == 1 {
        return Some(ActionIntercept::Run(by_command[0].clone(), false));
    }
    if by_command.len() > 1 {
        return Some(ActionIntercept::Ambiguous);
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    if matches!(head.to_ascii_lowercase().as_str(), "run" | "/run" | "action") && !rest.is_empty() {
        let by_prefix: Vec<_> = shortcuts
            .iter()
            .filter(|a| a.name.trim().eq_ignore_ascii_case(rest))
            .collect();
        if by_prefix.len() == 1 {
            return Some(ActionIntercept::Run(by_prefix[0].clone(), false));
        }
        if by_prefix.len() > 1 {
            return Some(ActionIntercept::Ambiguous);
        }
    }

    None
}

pub(crate) fn action_line_hits(shortcuts: &[crate::state::ActionShortcut], line: &str) -> bool {
    match_action_line(shortcuts, line).is_some()
}

pub(crate) fn log_companion_route(app: &tauri::AppHandle, route: &str) {
    crate::event_log::append_best(
        app,
        "companion-route",
        None,
        None,
        Some("agent"),
        true,
        &serde_json::json!({ "route": route }),
    );
}

/// Intercept persist: them-line always lands; you-line only when the line
/// read as chat. Same duplicate-you skip + cap 40 as the normal send path.
/// push_turn lands on the one companion transcript
/// (and a shadow copy under the current mode for old readers).
/// Land the you-row + last_user_line NOW, before the 180s model wait.
/// Collapsed CC and expanded transcript both read persist; waiting until
/// the reply meant a hung send showed nothing and state:changed wiped
/// the optimistic bubble.
pub(crate) fn persist_companion_you(app: &tauri::AppHandle, line: &str) {
    if crate::state::update(app, |st| {
        st.companion.last_user_line = line.to_string();
        let you_already_saved = st
            .companion
            .turns
            .last()
            .is_some_and(|t| t.role == "you" && t.text == line);
        if !you_already_saved {
            st.companion.push_turn("you", line);
        }
        Ok(())
    })
    .is_ok()
    {
        let _ = app.emit("state:changed", ());
    }
}

/// Saves the them-row stripped and returns that same stripped string, so
/// the live bubble never shows a button the saved row doesn't have.
#[must_use]
pub(crate) fn persist_action_intercept(app: &tauri::AppHandle, you: Option<&str>, them: &str) -> String {
    // Command output is not allowed to mint a handoff button either.
    let stripped = crate::mouth_hop::strip_handoff_marks(them);
    let them = stripped.as_str();
    if crate::state::update(app, |st| {
        if let Some(you) = you {
            st.companion.last_user_line = you.to_string();
            let you_already_saved = st
                .companion
                .turns
                .last()
                .is_some_and(|t| t.role == "you" && t.text == you);
            if !you_already_saved {
                st.companion.push_turn("you", you);
            }
        }
        st.companion.last_bot_line = them.to_string();
        st.companion.push_turn("them", them);
        Ok(())
    })
    .is_ok()
    {
        let _ = app.emit("state:changed", ());
    }
    stripped
}

// GO-MH-2-APP: leftover map inject (read-only, fail open)

/// Tiny structs with serde defaults — a hand-edited map with extra or
/// missing keys still parses. Schema: leftover_item.schema.json.
#[derive(serde::Deserialize)]
pub(crate) struct LeftoverMap {
    #[serde(default)]
    items: Vec<LeftoverItem>,
}

#[derive(serde::Deserialize)]
pub(crate) struct LeftoverItem {
    #[serde(default)]
    text: String,
    #[serde(default)]
    tag: String,
}

/// Prepend the leftover pocket to the outbound prompt (every mouth:
/// goose, grok, hermes, berd).
/// Fail OPEN: missing file / bad JSON / no usable items → the user line
/// unchanged; a ping is never blocked. The wrap is what the model sees
/// only — transcript, captions, and the event log keep the raw line.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub(crate) struct NowPacket {
    #[serde(default)]
    pub(crate) job: String,
    #[serde(default)]
    pub(crate) last_error: String,
    #[serde(default)]
    pub(crate) last_green: String,
    #[serde(default)]
    pub(crate) do_not_retry: String,
    #[serde(default)]
    pub(crate) files: Vec<String>,
}

pub(crate) fn now_path() -> PathBuf {
    companions_root().join("now.json")
}

pub(crate) fn load_now() -> NowPacket {
    let Ok(raw) = std::fs::read_to_string(now_path()) else {
        return NowPacket::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub(crate) fn save_now(n: &NowPacket) {
    let Ok(body) = serde_json::to_string_pretty(n) else {
        return;
    };
    let _ = std::fs::write(now_path(), body + "\n");
}

pub(crate) fn clip_now(s: &str, n: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= n {
        t
    } else {
        t.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Five-line current-job packet. Empty file = no block. Fail open.
pub(crate) fn now_block() -> String {
    let n = load_now();
    let mut lines: Vec<String> = Vec::new();
    if !n.job.trim().is_empty() {
        lines.push(format!("job: {}", clip_now(&n.job, 160)));
    }
    if !n.last_error.trim().is_empty() {
        lines.push(format!("last_error: {}", clip_now(&n.last_error, 160)));
    }
    if !n.last_green.trim().is_empty() {
        lines.push(format!("last_green: {}", clip_now(&n.last_green, 120)));
    }
    if !n.do_not_retry.trim().is_empty() {
        lines.push(format!("do_not_retry: {}", clip_now(&n.do_not_retry, 120)));
    }
    let files: Vec<_> = n
        .files
        .iter()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
        .take(4)
        .collect();
    if !files.is_empty() {
        lines.push(format!("files: {}", files.join(", ")));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("[now]\n{}\n", lines.join("\n"))
}

pub(crate) fn mode_job_line(mode: &str) -> &'static str {
    match mode {
        "chat" => "join and finesse; stay in the bit; no vaults; THIN drops the bit",
        "work" => "build/answer; no alliance; no so-close; task-stuck → say so",
        "hold" => "parked; notes only",
        _ => "desk light; thin; route heavy work out",
    }
}

pub(crate) fn wrap_with_pocket(app: &tauri::AppHandle, line: &str) -> String {
    const PREPEND_CAP: usize = 900;
    const MAX_ITEMS: usize = 5;
    const ITEM_TEXT_CAP: usize = 120;

    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode.clone())
        .unwrap_or_else(|| "spark".into());
    let mode_line = format!(
        "[orbitty-mode: {mode}] {}\n{}",
        mode_job_line(&mode),
        now_block()
    );

    let raw = match std::fs::read_to_string(companions_root().join("leftovers.json")) {
        Ok(s) => s,
        Err(_) => return format!("{mode_line}\nUSER:\n{line}"),
    };
    let map: LeftoverMap = match serde_json::from_str(&raw) {
        Ok(m) => m,
        Err(_) => return format!("{mode_line}\nUSER:\n{line}"),
    };

    let mut items: Vec<&LeftoverItem> = map
        .items
        .iter()
        .filter(|it| {
            let text = it.text.trim();
            !text.is_empty() && !text.eq_ignore_ascii_case("desk test")
        })
        .filter(|it| {
            let t = it.tag.trim().to_ascii_lowercase();
            t == "open" || t == "keep"
        })
        .collect();
    // Open first (live), then keep (working set).
    items.sort_by_key(|it| {
        if it.tag.trim().eq_ignore_ascii_case("open") {
            0
        } else {
            1
        }
    });
    items.truncate(MAX_ITEMS);

    let header = format!("{mode_line}[pocket]\n");
    let footer = "\nUSER:\n";
    if items.is_empty() {
        return format!("{header}{footer}{line}");
    }

    let mut budget = PREPEND_CAP.saturating_sub(header.chars().count() + footer.chars().count());
    let mut lines: Vec<String> = Vec::new();
    for it in items {
        let mut text = it.text.trim().replace('\n', " ");
        if text.chars().count() > ITEM_TEXT_CAP {
            text = text.chars().take(ITEM_TEXT_CAP - 1).collect::<String>() + "…";
        }
        let item_line = format!("- {text}\n");
        let n = item_line.chars().count();
        if n > budget {
            break;
        }
        budget -= n;
        lines.push(item_line);
    }
    if lines.is_empty() {
        return format!("{header}{footer}{line}");
    }
    format!("{header}{}{footer}{line}", lines.concat())
}
