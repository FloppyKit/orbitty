//! The stack buttons toggle like the dests —
//! first click shows + focuses (the button's existing show path), a
//! click while the window is already visible HIDES it. Hide is always
//! `panes::hide_window`: hide-not-destroy, never a quit (tray Quit
//! stays the only quit), never an AX-close of claimed OS windows.
//! Home (`bring_all_front`) and the Agents / Actions / Projects lanes
//! are untouched; × on each window still hides.
//!
//! Lock: ORBITTY_RAIL_CLOSE_LOCK_v0.md. One helper owns the
//! visible?-hide-else-show branch for the five rail labels so each
//! button's wiring stays a one-line invoke.

use tauri::Manager;

/// Rail stack-button toggle. Returns `Ok(true)` when the window ended
/// SHOWN, `Ok(false)` when HIDDEN — callers (the tray) fire show-only
/// side effects (the companion caret summon) on the show edge only.
///
/// - `stars` / `kanban`: separate windows. Visible → hide; else show.
/// - `companion`: hide rides `hide_window`, which keeps the companion
///   bookkeeping — `talk::stop_if_any`, unpin, persist visible=false.
/// - `sky` / `settings`: plain panes; no new visibility slice is
///   persisted for them (lock law 3).
pub(crate) fn toggle(app: &tauri::AppHandle, label: &str) -> Result<bool, String> {
    match label {
        "stars" | "kanban" | "companion" | "sky" | "settings" => toggle_plain(app, label),
        _ => Err(format!("rail toggle: unknown label {label}")),
    }
}

/// Live visibility, same check `toggle_next_card_window` already uses.
/// A never-created pane (sky stays lazy) reads as hidden → show path.
fn visible(app: &tauri::AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

/// Plain panes + the companion: visible → `hide_window`; else the
/// button's existing show path (`show_companion` keeps its visible=true
/// persist; `show_labeled` keeps the drawer leash).
fn toggle_plain(app: &tauri::AppHandle, label: &str) -> Result<bool, String> {
    if visible(app, label) {
        crate::panes::hide_window(app.clone(), label.to_string())?;
        return Ok(false);
    }
    if label == "companion" {
        crate::panes::show_companion(app)?;
    } else {
        crate::panes::show_labeled(app, label)?;
    }
    Ok(true)
}

/// The command the rail clicks invoke (`toggle_labeled`, one label
/// arg). Same helper, same law.
#[tauri::command]
pub fn toggle_labeled(app: tauri::AppHandle, label: String) -> Result<bool, String> {
    toggle(&app, &label)
}

/// Dock dest chips: Launchpad / STARS / inspector as lightswitches.
/// Visible in that mode → hide; else show and order front.
#[tauri::command]
pub fn dest_toggle(app: tauri::AppHandle, dest: String) -> Result<bool, String> {
    match dest.as_str() {
        "launchpad" => toggle(&app, "kanban"),
        "stars" => toggle(&app, "stars"),
        "inspector" => {
            if visible(&app, "main") {
                crate::panes::hide_window(app, "main".into())?;
                Ok(false)
            } else {
                crate::show_rail_inner(&app);
                Ok(true)
            }
        }
        _ => Err(format!("dest_toggle: unknown dest {dest}")),
    }
}

/// Raise-only (tray / leftover host events). Never hides.
#[tauri::command]
pub fn dest_show(app: tauri::AppHandle, dest: String) -> Result<(), String> {
    match dest.as_str() {
        "launchpad" => crate::panes::show_shared_pane(&app, "board"),
        "stars" => crate::panes::show_shared_pane(&app, "stars"),
        "inspector" => {
            crate::show_rail_inner(&app);
            Ok(())
        }
        _ => Err(format!("dest_show: unknown dest {dest}")),
    }
}

/// Chip highlight: which dest windows are up.
#[tauri::command]
pub fn dest_open_state(app: tauri::AppHandle) -> serde_json::Value {
    serde_json::json!({
        "launchpad": visible(&app, "kanban"),
        "stars": visible(&app, "stars"),
        "inspector": visible(&app, "main"),
    })
}
