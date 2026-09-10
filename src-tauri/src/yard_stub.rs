//! Yard stub — the herding-cats overlay is macOS-only (NSPanel ghost
//! pattern). Everywhere else the mode stays off and start is an honest
//! error, never a fake rain.

pub fn active() -> bool {
    false
}

pub fn start(_app: tauri::AppHandle) -> Result<(), String> {
    Err("yard: macOS only".into())
}

pub fn stop() -> Result<(), String> {
    Ok(())
}
