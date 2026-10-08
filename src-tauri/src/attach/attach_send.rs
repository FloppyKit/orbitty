//! Build the mouth prompt for one chip. Original path always.
//! Office → MarkItDown markdown when the binary is on PATH.
//! Images → vision path for Grok; honest note otherwise.

use crate::attach::AttachInfo;
use crate::{attach_md, attach_vision};

pub struct Prepared {
    pub prompt: String,
    /// Absolute image path for `grok --prompt-json`. None = text only.
    pub vision_path: Option<String>,
}

pub fn prepare(
    info: &AttachInfo,
    user_prompt: &str,
    lane: &str,
    model_vision: bool,
) -> Prepared {
    let path_note = format!(
        "[file: {} ({}, {}) — original path; read it there with your tools.]",
        info.path,
        info.name,
        crate::attach::human_size(info.size)
    );
    let mut prompt = if user_prompt.is_empty() {
        path_note
    } else {
        format!("{user_prompt}\n\n{path_note}")
    };

    if attach_vision::is_image(&info.path) {
        if attach_vision::backend_has_vision(lane, model_vision) {
            return Prepared {
                prompt,
                vision_path: Some(info.path.clone()),
            };
        }
        prompt.push_str("\n\n");
        prompt.push_str(&attach_vision::no_vision_note(info));
        return Prepared {
            prompt,
            vision_path: None,
        };
    }

    if attach_md::is_office(&info.path) && !attach_md::is_already_text(&info.path) {
        match attach_md::convert(info) {
            Ok(Some(md)) => {
                prompt.push_str("\n\n[markdown from markitdown]\n");
                prompt.push_str(&md);
            }
            Ok(None) => {}
            Err(e) => {
                prompt.push_str("\n\n[");
                prompt.push_str(&e);
                prompt.push_str(" — sending the original path.]");
            }
        }
    }

    Prepared {
        prompt,
        vision_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attach::AttachInfo;

    fn info(path: &str, name: &str) -> AttachInfo {
        AttachInfo {
            path: path.into(),
            name: name.into(),
            size: 12,
        }
    }

    #[test]
    fn rs_is_path_only() {
        let p = prepare(&info("/tmp/x.rs", "x.rs"), "see this", "grok", true);
        assert!(p.prompt.contains("/tmp/x.rs"));
        assert!(!p.prompt.contains("markitdown"));
        assert!(p.vision_path.is_none());
    }

    #[test]
    fn png_on_grok_sets_vision() {
        let p = prepare(&info("/tmp/x.png", "x.png"), "what is this", "grok", false);
        assert_eq!(p.vision_path.as_deref(), Some("/tmp/x.png"));
        assert!(!p.prompt.contains("no vision"));
    }

    #[test]
    fn png_on_local_vlm_sets_vision() {
        let p = prepare(&info("/tmp/x.png", "x.png"), "what is this", "local", true);
        assert_eq!(p.vision_path.as_deref(), Some("/tmp/x.png"));
        assert!(!p.prompt.contains("no vision"));
    }

    #[test]
    fn png_on_local_text_is_honest() {
        let p = prepare(&info("/tmp/x.png", "x.png"), "what is this", "local", false);
        assert!(p.vision_path.is_none());
        assert!(p.prompt.contains("no vision"));
        assert!(p.prompt.contains("[file: /tmp/x.png"));
    }

    #[test]
    fn png_on_goose_is_honest() {
        let p = prepare(&info("/tmp/x.png", "x.png"), "what is this", "goose", true);
        assert!(p.vision_path.is_none());
        assert!(p.prompt.contains("no vision"));
    }
}
