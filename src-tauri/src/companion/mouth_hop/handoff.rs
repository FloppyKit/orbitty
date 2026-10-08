use serde_json::{json, Value};
use tauri::Emitter;

use super::local::hop_messages;
use super::local::local_chat;
use super::openrouter::VisionGate;
use super::openrouter::compat_chat;
use super::openrouter::decide_vision_gate;
use super::openrouter::hop_lane_for;
use super::openrouter::key_path;
use super::openrouter::lane_cooked;
use super::openrouter::openrouter_chat;
use super::openrouter::openrouter_key;
use super::openrouter::openrouter_paid_chat;
use super::openrouter::openrouter_quota_cooked;
use super::openrouter::reply_model;
use super::openrouter::unseen_image_line;

pub(crate) const HANDOFF_TOKEN: &str = "[[handoff]]";
pub(crate) const HANDOFF_MARK: &str = "\n[[handoff]]\n";

/// A gate-issued "Ask online agent" offer. Only the gate mints one; the
/// chat trailer carries just its opaque id, never the line or the path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingHandoff {
    pub line: String,
    pub path: String,
}

/// In memory only: an app restart forgets every offer (the button then
/// refuses). Newest last; at most `PENDING_HANDOFF_CAP` open offers.
static PENDING_HANDOFFS: std::sync::Mutex<Vec<(String, PendingHandoff)>> =
    std::sync::Mutex::new(Vec::new());
pub(crate) const PENDING_HANDOFF_CAP: usize = 16;

fn pending_handoffs() -> std::sync::MutexGuard<'static, Vec<(String, PendingHandoff)>> {
    PENDING_HANDOFFS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn is_handoff_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// 128 random bits from /dev/urandom as 32 hex chars. None if urandom
/// can't be read (the offer then shows without a button).
pub(crate) fn new_handoff_id() -> Option<String> {
    use std::io::Read;
    let mut buf = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .ok()?;
    if buf.iter().all(|b| *b == 0) {
        return None;
    }
    Some(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// Store the offer server-side and return its id for the trailer.
pub(crate) fn issue_handoff(line: &str, path: &str) -> Option<String> {
    let id = new_handoff_id()?;
    let mut open = pending_handoffs();
    open.push((
        id.clone(),
        PendingHandoff {
            line: line.to_string(),
            path: path.to_string(),
        },
    ));
    let extra = open.len().saturating_sub(PENDING_HANDOFF_CAP);
    open.drain(..extra);
    Some(id)
}

/// One use per id. Unknown, malformed, evicted or already-used ids refuse.
pub(crate) fn take_handoff(id: &str) -> Result<PendingHandoff, String> {
    let id = id.trim();
    let refused = || "That online-agent offer is unknown or was already used.".to_string();
    if !is_handoff_id(id) {
        return Err(refused());
    }
    let mut open = pending_handoffs();
    let at = open.iter().position(|(k, _)| k == id).ok_or_else(refused)?;
    Ok(open.remove(at).1)
}

pub fn with_handoff_action(visible: &str, reason: &str, id: &str) -> String {
    let payload = json!({ "reason": reason, "id": id });
    format!("{visible}{HANDOFF_MARK}{payload}")
}

/// Remove every handoff marker (and the JSON line right after it) from a
/// model/agent reply, so no reply can mint an "Ask online agent" button.
/// Only the vision gate adds a trailer, after this runs.
pub fn strip_handoff_marks(text: &str) -> String {
    if !text.contains(HANDOFF_TOKEN) {
        return text.to_string();
    }
    let mut out = text.to_string();
    while let Some(i) = out.find(HANDOFF_TOKEN) {
        let before = out[..i].trim_end_matches(['\n', '\r', ' ', '\t']).to_string();
        let mut after = out[i + HANDOFF_TOKEN.len()..].trim_start_matches([' ', '\t']);
        after = after.strip_prefix("\r\n").or_else(|| after.strip_prefix('\n')).unwrap_or(after);
        let (first, rest) = after.split_once('\n').unwrap_or((after, ""));
        let first_t = first.trim();
        let rest = if first_t.starts_with('{') && serde_json::from_str::<Value>(first_t).is_ok() {
            rest
        } else if first_t.starts_with('{') {
            // A JSON trailer spread over lines, or broken JSON: drop to the end.
            ""
        } else {
            after
        };
        out = match (before.is_empty(), rest.trim().is_empty()) {
            (_, true) => before,
            (true, false) => rest.to_string(),
            (false, false) => format!("{before}\n{rest}"),
        };
    }
    out
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub struct HandoffAction {
    pub reason: String,
    pub id: String,
}

/// Same rule as CaptionPane's splitHandoff: a trailer needs a reason and a
/// well-formed id, else no button.
#[cfg(test)]
pub fn split_handoff(text: &str) -> (String, Option<HandoffAction>) {
    let Some((visible, raw)) = text.rsplit_once(HANDOFF_MARK) else {
        return (text.to_string(), None);
    };
    let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
        return (text.to_string(), None);
    };
    let reason = v.get("reason").and_then(Value::as_str).unwrap_or("").to_string();
    let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    if reason.is_empty() || !is_handoff_id(&id) {
        return (visible.to_string(), None);
    }
    (visible.to_string(), Some(HandoffAction { reason, id }))
}

pub fn normalize_reply_lane(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "local" | "lmstudio" | "ollama" => "local".into(),
        "openrouter" | "or" => "openrouter".into(),
        "free" | "freemode" | "free-mode" | "free_mode" | "free mode" => "free".into(),
        "compat" => "compat".into(),
        _ => "grok".into(),
    }
}

pub fn effective_reply_lane(app: &tauri::AppHandle) -> String {
    let Ok(s) = crate::state::load_state(app) else {
        return "grok".into();
    };
    let raw = s.companion.reply_lane.trim();
    if raw.is_empty() {
        if s.companion.free_mode {
            "free".into()
        } else {
            "grok".into()
        }
    } else {
        normalize_reply_lane(raw)
    }
}

/// Walk the free lanes in lock order, hop packet in hand. `notes`
/// collects one short line per lane tried (the honest skip / all-down
/// message names them). Some((lane, reply)) on the first healthy lane.
fn free_lanes_reply(
    app: &tauri::AppHandle,
    raw_line: &str,
    prompt: &str,
    notes: &mut Vec<String>,
    try_local: bool,
    try_compat: bool,
    try_or: bool,
    vision_path: Option<&str>,
) -> Option<(&'static str, String)> {
    let soul = crate::coordinator::companion_soul_text(app);
    let with_tools = crate::mouth_gist::needs_tools_lane(raw_line)
        && !crate::mouth_gist::needs_write_lane(raw_line);
    let include_history = vision_path.is_none();
    let mut messages: Option<Vec<Value>> = None;
    if try_local {
        let want = reply_model(app);
        let msgs = messages.get_or_insert_with(|| {
            hop_messages(app, &soul, raw_line, prompt, with_tools, include_history)
        });
        match local_chat(app, &want, msgs, with_tools, raw_line, vision_path) {
            Ok((lane, r)) if !r.is_empty() => return Some((lane, r)),
            Ok(_) => notes.push("local: empty reply".into()),
            Err(e) => notes.push(format!("local: {e}")),
        }
    }
    if try_compat {
        // Text only. The function can attach an image; Direct does not ask it to.
        if vision_path.is_some() {
            notes.push("compat: text only".into());
        } else {
            let (base, model) = crate::state::load_state(app)
                .map(|s| (s.companion.direct_base_url.clone(), s.companion.direct_model.clone()))
                .unwrap_or_default();
            match crate::direct_lane::read_secret_file(&crate::direct_lane::direct_key_path()) {
                Some(key) => {
                    let msgs = messages.get_or_insert_with(|| {
                        hop_messages(app, &soul, raw_line, prompt, false, include_history)
                    });
                    match compat_chat(&base, &key, &model, msgs, None) {
                        Ok(r) if !r.is_empty() => return Some(("compat", r)),
                        Ok(_) => notes.push("compat: empty reply".into()),
                        Err(e) => notes.push(format!("compat: {e}")),
                    }
                }
                None => notes.push("compat: no key".into()),
            }
        }
    }
    if try_or {
        match openrouter_key() {
            Some(key) => match openrouter_quota_cooked(&key) {
                Ok(true) => notes.push("openrouter: quota spent".into()),
                Ok(false) => {
                    let msgs = messages.get_or_insert_with(|| {
                        hop_messages(app, &soul, raw_line, prompt, false, include_history)
                    });
                    match openrouter_chat(&key, msgs, vision_path) {
                        Ok(r) if !r.is_empty() => return Some(("openrouter", r)),
                        Ok(_) => notes.push("openrouter: empty reply".into()),
                        Err(e) => notes.push(format!("openrouter: {e}")),
                    }
                }
                Err(e) => notes.push(format!("openrouter quota: {e}")),
            },
            None => notes.push("openrouter: no key".into()),
        }
    }
    None
}

/// One honest them-row when no lane answered — the line is already
/// persisted as a you-row, the transcript never resets, and the bubble
/// says why instead of going silent.
pub(crate) fn honest_down_line(free_mode: bool, notes: &[String]) -> String {
    let tried = notes.join("; ");
    if free_mode {
        format!(
            "Free Mode: no free lane is up ({tried}). Paid Grok stays off — no surprise bill. \
             Your line is saved above; try again when a lane is back."
        )
    } else {
        format!(
            "All mouth lanes are down right now ({tried}). Your line is saved above — \
             try again in a bit."
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_free_lanes(
    app: tauri::AppHandle,
    raw_line: String,
    prompt: String,
    skip_grok: bool,
    direct: bool,
    try_local: bool,
    try_compat: bool,
    try_or: bool,
    mut notes: Vec<String>,
    vision_path: Option<String>,
) -> Result<String, String> {
    let hopped = free_lanes_reply(
        &app,
        &raw_line,
        &prompt,
        &mut notes,
        try_local,
        try_compat,
        try_or,
        vision_path.as_deref(),
    );
    match hopped {
        Some((lane, reply)) => {
            let fields = if direct {
                json!({ "lane": lane, "direct": true })
            } else {
                json!({ "lane": lane, "freeMode": skip_grok })
            };
            crate::event_log::append_best(
                &app,
                "companion-hop",
                None,
                None,
                Some("agent"),
                true,
                &fields,
            );
            Ok(reply)
        }
        // Direct never uses the Free Mode sentence (a compat key may be paid).
        None => Ok(honest_down_line(!direct && skip_grok, &notes)),
    }
}

/// The grok-backend send, routed. Reply lane `grok` (default): Grok is
/// the mouth; a cooked lane hops local → OpenRouter free. `free` /
/// `local` / `openrouter` never run paid Grok. The reply lands on the
/// same `turns`; the bubble is never cleared.
pub async fn send_with_hops(
    app: &tauri::AppHandle,
    raw_line: &str,
    prose: &str,
    hop_ack: &str,
    vision_path: Option<&str>,
) -> Result<String, String> {
    let settings = effective_reply_lane(app);
    let has_attach = vision_path.is_some();
    let short = crate::mouth_gist::is_short_chat(raw_line, has_attach);
    let local_up = crate::mouth_scan::pick_local(
        &crate::state::load_state(app)
            .map(|s| s.companion.reply_model)
            .unwrap_or_default(),
    )
    .is_some();
    // Code picks short chat → local when Settings is Grok (default).
    // Explicit Local / Free / OpenRouter still win.
    let hop_lane = hop_lane_for(&settings, raw_line, has_attach, local_up);
    let (oneb, hp) = crate::mouth_sidecar::last_harness()
        .map(|(h, p)| (Some(h), p))
        .unwrap_or((None, 0.0));
    let file = crate::mouth_gist::file_note(prose);
    // A chip's prose already has the path line and any markitdown text.
    // Skip/Job/Tight would drop that; Pocket keeps the prose.
    let harness = if file.is_some() {
        crate::mouth_gist::Harness::Pocket
    } else {
        crate::mouth_gist::pick_harness(app, raw_line, "chat", oneb, hp, short)
    };
    let image_on = vision_path.is_some();
    let gist = match harness {
        crate::mouth_gist::Harness::Skip => None,
        crate::mouth_gist::Harness::Job => {
            crate::mouth_gist::build(app, raw_line, "chat", hop_ack, hop_lane, false, file)
        }
        crate::mouth_gist::Harness::Tight => crate::mouth_gist::build(
            app,
            raw_line,
            "chat",
            hop_ack,
            hop_lane,
            !image_on,
            file,
        ),
        crate::mouth_gist::Harness::Pocket => None,
    };
    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode)
        .unwrap_or_else(|| "spark".into());
    let packed = crate::mouth_gist::pack_prompt(&mode, harness, raw_line, prose, gist.as_deref());
    let n = packed.chars().count();
    crate::event_log::append_best(
        app,
        "companion-gist",
        None,
        None,
        Some("agent"),
        true,
        &json!({
            "lane": hop_lane,
            "short": short,
            "gist": crate::mouth_gist::is_gist(&packed),
            "harness": harness.as_str(),
            "n": n,
        }),
    );
    let skip_grok = settings != "grok" || hop_lane == "local" || hop_lane == "openrouter_free";
    let (try_local, try_or) = match settings.as_str() {
        "local" => (true, false),
        "openrouter" => (false, true),
        "free" | "compat" => (true, true),
        _ if hop_lane == "local" => (true, false),
        _ => (true, true),
    };
    if skip_grok && hop_lane != "grok" {
        let (a, r, p) = (app.clone(), raw_line.to_string(), packed);
        let tl = try_local;
        let to = try_or;
        let vision = vision_path.map(str::to_string);
        return tauri::async_runtime::spawn_blocking(move || {
            run_free_lanes(a, r, p, true, false, tl, false, to, Vec::new(), vision)
        })
        .await
        .map_err(|e| format!("hop task: {e}"))?;
    }
    let grok_prompt = if crate::mouth_gist::is_gist(&packed) {
        crate::mouth_gist::grok_user_prompt(&packed)
    } else {
        packed.clone()
    };
    let vision = vision_path.map(str::to_string);
    match crate::coordinator::companion_send_grok(app, &grok_prompt, vision.clone()).await {
        Ok(r) => Ok(r),
        Err(e) if lane_cooked(&e) => {
            let note = format!("grok: {}", e.chars().take(80).collect::<String>());
            let (a, r, p) = (app.clone(), raw_line.to_string(), packed);
            let vision = vision.clone();
            tauri::async_runtime::spawn_blocking(move || {
                run_free_lanes(a, r, p, false, false, true, false, true, vec![note], vision)
            })
            .await
            .map_err(|e| format!("hop task: {e}"))?
        }
        Err(e) => Err(e),
    }
}

// Settings surface (Free Mode toggle + BYOK key)

/// Free Mode toggle — also writes reply_lane so the Mouth Reply row
/// and this door stay one switch. Default off (paid Grok is today's mouth).
#[tauri::command]
pub fn set_companion_free_mode(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.companion.free_mode = enabled;
        st.companion.reply_lane = if enabled {
            "free".into()
        } else if st.companion.reply_lane.trim().eq_ignore_ascii_case("free") {
            "grok".into()
        } else {
            st.companion.reply_lane.clone()
        };
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_companion_reply_lane(app: tauri::AppHandle, lane: String) -> Result<String, String> {
    let lane = normalize_reply_lane(&lane);
    crate::state::update(&app, |st| {
        st.companion.reply_lane = lane.clone();
        st.companion.free_mode = lane == "free";
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(lane)
}

#[tauri::command]
pub fn set_companion_reply_model(app: tauri::AppHandle, model: String) -> Result<String, String> {
    let model = model.trim().chars().take(160).collect::<String>();
    crate::state::update(&app, |st| {
        st.companion.reply_model = model.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(model)
}

pub fn normalize_handoff_backend(raw: &str) -> Result<String, String> {
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "" => Ok(String::new()),
        "grok" | "goose" | "hermes" | "openrouter" => Ok(s),
        _ => Err("Unknown handoff agent.".into()),
    }
}

#[tauri::command]
pub fn set_companion_handoff_backend(
    app: tauri::AppHandle,
    backend: String,
) -> Result<String, String> {
    let backend = normalize_handoff_backend(&backend)?;
    crate::state::update(&app, |st| {
        st.companion.handoff_backend = backend.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(backend)
}

#[tauri::command]
pub fn set_companion_auto_handoff(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.companion.auto_handoff = enabled;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

fn handoff_settings(app: &tauri::AppHandle) -> (bool, String) {
    crate::state::load_state(app)
        .map(|s| (s.companion.auto_handoff, s.companion.handoff_backend))
        .unwrap_or((false, String::new()))
}

fn log_handoff(app: &tauri::AppHandle, from: &str, to: &str, reason: &str) {
    crate::event_log::append_best(
        app,
        "companion-handoff",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "from": from, "to": to, "reason": reason }),
    );
}

pub(crate) fn persist_them(app: &tauri::AppHandle, reply: &str) {
    if crate::state::update(app, |st| {
        st.companion.last_bot_line = reply.to_string();
        st.companion.push_turn("them", reply);
        Ok(())
    })
    .is_ok()
    {
        let _ = app.emit("state:changed", ());
    }
}

/// Handoff targets that get the image itself: Grok (--prompt-json) and
/// OpenRouter (paid vision model). Goose and Hermes get the path + note.
pub(crate) fn handoff_can_see(backend: &str, image: bool) -> bool {
    image && matches!(backend, "grok" | "openrouter")
}

/// Send the same line and attachment through one configured online agent.
/// Does not go back through the free-lane gate.
pub async fn dispatch_handoff(
    app: &tauri::AppHandle,
    backend: &str,
    line: &str,
    path: Option<&str>,
) -> Result<String, String> {
    let backend = normalize_handoff_backend(backend)?;
    if backend.is_empty() {
        return Err("pick an online agent in Mouth settings".into());
    }
    let info = match path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(crate::attach::stat_path(p)?),
        None => None,
    };
    let pocket = crate::coordinator::wrap_with_pocket(app, line);
    let image = info
        .as_ref()
        .is_some_and(|a| crate::attach_vision::is_image(&a.path));
    let can_see = handoff_can_see(&backend, image);
    let prep = match &info {
        Some(a) => crate::attach_send::prepare(a, &pocket, &backend, can_see),
        None => crate::attach_send::Prepared {
            prompt: pocket,
            vision_path: None,
        },
    };
    match backend.as_str() {
        "grok" => {
            crate::coordinator::companion_send_grok(app, &prep.prompt, prep.vision_path).await
        }
        "goose" => crate::coordinator::companion_send_goose(app, &prep.prompt).await,
        "hermes" => crate::coordinator::companion_send_hermes(app, &prep.prompt).await,
        "openrouter" => {
            let key = openrouter_key().ok_or_else(|| "OpenRouter key is not set.".to_string())?;
            let soul = crate::coordinator::companion_soul_text(app);
            let prompt = prep.prompt;
            let vision = prep.vision_path;
            tauri::async_runtime::spawn_blocking(move || {
                let mut messages = vec![
                    json!({ "role": "system", "content": soul }),
                    json!({ "role": "user", "content": prompt }),
                ];
                if let Some(p) = vision.as_deref() {
                    messages = crate::attach_vision::with_vision_parts(
                        &messages,
                        p,
                        crate::attach_vision::VisionWire::OpenAi,
                    )?;
                }
                openrouter_paid_chat(&key, &messages)
            })
            .await
            .map_err(|e| format!("handoff task: {e}"))?
        }
        _ => Err("Unknown handoff agent.".into()),
    }
}

/// Image the resolved lane can't see. Offer the button, or hand off when
/// auto-handoff is on and a backend is set. Does not call the local model.
pub async fn refuse_or_handoff(
    app: &tauri::AppHandle,
    line: &str,
    path: &str,
    from_lane: &str,
) -> Result<String, String> {
    let (auto, backend) = handoff_settings(app);
    match decide_vision_gate(true, false, auto, &backend) {
        VisionGate::Proceed => Ok(unseen_image_line().to_string()),
        VisionGate::Offer { text } => Ok(match issue_handoff(line, path) {
            Some(id) => with_handoff_action(&text, "no-vision", &id),
            None => text,
        }),
        VisionGate::Auto { backend, prefix } => {
            log_handoff(app, from_lane, &backend, "no-vision");
            let answer = dispatch_handoff(app, &backend, line, Some(path)).await?;
            let answer = strip_handoff_marks(&answer);
            Ok(format!("{prefix}\n\n{answer}"))
        }
    }
}

/// Button: resend this turn through the configured online agent.
/// Takes only the gate-issued offer id; the line and path come from the
/// server-side store, once. Unknown or reused ids refuse.
/// The reply lands on the same transcript. Logs lane names only.
#[tauri::command]
pub async fn companion_handoff(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let backend = crate::state::load_state(&app)
        .map(|s| s.companion.handoff_backend)
        .unwrap_or_default();
    if backend.trim().is_empty() {
        return Err("pick an online agent in Mouth settings".into());
    }
    let offer = take_handoff(&id)?;
    let from = effective_reply_lane(&app);
    log_handoff(&app, &from, backend.trim(), "no-vision");
    let path = Some(offer.path.as_str()).filter(|p| !p.trim().is_empty());
    let answer = dispatch_handoff(&app, backend.trim(), offer.line.trim(), path).await?;
    let answer = strip_handoff_marks(&answer);
    persist_them(&app, &answer);
    Ok(answer)
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterKeyStatus {
    pub set: bool,
    /// "env" | "file" — where the effective key comes from.
    pub source: String,
    /// Last 4 chars, so Settings can confirm WHICH key without ever
    /// reading it back.
    pub tail: String,
}

fn key_status() -> OpenRouterKeyStatus {
    let env = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let env = env.trim();
    if !env.is_empty() {
        return OpenRouterKeyStatus {
            set: true,
            source: "env".into(),
            tail: env.chars().rev().take(4).collect::<String>().chars().rev().collect(),
        };
    }
    match std::fs::read_to_string(key_path()) {
        Ok(k) if !k.trim().is_empty() => OpenRouterKeyStatus {
            set: true,
            source: "file".into(),
            tail: k.trim().chars().rev().take(4).collect::<String>().chars().rev().collect(),
        },
        _ => OpenRouterKeyStatus {
            set: false,
            source: String::new(),
            tail: String::new(),
        },
    }
}

/// Settings paints this; the key itself never leaves the native side.
#[tauri::command]
pub fn openrouter_key_status() -> OpenRouterKeyStatus {
    key_status()
}

/// Save (or clear, on empty) the BYOK OpenRouter key — 0600 under
/// companions/orbitty-primary/, never state.json. Whitespace/newlines
/// are rejected (a pasted blob is never a key).
#[tauri::command]
pub fn set_openrouter_key(key: String) -> Result<OpenRouterKeyStatus, String> {
    let key = key.trim().to_string();
    let path = key_path();
    if key.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        }
        return Ok(key_status());
    }
    if key.chars().any(char::is_whitespace) {
        return Err("That doesn't look like a key — no spaces or newlines.".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, format!("{key}\n")).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key_status())
}

