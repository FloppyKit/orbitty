//! Listen-only rail mouse diagnosis. Installed only while the stage is
//! revealed and only when `RAIL_MOUSE_DIAG=1`. Unset installs nothing and
//! logs nothing. The R2 local monitor in `windows_mac.rs` is a separate
//! path and is not modified here.
//!
//! Observers never consume, rewrite, or drop an event. The local NSEvent
//! monitor returns the same event. The CGEvent tap is created with
//! listen-only. This module does not prompt for TCC.

#![allow(unexpected_cfgs)]

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// NSEventType / CGEventType bits. directTouch is 37. Mouse-moved is absent.
const DIAG_MASK: u64 = (1u64 << 1)
    | (1u64 << 2)
    | (1u64 << 3)
    | (1u64 << 4)
    | (1u64 << 10)
    | (1u64 << 11)
    | (1u64 << 12)
    | (1u64 << 14)
    | (1u64 << 18)
    | (1u64 << 19)
    | (1u64 << 20)
    | (1u64 << 22)
    | (1u64 << 25)
    | (1u64 << 26)
    | (1u64 << 29)
    | (1u64 << 30)
    | (1u64 << 31)
    | (1u64 << 32)
    | (1u64 << 34)
    | (1u64 << 37);

const MASK_NO_DIRECT: u64 = DIAG_MASK & !(1u64 << 37);

/// Noisy streams. One count line per source per type per second.
const COUNTED: [u64; 6] = [18, 22, 29, 30, 34, 37];

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

/// kCGSessionEventTap, then kCGHIDEventTap. kCGEventTapOptionListenOnly is 1.
const TAP_SESSION: u32 = 1;
const TAP_HID: u32 = 0;
const TAP_HEAD: u32 = 0;
const TAP_LISTEN_ONLY: u32 = 1;

const FIELD_MOUSE_SUBTYPE: u32 = 7;
const FIELD_KEY_CODE: u32 = 9;
const FIELD_BUTTON: u32 = 3;

macro_rules! zero_counts {
    () => {
        [
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
        ]
    };
}

static COUNTS: [AtomicU64; 18] = zero_counts!();
static LAST_FLUSH: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicBool = AtomicBool::new(false);
static OFF_MAIN_LOGGED: AtomicBool = AtomicBool::new(false);
static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TAP_DEAD_LOG_MS: AtomicU64 = AtomicU64::new(0);

struct Slot {
    global_mon: *mut Object,
    local_mon: *mut Object,
    tap: *mut c_void,
    source: *mut c_void,
}

unsafe impl Send for Slot {}

static SLOT: Mutex<Option<Slot>> = Mutex::new(None);

#[repr(C)]
struct DiagBlockVoid {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const DiagBlockVoid, *mut Object),
    descriptor: *const DiagBlockDesc,
}

#[repr(C)]
struct DiagBlockRet {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const DiagBlockRet, *mut Object) -> *mut Object,
    descriptor: *const DiagBlockDesc,
}

#[repr(C)]
struct DiagBlockDesc {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static DIAG_CONCRETE_BLOCK: c_void;
    fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
}

static VOID_DESC: DiagBlockDesc = DiagBlockDesc {
    reserved: 0,
    size: std::mem::size_of::<DiagBlockVoid>() as std::os::raw::c_ulong,
};

static RET_DESC: DiagBlockDesc = DiagBlockDesc {
    reserved: 0,
    size: std::mem::size_of::<DiagBlockRet>() as std::os::raw::c_ulong,
};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: u8);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopGetCurrent() -> *mut c_void;
}

struct Detail {
    button: Option<i64>,
    key: Option<u16>,
    cmd: u8,
    shift: u8,
    opt: u8,
    ctrl: u8,
    func: u8,
    subtype: Option<i64>,
    data1: Option<i64>,
    data2: Option<i64>,
    phase: Option<u64>,
    mag: u8,
}

impl Detail {
    fn blank() -> Self {
        Self {
            button: None,
            key: None,
            cmd: 0,
            shift: 0,
            opt: 0,
            ctrl: 0,
            func: 0,
            subtype: None,
            data1: None,
            data2: None,
            phase: None,
            mag: 0,
        }
    }

    fn counted(ty: u64) -> Self {
        let mut d = Self::blank();
        if ty == 30 {
            d.mag = 1;
        }
        d
    }
}

struct Snap {
    active: u8,
    front: String,
    revealed: u8,
    t: u64,
}

fn diag_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| matches!(std::env::var("RAIL_MOUSE_DIAG").as_deref(), Ok("1")))
}

fn mono_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn type_name(ty: u64) -> &'static str {
    match ty {
        1 => "leftMouseDown",
        2 => "leftMouseUp",
        3 => "rightMouseDown",
        4 => "rightMouseUp",
        10 => "keyDown",
        11 => "keyUp",
        12 => "flagsChanged",
        14 => "systemDefined",
        18 => "rotate",
        19 => "beginGesture",
        20 => "endGesture",
        22 => "scrollWheel",
        25 => "otherMouseDown",
        26 => "otherMouseUp",
        29 => "gesture",
        30 => "magnify",
        31 => "swipe",
        32 => "smartMagnify",
        34 => "pressure",
        37 => "directTouch",
        0xFFFF_FFFE => "tapDisabledByTimeout",
        0xFFFF_FFFF => "tapDisabledByUserInput",
        _ => "other",
    }
}

fn is_counted(ty: u64) -> bool {
    COUNTED.contains(&ty)
}

fn type_slot(ty: u64) -> Option<usize> {
    COUNTED.iter().position(|&t| t == ty)
}

fn source_ix(source: &str) -> usize {
    match source {
        "global" => 0,
        "local" => 1,
        _ => 2,
    }
}

fn mods(flags: u64) -> (u8, u8, u8, u8, u8) {
    (
        u8::from(flags & (1u64 << 20) != 0),
        u8::from(flags & (1u64 << 17) != 0),
        u8::from(flags & (1u64 << 19) != 0),
        u8::from(flags & (1u64 << 18) != 0),
        u8::from(flags & (1u64 << 23) != 0),
    )
}

fn bundle_token(raw: &str) -> String {
    if raw.is_empty() || raw.len() > 128 {
        return "-".to_string();
    }
    let ok = raw
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
    if ok {
        raw.to_string()
    } else {
        "-".to_string()
    }
}

unsafe fn ns_token(obj: *mut Object) -> String {
    if obj.is_null() {
        return "-".to_string();
    }
    let utf: *const i8 = msg_send![obj, UTF8String];
    if utf.is_null() {
        return "-".to_string();
    }
    bundle_token(&CStr::from_ptr(utf).to_string_lossy())
}

unsafe fn own_bundle() -> String {
    let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
    if bundle.is_null() {
        return "-".to_string();
    }
    let ident: *mut Object = msg_send![bundle, bundleIdentifier];
    ns_token(ident)
}

unsafe fn front_bundle() -> String {
    let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    if ws.is_null() {
        return "-".to_string();
    }
    let app: *mut Object = msg_send![ws, frontmostApplication];
    if app.is_null() {
        return "-".to_string();
    }
    let ident: *mut Object = msg_send![app, bundleIdentifier];
    ns_token(ident)
}

unsafe fn app_is_active() -> u8 {
    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    if app.is_null() {
        return 0;
    }
    let on: bool = msg_send![app, isActive];
    u8::from(on)
}

fn on_main() -> bool {
    unsafe {
        let on: bool = msg_send![class!(NSThread), isMainThread];
        on
    }
}

fn snap() -> Snap {
    let (active, front) = unsafe { (app_is_active(), front_bundle()) };
    Snap {
        active,
        front,
        revealed: u8::from(crate::windows::drawers::rail_stage_is_revealed()),
        t: mono_ms(),
    }
}

fn dash_i64(v: Option<i64>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "-".to_string(),
    }
}

fn dash_u64(v: Option<u64>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "-".to_string(),
    }
}

fn dash_key(v: Option<u16>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "-".to_string(),
    }
}

fn emit(source: &str, ty: u64, d: &Detail, count: u64, snap: &Snap) {
    eprintln!(
        "rail-mouse-diag ev source={source} type={ty} name={} button={} key={} cmd={} shift={} opt={} ctrl={} fn={} subtype={} data1={} data2={} phase={} mag={} active={} front={} revealed={} t={} count={count}",
        type_name(ty),
        dash_i64(d.button),
        dash_key(d.key),
        d.cmd,
        d.shift,
        d.opt,
        d.ctrl,
        d.func,
        dash_i64(d.subtype),
        dash_i64(d.data1),
        dash_i64(d.data2),
        dash_u64(d.phase),
        d.mag,
        snap.active,
        snap.front,
        snap.revealed,
        snap.t,
    );
}

fn flush_counts() {
    LAST_FLUSH.store(mono_ms(), Ordering::Relaxed);
    let mut pending: [(usize, u64, u64); 18] = [(0, 0, 0); 18];
    let mut n = 0usize;
    for i in 0..18 {
        let c = COUNTS[i].swap(0, Ordering::Relaxed);
        if c == 0 {
            continue;
        }
        pending[n] = (i / 6, COUNTED[i % 6], c);
        n += 1;
    }
    if n == 0 {
        return;
    }
    let snap = snap();
    const SOURCES: [&str; 3] = ["global", "local", "tap"];
    for item in pending.iter().take(n) {
        let (si, ty, c) = *item;
        emit(SOURCES[si], ty, &Detail::counted(ty), c, &snap);
    }
}

fn note_count(source: &str, ty: u64) {
    let Some(slot) = type_slot(ty) else {
        return;
    };
    let ix = source_ix(source) * 6 + slot;
    COUNTS[ix].fetch_add(1, Ordering::Relaxed);
    let now = mono_ms();
    let prev = LAST_FLUSH.load(Ordering::Relaxed);
    if now.saturating_sub(prev) >= 1000
        && LAST_FLUSH
            .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        flush_counts();
    }
}

fn log_discrete(source: &str, ty: u64, detail: Detail) {
    flush_counts();
    let snap = snap();
    emit(source, ty, &detail, 1, &snap);
}

unsafe fn detail_ns(event: *mut Object, ty: u64) -> Detail {
    let mut d = Detail::blank();
    let flags: u64 = msg_send![event, modifierFlags];
    (d.cmd, d.shift, d.opt, d.ctrl, d.func) = mods(flags);
    if matches!(ty, 1 | 2 | 3 | 4 | 25 | 26) {
        let button: i64 = msg_send![event, buttonNumber];
        d.button = Some(button);
    }
    if matches!(ty, 10 | 11 | 12) {
        let key: u16 = msg_send![event, keyCode];
        d.key = Some(key);
    }
    if ty == 14 {
        let subtype: i64 = msg_send![event, subtype];
        let data1: i64 = msg_send![event, data1];
        let data2: i64 = msg_send![event, data2];
        d.subtype = Some(subtype);
        d.data1 = Some(data1);
        d.data2 = Some(data2);
    }
    if matches!(ty, 19 | 20 | 31 | 32) {
        let phase: u64 = msg_send![event, phase];
        d.phase = Some(phase);
    }
    if ty == 30 || ty == 32 {
        d.mag = 1;
    }
    d
}

unsafe fn detail_cg(event: *mut c_void, ty: u64) -> Detail {
    let mut d = Detail::blank();
    let flags = CGEventGetFlags(event);
    (d.cmd, d.shift, d.opt, d.ctrl, d.func) = mods(flags);
    if matches!(ty, 1 | 2 | 3 | 4 | 25 | 26) {
        d.button = Some(CGEventGetIntegerValueField(event, FIELD_BUTTON));
    }
    if matches!(ty, 10 | 11 | 12) {
        let key = CGEventGetIntegerValueField(event, FIELD_KEY_CODE);
        d.key = Some(key as u16);
    }
    if ty == 14 {
        d.subtype = Some(CGEventGetIntegerValueField(event, FIELD_MOUSE_SUBTYPE));
    }
    if ty == 30 || ty == 32 {
        d.mag = 1;
    }
    d
}

unsafe fn observe_ns(source: &'static str, event: *mut Object) {
    let ty: u64 = msg_send![event, type];
    if is_counted(ty) {
        note_count(source, ty);
        return;
    }
    let detail = detail_ns(event, ty);
    log_discrete(source, ty, detail);
}

fn note_tap_disabled(ty: u32) {
    let port = TAP_PORT.load(Ordering::Relaxed);
    if !port.is_null() {
        unsafe { CGEventTapEnable(port, 1) };
    }
    let now = mono_ms();
    let prev = TAP_DEAD_LOG_MS.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < 1000 {
        return;
    }
    if TAP_DEAD_LOG_MS
        .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    log_discrete("tap", ty as u64, Detail::blank());
}

extern "C" fn diag_global(_block: *const DiagBlockVoid, event: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if event.is_null() || !LIVE.load(Ordering::Relaxed) {
            return;
        }
        unsafe { observe_ns("global", event) };
    }));
}

extern "C" fn diag_local(_block: *const DiagBlockRet, event: *mut Object) -> *mut Object {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if event.is_null() || !LIVE.load(Ordering::Relaxed) {
            return;
        }
        unsafe { observe_ns("local", event) };
    }));
    event
}

extern "C" fn diag_tap(
    _proxy: *mut c_void,
    ty: u32,
    event: *mut c_void,
    _user: *mut c_void,
) -> *mut c_void {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !LIVE.load(Ordering::Relaxed) {
            return;
        }
        if ty == 0xFFFF_FFFE || ty == 0xFFFF_FFFF {
            note_tap_disabled(ty);
            return;
        }
        if event.is_null() {
            return;
        }
        let ty64 = ty as u64;
        if is_counted(ty64) {
            note_count("tap", ty64);
            return;
        }
        let detail = unsafe { detail_cg(event, ty64) };
        log_discrete("tap", ty64, detail);
    }));
    event
}

fn void_block() -> *const DiagBlockVoid {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(DiagBlockVoid {
            isa: unsafe { &DIAG_CONCRETE_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: diag_global,
            descriptor: &VOID_DESC,
        })) as usize
    }) as *const DiagBlockVoid
}

fn ret_block() -> *const DiagBlockRet {
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(DiagBlockRet {
            isa: unsafe { &DIAG_CONCRETE_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: diag_local,
            descriptor: &RET_DESC,
        })) as usize
    }) as *const DiagBlockRet
}

fn add_mask(global: bool, mask: u64) -> *mut Object {
    let mon: *mut Object = if global {
        let block = void_block();
        unsafe {
            msg_send![class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: mask
                handler: block]
        }
    } else {
        let block = ret_block();
        unsafe {
            msg_send![class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: mask
                handler: block]
        }
    };
    if !mon.is_null() {
        unsafe {
            let _: *mut Object = msg_send![mon, retain];
        }
    }
    mon
}

fn add_monitor(global: bool) -> *mut Object {
    let mon = add_mask(global, DIAG_MASK);
    if !mon.is_null() {
        return mon;
    }
    add_mask(global, MASK_NO_DIRECT)
}

fn runloop_mode() -> CFString {
    CFString::new("kCFRunLoopCommonModes")
}

struct TapMade {
    tap: *mut c_void,
    source: *mut c_void,
    status: &'static str,
    at: &'static str,
}

fn create_tap() -> TapMade {
    let mut at = "session";
    let mut tap = unsafe {
        CGEventTapCreate(
            TAP_SESSION,
            TAP_HEAD,
            TAP_LISTEN_ONLY,
            DIAG_MASK,
            diag_tap,
            std::ptr::null_mut(),
        )
    };
    if tap.is_null() {
        at = "hid";
        tap = unsafe {
            CGEventTapCreate(
                TAP_HID,
                TAP_HEAD,
                TAP_LISTEN_ONLY,
                DIAG_MASK,
                diag_tap,
                std::ptr::null_mut(),
            )
        };
    }
    if tap.is_null() {
        return TapMade {
            tap: std::ptr::null_mut(),
            source: std::ptr::null_mut(),
            status: "denied",
            at: "-",
        };
    }
    let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if source.is_null() {
        unsafe { CFRelease(tap as *const c_void) };
        return TapMade {
            tap: std::ptr::null_mut(),
            source: std::ptr::null_mut(),
            status: "denied",
            at: "-",
        };
    }
    let mode = runloop_mode();
    unsafe {
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, mode.as_concrete_TypeRef() as *const c_void);
        CGEventTapEnable(tap, 1);
    }
    TAP_PORT.store(tap, Ordering::Relaxed);
    TapMade {
        tap,
        source,
        status: "created",
        at,
    }
}

fn listen_word() -> &'static str {
    let sym = unsafe {
        dlsym(
            (-2isize) as *mut c_void,
            b"CGPreflightListenEventAccess\0".as_ptr() as *const i8,
        )
    };
    if sym.is_null() {
        return "unknown";
    }
    let f: unsafe extern "C" fn() -> u8 = unsafe { std::mem::transmute(sym) };
    if unsafe { f() } == 0 {
        "0"
    } else {
        "1"
    }
}

fn zero_count_slots() {
    for c in &COUNTS {
        c.store(0, Ordering::Relaxed);
    }
}

fn mon_word(mon: *mut Object) -> &'static str {
    if mon.is_null() {
        "nil"
    } else {
        "ok"
    }
}

fn install() {
    if !on_main() {
        if OFF_MAIN_LOGGED
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("rail-mouse-diag cap thread=off-main");
        }
        return;
    }
    {
        let guard = SLOT.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            return;
        }
    }
    let made = create_tap();
    let global_mon = add_monitor(true);
    let local_mon = add_monitor(false);
    {
        let mut guard = SLOT.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            unsafe {
                drop_monitor(global_mon);
                drop_monitor(local_mon);
                drop_tap(made.tap, made.source);
            }
            return;
        }
        *guard = Some(Slot {
            global_mon,
            local_mon,
            tap: made.tap,
            source: made.source,
        });
    }
    zero_count_slots();
    LAST_FLUSH.store(mono_ms(), Ordering::Relaxed);
    LIVE.store(true, Ordering::Relaxed);
    let ax = u8::from(unsafe { AXIsProcessTrusted() });
    let bundle = unsafe { own_bundle() };
    eprintln!(
        "rail-mouse-diag cap ax={ax} listen={} tap={} tapAt={} global={} local={} bundle={bundle}",
        listen_word(),
        made.status,
        made.at,
        mon_word(global_mon),
        mon_word(local_mon),
    );
}

unsafe fn drop_monitor(mon: *mut Object) {
    if mon.is_null() {
        return;
    }
    let _: () = msg_send![class!(NSEvent), removeMonitor: mon];
    let _: () = msg_send![mon, release];
}

unsafe fn drop_tap(tap: *mut c_void, source: *mut c_void) {
    TAP_PORT.store(std::ptr::null_mut(), Ordering::Relaxed);
    if !tap.is_null() {
        CGEventTapEnable(tap, 0);
    }
    if !source.is_null() {
        let mode = runloop_mode();
        let rl = CFRunLoopGetCurrent();
        CFRunLoopRemoveSource(rl, source, mode.as_concrete_TypeRef() as *const c_void);
        CFRelease(source as *const c_void);
    }
    if !tap.is_null() {
        CFRelease(tap as *const c_void);
    }
}

fn remove() {
    if !on_main() {
        if OFF_MAIN_LOGGED
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("rail-mouse-diag cap thread=off-main");
        }
        return;
    }
    LIVE.store(false, Ordering::Relaxed);
    let slot = {
        let mut guard = SLOT.lock().unwrap_or_else(|e| e.into_inner());
        guard.take()
    };
    let Some(slot) = slot else {
        return;
    };
    unsafe {
        drop_monitor(slot.global_mon);
        drop_monitor(slot.local_mon);
        drop_tap(slot.tap, slot.source);
    }
    flush_counts();
    eprintln!("rail-mouse-diag cap state=removed");
}

/// Install while revealed, remove while parked. No-op when the env is unset.
pub(crate) fn sync_revealed(revealed: bool) {
    if !diag_on() {
        return;
    }
    if revealed {
        install();
    } else {
        remove();
    }
}

/// App exit. Idempotent with park.
pub(crate) fn on_app_drop() {
    if !diag_on() {
        return;
    }
    remove();
}
