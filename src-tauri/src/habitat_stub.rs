//! Non-macOS stub — the habitat ships Mac first (roadmap S-phase).

pub fn start(_app: tauri::AppHandle) {}

pub fn sync(_app: &tauri::AppHandle, _snapshot: super::HabitatSnapshot) -> Result<(), String> {
    Ok(())
}

pub fn restore_all() {}

pub fn bring_all_front() {}

pub fn bring_planet_front(_project_id: &str) {}

pub fn set_planets_hidden(_hidden: bool) {}

pub fn persist_now() {}

pub fn locate_walker(_app: tauri::AppHandle, _entry_id: String) -> Result<(), String> {
    Err("habitat is macOS-only for now".into())
}

pub fn locate_planet(_app: tauri::AppHandle, _project_id: String) -> Result<(), String> {
    Err("habitat is macOS-only for now".into())
}

pub fn reload_pack_frames() {}

/// GO-ORBITTY-HERDING-CATS: yard meows skip while a boombox plays.
/// No habitat off macOS = never playing.
pub fn boombox_playing() -> bool {
    false
}

/// Being-editor preview is rendered with AppKit on macOS; elsewhere the
/// editor falls back to no preview (the planet still draws the being).
pub fn char_preview(_look: crate::chars::CharLook, _px: u32) -> Result<String, String> {
    Err("char preview is macOS-only".into())
}
