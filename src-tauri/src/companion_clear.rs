//! GO-ORBITTY-COMPANION-CLEAR: the idle collapsed companion is a ghost —
//! empty pixels click through to the desk while the spirit image box and
//! the chrome (CC, starstrip, agentrow, composer, ×, ⚙, note, planets
//! popup) still hit. Slab (expanded persist OR host stash) is always a
//! full window and never click-throughs.
//!
//! Cannot be JS-only: a fully cursor-ignoring window never sees the
//! mouseenter that would turn events back on. So CompanionPage reports
//! coarse window-local CSS-point hit rects (`set_companion_hits`) and
//! this module runs a ~10Hz native poll (same class as the habitat
//! discs' cursor_tick — no second DisplayLink) that flips
//! `set_ignore_cursor_events` by cursor-vs-rects. Coarse boxes only —
//! never PNG alpha sampling.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

/// One window-local hit rect in CSS points (top-left origin), reported
/// by CompanionPage.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct HitRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The last reported ghost hit list. Kept across slab periods (law 6):
/// an expand/host detour never clears it, so collapse-to-ghost reapplies
/// with the pre-slab rects and there is no click-through hole on the
/// spirit while the frontend re-measures.
static HITS: OnceLock<Mutex<Vec<HitRect>>> = OnceLock::new();
fn hits() -> &'static Mutex<Vec<HitRect>> {
    HITS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Mode mirrors so the 10Hz poll never reads the disk. `expanded` is the
/// persisted flag (seeded at start, mirrored by set_companion_expanded);
/// `hosted` is the memory-only host stash (mirrored by
/// set_companion_host — its only writer).
static EXPANDED: AtomicBool = AtomicBool::new(false);
static HOSTED: AtomicBool = AtomicBool::new(false);
/// Poll gate: true only while the companion is shown AND ghost. Hidden
/// panes eat no clicks, so the poll idles (300ms) until a show re-arms it.
static GHOST_ACTIVE: AtomicBool = AtomicBool::new(false);
/// GO-ORBITTY-COMPANION-RAISE summon hold: set on every entry into ghost
/// (show/summon, collapse, host-off). GO-ORBITTY-FRONT-HOLD flips the
/// direction: while set, the poll holds mouse-on until the cursor LEAVES
/// the companion frame — a summon landing over empty air inside the tall
/// window must not spend the hold and ghost-apply the window back into a
/// click-through the same tick (that re-buried the raised ghost). The
/// same hold means a collapse racing host-on never arms a punch-through
/// poll. Clears on cursor-leave, on slab, on hide.
static SUMMON_HOLD: AtomicBool = AtomicBool::new(false);
/// FRONT-HOLD leftover: the leave-frame spend is only real after the
/// cursor has been *inside* once. ⌘⇧O / tray with the pointer already
/// outside used to spend this tick, mouse-off the ghost, and let a
/// planet steal the click so Show Companion looked like a no-op.
static HOLD_SEEN_INSIDE: AtomicBool = AtomicBool::new(false);
/// Planet / walker NSMenu is tracking. Poll skips setLevel / ghost hops
/// so a nested run loop cannot beach-ball behind dispatch_sync.
static MENU_QUIET: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_menu_quiet(on: bool) {
    MENU_QUIET.store(on, Ordering::SeqCst);
}

pub(crate) fn menu_quiet() -> bool {
    MENU_QUIET.load(Ordering::SeqCst)
}

fn slab() -> bool {
    EXPANDED.load(Ordering::SeqCst) || HOSTED.load(Ordering::SeqCst)
}

/// Frontend reports the ghost hit rects (window-local CSS points).
/// Empty means "leaving ghost" — never stored (a shown ghost always has
/// chrome: ×/⚙/agentrow/composer), so the kept list survives the slab
/// detour and collapse has no hole.
#[tauri::command]
pub fn set_companion_hits(app: tauri::AppHandle, rects: Vec<HitRect>) -> Result<(), String> {
    if !rects.is_empty() {
        if let Ok(mut h) = hits().lock() {
            *h = rects;
        }
    }
    // Fresh hits land → re-test now (first show, layout shifts) instead
    // of waiting out the 100ms poll.
    if GHOST_ACTIVE.load(Ordering::SeqCst) {
        apply_on_main(&app);
    }
    Ok(())
}

/// coordinator::set_companion_expanded mirror. Law 6: expand forces the
/// slab (mouse on) NOW, before the JS re-render/re-report.
pub(crate) fn expanded_changed(app: &tauri::AppHandle, expanded: bool) {
    EXPANDED.store(expanded, Ordering::SeqCst);
    apply_mode(app);
}

/// panes::set_companion_host mirror. Host on → slab NOW; host off →
/// ghost re-test against the kept hits (frame is already restored).
pub(crate) fn host_changed(app: &tauri::AppHandle, on: bool) {
    HOSTED.store(on, Ordering::SeqCst);
    apply_mode(app);
}

/// Companion hid — the poll idles. Ignore state is left as-is; the next
/// show forces mouse-on and re-applies the mode. GO-ORBITTY-FRONT-HOLD:
/// the one-level-up front (apply_mode) ends with the show.
pub(crate) fn hidden(app: &tauri::AppHandle) {
    GHOST_ACTIVE.store(false, Ordering::SeqCst);
    SUMMON_HOLD.store(false, Ordering::SeqCst);
    HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
    set_front_level(app, false);
}

/// CHROME-Z leftover: `NSApp.isActive` is false while the user is on a
/// nonactivating planet panel, so the poll treated that as "Safari is
/// front" and dropped the companion to level 0 under the discs. The
/// real other-app test is the workspace frontmost pid.
#[cfg(target_os = "macos")]
fn orbitty_is_frontmost() -> bool {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return false;
        }
        let front: *mut Object = msg_send![ws, frontmostApplication];
        if front.is_null() {
            return false;
        }
        let pid: i32 = msg_send![front, processIdentifier];
        pid == std::process::id() as i32
    }
}

#[cfg(not(target_os = "macos"))]
fn orbitty_is_frontmost() -> bool {
    true
}

/// GO-ORBITTY-CHROME-Z: companion chrome is a normal window vs other
/// apps. Floating+1 (4) only while shown AND Orbitty is the frontmost
/// app — that still beats planet discs so Show Companion recovers.
/// Another app frontmost, or hide → NSNormalWindowLevel (0).
fn set_front_level(app: &tauri::AppHandle, shown: bool) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        apply_front_level(&handle, shown);
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (handle, shown);
}

#[cfg(target_os = "macos")]
fn apply_front_level(handle: &tauri::AppHandle, shown: bool) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    let Some(w) = handle.get_webview_window("companion") else {
        return;
    };
    let Ok(raw) = w.ns_window() else {
        return;
    };
    if raw.is_null() {
        return;
    }
    let want: i64 = if shown && orbitty_is_frontmost() { 4 } else { 0 };
    unsafe {
        let nsw = raw as *mut Object;
        let cur: i64 = msg_send![nsw, level];
        if cur != want {
            let _: () = msg_send![nsw, setLevel: want];
        }
    }
}

/// Activate (if we are not already frontmost — the extra hop on every
/// dock click was the menu beach ball) + level 4 + heavy orderFront.
/// One main-thread block so apply_mode cannot leave the window at 0
/// under the planets. ⌘⇧O / tray / dock Reopen / hit-mousedown.
/// GO-ORBITTY-TILE-YAW leftover (summon clamp): a summon ALWAYS recovers
/// — never bail on a hidden companion (the old `!is_visible` return
/// stranded a dock-click-dropped window until Quit + relaunch). Show
/// first, then the dock law: bottom edge sunk below every display (or no
/// 24px handle anywhere) → clamp back on-screen so the dock is
/// clickable — THEN raise.
pub(crate) fn raise_now(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let Some(w) = handle.get_webview_window("companion") else {
            return;
        };
        if !w.is_visible().unwrap_or(false) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_ignore_cursor_events(false);
        }
        crate::drawers::ensure_companion_dock_onscreen(&w);
        let Ok(raw) = w.ns_window() else {
            return;
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let nsw = raw as *mut Object;
            if !orbitty_is_frontmost() {
                let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                if !ns_app.is_null() {
                    let activate = sel!(activate);
                    let responds: bool = msg_send![ns_app, respondsToSelector: activate];
                    if responds {
                        let _: () = msg_send![ns_app, activate];
                    } else {
                        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    }
                }
            }
            let _: () = msg_send![nsw, setLevel: 4i64];
            let _: () = msg_send![nsw, orderFrontRegardless];
            let _: () = msg_send![nsw, orderWindow: 1i64 relativeTo: 0i64];
            let _: () = msg_send![nsw, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
        }
    });
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = handle.get_webview_window("companion") {
            // Summon clamp: show a hidden companion, dock on-screen, then
            // raise (same law as the macOS block above).
            if !w.is_visible().unwrap_or(false) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_ignore_cursor_events(false);
            }
            crate::drawers::ensure_companion_dock_onscreen(&w);
            crate::drawers::appkit_order_front(&handle, &w);
        }
    }
}

/// Planet mouseDown just orderFront'd a disc at the same level. If we
/// are still the frontmost app, lift the companion back above it —
/// no activate (that beach-balls the click). Safari-front: leave it.
pub(crate) fn keep_front_if_ours(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};
        if !orbitty_is_frontmost() {
            return;
        }
        let Some(w) = handle.get_webview_window("companion") else {
            return;
        };
        if !w.is_visible().unwrap_or(false) {
            return;
        }
        let Ok(raw) = w.ns_window() else {
            return;
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let nsw = raw as *mut Object;
            let _: () = msg_send![nsw, setLevel: 4i64];
            let _: () = msg_send![nsw, orderFrontRegardless];
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// GO-ORBITTY-FRONT-HOLD law 5: a mousedown on any ghost hit (dock /
/// spirit / bubble / × ⚙) heavy-raises the companion so no planet
/// restack can steal the window mid-click. CompanionPage fires this on
/// those mousedowns; a hidden companion is a no-op. Native menus
/// (select / planet context) never call this — activate+makeKey during
/// a tracking loop beach-balls.
#[tauri::command]
pub fn companion_front(app: tauri::AppHandle) -> Result<(), String> {
    let Some(w) = app.get_webview_window("companion") else {
        return Ok(());
    };
    if !w.is_visible().unwrap_or(false) {
        return Ok(());
    }
    raise_now(&app);
    Ok(())
}

/// Re-apply slab/ghost from the mirrored mode + live visibility. Called
/// on every companion show (show_drawer forces mouse-on first — a ghost
/// must not stay stuck there), on expand/collapse, and on host on/off.
pub(crate) fn apply_mode(app: &tauri::AppHandle) {
    let shown = app
        .get_webview_window("companion")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if !shown {
        GHOST_ACTIVE.store(false, Ordering::SeqCst);
        SUMMON_HOLD.store(false, Ordering::SeqCst);
        HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
        return;
    }
    // GO-ORBITTY-CHROME-Z: level follows shown + frontmost app (see
    // set_front_level). Summon raise_now after this forces 4.
    set_front_level(app, true);
    if slab() {
        GHOST_ACTIVE.store(false, Ordering::SeqCst);
        SUMMON_HOLD.store(false, Ordering::SeqCst);
        HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
        force_mouse_on(app);
        return;
    }
    GHOST_ACTIVE.store(true, Ordering::SeqCst);
    // GO-ORBITTY-FRONT-HOLD: hold mouse-on until the cursor LEAVES the
    // frame — entering empty air inside it must not spend the hold.
    SUMMON_HOLD.store(true, Ordering::SeqCst);
    HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
    // Immediate cursor test — no 100ms hole before the next poll.
    apply_on_main(app);
}

fn force_mouse_on(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        if let Some(w) = handle.get_webview_window("companion") {
            let _ = w.set_ignore_cursor_events(false);
        }
    });
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = handle.get_webview_window("companion") {
        let _ = w.set_ignore_cursor_events(false);
    }
}

/// Run the ghost cursor test on the main thread (AppKit frame reads +
/// setIgnoresMouseEvents), inline when already there.
fn apply_on_main(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        crate::main_thread_sync(move || ghost_apply(&handle));
    }
    #[cfg(not(target_os = "macos"))]
    ghost_apply(app);
}

/// Main-thread ghost turn. Self-healing: a stale gate or a mode flip
/// since the post is re-checked here, so the poll can only ever apply
/// mouse-off to a shown ghost.
fn ghost_apply(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("companion") else {
        GHOST_ACTIVE.store(false, Ordering::SeqCst);
        SUMMON_HOLD.store(false, Ordering::SeqCst);
        return;
    };
    if !w.is_visible().unwrap_or(false) || slab() {
        GHOST_ACTIVE.store(false, Ordering::SeqCst);
        SUMMON_HOLD.store(false, Ordering::SeqCst);
        HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
        let _ = w.set_ignore_cursor_events(false);
        return;
    }
    match cursor_where(&w) {
        // Fail-open: never strand the spirit unclickable.
        None => {
            SUMMON_HOLD.store(false, Ordering::SeqCst);
            HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
            let _ = w.set_ignore_cursor_events(false);
        }
        // GO-ORBITTY-FRONT-HOLD: the summon hold spends when the cursor
        // LEAVES the frame after having been inside — never when it
        // enters empty air, and never on the first Outside of a ⌘⇧O
        // that landed with the pointer already off the window.
        Some(CursorWhere::Outside) => {
            if HOLD_SEEN_INSIDE.load(Ordering::SeqCst) {
                SUMMON_HOLD.store(false, Ordering::SeqCst);
                HOLD_SEEN_INSIDE.store(false, Ordering::SeqCst);
            }
            let _ = w.set_ignore_cursor_events(true);
        }
        // Cursor inside the frame: an armed summon hold keeps mouse-on
        // until the leave (a summon landing over empty air stays
        // clickable). Hold spent, the hit test runs — InsideEmpty really
        // mouse-offs (air click-throughs, no dead zone), hits don't.
        Some(where_) => {
            HOLD_SEEN_INSIDE.store(true, Ordering::SeqCst);
            if SUMMON_HOLD.load(Ordering::SeqCst) {
                let _ = w.set_ignore_cursor_events(false);
            } else {
                let _ = w.set_ignore_cursor_events(!matches!(where_, CursorWhere::OverHit));
            }
        }
    }
}

/// Cursor vs the companion frame and the kept hit rects.
enum CursorWhere {
    /// Inside the frame and over a reported hit — or any mouse button is
    /// down (the ghost spirit is the drag handle; a live native drag
    /// must not lose the window mid-move).
    OverHit,
    /// Inside the frame, over empty air.
    InsideEmpty,
    /// Outside the frame entirely.
    Outside,
}

/// Cursor position in the window's local top-left points, tested against
/// the frame first, then the kept hit rects. None = the native frame
/// cannot be read — the caller fails open (mouse-on), never stranding
/// the spirit unclickable.
#[cfg(target_os = "macos")]
fn cursor_where(w: &tauri::WebviewWindow) -> Option<CursorWhere> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }
    unsafe {
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if buttons != 0 {
            return Some(CursorWhere::OverHit);
        }
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let raw = w.ns_window().ok()?;
        if raw.is_null() {
            return None;
        }
        let nsw = raw as *mut Object;
        let frame: NSRect = msg_send![nsw, frame];
        // Window-local, top-left origin (CSS points == AppKit points).
        let lx = mouse.x - frame.origin.x;
        let ly = frame.origin.y + frame.size.height - mouse.y;
        if lx < 0.0 || ly < 0.0 || lx > frame.size.width || ly > frame.size.height {
            return Some(CursorWhere::Outside);
        }
        let list = hits().lock().map(|h| h.clone()).unwrap_or_default();
        let over = list
            .iter()
            .any(|r| lx >= r.x && lx <= r.x + r.w && ly >= r.y && ly <= r.y + r.h);
        Some(if over {
            CursorWhere::OverHit
        } else {
            CursorWhere::InsideEmpty
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn cursor_where(_w: &tauri::WebviewWindow) -> Option<CursorWhere> {
    Some(CursorWhere::OverHit) // ghost never click-throughs off macOS
}

/// Seed the expanded mirror from persisted state (host is always off at
/// boot — the stash is memory-only) and run the 10Hz poll. The poll
/// posts to the main queue and only works while shown-and-ghost; hidden
/// or slab it idles at 300ms.
pub fn start(app: tauri::AppHandle) {
    if let Ok(st) = crate::state::load_state(&app) {
        EXPANDED.store(st.companion.expanded, Ordering::SeqCst);
    }
    std::thread::spawn(move || loop {
        // GO-ORBITTY-CHROME-Z: hosted Launchpad is a slab (ghost poll
        // idles). Still re-apply z-order so resign-active drops level 4.
        // Nested planet menus: skip — dispatch_sync into a tracking loop
        // was the beach ball while digging into Assign-to-planet.
        if MENU_QUIET.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        let shown = app
            .get_webview_window("companion")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false);
        if shown {
            let handle = app.clone();
            #[cfg(target_os = "macos")]
            crate::main_thread_async(move || {
                apply_front_level(&handle, true);
            });
            #[cfg(not(target_os = "macos"))]
            set_front_level(&handle, true);
        }
        if GHOST_ACTIVE.load(Ordering::SeqCst) {
            #[cfg(target_os = "macos")]
            {
                let handle = app.clone();
                crate::main_thread_async(move || ghost_apply(&handle));
            }
            #[cfg(not(target_os = "macos"))]
            apply_on_main(&app);
            std::thread::sleep(std::time::Duration::from_millis(100));
        } else {
            std::thread::sleep(std::time::Duration::from_millis(if shown { 100 } else { 300 }));
        }
    });
}
