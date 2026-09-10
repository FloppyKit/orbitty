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
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

type CFRef = *const c_void;
type AXUIElementRef = CFRef;
type AXError = i32;

const AX_SUCCESS: AXError = 0;
const K_AX_VALUE_TYPE_CG_POINT: i32 = 1;
const K_AX_VALUE_TYPE_CG_SIZE: i32 = 2;
const K_CF_NUMBER_S_INT64_TYPE: i32 = 4;
const K_SEC_CS_SIGNING_INFORMATION: u32 = 1 << 1;
const K_SEC_CODE_SIGNATURE_ADHOC: i64 = 2;
const NS_ACTIVATION_POLICY_REGULAR: i64 = 0;
const NS_COMPOSITING_OPERATION_SOURCE_OVER: usize = 2;
const NS_BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const ICON_PX: usize = 32;

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

// ---------- AX helpers (port of AXHelpers.swift) ----------

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

/// GO-ORBITTY-SLEEPERS: the window's AXDocument as a real on-disk path.
/// AXDocument arrives as a CFURL or a CFString URL; only `file://` URLs
/// whose target exists are stamped — never contents, never a guessed path.
unsafe fn ax_document_path(element: AXUIElementRef) -> Option<String> {
    let raw = ax_copy(element, ax_names().document)?;
    let ty = CFGetTypeID(raw);
    let url_string = if ty == CFStringGetTypeID() {
        Some(CFString::wrap_under_get_rule(raw as _).to_string())
    } else if ty == CFURLGetTypeID() {
        let s = CFURLGetString(raw);
        if s.is_null() {
            None
        } else {
            Some(CFString::wrap_under_get_rule(s as _).to_string())
        }
    } else {
        None
    };
    CFRelease(raw);
    file_url_to_path(&url_string?)
}

/// `file:///Users/…` (or `file://localhost/…`) → percent-decoded path that
/// exists right now. Anything else (http, untitled, missing) is None.
fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path_part = if let Some(p) = rest.strip_prefix('/') {
        p
    } else {
        // host form: only localhost is this machine
        let (host, p) = rest.split_once('/')?;
        if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            return None;
        }
        p
    };
    let decoded = percent_decode(path_part);
    let path = format!("/{decoded}");
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        None
    }
}

/// %XX decoding for file URLs (`+` stays literal — it is not a space in a
/// path). Invalid UTF-8 falls back to lossy rather than dropping the stamp.
fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------- AppKit via objc runtime ----------

unsafe fn ns_to_string(obj: *mut Object) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let utf8: *const c_char = msg_send![obj, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

struct RunningApp {
    pid: i32,
    name: String,
    bundle_id: Option<String>,
    icon_base64: Option<String>,
}

// ---------- app icons ----------

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64_ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64_ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64_ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Toll-free-bridged NSString, intentionally leaked (used as a constant).
unsafe fn ns_string(s: &str) -> *mut Object {
    let cf = CFString::new(s);
    let raw = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    raw
}

/// Icons are fetched once per app per session; only successes are cached so a
/// late-loading icon still appears on a later poll.
fn icon_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn cached_icon(app: *mut Object, cache_key: &str) -> Option<String> {
    if let Some(hit) = icon_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(cache_key)
    {
        return Some(hit.clone());
    }
    let fetched = unsafe { app_icon_png_base64(app) };
    if let Some(png) = &fetched {
        icon_cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(cache_key.to_string(), png.clone());
    }
    fetched
}

/// Draw the app's icon into a small offscreen bitmap and PNG-encode it, so the
/// payload stays ~2–4 KB regardless of the source icon's resolution.
unsafe fn app_icon_png_base64(app: *mut Object) -> Option<String> {
    let icon: *mut Object = msg_send![app, icon];
    if icon.is_null() {
        return None;
    }
    let rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
    let rep: *mut Object = msg_send![rep,
        initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
        pixelsWide: ICON_PX
        pixelsHigh: ICON_PX
        bitsPerSample: 8usize
        samplesPerPixel: 4usize
        hasAlpha: true
        isPlanar: false
        colorSpaceName: ns_string("NSCalibratedRGBColorSpace")
        bytesPerRow: 0usize
        bitsPerPixel: 0usize
    ];
    if rep.is_null() {
        return None;
    }
    let ctx: *mut Object =
        msg_send![class!(NSGraphicsContext), graphicsContextWithBitmapImageRep: rep];
    if ctx.is_null() {
        let _: () = msg_send![rep, release];
        return None;
    }
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: ctx];
    let frame = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: ICON_PX as f64,
            height: ICON_PX as f64,
        },
    };
    // NSZeroRect = the whole source image.
    let whole = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 0.0,
            height: 0.0,
        },
    };
    let _: () = msg_send![icon,
        drawInRect: frame
        fromRect: whole
        operation: NS_COMPOSITING_OPERATION_SOURCE_OVER
        fraction: 1.0f64
    ];
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
    let png: *mut Object = msg_send![rep,
        representationUsingType: NS_BITMAP_IMAGE_FILE_TYPE_PNG
        properties: std::ptr::null_mut::<Object>()
    ];
    let out = if png.is_null() {
        None
    } else {
        let bytes: *const u8 = msg_send![png, bytes];
        let len: usize = msg_send![png, length];
        if bytes.is_null() || len == 0 {
            None
        } else {
            Some(base64_encode(std::slice::from_raw_parts(bytes, len)))
        }
    };
    let _: () = msg_send![rep, release];
    out
}

/// What an enumeration pass collects. The full inspector list wants all
/// of it; the yard's ~1s poll takes titles + ids + frames ONLY — a cold
/// icon cache PNG-encodes every running app's icon on the caller's
/// thread (the 10–20s Drop stall), and the yard never shows an icon.
#[derive(Clone, Copy)]
struct EnumOpts {
    icons: bool,
    documents: bool,
    frontmost: bool,
}

/// Regular, non-terminated apps, excluding ourselves (port of WindowCatalog's filter).
unsafe fn running_apps(icons: bool) -> Vec<RunningApp> {
    let self_pid = std::process::id() as i32;
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let apps: *mut Object = msg_send![workspace, runningApplications];
    let count: usize = msg_send![apps, count];
    let mut out = Vec::new();
    for i in 0..count {
        let app: *mut Object = msg_send![apps, objectAtIndex: i];
        if app.is_null() {
            continue;
        }
        let policy: i64 = msg_send![app, activationPolicy];
        let terminated: bool = msg_send![app, isTerminated];
        let pid: i32 = msg_send![app, processIdentifier];
        if policy != NS_ACTIVATION_POLICY_REGULAR || terminated || pid == self_pid || pid <= 0 {
            continue;
        }
        let name: *mut Object = msg_send![app, localizedName];
        let bundle: *mut Object = msg_send![app, bundleIdentifier];
        let bundle_id = ns_to_string(bundle);
        // Never list our own windows — current Orbitty id and the retired
        // LiveSprite id (a leftover LiveSprite.app may still be running).
        if matches!(
            bundle_id.as_deref(),
            Some("space.orbitty.mac") | Some("app.livesprite.mac")
        ) {
            continue;
        }
        // Icons only when the caller asked: cached_icon PNG-encodes on a
        // cold cache and must never run on the yard's poll path.
        let icon_base64 = if icons {
            let cache_key = bundle_id.clone().unwrap_or_else(|| format!("pid:{pid}"));
            cached_icon(app, &cache_key)
        } else {
            None
        };
        out.push(RunningApp {
            pid,
            name: ns_to_string(name).unwrap_or_else(|| "App".into()),
            bundle_id,
            icon_base64,
        });
    }
    out
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

// ---------- enumeration (port of WindowCatalog) ----------

unsafe fn enumerate(opts: &EnumOpts) -> Vec<WindowInfo> {
    let mut result = Vec::new();
    for app in running_apps(opts.icons) {
        let ax_app = AXUIElementCreateApplication(app.pid);
        if ax_app.is_null() {
            continue;
        }
        if let Some(windows) = ax_copy(ax_app, ax_names().windows) {
            let count = CFArrayGetCount(windows);
            let mut fallback_index = 0u32;
            for i in 0..count {
                let ax_win = CFArrayGetValueAtIndex(windows, i);
                if ax_win.is_null() {
                    continue;
                }
                // Real AX windows only; drop tiny utility elements when size is readable.
                if ax_string(ax_win, ax_names().role).as_deref() != Some("AXWindow") {
                    continue;
                }
                let size = ax_size(ax_win, ax_names().size);
                if let Some(s) = &size {
                    if s.width < 40.0 || s.height < 40.0 {
                        continue;
                    }
                }
                // The yard rains each cat on the display its window is on;
                // either AX read may legitimately fail (minimized, hiccup).
                let position = ax_point(ax_win, ax_names().position);
                let frame = match (position, size) {
                    (Some(p), Some(s)) => Some(super::WindowFrame {
                        x: p.x,
                        y: p.y,
                        w: s.width,
                        h: s.height,
                    }),
                    _ => None,
                };
                let title = ax_string(ax_win, ax_names().title).unwrap_or_default();
                let minimized = ax_bool(ax_win, ax_names().minimized).unwrap_or(false);
                // Sleeper wake stamp: the document this window has open, when
                // the app exposes one and the file still exists. Skipped on
                // the light path — one AX read + stat the yard never uses.
                let document_path = if opts.documents {
                    ax_document_path(ax_win)
                } else {
                    None
                };
                let id = match ax_window_id(ax_win) {
                    Some(cg_id) => format!("{}:{}", app.pid, cg_id),
                    None => {
                        fallback_index += 1;
                        format!("{}:t:{}#{}", app.pid, title, fallback_index)
                    }
                };
                result.push(WindowInfo {
                    id,
                    pid: app.pid,
                    app_name: app.name.clone(),
                    title,
                    bundle_id: app.bundle_id.clone(),
                    stable_key: String::new(), // stamped below, post-enumerate
                    icon_base64: app.icon_base64.clone(),
                    is_frontmost: false,
                    is_minimized: minimized,
                    document_path,
                    frame,
                });
            }
            CFRelease(windows);
        }
        CFRelease(ax_app);
    }
    stamp_stable_keys(&mut result);
    result
}

/// `{bundleId or appName}|{title}|{n}` — n is 1-based in first-seen order
/// among windows sharing the same bundle+title in this enumerate.
fn stamp_stable_keys(list: &mut [WindowInfo]) {
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for w in list.iter_mut() {
        let base = format!(
            "{}|{}",
            w.bundle_id.as_deref().unwrap_or(&w.app_name),
            w.title
        );
        let n = counts.entry(base.clone()).or_insert(0);
        *n += 1;
        w.stable_key = format!("{base}|{n}");
    }
}

unsafe fn mark_frontmost(list: &mut [WindowInfo]) {
    let system = AXUIElementCreateSystemWide();
    if system.is_null() {
        return;
    }
    if let Some(focused_app) = ax_copy(system, ax_names().focused_app) {
        let mut pid: i32 = 0;
        if AXUIElementGetPid(focused_app, &mut pid) == AX_SUCCESS {
            let mut matched = false;
            if let Some(focused_win) = ax_copy(focused_app, ax_names().focused_window) {
                if let Some(cg_id) = ax_window_id(focused_win) {
                    let id = format!("{pid}:{cg_id}");
                    if let Some(w) = list.iter_mut().find(|w| w.id == id) {
                        w.is_frontmost = true;
                        matched = true;
                    }
                }
                CFRelease(focused_win);
            }
            // Focused window id unavailable — mark the app's first open window.
            if !matched {
                if let Some(w) = list.iter_mut().find(|w| w.pid == pid && !w.is_minimized) {
                    w.is_frontmost = true;
                }
            }
        }
        CFRelease(focused_app);
    }
    CFRelease(system);
}

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    list_with(EnumOpts {
        icons: true,
        documents: true,
        frontmost: true,
    })
}

/// GO-ORBITTY-HERDING-CATS leftover (rain now): the yard poll's list.
/// Same AX window enumeration as `list_windows` — titles + ids + frames,
/// same Orbitty/LiveSprite drop — but NEVER icon PNG-encoding, sleeper
/// document stamps, or frontmost marking. The warn pane / inspector keep
/// the full `list_windows`.
pub fn list_windows_light() -> Result<Vec<WindowInfo>, String> {
    list_with(EnumOpts {
        icons: false,
        documents: false,
        frontmost: false,
    })
}

fn list_with(opts: EnumOpts) -> Result<Vec<WindowInfo>, String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("accessibility not granted".into());
    }
    unsafe {
        let mut windows = enumerate(&opts);
        if opts.frontmost {
            mark_frontmost(&mut windows);
        }
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

// ---------- focus (port of WindowFocuser) ----------

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

        let ax_app = AXUIElementCreateApplication(pid);
        if ax_app.is_null() {
            return Err("could not attach to app".into());
        }
        let result = focus_in_app(ax_app, rest);
        CFRelease(ax_app);
        result
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
        let ax_app = AXUIElementCreateApplication(pid);
        if ax_app.is_null() {
            return Err("could not attach to app".into());
        }
        let target = find_window(ax_app, rest);
        if target.is_null() {
            CFRelease(ax_app);
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
        CFRelease(ax_app);
        result
    }
}

/// Rail Stow: ordinary AX minimize — the window sits in the Dock like any
/// other minimized window. LiveSprite never parks windows offscreen.
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
        let ax_app = AXUIElementCreateApplication(pid);
        if ax_app.is_null() {
            return Err("could not attach to app".into());
        }
        let target = find_window(ax_app, rest);
        if target.is_null() {
            CFRelease(ax_app);
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
        CFRelease(ax_app);
        result
    }
}

// ---------- locate ring (GO-ORBITTY-RAIL-ROW-MENU) ----------

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
        let ax_app = AXUIElementCreateApplication(pid);
        if ax_app.is_null() {
            return Err("could not attach to app".into());
        }
        let target = find_window(ax_app, rest);
        if target.is_null() {
            CFRelease(ax_app);
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
        CFRelease(ax_app);
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

// ---------- permission gate (port of PermissionGate) ----------

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
