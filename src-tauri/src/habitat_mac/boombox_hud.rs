//! Boombox HUD (GO-ORBITTY-BOOMBOX leftover: click) — the sprite's
//! left-click surface. A small floating panel on the boombox: vertical
//! volume slider (the remembered multiplier — Quiet/Low/Med/High grew
//! up into it), a speaker hard mute, and the `new track` pill floating
//! just underneath (the Choose music door, same picker as 6c6e9ae).
//! Drawn in the planet view, hit-tested in mouseDown — no NSMenu on
//! the sprite anywhere.
//!
//! Dismiss law: click empty glass / click away / Esc. The planets are
//! non-activating panels, so Esc and off-app clicks ride global/local
//! NSEvent monitors installed only while the HUD is open (yard block
//! pattern). A down on our own planet panel is left to mouse_down_inner
//! — the local monitor skips it or the click-toggle would double-fire.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::audio;
use super::{
    lock, ns_color, ns_string, set_fill, world_entry, CGSize, DecorKind, Habitat, NSPoint,
    NSRect, Planet, EXPANDED_RADIUS, VIEW_PADDING,
};

// geometry

/// HUD geometry, all view-local flipped (y down). Derived from the
/// boombox decor frame at draw/hit time, so layout_decor stays the one
/// owner of the sprite frame and the HUD never holds stale rects.
pub(crate) struct HudRects {
    /// Backer panel (slider + speaker).
    pub panel: NSRect,
    /// Slider track; the hit column is this, fattened.
    pub slider: NSRect,
    pub speaker: NSRect,
    /// "new track" pill floating just underneath the panel.
    pub pill: NSRect,
}

const TRACK_W: f64 = 8.0;
const TRACK_H: f64 = 44.0;
const SPEAKER: f64 = 20.0;
const PAD: f64 = 6.0;
const GAP: f64 = 4.0; // track ↔ speaker
const PANEL_W: f64 = 32.0;
const PILL_W: f64 = 64.0;
const PILL_H: f64 = 18.0;
const PILL_GAP: f64 = 4.0; // panel ↔ pill
const EDGE: f64 = 3.0; // min distance from the view edge

fn panel_h() -> f64 {
    PAD + TRACK_H + GAP + SPEAKER + PAD
}

/// Anchor cascade: above the sprite, below it, east, west — the first
/// that keeps the whole assembly inside the view. Overlapping the disc
/// is fine (the HUD is floating glass); clipping at the view bounds is
/// not. Last resort: above, clamped into the view.
pub(crate) fn rects(planet: &Planet) -> Option<HudRects> {
    let frame = planet
        .decor_frames
        .iter()
        .find(|(d, _)| d.kind == DecorKind::Boombox)?
        .1;
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    // Zoomed-down planets shrink the panel (the pill keeps its 11pt).
    let s = (planet.radius / EXPANDED_RADIUS).clamp(0.55, 1.0);
    let pw = PANEL_W * s;
    let ph = panel_h() * s;
    let asm_h = ph + PILL_GAP + PILL_H;
    let cx = frame.mid_x();
    let cy = frame.mid_y();

    let lay = |panel_x: f64, panel_y: f64| {
        let pill_x = (cx - PILL_W / 2.0).clamp(EDGE, (side - PILL_W - EDGE).max(EDGE));
        HudRects {
            panel: NSRect::new(panel_x, panel_y, pw, ph),
            slider: NSRect::new(
                panel_x + (pw - TRACK_W * s) / 2.0,
                panel_y + PAD * s,
                TRACK_W * s,
                TRACK_H * s,
            ),
            speaker: NSRect::new(
                panel_x + (pw - SPEAKER * s) / 2.0,
                panel_y + ph - PAD * s - SPEAKER * s,
                SPEAKER * s,
                SPEAKER * s,
            ),
            pill: NSRect::new(pill_x, panel_y + ph + PILL_GAP, PILL_W, PILL_H),
        }
    };

    let px = (cx - pw / 2.0).clamp(EDGE, (side - pw - EDGE).max(EDGE));
    // Above: pill sits just over the sprite's top edge.
    let above_y = frame.origin.y - EDGE - PILL_H - PILL_GAP - ph;
    if above_y >= EDGE {
        return Some(lay(px, above_y));
    }
    // Below: panel just under the sprite, pill underneath the panel.
    let below_y = frame.max_y() + EDGE;
    if below_y + asm_h <= side - EDGE {
        return Some(lay(px, below_y));
    }
    // Beside: assembly centered on the sprite's y, pill under the panel.
    let side_y = (cy - asm_h / 2.0).clamp(EDGE, (side - asm_h - EDGE).max(EDGE));
    let east_x = frame.max_x() + GAP;
    if east_x + pw <= side - EDGE {
        return Some(lay(east_x, side_y));
    }
    let west_x = frame.origin.x - GAP - pw;
    if west_x >= EDGE {
        return Some(lay(west_x, side_y));
    }
    Some(lay(px, above_y.clamp(EDGE, (side - asm_h - EDGE).max(EDGE))))
}

/// Track y → remembered multiplier. Top is High (1), bottom is Off (0).
pub(crate) fn slider_value(r: &HudRects, local: NSPoint) -> f32 {
    let t = 1.0 - (local.y - r.slider.origin.y) / r.slider.size.height;
    t.clamp(0.0, 1.0) as f32
}

pub(crate) enum HudHit {
    /// Press on the slider column; carries the value at the press point
    /// (a real control: the thumb jumps to the press, then drags).
    Slider(f32),
    Speaker,
    Pill,
    /// Backer glass — swallows the down (never a panel-drag start).
    Panel,
}

pub(crate) fn hit(planet: &Planet, local: NSPoint) -> Option<HudHit> {
    let r = rects(planet)?;
    if r.pill.inset(-2.0).contains(local) {
        return Some(HudHit::Pill);
    }
    if r.speaker.inset(-3.0).contains(local) {
        return Some(HudHit::Speaker);
    }
    if r.slider.inset(-6.0).contains(local) {
        return Some(HudHit::Slider(slider_value(&r, local)));
    }
    if r.panel.contains(local) {
        return Some(HudHit::Panel);
    }
    None
}

/// While the HUD is open its glass is interactive even off the disc
/// edge — without this the cursor watcher clicks the panel through and
/// the controls never see a down.
pub(crate) fn contains_hud(planet: &Planet, local: NSPoint) -> bool {
    let Some(r) = rects(planet) else {
        return false;
    };
    r.panel.inset(-2.0).contains(local) || r.pill.inset(-2.0).contains(local)
}

// open / close

pub(crate) fn toggle(h: &mut Habitat, project_id: &str) {
    if h.boombox_hud.as_deref() == Some(project_id) {
        close(h);
    } else {
        open(h, project_id);
    }
}

pub(crate) fn open(h: &mut Habitat, project_id: &str) {
    h.boombox_hud = Some(project_id.to_string());
    install_monitors();
    if let Some(planet) = h.planets.get_mut(project_id) {
        let view = planet.view as *mut Object;
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

/// Idempotent — every unhandled down anywhere calls this.
pub(crate) fn close(h: &mut Habitat) {
    let Some(pid) = h.boombox_hud.take() else {
        return;
    };
    remove_monitors();
    if let Some(planet) = h.planets.get_mut(&pid) {
        let view = planet.view as *mut Object;
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

// controls

/// Slider write: the remembered multiplier (persisted through
/// worlds_dirty), then the live player fades in place — same door as
/// the old menu steps. 0 is the Off end: the player stops (never a
/// fake silent box); cursor_tick's fader door restarts it when the
/// slider comes back up. A muted box keeps its player at 0. No file
/// yet: the slider remembers, nothing plays.
pub(crate) fn set_volume(h: &mut Habitat, project_id: &str, value: f32) {
    let value = value.clamp(0.0, 1.0);
    let (on, music, muted) = {
        let world = world_entry(h, project_id);
        world.boombox_volume = value;
        (world.boombox, world.music.clone(), world.boombox_muted)
    };
    h.worlds_dirty = true;
    let reduce = h.reduce_motion;
    if let Some(planet) = h.planets.get_mut(project_id) {
        if on && !music.is_empty() {
            if muted {
                audio::fade_audio(planet, 0.0);
            } else {
                let live = audio::live_volume(value, planet.radius, reduce);
                audio::start_audio(planet, &music, live);
            }
        }
        let view = planet.view as *mut Object;
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

/// Speaker: hard mute — the player is KEPT, faded to 0 (not the Off
/// end's stop/release), so unmute continues the track mid-file. Never
/// the zoom-quiet fade: live level stays boomboxVolume × zoomGain.
pub(crate) fn toggle_mute(h: &mut Habitat, project_id: &str) {
    let (on, music, remembered, muted) = {
        let world = world_entry(h, project_id);
        world.boombox_muted = !world.boombox_muted;
        (
            world.boombox,
            world.music.clone(),
            world.boombox_volume,
            world.boombox_muted,
        )
    };
    h.worlds_dirty = true;
    let reduce = h.reduce_motion;
    if let Some(planet) = h.planets.get_mut(project_id) {
        if on && !music.is_empty() {
            if muted {
                audio::fade_audio(planet, 0.0);
            } else {
                let live = audio::live_volume(remembered, planet.radius, reduce);
                if planet.audio.is_some() {
                    audio::fade_audio(planet, live);
                } else if live > 0.0 {
                    audio::start_audio(planet, &music, live);
                }
            }
        }
        let view = planet.view as *mut Object;
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

// draw

/// Everything drawRect needs, copied under the lock (DrawData rides
/// lock-free after).
pub(crate) struct HudDraw {
    pub rects: HudRects,
    pub value: f32,
    pub muted: bool,
}

pub(crate) unsafe fn draw(hud: &HudDraw) {
    let r = &hud.rects;
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];

    // Backer + pill glass — the click caption's dark pill language.
    let bg: *mut Object = msg_send![black, colorWithAlphaComponent: 0.72f64];
    set_fill(bg);
    let backer: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithRoundedRect: r.panel
        xRadius: 8.0f64
        yRadius: 8.0f64];
    let _: () = msg_send![backer, fill];

    // Track.
    let track_ink: *mut Object = msg_send![white, colorWithAlphaComponent: 0.25f64];
    set_fill(track_ink);
    let track: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithRoundedRect: r.slider
        xRadius: r.slider.size.width / 2.0
        yRadius: r.slider.size.width / 2.0];
    let _: () = msg_send![track, fill];

    // Fill rides up from the bottom (Off) to the thumb.
    let v = (hud.value as f64).clamp(0.0, 1.0);
    if v > 0.005 {
        let fill_h = r.slider.size.height * v;
        let fill_rect = NSRect::new(
            r.slider.origin.x,
            r.slider.max_y() - fill_h,
            r.slider.size.width,
            fill_h,
        );
        set_fill(ns_color(0.38, 0.72, 1.0, 0.95));
        let fill_path: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRoundedRect: fill_rect
            xRadius: r.slider.size.width / 2.0
            yRadius: r.slider.size.width / 2.0];
        let _: () = msg_send![fill_path, fill];
    }

    // Thumb.
    let thumb_d = r.slider.size.width + 6.0;
    let thumb_y = r.slider.max_y() - r.slider.size.height * v;
    let thumb_ink: *mut Object = msg_send![white, colorWithAlphaComponent: 0.95f64];
    set_fill(thumb_ink);
    let thumb: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithOvalInRect: NSRect::new(
            r.slider.mid_x() - thumb_d / 2.0,
            thumb_y - thumb_d / 2.0,
            thumb_d,
            thumb_d,
        )];
    let _: () = msg_send![thumb, fill];

    draw_speaker(r.speaker, hud.muted, white);

    // `new track` pill, floating just underneath.
    set_fill(bg);
    let pill: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithRoundedRect: r.pill
        xRadius: r.pill.size.height / 2.0
        yRadius: r.pill.size.height / 2.0];
    let _: () = msg_send![pill, fill];
    let text = ns_string("new track");
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
    let keys = [ns_string("NSFont"), ns_string("NSColor")];
    let vals = [font, white];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 2usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![text, sizeWithAttributes: attrs];
    let _: () = msg_send![text,
        drawAtPoint: NSPoint {
            x: r.pill.mid_x() - size.width / 2.0,
            y: r.pill.mid_y() - size.height / 2.0,
        }
        withAttributes: attrs];
}

/// Speaker glyph: base + cone + two wave arcs; muted swaps the waves
/// for an × and dims. Vector-drawn — no text glyph, no asset.
unsafe fn draw_speaker(rect: NSRect, muted: bool, white: *mut Object) {
    let cx = rect.mid_x();
    let cy = rect.mid_y();
    let u = rect.size.width / 22.0;
    let ink: *mut Object =
        msg_send![white, colorWithAlphaComponent: if muted { 0.45f64 } else { 0.95f64 }];
    set_fill(ink);
    let _: () = msg_send![class!(NSBezierPath),
        fillRect: NSRect::new(cx - 9.0 * u, cy - 3.5 * u, 4.2 * u, 7.0 * u)];
    let cone: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
    let _: () = msg_send![cone, moveToPoint: NSPoint { x: cx - 4.8 * u, y: cy - 4.0 * u }];
    let _: () = msg_send![cone, lineToPoint: NSPoint { x: cx + 0.5 * u, y: cy - 8.5 * u }];
    let _: () = msg_send![cone, lineToPoint: NSPoint { x: cx + 0.5 * u, y: cy + 8.5 * u }];
    let _: () = msg_send![cone, lineToPoint: NSPoint { x: cx - 4.8 * u, y: cy + 4.0 * u }];
    let _: () = msg_send![cone, closePath];
    let _: () = msg_send![cone, fill];
    let _: () = msg_send![ink, setStroke];
    if muted {
        let x_path: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
        let _: () = msg_send![x_path, moveToPoint: NSPoint { x: cx + 4.0 * u, y: cy - 5.0 * u }];
        let _: () = msg_send![x_path, lineToPoint: NSPoint { x: cx + 10.5 * u, y: cy + 5.0 * u }];
        let _: () = msg_send![x_path, moveToPoint: NSPoint { x: cx + 10.5 * u, y: cy - 5.0 * u }];
        let _: () = msg_send![x_path, lineToPoint: NSPoint { x: cx + 4.0 * u, y: cy + 5.0 * u }];
        let _: () = msg_send![x_path, setLineWidth: 1.8 * u];
        let _: () = msg_send![x_path, stroke];
    } else {
        for (radius, sweep) in [(4.5 * u, 50.0f64), (8.0 * u, 42.0f64)] {
            let arc: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
            let _: () = msg_send![arc,
                appendBezierPathWithArcWithCenter: NSPoint { x: cx + 1.0 * u, y: cy }
                radius: radius
                startAngle: -sweep
                endAngle: sweep];
            let _: () = msg_send![arc, setLineWidth: 1.5 * u];
            let _: () = msg_send![arc, stroke];
        }
    }
}

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
        close(&mut h);
    });
}

fn install_monitors() {
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

fn remove_monitors() {
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
