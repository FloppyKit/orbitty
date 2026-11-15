//! Esc / empty-glass monitors for the peek pills and preview.
//!
//! Installed only while one of those panes is shown. One event dismisses
//! one step: an open preview closes and the pills return; the next Esc or
//! click-away hides the pills. A walker hit is not click-away — the
//! Inspect matrix in mouse_up stays. A click on the pills or the preview
//! is not click-away.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::Manager;

use super::NSPoint;

/// NSEventMask: keyDown (1<<10) | leftMouseDown (1<<1).
const PEEK_EVENT_MASK: u64 = 1026;

static PEEK_MONITOR: AtomicUsize = AtomicUsize::new(0);
static PEEK_LOCAL_MONITOR: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct PeekBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const PeekBlock, *mut Object),
    descriptor: *const PeekBlockDescriptor,
}

#[repr(C)]
struct PeekBlockDescriptor {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static NS_CONCRETE_GLOBAL_BLOCK: c_void;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static PEEK_BLOCK_DESCRIPTOR: PeekBlockDescriptor = PeekBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<PeekBlock>() as std::os::raw::c_ulong,
};

fn peek_block() -> *const PeekBlock {
    static BLOCK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(PeekBlock {
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: peek_invoke,
            descriptor: &PEEK_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const PeekBlock
}

#[repr(C)]
struct PeekLocalBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const PeekLocalBlock, *mut Object) -> *mut Object,
    descriptor: *const PeekBlockDescriptor,
}

static PEEK_LOCAL_BLOCK_DESCRIPTOR: PeekBlockDescriptor = PeekBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<PeekLocalBlock>() as std::os::raw::c_ulong,
};

fn peek_local_block() -> *const PeekLocalBlock {
    static BLOCK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(PeekLocalBlock {
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: peek_local_invoke,
            descriptor: &PEEK_LOCAL_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const PeekLocalBlock
}

fn hop_dismiss() {
    super::audio::defer_main(|| {
        if let Some(app) = super::app_handle() {
            let _ = crate::panes::peek_dismiss_one_step(app);
        }
    });
}

fn pane_ptr(label: &str) -> usize {
    let Some(app) = super::app_handle() else {
        return 0;
    };
    app.get_webview_window(label)
        .and_then(|w| w.ns_window().ok())
        .map(|raw| raw as usize)
        .unwrap_or(0)
}

/// NSEventType: 1 leftDown, 10 keyDown. kVK_Escape = 53.
unsafe fn local_should_dismiss(event: *mut Object) -> bool {
    let ty: u64 = msg_send![event, type];
    if ty == 10 {
        let key: u16 = msg_send![event, keyCode];
        return key == 53;
    }
    if ty != 1 {
        return false;
    }
    let win: *mut Object = msg_send![event, window];
    if win.is_null() {
        return true;
    }
    let win_ptr = win as usize;
    let on_pills = pane_ptr("peek-capsule") == win_ptr && win_ptr != 0;
    let on_preview = pane_ptr("peek-preview") == win_ptr && win_ptr != 0;
    // Copy the view pointer, then hit-test. Do not call AppKit while the
    // habitat lock is held.
    let view_ptr = {
        let h = super::lock();
        h.planets
            .values()
            .find(|p| p.panel == win_ptr)
            .map(|p| p.view)
    };
    let on_planet = view_ptr.is_some();
    let on_walker = if let Some(view_ptr) = view_ptr {
        let view = view_ptr as *mut Object;
        if view.is_null() {
            false
        } else {
            let loc: NSPoint = msg_send![event, locationInWindow];
            let local: NSPoint =
                msg_send![view, convertPoint: loc fromView: std::ptr::null::<Object>()];
            let h = super::lock();
            // A walker hit is not click-away.
            h.planets
                .values()
                .find(|p| p.panel == win_ptr)
                .is_some_and(|planet| {
                    matches!(super::input::hit_being(planet, local), Some((_, true)))
                })
        }
    } else {
        false
    };
    if !on_planet && !on_pills && !on_preview {
        // Rail and other Orbitty windows are not empty glass.
        return false;
    }
    crate::panes::peek_click_is_away(on_pills, on_preview, on_walker)
}

extern "C" fn peek_invoke(_block: *const PeekBlock, event: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        // Global monitor: other apps. Esc or a click outside Orbitty.
        let ty: u64 = msg_send![event, type];
        let away = match ty {
            10 => {
                let key: u16 = msg_send![event, keyCode];
                key == 53
            }
            1 => true,
            _ => false,
        };
        if away {
            hop_dismiss();
        }
    }));
}

extern "C" fn peek_local_invoke(_block: *const PeekLocalBlock, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        if local_should_dismiss(event) {
            hop_dismiss();
        }
    }));
    event
}

fn install_monitors() {
    unsafe {
        if PEEK_MONITOR.load(Ordering::SeqCst) == 0 {
            let monitor: *mut Object = msg_send![class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: PEEK_EVENT_MASK
                handler: peek_block()];
            if !monitor.is_null() {
                let _: *mut Object = msg_send![monitor, retain];
                PEEK_MONITOR.store(monitor as usize, Ordering::SeqCst);
            }
        }
        if PEEK_LOCAL_MONITOR.load(Ordering::SeqCst) == 0 {
            let monitor: *mut Object = msg_send![class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: PEEK_EVENT_MASK
                handler: peek_local_block()];
            if !monitor.is_null() {
                let _: *mut Object = msg_send![monitor, retain];
                PEEK_LOCAL_MONITOR.store(monitor as usize, Ordering::SeqCst);
            }
        }
    }
}

fn remove_monitors() {
    let global = PEEK_MONITOR.swap(0, Ordering::SeqCst);
    if global != 0 {
        unsafe {
            let monitor = global as *mut Object;
            let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
            let _: () = msg_send![monitor, release];
        }
    }
    let local = PEEK_LOCAL_MONITOR.swap(0, Ordering::SeqCst);
    if local != 0 {
        unsafe {
            let monitor = local as *mut Object;
            let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
            let _: () = msg_send![monitor, release];
        }
    }
}

pub(crate) fn set_installed(on: bool) {
    if on {
        install_monitors();
    } else {
        remove_monitors();
    }
}
