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

pub fn backend_has_vision(backend: &str) -> bool {
    backend == "grok"
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
    fn grok_only() {
        assert!(backend_has_vision("grok"));
        assert!(!backend_has_vision("goose"));
        assert!(!backend_has_vision("hermes"));
    }

    #[test]
    fn json_names_the_path() {
        let j = prompt_json("look", "/tmp/x.png");
        assert!(j.contains("/tmp/x.png"));
        assert!(j.contains("\"image\""));
        assert!(!j.contains("base64"));
    }
}
