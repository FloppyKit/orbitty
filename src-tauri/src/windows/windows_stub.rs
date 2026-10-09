//! Non-macOS stub — Orbitty ships Mac first (roadmap S-phase).

use super::{AccessibilityStatus, WindowInfo};

pub fn install_rail_mouse_buttons() {}

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    Ok(vec![])
}

pub fn focus_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn minimize_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn restore_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn deminiaturize_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn close_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn locate_window(_id: &str) -> Result<(), String> {
    Err("window corral is macOS-only for now".into())
}

pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: false,
        bundle_path: String::new(),
        signing_line: "unsupported platform".into(),
        in_bundle: false,
        is_adhoc: false,
    }
}

pub fn open_accessibility_settings() {}

pub fn prompt_accessibility() -> bool {
    false
}

pub fn frontmost_claimable() -> Result<Option<WindowInfo>, String> {
    Ok(None)
}

pub fn install_frontmost_tracker() {}

