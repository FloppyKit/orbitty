//! Window corral — shared types + platform dispatch.
//! S2: list/focus/AX unchanged; grouping + claims live in the frontend state.
//! Titles only, never window contents.
//! Exception: the rail's in-memory layer-2 preview (`window_thumb`) may hold
//! a JPEG of the centred window. That buffer stays in the process: it is
//! never written to state, disk, logs, companion, agents, relay, or MCP.

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    /// `pid:cgWindowID` when the private AX→CG correlation works,
    /// else `pid:t:<title>#<n>` (session-stable while title is unchanged).
    pub id: String,
    pub pid: i32,
    pub app_name: String,
    pub title: String,
    pub bundle_id: Option<String>,
    /// Reboot-stable fingerprint: `{bundleId or appName}|{title}|{n}` where
    /// `n` is 1-based among same bundle+title in this enumerate. Live ids
    /// (`pid:cgWindowID`) die on reboot; this key is what claims rematch on.
    pub stable_key: String,
    /// 32×32 PNG, base64 with no `data:` prefix; cached by bundle id.
    pub icon_base64: Option<String>,
    pub is_frontmost: bool,
    pub is_minimized: bool,
    /// The window's AXDocument as a filesystem path —
    /// file:// URLs only, and only when the path exists at poll time.
    /// Titles-only rule kept: never the file's contents.
    pub document_path: Option<String>,
}

/// One window's live box, in AX points (top-left origin, y grows down).
/// The scene reads these to snapshot the desk before it moves anything and
/// to learn which windows are on a visible Space.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrame {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub is_minimized: bool,
    /// On a visible Space. False = another Space, or a display that has
    /// gone, so a scene leaves the window alone (42b Needs Matt 4).
    pub on_screen: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityStatus {
    pub trusted: bool,
    /// Path TCC actually ties the grant to: the .app when bundled,
    /// the exact binary when running unbundled (`tauri dev`).
    pub bundle_path: String,
    pub signing_line: String,
    pub in_bundle: bool,
    pub is_adhoc: bool,
}

#[cfg(target_os = "macos")]
mod minimize_watch;
#[cfg(target_os = "macos")]
#[path = "windows_mac.rs"]
mod platform;

#[cfg(not(target_os = "macos"))]
#[path = "windows_stub.rs"]
mod platform;

pub use platform::{
    accessibility_status, deminiaturize_window, frontmost_claimable, install_frontmost_tracker,
    install_rail_mouse_buttons, list_windows, open_accessibility_settings, prompt_accessibility,
    sync_rail_mouse_keys,
};

#[cfg(target_os = "macos")]
pub use minimize_watch::start as start_minimize_watch;
#[cfg(not(target_os = "macos"))]
pub fn start_minimize_watch(_app: tauri::AppHandle) {}

/// `tab:` ids never reach AX. Asleep tabs reopen; live tabs focus.
pub fn focus_window(id: &str) -> Result<(), String> {
    if let Some(done) = crate::tabbity::dispatch_window(id, "focus") {
        return done;
    }
    platform::focus_window(id)
}

pub fn minimize_window(id: &str) -> Result<(), String> {
    if let Some(done) = crate::tabbity::dispatch_window(id, "minimize") {
        return done;
    }
    platform::minimize_window(id)
}

pub fn restore_window(id: &str) -> Result<(), String> {
    if let Some(done) = crate::tabbity::dispatch_window(id, "restore") {
        return done;
    }
    platform::restore_window(id)
}

pub fn close_window(id: &str) -> Result<(), String> {
    if let Some(done) = crate::tabbity::dispatch_window(id, "close") {
        return done;
    }
    platform::close_window(id)
}

pub fn locate_window(id: &str) -> Result<(), String> {
    if let Some(done) = crate::tabbity::dispatch_window(id, "locate") {
        return done;
    }
    platform::locate_window(id)
}

/// Planet scenes (GO 42b): live frames for these window ids. An id that no
/// longer resolves is simply absent from the reply, so a scene skips it
/// instead of failing. Read only — this never moves a window.
pub fn window_frames(ids: &[String]) -> Result<Vec<WindowFrame>, String> {
    let live: Vec<String> = ids
        .iter()
        .filter(|id| !id.starts_with("tab:"))
        .cloned()
        .collect();
    platform::window_frames(&live)
}

/// Planet scenes (GO 42b): put one window back in a frame. AXPosition is a
/// top-left-origin point; scene::Frame pins that, never AppKit's bottom-left.
pub fn set_window_frame(id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
    if id.starts_with("tab:") {
        return Err("a tab has no window frame".into());
    }
    platform::set_window_frame(id, x, y, w, h)
}
