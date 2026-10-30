//! macOS window corral — AX-first enumeration + focus.
//! Port of the Swift reference (WindowCatalog / WindowFocuser / PermissionGate /
//! AXHelpers). AXUIElement is a CFType, so the AX surface is plain C FFI; AppKit
//! (NSWorkspace / NSRunningApplication) goes through the objc runtime.

// objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that newer
// rustc flags; harmless upstream noise.
#![allow(unexpected_cfgs)]

use super::{AccessibilityStatus, WindowInfo};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

mod windows_mac_ax;
use windows_mac_ax::{enumerate, mark_frontmost};

type CFRef = *const c_void;
type AXUIElementRef = CFRef;
type AXError = i32;

const AX_SUCCESS: AXError = 0;
const K_AX_VALUE_TYPE_CG_POINT: i32 = 1;
const K_AX_VALUE_TYPE_CG_SIZE: i32 = 2;
const K_CF_NUMBER_S_INT64_TYPE: i32 = 4;
const K_SEC_CS_SIGNING_INFORMATION: u32 = 1 << 1;
const K_SEC_CODE_SIGNATURE_ADHOC: i64 = 2;

#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
struct NSRect {
    origin: NSPoint,
    size: CGSize,
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRetain(cf: CFRef) -> CFRef;
    fn CFRelease(cf: CFRef);
    fn CFGetTypeID(cf: CFRef) -> u64;
    fn CFStringGetTypeID() -> u64;
    fn CFURLGetTypeID() -> u64;
    fn CFURLGetString(url: CFRef) -> CFRef;
    fn CFArrayGetCount(array: CFRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFRef, idx: isize) -> CFRef;
    fn CFDictionaryGetValue(dict: CFRef, key: CFRef) -> CFRef;
    fn CFNumberGetValue(number: CFRef, the_type: i32, value_ptr: *mut c_void) -> bool;
    fn CFBooleanGetValue(boolean: CFRef) -> bool;
    fn CFURLCreateWithFileSystemPath(
        allocator: CFRef,
        path: CFRef,
        style: isize,
        is_directory: bool,
    ) -> CFRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFRef) -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_s: f32) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFRef,
        value: *mut CFRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(element: AXUIElementRef, attribute: CFRef, value: CFRef)
        -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFRef) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueGetTypeID() -> u64;
    fn AXValueGetValue(value: CFRef, the_type: i32, value_ptr: *mut c_void) -> bool;
    // Private HIServices API, stable in practice and used by FOSS window tools
    // (AltTab etc.) to correlate an AX window with its CGWindowID.
    fn _AXUIElementGetWindow(element: AXUIElementRef, window_id: *mut u32) -> AXError;
}

// The kAX* name constants are not exported through the ApplicationServices
// umbrella .tbd (only the functions are), so build them as CFStrings once.
struct AxNames {
    windows: CFRef,
    title: CFRef,
    minimized: CFRef,
    role: CFRef,
    size: CFRef,
    position: CFRef,
    focused_app: CFRef,
    focused_window: CFRef,
    main: CFRef,
    raise: CFRef,
    close_button: CFRef,
    press: CFRef,
    trusted_check_prompt: CFRef,
    document: CFRef,
}

// Immutable, process-lifetime CFString constants — safe to share across threads.
unsafe impl Send for AxNames {}
unsafe impl Sync for AxNames {}

fn ax_names() -> &'static AxNames {
    static NAMES: OnceLock<AxNames> = OnceLock::new();
    NAMES.get_or_init(|| {
        fn name(s: &str) -> CFRef {
            let cf = CFString::new(s);
            let raw = cf.as_concrete_TypeRef() as CFRef;
            std::mem::forget(cf); // process-lifetime constant
            raw
        }
        AxNames {
            windows: name("AXWindows"),
            title: name("AXTitle"),
            minimized: name("AXMinimized"),
            role: name("AXRole"),
            size: name("AXSize"),
            position: name("AXPosition"),
            focused_app: name("AXFocusedApplication"),
            focused_window: name("AXFocusedWindow"),
            main: name("AXMain"),
            raise: name("AXRaise"),
            close_button: name("AXCloseButton"),
            press: name("AXPress"),
            trusted_check_prompt: name("AXTrustedCheckOptionPrompt"),
            document: name("AXDocument"),
        }
    })
}

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecStaticCodeCreateWithPath(path: CFRef, flags: u32, static_code: *mut CFRef) -> i32;
    fn SecCodeCopySigningInformation(code: CFRef, flags: u32, info: *mut CFRef) -> i32;
    fn SecCertificateCopyCommonName(cert: CFRef, common_name: *mut CFRef) -> i32;

    static kSecCodeInfoFlags: CFRef;
    static kSecCodeInfoTeamIdentifier: CFRef;
    static kSecCodeInfoIdentifier: CFRef;
    static kSecCodeInfoCertificates: CFRef;
}

// AX helpers (port of AXHelpers.swift)

/// Copy rule: caller owns the returned ref.
unsafe fn ax_copy(element: AXUIElementRef, attribute: CFRef) -> Option<CFRef> {
    let mut value: CFRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(element, attribute, &mut value) == AX_SUCCESS
        && !value.is_null()
    {
        Some(value)
    } else {
        None
    }
}

unsafe fn ax_string(element: AXUIElementRef, attribute: CFRef) -> Option<String> {
    let raw = ax_copy(element, attribute)?;
    Some(CFString::wrap_under_create_rule(raw as _).to_string())
}

unsafe fn ax_bool(element: AXUIElementRef, attribute: CFRef) -> Option<bool> {
    let raw = ax_copy(element, attribute)?;
    let value = CFBooleanGetValue(raw);
    CFRelease(raw);
    Some(value)
}

unsafe fn ax_size(element: AXUIElementRef, attribute: CFRef) -> Option<CGSize> {
    let raw = ax_copy(element, attribute)?;
    let mut out = None;
    if CFGetTypeID(raw) == AXValueGetTypeID() {
        let mut size = CGSize {
            width: 0.0,
            height: 0.0,
        };
        if AXValueGetValue(raw, K_AX_VALUE_TYPE_CG_SIZE, &mut size as *mut _ as *mut c_void) {
            out = Some(size);
        }
    }
    CFRelease(raw);
    out
}

/// AXPosition arrives as an AXValue-wrapped CGPoint (top-left origin).
unsafe fn ax_point(element: AXUIElementRef, attribute: CFRef) -> Option<NSPoint> {
    let raw = ax_copy(element, attribute)?;
    let mut out = None;
    if CFGetTypeID(raw) == AXValueGetTypeID() {
        let mut pt = NSPoint { x: 0.0, y: 0.0 };
        if AXValueGetValue(raw, K_AX_VALUE_TYPE_CG_POINT, &mut pt as *mut _ as *mut c_void) {
            out = Some(pt);
        }
    }
    CFRelease(raw);
    out
}

unsafe fn ax_window_id(element: AXUIElementRef) -> Option<u32> {
    let mut id = 0u32;
    if _AXUIElementGetWindow(element, &mut id) == AX_SUCCESS && id != 0 {
        Some(id)
    } else {
        None
    }
}


unsafe fn activate(app: *mut Object) {
    let current: *mut Object = msg_send![class!(NSRunningApplication), currentApplication];
    // macOS 14+ activation API; fall back to the legacy options call below it.
    let modern = sel!(activateFromApplication:);
    let responds: bool = msg_send![app, respondsToSelector: modern];
    if responds {
        let _: bool = msg_send![app, activateFromApplication: current];
    } else {
        // NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps
        let _: bool = msg_send![app, activateWithOptions: 3usize];
    }
}


/// Messaging timeout for every AX element this GO creates while `ax_safe` is on.
pub(crate) const AX_TIMEOUT_S: f32 = 0.25;

/// One AX application element. Drop releases it.
pub(crate) struct AxApp(AXUIElementRef);

impl AxApp {
    /// Null create → None. Timeout is set only while `ax_safe` is on.
    pub(crate) fn for_pid(pid: i32) -> Option<AxApp> {
        let el = unsafe { AXUIElementCreateApplication(pid) };
        if el.is_null() {
            return None;
        }
        if ax_safe_on() {
            set_ax_timeout(el);
        }
        Some(AxApp(el))
    }

    pub(crate) fn raw(&self) -> AXUIElementRef {
        self.0
    }
}

impl Drop for AxApp {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn set_ax_timeout(el: AXUIElementRef) {
    let _ = unsafe { AXUIElementSetMessagingTimeout(el, AX_TIMEOUT_S) };
    #[cfg(test)]
    test_seam::record(AX_TIMEOUT_S);
}

#[cfg(test)]
pub(crate) mod test_seam {
    use std::cell::RefCell;

    thread_local! {
        static RECORDED: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(v: f32) {
        RECORDED.with(|c| c.borrow_mut().push(v));
    }

    pub(crate) fn clear() {
        RECORDED.with(|c| c.borrow_mut().clear());
    }

    pub(crate) fn recorded() -> Vec<f32> {
        RECORDED.with(|c| c.borrow().clone())
    }
}

fn ax_safe_on() -> bool {
    crate::flags::ax_safe_on()
}

/// Flag on: timed `AxApp`. Flag off: the old create, released on drop.
fn ax_app_ref(pid: i32) -> Option<AxRef> {
    if ax_safe_on() {
        AxApp::for_pid(pid).map(AxRef::Safe)
    } else {
        let el = unsafe { AXUIElementCreateApplication(pid) };
        if el.is_null() {
            None
        } else {
            Some(AxRef::Raw(el))
        }
    }
}

enum AxRef {
    Safe(AxApp),
    Raw(AXUIElementRef),
}

impl AxRef {
    fn raw(&self) -> AXUIElementRef {
        match self {
            AxRef::Safe(app) => app.raw(),
            AxRef::Raw(el) => *el,
        }
    }
}

impl Drop for AxRef {
    fn drop(&mut self) {
        if let AxRef::Raw(el) = self {
            unsafe { CFRelease(*el) }
        }
    }
}

/// Own process, any Orbitty bundle, or an "Orbitty…" localized name.
pub(crate) fn is_self_app(
    bundle: Option<&str>,
    name: Option<&str>,
    pid: i32,
    own_pid: i32,
) -> bool {
    if pid == own_pid {
        return true;
    }
    if let Some(bundle) = bundle {
        if bundle.starts_with("space.orbitty.") {
            return true;
        }
    }
    if let Some(name) = name {
        if name == "Orbitty" || name.starts_with("Orbitty ") {
            return true;
        }
    }
    false
}

fn is_orbitty_win(w: &WindowInfo) -> bool {
    if ax_safe_on() {
        return is_self_app(
            w.bundle_id.as_deref(),
            Some(w.app_name.as_str()),
            w.pid,
            std::process::id() as i32,
        );
    }
    matches!(w.bundle_id.as_deref(), Some("space.orbitty.mac"))
        || w.app_name.eq_ignore_ascii_case("Orbitty")
}

static LAST_OTHER_FRONTMOST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn note_other_frontmost(list: &[WindowInfo]) {
    if let Some(w) = list
        .iter()
        .find(|w| w.is_frontmost && !w.is_minimized && !is_orbitty_win(w))
    {
        remember_other_id(w.id.clone());
    }
}

pub fn last_other_frontmost_id() -> Option<String> {
    LAST_OTHER_FRONTMOST.lock().ok().and_then(|g| g.clone())
}

fn remember_other_id(id: String) {
    if let Ok(mut g) = LAST_OTHER_FRONTMOST.lock() {
        *g = Some(id);
    }
}

struct Front {
    pid: i32,
    bundle: Option<String>,
    name: Option<String>,
}

fn ns_utf8(obj: *mut Object) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    unsafe {
        let utf: *const i8 = msg_send![obj, UTF8String];
        if utf.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf).to_string_lossy().into_owned())
    }
}

/// NSWorkspace frontmost only. Safe to run inside `main_thread_sync`.
fn read_workspace_front() -> Option<Front> {
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return None;
        }
        let app: *mut Object = msg_send![ws, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let bundle: *mut Object = msg_send![app, bundleIdentifier];
        let name: *mut Object = msg_send![app, localizedName];
        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            return None;
        }
        Some(Front {
            pid,
            bundle: ns_utf8(bundle),
            name: ns_utf8(name),
        })
    }
}

/// Focused-window id for `pid`. Runs on the caller's thread.
fn ax_focused_window_id(pid: i32) -> Option<String> {
    let ax = ax_app_ref(pid)?;
    unsafe {
        let focused = ax_copy(ax.raw(), ax_names().focused_window)?;
        let id = ax_window_id(focused).map(|cg_id| format!("{pid}:{cg_id}"));
        CFRelease(focused);
        id
    }
}

/// Global (all displays) last non-Orbitty frontmost — NSWorkspace, not
/// CG z-order on the tray's screen.
pub fn note_workspace_frontmost() {
    if ax_safe_on() {
        let Some(front) = read_workspace_front() else {
            return;
        };
        if is_self_app(
            front.bundle.as_deref(),
            front.name.as_deref(),
            front.pid,
            std::process::id() as i32,
        ) {
            return;
        }
        if let Some(id) = ax_focused_window_id(front.pid) {
            remember_other_id(id);
        }
    } else {
        unsafe {
            let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
            if ws.is_null() {
                return;
            }
            let app: *mut Object = msg_send![ws, frontmostApplication];
            if app.is_null() {
                return;
            }
            let bundle: *mut Object = msg_send![app, bundleIdentifier];
            if !bundle.is_null() {
                let utf: *const i8 = msg_send![bundle, UTF8String];
                if !utf.is_null() {
                    let s = std::ffi::CStr::from_ptr(utf).to_string_lossy();
                    if s == "space.orbitty.mac" {
                        return;
                    }
                }
            }
            let name: *mut Object = msg_send![app, localizedName];
            if !name.is_null() {
                let utf: *const i8 = msg_send![name, UTF8String];
                if !utf.is_null() {
                    let s = std::ffi::CStr::from_ptr(utf).to_string_lossy();
                    if s.eq_ignore_ascii_case("Orbitty") {
                        return;
                    }
                }
            }
            let pid: i32 = msg_send![app, processIdentifier];
            if pid <= 0 {
                return;
            }
            let ax_app = AXUIElementCreateApplication(pid);
            if ax_app.is_null() {
                return;
            }
            if let Some(focused_win) = ax_copy(ax_app, ax_names().focused_window) {
                if let Some(cg_id) = ax_window_id(focused_win) {
                    remember_other_id(format!("{pid}:{cg_id}"));
                }
                CFRelease(focused_win);
            }
            CFRelease(ax_app);
        }
    }
}

/// Keep LAST_OTHER fresh across displays while the user works — tray
/// claim then resolves the window they were in, not the menu-bar screen.
pub fn install_frontmost_tracker() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::thread::Builder::new()
            .name("orbitty-frontmost".into())
            .spawn(|| {
                let safe = ax_safe_on();
                loop {
                    if safe {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let _ = crate::main_thread_sync(move || {
                            let _ = tx.send(read_workspace_front());
                        });
                        if let Some(front) = rx.recv().unwrap_or(None) {
                            if !is_self_app(
                                front.bundle.as_deref(),
                                front.name.as_deref(),
                                front.pid,
                                std::process::id() as i32,
                            ) {
                                if let Some(id) = ax_focused_window_id(front.pid) {
                                    remember_other_id(id);
                                }
                            }
                        }
                    } else {
                        let _ = crate::main_thread_sync(|| {
                            note_workspace_frontmost();
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            })
            .ok();
    });
}


/// Frontmost claimable window across the **whole desktop** (not the
/// tray's monitor). Real AX frontmost if it isn't Orbitty; else the
/// remembered last non-Orbitty frontmost (NSWorkspace tracker + list
/// polls). No CG z-order fallback — that preferred whichever app sat on
/// the menu-bar display (Proton Mail over Calculator on another screen).
pub fn frontmost_claimable() -> Result<Option<WindowInfo>, String> {
    // Refresh memory from NSWorkspace before we resolve — cheap, and
    // covers the gap between rail polls when the user focused a window
    // on another display then opened the tray.
    note_workspace_frontmost();
    let list = list_windows()?;
    if let Some(w) = list
        .iter()
        .find(|w| w.is_frontmost && !w.is_minimized && !is_orbitty_win(w))
    {
        return Ok(Some(w.clone()));
    }
    if let Some(id) = last_other_frontmost_id() {
        if let Some(w) = list
            .iter()
            .find(|w| w.id == id && !w.is_minimized && !is_orbitty_win(w))
        {
            return Ok(Some(w.clone()));
        }
    }
    Ok(None)
}

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    unsafe {
        let mut windows = enumerate();
        mark_frontmost(&mut windows);
        note_other_frontmost(&windows);
        windows.sort_by(|a, b| {
            b.is_frontmost
                .cmp(&a.is_frontmost)
                .then(a.is_minimized.cmp(&b.is_minimized))
                .then_with(|| {
                    a.app_name
                        .to_lowercase()
                        .cmp(&b.app_name.to_lowercase())
                })
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(windows)
    }
}

// focus (port of WindowFocuser)

pub fn focus_window(id: &str) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    let (pid, rest) = parse_id(id)?;

    unsafe {
        let app: *mut Object =
            msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return Err("app no longer running".into());
        }
        let terminated: bool = msg_send![app, isTerminated];
        if terminated {
            return Err("app terminated".into());
        }
        activate(app);

        let ax = match ax_app_ref(pid) {
            Some(ax) => ax,
            None => return Err("could not attach to app".into()),
        };
        focus_in_app(ax.raw(), rest)
    }
}

/// AX handles go stale between polls, so re-find the window inside the owning
/// app (Swift `rematch`). Returns a retained handle, or null when the window
/// is gone. Caller releases.
unsafe fn find_window(ax_app: AXUIElementRef, rest: &str) -> AXUIElementRef {
    let windows = match ax_copy(ax_app, ax_names().windows) {
        Some(w) => w,
        None => return std::ptr::null(),
    };
    let count = CFArrayGetCount(windows);
    let mut target: AXUIElementRef = std::ptr::null();

    if let Ok(cg_id) = rest.parse::<u32>() {
        for i in 0..count {
            let w = CFArrayGetValueAtIndex(windows, i);
            if !w.is_null() && ax_window_id(w) == Some(cg_id) {
                target = w;
                break;
            }
        }
    } else if let Some(stripped) = rest.strip_prefix("t:") {
        // Fallback id `t:<title>#<ordinal>` — re-match by title among real windows.
        if let Some(hash) = stripped.rfind('#') {
            let title = &stripped[..hash];
            let ordinal: usize = stripped[hash + 1..].parse().unwrap_or(0);
            let mut seen = 0usize;
            for i in 0..count {
                let w = CFArrayGetValueAtIndex(windows, i);
                if w.is_null() {
                    continue;
                }
                if ax_string(w, ax_names().role).as_deref() != Some("AXWindow") {
                    continue;
                }
                if ax_string(w, ax_names().title).unwrap_or_default() == title {
                    seen += 1;
                    if seen == ordinal {
                        target = w;
                        break;
                    }
                }
            }
        }
    }

    if target.is_null() {
        CFRelease(windows);
        return std::ptr::null();
    }
    let target = CFRetain(target);
    CFRelease(windows);
    target
}

/// Split a window id into (pid, match-key). Shared by focus/minimize/restore.
fn parse_id(id: &str) -> Result<(i32, &str), String> {
    let (pid_part, rest) = id
        .split_once(':')
        .ok_or_else(|| format!("bad window id: {id}"))?;
    let pid: i32 = pid_part
        .parse()
        .map_err(|_| format!("bad window id: {id}"))?;
    Ok((pid, rest))
}

/// Set AXMinimized without activating the owning app (Swift StowController).
fn set_minimized(id: &str, minimized: bool) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    let (pid, rest) = parse_id(id)?;
    unsafe {
        let ax = match ax_app_ref(pid) {
            Some(ax) => ax,
            None => return Err("could not attach to app".into()),
        };
        let target = find_window(ax.raw(), rest);
        if target.is_null() {
            return Err("window no longer exists".into());
        }
        let current = ax_bool(target, ax_names().minimized).unwrap_or(false);
        let mut result = Ok(());
        if current != minimized {
            let value = if minimized {
                CFBoolean::true_value()
            } else {
                CFBoolean::false_value()
            };
            if AXUIElementSetAttributeValue(
                target,
                ax_names().minimized,
                value.as_concrete_TypeRef() as CFRef,
            ) != AX_SUCCESS
            {
                result = Err(if minimized {
                    "minimize refused by app".into()
                } else {
                    "restore refused by app".into()
                });
            }
        }
        CFRelease(target);
        result
    }
}

/// Rail Stow: ordinary AX minimize — the window sits in the Dock like any
/// other minimized window. Orbitty never parks windows offscreen.
pub fn minimize_window(id: &str) -> Result<(), String> {
    set_minimized(id, true)
}

/// Deminiaturize only, no activation — the tray's restore-all escape hatch.
pub fn deminiaturize_window(id: &str) -> Result<(), String> {
    set_minimized(id, false)
}

/// Sprite click / rail Restore: deminiaturize + focus (Swift `restore(id:)`).
pub fn restore_window(id: &str) -> Result<(), String> {
    focus_window(id)
}

/// Circle-complete: press the window's own AX close button (same door as the
/// red traffic light / ⌘W) — never a pid kill, so the app can still prompt
/// to save. A refusal (no close button, app ignores the press) is an Err the
/// rail swallows: the card still completes.
pub fn close_window(id: &str) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    let (pid, rest) = parse_id(id)?;
    unsafe {
        let ax = match ax_app_ref(pid) {
            Some(ax) => ax,
            None => return Err("could not attach to app".into()),
        };
        let target = find_window(ax.raw(), rest);
        if target.is_null() {
            return Err("window no longer exists".into());
        }
        let result = match ax_copy(target, ax_names().close_button) {
            Some(btn) if !btn.is_null() => {
                let pressed = AXUIElementPerformAction(btn, ax_names().press);
                CFRelease(btn);
                if pressed == AX_SUCCESS {
                    Ok(())
                } else {
                    Err("close refused by app".into())
                }
            }
            _ => Err("window has no close button".into()),
        };
        CFRelease(target);
        result
    }
}

// Locate ring

// GCD main-queue hops (same binding trick as habitat_mac): the SDK's
// dispatch_get_main_queue() is a header inline returning &_dispatch_main_q.
extern "C" {
    #[link_name = "_dispatch_main_q"]
    static DISPATCH_MAIN_Q: c_void;
    fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_after_f(
        when: u64,
        queue: *mut c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn dispatch_time(when: i64, delta: i64) -> u64;
}

/// One ring panel at a time; a re-locate replaces it. The hide timer is
/// generation-gated so a stale 500ms pop never kills a freshly re-armed ring.
static LOCATE_PANEL: AtomicUsize = AtomicUsize::new(0);
static LOCATE_GEN: AtomicU64 = AtomicU64::new(0);

/// AX frame (top-left origin) carried onto the main queue.
struct RingJob {
    ax_x: f64,
    ax_y: f64,
    w: f64,
    h: f64,
}

/// Rail row left-click locate: an Orbitty-owned, borderless, ignore-mouse,
/// non-activating ring around the window's AX frame for ~500ms. Never
/// activates the app, never AX-raises — the window keeps its focus state.
pub fn locate_window(id: &str) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    let (pid, rest) = parse_id(id)?;
    unsafe {
        let ax = match ax_app_ref(pid) {
            Some(ax) => ax,
            None => return Err("could not attach to app".into()),
        };
        let target = find_window(ax.raw(), rest);
        if target.is_null() {
            return Err("window no longer exists".into());
        }
        let minimized = ax_bool(target, ax_names().minimized).unwrap_or(false);
        let frame = match (
            ax_point(target, ax_names().position),
            ax_size(target, ax_names().size),
        ) {
            (Some(p), Some(s)) => Some((p, s)),
            _ => None,
        };
        CFRelease(target);
        drop(ax);
        if minimized {
            // A minimized window has no on-screen frame to ring; the rail
            // routes minimized claims through the walker stroke instead.
            return Err("window is minimized".into());
        }
        let (pos, size) = frame.ok_or_else(|| "window frame unreadable".to_string())?;
        let job = Box::into_raw(Box::new(RingJob {
            ax_x: pos.x,
            ax_y: pos.y,
            w: size.width,
            h: size.height,
        })) as *mut c_void;
        dispatch_async_f(
            &DISPATCH_MAIN_Q as *const c_void as *mut c_void,
            job,
            show_ring_main,
        );
        Ok(())
    }
}

extern "C" fn show_ring_main(ctx: *mut c_void) {
    let job = unsafe { Box::from_raw(ctx as *mut RingJob) };
    unsafe {
        // AX (top-left of the primary) → AppKit (bottom-left of the primary).
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return;
        }
        let primary: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        if primary.is_null() {
            return;
        }
        let pf: NSRect = msg_send![primary, frame];
        let pad = 6.0;
        let rect = NSRect {
            origin: NSPoint {
                x: job.ax_x - pad,
                y: pf.size.height - job.ax_y - job.h - pad,
            },
            size: CGSize {
                width: job.w + pad * 2.0,
                height: job.h + pad * 2.0,
            },
        };
        // Replace any ring still up; its hide timer is gen-gated and no-ops.
        let prev = LOCATE_PANEL.swap(0, Ordering::SeqCst);
        if prev != 0 {
            let p = prev as *mut Object;
            let _: () = msg_send![p, orderOut: std::ptr::null::<Object>()];
            let _: () = msg_send![p, release];
        }
        let panel: *mut Object = msg_send![class!(NSPanel), alloc];
        // NSWindowStyleMaskNonactivatingPanel (1<<7) — borderless, never
        // activates. Shown over everything at the floating level.
        let panel: *mut Object = msg_send![panel,
            initWithContentRect: rect
            styleMask: 128usize
            backing: 2usize // NSBackingStoreBuffered
            defer: false];
        if panel.is_null() {
            return;
        }
        let _: () = msg_send![panel, setLevel: 3i64]; // NSFloatingWindowLevel
        // canJoinAllSpaces (1) | stationary (1<<4) | fullScreenAuxiliary (1<<8)
        let _: () = msg_send![panel, setCollectionBehavior: 273usize];
        let _: () = msg_send![panel, setOpaque: false];
        let clear: *mut Object = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![panel, setBackgroundColor: clear];
        let _: () = msg_send![panel, setHasShadow: false];
        let _: () = msg_send![panel, setIgnoresMouseEvents: true];
        let _: () = msg_send![panel, setReleasedWhenClosed: false];
        let _: () = msg_send![panel, setHidesOnDeactivate: false];
        let _: () = msg_send![panel, setCanHide: false];
        // The ring is the content view's layer border — no custom draw pass.
        let content: *mut Object = msg_send![panel, contentView];
        if !content.is_null() {
            let _: () = msg_send![content, setWantsLayer: true];
            let layer: *mut Object = msg_send![content, layer];
            if !layer.is_null() {
                let white: *mut Object = msg_send![class!(NSColor), whiteColor];
                let ring: *mut Object = msg_send![white, colorWithAlphaComponent: 0.85f64];
                let cg: *mut Object = msg_send![ring, CGColor];
                let _: () = msg_send![layer, setBorderColor: cg];
                let _: () = msg_send![layer, setBorderWidth: 2.5f64];
                let _: () = msg_send![layer, setCornerRadius: 10.0f64];
            }
        }
        let _: () = msg_send![panel, orderFrontRegardless];
        LOCATE_PANEL.store(panel as usize, Ordering::SeqCst);
        let gen = LOCATE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        let timer = Box::into_raw(Box::new((panel as usize, gen))) as *mut c_void;
        let when = dispatch_time(0, 500_000_000); // DISPATCH_TIME_NOW + 500ms
        dispatch_after_f(
            when,
            &DISPATCH_MAIN_Q as *const c_void as *mut c_void,
            timer,
            hide_ring_main,
        );
    }
}

extern "C" fn hide_ring_main(ctx: *mut c_void) {
    let (panel, gen) = unsafe { *Box::from_raw(ctx as *mut (usize, u64)) };
    // A newer locate replaced this panel (and already closed it) — no-op.
    if LOCATE_GEN.load(Ordering::SeqCst) != gen {
        return;
    }
    unsafe {
        let p = panel as *mut Object;
        let _: () = msg_send![p, orderOut: std::ptr::null::<Object>()];
        let _: () = msg_send![p, release];
    }
    LOCATE_PANEL.store(0, Ordering::SeqCst);
}

/// App is already activated; missing window is not an error.
unsafe fn focus_in_app(ax_app: AXUIElementRef, rest: &str) -> Result<(), String> {
    let target = find_window(ax_app, rest);
    if target.is_null() {
        return Ok(()); // activated app only
    }

    if ax_bool(target, ax_names().minimized).unwrap_or(false) {
        AXUIElementSetAttributeValue(
            target,
            ax_names().minimized,
            CFBoolean::false_value().as_concrete_TypeRef() as CFRef,
        );
    }
    AXUIElementPerformAction(target, ax_names().raise);
    AXUIElementSetAttributeValue(ax_app, ax_names().focused_window, target);
    AXUIElementSetAttributeValue(
        target,
        ax_names().main,
        CFBoolean::true_value().as_concrete_TypeRef() as CFRef,
    );
    CFRelease(target);
    Ok(())
}

// permission gate (port of PermissionGate)

pub fn accessibility_status() -> AccessibilityStatus {
    let trusted = unsafe { AXIsProcessTrusted() };
    let bundle_path = bundle_path();
    let in_bundle = bundle_path.ends_with(".app");
    let signing = read_signing(&bundle_path);
    let signing_line = if !in_bundle {
        "Running unbundled (tauri dev) — grant Accessibility to this exact binary; \
         a rebuild can invalidate the grant."
            .to_string()
    } else if signing.adhoc {
        format!(
            "Signed ad-hoc ({}) — Accessibility breaks on every rebuild.",
            signing.identifier
        )
    } else {
        let mut parts = vec!["Signed".to_string()];
        parts.push(
            signing
                .common_name
                .clone()
                .unwrap_or_else(|| signing.identifier.clone()),
        );
        if let Some(team) = &signing.team {
            parts.push(format!("team {team}"));
        }
        parts.join(" · ")
    };
    AccessibilityStatus {
        trusted,
        bundle_path,
        signing_line,
        in_bundle,
        is_adhoc: signing.adhoc,
    }
}

/// What TCC ties the grant to: the .app when bundled, else the exact binary.
fn bundle_path() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let path = exe.to_string_lossy().into_owned();
    match path.find(".app/") {
        Some(idx) => path[..idx + 4].to_string(),
        None => path,
    }
}

struct SigningInfo {
    adhoc: bool,
    identifier: String,
    team: Option<String>,
    common_name: Option<String>,
}

fn read_signing(path: &str) -> SigningInfo {
    let mut info = SigningInfo {
        adhoc: false,
        identifier: "space.orbitty.mac".into(),
        team: None,
        common_name: None,
    };
    unsafe {
        let cf_path = CFString::new(path);
        let url = CFURLCreateWithFileSystemPath(
            std::ptr::null(),
            cf_path.as_concrete_TypeRef() as CFRef,
            0, // kCFURLPOSIXPathStyle
            path.ends_with(".app"),
        );
        if url.is_null() {
            return info;
        }
        let mut code: CFRef = std::ptr::null();
        let status = SecStaticCodeCreateWithPath(url, 0, &mut code);
        CFRelease(url);
        if status != 0 || code.is_null() {
            return info;
        }
        let mut dict: CFRef = std::ptr::null();
        let status = SecCodeCopySigningInformation(code, K_SEC_CS_SIGNING_INFORMATION, &mut dict);
        CFRelease(code);
        if status != 0 || dict.is_null() {
            return info;
        }

        let flags_ref = CFDictionaryGetValue(dict, kSecCodeInfoFlags);
        if !flags_ref.is_null() {
            let mut flags: i64 = 0;
            if CFNumberGetValue(
                flags_ref,
                K_CF_NUMBER_S_INT64_TYPE,
                &mut flags as *mut _ as *mut c_void,
            ) {
                info.adhoc = (flags & K_SEC_CODE_SIGNATURE_ADHOC) != 0;
            }
        }
        if let Some(id) = dict_string(dict, kSecCodeInfoIdentifier) {
            info.identifier = id;
        }
        info.team = dict_string(dict, kSecCodeInfoTeamIdentifier);

        let certs = CFDictionaryGetValue(dict, kSecCodeInfoCertificates);
        if !certs.is_null() && CFArrayGetCount(certs) > 0 {
            let leaf = CFArrayGetValueAtIndex(certs, 0);
            if !leaf.is_null() {
                let mut name: CFRef = std::ptr::null();
                if SecCertificateCopyCommonName(leaf, &mut name) == 0 && !name.is_null() {
                    info.common_name =
                        Some(CFString::wrap_under_create_rule(name as _).to_string());
                }
            }
        }
        CFRelease(dict);
    }
    info
}

unsafe fn dict_string(dict: CFRef, key: CFRef) -> Option<String> {
    let value = CFDictionaryGetValue(dict, key);
    if value.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(value as _).to_string())
}

pub fn open_accessibility_settings() {
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        // Current System Settings deep link first, then legacy, then the app itself.
        let candidates = [
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        ];
        for candidate in candidates {
            let cf = CFString::new(candidate);
            let url: *mut Object =
                msg_send![class!(NSURL), URLWithString: cf.as_concrete_TypeRef() as *mut Object];
            if url.is_null() {
                continue;
            }
            let ok: bool = msg_send![workspace, openURL: url];
            if ok {
                return;
            }
        }
        let cf = CFString::new("/System/Applications/System Settings.app");
        let url: *mut Object =
            msg_send![class!(NSURL), fileURLWithPath: cf.as_concrete_TypeRef() as *mut Object];
        if !url.is_null() {
            let _: bool = msg_send![workspace, openURL: url];
        }
    }
}

/// Shows the system prompt. Safe to call again after the user toggles Settings.
pub fn prompt_accessibility() -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(ax_names().trusted_check_prompt as _);
        let dict: CFDictionary<CFString, CFBoolean> =
            CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as CFRef)
    }
}

/// Local other-mouse monitor. Phase 0 never saw a device, so NSEvent
/// buttonNumber 3 and 4 pass through as DOM buttons 3 and 4.
/// Down stays in the mask; an up (button bit already clear) is the one event.
static RAIL_MOUSE_APP: OnceLock<AppHandle> = OnceLock::new();

#[repr(C)]
struct RailMouseBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const RailMouseBlock, *mut Object) -> *mut Object,
    descriptor: *const RailMouseBlockDescriptor,
}

#[repr(C)]
struct RailMouseBlockDescriptor {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static RAIL_MOUSE_GLOBAL_BLOCK: c_void;
}

const RAIL_MOUSE_BLOCK_GLOBAL: i32 = 1 << 28;

static RAIL_MOUSE_BLOCK_DESCRIPTOR: RailMouseBlockDescriptor = RailMouseBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<RailMouseBlock>() as std::os::raw::c_ulong,
};

fn rail_mouse_block() -> *const RailMouseBlock {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(RailMouseBlock {
            isa: unsafe { &RAIL_MOUSE_GLOBAL_BLOCK as *const c_void },
            flags: RAIL_MOUSE_BLOCK_GLOBAL,
            reserved: 0,
            invoke: rail_mouse_invoke,
            descriptor: &RAIL_MOUSE_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const RailMouseBlock
}

extern "C" fn rail_mouse_invoke(_block: *const RailMouseBlock, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        let ty: u64 = msg_send![event, type];
        if !rail_mouse_other_mouse_up(ty) {
            return;
        }
        let button: isize = msg_send![event, buttonNumber];
        if button != 3 && button != 4 {
            return;
        }
        let Some(app) = RAIL_MOUSE_APP.get() else {
            return;
        };
        let _ = app.emit("rail:mouse-button", button as i32);
    }));
    event
}

pub fn install_rail_mouse_buttons(app: AppHandle) {
    let _ = RAIL_MOUSE_APP.set(app);
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if INSTALLED.set(()).is_err() {
        return;
    }
    unsafe {
        // NSEventMaskOtherMouseDown | NSEventMaskOtherMouseUp.
        let monitor: *mut Object = msg_send![class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: ((1u64 << 25) | (1u64 << 26))
            handler: rail_mouse_block()];
        if !monitor.is_null() {
            let _: *mut Object = msg_send![monitor, retain];
        }
    }
}

/// Mouse Assistant chords from the 2026-10-09 capture.
/// Back = keyCode 30, Forward = keyCode 42. DOGFOOD: swap these two
/// constants if Matt's presses come out backwards.
const RAIL_MOUSE_KEY_BACK: u16 = 30;
const RAIL_MOUSE_KEY_FORWARD: u16 = 42;
const RAIL_MOUSE_DEDUPE_MS: u64 = 50;
const RAIL_MOUSE_MOD_SHIFT: u64 = 1 << 17;
const RAIL_MOUSE_MOD_CTRL: u64 = 1 << 18;
const RAIL_MOUSE_MOD_OPT: u64 = 1 << 19;
const RAIL_MOUSE_MOD_CMD: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RailMouseKeyAction {
    None,
    Back,
    Forward,
}

/// Cmd+keyDown while the stage is revealed. keyUp, repeats, a hidden
/// stage, and shift/opt/ctrl all map to none. cmd is required.
fn rail_mouse_key_action(
    event_type: u64,
    key_code: u16,
    cmd: bool,
    shift: bool,
    opt: bool,
    ctrl: bool,
    stage_revealed: bool,
    is_repeat: bool,
) -> RailMouseKeyAction {
    if event_type != 10 || !stage_revealed || is_repeat || !cmd || shift || opt || ctrl {
        return RailMouseKeyAction::None;
    }
    if key_code == RAIL_MOUSE_KEY_BACK {
        return RailMouseKeyAction::Back;
    }
    if key_code == RAIL_MOUSE_KEY_FORWARD {
        return RailMouseKeyAction::Forward;
    }
    RailMouseKeyAction::None
}

fn rail_mouse_dom_button(action: RailMouseKeyAction) -> Option<i32> {
    match action {
        RailMouseKeyAction::Back => Some(3),
        RailMouseKeyAction::Forward => Some(4),
        RailMouseKeyAction::None => None,
    }
}

/// True when this keyDown is a new physical press. `last_ms` is None
/// until the first accepted press. A second keyDown inside `window_ms`
/// is dropped. The window is the same 50 ms as `mouseButtonAt`.
fn rail_mouse_dedupe_open(now_ms: u64, last_ms: Option<u64>, window_ms: u64) -> bool {
    match last_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= window_ms,
    }
}

/// N1. Other-mouse up is event type 26. Not the live button bits.
fn rail_mouse_other_mouse_up(event_type: u64) -> bool {
    event_type == 26
}

static RAIL_MOUSE_KEYS_LIVE: AtomicBool = AtomicBool::new(false);
static RAIL_MOUSE_KEY_MON: AtomicPtr<Object> = AtomicPtr::new(std::ptr::null_mut());
static RAIL_MOUSE_KEY_LAST: AtomicU64 = AtomicU64::new(0);
static RAIL_MOUSE_KEY_ARMED: AtomicBool = AtomicBool::new(false);

fn rail_mouse_on_main() -> bool {
    unsafe {
        let on: bool = msg_send![class!(NSThread), isMainThread];
        on
    }
}

fn rail_mouse_now_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn rail_mouse_key_accept(now_ms: u64) -> bool {
    let last = if RAIL_MOUSE_KEY_ARMED.load(Ordering::Relaxed) {
        Some(RAIL_MOUSE_KEY_LAST.load(Ordering::Relaxed))
    } else {
        None
    };
    if !rail_mouse_dedupe_open(now_ms, last, RAIL_MOUSE_DEDUPE_MS) {
        return false;
    }
    RAIL_MOUSE_KEY_LAST.store(now_ms, Ordering::Relaxed);
    RAIL_MOUSE_KEY_ARMED.store(true, Ordering::Relaxed);
    true
}

fn rail_mouse_key_block() -> *const RailMouseBlock {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(RailMouseBlock {
            isa: unsafe { &RAIL_MOUSE_GLOBAL_BLOCK as *const c_void },
            flags: RAIL_MOUSE_BLOCK_GLOBAL,
            reserved: 0,
            invoke: rail_mouse_key_invoke,
            descriptor: &RAIL_MOUSE_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const RailMouseBlock
}

extern "C" fn rail_mouse_key_invoke(_block: *const RailMouseBlock, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        let ty: u64 = msg_send![event, type];
        let key: u16 = msg_send![event, keyCode];
        let flags: u64 = msg_send![event, modifierFlags];
        let repeat: bool = msg_send![event, isARepeat];
        let action = rail_mouse_key_action(
            ty,
            key,
            flags & RAIL_MOUSE_MOD_CMD != 0,
            flags & RAIL_MOUSE_MOD_SHIFT != 0,
            flags & RAIL_MOUSE_MOD_OPT != 0,
            flags & RAIL_MOUSE_MOD_CTRL != 0,
            RAIL_MOUSE_KEYS_LIVE.load(Ordering::Relaxed),
            repeat,
        );
        let Some(button) = rail_mouse_dom_button(action) else {
            return;
        };
        if !rail_mouse_key_accept(rail_mouse_now_ms()) {
            return;
        }
        let Some(app) = RAIL_MOUSE_APP.get() else {
            return;
        };
        let _ = app.emit("rail:mouse-button", button);
    }));
    // Not consumed. The menu ring ignores meta chords, and no menu
    // key equivalent binds Cmd+] or Cmd+\. A hidden stage has no monitor.
    event
}

/// Local key monitor. On while the stage is revealed, off on park and on
/// app drop. Matt, 12:57 CT: Back/Forward only while Orbitty is active.
pub fn sync_rail_mouse_keys(revealed: bool) {
    if !rail_mouse_on_main() {
        return;
    }
    if revealed {
        RAIL_MOUSE_KEYS_LIVE.store(true, Ordering::Relaxed);
        if !RAIL_MOUSE_KEY_MON.load(Ordering::Relaxed).is_null() {
            return;
        }
        let monitor: *mut Object = unsafe {
            msg_send![class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: (1u64 << 10)
                handler: rail_mouse_key_block()]
        };
        if monitor.is_null() {
            return;
        }
        unsafe {
            let _: *mut Object = msg_send![monitor, retain];
        }
        RAIL_MOUSE_KEY_MON.store(monitor, Ordering::Relaxed);
        return;
    }
    RAIL_MOUSE_KEYS_LIVE.store(false, Ordering::Relaxed);
    let monitor = RAIL_MOUSE_KEY_MON.swap(std::ptr::null_mut(), Ordering::Relaxed);
    if monitor.is_null() {
        return;
    }
    unsafe {
        let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
        let _: () = msg_send![monitor, release];
    }
}

#[cfg(test)]
mod rail_mouse_key_tests {
    use super::*;

    fn act(
        event_type: u64,
        key_code: u16,
        cmd: bool,
        shift: bool,
        opt: bool,
        ctrl: bool,
        revealed: bool,
        is_repeat: bool,
    ) -> RailMouseKeyAction {
        rail_mouse_key_action(
            event_type, key_code, cmd, shift, opt, ctrl, revealed, is_repeat,
        )
    }

    #[test]
    fn mouse_assistant_cmd_key_maps_back_and_forward() {
        assert_eq!(act(10, 30, true, false, false, false, true, false), RailMouseKeyAction::Back);
        assert_eq!(act(10, 42, true, false, false, false, true, false), RailMouseKeyAction::Forward);
        assert_eq!(rail_mouse_dom_button(RailMouseKeyAction::Back), Some(3));
        assert_eq!(rail_mouse_dom_button(RailMouseKeyAction::Forward), Some(4));
        assert_eq!(rail_mouse_dom_button(RailMouseKeyAction::None), None);
        assert_eq!(RAIL_MOUSE_KEY_BACK, 30);
        assert_eq!(RAIL_MOUSE_KEY_FORWARD, 42);
    }

    #[test]
    fn mouse_assistant_chord_rejects_repeat_keyup_mods_and_hidden() {
        assert_eq!(act(10, 30, true, false, false, false, true, true), RailMouseKeyAction::None);
        assert_eq!(act(11, 30, true, false, false, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(11, 42, false, false, false, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 30, false, false, false, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 30, true, true, false, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 42, true, false, true, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 42, true, false, false, true, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 30, true, false, false, false, false, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 53, true, false, false, false, true, false), RailMouseKeyAction::None);
        assert_eq!(act(10, 33, true, false, false, false, true, false), RailMouseKeyAction::None);
    }

    #[test]
    fn doubled_keydown_inside_50ms_is_dropped() {
        assert!(rail_mouse_dedupe_open(0, None, RAIL_MOUSE_DEDUPE_MS));
        assert!(!rail_mouse_dedupe_open(20, Some(0), RAIL_MOUSE_DEDUPE_MS));
        assert!(!rail_mouse_dedupe_open(1, Some(0), RAIL_MOUSE_DEDUPE_MS));
        assert!(!rail_mouse_dedupe_open(49, Some(0), RAIL_MOUSE_DEDUPE_MS));
        assert!(rail_mouse_dedupe_open(50, Some(0), RAIL_MOUSE_DEDUPE_MS));
        assert!(rail_mouse_dedupe_open(240, Some(0), RAIL_MOUSE_DEDUPE_MS));
        assert_eq!(RAIL_MOUSE_DEDUPE_MS, 50);
    }

    #[test]
    fn other_mouse_up_keys_on_event_type_26() {
        assert!(rail_mouse_other_mouse_up(26));
        assert!(!rail_mouse_other_mouse_up(25));
        assert!(!rail_mouse_other_mouse_up(10));
        let src = include_str!("windows_mac.rs");
        let start = src.find("fn rail_mouse_invoke").expect("invoke");
        let rest = &src[start..];
        let end = rest.find("\npub fn install_rail_mouse_buttons").expect("install");
        let body = &rest[..end];
        assert!(body.contains("rail_mouse_other_mouse_up"));
        assert!(!body.contains("pressedMouseButtons"));
    }

    #[test]
    fn key_monitor_is_local_and_does_not_swallow() {
        let src = include_str!("windows_mac.rs");
        let start = src.find("fn rail_mouse_key_invoke").expect("key invoke");
        let rest = &src[start..];
        let end = rest.find("\npub fn sync_rail_mouse_keys").expect("sync");
        let body = &rest[..end];
        assert!(body.contains("rail_mouse_key_action"));
        assert!(body.contains("rail_mouse_key_accept"));
        assert!(body.contains("\n    event\n"));
        assert!(!body.contains("null_mut()"));
        let accept_at = src.find("fn rail_mouse_key_accept").expect("accept");
        let accept = &src[accept_at..accept_at + 500];
        assert!(accept.contains("rail_mouse_dedupe_open"));
        assert!(accept.contains("RAIL_MOUSE_DEDUPE_MS"));
        let sync_at = src.find("pub fn sync_rail_mouse_keys").expect("sync fn");
        let sync = &src[sync_at..sync_at + 900];
        assert!(sync.contains("addLocalMonitorForEventsMatchingMask"));
        assert!(sync.contains("1u64 << 10"));
        assert!(!sync.contains("addGlobalMonitor"));
        assert!(!sync.contains("CGEventTapCreate"));
    }
}

#[cfg(test)]
#[path = "ax_safe_tests.rs"]
mod ax_safe_tests;
