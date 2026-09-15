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
/// - `stars` / `kanban`: the one shared 780×520 overlay (BOARD-STARS).
///   "Visible" for the button means the overlay is up AND already in
///   that button's mode — an overlay visible in the OTHER mode takes
///   the show path (a warm mode swap), never a hide. Hide targets the
///   `kanban` window; the legacy `stars` pane can never exist
///   (PANE-404), so `hide_window("stars")` is never the real path.
/// - `companion`: hide rides `hide_window`, which keeps the companion
///   bookkeeping — `talk::stop_if_any`, unpin, persist visible=false.
/// - `sky` / `settings`: plain panes; no new visibility slice is
///   persisted for them (lock law 3).
pub(crate) fn toggle(app: &tauri::AppHandle, label: &str) -> Result<bool, String> {
    match label {
        "stars" | "kanban" => toggle_shared(app, label),
        "companion" | "sky" | "settings" => toggle_plain(app, label),
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

/// Board / STARS share the one `kanban` overlay. Hide only when the
/// overlay is up in THIS button's mode; otherwise the existing show
/// path (which swaps a warm overlay's mode live). A state-load miss
/// never hides — the show path is the safe default.
fn toggle_shared(app: &tauri::AppHandle, label: &str) -> Result<bool, String> {
    let mode = if label == "stars" { "stars" } else { "board" };
    let cur = crate::state::load_state(app)
        .ok()
        .map(|s| s.shared_pane_mode);
    if visible(app, "kanban") && cur.as_deref() == Some(mode) {
        crate::panes::hide_window(app.clone(), "kanban".to_string())?;
        return Ok(false);
    }
    crate::panes::show_shared_pane(app, mode)?;
    Ok(true)
}

/// The command the rail clicks invoke (`toggle_labeled`, one label
/// arg). Same helper, same law.
#[tauri::command]
pub fn toggle_labeled(app: tauri::AppHandle, label: String) -> Result<bool, String> {
    toggle(&app, &label)
}
