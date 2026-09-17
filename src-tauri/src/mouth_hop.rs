//! Spark mouth lane hop. One bubble; never wipe `companion.turns`.
//! Lanes: local (LM Studio `:1234` / Ollama) → OpenRouter `:free`
//! (never `openrouter/auto`) → paid Grok. Key never in state.json.
//! Bearer never on argv.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tauri::Emitter;

/// Hop-packet history budget — same shape as mouth_roll's handoff:
/// last ~6 turns, ~1200 chars, ack-noise rows skipped.
const HOP_TURNS: usize = 6;
const HOP_HISTORY_CAP: usize = 1200;

/// pub(crate): mouth_ack's conductor rides the same local probe/chat.
pub(crate) const OLLAMA_BASE: &str = "http://localhost:11434";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

/// The one default free-lane model. `openrouter/auto` (any flavor) is
/// forbidden by the lock — the free lane is explicit.
const OPENROUTER_FREE_MODEL: &str = "openrouter/free";

// hop packet

/// Soul (system) + the last few turns of the ACTIVE thread + the
/// wrapped line, as chat messages for a stateless lane. The just-saved
/// you-row (== raw_line) is dropped from history so the line lands once
/// — as the final user message. Compact: newest-first budget fill, same
/// skip list as mouth_roll's handoff (ack noise never rides the packet).
fn hop_messages(
    app: &tauri::AppHandle,
    soul: &str,
    raw_line: &str,
    prompt: &str,
    with_tools: bool,
) -> Vec<Value> {
    if crate::mouth_gist::is_gist(prompt) {
        let mut sys = crate::mouth_gist::hop_system(soul);
        if with_tools {
            sys.push_str(
                "\nIf you need the board, call kanban_list. Then answer in English.",
            );
        }
        return vec![
            json!({ "role": "system", "content": sys }),
            json!({ "role": "user", "content": prompt }),
        ];
    }
    let mut messages = vec![json!({ "role": "system", "content": soul })];
    let turns = crate::state::load_state(app)
        .map(|s| s.companion.turns)
        .unwrap_or_default();
    let start = turns.len().saturating_sub(HOP_TURNS + 1);
    let mut rows: Vec<&crate::state::CompanionTurn> = turns[start..]
        .iter()
        .filter(|t| {
            let low = t.text.to_ascii_lowercase();
            !(low.starts_with("noted ")
                || low.starts_with("heard you")
                || low.contains("companion stays the mouth")
                || (low.starts_with("dogfood ") && low.contains("is in needed")))
        })
        .collect();
    // The send path persists the you-row BEFORE the mouth runs — the
    // wrapped prompt at the end IS that line; never twice.
    if rows
        .last()
        .is_some_and(|t| t.role == "you" && t.text == raw_line)
    {
        rows.pop();
    }
    // Newest-first budget fill, then flip back to chronological.
    let mut kept: Vec<&crate::state::CompanionTurn> = Vec::new();
    let mut used = 0usize;
    for t in rows.iter().rev() {
        let n = t.text.chars().count();
        if !kept.is_empty() && used + n > HOP_HISTORY_CAP {
            break;
        }
        used += n;
        kept.push(t);
        if kept.len() >= HOP_TURNS {
            break;
        }
    }
    kept.reverse();
    for t in kept {
        let role = if t.role == "you" { "user" } else { "assistant" };
        messages.push(json!({ "role": role, "content": t.text.replace('\n', " ") }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));
    messages
}

// curl (the crate's only HTTP — no client dep on purpose)

fn curl_bin() -> PathBuf {
    let sys = PathBuf::from("/usr/bin/curl");
    if sys.is_file() {
        return sys;
    }
    crate::coordinator::find_on_path("curl").unwrap_or(sys)
}

/// 0600 temp file, unlinked on drop — prompt bodies and the bearer
/// config never sit world-readable, never ride argv.
struct TmpFile(PathBuf);

impl TmpFile {
    fn write(suffix: &str, body: &str) -> Result<TmpFile, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!("orbitty-hop-{}-{nanos}{suffix}", std::process::id()));
        std::fs::write(&p, body).map_err(|e| format!("write {}: {e}", p.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(TmpFile(p))
    }
}

impl Drop for TmpFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One curl call, JSON in/out. Returns (HTTP status, body). A transport
/// failure (curl exit ≠ 0 — refused, timed out, DNS) is the Err; an
/// HTTP error status is NOT an Err — the lane reads the code + body.
/// pub(crate): mouth_ack's conductor reuses the one curl door (the
/// 0600-key discipline carries over).
pub(crate) fn curl_json(
    method: &str,
    url: &str,
    bearer: Option<&str>,
    body: Option<&Value>,
    timeout_secs: u64,
) -> Result<(u16, String), String> {
    let body_file = match body {
        Some(b) => Some(TmpFile::write(".json", &b.to_string())?),
        None => None,
    };
    // The key rides a config file, never argv (`ps` would show it).
    let cfg_file = match bearer {
        Some(k) => Some(TmpFile::write(
            ".cfg",
            &format!("header = \"Authorization: Bearer {k}\"\n"),
        )?),
        None => None,
    };
    let mut args: Vec<String> = vec![
        "-sS".into(),
        "--max-time".into(),
        timeout_secs.to_string(),
        "-X".into(),
        method.into(),
    ];
    if let Some(c) = &cfg_file {
        args.push("-K".into());
        args.push(c.0.display().to_string());
    }
    if let Some(b) = &body_file {
        // --json sets Content-Type + reads the body from the file.
        args.push("--json".into());
        args.push(format!("@{}", b.0.display()));
    }
    args.push("-w".into());
    args.push("\n%{http_code}".into());
    args.push(url.into());
    let out = Command::new(curl_bin())
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("launch curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        return Err(if line.is_empty() {
            format!("curl exit {}", out.status.code().unwrap_or(-1))
        } else {
            // curl prefixes with "curl: (7) " — keep the human part short.
            line.trim_start_matches("curl: ").chars().take(80).collect()
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (body_text, status) = match text.rfind('\n') {
        Some(i) => {
            let code = text[i + 1..].trim().parse::<u16>().unwrap_or(0);
            (&text[..i], code)
        }
        None => (&text[..], 0),
    };
    Ok((status, body_text.to_string()))
}

/// Pull one short error message out of a JSON error body
/// ({"error":"…"} or {"error":{"message":"…"}}); falls back to the
/// raw body capped — the bubble note stays one line.
fn json_error_note(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = v
        .get("error")
        .and_then(|e| {
            e.as_str()
                .map(str::to_string)
                .or_else(|| e.get("message").and_then(Value::as_str).map(str::to_string))
        })
        .unwrap_or_else(|| body.replace('\n', " "));
    msg.trim().chars().take(80).collect()
}

// lane 1: local Ollama

/// Up = the HTTP server answers /api/version. The `ollama` bin is not
/// required — Orbitty talks HTTP, never shells the model.
/// pub(crate): mouth_ack probes through here (behind its own 15s cache).
pub(crate) fn ollama_up() -> bool {
    matches!(
        curl_json("GET", &format!("{OLLAMA_BASE}/api/version"), None, None, 2),
        Ok((200..=299, _))
    )
}

/// ORBITTY_OLLAMA_MODEL wins when set; else the first pulled model.
/// pub(crate): the conductor's blank setting resolves here too.
pub(crate) fn ollama_model() -> Result<String, String> {
    let env = std::env::var("ORBITTY_OLLAMA_MODEL").unwrap_or_default();
    if !env.trim().is_empty() {
        return Ok(env.trim().to_string());
    }
    let (status, body) =
        curl_json("GET", &format!("{OLLAMA_BASE}/api/tags"), None, None, 3)?;
    if !(200..=299).contains(&status) {
        return Err(format!("tags http {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("tags json: {e}"))?;
    v.get("models")
        .and_then(Value::as_array)
        .and_then(|ms| ms.first())
        .and_then(|m| m.get("name").or_else(|| m.get("model")))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "no models pulled".to_string())
}

fn openai_chat_content(text: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("chat json: {e}"))?;
    Ok(v.get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string())
}

fn openai_message(text: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .cloned()
}

fn lmstudio_chat(
    app: &tauri::AppHandle,
    chat_url: &str,
    model: &str,
    messages: &[Value],
    with_tools: bool,
) -> Result<String, String> {
    let mut msgs = messages.to_vec();
    let mut tools_on = with_tools;
    for round in 0..3 {
        let mut body = json!({
            "model": model,
            "messages": msgs,
            "stream": false,
        });
        if tools_on {
            body["tools"] = crate::mouth_tools::schemas();
            body["tool_choice"] = json!("auto");
        }
        let (status, text) = curl_json("POST", chat_url, None, Some(&body), 120)?;
        if tools_on && !(200..=299).contains(&status) {
            tools_on = false;
            continue;
        }
        if !(200..=299).contains(&status) {
            return Err(format!("http {status}: {}", json_error_note(&text)));
        }
        let Some(msg) = openai_message(&text) else {
            return openai_chat_content(&text);
        };
        let calls = msg
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if calls.is_empty() {
            let content = msg
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if tools_on && round == 0 {
                let list = crate::mouth_tools::run(
                    app,
                    "kanban_list",
                    &json!({ "project": "orbitty", "status": "open" }),
                );
                if !content.is_empty() {
                    msgs.push(msg);
                }
                msgs.push(json!({
                    "role": "user",
                    "content": format!("Board snapshot:\n{list}\nAnswer the ask in English. Do not echo JSON."),
                }));
                tools_on = false;
                continue;
            }
            if !content.is_empty() {
                return Ok(content);
            }
            return openai_chat_content(&text);
        }
        msgs.push(msg);
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("call")
                .to_string();
            let name = call
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let args_raw = call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let args: Value = serde_json::from_str(args_raw).unwrap_or(json!({}));
            let out = crate::mouth_tools::run(app, name, &args);
            msgs.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": out,
            }));
        }
    }
    Err("tool loop".into())
}

fn ollama_chat_named(model: &str, messages: &[Value]) -> Result<String, String> {
    let body = json!({ "model": model, "messages": messages, "stream": false });
    let (status, text) = curl_json(
        "POST",
        &format!("{OLLAMA_BASE}/api/chat"),
        None,
        Some(&body),
        120,
    )?;
    if !(200..=299).contains(&status) {
        return Err(format!("http {status}: {}", json_error_note(&text)));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("chat json: {e}"))?;
    Ok(v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string())
}

#[allow(dead_code)]
fn ollama_chat(messages: &[Value]) -> Result<String, String> {
    let model = ollama_model()?;
    ollama_chat_named(&model, messages)
}

/// Local reply: chosen Reply id on LM Studio, else Ollama. `want` is
/// `companion.reply_model` (empty = prefer E4B).
fn local_chat(
    app: &tauri::AppHandle,
    want: &str,
    messages: &[Value],
    with_tools: bool,
) -> Result<(&'static str, String), String> {
    match crate::mouth_scan::pick_local(want) {
        Some(crate::mouth_scan::LocalTarget::LmStudio { chat_url, model }) => {
            lmstudio_chat(app, &chat_url, &model, messages, with_tools).map(|r| ("lmstudio", r))
        }
        Some(crate::mouth_scan::LocalTarget::Ollama { model }) => {
            ollama_chat_named(&model, messages).map(|r| ("ollama", r))
        }
        None => Err("none found — open Ollama or LM Studio.".into()),
    }
}

// lane 2: OpenRouter free (BYOK, failover — never identity)

fn key_path() -> PathBuf {
    crate::coordinator::companions_root().join("openrouter.key")
}

/// Env first (standard OPENROUTER_API_KEY), then the Settings-saved
/// file. Trimmed; empty = no key.
fn openrouter_key() -> Option<String> {
    let env = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if !env.trim().is_empty() {
        return Some(env.trim().to_string());
    }
    std::fs::read_to_string(key_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// The free lane is explicit: `openrouter/free`, or an
/// ORBITTY_OPENROUTER_MODEL override ONLY when it names an explicit
/// `:free` id. `openrouter/auto` (any flavor) never passes.
fn openrouter_model() -> String {
    let env = std::env::var("ORBITTY_OPENROUTER_MODEL").unwrap_or_default();
    let m = env.trim();
    if m == OPENROUTER_FREE_MODEL || m.ends_with(":free") {
        m.to_string()
    } else {
        OPENROUTER_FREE_MODEL.into()
    }
}

/// GET quota before spending a request (lock #4). Cooked = a known
/// limit is spent. Unknown shape / free tier = try the lane — the chat
/// call's own 402/429 still cooks it. A failed GET is an honest Err
/// note, and the lane is skipped (the chat call would fail the same).
fn openrouter_quota_cooked(key: &str) -> Result<bool, String> {
    let (status, body) = curl_json(
        "GET",
        &format!("{OPENROUTER_BASE}/auth/key"),
        Some(key),
        None,
        10,
    )?;
    if !(200..=299).contains(&status) {
        return Err(format!("http {status}: {}", json_error_note(&body)));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("quota json: {e}"))?;
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    if let Some(remaining) = data.get("limit_remaining").and_then(Value::as_f64) {
        return Ok(remaining <= 0.0);
    }
    match (
        data.get("limit").and_then(Value::as_f64),
        data.get("usage").and_then(Value::as_f64),
    ) {
        (Some(limit), Some(usage)) => Ok(usage >= limit),
        // No cap on record (free tier / unlimited) = not cooked.
        _ => Ok(false),
    }
}

fn openrouter_chat(key: &str, messages: &[Value]) -> Result<String, String> {
    let body = json!({ "model": openrouter_model(), "messages": messages });
    let (status, text) = curl_json(
        "POST",
        &format!("{OPENROUTER_BASE}/chat/completions"),
        Some(key),
        Some(&body),
        90,
    )?;
    if !(200..=299).contains(&status) {
        return Err(format!("http {status}: {}", json_error_note(&text)));
    }
    openai_chat_content(&text)
}

// the router

/// True when a mouth failure says the LANE is cooked — rate limit
/// (429), payment/quota (402), or a session wall — so a fresh Grok
/// session would fail the same way and the right move is to hop lanes.
/// Consulted ONLY on the error that escapes the Grok path (mouth_roll
/// already rolled a dead session); a slow-generate timeout never
/// matches — it stays an honest Err.
pub fn lane_cooked(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("429")
        || e.contains("rate limit")
        || e.contains("too many requests")
        || e.contains("402")
        || e.contains("payment")
        || e.contains("billing")
        || e.contains("credits")
        || e.contains("quota")
        || e.contains("session limit")
        || e.contains("session cap")
        || e.contains("too many sessions")
        || e.contains("session wall")
}

fn reply_model(app: &tauri::AppHandle) -> String {
    crate::state::load_state(app)
        .map(|s| s.companion.reply_model)
        .unwrap_or_default()
}

pub fn normalize_reply_lane(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "local" | "lmstudio" | "ollama" => "local".into(),
        "openrouter" | "or" => "openrouter".into(),
        "free" | "freemode" | "free-mode" | "free_mode" | "free mode" => "free".into(),
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
    try_or: bool,
) -> Option<(&'static str, String)> {
    let soul = crate::coordinator::companion_soul_text(app);
    let with_tools = crate::mouth_gist::needs_tools_lane(raw_line)
        && !crate::mouth_gist::needs_write_lane(raw_line);
    let mut messages: Option<Vec<Value>> = None;
    if try_local {
        let want = reply_model(app);
        let msgs = messages
            .get_or_insert_with(|| hop_messages(app, &soul, raw_line, prompt, with_tools));
        match local_chat(app, &want, msgs, with_tools) {
            Ok((lane, r)) if !r.is_empty() => return Some((lane, r)),
            Ok(_) => notes.push("local: empty reply".into()),
            Err(e) => notes.push(format!("local: {e}")),
        }
    }
    if try_or {
        match openrouter_key() {
            Some(key) => match openrouter_quota_cooked(&key) {
                Ok(true) => notes.push("openrouter: quota spent".into()),
                Ok(false) => {
                    let msgs = messages.get_or_insert_with(|| {
                        hop_messages(app, &soul, raw_line, prompt, false)
                    });
                    match openrouter_chat(&key, msgs) {
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
fn honest_down_line(free_mode: bool, notes: &[String]) -> String {
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

fn run_free_lanes(
    app: tauri::AppHandle,
    raw_line: String,
    prompt: String,
    skip_grok: bool,
    try_local: bool,
    try_or: bool,
    mut notes: Vec<String>,
) -> Result<String, String> {
    let hopped = free_lanes_reply(&app, &raw_line, &prompt, &mut notes, try_local, try_or);
    match hopped {
        Some((lane, reply)) => {
            crate::event_log::append_best(
                &app,
                "companion-hop",
                None,
                None,
                Some("agent"),
                true,
                &json!({ "lane": lane, "freeMode": skip_grok }),
            );
            Ok(reply)
        }
        None => Ok(honest_down_line(skip_grok, &notes)),
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
    let hop_lane = match settings.as_str() {
        "local" => "local",
        "openrouter" => "openrouter_free",
        "free" => {
            if local_up {
                "local"
            } else {
                "openrouter_free"
            }
        }
        _ if crate::mouth_gist::needs_write_lane(raw_line) => "grok",
        _ if short && local_up => "local",
        _ => "grok",
    };
    let gist = crate::mouth_gist::build(app, raw_line, "chat", hop_ack, hop_lane);
    let packed = gist.clone().unwrap_or_else(|| prose.to_string());
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
            "gist": gist.is_some(),
            "n": n,
        }),
    );
    let skip_grok = settings != "grok" || hop_lane == "local" || hop_lane == "openrouter_free";
    let (try_local, try_or) = match settings.as_str() {
        "local" => (true, false),
        "openrouter" => (false, true),
        "free" => (true, true),
        _ if hop_lane == "local" => (true, false),
        _ => (true, true),
    };
    if skip_grok && hop_lane != "grok" {
        let (a, r, p) = (app.clone(), raw_line.to_string(), packed);
        let tl = try_local;
        let to = try_or;
        return tauri::async_runtime::spawn_blocking(move || {
            run_free_lanes(a, r, p, true, tl, to, Vec::new())
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
    match crate::coordinator::companion_send_grok(app, &grok_prompt, vision).await {
        Ok(r) => Ok(r),
        Err(e) if lane_cooked(&e) => {
            let note = format!("grok: {}", e.chars().take(80).collect::<String>());
            let (a, r, p) = (app.clone(), raw_line.to_string(), packed);
            tauri::async_runtime::spawn_blocking(move || {
                run_free_lanes(a, r, p, false, true, true, vec![note])
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
    let mut st = crate::state::load_state(&app)?;
    st.companion.free_mode = enabled;
    st.companion.reply_lane = if enabled {
        "free".into()
    } else if st.companion.reply_lane.trim().eq_ignore_ascii_case("free") {
        "grok".into()
    } else {
        st.companion.reply_lane.clone()
    };
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_companion_reply_lane(app: tauri::AppHandle, lane: String) -> Result<String, String> {
    let lane = normalize_reply_lane(&lane);
    let mut st = crate::state::load_state(&app)?;
    st.companion.reply_lane = lane.clone();
    st.companion.free_mode = lane == "free";
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(lane)
}

#[tauri::command]
pub fn set_companion_reply_model(app: tauri::AppHandle, model: String) -> Result<String, String> {
    let model = model.trim().chars().take(160).collect::<String>();
    let mut st = crate::state::load_state(&app)?;
    st.companion.reply_model = model.clone();
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(model)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_lane_names() {
        assert_eq!(normalize_reply_lane("grok"), "grok");
        assert_eq!(normalize_reply_lane("local"), "local");
        assert_eq!(normalize_reply_lane("lmstudio"), "local");
        assert_eq!(normalize_reply_lane("openrouter"), "openrouter");
        assert_eq!(normalize_reply_lane("free"), "free");
        assert_eq!(normalize_reply_lane("Free Mode"), "free");
        assert_eq!(normalize_reply_lane(""), "grok");
        assert_eq!(normalize_reply_lane("paid"), "grok");
    }
}
