//! Esc scoop and the public yard door. Calls lock and yard in the parent.

use super::art::load_cat_art;
use super::{
    app_handle, app_handle_cell, lock, panic_msg, spawn_worker, sync_panels, Yard, HOP_PENDING,
};
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use tauri::AppHandle;

// Esc monitor (scoop all)

/// Capture-less global block for the NSEvent monitor — the same ABI
/// trick as the boombox picker's completion handler.
#[repr(C)]
struct EscBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const EscBlock, *mut Object),
    descriptor: *const EscBlockDescriptor,
}

#[repr(C)]
struct EscBlockDescriptor {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static NS_CONCRETE_GLOBAL_BLOCK: c_void;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static ESC_BLOCK_DESCRIPTOR: EscBlockDescriptor = EscBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<EscBlock>() as std::os::raw::c_ulong,
};

fn esc_block() -> *const EscBlock {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(EscBlock {
            // SAFETY: referencing the libSystem global-block class.
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: esc_invoke,
            descriptor: &ESC_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const EscBlock
}

extern "C" fn esc_invoke(_block: *const EscBlock, event: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        // kVK_Escape — while the clowder is out, Esc anywhere scoops all
        // (the overlay is non-activating, so no focusable surface of ours
        // could ever see the key). The event still reaches the front app:
        // a global monitor observes, never eats. stop() hops to main when
        // the monitor handler isn't already there.
        let key: u16 = msg_send![event, keyCode];
        if key == 53 {
            let _ = stop();
        }
    }));
}

/// Local-monitor twin of EscBlock: the handler MUST return the event
/// (returning nil would eat the key). Same layout, pointer return.
#[repr(C)]
struct EscLocalBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const EscLocalBlock, *mut Object) -> *mut Object,
    descriptor: *const EscBlockDescriptor,
}

fn esc_local_block() -> *const EscLocalBlock {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(EscLocalBlock {
            // SAFETY: referencing the libSystem global-block class.
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: esc_local_invoke,
            descriptor: &ESC_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const EscLocalBlock
}

extern "C" fn esc_local_invoke(_block: *const EscLocalBlock, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        // kVK_Escape while ORBITTY is key — the global monitor never sees
        // our own app's keys, so without this Esc did nothing after Drop
        // (the warn pane hides and Orbitty stays key). Scoop, then hand
        // the event back: this is the product Esc, but typing in other
        // fields is never eaten.
        let key: u16 = msg_send![event, keyCode];
        if key == 53 {
            let _ = stop();
        }
    }));
    event
}

unsafe fn install_esc_monitor(y: &mut Yard) {
    // NSEventMaskKeyDown. The global monitor needs the same AX trust the
    // whole app already requires; a null return just leaves ⌘⇧Y as the
    // scoop. Each monitor guards itself so a retry after a null never
    // double-installs the other.
    if y.esc_monitor == 0 {
        let monitor: *mut Object = msg_send![class!(NSEvent),
            addGlobalMonitorForEventsMatchingMask: 1024u64
            handler: esc_block()];
        if !monitor.is_null() {
            let _: *mut Object = msg_send![monitor, retain];
            y.esc_monitor = monitor as usize;
        }
    }
    if y.esc_local_monitor == 0 {
        let monitor: *mut Object = msg_send![class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: 1024u64
            handler: esc_local_block()];
        if !monitor.is_null() {
            let _: *mut Object = msg_send![monitor, retain];
            y.esc_local_monitor = monitor as usize;
        }
    }
}

unsafe fn remove_esc_monitor(y: &mut Yard) {
    if y.esc_monitor != 0 {
        let monitor = y.esc_monitor as *mut Object;
        let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
        let _: () = msg_send![monitor, release];
        y.esc_monitor = 0;
    }
    if y.esc_local_monitor != 0 {
        let monitor = y.esc_local_monitor as *mut Object;
        let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
        let _: () = msg_send![monitor, release];
        y.esc_local_monitor = 0;
    }
}

// public door

pub fn active() -> bool {
    lock().on
}

/// Drop the cats. No-op on an already-on yard (never double-spawn).
pub fn start(app: AppHandle) -> Result<(), String> {
    if active() {
        return Ok(());
    }
    let cell = app_handle_cell();
    let _ = cell.set(app.clone());
    crate::main_thread_sync(move || {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe { start_on_main() }))
            .map_err(|p| eprintln!("yard: start panicked: {}", panic_msg(&*p)));
    });
    // Honesty: start_on_main no-ops when no cat art decodes or the view
    // class fails — Drop must not pretend the yard is on.
    if active() {
        Ok(())
    } else {
        Err("yard stayed off (no cat art decoded, or view class failed)".to_string())
    }
}

/// Main thread. Builds the panels, decodes the cat colorways, arms the
/// meow clock + Esc monitor, and wakes the worker. One lock epoch for the
/// whole bring-up so a stop() mid-start can't leave panels orphaned on
/// an already-off yard.
unsafe fn start_on_main() {
    let mut y = lock();
    if y.on {
        return;
    }
    let Some(app) = app_handle().cloned() else {
        return;
    };
    let packs = load_cat_art(&app);
    if packs.is_empty() {
        // Never rain invisible cats.
        eprintln!("yard: no cat pack frames decoded — staying off");
        return;
    }
    y.on = true;
    y.poll_done = false;
    y.scooped.clear();
    y.spawn_count = 0;
    y.last_hop = None;
    y.hop_count = 0;
    y.press = None;
    y.caption = None;
    HOP_PENDING.store(false, Ordering::SeqCst);
    y.packs = packs;
    y.meow = super::meow::Meow::armed();
    sync_panels(&mut y);
    install_esc_monitor(&mut y);
    drop(y);
    flush_yard_views();
    spawn_worker();
}

/// displayIfNeeded with the yard lock released so drawRect can take it.
/// Panels are born at the display frame — do not setFrame here (that was
/// the 10×10 / deadlock path). Do not orderFrontRegardless (that restack
/// hid the companion dock).
pub(crate) unsafe fn flush_yard_views() {
    let snaps: Vec<usize> = lock().panels.iter().map(|p| p.view).collect();
    for view in snaps {
        let view = view as *mut Object;
        if view.is_null() {
            continue;
        }
        let _: () = msg_send![view, setNeedsDisplay: true];
        let _: () = msg_send![view, displayIfNeeded];
    }
}

/// Scoop all: windows unchanged, overlay gone, meow killed. Idempotent;
/// safe from any thread (hops to main when needed) and from app exit.
pub fn stop() -> Result<(), String> {
    if !active() {
        return Ok(());
    }
    crate::main_thread_sync(|| {
        let _ = catch_unwind(AssertUnwindSafe(stop_on_main))
            .map_err(|p| eprintln!("yard: stop panicked: {}", panic_msg(&*p)));
    });
    Ok(())
}

/// Main thread. Tears down panels, kills the meow, pulls the Esc
/// monitor, forgets every cat (session only — nothing to persist).
pub(crate) fn stop_on_main() {
    let mut y = lock();
    if !y.on {
        return;
    }
    y.on = false;
    super::meow::kill(&mut y.meow);
    unsafe {
        remove_esc_monitor(&mut y);
        for p in y.panels.drain(..) {
            let panel = p.panel as *mut Object;
            let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
            let _: () = msg_send![panel, release];
        }
        for art in y.packs.drain(..) {
            for img in art.walk.into_iter().chain(art.idle) {
                let _: () = msg_send![img as *mut Object, release];
            }
        }
    }
    y.cats.clear();
    y.queue.clear();
    y.scooped.clear();
    y.press = None;
    y.caption = None;
}
