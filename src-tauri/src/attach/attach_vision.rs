//! Image chips: vision first, OCR never by default.
//! Grok headless takes `--prompt-json` content blocks. Other mouths
//! stay path-only with an honest note. No Tesseract, no OpenAI.

use std::path::Path;

use serde_json::json;

use crate::attach::AttachInfo;

const IMAGES: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

pub fn is_image(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|e| IMAGES.contains(&e.to_ascii_lowercase().as_str()))
}

/// Resolved lane plus the probed model's capability. Grok's CLI takes
/// images. Local and OpenRouter see only when the probe said so.
/// Goose, Hermes, and anything else do not.
pub fn backend_has_vision(lane: &str, model_vision: bool) -> bool {
    match lane.trim().to_ascii_lowercase().as_str() {
        "grok" => true,
        "local" | "lmstudio" | "ollama" | "openrouter" | "openrouter_free" | "free" => {
            model_vision
        }
        _ => false,
    }
}

/// The send-path "can't see images" refusal applies only to the probed free
/// lanes (local, OpenRouter free) when the model is text-only. Grok, Goose,
/// Hermes, Berd and any other harness keep the base behavior: the file path
/// plus the no-vision note, and they read the file with their own tools.
pub fn vision_gate_applies(lane: &str, model_vision: bool) -> bool {
    matches!(
        lane.trim().to_ascii_lowercase().as_str(),
        "local" | "openrouter_free" | "compat"
    ) && !model_vision
}

/// Refuse anything over this before it becomes a data URI.
pub const IMAGE_BYTE_CAP: u64 = 8 * 1024 * 1024;

pub fn image_mime(path: &str) -> Option<&'static str> {
    match Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        Some("gif") => Some("image/gif"),
        _ => None,
    }
}

pub fn oversize_line() -> String {
    "This image is over 8 MB, so it was not sent.".into()
}

pub fn check_image_len(len: u64) -> Result<(), String> {
    if len > IMAGE_BYTE_CAP {
        Err(oversize_line())
    } else {
        Ok(())
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    match data.len() - i {
        1 => {
            let n = (data[i] as u32) << 16;
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

pub fn load_image_b64(path: &str) -> Result<(String, &'static str), String> {
    let mime = image_mime(path).ok_or_else(|| "not an image".to_string())?;
    let meta = std::fs::metadata(path).map_err(|e| format!("read image: {e}"))?;
    check_image_len(meta.len())?;
    let bytes = std::fs::read(path).map_err(|e| format!("read image: {e}"))?;
    check_image_len(bytes.len() as u64)?;
    Ok((b64(&bytes), mime))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisionWire {
    OpenAi,
    Ollama,
}

/// Last user message becomes a vision payload. OpenAI-compat (LM Studio,
/// OpenRouter) uses an `image_url` data URI. Ollama uses `images`.
pub fn with_vision_parts(
    messages: &[serde_json::Value],
    path: &str,
    wire: VisionWire,
) -> Result<Vec<serde_json::Value>, String> {
    let (encoded, mime) = load_image_b64(path)?;
    let mut msgs = messages.to_vec();
    let Some(idx) = msgs.iter().rposition(|m| {
        m.get("role").and_then(|r| r.as_str()) == Some("user")
    }) else {
        return Err("no user message for the image".into());
    };
    let text = match msgs[idx].get("content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .find_map(|p| p.get("text").and_then(|t| t.as_str()))
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    match wire {
        VisionWire::OpenAi => {
            msgs[idx]["content"] = json!([
                { "type": "text", "text": text },
                {
                    "type": "image_url",
                    "image_url": { "url": format!("data:{mime};base64,{encoded}") }
                }
            ]);
        }
        VisionWire::Ollama => {
            msgs[idx]["content"] = json!(text);
            msgs[idx]["images"] = json!([encoded]);
        }
    }
    Ok(msgs)
}

/// JSON content blocks for `grok --prompt-json`. Path, not base64 —
/// a 25 MB chip must not ride argv.
pub fn prompt_json(text: &str, image_path: &str) -> String {
    json!([
        { "type": "text", "text": text },
        { "type": "image", "path": image_path }
    ])
    .to_string()
}

pub fn no_vision_note(info: &AttachInfo) -> String {
    format!(
        "mouth has no vision on this backend — path only: {} ({})",
        info.path,
        crate::attach::human_size(info.size)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_exts() {
        assert!(is_image("/tmp/a.PNG"));
        assert!(is_image("/tmp/a.webp"));
        assert!(!is_image("/tmp/a.pdf"));
        assert!(!is_image("/tmp/a.rs"));
    }

    #[test]
    fn lane_plus_model_capability() {
        assert!(backend_has_vision("grok", false));
        assert!(backend_has_vision("local", true));
        assert!(!backend_has_vision("local", false));
        assert!(backend_has_vision("openrouter_free", true));
        assert!(!backend_has_vision("openrouter", false));
        assert!(!backend_has_vision("goose", true));
        assert!(!backend_has_vision("hermes", true));
    }

    #[test]
    fn mime_by_extension() {
        assert_eq!(image_mime("/tmp/a.PNG"), Some("image/png"));
        assert_eq!(image_mime("/tmp/a.jpg"), Some("image/jpeg"));
        assert_eq!(image_mime("/tmp/a.jpeg"), Some("image/jpeg"));
        assert_eq!(image_mime("/tmp/a.webp"), Some("image/webp"));
        assert_eq!(image_mime("/tmp/a.gif"), Some("image/gif"));
        assert_eq!(image_mime("/tmp/a.pdf"), None);
    }

    #[test]
    fn eight_mb_cap_is_honest() {
        assert!(check_image_len(IMAGE_BYTE_CAP).is_ok());
        let err = check_image_len(IMAGE_BYTE_CAP + 1).unwrap_err();
        assert!(err.contains("8 MB"));
        assert!(err.len() < 120);
        let p = std::env::temp_dir().join(format!(
            "orbitty-vision-big-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(IMAGE_BYTE_CAP + 1).unwrap();
        drop(f);
        let err = load_image_b64(p.to_str().unwrap()).unwrap_err();
        let _ = std::fs::remove_file(&p);
        assert!(err.contains("8 MB"));
        assert!(err.len() < 120);
    }

    #[test]
    fn lmstudio_body_image_url_part_types_only() {
        let p = std::env::temp_dir().join(format!("orbitty-vision-tiny-{}.png", std::process::id()));
        std::fs::write(&p, b"not-a-real-png").unwrap();
        let messages = vec![
            json!({ "role": "system", "content": "soul" }),
            json!({ "role": "user", "content": "what is this image?" }),
        ];
        let msgs = with_vision_parts(&messages, p.to_str().unwrap(), VisionWire::OpenAi).unwrap();
        let _ = std::fs::remove_file(&p);
        let body = json!({
            "model": "google/gemma-4-e4b",
            "messages": msgs,
            "stream": false,
        });
        let parts = body["messages"][1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image_url");
        let url = parts[1]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "data-uri prefix");
        assert!(url.split_once(',').is_some_and(|(_, rest)| !rest.is_empty()));
        assert!(!body.to_string().contains("not-a-real-png"));
    }

    #[test]
    fn ollama_message_uses_images_field() {
        let p = std::env::temp_dir().join(format!("orbitty-vision-ollama-{}.jpg", std::process::id()));
        std::fs::write(&p, b"jj").unwrap();
        let messages = vec![json!({ "role": "user", "content": "what is this" })];
        let msgs = with_vision_parts(&messages, p.to_str().unwrap(), VisionWire::Ollama).unwrap();
        let _ = std::fs::remove_file(&p);
        assert!(msgs[0]["content"].is_string());
        assert!(msgs[0]["images"].as_array().is_some_and(|a| a.len() == 1));
        assert!(msgs[0].get("image_url").is_none());
        assert!(!msgs[0].to_string().contains("\"image_url\""));
    }

    #[test]
    fn json_names_the_path() {
        let j = prompt_json("look", "/tmp/x.png");
        assert!(j.contains("/tmp/x.png"));
        assert!(j.contains("\"image\""));
        assert!(!j.contains("base64"));
    }

    // B1: the send-path refusal is only for text-only free lanes. Every
    // other backend keeps the path + no-vision note it had at base.
    #[test]
    fn vision_gate_only_on_text_only_free_lanes() {
        for lane in ["hermes", "goose", "berd", "cursor", "grok", "whatever", ""] {
            assert!(!vision_gate_applies(lane, false), "{lane}");
            assert!(!vision_gate_applies(lane, true), "{lane}");
        }
        assert!(vision_gate_applies("local", false));
        assert!(vision_gate_applies("openrouter_free", false));
        assert!(vision_gate_applies("compat", false));
        assert!(!vision_gate_applies("compat", true));
        assert!(vision_gate_applies(" LOCAL ", false));
        assert!(!vision_gate_applies("local", true));
        assert!(!vision_gate_applies("openrouter_free", true));
    }

    // B1 + V14: companion_send_inner applies exactly this gate, before the
    // backend is called; the non-grok lane stays (backend, false).
    #[test]
    fn send_path_uses_the_narrow_gate_before_the_backend() {
        let coord = concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
);
        let at = coord.find("async fn companion_send_inner(").unwrap();
        let rest = &coord[at..];
        let send = &rest[..rest.find("\n}\n").unwrap()];
        let gate = send
            .find("if image && crate::attach_vision::vision_gate_applies(&lane, model_vision) {")
            .expect("send-path gate");
        let refuse = send.find("crate::mouth_hop::refuse_or_handoff(").unwrap();
        let call = send.find("let reply = match backend.as_str() {").unwrap();
        assert!(gate < refuse && refuse < call);
        assert!(!send.contains("backend_has_vision("), "old wide gate is back");
        assert!(send.contains("(b, false)"));
    }
}
