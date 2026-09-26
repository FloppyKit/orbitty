//! Unclaimed windows as session-only cats. One panel per display.
//! Never persist. Never steal a claimed walker. Hits live in input.rs.

// objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that newer
// rustc flags; harmless upstream noise (same allowance as habitat_mac).
#![allow(unexpected_cfgs)]

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use serde::Serialize;
use std::collections::HashSet;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

mod input;
mod meow;

// geometry (habitat stays crate-private; these are the same
// handful of POD helpers)

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct NSRect {
    origin: NSPoint,
    size: CGSize,
}

impl NSRect {
    fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        NSRect {
            origin: NSPoint { x, y },
            size: CGSize {
                width: w,
                height: h,
            },
        }
    }
    fn max_x(&self) -> f64 {
        self.origin.x + self.size.width
    }
    fn max_y(&self) -> f64 {
        self.origin.y + self.size.height
    }
    fn inset(&self, d: f64) -> Self {
        NSRect::new(
            self.origin.x + d,
            self.origin.y + d,
            self.size.width - 2.0 * d,
            self.size.height - 2.0 * d,
        )
    }
    fn contains(&self, p: NSPoint) -> bool {
        p.x >= self.origin.x
            && p.x <= self.max_x()
            && p.y >= self.origin.y
            && p.y <= self.max_y()
    }
}

// Lets NSRect appear in a registered method signature (drawRect:); the
// string must match AppKit's own encoding for CGRect.
unsafe impl objc::Encode for NSRect {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

unsafe impl objc::Encode for CGSize {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGSize=dd}") }
    }
}

// tuning

/// Bitty cat frames are 32×32; drawn at 2× on the desktop (Neko size).
const CAT: f64 = 64.0;
/// Worker cadence; the main hop runs at the same rate while the yard is on.
const TICK_MS: u64 = 33;
/// Rain fall speed (px/s) — fast enough to read as rain, slow enough to see.
const RAIN_VY: f64 = 560.0;
/// Amble speed band (px/s). y is damped so paths read as a cat strolling,
/// not a puck on ice.
const WALK_MIN: f64 = 22.0;
const WALK_MAX: f64 = 44.0;
/// Spawn stagger between raining cats (lock: ~80–150ms apart).
const STAGGER_MIN_MS: u64 = 80;
const STAGGER_MAX_MS: u64 = 150;
/// Unmapped re-poll cadence (the rail ticks at 800ms; the yard is no
/// hotter). New unclaimed windows rain in; claimed/closed cats leave.
const POLL_EVERY: u32 = 30; // ~1s at 33ms

/// The four yard colorways, cycled by spawn order so the clowder is never
/// clones. Walk frames are already in the tree (bitty-crew, seeded to
/// app-support packs at setup).
const CAT_PACKS: [&str; 4] = [
    "orbitty-bitty-cat",
    "orbitty-bitty-cat-black",
    "orbitty-bitty-cat-calico",
    "orbitty-bitty-cat-gray",
];

// state

#[derive(Clone, Copy, PartialEq)]
enum Gait {
    /// Falling from above the visible frame toward the floor.
    Rain,
    /// Ambling (or paused) on the desktop.
    Walk,
}

struct Cat {
    window_id: String,
    app_name: String,
    title: String,
    screen_id: String,
    /// View-local flipped (y-down) top-left of the sprite box.
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    gait: Gait,
    /// Index into `packs` (the CAT_PACKS cycle).
    pack: usize,
    anim: u32,
    facing_left: bool,
    /// Landing strip (local y of the floor for this cat).
    floor_y: f64,
    move_until: Option<Instant>,
    pause_until: Option<Instant>,
}

struct PendingCat {
    window_id: String,
    app_name: String,
    title: String,
    at: Instant,
}

/// A live press on a cat (mouseDown → mouseUp), click-or-drag undecided
/// until the slop or the up. Mirrors the habitat's walker press: ≥4px
/// of travel is a drag, under that a click.
struct CatPress {
    window_id: String,
    /// Global bottom-left pointer at the DOWN EVENT (its own location,
    /// never a dispatch-time mouseLocation) — the slop anchor.
    start: NSPoint,
    moved: f64,
    /// Pointer offset inside the sprite box at grab — the cat follows
    /// the hand without snapping its center to the pointer.
    grab_dx: f64,
    grab_dy: f64,
    /// The down event's clickCount — the release catch in press_tick
    /// can run the full click verdict even when the hop beats mouseUp.
    clicks: usize,
}

struct YardPanel {
    screen_id: String,
    panel: usize, // NSPanel
    view: usize,  // OrbittyYardView
    /// Global bottom-left frame of the whole display.
    frame: NSRect,
    /// View-local flipped walk bounds (visibleFrame minus OS chrome).
    walk: NSRect,
    /// Drew cats last frame — one extra clear when the last one leaves.
    drawn: bool,
}

/// One decoded colorway: retained NSImages (walk cycle + optional idle).
struct CatArt {
    walk: Vec<usize>,
    idle: Vec<usize>,
}

struct Yard {
    on: bool,
    panels: Vec<YardPanel>,
    cats: Vec<Cat>,
    queue: Vec<PendingCat>,
    /// Scooped-this-cat window ids, suppressed until the mode ends.
    scooped: HashSet<String>,
    packs: Vec<CatArt>,
    spawn_count: usize,
    /// First successful poll landed — the auto-off ("last cat gone") only
    /// arms after this, so the start-up beat never insta-stops.
    poll_done: bool,
    last_hop: Option<Instant>,
    hop_count: u32,
    meow: meow::Meow,
    /// NSEvent global Esc monitor (retained), 0 = not installed.
    esc_monitor: usize,
    /// NSEvent local Esc monitor (retained), 0 = not installed. Global
    /// monitors never see our own app's keys, so this is what scoops
    /// while Orbitty is key.
    esc_local_monitor: usize,
    /// Live press on a cat (leftover: hits) — click-or-drag until the
    /// 4px slop or the up decides. input.rs owns the verdicts.
    press: Option<CatPress>,
    /// Single-click app-name chip: (window id, armed at), ~2s.
    caption: Option<(String, Instant)>,
}

impl Default for Yard {
    fn default() -> Self {
        Yard {
            on: false,
            panels: Vec::new(),
            cats: Vec::new(),
            queue: Vec::new(),
            scooped: HashSet::new(),
            packs: Vec::new(),
            spawn_count: 0,
            poll_done: false,
            last_hop: None,
            hop_count: 0,
            meow: meow::Meow::default(),
            esc_monitor: 0,
            esc_local_monitor: 0,
            press: None,
            caption: None,
        }
    }
}

fn yard() -> &'static Mutex<Yard> {
    static Y: OnceLock<Mutex<Yard>> = OnceLock::new();
    Y.get_or_init(|| Mutex::new(Yard::default()))
}

fn lock() -> std::sync::MutexGuard<'static, Yard> {
    yard().lock().unwrap_or_else(|e| e.into_inner())
}

fn app_handle_cell() -> &'static OnceLock<AppHandle> {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    &APP
}

fn app_handle() -> Option<&'static AppHandle> {
    app_handle_cell().get()
}

/// Coalesces main-queue hops: at most one queued-or-running.
static HOP_PENDING: AtomicBool = AtomicBool::new(false);

// small objc helpers

/// Toll-free-bridged NSString, intentionally leaked (used as a constant).
unsafe fn ns_string(s: &str) -> *mut Object {
    let cf = CFString::new(s);
    let raw = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    raw
}

fn rand_unit() -> f64 {
    static RNG: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut g = RNG
        .get_or_init(|| {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
                .unwrap_or(0);
            Mutex::new(seed ^ 0x9e37_79b9_7f4a_7c15)
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut x = *g;
    if x == 0 {
        x = 0x2545_f491_4f6c_dd1d;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *g = x;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn rand_range(lo: f64, hi: f64) -> f64 {
    lo + rand_unit() * (hi - lo)
}

fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

// screens

unsafe fn screens() -> Vec<(String, NSRect)> {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![arr, count];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, frame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: ns_string("NSScreenNumber")]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        out.push((id, frame));
    }
    out
}

/// visibleFrame excludes the menu bar / Dock — the walk bounds, so a cat
/// never ambles under OS chrome. Rain still falls through the full frame.
unsafe fn visible_frame_for(s: *mut Object) -> NSRect {
    msg_send![s, visibleFrame]
}

/// Walk bounds in view-local flipped (y-down) coords for a display whose
/// global frame is `frame` and visibleFrame is `vis`.
fn walk_rect(frame: &NSRect, vis: &NSRect) -> NSRect {
    let x0 = vis.origin.x - frame.origin.x;
    let top = frame.max_y() - vis.max_y(); // menu-bar strip
    let bottom = frame.max_y() - vis.origin.y; // above the Dock
    NSRect::new(
        x0 + 4.0,
        top + 4.0,
        (vis.size.width - 8.0).max(CAT * 2.0),
        (bottom - top - 8.0).max(CAT * 2.0),
    )
}

// cat art (pack frames; habitat's loaders stay crate-private)

/// NSImage from encoded bytes — initWithData decodes png/gif/webp alike.
/// Returns a retained image (caller releases).
unsafe fn ns_image_from_bytes(bytes: &[u8]) -> *mut Object {
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let img: *mut Object = msg_send![class!(NSImage), alloc];
    let img: *mut Object = msg_send![img, initWithData: data];
    if img.is_null() {
        return std::ptr::null_mut();
    }
    // initWithData can leave NSSizeZero until a rep is queried; drawInRect
    // of a zero-size image is a silent no-op (meows, no cats).
    let size: CGSize = msg_send![img, size];
    if size.width < 1.0 || size.height < 1.0 {
        let reps: *mut Object = msg_send![img, representations];
        let n: usize = if reps.is_null() {
            0
        } else {
            msg_send![reps, count]
        };
        if n > 0 {
            let rep: *mut Object = msg_send![reps, objectAtIndex: 0usize];
            if !rep.is_null() {
                let w: isize = msg_send![rep, pixelsWide];
                let h: isize = msg_send![rep, pixelsHigh];
                if w > 0 && h > 0 {
                    let _: () = msg_send![img, setSize: CGSize {
                        width: w as f64,
                        height: h as f64,
                    }];
                }
            }
        }
    }
    img
}

/// Decode the four yard colorways from the seeded packs dir. Paths only
/// from packs::pack_art, decoded here — a failed file just drops out of
/// its cycle. Empty across all four = no art, and start refuses (never
/// rain invisible cats).
fn load_cat_art(app: &AppHandle) -> Vec<CatArt> {
    let all = crate::packs::pack_art(app);
    let mut out = Vec::new();
    for slug in CAT_PACKS {
        let Some((_, art)) = all.iter().find(|(id, _)| id == slug) else {
            continue;
        };
        let walk = unsafe { load_source(&art.walk) };
        if walk.is_empty() {
            continue;
        }
        let idle = match &art.idle {
            Some(src) => unsafe { load_source(src) },
            None => Vec::new(),
        };
        out.push(CatArt { walk, idle });
    }
    out
}

unsafe fn load_source(src: &crate::packs::ArtSource) -> Vec<usize> {
    match src {
        crate::packs::ArtSource::Frames(paths) => load_frame_files(paths),
        crate::packs::ArtSource::Sheet { file, spec } => slice_sheet(file, spec),
    }
}

const MAX_WALK_FRAMES: usize = 8;

unsafe fn load_frame_files(paths: &[std::path::PathBuf]) -> Vec<usize> {
    let mut out = Vec::new();
    for p in paths {
        if out.len() >= MAX_WALK_FRAMES {
            break;
        }
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let img = ns_image_from_bytes(&bytes);
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGImageCreateWithImageInRect(image: *const c_void, rect: NSRect) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Bitty packs ship loose frames; the sheet path only exists so an
/// imported sheet-style recolor still walks.
unsafe fn slice_sheet(path: &std::path::Path, spec: &crate::packs::SheetSpec) -> Vec<usize> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return Vec::new();
    }
    let rep: *mut Object = msg_send![class!(NSBitmapImageRep), imageRepWithData: data];
    if rep.is_null() {
        return Vec::new();
    }
    let fw = spec.frame_w as isize;
    let fh = spec.frame_h as isize;
    let width: isize = msg_send![rep, pixelsWide];
    let height: isize = msg_send![rep, pixelsHigh];
    if fw <= 0 || fh <= 0 || width < fw || height < fh {
        return Vec::new();
    }
    let cg: *const c_void = msg_send![rep, CGImage];
    if cg.is_null() {
        return Vec::new();
    }
    let cols = (width / fw).max(1);
    let rows = (height / fh).max(1);
    let total = (spec.count as isize).min(cols * rows).max(0);
    let mut out = Vec::new();
    for i in 0..total {
        let x = (i % cols) * fw;
        let y = (i / cols) * fh;
        let sub = CGImageCreateWithImageInRect(
            cg,
            NSRect::new(x as f64, y as f64, fw as f64, fh as f64),
        );
        if sub.is_null() {
            continue;
        }
        let frame_rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
        let frame_rep: *mut Object = msg_send![frame_rep, initWithCGImage: sub];
        CFRelease(sub);
        if frame_rep.is_null() {
            continue;
        }
        let img: *mut Object = msg_send![class!(NSImage), alloc];
        let img: *mut Object = msg_send![img, init];
        let _: () = msg_send![img, addRepresentation: frame_rep];
        let _: () = msg_send![frame_rep, release];
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}

// the yard view class

fn yard_view_class() -> Option<&'static Class> {
    static CLS: OnceLock<Option<&'static Class>> = OnceLock::new();
    CLS.get_or_init(|| {
        catch_unwind(AssertUnwindSafe(|| unsafe { register_yard_view_class() }))
            .unwrap_or_else(|p| {
                eprintln!("yard: view class registration panicked: {}", panic_msg(&*p));
                None
            })
    })
    .as_ref()
    .copied()
}

unsafe fn register_yard_view_class() -> Option<&'static Class> {
    // Already registered (re-start) — reuse, never panic.
    if let Some(cls) = Class::get("OrbittyYardView") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyYardView", class!(NSView))?;
    decl.add_method(
        sel!(drawRect:),
        draw_rect as extern "C" fn(&Object, Sel, NSRect),
    );
    decl.add_method(
        sel!(mouseDown:),
        input::mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(mouseUp:),
        input::mouse_up as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(rightMouseDown:),
        right_mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(sel!(isFlipped), is_flipped as extern "C" fn(&Object, Sel) -> bool);
    decl.add_method(sel!(isOpaque), is_opaque as extern "C" fn(&Object, Sel) -> bool);
    decl.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, *mut Object) -> bool,
    );
    Some(decl.register())
}

extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> bool {
    true
}

extern "C" fn is_opaque(_this: &Object, _cmd: Sel) -> bool {
    false
}

extern "C" fn accepts_first_mouse(_this: &Object, _cmd: Sel, _event: *mut Object) -> bool {
    true
}

/// Target for the cat right-click menu. A single leaked instance lives
/// forever (NSMenuItem does not retain its target).
fn menu_target() -> Option<usize> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    TARGET
        .get_or_init(|| {
            catch_unwind(AssertUnwindSafe(|| unsafe {
                let cls = register_menu_target_class()?;
                let obj: *mut Object = msg_send![cls, new];
                if obj.is_null() {
                    None
                } else {
                    Some(obj as usize)
                }
            }))
            .unwrap_or_else(|p| {
                eprintln!("yard: menu target registration panicked: {}", panic_msg(&*p));
                None
            })
        })
        .as_ref()
        .copied()
}

unsafe fn register_menu_target_class() -> Option<&'static Class> {
    if let Some(cls) = Class::get("OrbittyYardMenuTarget") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyYardMenuTarget", class!(NSObject))?;
    decl.add_method(
        sel!(yardAssign:),
        menu_assign as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(yardClose:),
        menu_close as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(yardScoop:),
        menu_scoop as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(yardScoopAll:),
        menu_scoop_all as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

// panels

/// One fullscreen click-through panel. Same recipe as the habitat's
/// make_panel, but floating (Neko/Shimeji visibility: cats walk OVER
/// chrome, planet discs included) and sized to a whole display.
unsafe fn make_yard_panel(frame: NSRect) -> *mut Object {
    let panel: *mut Object = msg_send![class!(NSPanel), alloc];
    // NSWindowStyleMaskBorderless (0) | NSWindowStyleMaskNonactivatingPanel (1<<7)
    // Init at the REAL display frame — a 10×10 seed left a postage-stamp
    // backing store (meows, no cats) even after later setFrame calls.
    let panel: *mut Object = msg_send![panel,
        initWithContentRect: frame
        styleMask: 128usize
        backing: 2usize // NSBackingStoreBuffered
        defer: false];
    // NSFloatingWindowLevel — always-on-top so the clowder is visible on
    // the desktop; still below the menu bar / Dock / notification banners.
    let _: () = msg_send![panel, setLevel: 3i64];
    // canJoinAllSpaces (1) | stationary (1<<4) | fullScreenAuxiliary (1<<8)
    let _: () = msg_send![panel, setCollectionBehavior: 273usize];
    let _: () = msg_send![panel, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: false];
    // Click-through by default; the hop flips this off only while the
    // pointer is over a cat's hit-rect on THIS display.
    let _: () = msg_send![panel, setIgnoresMouseEvents: true];
    let _: () = msg_send![panel, setReleasedWhenClosed: false];
    let _: () = msg_send![panel, setHidesOnDeactivate: false];
    let _: () = msg_send![panel, setCanHide: false];
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: true];
    panel
}

unsafe fn prepare_yard_view(view: *mut Object) {
    let _: () = msg_send![view, setWantsLayer: true];
    let layer: *mut Object = msg_send![view, layer];
    if layer.is_null() {
        return;
    }
    let _: () = msg_send![layer, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let cg: *mut c_void = msg_send![clear, CGColor];
    if !cg.is_null() {
        let _: () = msg_send![layer, setBackgroundColor: cg];
    }
}

/// One panel per display, kept in step with `screens()` (display
/// unplug/rearrange while the clowder is out). Cats on a gone display
/// re-home to the primary, clamped into its walk bounds.
unsafe fn sync_panels(y: &mut Yard) {
    let all = screens();
    // Drop stale panels.
    let stale: Vec<String> = y
        .panels
        .iter()
        .filter(|p| !all.iter().any(|(id, _)| id == &p.screen_id))
        .map(|p| p.screen_id.clone())
        .collect();
    for id in &stale {
        if let Some(idx) = y.panels.iter().position(|p| &p.screen_id == id) {
            let p = y.panels.remove(idx);
            let panel = p.panel as *mut Object;
            let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
            let _: () = msg_send![panel, release];
        }
        if let Some((primary, _)) = all.first() {
            for cat in y.cats.iter_mut().filter(|c| &c.screen_id == id) {
                cat.screen_id = primary.clone();
            }
        }
    }
    // Create missing / refresh frames.
    for (idx, (id, frame)) in all.iter().enumerate() {
        let vis = {
            let arr: *mut Object = msg_send![class!(NSScreen), screens];
            let s: *mut Object = msg_send![arr, objectAtIndex: idx];
            if s.is_null() {
                *frame
            } else {
                visible_frame_for(s)
            }
        };
        let walk = walk_rect(frame, &vis);
        if let Some(p) = y.panels.iter_mut().find(|p| &p.screen_id == id) {
            if (p.frame.origin.x - frame.origin.x).abs() > 0.5
                || (p.frame.origin.y - frame.origin.y).abs() > 0.5
                || (p.frame.size.width - frame.size.width).abs() > 0.5
                || (p.frame.size.height - frame.size.height).abs() > 0.5
            {
                let panel = p.panel as *mut Object;
                let _: () = msg_send![panel, setFrame: *frame display: false];
                let view = p.view as *mut Object;
                let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, frame.size.width, frame.size.height)];
                p.frame = *frame;
                p.walk = walk;
            }
            continue;
        }
        let Some(cls) = yard_view_class() else {
            continue;
        };
        let view: *mut Object = msg_send![cls, alloc];
        let view: *mut Object = msg_send![view,
            initWithFrame: NSRect::new(0.0, 0.0, frame.size.width, frame.size.height)];
        if view.is_null() {
            continue;
        }
        let panel = make_yard_panel(*frame);
        if panel.is_null() {
            let _: () = msg_send![view, release];
            continue;
        }
        let _: () = msg_send![panel, setContentView: view];
        let _: () = msg_send![view, release];
        // NSViewWidthSizable (2) | NSViewHeightSizable (16).
        let _: () = msg_send![view, setAutoresizingMask: 18usize];
        let _: () = msg_send![view,
            setFrame: NSRect::new(0.0, 0.0, frame.size.width, frame.size.height)];
        prepare_yard_view(view);
        let _: () = msg_send![panel, orderFrontRegardless];
        let _: () = msg_send![view, setNeedsDisplay: true];
        // Do not displayIfNeeded here: drawRect locks the yard mutex
        // (non-reentrant) and this call still holds it. First blit is
        // flush_yard_views after the lock drops.
        eprintln!(
            "yard: overlay up on screen {id} {:.0}x{:.0} at {:.0},{:.0}",
            frame.size.width, frame.size.height, frame.origin.x, frame.origin.y
        );
        y.panels.push(YardPanel {
            screen_id: id.clone(),
            panel: panel as usize,
            view: view as usize,
            frame: *frame,
            walk,
            drawn: false,
        });
    }
    // Keep every cat inside its (possibly new) walk bounds.
    for cat in y.cats.iter_mut() {
        if let Some(p) = y.panels.iter().find(|p| p.screen_id == cat.screen_id) {
            clamp_cat(cat, &p.walk);
        }
    }
}

fn clamp_cat(cat: &mut Cat, walk: &NSRect) {
    cat.x = cat.x.clamp(walk.origin.x, (walk.max_x() - CAT).max(walk.origin.x));
    if cat.gait == Gait::Walk {
        cat.y = cat
            .y
            .clamp(walk.origin.y, (walk.max_y() - CAT).max(walk.origin.y));
        cat.floor_y = cat.floor_y.min((walk.max_y() - CAT).max(walk.origin.y));
    }
}

// spawn / motion

/// Hash-stable display pick: the clowder spreads across every display
/// instead of piling on the primary.
fn screen_for(panels: &[YardPanel], window_id: &str) -> Option<String> {
    if panels.is_empty() {
        return None;
    }
    let sum: usize = window_id.bytes().map(|b| b as usize).sum();
    Some(panels[sum % panels.len()].screen_id.clone())
}

/// Move due queued cats onto the desktop (staggered rain), then step
/// every cat's motion by dt.
fn motion_step(y: &mut Yard, dt: f64) {
    let now = Instant::now();
    // Rain the due ones.
    let mut i = 0;
    while i < y.queue.len() {
        if y.queue[i].at > now {
            i += 1;
            continue;
        }
        let pending = y.queue.remove(i);
        let Some(screen_id) = screen_for(&y.panels, &pending.window_id) else {
            continue;
        };
        let Some(panel) = y.panels.iter().find(|p| p.screen_id == screen_id) else {
            continue;
        };
        let walk = panel.walk;
        let pack = y.spawn_count % y.packs.len().max(1);
        y.spawn_count += 1;
        let x = rand_range(walk.origin.x, (walk.max_x() - CAT).max(walk.origin.x));
        let floor_y = (walk.max_y() - CAT - rand_range(0.0, 6.0)).max(walk.origin.y);
        eprintln!(
            "yard: cat for {} rains on screen {}",
            pending.window_id, screen_id
        );
        y.cats.push(Cat {
            window_id: pending.window_id,
            app_name: pending.app_name,
            title: pending.title,
            screen_id,
            x,
            y: -CAT - rand_range(0.0, 200.0), // above the visible frame
            vx: 0.0,
            vy: 0.0,
            gait: Gait::Rain,
            pack,
            anim: 0,
            facing_left: rand_unit() < 0.5,
            floor_y,
            move_until: None,
            pause_until: None,
        });
    }
    // Step motion. A cat held by hand (leftover: hits) is owned by
    // input::press_tick — anim still advances so the drag reads alive,
    // but gait/position are the hand's.
    let held_id = y.press.as_ref().map(|p| p.window_id.clone());
    for idx in 0..y.cats.len() {
        let Some(panel) = y
            .panels
            .iter()
            .find(|p| p.screen_id == y.cats[idx].screen_id)
        else {
            continue;
        };
        let walk = panel.walk;
        let cat = &mut y.cats[idx];
        cat.anim = cat.anim.wrapping_add(1);
        if held_id.as_deref() == Some(cat.window_id.as_str()) {
            continue;
        }
        match cat.gait {
            Gait::Rain => {
                cat.y += RAIN_VY * dt;
                if cat.y >= cat.floor_y {
                    cat.y = cat.floor_y;
                    cat.gait = Gait::Walk;
                    cat.pause_until = Some(now + Duration::from_secs_f64(rand_range(0.2, 0.9)));
                }
            }
            Gait::Walk => {
                if let Some(until) = cat.pause_until {
                    if now < until {
                        continue; // standing: idle frame, no drift
                    }
                    cat.pause_until = None;
                    // New amble: random heading, damped y, 1.5–4s leg.
                    let ang = rand_unit() * std::f64::consts::TAU;
                    let speed = rand_range(WALK_MIN, WALK_MAX);
                    cat.vx = ang.cos() * speed;
                    cat.vy = ang.sin() * speed * 0.5;
                    cat.move_until = Some(now + Duration::from_secs_f64(rand_range(1.5, 4.0)));
                }
                if cat
                    .move_until
                    .as_ref()
                    .is_some_and(|until| now >= *until)
                {
                    cat.move_until = None;
                    cat.vx = 0.0;
                    cat.vy = 0.0;
                    cat.pause_until = Some(now + Duration::from_secs_f64(rand_range(0.6, 2.2)));
                    continue;
                }
                cat.x += cat.vx * dt;
                cat.y += cat.vy * dt;
                if cat.vx.abs() > 0.5 {
                    cat.facing_left = cat.vx < 0.0;
                }
                // Bounce on the display (walk-bounds) edges.
                let lo_x = walk.origin.x;
                let hi_x = (walk.max_x() - CAT).max(lo_x);
                let lo_y = walk.origin.y;
                let hi_y = (walk.max_y() - CAT).max(lo_y);
                if cat.x <= lo_x {
                    cat.x = lo_x;
                    cat.vx = cat.vx.abs();
                } else if cat.x >= hi_x {
                    cat.x = hi_x;
                    cat.vx = -cat.vx.abs();
                }
                if cat.y <= lo_y {
                    cat.y = lo_y;
                    cat.vy = cat.vy.abs();
                } else if cat.y >= hi_y {
                    cat.y = hi_y;
                    cat.vy = -cat.vy.abs();
                }
            }
        }
    }
}

// the unmapped poll (worker thread; no AppKit here)

/// The inspector's Unclaimed set, native side: `list_windows` (already
/// drops Orbitty) minus claims whose project still exists —
/// the same formula RailPage and the warn pane compute in JS. None on a
/// read hiccup: a skipped beat never invents or kills a cat.
fn poll_unmapped(app: &AppHandle) -> Option<Vec<crate::windows::WindowInfo>> {
    let wins = crate::windows::list_windows().ok()?;
    let st = crate::state::load_state(app).ok()?;
    let live: HashSet<&str> = st
        .claims
        .iter()
        .filter(|c| st.projects.iter().any(|p| p.id == c.project_id))
        .map(|c| c.window_id.as_str())
        .collect();
    Some(
        wins.into_iter()
            .filter(|w| !live.contains(w.id.as_str()))
            .collect(),
    )
}

/// Diff the fresh unmapped set against the clowder. Pure data — safe on
/// the worker under the lock. New windows queue for staggered rain;
/// claimed/closed windows' cats poof; scooped ids stay suppressed.
fn apply_unmapped(y: &mut Yard, unmapped: Vec<crate::windows::WindowInfo>) {
    y.poll_done = true;
    let fresh: HashSet<&str> = unmapped.iter().map(|w| w.id.as_str()).collect();
    y.cats.retain(|c| fresh.contains(c.window_id.as_str()));
    y.queue.retain(|c| fresh.contains(c.window_id.as_str()));
    let have: HashSet<String> = y
        .cats
        .iter()
        .map(|c| c.window_id.clone())
        .chain(y.queue.iter().map(|c| c.window_id.clone()))
        .collect();
    let mut delay = Duration::ZERO;
    for w in unmapped {
        if have.contains(&w.id) || y.scooped.contains(&w.id) {
            continue;
        }
        delay += Duration::from_millis(
            (STAGGER_MIN_MS as f64 + rand_unit() * (STAGGER_MAX_MS - STAGGER_MIN_MS) as f64)
                as u64,
        );
        y.queue.push(PendingCat {
            window_id: w.id,
            app_name: w.app_name,
            title: w.title,
            at: Instant::now() + delay,
        });
    }
}

// worker + main hop

fn spawn_worker() {
    std::thread::spawn(|| {
        let mut n = 0u32;
        loop {
            if !active() {
                return;
            }
            // Off-main poll: AX + state.json, then a pure-data diff.
            if n % POLL_EVERY == 0 {
                if let Some(app) = app_handle() {
                    if let Some(unmapped) = poll_unmapped(app) {
                        let mut y = lock();
                        if y.on {
                            apply_unmapped(&mut y, unmapped);
                        }
                    }
                }
            }
            n = n.wrapping_add(1);
            // Post-and-return, coalesced — a stalled main drops a beat
            // instead of piling up late hops.
            if HOP_PENDING
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                crate::main_thread_async(|| {
                    let _ = catch_unwind(AssertUnwindSafe(hop));
                });
            }
            std::thread::sleep(Duration::from_millis(TICK_MS));
        }
    });
}

/// The main-thread turn: motion, spawn, panels, cursor ghost, meow, draw.
fn hop() {
    HOP_PENDING.store(false, Ordering::SeqCst);
    let mut stop_after = false;
    let mut cats_out = false;
    // Double-click verdict from the hop's release catch — the AX restore
    // runs after the lock drops, never under it.
    let mut focus_after: Option<String> = None;
    {
        let mut y = lock();
        if !y.on {
            return;
        }
        let now = Instant::now();
        let dt = y
            .last_hop
            .map(|t| now.duration_since(t).as_secs_f64().min(0.1))
            .unwrap_or(0.033);
        y.last_hop = Some(now);
        y.hop_count = y.hop_count.wrapping_add(1);
        if !y.packs.is_empty() {
            motion_step(&mut y, dt);
        }
        // The drag step / release catch (leftover: hits). First taker
        // wins against mouseUp, same verdict either way.
        focus_after = input::press_tick(&mut y);
        // Retire an expired app-name chip (draw also gates on the TTL;
        // this just keeps the state honest between blits).
        if y
            .caption
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= input::CAPTION_TTL)
        {
            y.caption = None;
        }
        // Display hotplug / rearrange, ~1s cadence (cheap on main).
        if y.hop_count % POLL_EVERY == 0 {
            unsafe { sync_panels(&mut y) };
        }
        cursor_ghost(&mut y);
        cats_out = !y.cats.is_empty();
        meow::tick(&mut y.meow, cats_out);
        // Redraw only panels with cats (plus one clearing frame after the
        // last cat leaves a display).
        let busy: HashSet<String> = y.cats.iter().map(|c| c.screen_id.clone()).collect();
        for p in y.panels.iter_mut() {
            let has = busy.contains(&p.screen_id);
            if has || p.drawn {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
                p.drawn = has;
            }
        }
        // Last cat gone (claimed, closed, or scooped) = overlay gone,
        // mode off. Armed only after the first real poll so the start-up
        // beat never insta-stops an empty clowder.
        if y.poll_done && y.cats.is_empty() && y.queue.is_empty() {
            stop_after = true;
        }
    }
    // A double-click the hop caught: open/focus/restore off-lock (the
    // same AX path mouseUp uses). The cat stays — a focused unclaimed
    // window is still unclaimed.
    if let Some(id) = focus_after {
        let _ = crate::windows::restore_window(&id);
    }
    if stop_after {
        stop_on_main();
        return;
    }
    // Paint AFTER the lock: drawRect takes this mutex. Under the lock a
    // sync blit deadlocks; without a post-lock blit, meows fire and the
    // glass stays empty. Do not orderFront here — that restack ate the
    // companion dock (2542c9d).
    if cats_out {
        unsafe { flush_yard_views() };
    }
}

/// Companion-ghost toggle: the fullscreen panel swallows clicks ONLY
/// while the pointer is over a cat on that display; empty overlay air
/// stays click-through.
fn cursor_ghost(y: &mut Yard) {
    unsafe {
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        // A held press keeps its panel's events on even if the pointer
        // slips off the sprite mid-press (leftover: hits) — otherwise
        // the drag/up dies en route and the cat sticks to the hand.
        let held: Option<String> = y.press.as_ref().and_then(|press| {
            y.cats
                .iter()
                .find(|c| c.window_id == press.window_id)
                .map(|c| c.screen_id.clone())
        });
        for p in y.panels.iter() {
            let local = NSPoint {
                x: mouse.x - p.frame.origin.x,
                y: p.frame.max_y() - mouse.y,
            };
            let over = held.as_deref() == Some(p.screen_id.as_str())
                || y.cats
                    .iter()
                    .any(|c| c.screen_id == p.screen_id && cat_rect(c).inset(-3.0).contains(local));
            let panel = p.panel as *mut Object;
            let _: () = msg_send![panel, setIgnoresMouseEvents: !over];
        }
    }
}

fn cat_rect(cat: &Cat) -> NSRect {
    NSRect::new(cat.x, cat.y, CAT, CAT)
}

/// The cat under a view-local point, topmost (last drawn) first. Returns
/// the window id (not an index) so a poll diff between lock epochs can
/// never mis-point a click at the wrong cat.
fn hit_cat(y: &Yard, view_ptr: usize, local: NSPoint) -> Option<String> {
    let panel = y.panels.iter().find(|p| p.view == view_ptr)?;
    y.cats
        .iter()
        .rev()
        .find(|c| c.screen_id == panel.screen_id && cat_rect(c).inset(-3.0).contains(local))
        .map(|c| c.window_id.clone())
}

// view callbacks (main thread)

extern "C" fn draw_rect(this: &Object, _cmd: Sel, _dirty: NSRect) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { draw_inner(this) }))
        .map_err(|p| eprintln!("yard: drawRect panicked: {}", panic_msg(&*p)));
}

unsafe fn draw_inner(this: &Object) {
    let y = lock();
    let view_ptr = this as *const Object as usize;
    let Some(panel) = y.panels.iter().find(|p| p.view == view_ptr) else {
        return;
    };
    let bounds: NSRect = msg_send![this, bounds];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![clear, set];
    let _: () = msg_send![class!(NSBezierPath), fillRect: bounds];
    let screen_id = panel.screen_id.clone();
    for (i, cat) in y.cats.iter().enumerate() {
        if cat.screen_id != screen_id {
            continue;
        }
        let Some(art) = y.packs.get(cat.pack) else {
            continue;
        };
        let moving = cat.gait == Gait::Walk && cat.pause_until.is_none();
        let img = if moving && !art.walk.is_empty() {
            // ~10Hz frame advance at the 33ms hop, desynced per cat.
            art.walk[(cat.anim as usize / 3 + i) % art.walk.len()]
        } else {
            art.idle.first().or_else(|| art.walk.first()).copied().unwrap_or(0)
        };
        if img == 0 {
            continue;
        }
        let rect = cat_rect(cat);
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        if let Some(ctx) = current_graphics_context() {
            // Pixel art: nearest-neighbor, never a blurred bitty.
            let _: () = msg_send![ctx, setImageInterpolation: 1usize];
        }
        if cat.facing_left {
            let cx = rect.origin.x + CAT / 2.0;
            let tf: *mut Object = msg_send![class!(NSAffineTransform), transform];
            let _: () = msg_send![tf, translateXBy: cx yBy: 0.0];
            let _: () = msg_send![tf, scaleXBy: -1.0 yBy: 1.0];
            let _: () = msg_send![tf, translateXBy: -cx yBy: 0.0];
            let _: () = msg_send![tf, concat];
        }
        let zero = NSRect::default();
        let _: () = msg_send![img as *mut Object,
            drawInRect: rect
            fromRect: zero
            operation: 2usize // NSCompositeSourceOver
            fraction: 1.0
            respectFlipped: true
            hints: std::ptr::null::<Object>()];
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
    }
    // Single-click chip (leftover: hits): the cat's app name above its
    // sprite for ~2s — the walker locate stroke's yard form. Drawn last
    // so it sits over the sprite; never a focus/restore.
    if let Some((id, at)) = &y.caption {
        if at.elapsed() < input::CAPTION_TTL {
            if let Some(cat) = y
                .cats
                .iter()
                .find(|c| &c.window_id == id && c.screen_id == screen_id)
            {
                input::draw_cat_chip(cat, &cat.app_name);
            }
        }
    }
}

unsafe fn current_graphics_context() -> Option<*mut Object> {
    let ctx: *mut Object = msg_send![class!(NSGraphicsContext), currentContext];
    if ctx.is_null() {
        None
    } else {
        Some(ctx)
    }
}

// mouseDown / mouseUp (click / drag / double-click on a cat) live in
// input.rs — the hit machine would pile onto this file (module law).

extern "C" fn right_mouse_down(this: &Object, _cmd: Sel, event: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let local: NSPoint =
            msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
        let id = {
            let y = lock();
            hit_cat(&y, this as *const Object as usize, local)
        };
        let Some(id) = id else {
            return;
        };
        cat_menu(this, event, &id);
    }))
    .map_err(|p| eprintln!("yard: rightMouseDown panicked: {}", panic_msg(&*p)));
}

// cat menu

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct YardAssignEvent {
    window_id: String,
    project_id: String,
}

unsafe fn add_menu_item(
    menu: *mut Object,
    title: &str,
    action: Sel,
    target: usize,
    represented: &str,
    enabled: bool,
) {
    let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let item: *mut Object = msg_send![item,
        initWithTitle: ns_string(title)
        action: action
        keyEquivalent: ns_string("")];
    if item.is_null() {
        return;
    }
    let _: () = msg_send![item, setTarget: target as *mut Object];
    let _: () = msg_send![item, setRepresentedObject: ns_string(represented)];
    if !enabled {
        let _: () = msg_send![item, setEnabled: false];
    }
    let _: () = msg_send![menu, addItem: item];
    let _: () = msg_send![item, release];
}

fn sender_represented(sender: *mut Object) -> Option<String> {
    unsafe {
        let obj: *mut Object = msg_send![sender, representedObject];
        if obj.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![obj, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Assign to planet (existing claim — the rail's claim() rewrites
/// projectId through the one persist door) · Close window (existing AX
/// close) · Scoop this cat (hide this one, window stays) · Scoop all.
unsafe fn cat_menu(this: &Object, event: *mut Object, window_id: &str) {
    let (window_id, app_name, title) = {
        let y = lock();
        let Some(cat) = y.cats.iter().find(|c| c.window_id == window_id) else {
            return;
        };
        (cat.window_id.clone(), cat.app_name.clone(), cat.title.clone())
    };
    // Planet names come from the same state.json the poll reads — the
    // native side never writes claims, only emits the pick.
    let projects = app_handle()
        .and_then(|app| crate::state::load_state(app).ok())
        .map(|s| s.projects)
        .unwrap_or_default();
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    // Which window this cat is (titles only, truncated).
    let mut head = if title.is_empty() {
        app_name.clone()
    } else {
        format!("{app_name} — {title}")
    };
    if head.chars().count() > 44 {
        head = head.chars().take(43).collect::<String>() + "…";
    }
    let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let item: *mut Object = msg_send![item,
        initWithTitle: ns_string(&head)
        action: Sel::from_ptr(std::ptr::null())
        keyEquivalent: ns_string("")];
    if !item.is_null() {
        let _: () = msg_send![item, setEnabled: false];
        let _: () = msg_send![menu, addItem: item];
        let _: () = msg_send![item, release];
    }
    if !projects.is_empty() {
        let sub: *mut Object = msg_send![class!(NSMenu), alloc];
        let sub: *mut Object = msg_send![sub, initWithTitle: ns_string("")];
        if !sub.is_null() {
            for p in &projects {
                add_menu_item(
                    sub,
                    &p.name,
                    sel!(yardAssign:),
                    target,
                    &format!("{window_id}\u{1f}{}", p.id),
                    true,
                );
            }
            let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
            let item: *mut Object = msg_send![item,
                initWithTitle: ns_string("Assign to planet")
                action: Sel::from_ptr(std::ptr::null())
                keyEquivalent: ns_string("")];
            if !item.is_null() {
                let _: () = msg_send![item, setSubmenu: sub];
                let _: () = msg_send![menu, addItem: item];
                let _: () = msg_send![item, release];
            }
            let _: () = msg_send![sub, release];
        }
    }
    add_menu_item(
        menu,
        "Close window",
        sel!(yardClose:),
        target,
        &window_id,
        true,
    );
    add_menu_item(
        menu,
        "Scoop this cat",
        sel!(yardScoop:),
        target,
        &window_id,
        true,
    );
    let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep];
    add_menu_item(menu, "Scoop all", sel!(yardScoopAll:), target, "", true);
    let view = this as *const Object as *mut Object;
    let _: () = msg_send![class!(NSMenu), popUpContextMenu: menu withEvent: event forView: view];
    let _: () = msg_send![menu, release];
}

/// Remove one cat by window id (menu acts; the poll agrees within ~1s).
/// Returns true when the clowder just emptied (caller stops the mode).
fn remove_cat(window_id: &str) -> bool {
    let mut y = lock();
    y.cats.retain(|c| c.window_id != window_id);
    y.queue.retain(|c| c.window_id != window_id);
    let empty = y.cats.is_empty() && y.queue.is_empty();
    for p in y.panels.iter() {
        unsafe {
            let view = p.view as *mut Object;
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
    empty && y.poll_done
}

extern "C" fn menu_assign(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(raw) = sender_represented(sender) else {
            return;
        };
        let Some((window_id, project_id)) = raw.split_once('\u{1f}') else {
            return;
        };
        if project_id.is_empty() {
            return;
        }
        // The rail's claim() is the one claim writer — the yard emits,
        // never persists. The cat leaves now; the next poll agrees.
        if let Some(app) = app_handle() {
            let _ = app.emit_to(
                "main",
                "yard:assign",
                YardAssignEvent {
                    window_id: window_id.to_string(),
                    project_id: project_id.to_string(),
                },
            );
        }
        if remove_cat(window_id) {
            stop_on_main();
        }
    }))
    .map_err(|p| eprintln!("yard: assign panicked: {}", panic_msg(&*p)));
}

extern "C" fn menu_close(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(window_id) = sender_represented(sender) else {
            return;
        };
        // AX close-button press (never a pid kill). A refusal leaves the
        // window open and the poll keeps the cat — honest either way.
        let _ = crate::windows::close_window(&window_id);
    }))
    .map_err(|p| eprintln!("yard: close panicked: {}", panic_msg(&*p)));
}

extern "C" fn menu_scoop(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(window_id) = sender_represented(sender) else {
            return;
        };
        {
            let mut y = lock();
            y.scooped.insert(window_id.clone());
        }
        if remove_cat(&window_id) {
            stop_on_main();
        }
    }))
    .map_err(|p| eprintln!("yard: scoop panicked: {}", panic_msg(&*p)));
}

extern "C" fn menu_scoop_all(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| stop_on_main()))
        .map_err(|p| eprintln!("yard: scoop-all panicked: {}", panic_msg(&*p)));
}

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
    y.meow = meow::Meow::armed();
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
unsafe fn flush_yard_views() {
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
fn stop_on_main() {
    let mut y = lock();
    if !y.on {
        return;
    }
    y.on = false;
    meow::kill(&mut y.meow);
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
