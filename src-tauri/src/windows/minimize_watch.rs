//! Hear a shrink as it happens.
//!
//! One accessibility observer on the front app, rebound when that app
//! changes. The existing frontmost tracker does the rebind. This is not
//! a second window list and not a faster poll.

use core_foundation::base::TCFType;
use std::ffi::c_void;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

type AXObserverRef = *mut c_void;
type AXUIElementRef = *const c_void;
type CFRunLoopSourceRef = *mut c_void;

const AX_SUCCESS: i32 = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXObserverCreate(
        pid: i32,
        callback: extern "C" fn(AXObserverRef, AXUIElementRef, *const c_void, *mut c_void),
        out: *mut AXObserverRef,
    ) -> i32;
    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: *const c_void,
        refcon: *mut c_void,
    ) -> i32;
    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    fn _AXUIElementGetWindow(element: AXUIElementRef, id: *mut u32) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: CFRunLoopSourceRef, mode: *const c_void);
    fn CFRunLoopRemoveSource(rl: *mut c_void, source: CFRunLoopSourceRef, mode: *const c_void);
    static kCFRunLoopDefaultMode: *const c_void;
}

struct Bound {
    pid: i32,
    observer: AXObserverRef,
    app: AXUIElementRef,
    source: CFRunLoopSourceRef,
}

// Touched on the main thread only. The pointers are CoreFoundation refs.
unsafe impl Send for Bound {}

static APP: Mutex<Option<AppHandle>> = Mutex::new(None);
static BOUND: Mutex<Option<Bound>> = Mutex::new(None);

/// Remember the app so a shrink can tell the rail. Call once at startup.
pub fn start(app: AppHandle) {
    if let Ok(mut slot) = APP.lock() {
        *slot = Some(app);
    }
}

/// Watch `pid` for shrinks. Same pid is a no-op. Own process is ignored.
/// Main thread only.
pub fn bind_front(pid: i32) {
    if pid <= 0 || pid == std::process::id() as i32 {
        return;
    }
    let Ok(mut slot) = BOUND.lock() else {
        return;
    };
    if slot.as_ref().is_some_and(|b| b.pid == pid) {
        return;
    }
    drop_bound(&mut slot);
    unsafe {
        let mut observer: AXObserverRef = std::ptr::null_mut();
        if AXObserverCreate(pid, on_minimized, &mut observer) != AX_SUCCESS || observer.is_null() {
            return;
        }
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            CFRelease(observer as *const c_void);
            return;
        }
        let note = core_foundation::string::CFString::new("AXWindowMiniaturized");
        if AXObserverAddNotification(observer, app, note.as_concrete_TypeRef() as *const c_void, std::ptr::null_mut())
            != AX_SUCCESS
        {
            CFRelease(app);
            CFRelease(observer as *const c_void);
            return;
        }
        let source = AXObserverGetRunLoopSource(observer);
        if source.is_null() {
            CFRelease(app);
            CFRelease(observer as *const c_void);
            return;
        }
        CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopDefaultMode);
        *slot = Some(Bound {
            pid,
            observer,
            app,
            source,
        });
    }
}

fn drop_bound(slot: &mut Option<Bound>) {
    let Some(old) = slot.take() else {
        return;
    };
    unsafe {
        CFRunLoopRemoveSource(CFRunLoopGetMain(), old.source, kCFRunLoopDefaultMode);
        CFRelease(old.app);
        CFRelease(old.observer as *const c_void);
    }
}

extern "C" fn on_minimized(
    _observer: AXObserverRef,
    element: AXUIElementRef,
    _notification: *const c_void,
    _refcon: *mut c_void,
) {
    if element.is_null() {
        return;
    }
    let mut pid: i32 = 0;
    let mut cg: u32 = 0;
    unsafe {
        if AXUIElementGetPid(element, &mut pid) != AX_SUCCESS || pid <= 0 {
            return;
        }
        if _AXUIElementGetWindow(element, &mut cg) != AX_SUCCESS || cg == 0 {
            return;
        }
    }
    let id = format!("{pid}:{cg}");
    let Ok(slot) = APP.lock() else {
        return;
    };
    let Some(app) = slot.as_ref() else {
        return;
    };
    let _ = app.emit_to(
        "main",
        "orbitty:window-minimized",
        serde_json::json!({ "id": id }),
    );
}
