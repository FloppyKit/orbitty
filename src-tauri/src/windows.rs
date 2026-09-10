//! Window corral — shared types + platform dispatch.
//! S2: list/focus/AX unchanged; grouping + claims live in the frontend state.
//! Titles only, never window contents.

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
    /// GO-ORBITTY-SLEEPERS: the window's AXDocument as a filesystem path —
    /// file:// URLs only, and only when the path exists at poll time.
    /// Titles-only rule kept: never the file's contents.
    pub document_path: Option<String>,
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
#[path = "windows_mac.rs"]
mod platform;

#[cfg(not(target_os = "macos"))]
#[path = "windows_stub.rs"]
mod platform;

pub use platform::{
    accessibility_status, close_window, deminiaturize_window, focus_window, list_windows,
    locate_window, minimize_window, open_accessibility_settings, prompt_accessibility,
    restore_window,
};
