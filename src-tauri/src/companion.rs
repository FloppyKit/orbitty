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
    /// GO-ORBITTY-LOOK-AT: 8-dir eye cells (E NE N NW W SW S SE). Empty
    /// when the set is missing/incomplete, or on a user pack — the
    /// webview stays on the idle loop (fail soft).
    #[serde(default)]
    pub look_frames: Vec<String>,
    #[serde(default)]
    pub look_bust_frames: Vec<String>,
    /// Reaction overlay, order: blink, poke, sleepy, sparkle, delighted,
    /// dizzy. All-or-nothing (6) or empty.
    #[serde(default)]
    pub overlay_frames: Vec<String>,
    #[serde(default)]
    pub overlay_bust_frames: Vec<String>,
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
                look_frames: Vec::new(),
                look_bust_frames: Vec::new(),
                overlay_frames: Vec::new(),
                overlay_bust_frames: Vec::new(),
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
                look_frames: Vec::new(),
                look_bust_frames: Vec::new(),
                overlay_frames: Vec::new(),
                overlay_bust_frames: Vec::new(),
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
    spirit_art_from(&dir)
}

fn numbered_pngs(dir: &Path, prefix: &str, n: u32) -> Vec<String> {
    (0..n)
        .filter_map(|i| data_url(&dir.join(format!("{prefix}{i}.png"))))
        .collect()
}

fn overlay_pngs(dir: &Path, prefix: &str) -> Vec<String> {
    let v: Vec<String> = crate::companion_look::OVERLAY_KINDS
        .iter()
        .filter_map(|k| data_url(&dir.join(format!("{prefix}{k}.png"))))
        .collect();
    if v.len() == crate::companion_look::OVERLAY_KINDS.len() {
        v
    } else {
        Vec::new()
    }
}

fn all_or_nothing(frames: Vec<String>, n: usize) -> Vec<String> {
    if frames.len() == n {
        frames
    } else {
        Vec::new()
    }
}

/// Blessed spirit dir (`companion/spirit`). Look-at / overlay fail soft.
fn spirit_art_from(dir: &Path) -> Result<CompanionArt, String> {
    // Thinking v9: all-or-nothing — a partial set would hitch the loop, so
    // anything short of 8 slices falls back to rest-only (fail soft).
    let thinking_dir = dir.join("thinking");
    let thinking_frames = all_or_nothing(numbered_pngs(&thinking_dir, "think_", 8), 8);
    // HEADROOM bust sets: same all-or-nothing rule, same cadence. Missing
    // busts are a quiet miss — the webview sits on Full, Head disabled.
    let bust_frames = all_or_nothing(numbered_pngs(dir, "bust_", 8), 8);
    let bust_thinking_frames = all_or_nothing(numbered_pngs(&thinking_dir, "bust_", 8), 8);
    let look = dir.join("look");
    let look_frames = all_or_nothing(numbered_pngs(&look, "look_", 8), 8);
    let look_bust_frames = all_or_nothing(numbered_pngs(&look, "bust_", 8), 8);
    let overlay_frames = overlay_pngs(&look, "overlay_");
    let overlay_bust_frames = overlay_pngs(&look, "overlay_bust_");
    let frames = numbered_pngs(dir, "idle_", 8);
    if frames.len() == 8 {
        return Ok(CompanionArt {
            frames,
            thinking_frames,
            bust_frames,
            bust_thinking_frames,
            sheet: None,
            frame_ms: FRAME_MS,
            smooth: false,
            look_frames,
            look_bust_frames,
            overlay_frames,
            overlay_bust_frames,
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
            look_frames,
            look_bust_frames,
            overlay_frames,
            overlay_bust_frames,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn brand_spirit() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../brand/companion/spirit")
    }

    #[test]
    fn spirit_loads_eight_dir_and_overlay() {
        let art = spirit_art_from(&brand_spirit()).expect("blessed spirit dir");
        assert_eq!(art.frames.len(), 8);
        assert_eq!(art.look_frames.len(), 8, "8-dir look cells");
        assert_eq!(art.look_bust_frames.len(), 8);
        assert_eq!(art.overlay_frames.len(), 6, "blink poke sleepy sparkle delighted dizzy");
        assert_eq!(art.overlay_bust_frames.len(), 6);
        assert!(art.look_frames[0].starts_with("data:image/png;base64,"));
        assert_ne!(
            art.look_frames[0], art.look_frames[4],
            "east and west cells must differ"
        );
        assert_ne!(art.overlay_frames[0], art.overlay_frames[1]);
    }

    #[test]
    fn missing_look_dir_fails_soft() {
        let tmp = std::env::temp_dir().join(format!(
            "orbitty-look-at-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let src = brand_spirit();
        for i in 0..8 {
            std::fs::copy(src.join(format!("idle_{i}.png")), tmp.join(format!("idle_{i}.png")))
                .unwrap();
        }
        let art = spirit_art_from(&tmp).unwrap();
        assert_eq!(art.frames.len(), 8);
        assert!(art.look_frames.is_empty());
        assert!(art.overlay_frames.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
