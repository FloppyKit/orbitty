use tauri::{Emitter, Manager};

/// Panes keep warm on hide, so a hidden webview's
/// `document.hidden` never flips and its timers keep firing into the void.
/// Pages gate those timers on this event instead. Payload: `{ label,
/// visible }`.
pub(crate) fn emit_pane_vis(app: &tauri::AppHandle, label: &str, visible: bool) {
    let _ = app.emit(
        "orbitty:pane-vis",
        serde_json::json!({ "label": label, "visible": visible }),
    );
}

/// Banner × on either webview window. The JS `getCurrentWindow().hide()`
/// path proved unreliable from the Lists window (drag-region eats the
/// click), so the × buttons call this directly. Hide only — never quit;
/// Quit lives on the tray menu alone.
#[tauri::command]
pub fn hide_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let Some(w) = app.get_webview_window(&label) else {
        return Err(format!("no window labeled {label}"));
    };
    // A hide while hosted persists the RESTORED
    // idle rect, never the host frame. Same for the hug bar —
    // persist the tall idle, never the bar. And never a
    // composer-grown frame.
    if label == "companion" {
        unhost_before_persist(&app);
        unhug_before_persist(&app);
        ungrow_companion_composer(&app);
    }
    if crate::state::DRAWER_LABELS.contains(&label.as_str()) {
        crate::drawers::persist_shown_place(&app, &label);
    }
    if label == "next-card" || label == "cheatsheet" {
        crate::nextcard::watch(&label, false);
        let _ = w.set_always_on_top(false);
    }
    w.hide().map_err(|e| e.to_string())?;
    emit_pane_vis(&app, &label, false);
    if label == "companion" {
        crate::talk::stop_if_any();
        companion_pin(&app, &w, false);
        persist_companion_hidden(&app, &w);
        crate::companion_clear::hidden(&app);
    }
    Ok(())
}

/// Pane specs mirror the pre-slim tauri.conf.json
/// entries (transparent, Overlay, hiddenTitle, no drag-drop).
/// 619×647 was a miss (oversized persist).
/// Idle floor is 280×360 until Matt sizes and we lock live AX. Hug 280×120.
/// Returns (title, width, height, min (w, h)).
pub(crate) fn pane_spec(label: &str) -> Option<(&'static str, f64, f64, Option<(f64, f64)>)> {
    match label {
        "kanban" => Some(("", 780.0, 520.0, None)),
        "stars" => Some(("", 780.0, 520.0, None)),
        "companion" => Some(("", 619.0, 647.0, Some((280.0, 360.0)))),
        "settings" | "sky" => Some(("", 420.0, 760.0, None)),
        // The Herding Cats confirm is its own small
        // window (like Settings) so ⌘⇧Y works with Settings closed. Plain
        // pane, not a drawer — a confirm never docks on the rail.
        "yard-warn" => Some(("", 400.0, 330.0, None)),
        // own small card window, not dock chrome.
        "next-card" => Some(("", 340.0, 260.0, Some((280.0, 180.0)))),
        "cheatsheet" => Some(("", 360.0, 420.0, Some((280.0, 240.0)))),
        _ => None,
    }
}

/// The companion pins only while SHOWN. The
/// avatar-large lock (never pin companion) is why a shown companion's
/// isVisible never stuck and the mouse gate kept hiding it — pin on a
/// verified show so it stays front, unpin on every hide so Show Desktop
/// never keeps a hidden giant avatar. kanban/settings stay pinned from
/// create; this helper is companion-only. Sync main-thread hop, same as
/// the PANE-RAISE raise.
pub(crate) fn companion_pin(app: &tauri::AppHandle, w: &tauri::WebviewWindow, pinned: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        let w = w.clone();
        crate::main_thread_sync(move || {
            if pinned {
                crate::glass::pin_stay_visible(&w);
            } else {
                crate::glass::unpin_stay_visible(&w);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, w, pinned);
}

/// Create-on-first-open: return the window, building it with the pane spec
/// when it doesn't exist yet. Same SPA bundle as the rail — the frontend
/// mounts the page matching the window label. After create: close-to-hide
/// wiring (+ companion bounds throttle), then the S4.3/S4.4 glass recipe;
/// stay-visible pin on every pane except the companion (avatar-large lock).
pub(crate) fn ensure_window(
    app: &tauri::AppHandle,
    label: &str,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(label) {
        // COMPANION-MIN: already-created panes keep the old 280×360 until
        // hug/unhug; re-assert the idle floor on every ensure.
        if label == "companion" && !companion_hugged() {
            companion_min_size(&w, false);
        }
        return Ok(w);
    }
    let Some((title, width, height, min)) = pane_spec(label) else {
        return Err(format!("no window labeled {label}"));
    };
    // Every pane takes the rail recipe —
    // transparent, Overlay, hidden title. The SPA hydrates now, so the
    // PAINT-era Visible titlebar + opaque slab on drawers are retired:
    // no extra native strip above the in-SPA chrome, and the CSS
    // --bg-fill alpha (Settings slider) shows the desk on drawers
    // exactly like the rail.
    // Built through the same from_config path the conf-declared windows
    // used, so every flag (including webview transparency) matches exactly.
    // Default url is App("index.html") — the rail's SPA bundle.
    let mut config = tauri::utils::config::WindowConfig {
        label: label.to_string(),
        title: title.to_string(),
        width,
        height,
        min_width: min.map(|m| m.0),
        min_height: min.map(|m| m.1),
        visible: false,
        center: true,
        resizable: true,
        fullscreen: false,
        transparent: true,
        title_bar_style: tauri::TitleBarStyle::Overlay,
        hidden_title: true,
        // The companion composer takes a dragged file
        // (one chip, original path) — every other pane stays drop-deaf.
        drag_drop_enabled: label == "companion",
        ..Default::default()
    };
    // Dest chips close in one click while Launchpad/STARS hold key-window
    // — without this, the first click only activates companion.
    config.accept_first_mouse = label == "companion";
    // Chrome panes: no native Overlay title strip. That invisible ~28px
    // bar plus the in-SPA banner was two drag sources — grab failed
    // or the window ran ahead of the cursor. Borderless; JS
    // startDragging on the banner is the one handle. Rail stays Overlay.
    // the companion joins them — its
    // Overlay strip flashed on the host grow / sprite-hug resize (a
    // titlebar blink that moved the screen). The JS dock grip + spirit
    // drag stay the one handle; the grow itself stays instant
    // keep-bottom, never an animated NSWindow bounds.
    if crate::state::DRAWER_LABELS.contains(&label)
        || label == "sky"
        || label == "companion"
        || label == "yard-warn"
        || label == "next-card"
        || label == "cheatsheet"
    {
        config.decorations = false;
        config.maximizable = false;
        config.minimizable = false;
    }
    // The URL is ALWAYS the bare `index.html` — never
    // a `?mode=` query. WKWebView looks up the query-bearing name as a
    // file, misses, and the pinned overlay paints a click-eating 404. The
    // shared pane's mode is persisted before show (show_shared_pane) and
    // the cold webview reconciles it via load_state on mount.
    let builder = tauri::WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?;
    // Belt-and-braces for any future bad URL or asset miss: on an app URL
    // the nav guard refuses query-bearing navigations (the `?mode=` 404)
    // and a non-2xx document is logged — but a rejected nav or a bad
    // response NEVER hides, click-throughs, or unpins the pane
    // (HOLD says a requested show stays shown; the
    // old path allow-list cancelled real SvelteKit navigations out from
    // under a loading drawer and left a blank document).
    let nav_label = label.to_string();
    let req_label = label.to_string();
    let w = builder
        .on_navigation(move |url| {
            let scheme = url.scheme();
            let app_url = scheme == "tauri"
                || ((scheme == "http" || scheme == "https")
                    && matches!(url.host_str(), Some("localhost") | Some("tauri.localhost")));
            if !app_url {
                return true;
            }
            // The guard is query-only — any bare path
            // on the app origin is the SPA's own (client start / history
            // from index.html). The only poison left is a `?…` URL.
            let ok = url.query().is_none();
            if !ok {
                eprintln!("pane-spa: {nav_label} refused app-url navigation with query: {url}");
            }
            ok
        })
        .on_web_resource_request(move |request, response| {
            let uri = request.uri();
            let is_document = (uri.scheme_str() == Some("tauri")
                || (matches!(uri.scheme_str(), Some("http") | Some("https"))
                    && matches!(uri.host(), Some("localhost") | Some("tauri.localhost"))))
                && matches!(uri.path(), "/" | "/index.html");
            // Log only — never hide a drawer on a bad
            // response.
            if is_document && !response.status().is_success() {
                eprintln!(
                    "pane-spa: {req_label} document came back {} — leaving drawer alone",
                    response.status()
                );
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    // Stamp the real build (the cache hit above
    // returns before this) — the mount check spares a pane this young
    // from the SPA-law reload while its first paint hydrates.
    crate::drawers::stamp_window_created(label);
    // Mouse on from birth. Hidden panes do not eat clicks; a shown pane
    // is a window, not a privilege gated on placement purity.
    let _ = w.set_ignore_cursor_events(false);
    wire_pane_events(app, &w, label == "companion");
    crate::drawers::wire_drawer_events(app, &w, label);
    // Glass only. Chrome is not pinned — that recipe is for the rail
    // (Show Desktop). Drawers join normal z-order so they can overlap
    // and stay where the user put them.
    let _ = w.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    let surgery = w.clone();
    let _ = app.run_on_main_thread(move || {
        crate::glass::ensure_transparent(&surgery);
        crate::glass::hide_traffic_lights(&surgery);
    });
    Ok(w)
}

/// Hide-not-destroy on any close request (Quit lives on the tray menu
/// alone), plus the companion's throttled bounds persist on move/resize.
/// Wired at setup for config-declared windows and at create time for lazy
/// panes.
pub(crate) fn wire_pane_events(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    is_companion: bool,
) {
    let keeper = window.clone();
    let handle = app.clone();
    let pane_label = window.label().to_string();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if pane_label == "companion" {
                unhost_before_persist(&handle);
                unhug_before_persist(&handle);
                // COLLAPSE-CC: persist the idle rect, never a
                // composer-grown one.
                ungrow_companion_composer(&handle);
            }
            if crate::state::DRAWER_LABELS.contains(&pane_label.as_str()) {
                crate::drawers::persist_shown_place(&handle, &pane_label);
            }
            let _ = keeper.hide();
            emit_pane_vis(&handle, &pane_label, false);
            if is_companion {
                crate::talk::stop_if_any();
                companion_pin(&handle, &keeper, false);
                persist_companion_hidden(&handle, &keeper);
                crate::companion_clear::hidden(&handle);
            }
        }
    });
}

pub(crate) fn toggle_cheatsheet(app: tauri::AppHandle) -> Result<(), String> {
    let up = app
        .get_webview_window("cheatsheet")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if up {
        hide_window(app, "cheatsheet".to_string())
    } else {
        show_labeled(&app, "cheatsheet")
    }
}

pub(crate) fn show_labeled(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    // The three chrome drawers show through the
    // leash (docked slot / plucked bounds). `sky` and the rail stay plain.
    if crate::state::DRAWER_LABELS.contains(&label) {
        return crate::drawers::show_drawer(app, label);
    }
    let w = ensure_window(app, label)?;
    w.show().map_err(|e| e.to_string())?;
    if label == "next-card" || label == "cheatsheet" {
        let _ = w.set_always_on_top(true);
        crate::nextcard::watch(label, true);
    }
    w.set_focus().map_err(|e| e.to_string())?;
    crate::drawers::appkit_order_front(app, &w);
    if !crate::drawers::pane_appkit_visible(&w) {
        let _ = w.show();
        crate::drawers::appkit_order_front(app, &w);
    }
    let _ = w.set_ignore_cursor_events(false);
    emit_pane_vis(app, label, true);
    Ok(())
}

/// Launchpad (`kanban`) and STARS (`stars`) are separate windows.
pub(crate) fn show_shared_pane(app: &tauri::AppHandle, mode: &str) -> Result<(), String> {
    if mode == "stars" {
        show_labeled(app, "stars")
    } else {
        show_labeled(app, "kanban")
    }
}

/// The companion window is the host for the
/// Launchpad/STARS well. The idle rect stash is memory-only — never a
/// persisted hostBounds/hitchedTo key — so a quit while hosted cannot
/// turn the host frame into the new idle size.
static HOST_STASH: std::sync::OnceLock<
    std::sync::Mutex<Option<crate::state::CompanionBounds>>,
> = std::sync::OnceLock::new();

fn host_stash() -> &'static std::sync::Mutex<Option<crate::state::CompanionBounds>> {
    HOST_STASH.get_or_init(|| std::sync::Mutex::new(None))
}

/// Idle collapsed + sprite off hugs the window to
/// the JS-measured CC+dock bar (keep the bottom — the dock never jumps).
/// HUG_STASH holds the TALL idle rect while hugged — separate from
/// HOST_STASH — and the persisted idle place is always the tall rect
/// (unhug_before_persist, the unhost_before_persist analog): a quit while
/// hugged reopens sprite-off from the tall rect and the page re-hugs.
/// HUG_HEIGHT is the last JS-measured bar height, reused by the native
/// re-hug on dismiss-host.
static HUG_STASH: std::sync::OnceLock<
    std::sync::Mutex<Option<crate::state::CompanionBounds>>,
> = std::sync::OnceLock::new();
static HUG_HEIGHT: std::sync::OnceLock<std::sync::Mutex<Option<f64>>> =
    std::sync::OnceLock::new();

fn hug_stash() -> &'static std::sync::Mutex<Option<crate::state::CompanionBounds>> {
    HUG_STASH.get_or_init(|| std::sync::Mutex::new(None))
}

fn hug_height_slot() -> &'static std::sync::Mutex<Option<f64>> {
    HUG_HEIGHT.get_or_init(|| std::sync::Mutex::new(None))
}

/// True while the companion is hugged to the bar. drawers::show_drawer
/// reads this so the sub-360 bar is never mistaken for a ghosted
/// min-size window and re-placed to spec on a summon.
pub(crate) fn companion_hugged() -> bool {
    hug_stash().lock().map(|s| s.is_some()).unwrap_or(false)
}

/// Hug bar sits below the idle min, so the floor drops to 120 while
/// hugged and returns to the pane-spec min on unhug.
fn companion_min_size(w: &tauri::WebviewWindow, hugged: bool) {
    let (mw, mh) = if hugged {
        (280.0, 120.0)
    } else {
        pane_spec("companion")
            .and_then(|(_, _, _, m)| m)
            .unwrap_or((280.0, 360.0))
    };
    let _ = w.set_min_size(Some(tauri::LogicalSize::new(mw, mh)));
}

/// Hug the shown idle collapsed companion to `hug_height` (None = the
/// last JS measure), keeping the bottom edge: y' = y + (h - hug_h). The
/// first hug stashes the tall idle rect; re-hugs with a fresh height
/// grow/shrink the bar from that same bottom (never taller than the
/// stash, so a first hug with nothing to trim is a no-op). Never hugs
/// while hosted; never persists the bar.
pub(crate) fn hug_companion(app: &tauri::AppHandle, hug_height: Option<f64>) {
    if host_stash().lock().map(|s| s.is_some()).unwrap_or(false) {
        return;
    }
    let h = hug_height.or_else(|| hug_height_slot().lock().ok().and_then(|s| *s));
    let Some(h) = h else { return };
    let Some(w) = app.get_webview_window("companion") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    // never hug from a rect
    // whose dock already hangs below every display — the keep-bottom bar
    // would inherit the sunk bottom and strand the dock unclickable.
    // Rescue first so the bar math below bases on a clickable rect; runs
    // before the already-fit early-return so a lost bar re-fits too.
    crate::drawers::ensure_companion_dock_onscreen(&w);
    let Some(live) = crate::drawers::live_points_rect(&w) else {
        return;
    };
    let existing = hug_stash().lock().ok().and_then(|s| *s);
    let tall = existing.unwrap_or(live);
    let h = h.clamp(120.0, tall.h);
    if let Ok(mut slot) = hug_height_slot().lock() {
        *slot = Some(h);
    }
    if (live.h - h).abs() < 1.0 {
        return;
    }
    if existing.is_none() {
        // First real hug: stash the tall idle rect.
        if let Ok(mut s) = hug_stash().lock() {
            *s = Some(live);
        }
    }
    companion_min_size(&w, true);
    let bar = crate::state::CompanionBounds {
        x: live.x,
        y: live.y + (live.h - h),
        w: live.w,
        h,
    };
    crate::drawers::place_hug_rect(&w, "companion", &bar);
}

/// Restore the tall idle from the hug stash at the live origin,
/// keep-bottom (y' = y - (tall_h - hug_h) — the dock stays). No-op when
/// not hugged.
pub(crate) fn unhug_companion(app: &tauri::AppHandle) {
    let tall = hug_stash().lock().ok().and_then(|mut s| s.take());
    let Some(tall) = tall else { return };
    if let Some(w) = app.get_webview_window("companion") {
        let live = crate::drawers::live_points_rect(&w);
        let r = crate::state::CompanionBounds {
            x: live.as_ref().map(|r| r.x).unwrap_or(tall.x),
            y: live.as_ref().map(|r| r.y + r.h - tall.h).unwrap_or(tall.y),
            w: tall.w,
            h: tall.h,
        };
        crate::drawers::place_logical_rect(&w, "companion", &r);
        // the keep-bottom
        // restore inherits the bar's bottom — rescue if that leaves the
        // dock sunk below every display.
        crate::drawers::ensure_companion_dock_onscreen(&w);
        companion_min_size(&w, false);
    }
}

/// A hide/close while hugged persists the TALL
/// idle rect, never the bar — restore the stash first (the
/// unhost_before_persist analog). The page re-hugs on the next show.
pub(crate) fn unhug_before_persist(app: &tauri::AppHandle) {
    if !companion_hugged() {
        return;
    }
    unhug_companion(app);
}

/// Collapsed sprite-ON composer grow — the window
/// grows DOWN (keep top) with the wrapping composer instead of squashing
/// the spirit. COMPOSER_GROW is the last applied px delta, never a rect
/// stash: a grow adds only the delta from the last report and ungrow
/// removes exactly what was added, so a user edge-resize mid-type
/// survives. Session-only — expand / host / sprite-off / hide / close
/// shed the extra first, so the persisted idle place never includes it.
static COMPOSER_GROW: std::sync::OnceLock<std::sync::Mutex<f64>> =
    std::sync::OnceLock::new();

fn composer_grow_slot() -> &'static std::sync::Mutex<f64> {
    COMPOSER_GROW.get_or_init(|| std::sync::Mutex::new(0.0))
}

/// Take the outstanding composer extra (0 when none). Host-enter reads
/// this to strip the extra from its stash base without placing first.
pub(crate) fn composer_grow_take() -> f64 {
    composer_grow_slot()
        .lock()
        .map(|mut s| std::mem::take(&mut *s))
        .unwrap_or(0.0)
}

/// Remove an outstanding composer grow (keep top). No-op at 0.
pub(crate) fn ungrow_companion_composer(app: &tauri::AppHandle) {
    let extra = composer_grow_take();
    if extra <= 0.0 {
        return;
    }
    let Some(w) = app.get_webview_window("companion") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    let Some(live) = crate::drawers::live_points_rect(&w) else {
        return;
    };
    let r = crate::state::CompanionBounds {
        x: live.x,
        y: live.y,
        w: live.w,
        h: live.h - extra,
    };
    crate::drawers::place_logical_rect(&w, "companion", &r);
}

/// JS report of the composer's grown-past-one-line px. Only idle
/// collapsed + sprite-on + not-hosted grows (keep top); every other
/// surface sheds an outstanding extra and no-ops. A hidden window
/// no-ops — the hide already ungrew and the show re-reports.
#[tauri::command]
pub fn set_companion_composer_grow(app: tauri::AppHandle, extra: f64) -> Result<(), String> {
    let st = crate::state::load_state(&app)?;
    let hosted = host_stash().lock().map(|s| s.is_some()).unwrap_or(false);
    if st.companion.expanded || !st.companion.sprite || hosted || companion_hugged() {
        ungrow_companion_composer(&app);
        return Ok(());
    }
    let extra = extra.clamp(0.0, 600.0);
    let prev = composer_grow_slot().lock().map(|s| *s).unwrap_or(0.0);
    let delta = extra - prev;
    if delta.abs() < 1.0 {
        return Ok(());
    }
    let Some(w) = app.get_webview_window("companion") else {
        return Ok(());
    };
    if !w.is_visible().unwrap_or(false) {
        return Ok(());
    }
    let Some(live) = crate::drawers::live_points_rect(&w) else {
        return Ok(());
    };
    if let Ok(mut s) = composer_grow_slot().lock() {
        *s = extra;
    }
    let r = crate::state::CompanionBounds {
        x: live.x,
        y: live.y,
        w: live.w,
        h: live.h + delta,
    };
    crate::drawers::place_logical_rect(&w, "companion", &r);
    // A grow-down can sink the dock below every display — the dock-click
    // law rescues (the composer never strands off-screen).
    crate::drawers::ensure_companion_dock_onscreen(&w);
    Ok(())
}

/// While hosted, hug the live frame to `height`
/// (keep-bottom), clamped to [280, host_grown_rect.h]. Never writes
/// HOST_STASH. No-op when not hosted / hidden / already fit.
#[tauri::command]
pub fn set_companion_host_hug(app: tauri::AppHandle, height: f64) -> Result<(), String> {
    let hosted = host_stash().lock().map(|s| s.is_some()).unwrap_or(false);
    if !hosted {
        return Ok(());
    }
    let Some(w) = app.get_webview_window("companion") else {
        return Ok(());
    };
    if !w.is_visible().unwrap_or(false) {
        return Ok(());
    }
    let Some(live) = crate::drawers::live_points_rect(&w) else {
        return Ok(());
    };
    let max_h = crate::drawers::host_grown_rect(live).h;
    let h = height.clamp(280.0, max_h);
    if (live.h - h).abs() < 2.0 {
        return Ok(());
    }
    let (mw, mh) = pane_spec("companion")
        .and_then(|(_, _, _, m)| m)
        .unwrap_or((280.0, 360.0));
    let _ = w.set_min_size(Some(tauri::LogicalSize::new(mw, 280.0_f64.min(mh))));
    let pinned = crate::drawers::pin_bottom_center(live, live.w, h);
    let placed = crate::drawers::clamp_on_displays_keep_bottom(pinned).unwrap_or(pinned);
    crate::drawers::place_logical_rect_keep_bottom(&w, "companion", &placed);
    crate::drawers::ensure_companion_dock_onscreen(&w);
    Ok(())
}

/// The ONE native path that resizes the companion
/// for hosting — the frontend never set_sizes the companion itself.
///   "launchpad" | "stars" | "inspector" → stash the live idle rect (once per host
///     session — a Launchpad ↔ STARS well swap never re-stashes the grown
///     rect), hide the shared `kanban` overlay if it is visible (never
///     destroy), grow to the display-fit host WIDTH (1080 landscape /
///     620 portrait) at the live height (floor 360, cap the display-fit
///     max). JS grows height up to content — never max-then-shrink.
///   "off" → idle size from the stash (spec 520×780 when there is none)
///     at the hosted window's LIVE origin, clamped on that one display
///     only (never re-centered, never another
///     monitor), then persist_shown_place on the RESTORED rect, so the
///     host size never becomes the new idle size.
/// Which component sits in the well is frontend session state; this
/// command only owns the window frame.
#[tauri::command]
pub fn set_companion_host(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let on = mode == "launchpad" || mode == "stars" || mode == "inspector";
    let Some(w) = app.get_webview_window("companion") else {
        return Err("no window labeled companion".to_string());
    };
    if on {
        let stashed = host_stash().lock().map(|s| s.is_some()).unwrap_or(false);
        // Already hosted (Launchpad ↔ STARS
        // ↔ inspector well swap, tray re-assert) — host_changed only, so HOSTED
        // re-asserts and apply_mode slabs. Never a second stash, never a
        // re-place: no re-grow, no hop.
        if stashed {
            crate::companion_clear::host_changed(&app, true);
            return Ok(());
        }
        // Enter-host from hugged must NOT
        // physically unhug first — bar → tall → grown is two waited
        // setFrames with a runloop turn between, and the tall idle
        // frame flashes on screen mid-grow (the sprite-off flash).
        // Compute the rect unhug_companion WOULD restore (same math,
        // never placed) so the host stash below is the tall rect —
        // never the live bar — then grow once: bar → grown, a single
        // instant keep-bottom jump. The hug-height slot survives for
        // the dismiss-time re-hug.
        let from_hug = hug_stash().lock().ok().and_then(|mut s| s.take());
        let base = match from_hug {
            Some(tall) => {
                companion_min_size(&w, false);
                let live = crate::drawers::live_points_rect(&w);
                Some(crate::state::CompanionBounds {
                    x: live.as_ref().map(|r| r.x).unwrap_or(tall.x),
                    y: live.as_ref().map(|r| r.y + r.h - tall.h).unwrap_or(tall.y),
                    w: tall.w,
                    h: tall.h,
                })
            }
            None => crate::drawers::live_points_rect(&w),
        };
        // A sprite-on composer grow is session
        // chrome, never the idle rect — strip the outstanding extra from
        // the stash base (never placed) so the host stash and the
        // dismiss-time restore keep the idle height. Hugged and grown
        // are mutually exclusive (sprite off vs on): from_hug bases are
        // untouched by this.
        let grow_extra = composer_grow_take();
        let base = base.map(|mut r| {
            r.h = (r.h - grow_extra).max(360.0);
            r
        });
        if let Some(r) = base {
            if let Ok(mut s) = host_stash().lock() {
                *s = Some(r);
            }
        }
        // Hosted hides the shared overlay — never destroys it.
        if let Some(k) = app.get_webview_window("kanban") {
            if k.is_visible().unwrap_or(false) {
                crate::drawers::persist_shown_place(&app, "kanban");
                let _ = k.hide();
                emit_pane_vis(&app, "kanban", false);
            }
        }
        // The well IS the inspector. Hide the
        // rail window (never destroy) so the desk isn't two inspectors.
        // Tray Inspector / embedded pop-out still show_rail.
        if mode == "inspector" {
            if let Some(m) = app.get_webview_window("main") {
                if m.is_visible().unwrap_or(false) {
                    let _ = m.hide();
                    emit_pane_vis(&app, "main", false);
                }
            }
        }
        // Pin from the same base the stash kept (the would-be-restored
        // tall rect when hugged — its bottom-center is the bar's, so
        // the dock stays put) — never the still-hugged live bar.
        let cur = base.unwrap_or(crate::state::CompanionBounds {
            x: 0.0,
            y: 0.0,
            w: 520.0,
            h: 780.0,
        });
        // WIDTH follows the display (1080 landscape,
        // 620 portrait). Leftover: HEIGHT does
        // not jump to the display max — that max-then-hug-down jittered
        // Launchpad. Enter at the live height (floor 360, cap the max);
        // JS then grows UP to content via set_companion_host_hug.
        // Pin the live bottom-center so the dest
        // dock stays under the cursor.
        let grown = crate::drawers::host_grown_rect(cur);
        let live_h = crate::drawers::live_points_rect(&w)
            .map(|r| r.h)
            .unwrap_or(cur.h);
        let enter_h = live_h.clamp(360.0, grown.h);
        let pinned = crate::drawers::pin_bottom_center(cur, grown.w, enter_h);
        let placed = crate::drawers::clamp_on_displays_keep_bottom(pinned).unwrap_or(pinned);
        crate::drawers::place_logical_rect_keep_bottom(&w, "companion", &placed);
        // the keep-bottom
        // grow must not leave the dock sunk below every display (a
        // low-parked idle pins a sunk bottom; a fully-off pinned rect
        // survives the unwrap_or) — rescue onto the display.
        crate::drawers::ensure_companion_dock_onscreen(&w);
    } else {
        companion_min_size(&w, false);
        let stash = host_stash().lock().ok().and_then(|mut s| s.take());
        // Dismiss = idle SIZE pinned
        // to the LIVE hosted bottom-center (stash origin is enter-time
        // history). Clamp on that one display only.
        let live = crate::drawers::live_points_rect(&w);
        let rw = stash.as_ref().map(|r| r.w).unwrap_or(520.0);
        let rh = stash.as_ref().map(|r| r.h).unwrap_or(780.0);
        let pin_from = live.unwrap_or_else(|| {
            stash.unwrap_or(crate::state::CompanionBounds {
                x: 0.0,
                y: 0.0,
                w: rw,
                h: rh,
            })
        });
        let restore = crate::drawers::pin_bottom_center(pin_from, rw, rh);
        let placed = crate::drawers::clamp_on_host_display(live, restore);
        crate::drawers::place_logical_rect_keep_bottom(&w, "companion", &placed);
        // never dismiss into
        // a rect whose dock is sunk — rescue BEFORE the persist so the
        // saved idle place is clickable too.
        crate::drawers::ensure_companion_dock_onscreen(&w);
        // Persist the RESTORED idle rect — never the host frame.
        crate::drawers::persist_shown_place(&app, "companion");
        // Dismiss-host with the sprite still off
        // (and still collapsed) hugs again on this display — the tall
        // restore happened above; the bar re-fit uses the last JS
        // measure and the page's re-report refines it.
        let hug = crate::state::load_state(&app)
            .map(|s| !s.companion.sprite && !s.companion.expanded)
            .unwrap_or(false);
        if hug {
            hug_companion(&app, None);
        }
    }
    // Host on forces the slab (mouse on) NOW;
    // host off reapplies the ghost against the kept hits on the restored
    // idle frame.
    crate::companion_clear::host_changed(&app, on);
    Ok(())
}

/// A hide while hosted must persist the IDLE rect,
/// never the host frame — restore the stash first (a no-op when not
/// hosted). The frontend re-asserts the host frame on the next show while
/// its session flag is still set.
pub(crate) fn unhost_before_persist(app: &tauri::AppHandle) {
    let hosted = host_stash().lock().map(|s| s.is_some()).unwrap_or(false);
    if !hosted {
        return;
    }
    let _ = set_companion_host(app.clone(), "off".to_string());
}

pub(crate) fn show_companion(app: &tauri::AppHandle) -> Result<(), String> {
    // Docked companions take the inboard slot;
    // plucked ones restore bounds inside show_drawer.
    // an already-visible summon is raise-only —
    // rewriting companion.visible + emitting state:changed on every ⌘⇧O
    // reloaded every pane and beach-balled the desk.
    let already = app
        .get_webview_window("companion")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    crate::drawers::show_drawer(app, "companion")?;
    if already {
        return Ok(());
    }
    let mut st = crate::state::load_state(app).unwrap_or_default();
    st.companion.visible = true;
    let _ = crate::state::save_state(app, &st);
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Launchpad Needed click: native raise (makeKey + orderFront) then tell
/// the companion webview which Needed owns the mouth. JS setFocus from
/// the companion webview cannot steal key from Launchpad — ⌘⇧O already
/// works because it goes through show_companion first.
#[tauri::command]
pub fn arm_companion_mouth(
    app: tauri::AppHandle,
    id: Option<String>,
    title: Option<String>,
) -> Result<(), String> {
    show_companion(&app)?;
    let _ = app.emit_to(
        "companion",
        "orbitty:needed-focus",
        serde_json::json!({ "id": id, "title": title }),
    );
    let _ = app.emit_to("companion", "orbitty:companion-summon", ());
    Ok(())
}

/// Last-ditch companion place when no display geometry is known to
/// drawers: center on the primary monitor. Size stays the live/spec
/// frame — do not resize or maximize here.
pub(crate) fn companion_default_frame(app: &tauri::AppHandle, w: &tauri::WebviewWindow) {
    let mon = w
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| w.current_monitor().ok().flatten())
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|m| m.current_monitor().ok().flatten())
        });
    if let Some(mon) = mon {
        let scale = mon.scale_factor();
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let ms_w = mon.size().width as f64 / scale;
        let ms_h = mon.size().height as f64 / scale;
        let mp_x = mon.position().x as f64 / scale;
        let mp_y = mon.position().y as f64 / scale;
        let (win_w, win_h) = w
            .outer_size()
            .ok()
            .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
            .unwrap_or((520.0, 780.0));
        let x = mp_x + ((ms_w - win_w) / 2.0);
        let y = mp_y + ((ms_h - win_h) / 2.0);
        let _ = w.set_position(tauri::LogicalPosition::new(x, y));
    }
}

/// Companion hide bookkeeping: visible=false only. Place is already
/// written by persist_shown_place before hide.
fn persist_companion_hidden(app: &tauri::AppHandle, _w: &tauri::WebviewWindow) {
    let Ok(mut st) = crate::state::load_state(app) else {
        return;
    };
    st.companion.visible = false;
    let _ = crate::state::save_state(app, &st);
    crate::drawers::drawer_cache_refresh(&st);
    let _ = app.emit("state:changed", ());
}
