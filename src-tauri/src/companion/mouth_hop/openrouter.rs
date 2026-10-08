use std::path::PathBuf;

use serde_json::{json, Value};

use super::handoff::effective_reply_lane;
use super::local::OPENROUTER_BASE;
use super::local::OPENROUTER_FREE_MODEL;
use super::local::apply_vision_or_honest;
use super::local::curl_json;
use super::local::json_error_note;
use super::local::openai_chat_content;

// lane 2: OpenRouter free (BYOK, failover — never identity)

pub(crate) fn key_path() -> PathBuf {
    crate::coordinator::companions_root().join("openrouter.key")
}

/// Env first (standard OPENROUTER_API_KEY), then the Settings-saved
/// file. Trimmed; empty = no key.
pub(crate) fn openrouter_key() -> Option<String> {
    let env = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if !env.trim().is_empty() {
        return Some(env.trim().to_string());
    }
    std::fs::read_to_string(key_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) fn openrouter_configured() -> bool {
    openrouter_key().is_some()
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
pub(crate) fn openrouter_quota_cooked(key: &str) -> Result<bool, String> {
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

pub(crate) fn openrouter_chat(
    key: &str,
    messages: &[Value],
    vision_path: Option<&str>,
) -> Result<String, String> {
    let messages = match apply_vision_or_honest(
        messages,
        vision_path,
        crate::attach_vision::VisionWire::OpenAi,
    ) {
        Ok(m) => m,
        Err(e) if e == crate::attach_vision::oversize_line() => return Ok(e),
        Err(e) => return Err(e),
    };
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

/// OpenAI-compatible `{base}/chat/completions`. Base is validated before
/// any curl. Vision parts use the OpenAI wire; Direct passes no image
/// because the compat lane is text-only.
pub(crate) fn compat_chat(
    base: &str,
    key: &str,
    model: &str,
    messages: &[Value],
    vision_path: Option<&str>,
) -> Result<String, String> {
    let base = crate::direct_lane::validate_direct_base(base)?;
    if base.is_empty() {
        return Err("no base url".into());
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("no model".into());
    }
    let messages = match apply_vision_or_honest(
        messages,
        vision_path,
        crate::attach_vision::VisionWire::OpenAi,
    ) {
        Ok(m) => m,
        Err(e) if e == crate::attach_vision::oversize_line() => return Ok(e),
        Err(e) => return Err(e),
    };
    let body = json!({ "model": model, "messages": messages });
    let url = format!("{base}/chat/completions");
    let (status, text) = curl_json("POST", &url, Some(key), Some(&body), 90)?;
    if !(200..=299).contains(&status) {
        return Err(format!("http {status}: {}", json_error_note(&text)));
    }
    openai_chat_content(&text)
}

/// Paid/BYOK handoff model. Never the free lane and never `openrouter/auto`.
pub(crate) const OPENROUTER_HANDOFF_MODEL: &str = "openai/gpt-4o-mini";

pub fn filter_handoff_model(raw: &str) -> String {
    let m = raw.trim();
    if m.is_empty()
        || m == OPENROUTER_FREE_MODEL
        || m.contains("openrouter/auto")
        || m.ends_with(":free")
    {
        OPENROUTER_HANDOFF_MODEL.into()
    } else {
        m.to_string()
    }
}

fn openrouter_handoff_model() -> String {
    filter_handoff_model(&std::env::var("ORBITTY_OPENROUTER_HANDOFF_MODEL").unwrap_or_default())
}

pub(crate) fn openrouter_paid_chat(key: &str, messages: &[Value]) -> Result<String, String> {
    let body = json!({ "model": openrouter_handoff_model(), "messages": messages });
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

pub(crate) fn reply_model(app: &tauri::AppHandle) -> String {
    crate::state::load_state(app)
        .map(|s| s.companion.reply_model)
        .unwrap_or_default()
}

pub fn hop_lane_for(settings: &str, line: &str, has_attach: bool, local_up: bool) -> &'static str {
    let short = crate::mouth_gist::is_short_chat(line, has_attach);
    match settings {
        "local" => "local",
        "openrouter" => "openrouter_free",
        // compat on the grok harness is Free Mode: never the paid lane,
        // and never this file's compat HTTP call (that belongs to Direct).
        "free" | "compat" => {
            if local_up {
                "local"
            } else {
                "openrouter_free"
            }
        }
        _ if crate::mouth_gist::needs_write_lane(line) => "grok",
        _ if short && local_up => "local",
        _ => "grok",
    }
}

/// Lane name plus whether that lane's model can see an image.
/// Grok's CLI can. Local uses the scan. OpenRouter uses input modalities.
pub fn image_lane_vision(app: &tauri::AppHandle, line: &str, has_attach: bool) -> (String, bool) {
    let settings = effective_reply_lane(app);
    let want = reply_model(app);
    let (scanned, _, _) = crate::mouth_scan::scan_running();
    let local_up = crate::mouth_scan::resolve_local(&want, &scanned).is_some();
    let hop = hop_lane_for(&settings, line, has_attach, local_up);
    let vision = match hop {
        "grok" => true,
        "local" => crate::mouth_scan::scanned_model_vision(&want, &scanned),
        "openrouter_free" => openrouter_free_can_see(),
        _ => false,
    };
    (hop.to_string(), vision)
}

fn openrouter_free_can_see() -> bool {
    let Some(key) = openrouter_key() else {
        return false;
    };
    let model = openrouter_model();
    let url = format!("{OPENROUTER_BASE}/models");
    match curl_json("GET", &url, Some(&key), None, 8) {
        Ok((status, body)) if (200..=299).contains(&status) => {
            crate::mouth_scan::openrouter_model_has_image(&body, &model)
        }
        _ => false,
    }
}

pub fn unseen_image_line() -> &'static str {
    "This local model can't see images. Ask online agent?"
}

pub fn handoff_agent_label(id: &str) -> &'static str {
    match id.trim().to_ascii_lowercase().as_str() {
        "grok" => "Grok",
        "goose" => "Goose",
        "hermes" => "Hermes",
        "openrouter" => "OpenRouter",
        _ => "online agent",
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VisionGate {
    Proceed,
    Offer { text: String },
    Auto { backend: String, prefix: String },
}

/// Image on a lane that can't see: never call the model. Auto-handoff
/// only when the toggle is on AND a backend is set. Otherwise the chat
/// shows the button. Paid Grok is not chosen here unless auto names it.
pub fn decide_vision_gate(needs_image: bool, can_see: bool, auto: bool, backend: &str) -> VisionGate {
    if !needs_image || can_see {
        return VisionGate::Proceed;
    }
    let backend = backend.trim();
    if auto && !backend.is_empty() {
        let label = handoff_agent_label(backend);
        VisionGate::Auto {
            backend: backend.to_string(),
            prefix: format!("Asked {label} (local model can't see images)"),
        }
    } else if auto {
        VisionGate::Offer {
            text: format!(
                "{}\n\npick an online agent in Mouth settings",
                unseen_image_line()
            ),
        }
    } else {
        VisionGate::Offer {
            text: unseen_image_line().to_string(),
        }
    }
}

