//! Scan local mouths already running (Ollama / LM Studio). Settings
//! lists names; hop talks HTTP. Never auto-install, never silent pull.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Ollama tags — 127.0.0.1 as the Settings probe (localhost works too).
const OLLAMA_TAGS: &str = "http://127.0.0.1:11434/api/tags";
const OLLAMA_BASE: &str = "http://127.0.0.1:11434";
/// LM Studio default + the other common OpenAI-compat port.
const OPENAI_PORTS: &[u16] = &[1234, 8080];
/// Quick mouth. Catalog id Matt loads in LM Studio, not the 1B scorer.
pub const PINNED_REPLY_ID: &str = "google/gemma-4-e4b";

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScannedModel {
    pub id: String,
    /// "ollama" | "lmstudio"
    pub source: String,
    pub base: String,
    pub label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MouthSuggest {
    /// "reply" — Quick ack is off/built-in this GO, never a download.
    pub job: String,
    pub name: String,
    pub why: String,
    /// Ollama pull tag when one exists. E4B is LM Studio, so None.
    pub ollama_tag: Option<String>,
    pub have: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MouthScan {
    pub models: Vec<ScannedModel>,
    pub ollama_up: bool,
    pub lmstudio_up: bool,
    pub suggest: Vec<MouthSuggest>,
    pub ram_gb: u64,
    pub apple_silicon: bool,
    pub empty_hint: String,
    /// A scanned small local, shown as "next phrase" — not wired as
    /// Quick ack this GO.
    pub next_ack: Option<String>,
    pub ollama_bin: bool,
}

/// One running local we can hop to. LM Studio uses OpenAI-compat
/// `/v1/chat/completions`; Ollama uses `/api/chat`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalTarget {
    LmStudio { chat_url: String, model: String },
    Ollama { model: String },
}

pub fn is_twelve_b(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("12b") || l.contains("12-b") || l.contains("12_b")
}

pub fn is_embedding(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("embed") || l.contains("embedding")
}

pub fn looks_like_e4b(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("e4b") && (l.contains("gemma") || l.contains("google"))
}

/// Small enough to mention as a future Quick ack — never E4B, never 12B.
pub fn is_small_ack_candidate(id: &str) -> bool {
    if is_twelve_b(id) || looks_like_e4b(id) || is_embedding(id) {
        return false;
    }
    let l = id.to_ascii_lowercase();
    ["0.5b", "1.5b", "1b", "2b", "3b"]
        .iter()
        .any(|s| l.contains(s))
}

fn source_name(source: &str) -> &'static str {
    match source {
        "ollama" => "Ollama",
        _ => "LM Studio",
    }
}

pub fn model_label(id: &str, source: &str) -> String {
    let pretty = if looks_like_e4b(id) {
        "Gemma 4 E4B".to_string()
    } else {
        short_id(id)
    };
    format!("{pretty} ({})", source_name(source))
}

fn short_id(id: &str) -> String {
    let tail = id.rsplit('/').next().unwrap_or(id);
    if tail.chars().count() <= 48 {
        tail.to_string()
    } else {
        tail.chars().take(48).collect()
    }
}

fn keep_reply(id: &str) -> bool {
    !id.trim().is_empty() && !is_twelve_b(id) && !is_embedding(id)
}

pub fn parse_ollama_tags(body: &str) -> Vec<ScannedModel> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let mut out = Vec::new();
    let Some(models) = v.get("models").and_then(Value::as_array) else {
        return out;
    };
    for m in models {
        let id = m
            .get("name")
            .or_else(|| m.get("model"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !keep_reply(id) {
            continue;
        }
        if out.iter().any(|x: &ScannedModel| x.id == id) {
            continue;
        }
        out.push(ScannedModel {
            id: id.to_string(),
            source: "ollama".into(),
            base: OLLAMA_BASE.into(),
            label: model_label(id, "ollama"),
        });
    }
    out
}

pub fn parse_openai_models(body: &str, base: &str, source: &str) -> Vec<ScannedModel> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let mut out = Vec::new();
    let Some(data) = v.get("data").and_then(Value::as_array) else {
        return out;
    };
    for m in data {
        let id = m.get("id").and_then(Value::as_str).unwrap_or("").trim();
        if !keep_reply(id) {
            continue;
        }
        if out.iter().any(|x: &ScannedModel| x.id == id && x.base == base) {
            continue;
        }
        out.push(ScannedModel {
            id: id.to_string(),
            source: source.to_string(),
            base: base.to_string(),
            label: model_label(id, source),
        });
    }
    out
}

fn to_target(m: &ScannedModel) -> LocalTarget {
    if m.source == "ollama" {
        LocalTarget::Ollama {
            model: m.id.clone(),
        }
    } else {
        LocalTarget::LmStudio {
            chat_url: format!("{}/v1/chat/completions", m.base.trim_end_matches('/')),
            model: m.id.clone(),
        }
    }
}

/// Catalog E4B if it is up, else any other E4B id.
pub fn prefer_reply_id(scanned: &[ScannedModel]) -> Option<&ScannedModel> {
    scanned
        .iter()
        .find(|m| m.id == PINNED_REPLY_ID)
        .or_else(|| scanned.iter().find(|m| looks_like_e4b(&m.id)))
}

/// Pick the hop target: saved id if still up, else pinned E4B, else first.
pub fn resolve_local(chosen: &str, scanned: &[ScannedModel]) -> Option<LocalTarget> {
    let chosen = chosen.trim();
    if !chosen.is_empty() {
        if let Some(m) = scanned.iter().find(|m| m.id == chosen) {
            return Some(to_target(m));
        }
    }
    prefer_reply_id(scanned)
        .map(to_target)
        .or_else(|| scanned.first().map(to_target))
}

/// Persist Quick mouth when the catalog id is loaded and the field is empty.
pub fn pin_reply_if_empty(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    if !st.companion.reply_model.trim().is_empty() {
        return;
    }
    let (scanned, _, lmstudio_up) = scan_running();
    if !lmstudio_up {
        return;
    }
    if !scanned.iter().any(|m| m.id == PINNED_REPLY_ID) {
        return;
    }
    if crate::state::update(app, |st| {
        if !st.companion.reply_model.trim().is_empty() {
            return Ok(false);
        }
        st.companion.reply_model = PINNED_REPLY_ID.to_string();
        Ok(true)
    })
    .ok()
    .unwrap_or(false)
    {
        let _ = app.emit("state:changed", ());
    }
}

/// Live pick for hop — scans running servers. None = honest empty.
pub fn pick_local(want: &str) -> Option<LocalTarget> {
    let (scanned, _, _) = scan_running();
    resolve_local(want, &scanned)
}

pub fn ram_gb() -> u64 {
    let out = Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok();
    let bytes = out
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    bytes / 1_073_741_824
}

pub fn apple_silicon() -> bool {
    let out = Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.optional.arm64"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

/// Fits-this-Mac list. 16 GB Apple Silicon → Gemma 4 E4B. Never 12B.
pub fn suggest_for(ram_gb: u64, apple: bool, scanned: &[ScannedModel]) -> Vec<MouthSuggest> {
    let have = scanned.iter().any(|m| looks_like_e4b(&m.id));
    let mac = if apple {
        format!("{ram_gb} GB Apple Silicon")
    } else {
        format!("{ram_gb} GB")
    };
    let why = if have {
        format!("Fits this Mac ({mac}) — already in the list above.")
    } else {
        format!("Fits this Mac ({mac}). Load Gemma 4 E4B in LM Studio (MiniHangar).")
    };
    vec![MouthSuggest {
        job: "reply".into(),
        name: "Gemma 4 E4B".into(),
        why,
        ollama_tag: None,
        have,
    }]
}

fn next_ack_label(scanned: &[ScannedModel]) -> Option<String> {
    scanned
        .iter()
        .find(|m| is_small_ack_candidate(&m.id))
        .map(|m| m.label.clone())
}

pub fn ollama_bin() -> Option<PathBuf> {
    if let Some(p) = crate::coordinator::find_on_path("ollama") {
        if p.is_file() {
            return Some(p);
        }
    }
    for c in [
        "/Applications/Ollama.app/Contents/Resources/ollama",
        "/usr/local/bin/ollama",
        "/opt/homebrew/bin/ollama",
        "/opt/homebrew/opt/ollama/bin/ollama",
    ] {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn probe_openai(port: u16) -> Option<Vec<ScannedModel>> {
    let base = format!("http://127.0.0.1:{port}");
    let url = format!("{base}/v1/models");
    let (status, body) = crate::mouth_hop::curl_json("GET", &url, None, None, 2).ok()?;
    if !(200..=299).contains(&status) {
        return None;
    }
    let models = parse_openai_models(&body, &base, "lmstudio");
    Some(models)
}

fn probe_ollama() -> Option<Vec<ScannedModel>> {
    let (status, body) = crate::mouth_hop::curl_json("GET", OLLAMA_TAGS, None, None, 2).ok()?;
    if !(200..=299).contains(&status) {
        return None;
    }
    Some(parse_ollama_tags(&body))
}

pub fn scan_running() -> (Vec<ScannedModel>, bool, bool) {
    let mut models = Vec::new();
    let mut lmstudio_up = false;
    for port in OPENAI_PORTS {
        if let Some(ms) = probe_openai(*port) {
            lmstudio_up = true;
            for m in ms {
                if models
                    .iter()
                    .any(|x: &ScannedModel| x.id == m.id && x.base == m.base)
                {
                    continue;
                }
                models.push(m);
            }
        }
    }
    let mut ollama_up = false;
    if let Some(ms) = probe_ollama() {
        ollama_up = true;
        for m in ms {
            if models.iter().any(|x: &ScannedModel| x.id == m.id && x.source == "ollama")
            {
                continue;
            }
            models.push(m);
        }
    }
    (models, ollama_up, lmstudio_up)
}

fn build_scan() -> MouthScan {
    let (models, ollama_up, lmstudio_up) = scan_running();
    let ram = ram_gb();
    let apple = apple_silicon();
    let suggest = suggest_for(ram, apple, &models);
    let empty_hint = if models.is_empty() {
        "none found — open Ollama or LM Studio.".into()
    } else {
        String::new()
    };
    MouthScan {
        next_ack: next_ack_label(&models),
        ollama_bin: ollama_bin().is_some(),
        models,
        ollama_up,
        lmstudio_up,
        suggest,
        ram_gb: ram,
        apple_silicon: apple,
        empty_hint,
    }
}

/// Settings paints this. HTTP only — no auto-install.
/// Async + spawn_blocking: a sync Tauri command runs on the main thread,
/// and three curl probes (2 s max each) + two sysctl spawns froze the
/// whole app (beachball) whenever Settings mounted or regained focus.
#[tauri::command]
pub async fn mouth_scan() -> Result<MouthScan, String> {
    tauri::async_runtime::spawn_blocking(build_scan)
        .await
        .map_err(|e| format!("mouth scan task: {e}"))
}

/// `ollama pull <tag>` — only when Matt clicks, only if Ollama is up.
/// Never a silent pull. Never Azure/OpenAI. Off the main thread: a pull
/// can take minutes.
#[tauri::command]
pub async fn pull_mouth_model(tag: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || pull_mouth_model_blocking(tag))
        .await
        .map_err(|e| format!("pull task: {e}"))?
}

fn pull_mouth_model_blocking(tag: String) -> Result<String, String> {
    let tag = tag.trim().to_string();
    if tag.is_empty() {
        return Err("No model tag to pull.".into());
    }
    if is_twelve_b(&tag) {
        return Err("12B is gone — not pulling it.".into());
    }
    if probe_ollama().is_none() {
        return Err("Ollama is not running.".into());
    }
    let bin = ollama_bin().ok_or_else(|| "Ollama is running but the ollama command is not on this Mac.".to_string())?;
    let out = Command::new(&bin)
        .args(["pull", &tag])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("launch ollama: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("pull failed");
        return Err(line.chars().take(120).collect());
    }
    Ok(format!("pulled {tag}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_12b_and_embeddings() {
        assert!(is_twelve_b("gemma-3-12b-it"));
        assert!(is_twelve_b("Llama-3.1-12B"));
        assert!(!is_twelve_b("gemma-4-E4B-it-MLX-4bit"));
        assert!(is_embedding("text-embedding-nomic-embed-text-v1.5"));
        assert!(!is_embedding("gemma-2-2b-it"));
    }

    #[test]
    fn e4b_label() {
        let id = "lmstudio-community/gemma-4-E4B-it-MLX-4bit";
        assert!(looks_like_e4b(id));
        assert_eq!(model_label(id, "lmstudio"), "Gemma 4 E4B (LM Studio)");
    }

    #[test]
    fn parse_lmstudio_running_json() {
        let body = r#"{
          "data": [
            {"id": "gemma-2-2b-it", "object": "model"},
            {"id": "text-embedding-nomic-embed-text-v1.5", "object": "model"},
            {"id": "lmstudio-community/gemma-4-E4B-it-MLX-4bit", "object": "model"},
            {"id": "someone-12b-instruct", "object": "model"}
          ],
          "object": "list"
        }"#;
        let ms = parse_openai_models(body, "http://127.0.0.1:1234", "lmstudio");
        let ids: Vec<_> = ms.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"gemma-2-2b-it"));
        assert!(ids.contains(&"lmstudio-community/gemma-4-E4B-it-MLX-4bit"));
        assert!(!ids.iter().any(|i| i.contains("embed")));
        assert!(!ids.iter().any(|i| is_twelve_b(i)));
    }

    #[test]
    fn parse_ollama_tags_json() {
        let body = r#"{
          "models": [
            {"name": "llama3.2:3b"},
            {"name": "foo-12b"}
          ]
        }"#;
        let ms = parse_ollama_tags(body);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, "llama3.2:3b");
        assert_eq!(ms[0].source, "ollama");
    }

    #[test]
    fn resolve_prefers_chosen_then_e4b() {
        let scanned = parse_openai_models(
            r#"{"data":[
                {"id":"gemma-2-2b-it"},
                {"id":"lmstudio-community/gemma-4-E4B-it-MLX-4bit"}
            ]}"#,
            "http://127.0.0.1:1234",
            "lmstudio",
        );
        match resolve_local("gemma-2-2b-it", &scanned) {
            Some(LocalTarget::LmStudio { model, chat_url }) => {
                assert_eq!(model, "gemma-2-2b-it");
                assert!(chat_url.contains("1234"));
            }
            other => panic!("expected lmstudio 2b, got {other:?}"),
        }
        match resolve_local("", &scanned) {
            Some(LocalTarget::LmStudio { model, .. }) => {
                assert!(looks_like_e4b(&model));
            }
            other => panic!("expected e4b, got {other:?}"),
        }
        let both = parse_openai_models(
            r#"{"data":[
                {"id":"mlx-community/gemma-4-e4b-it-mlx"},
                {"id":"google/gemma-4-e4b"},
                {"id":"gemma-2-2b-it"}
            ]}"#,
            "http://127.0.0.1:1234",
            "lmstudio",
        );
        match resolve_local("", &both) {
            Some(LocalTarget::LmStudio { model, .. }) => {
                assert_eq!(model, PINNED_REPLY_ID);
            }
            other => panic!("expected pinned e4b, got {other:?}"),
        }
    }

    #[test]
    fn suggest_16gb_is_e4b_never_12b() {
        let s = suggest_for(16, true, &[]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "Gemma 4 E4B");
        assert!(s[0].ollama_tag.is_none());
        assert!(!s[0].have);
        let joined = format!("{} {}", s[0].name, s[0].why).to_ascii_lowercase();
        assert!(!joined.contains("12b"));
        let have = parse_openai_models(
            r#"{"data":[{"id":"gemma-4-E4B-it-MLX-4bit"}]}"#,
            "http://127.0.0.1:1234",
            "lmstudio",
        );
        let s = suggest_for(16, true, &have);
        assert!(s[0].have);
    }

    #[test]
    fn small_local_is_next_phrase_not_e4b() {
        assert!(is_small_ack_candidate("gemma-2-2b-it"));
        assert!(!is_small_ack_candidate("gemma-4-E4B-it-MLX-4bit"));
        assert!(!is_small_ack_candidate("foo-12b"));
    }
}
