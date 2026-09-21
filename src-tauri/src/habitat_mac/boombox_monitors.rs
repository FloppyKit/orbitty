//! Boombox HUD Esc / click-away NSEvent monitors (DIET-HABITAT).
//!
//! Peeled from `boombox_hud.rs` — yard block pattern; installed only while
//! the HUD is open.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::audio;
use super::boombox_hud;
use super::lock;

// Esc / click-away monitors (yard block pattern)

/// NSEventMask: keyDown (1<<10) | leftMouseDown (1<<1) | rightMouseDown
/// (1<<2). Installed only while the HUD is open.
const HUD_EVENT_MASK: u64 = 1030;

static HUD_MONITOR: AtomicUsize = AtomicUsize::new(0);
static HUD_LOCAL_MONITOR: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct HudBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const HudBlock, *mut Object),
    descriptor: *const HudBlockDescriptor,
}

#[repr(C)]
struct HudBlockDescriptor {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static NS_CONCRETE_GLOBAL_BLOCK: c_void;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static HUD_BLOCK_DESCRIPTOR: HudBlockDescriptor = HudBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<HudBlock>() as std::os::raw::c_ulong,
};

fn hud_block() -> *const HudBlock {
    static BLOCK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(HudBlock {
            // SAFETY: referencing the libSystem global-block class.
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: hud_invoke,
            descriptor: &HUD_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const HudBlock
}

/// Local-monitor twin: the handler MUST hand the event back (nil would
/// eat the key/click). Same layout, pointer return.
#[repr(C)]
struct HudLocalBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const HudLocalBlock, *mut Object) -> *mut Object,
    descriptor: *const HudBlockDescriptor,
}

static HUD_LOCAL_BLOCK_DESCRIPTOR: HudBlockDescriptor = HudBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<HudLocalBlock>() as std::os::raw::c_ulong,
};

fn hud_local_block() -> *const HudLocalBlock {
    static BLOCK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(HudLocalBlock {
            // SAFETY: referencing the libSystem global-block class.
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: hud_local_invoke,
            descriptor: &HUD_LOCAL_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const HudLocalBlock
}

/// NSEventType: 1 leftDown, 3 rightDown, 10 keyDown. kVK_Escape = 53.
unsafe fn event_dismisses(event: *mut Object) -> bool {
    let ty: u64 = msg_send![event, type];
    match ty {
        10 => {
            let key: u16 = msg_send![event, keyCode];
            key == 53
        }
        1 | 3 => true,
        _ => false,
    }
}

extern "C" fn hud_invoke(_block: *const HudBlock, event: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        // Global monitor: only OTHER apps' events — every one is away.
        if event_dismisses(event) {
            hop_close();
        }
    }));
}

extern "C" fn hud_local_invoke(_block: *const HudLocalBlock, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if event.is_null() {
            return;
        }
        let ty: u64 = msg_send![event, type];
        let hit = match ty {
            10 => {
                let key: u16 = msg_send![event, keyCode];
                key == 53
            }
            1 | 3 => {
                // Our planet panels handle their own downs in
                // mouse_down_inner (the boombox down must fall through
                // to the click toggle) — the monitor's job is the rest
                // of our glass (rail, companion).
                let win: *mut Object = msg_send![event, window];
                let on_planet = {
                    let h = lock();
                    h.planets.values().any(|p| p.panel == win as usize)
                };
                !on_planet
            }
            _ => false,
        };
        if hit {
            hop_close();
        }
    }));
    event
}

/// Monitor handlers are not promised the main thread — the close rides
/// the same main-queue hop as the music picker.
fn hop_close() {
    audio::defer_main(|| {
        let mut h = lock();
        boombox_hud::close(&mut h);
    });
}

pub(crate) fn install_monitors() {
    unsafe {
        if HUD_MONITOR.load(Ordering::SeqCst) == 0 {
            let monitor: *mut Object = msg_send![class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: HUD_EVENT_MASK
                handler: hud_block()];
            if !monitor.is_null() {
                let _: *mut Object = msg_send![monitor, retain];
                HUD_MONITOR.store(monitor as usize, Ordering::SeqCst);
            }
        }
        if HUD_LOCAL_MONITOR.load(Ordering::SeqCst) == 0 {
            let monitor: *mut Object = msg_send![class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: HUD_EVENT_MASK
                handler: hud_local_block()];
            if !monitor.is_null() {
                let _: *mut Object = msg_send![monitor, retain];
                HUD_LOCAL_MONITOR.store(monitor as usize, Ordering::SeqCst);
            }
        }
    }
}

pub(crate) fn remove_monitors() {
    let global = HUD_MONITOR.swap(0, Ordering::SeqCst);
    if global != 0 {
        unsafe {
            let monitor = global as *mut Object;
            let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
            let _: () = msg_send![monitor, release];
        }
    }
    let local = HUD_LOCAL_MONITOR.swap(0, Ordering::SeqCst);
    if local != 0 {
        unsafe {
            let monitor = local as *mut Object;
            let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
            let _: () = msg_send![monitor, release];
        }
    }
}
