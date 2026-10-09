//! AX catalog walk used by list_windows. Private to the macOS corral.

use super::super::WindowInfo;
use super::{
    ax_bool, ax_copy, ax_names, ax_size, ax_string, ax_window_id, AXUIElementCreateApplication,
    AXUIElementCreateSystemWide, AXUIElementGetPid, AXUIElementRef, AX_SUCCESS, CFArrayGetCount,
    CFArrayGetValueAtIndex, CFGetTypeID, CFRelease, CFStringGetTypeID, CFURLGetString,
    CFURLGetTypeID, CGSize, NSPoint, NSRect,
};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_char, CStr};
use std::sync::OnceLock;

const NS_ACTIVATION_POLICY_REGULAR: i64 = 0;
const NS_COMPOSITING_OPERATION_SOURCE_OVER: usize = 2;
const NS_BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const ICON_PX: usize = 64;
/// The window's AXDocument as a real on-disk path.
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

// AppKit via objc runtime

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

// app icons

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

fn icon_flight() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static FLIGHT: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    FLIGHT.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn cached_icon(app: *mut Object, cache_key: &str) -> Option<String> {
    if let Some(hit) = icon_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(cache_key)
    {
        return Some(hit.clone());
    }
    // WK custom-scheme IPC (`list_windows`) runs on the main thread.
    // `NSRunningApplication.icon` can block in getattrlist. A miss returns
    // None and warms the cache off this thread. The next poll paints it.
    let is_main: bool = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main {
        warm_icon(cache_key.to_string());
        return None;
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

/// One background fill per bundle id (or `pid:` key). The job looks the app
/// up itself so the main-thread object pointer never crosses threads.
fn warm_icon(cache_key: String) {
    {
        let cache = icon_cache().lock().unwrap_or_else(|e| e.into_inner());
        if cache.contains_key(&cache_key) {
            return;
        }
        let mut flight = icon_flight().lock().unwrap_or_else(|e| e.into_inner());
        if !flight.insert(cache_key.clone()) {
            return;
        }
    }
    std::thread::spawn(move || {
        let fetched = objc::rc::autoreleasepool(|| unsafe { icon_png_for_key(&cache_key) });
        if let Some(png) = fetched {
            icon_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(cache_key.clone(), png);
        }
        icon_flight()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&cache_key);
    });
}

unsafe fn icon_png_for_key(cache_key: &str) -> Option<String> {
    let want_pid = cache_key
        .strip_prefix("pid:")
        .and_then(|s| s.parse::<i32>().ok());
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let apps: *mut Object = msg_send![workspace, runningApplications];
    if apps.is_null() {
        return None;
    }
    let count: usize = msg_send![apps, count];
    for i in 0..count {
        let app: *mut Object = msg_send![apps, objectAtIndex: i];
        if app.is_null() {
            continue;
        }
        let hit = if let Some(pid) = want_pid {
            let got: i32 = msg_send![app, processIdentifier];
            got == pid
        } else {
            let bundle: *mut Object = msg_send![app, bundleIdentifier];
            ns_to_string(bundle).as_deref() == Some(cache_key)
        };
        if hit {
            return app_icon_png_base64(app);
        }
    }
    None
}

/// Draw the app's icon into a small offscreen bitmap and PNG-encode it.
/// Runs off the main thread. A 32 px sample was 2930 bytes, so 64 px stays small.
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

/// Regular, non-terminated apps, excluding ourselves (port of WindowCatalog's filter).
unsafe fn running_apps() -> Vec<RunningApp> {
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
        // Never list our own windows (current Orbitty id).
        if matches!(bundle_id.as_deref(), Some("space.orbitty.mac")) {
            continue;
        }
        let cache_key = bundle_id.clone().unwrap_or_else(|| format!("pid:{pid}"));
        let icon_base64 = cached_icon(app, &cache_key);
        out.push(RunningApp {
            pid,
            name: ns_to_string(name).unwrap_or_else(|| "App".into()),
            bundle_id,
            icon_base64,
        });
    }
    out
}
// enumeration (port of WindowCatalog)

pub(crate) unsafe fn enumerate() -> Vec<WindowInfo> {
    let mut result = Vec::new();
    for app in running_apps() {
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
                if let Some(size) = ax_size(ax_win, ax_names().size) {
                    if size.width < 40.0 || size.height < 40.0 {
                        continue;
                    }
                }
                let title = ax_string(ax_win, ax_names().title).unwrap_or_default();
                let minimized = ax_bool(ax_win, ax_names().minimized).unwrap_or(false);
                // Sleeper wake stamp: the document this window has open, when
                // the app exposes one and the file still exists.
                let document_path = ax_document_path(ax_win);
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

pub(crate) unsafe fn mark_frontmost(list: &mut [WindowInfo]) {
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
