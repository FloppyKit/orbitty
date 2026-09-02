mod chars;
mod companion;
mod companion_clear;
#[cfg(unix)]
mod control;
mod coordinator;
mod drawers;
mod event_log;
mod glass;
mod panes;
mod habitat;
mod packs;
mod state;
mod talk;
mod windows;

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

/// GO-ORBITTY-RAIL-ROW-MENU — rail row left-click: an Orbitty-owned,
/// ignore-mouse, non-activating ring around the window's AX frame (~500ms).
/// Never focuses, never AX-raises.
#[tauri::command]
fn locate_window(id: String) -> Result<(), String> {
    windows::locate_window(&id)
}

/// GO-ORBITTY-RAIL-ROW-MENU — `habitat:locate` for a claimed window: stroke
/// the walker's frame on its planet for ~500ms (glow only).
#[tauri::command]
fn locate_walker(app: tauri::AppHandle, entry_id: String) -> Result<(), String> {
    habitat::locate_walker(app, entry_id)
}

/// Frontend pushes the claimed-project snapshot; native side owns the panels.
#[tauri::command]
fn sync_habitat(app: tauri::AppHandle, snapshot: habitat::HabitatSnapshot) -> Result<(), String> {
    habitat::sync(&app, snapshot)
}

/// GO-ORBITTY-CHARS — deterministic roll: the Being editor's reroll mints a
/// seed, this maps it to a Kind+Palette (same function the habitat draws
/// from when a claim has no explicit look yet).
#[tauri::command]
fn roll_char_look(seed: u64) -> chars::CharLook {
    chars::look_from_seed(seed)
}

/// GO-ORBITTY-S1-BITTYS — the Season 1 crew for the Being editor: 11 kinds,
/// each with its 4 palette names (default + 3 recolors). These are the only
/// silhouette controls; the old glyph-layer cyclers are gone.
#[tauri::command]
fn bitty_catalog() -> Vec<chars::BittyKind> {
    chars::bitty_catalog()
}

/// GO-ORBITTY-S1-BITTYS — fill/clamp a look's kind+palette (legacy pre-S1
/// looks map via chars::resolve), so the Being editor always edits concrete
/// indices. Persisted only when the editor commits.
#[tauri::command]
fn normalize_char_look(look: chars::CharLook) -> chars::CharLook {
    let mut look = look;
    chars::normalize(&mut look);
    look
}

/// GO-ORBITTY-CHARS — WYSIWYG preview: the same grid/palette the planet
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
/// GO-ORBITTY-INSPECTOR: the tray Inspector item is the caller — the rail
/// is the inspector, summoned on purpose (rail delete is forbidden).
fn show_rail(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        drawers::heal_rail(app);
        drawers::appkit_order_front(app, &window);
        let _ = window.set_focus();
    }
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

/// S4.2: the Kanban board lives in its own window, never in the rail column.
/// GO-ORBITTY-SLIM-PANES: created on first open (transparent, so the opacity
/// slider applies there too), then kept warm — this shows + focuses it.
/// Same SPA bundle as the rail — the frontend mounts KanbanPage when the
/// window label matches.
/// GO-ORBITTY-BOARD-STARS: the 780×520 pane is now shared — this shows it
/// in board mode (KanbanPage).
#[tauri::command]
fn open_kanban(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_shared_pane(&app, "board")
}

/// GO-LS-TAURI-AVATAR-LARGE — same trick as Lists: the companion window is
/// created on first open (slim-panes); this shows + focuses it, applying
/// persisted bounds or the first-show default (~⅓ of the display at the
/// trailing edge; a portrait display maximizes). Persists visible=true.
#[tauri::command]
fn open_companion(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_companion(&app)
}

/// GO-ORBITTY-BOARD-STARS (supersedes STARS S1.1's own-window pop-out):
/// STARS mounts inside the one shared 780×520 `kanban` overlay — this
/// shows that pane in stars mode (StarsPage). The legacy 420×560 `stars`
/// pane is never spawned here; if one is somehow live it hides, never
/// destroys. Close / × hides, never destroys.
#[tauri::command]
fn open_stars(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_shared_pane(&app, "stars")
}

/// GO-ORBITTY-SETTINGS-WIN: gear pops Settings into its own window.
/// Close / × hides, never destroys. Same SPA bundle — frontend mounts
/// SettingsPage when the window label matches.
#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_labeled(&app, "settings")
}

/// GO-ORBITTY-WORD-SKY: the word sky pops into its own window, same trick
/// as STARS — created on first open (slim-panes); this shows + focuses it.
/// Close / × hides, never destroys. Same SPA bundle — the frontend mounts
/// SkyPage when the window label matches. Deliberately no ctl/MCP verb.
#[tauri::command]
fn open_sky_window(app: tauri::AppHandle) -> Result<(), String> {
    panes::show_labeled(&app, "sky")
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

/// S4.4: re-apply the transparency surgery on the overlay windows. The
/// frontend invokes this on mount (post-load), so a WKWebView that came up
/// with the default white backing gets its drawsBackground killed for real.
/// GO-ORBITTY-PANE-MATCH: drawers take the same transparent path as the
/// rail — never an opaque slab — so --bg-fill alpha shows the desk.
#[tauri::command]
fn ensure_glass(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // AppKit view surgery must happen on the main thread.
        let handle = app.clone();
        app.run_on_main_thread(move || {
            for label in ["main", "kanban", "companion", "stars", "settings", "sky"] {
                if let Some(w) = handle.get_webview_window(label) {
                    glass::ensure_transparent(&w);
                    glass::hide_traffic_lights(&w);
                    // Rail only: planet-panel pin so Show Desktop cannot
                    // tuck it. Chrome drawers are normal windows — unpin
                    // in case a previous recipe left them stationary.
                    if label == "main" {
                        glass::pin_stay_visible(&w);
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
    state::load_state(&app)
}

#[tauri::command]
fn save_state(app: tauri::AppHandle, state: state::PersistedState) -> Result<(), String> {
    // GO-ORBITTY-EVENT-LOG: this command is the one native writer of
    // `claims` — diff disk-before against the incoming snapshot and emit
    // claim/unclaim (best-effort; a dead log never fails the save). A
    // missing/corrupt old file skips the diff rather than inventing churn.
    let before = state::load_state(&app).ok();
    state::save_state(&app, &state)?;
    if let Some(before) = before {
        event_log::diff_claims(&app, &before.claims, &state.claims);
    }
    // Keep the separate Kanban window in sync with the rail's writes.
    let _ = app.emit("state:changed", ());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_windows,
            focus_window,
            minimize_window,
            restore_window,
            close_window,
            locate_window,
            locate_walker,
            sync_habitat,
            accessibility_status,
            open_accessibility_settings,
            prompt_accessibility,
            load_state,
            save_state,
            event_log::log_event,
            set_vibrancy,
            open_kanban,
            open_companion,
            open_stars,
            open_settings_window,
            open_sky_window,
            drawers::retether_drawer,
            drawers::retether_all_drawers,
            companion::companion_frames,
            panes::hide_window,
            panes::set_companion_host,
            companion_clear::set_companion_hits,
            companion_clear::companion_front,
            bring_all_front,
            bring_planet_front,
            ensure_glass,
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
            coordinator::companion_agents,
            coordinator::set_companion_agent,
            coordinator::set_companion_pack,
            coordinator::set_companion_expanded,
            coordinator::set_companion_view,
            coordinator::set_companion_sprite,
            coordinator::set_companion_speak,
            coordinator::set_companion_talk_source,
            talk::talk_start,
            talk::talk_stop,
            coordinator::set_stars_prompts,
            coordinator::ack_stars_prompt,
            coordinator::speak_reply,
            coordinator::run_action,
            coordinator::launch_agent,
            coordinator::wake_sleeper,
            coordinator::load_soul,
            coordinator::save_soul,
            coordinator::soul_path,
            coordinator::open_soul_in_editor,
        ])
        .setup(|app| {
            // GO-LS-TAURI-CUTOVER: one-time app-support migration
            // (app.livesprite.mac → space.orbitty.mac) before anything
            // reads/writes state under the new identifier.
            state::migrate_app_support(app.handle());

            // GO-ORBITTY-CONTROL-PLANE-0: local Unix control socket (0600)
            // serving the same handlers as invoke. Bind failure is logged
            // once and the app runs on without it.
            #[cfg(unix)]
            control::start(app.handle());

            habitat::start(app.handle().clone());

            // GO-ORBITTY-COMPANION-CLEAR: ghost click-through poll for the
            // idle collapsed companion (10Hz, idles while hidden/slab).
            companion_clear::start(app.handle().clone());

            // S4.3: pin both webview windows to a fully transparent native
            // background so the JS rgba chrome fills (not an opaque NSColor)
            // decide what the wallpaper shows through.
            // S4.4: plus explicit surgery on the live NSWindow/WKWebView —
            // the config-level chain alone still washed white/grey.
            #[cfg(target_os = "macos")]
            for label in ["main", "kanban", "companion", "stars", "settings", "sky"] {
                if let Some(w) = app.get_webview_window(label) {
                    // GO-ORBITTY-PANE-MATCH: one recipe for every pane —
                    // clear background + transparency surgery (the rail
                    // recipe); drawers never get an opaque slab.
                    let _ = w.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
                    glass::ensure_transparent(&w);
                    glass::hide_traffic_lights(&w);
                    if label == "main" {
                        glass::pin_stay_visible(&w);
                    } else {
                        glass::unpin_stay_visible(&w);
                    }
                }
            }

            // S4.4: sprite-pack drop folder + README (local import only).
            packs::ensure_packs_dir(app.handle());

            // GO-ORBITTY-SUMMON: ⌘⇧O raises the companion and focuses its
            // type field. Not a toggle — an already-visible companion stays
            // up and just takes focus; "main" is never touched here.
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
                        let _ = panes::show_companion(&handle);
                        let _ = handle.emit_to("companion", "orbitty:companion-summon", ());
                    })?;
            }

            // All webview windows: any close request (⌘W, menu Close Window,
            // programmatic close) hides instead of destroying. Quit lives on
            // the tray menu alone; windows (and their transparency) survive
            // reopens. The companion additionally persists its bounds
            // (throttled) on move/resize and its visibility on hide —
            // restore-on-launch reads that slice. GO-ORBITTY-SLIM-PANES: only
            // the rail exists at launch; lazy panes get the same wiring from
            // ensure_window at create time.
            for label in ["main", "kanban", "companion", "stars", "settings", "sky"] {
                if let Some(window) = app.get_webview_window(label) {
                    panes::wire_pane_events(app.handle(), &window, label == "companion");
                    drawers::wire_drawer_events(app.handle(), &window, label);
                }
            }

            // GO-ORBITTY-RAIL-DRAWER: docked panes follow-translate the rail
            // (Moved; Resized too — the inboard edge moves with rail width).
            if let Some(main) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if matches!(
                        event,
                        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                    ) {
                        drawers::follow_docked_panes(&handle);
                    }
                });
            }

            // GO-ORBITTY-PANE-WARM: build the three chrome drawer
            // webviews now — still hidden, still click-through — so the
            // first rail click is show + orderFront of an already-created
            // WKWebView, not a WebviewWindowBuilder on the click path
            // (that create, plus the mount-check reload of a hydrating
            // view, was the first-show beach ball). No show(), no
            // companion pin: visibility and the pin are still earned only
            // by a real show_drawer. STARS shares the kanban pane; sky
            // stays lazy. ensure_window wires events + glass itself, so
            // this must stay AFTER the rail-only wiring loops above.
            // GO-ORBITTY-PANE-STAY: park each fresh hidden pane on a real
            // display through the same honor-or-slot law as show_drawer —
            // Tauri's hidden create leaves it on an arbitrary monitor
            // (this desk: s2 @2x) and every later place inherited that
            // wrong-scale origin (the click-hop). Still visible:false,
            // still click-through, no show(), no companion pin.
            for label in ["companion", "kanban", "settings"] {
                if let Err(e) = panes::ensure_window(app.handle(), label) {
                    eprintln!("pane-warm: ensure_window({label}) failed: {e}");
                    continue;
                }
                drawers::park_warm_drawer(app.handle(), label);
            }

            // GO-LS-TAURI-AVATAR-LARGE S2.1: the companion launches with the
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

            // GO-ORBITTY-TRAY-REMOTE: the tray is a remote for the
            // companion, not the rail. Left click raises the companion
            // (caret in the field, same as ⌘⇧O); the right-click menu is
            // chrome — Launchpad/STARS enter the companion's host well,
            // never the kanban/stars overlay.
            let show_companion_i = MenuItem::with_id(
                app,
                "show_companion",
                "Show Companion",
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
                    &show_companion_i,
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
                    "quit" => {
                        app.exit(0);
                    }
                    "show_companion" => {
                        let _ = panes::show_companion(app);
                        let _ = app.emit_to("companion", "orbitty:companion-summon", ());
                    }
                    // Raise-only: show + grow the frame native-side + ask
                    // the page to flip the well (hosted is page $state).
                    // Never a toggle — the dest buttons still own toggling.
                    mode @ ("launchpad" | "stars") => {
                        let _ = panes::show_companion(app);
                        let _ = panes::set_companion_host(app.clone(), mode.to_string());
                        let _ = app.emit_to("companion", "orbitty:enter-host", mode);
                    }
                    "settings" => {
                        let _ = panes::show_labeled(app, "settings");
                    }
                    "inspector" => {
                        show_rail(app);
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

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Orbitty")
        .run(|app_handle, event| {
            match event {
                // Dock icon click: raise the companion with the caret in
                // the field (same as tray left-click / ⌘⇧O), even when it
                // is already visible (macOS otherwise no-ops).
                tauri::RunEvent::Reopen { .. } => {
                    let _ = panes::show_companion(app_handle);
                    let _ = app_handle.emit_to("companion", "orbitty:companion-summon", ());
                }
                #[cfg(unix)]
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
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
