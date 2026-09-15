//! Art source for the large companion window.
//! Default: the blessed spirit (`brand/companion/spirit`, bundled at
//! `companion/spirit`) — 8 idle frames @ 140 ms, hard alpha, nearest-
//! neighbor in the webview. "Use my image…" swaps in a local pack
//! (packs/<id>, via packs::import_pack_image); clearing returns to the
//! spirit. Missing art fails obvious ("spirit missing") — never a Bitty,
//! never chars.rs glyphs, never a redraw of the blessed pixels.

use serde::Serialize;
use std::path::Path;
use tauri::Manager;

/// Lock: 8 idle frames @ 140 ms. A user GIF animates inside the <img>
/// itself; user frame sets cycle at this same cadence.
pub const FRAME_MS: u32 = 140;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompanionSheet {
    pub data_url: String,
    pub frame_w: u32,
    pub frame_h: u32,
    pub count: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompanionArt {
    /// Ready-to-show frames as data URLs. Empty when `sheet` is set.
    pub frames: Vec<String>,
    /// Thinking v9 loop (companion/spirit/thinking). Fail soft: empty when
    /// the set is missing or incomplete — the webview stays on rest frames.
    /// User packs never carry a thinking set.
    #[serde(default)]
    pub thinking_frames: Vec<String>,
    /// Bust set (head+neck+collar, hard
    /// alpha) for the Head view, same cadence as the idle loop. Empty when
    /// the body has no bust art — the webview sits on Full with Head
    /// disabled. NEVER a CSS crop of the full frames.
    #[serde(default)]
    pub bust_frames: Vec<String>,
    /// Thinking swap for the bust set (companion/spirit/thinking/bust_*).
    /// Fail soft to the rest busts when missing/incomplete.
    #[serde(default)]
    pub bust_thinking_frames: Vec<String>,
    /// An unsliced horizontal strip (user packs, or the blessed ghost.png
    /// fallback). The webview slices 1:1 on a canvas — a blit, not a redraw.
    pub sheet: Option<CompanionSheet>,
    pub frame_ms: u32,
    /// 3D companion bakes: skip nearest-neighbor (spirit stays pixelated).
    #[serde(default)]
    pub smooth: bool,
}

fn mime_for(p: &Path) -> &'static str {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => "image/gif",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "image/png",
    }
}

fn data_url(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    Some(format!(
        "data:{};base64,{}",
        mime_for(p),
        base64_encode(&bytes)
    ))
}

fn source_art(src: &crate::packs::ArtSource, frame_ms: u32, smooth: bool) -> Result<CompanionArt, String> {
    let ms = if frame_ms > 0 { frame_ms } else { FRAME_MS };
    match src {
        crate::packs::ArtSource::Frames(paths) => {
            let frames: Vec<String> = paths.iter().filter_map(|p| data_url(p)).collect();
            if frames.is_empty() {
                return Err("companion pack frames unreadable".into());
            }
            Ok(CompanionArt {
                frames,
                thinking_frames: Vec::new(),
                bust_frames: Vec::new(),
                bust_thinking_frames: Vec::new(),
                sheet: None,
                frame_ms: ms,
                smooth,
            })
        }
        crate::packs::ArtSource::Sheet { file, spec } => {
            let sheet_url = data_url(file).ok_or("companion pack sheet unreadable")?;
            Ok(CompanionArt {
                frames: Vec::new(),
                thinking_frames: Vec::new(),
                bust_frames: Vec::new(),
                bust_thinking_frames: Vec::new(),
                sheet: Some(CompanionSheet {
                    data_url: sheet_url,
                    frame_w: spec.frame_w,
                    frame_h: spec.frame_h,
                    count: spec.count,
                }),
                frame_ms: ms,
                smooth,
            })
        }
    }
}

#[tauri::command]
pub fn companion_frames(
    app: tauri::AppHandle,
    pack_id: Option<String>,
) -> Result<CompanionArt, String> {
    if let Some(id) = pack_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // "Use my image…" winner: the idle cycle when the pack has one,
        // else the walk cycle (a lone image is a still spirit).
        let art = crate::packs::companion_pack_art(&app, id)
            .ok_or_else(|| format!("companion pack {id:?} missing"))?;
        // HEADROOM: the pack's own bust_* set when it carries one — never
        // derived from the idle frames (no crop fallback).
        let bust_frames: Vec<String> = crate::packs::companion_pack_bust(&app, id)
            .iter()
            .filter_map(|p| data_url(p))
            .collect();
        let src = art.idle.unwrap_or(art.walk);
        let mut out = source_art(&src, art.frame_ms, art.smooth)?;
        out.bust_frames = bust_frames;
        return Ok(out);
    }
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource dir: {e}"))?
        .join("companion")
        .join("spirit");
    // Thinking v9: all-or-nothing — a partial set would hitch the loop, so
    // anything short of 8 slices falls back to rest-only (fail soft).
    let thinking_dir = dir.join("thinking");
    let thinking: Vec<String> = (0..8)
        .filter_map(|i| data_url(&thinking_dir.join(format!("think_{i}.png"))))
        .collect();
    let thinking_frames = if thinking.len() == 8 { thinking } else { Vec::new() };
    // HEADROOM bust sets: same all-or-nothing rule, same cadence. Missing
    // busts are a quiet miss — the webview sits on Full, Head disabled.
    let bust: Vec<String> = (0..8)
        .filter_map(|i| data_url(&dir.join(format!("bust_{i}.png"))))
        .collect();
    let bust_frames = if bust.len() == 8 { bust } else { Vec::new() };
    let bust_think: Vec<String> = (0..8)
        .filter_map(|i| data_url(&thinking_dir.join(format!("bust_{i}.png"))))
        .collect();
    let bust_thinking_frames = if bust_think.len() == 8 { bust_think } else { Vec::new() };
    let frames: Vec<String> = (0..8)
        .filter_map(|i| data_url(&dir.join(format!("idle_{i}.png"))))
        .collect();
    if frames.len() == 8 {
        return Ok(CompanionArt {
            frames,
            thinking_frames,
            bust_frames,
            bust_thinking_frames,
            sheet: None,
            frame_ms: FRAME_MS,
            smooth: false,
        });
    }
    // Slices missing: play the blessed sheet, sliced 1:1 in the webview.
    if let Some(sheet_url) = data_url(&dir.join("ghost.png")) {
        return Ok(CompanionArt {
            frames: Vec::new(),
            thinking_frames,
            bust_frames,
            bust_thinking_frames,
            sheet: Some(CompanionSheet {
                data_url: sheet_url,
                frame_w: 256,
                frame_h: 384,
                count: 8,
            }),
            frame_ms: FRAME_MS,
            smooth: false,
        });
    }
    Err("spirit missing".into())
}

/// Resolve a bundled 3D companion body
/// (`companion/3d/{id}.glb`) to its filesystem path — the webview wraps it
/// with convertFileSrc for GLTFLoader (asset protocol scope:
/// `$RESOURCE/companion/3d/**`). `tang` / `female` are reserved ids, never
/// pack folders; anything else (or a missing file) returns "" so the
/// webview fails soft to the spirit — never an Err that blanks the window.
#[tauri::command]
pub fn companion_glb_url(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let id = id.trim();
    if !matches!(id, "tang" | "female") {
        return Ok(String::new());
    }
    let path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource dir: {e}"))?
        .join("companion")
        .join("3d")
        .join(format!("{id}.glb"));
    if path.is_file() {
        Ok(path.to_string_lossy().into_owned())
    } else {
        Ok(String::new())
    }
}

/// The five allowlisted gestures on the 3D
/// body. `wave` ships in the GLBs but is NOT allowlisted (shoulder not
/// locked) — it and any other name are an honest error, never a silent
/// idle. The spirit sheet mounts no `companion:play` listener, so a play
/// there is a no-op success — never fake frames on the PNGs.
pub const PLAY_CLIPS: &[&str] = &["idle", "think", "nod", "look-left", "look-right"];

pub fn companion_play(app: &tauri::AppHandle, clip: &str) -> Result<serde_json::Value, String> {
    let clip = clip.trim();
    if !PLAY_CLIPS.contains(&clip) {
        return Err(format!(
            "unknown clip {clip:?} — allowlist: {}",
            PLAY_CLIPS.join(", ")
        ));
    }
    use tauri::Emitter;
    app.emit("companion:play", serde_json::json!({ "clip": clip }))
        .map_err(|e| format!("emit companion:play: {e}"))?;
    Ok(serde_json::json!({ "played": clip }))
}

/// Standard base64. habitat_mac keeps its own copy; this one is cfg-free.
fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}



