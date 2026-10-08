mod actions;
mod attach;
mod attach_md;
mod attach_send;
mod attach_vision;
#[cfg(unix)]
mod agent_runs;
#[cfg(unix)]
mod agent_stop;
#[cfg(unix)]
mod agents;
#[cfg(unix)]
mod board;
mod card_links;
mod chars;
mod companion;
mod companion_backend;
mod direct_lane;
mod companion_clear;
mod companion_desk;
#[cfg(unix)]
mod companion_todo;
#[cfg(unix)]
mod companion_go;
#[cfg(unix)]
mod control;
mod coordinator;
mod thin;
#[cfg(unix)]
mod doing_fire;
mod verify_close;
mod rail;
pub(crate) use rail::rail_edge;
pub(crate) use rail::rail_stage;
pub(crate) use rail::rail_toggle;
mod event_keys;
mod event_log;
mod event_relay;
#[cfg(unix)]
mod gate_caps;
mod interrupt_gate;
mod mouth_ack;
mod mouth_blip;
mod mouth_gist;
mod mouth_hop;
mod mouth_scan;
mod mouth_sidecar;
mod mouth_tools;
mod mouth_board;
mod mouth_route;
mod mouth_roll;
mod needed_rules;
mod nextcard;
mod plans;
mod habitat;
mod herdr;
mod droid_inspect;
mod task;
mod todo_trees;
mod packs;
#[cfg(unix)]
mod shift;
#[cfg(unix)]
mod shift_continue;
mod state;
mod starter;
mod talk;
mod windows;
pub(crate) use windows::claim_front;
pub(crate) use windows::drawers;
pub(crate) use windows::glass;
pub(crate) use windows::panes;
pub(crate) use windows::tilechords;
pub(crate) use windows::window_chrome;
pub(crate) use windows::window_thumb;
mod yard;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};

/// Matt's tray marks (IcoB / IcoW). Not a template image — we swap the
/// actual black vs white PNG when the menu bar appearance changes.
const TRAY_LIGHT: &[u8] = include_bytes!("../icons/tray-light.rgba"); // IcoB — light bar
const TRAY_DARK: &[u8] = include_bytes!("../icons/tray-dark.rgba"); // IcoW — dark bar
const TRAY_SIZE: u32 = 44;

fn tray_image(dark_bar: bool) -> tauri::image::Image<'static> {
    let bytes = if dark_bar { TRAY_DARK } else { TRAY_LIGHT };
    tauri::image::Image::new_owned(bytes.to_vec(), TRAY_SIZE, TRAY_SIZE)
}

/// Light menu bar → AppleInterfaceStyle is unset. Dark → "Dark".
fn menu_bar_is_dark() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        use objc::runtime::Object;
        let defaults: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let key = std::ffi::CString::new("AppleInterfaceStyle").unwrap();
        let nskey: *mut Object = msg_send![class!(NSString), stringWithUTF8String: key.as_ptr()];
        let val: *mut Object = msg_send![defaults, stringForKey: nskey];
        !val.is_null()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[tauri::command]
fn list_windows() -> Result<Vec<windows::WindowInfo>, String> {
    windows::list_windows()
}

#[tauri::command]
fn focus_window(id: String) -> Result<(), String> {
    windows::focus_window(&id)
}

/// Rail Stow: ordinary AX minimize — the window sits in the Dock like any
/// other minimized window; the habitat shows its sprite on the home planet.
#[tauri::command]
fn minimize_window(id: String) -> Result<(), String> {
    windows::minimize_window(&id)
}

/// Rail Restore / sprite click: deminiaturize + focus.
#[tauri::command]
fn restore_window(id: String) -> Result<(), String> {
    windows::restore_window(&id)
}

/// Circle-complete: AX close-button press on the window (never a pid kill).
/// A refusal bubbles up as Err; the rail completes the card regardless.
#[tauri::command]
fn close_window(id: String) -> Result<(), String> {
    windows::close_window(&id)
}

/// Rail row left-click: an Orbitty-owned,
/// ignore-mouse, non-activating ring around the window's AX frame (~500ms).
/// Never focuses, never AX-raises.
#[tauri::command]
fn locate_window(id: String) -> Result<(), String> {
    windows::locate_window(&id)
}

/// `habitat:locate` for a claimed window: stroke
/// the walker's frame on its planet for ~500ms (glow only).
#[tauri::command]
fn locate_walker(app: tauri::AppHandle, entry_id: String) -> Result<(), String> {
    habitat::locate_walker(app, entry_id)
}

#[tauri::command]
fn inspect_droid(app: tauri::AppHandle, entry_id: String) {
    crate::droid_inspect::focus(&app, &entry_id);
}

/// Dest-menu find: glow that planet ~1.4s (marble saturn ring / expanded
/// rim + center name). Glow only — `bring_planet_front` is a separate raise.
#[tauri::command]
fn locate_planet(app: tauri::AppHandle, id: String) -> Result<(), String> {
    habitat::locate_planet(app, id)
}

/// the rail/sheet × door —
/// the SAME save door as ctl (`board::project_rm`): atomic save, claim
/// diff, `state:changed` emit. Cards keep their projectId; walkers vanish
/// with the disc on the next habitat sync. Unknown id = honest error.
#[cfg(unix)]
#[tauri::command]
fn project_rm(app: tauri::AppHandle, id: String) -> Result<state::Project, String> {
    board::project_rm(&app, &id)
}

#[cfg(not(unix))]
#[tauri::command]
fn project_rm(_app: tauri::AppHandle, _id: String) -> Result<state::Project, String> {
    Err("project.rm: unix only".into())
}

/// Frontend pushes the claimed-project snapshot; native side owns the panels.
#[tauri::command]
fn sync_habitat(app: tauri::AppHandle, snapshot: habitat::HabitatSnapshot) -> Result<(), String> {
    habitat::sync(&app, snapshot)
}

/// R-click Planets dest: hide or show every desktop disc.
#[tauri::command]
fn set_planets_hidden(app: tauri::AppHandle, hidden: bool) -> Result<(), String> {
    state::update(&app, |st| {
        st.companion.planets_hidden = hidden;
        Ok(())
    })?;
    habitat::set_planets_hidden(hidden);
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[cfg(unix)]
#[tauri::command]
fn needed_answer(
    app: tauri::AppHandle,
    id: String,
    answer: String,
) -> Result<state::KanbanCard, String> {
    board::needed_answer(&app, &id, &answer)
}

#[cfg(not(unix))]
#[tauri::command]
fn needed_answer(
    _app: tauri::AppHandle,
    _id: String,
    _answer: String,
) -> Result<state::KanbanCard, String> {
    Err("needed.answer: unix only".into())
}

/// The "if this again" row's Save.
/// Thin door: the rules.json write stays the python CLI's job
/// (`orbitty-needed rule add` — validation, 48 cap, atomic save, log);
/// this only builds argv and returns the minted rule, or the CLI's
/// refusal verbatim. It never answers the card — the normal
/// answer/dispatch path is untouched and no RULE: stanza is stuffed.
#[cfg(unix)]
#[tauri::command]
fn needed_rule_add(
    params: Vec<(String, String)>,
    answer: String,
    from_needed: Option<String>,
) -> Result<serde_json::Value, String> {
    needed_rules::rule_add(&params, &answer, from_needed.as_deref())
}

#[cfg(not(unix))]
#[tauri::command]
fn needed_rule_add(
    _params: Vec<(String, String)>,
    _answer: String,
    _from_needed: Option<String>,
) -> Result<serde_json::Value, String> {
    Err("needed.rule_add: unix only".into())
}

/// Deterministic roll: the Being editor's reroll mints a
/// seed, this maps it to a Kind+Palette (same function the habitat draws
/// from when a claim has no explicit look yet).
#[tauri::command]
fn roll_char_look(seed: u64) -> chars::CharLook {
    chars::look_from_seed(seed)
}

/// The Season 1 crew for the Being editor: 11 kinds,
/// each with its 4 palette names (default + 3 recolors). These are the only
/// silhouette controls; the old glyph-layer cyclers are gone.
#[tauri::command]
fn bitty_catalog() -> Vec<chars::BittyKind> {
    chars::bitty_catalog()
}

/// Fill/clamp a look's kind+palette (legacy
/// looks map via chars::resolve), so the Being editor always edits concrete
/// indices. Persisted only when the editor commits.
#[tauri::command]
fn normalize_char_look(look: chars::CharLook) -> chars::CharLook {
    let mut look = look;
    chars::normalize(&mut look);
    look
}

/// WYSIWYG preview: the same grid/palette the planet
/// draws, rasterized to a base64 PNG (macOS; the editor hides the preview
/// frame when this errors elsewhere).
#[tauri::command]
fn char_preview(look: chars::CharLook, px: u32) -> Result<String, String> {
    habitat::char_preview(look, px)
}

#[tauri::command]
fn accessibility_status() -> windows::AccessibilityStatus {
    windows::accessibility_status()
}

#[tauri::command]
fn open_accessibility_settings() {
    windows::open_accessibility_settings();
}

#[tauri::command]
fn prompt_accessibility() -> bool {
    windows::prompt_accessibility()
}

/// Bring the rail back. Always show + front; a rail that has drifted off
/// every display is re-homed on the primary (left inset).
/// The tray Inspector item is the caller — the rail
/// is the inspector, summoned on purpose (rail delete is forbidden).
/// Embedded pop-out also invokes this.
#[tauri::command]
fn show_rail(app: tauri::AppHandle) {
    show_rail_inner(&app);
}

pub(crate) fn show_rail_inner(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        drawers::rail_stage_apply(app, true);
        drawers::heal_rail(app);
        drawers::appkit_order_front(app, &window);
        let _ = window.set_focus();
    }
}

/// Machine rail edge + auto-hide. Not a planet pref. A later window
/// `save_state` keeps these (merge copies them from disk).
#[tauri::command]
fn set_rail_stage_prefs(
    app: tauri::AppHandle,
    edge: String,
    auto_hide_on_blur: bool,
) -> Result<(), String> {
    let edge = state::normalize_rail_edge(&edge).to_string();
    state::update(&app, |st| {
        st.rail_edge = edge.clone();
        st.rail_auto_hide_on_blur = auto_hide_on_blur;
        Ok(())
    })?;
    rail_edge::note_edge_prefs(&edge, auto_hide_on_blur);
    drawers::rail_stage_apply(&app, drawers::rail_stage_is_revealed());
    let _ = app.emit(
        "orbitty:rail-stage-prefs",
        serde_json::json!({
            "railEdge": edge,
            "railAutoHideOnBlur": auto_hide_on_blur,
        }),
    );
    Ok(())
}

/// Rail pointer, key, or wheel. Memory only — resets the starter glow idle clock.
#[tauri::command]
fn starter_note_act() {
    starter::note_act();
}

/// × / Esc on the coach line. The story slice is Rust-owned.
#[tauri::command]
fn set_starter_coach_done(app: tauri::AppHandle) -> Result<(), String> {
    starter::set_coach_done(&app)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Hover, hotkey, or auto-hide. `revealed` false parks the strip on the
/// preferred edge and leaves the sliver.
#[tauri::command]
fn rail_stage_set(
    app: tauri::AppHandle,
    revealed: bool,
    reason: Option<String>,
) -> Result<(), String> {
    drawers::rail_stage_set_reason(&app, revealed, reason.as_deref());
    Ok(())
}

#[tauri::command]
async fn window_thumb(
    id: String,
    title: String,
    pid: i32,
    focus_epoch: Option<u64>,
) -> Result<serde_json::Value, String> {
    let epoch = focus_epoch.unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || window_thumb::capture_thumb(&id, &title, pid, epoch))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn window_thumb_permission() -> serde_json::Value {
    serde_json::json!({ "granted": window_thumb::permission_granted() })
}

/// Rail Home button: order every planet panel front (peer order among
/// themselves is preserved; other apps keep their relative stacking).
#[tauri::command]
fn bring_all_front() {
    habitat::bring_all_front();
}

/// Planets popup: order one planet's panel front — that disc only.
/// Unknown ids are a native no-op.
#[tauri::command]
fn bring_planet_front(id: String) {
    habitat::bring_planet_front(&id);
}

/// The Kanban board lives in its own window, never in the rail column.
/// Created on first open (transparent, so the opacity
/// slider applies there too), then kept warm — this shows + focuses it.
/// Same SPA bundle as the rail — the frontend mounts KanbanPage when the
/// window label matches.
/// The 780×520 pane is shared — this shows it
/// in board mode (KanbanPage).
#[tauri::command]
fn open_kanban(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_shared_pane(&app, "board")
}

/// Settings → Appearance → Herding Cats… pops the
/// confirm into its own small window (same trick as Settings), so ⌘⇧Y
/// works with Settings closed. Close / × / Esc hides, never destroys.
/// While the clowder is out, the Settings row is
/// a scoop-all (the exits table: Esc, ⌘⇧Y, Settings while on) — the pane
/// and the yard never coexist.
#[tauri::command]
fn open_yard_warn_window(app: tauri::AppHandle) -> Result<(), String> {
    if yard::active() {
        return yard::stop();
    }
    panes::show_labeled(&app, "yard-warn")
}

/// The warn pane's Drop button. Rains the
/// current unclaimed set as bitty cats (one overlay per display). No-op
/// on an already-on yard — never double-spawn.
#[tauri::command]
fn yard_start(app: tauri::AppHandle) -> Result<(), String> {
    yard::start(app)
}

/// Scoop all: windows unchanged, overlay gone, meow killed. Idempotent.
#[tauri::command]
fn yard_stop() -> Result<(), String> {
    yard::stop()
}

/// Warn-pane / Settings truth: is the clowder out?
#[tauri::command]
fn yard_status() -> bool {
    yard::active()
}

/// own small card window. Dock pip and
/// ⌘⇧N toggle. Click-off / Esc / × hide. Not a dest chip.
#[tauri::command]
fn toggle_next_card_window(app: tauri::AppHandle) -> Result<(), String> {
    let up = app
        .get_webview_window("next-card")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if up {
        panes::hide_window(app, "next-card".to_string())
    } else {
        panes::show_labeled(&app, "next-card")
    }
}

#[tauri::command]
fn toggle_cheatsheet(app: tauri::AppHandle) -> Result<(), String> {
    panes::toggle_cheatsheet(app)
}

#[tauri::command]
fn plans_settings() -> Result<serde_json::Value, String> {
    crate::plans::settings_snapshot()
}

#[tauri::command]
fn plans_reveal() -> Result<(), String> {
    crate::plans::reveal_plans_dir()
}

#[tauri::command]
fn plans_set_dir(path: String) -> Result<serde_json::Value, String> {
    crate::plans::set_plans_dir(&path)
}

#[tauri::command]
fn plans_set_runner(
    runner: String,
    harness: String,
    model: String,
    permissions: Option<String>,
) -> Result<serde_json::Value, String> {
    crate::plans::set_runner_choice(&runner, &harness, &model, permissions.as_deref())
}

#[tauri::command]
fn agent_pickup(
    app: tauri::AppHandle,
    id: Option<String>,
    name: Option<String>,
    card_id: Option<String>,
) -> Result<serde_json::Value, String> {
    agents::agent_pickup(
        &app,
        id.as_deref(),
        name.as_deref(),
        card_id.as_deref(),
    )
}

#[tauri::command]
fn undo_task(app: tauri::AppHandle, card_id: String) -> Result<(), String> {
    crate::task::undo(&app, &card_id)
}

#[tauri::command]
fn set_workspace_root(app: tauri::AppHandle, project_id: String, path: String) -> Result<(), String> {
    crate::task::set_workspace_root(&app, &project_id, &path)
}

#[tauri::command]
fn task_has_before(app: tauri::AppHandle, card_id: String) -> bool {
    crate::task::has_before(&app, &card_id)
}

/// Run `f` on the main thread and BLOCK until it finishes. Inline when
/// already on the main thread (dispatch_sync of the main queue from the
/// main thread deadlocks). A panic inside `f` is swallowed — a native
/// hiccup must never take down the caller.
#[cfg(target_os = "macos")]
pub(crate) fn main_thread_sync<F: FnOnce() + Send>(f: F) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    let is_main: bool = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main {
        f();
        return;
    }
    extern "C" {
        // The SDK header makes dispatch_get_main_queue() an inline over
        // the exported `_dispatch_main_q` global, so the function symbol
        // itself is not linkable — bind the global (same main queue).
        static _dispatch_main_q: c_void;
        fn dispatch_sync_f(
            queue: *mut c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }
    extern "C" fn trampoline<F: FnOnce()>(ctx: *mut c_void) {
        let f = unsafe { Box::from_raw(ctx as *mut F) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    }
    let ctx = Box::into_raw(Box::new(f)) as *mut c_void;
    unsafe {
        dispatch_sync_f(
            std::ptr::addr_of!(_dispatch_main_q) as *mut c_void,
            ctx,
            trampoline::<F>,
        );
    }
}

/// Same trampoline as `main_thread_sync`, but it never waits. The 10Hz
/// companion poll uses this so a nested NSMenu run loop cannot beach-ball
/// behind a `dispatch_sync` from a worker.
#[cfg(target_os = "macos")]
pub(crate) fn main_thread_async<F: FnOnce() + Send + 'static>(f: F) {
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    let is_main: bool = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main {
        f();
        return;
    }
    extern "C" {
        static _dispatch_main_q: c_void;
        fn dispatch_async_f(
            queue: *mut c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }
    extern "C" fn trampoline<F: FnOnce()>(ctx: *mut c_void) {
        let f = unsafe { Box::from_raw(ctx as *mut F) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    }
    let ctx = Box::into_raw(Box::new(f)) as *mut c_void;
    unsafe {
        dispatch_async_f(
            std::ptr::addr_of!(_dispatch_main_q) as *mut c_void,
            ctx,
            trampoline::<F>,
        );
    }
}

/// Re-apply the transparency surgery on the overlay windows. The
/// frontend invokes this on mount (post-load), so a WKWebView that came up
/// with the default white backing gets its drawsBackground killed for real.
/// Drawers take the same transparent path as the
/// rail — never an opaque slab — so --bg-fill alpha shows the desk.
#[tauri::command]
fn ensure_glass(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // AppKit view surgery must happen on the main thread.
        let handle = app.clone();
        app.run_on_main_thread(move || {
            let ctx = window_chrome::lights_ctx(&handle);
            for label in ["main", "kanban", "companion", "stars", "settings", "sky", "cheatsheet"] {
                if let Some(w) = handle.get_webview_window(label) {
                    glass::ensure_transparent(&w);
                    window_chrome::apply_lights(&w, label, ctx);
                    // Rail only: planet-panel pin so Show Desktop cannot
                    // tuck it. Chrome drawers are normal windows — unpin
                    // in case a previous recipe left them stationary.
                    if label == "main" {
                        glass::pin_stay_visible(&w);
                        glass::no_shadow(&w);
                        glass::stage_borderless(&w);
                    } else {
                        glass::unpin_stay_visible(&w);
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    Ok(())
}

/// Kept only so the frontend can explicitly clear any vibrancy material
/// (S4.2 retired the Clear theme; the frontend always passes false).
/// No-op off macOS.
#[tauri::command]
fn set_vibrancy(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{
            apply_vibrancy, clear_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
        };
        // AppKit view surgery must happen on the main thread.
        let handle = app.clone();
        app.run_on_main_thread(move || {
            let Some(window) = handle.get_webview_window("main") else {
                return;
            };
            // Idempotent: always clear first so repeat applies never stack views.
            let _ = clear_vibrancy(&window);
            if enabled {
                if let Err(e) = apply_vibrancy(
                    &window,
                    NSVisualEffectMaterial::HudWindow,
                    Some(NSVisualEffectState::Active),
                    Some(14.0),
                ) {
                    eprintln!("set_vibrancy: apply failed: {e}");
                }
            }
        })
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, enabled);
    Ok(())
}

#[tauri::command]
fn load_state(app: tauri::AppHandle) -> Result<state::PersistedState, String> {
    let mut st = state::load_state(&app)?;
    crate::todo_trees::reconcile(&mut st);
    Ok(st)
}

#[tauri::command(rename = "save_state")]
fn save_window_state(
    app: tauri::AppHandle,
    state: state::PersistedState,
    origin: Option<String>,
) -> Result<(), String> {
    // Window writers merge under the lock. Rust-owned slices
    // (agentRuns, companion.hack*, the carry-list fields) stay on disk.
    // Cards merge by lastTouchedAt; a newer tombstone is the only delete.
    let (before, after, stats) = state::update(&app, |disk| {
        let before = disk.clone();
        let (mut merged, stats) = state::merge_window_save(disk, &state);
        #[cfg(unix)]
        crate::agent_runs::reconcile_incoming(&mut merged);
        crate::todo_trees::reconcile(&mut merged);
        *disk = merged.clone();
        Ok((before, merged, stats))
    })?;
    event_log::append_best(
        &app,
        "state-save-merge",
        None,
        None,
        None,
        true,
        &serde_json::json!({
            "window": true,
            "kept_disk": stats.kept_disk,
            "took_incoming": stats.took_incoming,
        }),
    );
    event_log::diff_claims(&app, &before.claims, &after.claims);
    starter::after_save(
        &app,
        starter::view(&after),
        starter::save_origin(origin.as_deref()),
    );
    // After the save returns so the
    // board column is not waiting on Needed mint + extra saves.
    #[cfg(unix)]
    doing_fire::spawn_fire_and_attach(&app, before, after.clone());
    // Keep the separate Kanban window in sync with the rail's writes.
    let _ = app.emit("state:changed", ());
    // Every persist re-syncs action combo
    // registrations (best-effort — a collision marks conflict, never errs).
    actions::sync_combos(&app, &after.action_shortcuts);
    Ok(())
}


/// PEER-CHROME door — Settings toggle / demo graduation. Whole-state
/// save_state strip-traps this flag so rail writers cannot wipe it.
#[tauri::command]
fn set_peer_chrome(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    state::update(&app, |st| {
        st.peer_chrome = on;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                drawers::clear_pane_mounted(webview.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_windows,
            focus_window,
            minimize_window,
            restore_window,
            close_window,
            locate_window,
            locate_walker,
            inspect_droid,
            locate_planet,
            project_rm,
            sync_habitat,
            accessibility_status,
            open_accessibility_settings,
            prompt_accessibility,
            load_state,
            save_window_state,
            set_peer_chrome,
            event_log::log_event,
            set_vibrancy,
            open_kanban,
            open_yard_warn_window,
            yard_start,
            yard_stop,
            yard_status,
            toggle_next_card_window,
            toggle_cheatsheet,
            rail_toggle::toggle_labeled,
            rail_toggle::dest_toggle,
            rail_toggle::dest_show,
            rail_toggle::dest_open_state,
            drawers::companion_drag_begin,
            drawers::pane_drag_begin,
            drawers::pane_mounted,
            companion::companion_frames,
            companion::companion_glb_url,
            panes::hide_window,
            panes::arm_companion_mouth,
            panes::set_companion_host,
            panes::set_companion_composer_grow,
            panes::set_companion_host_hug,
            panes::take_board_focus,
            show_rail,
            set_rail_stage_prefs,
            set_starter_coach_done,
            starter_note_act,
            rail_stage_set,
            window_thumb,
            window_thumb_permission,
            set_planets_hidden,
            needed_answer,
            needed_rule_add,
            plans_settings,
            plans_reveal,
            plans_set_dir,
            plans_set_runner,
            agent_pickup,
            bring_all_front,
            bring_planet_front,
            ensure_glass,
            window_chrome::window_appearance,
            window_chrome::window_title_dblclick,
            packs::list_packs,
            packs::list_companion_packs,
            packs::import_pack_folder,
            packs::import_pack_zip,
            packs::import_pack_image,
            packs::remove_pack,
            packs::packs_dir,
            roll_char_look,
            bitty_catalog,
            normalize_char_look,
            char_preview,
            coordinator::launch_coordinator,
            coordinator::companion_send,
            attach::companion_attach_stat,
            attach::companion_attach_pick,
            coordinator::companion_agents,
            coordinator::set_companion_agent,
            companion_backend::companion_harnesses,
            companion_backend::set_companion_backend,
            direct_lane::direct_key_status,
            direct_lane::set_direct_key,
            direct_lane::set_direct_endpoint,
            coordinator::set_companion_mode,
            interrupt_gate::interrupt_gate_set_armed,
            interrupt_gate::interrupt_gate_status,
            interrupt_gate::interrupt_gate_clear_freeze,
            interrupt_gate::interrupt_gate_inject,
            coordinator::companion_thin,
            coordinator::companion_now,
            coordinator::set_companion_pack,
            coordinator::set_companion_expanded,
            coordinator::set_companion_view,
            coordinator::set_companion_sprite,
            coordinator::set_companion_speak,
            coordinator::set_companion_speak_volume,
            coordinator::set_companion_captions,
            coordinator::set_companion_talk_source,
            mouth_hop::set_companion_free_mode,
            mouth_hop::set_companion_reply_lane,
            mouth_hop::set_companion_reply_model,
            mouth_hop::set_companion_handoff_backend,
            mouth_hop::set_companion_auto_handoff,
            mouth_hop::companion_handoff,
            mouth_scan::handoff_backends,
            mouth_hop::set_openrouter_key,
            mouth_hop::openrouter_key_status,
            mouth_scan::mouth_scan,
            mouth_scan::pull_mouth_model,
            mouth_ack::set_companion_conductor,
            mouth_ack::set_companion_conductor_model,
            talk::talk_start,
            talk::talk_stop,
            coordinator::set_stars_prompts,
            coordinator::ack_stars_prompt,
            coordinator::speak_reply,
            coordinator::run_action,
            undo_task,
            claim_front::claim_front_window,
            claim_front::claim_front_undo,
            claim_front::claim_front_change,
            claim_front::claim_front_to,
            claim_front::claim_front_unclaim,
            set_workspace_root,
            task_has_before,
            coordinator::wake_sleeper,
            coordinator::load_soul,
            coordinator::save_soul,
            coordinator::soul_path,
            coordinator::open_soul_in_editor,
            card_links::open_card_link,
            card_links::click_link,
            card_links::open_link,
            card_links::reveal_link,
            card_links::list_link_approvals,
            card_links::remove_link_approval,
            card_links::clear_link_approvals,
        ])
        .setup(|app| {
            // one-time 0–4 → 1–5 checkin shift
            // (scaleV flag — never twice), before any window reads state.
            state::migrate_stars_scale(app.handle());
            starter::boot(app.handle());

            // One-time backfill — exact-token
            // GO-ORBITTY-* todos with empty notes get the locked one-liner
            // (never overwrites a note Matt typed). Best-effort.
            board::backfill_go_card_notes(app.handle());
            crate::task::boot(app.handle());

            // Local Unix control socket (0600)
            // serving the same handlers as invoke. Bind failure is logged
            // once and the app runs on without it.
            #[cfg(unix)]
            control::start(app.handle());
            #[cfg(unix)]
            shift::boot(app.handle());

            habitat::start(app.handle().clone());
            coordinator::heal_stuck_hack(app.handle());

            // Companion z-order poll (10Hz): shown pane stays floating.
            companion_clear::start(app.handle().clone());
            nextcard::bind(app.handle().clone());

            // S4.3: pin both webview windows to a fully transparent native
            // background so the JS rgba chrome fills (not an opaque NSColor)
            // decide what the wallpaper shows through.
            // S4.4: plus explicit surgery on the live NSWindow/WKWebView —
            // the config-level chain alone still washed white/grey.
            #[cfg(target_os = "macos")]
            {
                let boot_ctx = window_chrome::lights_ctx(app.handle());
                for label in ["main", "kanban", "companion", "stars", "settings", "sky", "cheatsheet"] {
                    if let Some(w) = app.get_webview_window(label) {
                        // One recipe for every pane —
                        // clear background + transparency surgery (the rail
                        // recipe); drawers never get an opaque slab.
                        let _ = w.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
                        glass::ensure_transparent(&w);
                        window_chrome::apply_lights(&w, label, boot_ctx);
                        if label == "main" {
                            glass::pin_stay_visible(&w);
                            glass::no_shadow(&w);
                            glass::stage_borderless(&w);
                        } else {
                            glass::unpin_stay_visible(&w);
                        }
                    }
                }
            }

            // Sprite-pack drop folder + README (local import only).
            packs::ensure_packs_dir(app.handle());

            // ⌘⇧O reveals the inspector rail (Matt locked 2026-10-05).
            // Companion no longer owns this chord. Tray, dock Reopen, and
            // the Actions ⌥⌘O row still open the companion.
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };
                let summon = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyO);
                let handle = app.handle().clone();
                app.global_shortcut()
                    .on_shortcut(summon, move |_app, _shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        drawers::rail_stage_toggle(&handle);
                    })?;

                // ⌘⇧Y opens the Herding Cats confirm
                // (its own small window — works with Settings closed); a
                // press while the confirm is up scoops it (hide = Cancel).
                // While the clowder is out, ⌘⇧Y
                // scoops all with no second warning (the exits table) —
                // the pane toggle resumes once the yard is off. ⌘⇧O is the
                // rail reveal — never retargeted here; actions.rs reserves both chords.
                let yard_warn = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyY);
                let yard_handle = app.handle().clone();
                app.global_shortcut()
                    .on_shortcut(yard_warn, move |_app, _shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        if yard::active() {
                            let _ = yard::stop();
                            return;
                        }
                        let up = yard_handle
                            .get_webview_window("yard-warn")
                            .map(|w| w.is_visible().unwrap_or(false))
                            .unwrap_or(false);
                        if up {
                            let _ = panes::hide_window(yard_handle.clone(), "yard-warn".to_string());
                        } else {
                            let _ = panes::show_labeled(&yard_handle, "yard-warn");
                        }
                    })?;

                // ⌘⇧N toggles the next-card
                // window. Reserved in actions.rs so a user combo cannot steal it.
                let next_card = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyN);
                let next_handle = app.handle().clone();
                app.global_shortcut()
                    .on_shortcut(next_card, move |_app, _shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        let _ = toggle_next_card_window(next_handle.clone());
                    })?;
            }

            // ⌘⌥1/2/3 = the dest chips (drop /
            // yaw / dismiss toggle), ⌘⌥←/→ yaw the ring while hosted.
            // Registered before action combos so a colliding user combo
            // loses (actions.rs also refuses these as reserved).
            tilechords::register(app.handle());

            // Register persisted action combos on
            // load (same plugin as the summon above). Every later persist
            // re-syncs via the save_state command / action.* tools.
            // Seed the real first-run set and
            // drop the TextEdit toys first, so a fresh install boots with
            // the companion/kanban/stars chords, not empty/toys.
            let boot_actions = actions::bake_defaults(app.handle());
            actions::sync_combos(app.handle(), &boot_actions);

            // All webview windows: any close request (⌘W, menu Close Window,
            // programmatic close) hides instead of destroying. Quit lives on
            // the tray menu alone; windows (and their transparency) survive
            // reopens. The companion additionally persists its bounds
            // (throttled) on move/resize and its visibility on hide —
            // restore-on-launch reads that slice. Only
            // the rail exists at launch; lazy panes get the same wiring from
            // ensure_window at create time.
            for label in ["main", "kanban", "companion", "stars", "settings", "sky", "cheatsheet"] {
                if let Some(window) = app.get_webview_window(label) {
                    panes::wire_pane_events(app.handle(), &window, label == "companion");
                    drawers::wire_drawer_events(app.handle(), &window, label);
                }
            }

            // Build the three chrome drawer
            // webviews now — still hidden, still click-through — so the
            // first rail click is show + orderFront of an already-created
            // WKWebView, not a WebviewWindowBuilder on the click path
            // (that create, plus the mount-check reload of a hydrating
            // view, was the first-show beach ball). No show(), no
            // companion pin: visibility and the pin are still earned only
            // by a real show_drawer. STARS shares the kanban pane; sky
            // stays lazy. ensure_window wires events + glass itself, so
            // this must stay AFTER the rail-only wiring loops above.
            // Park each fresh hidden pane on a real
            // display through the same honor-or-slot law as show_drawer —
            // Tauri's hidden create leaves it on an arbitrary monitor
            // (this desk: s2 @2x) and every later place inherited that
            // wrong-scale origin (the click-hop). Still visible:false,
            // still click-through, no show(), no companion pin.
            for label in ["companion", "kanban", "settings", "stars"] {
                if let Err(e) = panes::ensure_window(app.handle(), label) {
                    eprintln!("pane-warm: ensure_window({label}) failed: {e}");
                    continue;
                }
                drawers::park_warm_drawer(app.handle(), label);
            }

            // The companion launches with the
            // app — show when `visible` is true OR missing (state.rs defaults
            // a missing key to true, so first run opens it). After that the
            // native hide/show handlers persist the last choice. Bounds come
            // along inside show_companion.
            if state::load_state(app.handle())
                .map(|s| s.companion.visible)
                .unwrap_or(false)
            {
                let _ = panes::show_companion(app.handle());
            }

            // The tray is a remote for the
            // companion, not the rail. Left click raises the companion
            // (caret in the field). ⌘⇧O reveals the rail instead. The right-click menu is
            // chrome — Launchpad/STARS enter the companion's host well,
            // never the kanban/stars overlay.
            let claim_front_i = MenuItem::with_id(
                app,
                "claim_front",
                "Claim front window",
                true,
                None::<&str>,
            )?;
            let claim_undo_i = MenuItem::with_id(
                app,
                "claim_undo",
                "Undo last claim",
                true,
                None::<&str>,
            )?;
            let show_companion_i = MenuItem::with_id(
                app,
                "show_companion",
                "Show Companion",
                true,
                None::<&str>,
            )?;
            let hide_dock_i = MenuItem::with_id(
                app,
                "hide_dock",
                "Hide Dock",
                true,
                None::<&str>,
            )?;
            let launchpad_i =
                MenuItem::with_id(app, "launchpad", "Launchpad", true, None::<&str>)?;
            let stars_i = MenuItem::with_id(app, "stars", "STARS", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let inspector_i =
                MenuItem::with_id(app, "inspector", "Inspector", true, None::<&str>)?;
            let restore_i = MenuItem::with_id(
                app,
                "restore_all",
                "Restore All Stowed Windows",
                true,
                None::<&str>,
            )?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Orbitty", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &claim_front_i,
                    &claim_undo_i,
                    &show_companion_i,
                    &hide_dock_i,
                    &launchpad_i,
                    &stars_i,
                    &settings_i,
                    &inspector_i,
                    &restore_i,
                    &quit_i,
                ],
            )?;

            let tray_icon = tray_image(menu_bar_is_dark());
            let tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(false)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "claim_front" => {
                        if let Err(e) = claim_front::claim_front(app, &claim_front::ClaimOrigin::User)
                        {
                            eprintln!("claim-front: {e}");
                        }
                    }
                    "claim_undo" => {
                        if let Err(e) = claim_front::undo_last(app, &claim_front::ClaimOrigin::User)
                        {
                            eprintln!("claim-undo: {e}");
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    "show_companion" => {
                        // Second click hides —
                        // talk-stop + persist hidden ride hide_window.
                        // The caret summon fires on the show edge only;
                        // left-click / dock Reopen stay raise-only
                        // summons (FRONT-HOLD). ⌘⇧O reveals the rail.
                        if let Ok(true) = rail_toggle::toggle(app, "companion") {
                            let _ = app.emit_to("companion", "orbitty:companion-summon", ());
                        }
                    }
                    "hide_dock" => {
                        let _ = panes::hide_window(app.clone(), "companion".to_string());
                    }
                    // Raise-only windows — dest chips on the dock toggle.
                    mode @ ("launchpad" | "stars") => {
                        let pane = if mode == "stars" { "stars" } else { "board" };
                        let _ = panes::show_shared_pane(app, pane);
                    }
                    "settings" => {
                        // Same rule as the dock
                        // gear — a click while visible hides.
                        let _ = rail_toggle::toggle(app, "settings");
                    }
                    "inspector" => {
                        show_rail_inner(app);
                    }
                    "restore_all" => {
                        habitat::restore_all();
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let _ = panes::show_companion(app);
                        let _ = app.emit_to("companion", "orbitty:companion-summon", ());
                    }
                })
                .build(app)?;

            std::thread::spawn(move || {
                let mut last = menu_bar_is_dark();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(900));
                    let dark = menu_bar_is_dark();
                    if dark != last {
                        last = dark;
                        let _ = tray.set_icon(Some(tray_image(dark)));
                        let _ = tray.set_icon_as_template(false);
                    }
                }
            });

            #[cfg(target_os = "macos")]
            claim_front::install_orbitty_dock_menu(app.handle().clone());
            windows::install_frontmost_tracker();

            // Stage shell starts parked off the preferred edge (default left).
            drawers::rail_stage_apply(app.handle(), false);
            rail_edge::start(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Orbitty")
        .run(|app_handle, event| {
            match event {
                // Dock icon click: raise the companion with the caret in
                // the field (same as tray left-click), even when it
                // is already visible (macOS otherwise no-ops). ⌘⇧O reveals the rail.
                tauri::RunEvent::Reopen { .. } => {
                    let _ = panes::show_companion(app_handle);
                    let _ = app_handle.emit_to("companion", "orbitty:companion-summon", ());
                }
                #[cfg(unix)]
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    // Kill the meow + tear the
                    // overlay on quit (boombox-kill law — no orphan audio).
                    let _ = yard::stop();
                    habitat::persist_now();
                    if drawers::drawer_rt()
                        .lock()
                        .map(|rt| rt.dirty)
                        .unwrap_or(false)
                    {
                        drawers::drawer_persist(app_handle);
                    }
                    control::cleanup();
                }
                _ => {}
            }
        });
}

// Keep unused import quiet if feature sets differ.
#[allow(dead_code)]
fn _runtime_marker<R: Runtime>(_app: &tauri::AppHandle<R>) {}
