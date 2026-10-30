//! Spark mouth lane hop. One bubble; never wipe `companion.turns`.
//! Lanes: local (LM Studio `:1234` / Ollama) → OpenRouter `:free`
//! (never `openrouter/auto`) → paid Grok. Key never in state.json.
//! Bearer never on argv.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use super::handoff::strip_handoff_marks;
use super::openrouter::unseen_image_line;

/// Hop-packet history budget — same shape as mouth_roll's handoff:
/// last ~6 turns, ~1200 chars, ack-noise rows skipped.
const HOP_TURNS: usize = 6;
const HOP_HISTORY_CAP: usize = 1200;

/// pub(crate): mouth_ack's conductor rides the same local probe/chat.
pub(crate) const OLLAMA_BASE: &str = "http://localhost:11434";
pub(crate) const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

/// The one default free-lane model. `openrouter/auto` (any flavor) is
/// forbidden by the lock — the free lane is explicit.
pub(crate) const OPENROUTER_FREE_MODEL: &str = "openrouter/free";

// hop packet

/// Soul (system) + the last few turns of the ACTIVE thread + the
/// wrapped line, as chat messages for a stateless lane. The just-saved
/// you-row (== raw_line) is dropped from history so the line lands once
/// — as the final user message. Compact: newest-first budget fill, same
/// skip list as mouth_roll's handoff (ack noise never rides the packet).
fn fenced_soul(soul: &str, include_history: bool) -> String {
    if include_history {
        soul.to_string()
    } else {
        format!(
            "{soul}\n\nThe attached image is the only picture in this message. Earlier turns are not that image."
        )
    }
}

pub(crate) fn hop_messages(
    app: &tauri::AppHandle,
    soul: &str,
    raw_line: &str,
    prompt: &str,
    with_tools: bool,
    include_history: bool,
) -> Vec<Value> {
    if crate::companion::memory::enabled(app) && !crate::mouth_gist::is_gist(prompt) {
        let rendered = crate::companion::memory::prompt_block(app);
        let none: &[crate::state::CompanionTurn] = &[];
        return assemble_hop_messages(&rendered, raw_line, prompt, with_tools, none, false);
    }
    let turns = crate::state::load_state(app)
        .map(|s| s.companion.turns)
        .unwrap_or_default();
    let _bound = crate::shims::Bind::arm(app);
    assemble_hop_messages(soul, raw_line, prompt, with_tools, &turns, include_history)
}

pub(crate) fn assemble_hop_messages(
    soul: &str,
    raw_line: &str,
    prompt: &str,
    with_tools: bool,
    turns: &[crate::state::CompanionTurn],
    include_history: bool,
) -> Vec<Value> {
    if crate::mouth_gist::is_gist(prompt) {
        let sys = if with_tools {
            // Fat spark.md prefill is a big slice of the 25s list hop.
            format!(
                "{}\n{}",
                crate::mouth_gist::CONSUME,
                crate::mouth_gist::BOARD_LIST
            )
        } else {
            crate::mouth_gist::hop_system(soul)
        };
        return vec![
            json!({ "role": "system", "content": fenced_soul(&sys, include_history) }),
            json!({ "role": "user", "content": prompt }),
        ];
    }
    let mut messages =
        vec![json!({ "role": "system", "content": fenced_soul(soul, include_history) })];
    if !include_history {
        messages.push(json!({ "role": "user", "content": prompt }));
        return messages;
    }
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
        let text = strip_handoff_marks(&t.text);
        if text != t.text {
            crate::shims::fired_bound("handoff_strip", crate::shims::ShimCtx { seat: None });
        }
        messages.push(json!({ "role": role, "content": text.replace('\n', " ") }));
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
        let p = std::env::temp_dir().join(format!(
            "orbitty-hop-{}-{nanos}{suffix}",
            std::process::id()
        ));
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
        let line = err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
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
pub(crate) fn json_error_note(body: &str) -> String {
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
    let (status, body) = curl_json("GET", &format!("{OLLAMA_BASE}/api/tags"), None, None, 3)?;
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

pub(crate) fn openai_chat_content(text: &str) -> Result<String, String> {
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

pub(crate) fn apply_vision_or_honest(
    messages: &[Value],
    vision_path: Option<&str>,
    wire: crate::attach_vision::VisionWire,
) -> Result<Vec<Value>, String> {
    let Some(path) = vision_path else {
        return Ok(messages.to_vec());
    };
    match crate::attach_vision::with_vision_parts(messages, path, wire) {
        Ok(msgs) => Ok(msgs),
        Err(e) if e == crate::attach_vision::oversize_line() => Err(e),
        Err(e) => Err(e),
    }
}

fn lmstudio_chat(
    app: &tauri::AppHandle,
    chat_url: &str,
    model: &str,
    messages: &[Value],
    with_tools: bool,
    raw_line: &str,
    vision_path: Option<&str>,
) -> Result<String, String> {
    let mut msgs = match apply_vision_or_honest(
        messages,
        vision_path,
        crate::attach_vision::VisionWire::OpenAi,
    ) {
        Ok(m) => m,
        Err(e) if e == crate::attach_vision::oversize_line() => return Ok(e),
        Err(e) => return Err(e),
    };
    // Board reads: inject the snapshot first. A failed tool round was
    // a second 10–20s E4B generate. Writes still hop Grok.
    let mut tools_on = false;
    let project = crate::mouth_tools::project_for_line(app, raw_line);
    if with_tools {
        let list = crate::mouth_tools::run(
            app,
            "kanban_list",
            &json!({ "project": project, "status": "open" }),
        );
        msgs.push(json!({
            "role": "user",
            "content": format!("Board snapshot:\n{list}\n{}", crate::mouth_gist::BOARD_LIST),
        }));
    }
    for round in 0..3 {
        let mut body = json!({
            "model": model,
            "messages": msgs,
            "stream": false,
        });
        if with_tools {
            body["max_tokens"] = json!(400);
        }
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
                    &json!({ "project": project, "status": "open" }),
                );
                if !content.is_empty() {
                    msgs.push(msg);
                }
                msgs.push(json!({
                    "role": "user",
                    "content": format!(
                        "Board snapshot:\n{list}\n{}",
                        crate::mouth_gist::BOARD_LIST
                    ),
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

fn ollama_chat_named(
    model: &str,
    messages: &[Value],
    vision_path: Option<&str>,
) -> Result<String, String> {
    let messages = match apply_vision_or_honest(
        messages,
        vision_path,
        crate::attach_vision::VisionWire::Ollama,
    ) {
        Ok(m) => m,
        Err(e) if e == crate::attach_vision::oversize_line() => return Ok(e),
        Err(e) => return Err(e),
    };
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
    ollama_chat_named(&model, messages, None)
}

/// Local reply: chosen Reply id on LM Studio, else Ollama. `want` is
/// `companion.reply_model` (empty = prefer E4B).
/// A text-only local model never gets called with the image stripped off:
/// it would answer about a picture it never saw.
pub(crate) fn local_refuses_image(has_image: bool, model_vision: bool) -> bool {
    has_image && !model_vision
}

pub(crate) fn local_chat(
    app: &tauri::AppHandle,
    want: &str,
    messages: &[Value],
    with_tools: bool,
    raw_line: &str,
    vision_path: Option<&str>,
) -> Result<(&'static str, String), String> {
    match crate::mouth_scan::pick_local(want) {
        Some(crate::mouth_scan::LocalTarget::LmStudio {
            chat_url,
            model,
            vision,
        }) => {
            if local_refuses_image(vision_path.is_some(), vision) {
                return Ok(("lmstudio", unseen_image_line().to_string()));
            }
            lmstudio_chat(
                app,
                &chat_url,
                &model,
                messages,
                with_tools,
                raw_line,
                vision_path.filter(|_| vision),
            )
            .map(|r| ("lmstudio", r))
        }
        Some(crate::mouth_scan::LocalTarget::Ollama { model, vision }) => {
            if local_refuses_image(vision_path.is_some(), vision) {
                return Ok(("ollama", unseen_image_line().to_string()));
            }
            ollama_chat_named(&model, messages, vision_path.filter(|_| vision))
                .map(|r| ("ollama", r))
        }
        None => Err("none found — open Ollama or LM Studio.".into()),
    }
}
