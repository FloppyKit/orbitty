//! Cat hits: click names the app, hold drags the sprite, double opens.
//! First taker wins. Sub-slop release is a click, never a drop.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use super::{
    cat_rect, clamp_cat, hit_cat, lock, ns_string, panic_msg, rand_range, Cat, CatPress, Gait,
    NSPoint, NSRect, Yard, CAT,
};

/// App-name chip lifetime — the habitat caption pill's ~2s.
pub(crate) const CAPTION_TTL: Duration = Duration::from_secs(2);

/// Press-release under this distance is a click, not a drag — the
/// walker slop, moved as-is.
const DRAG_SLOP: f64 = 4.0;

/// Top-fifth drop band: this fraction of the cat's OWN screen height
/// (view-local flipped y; 0 = that screen's top edge).
const RAIN_DROP_TOP: f64 = 0.2;

// mouseDown: arm the press

pub(crate) extern "C" fn mouse_down(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { mouse_down_inner(this, event) })) {
        eprintln!("yard: mouseDown panicked: {}", panic_msg(&*p));
    }
}

unsafe fn mouse_down_inner(this: &Object, event: *mut Object) {
    let loc: NSPoint = msg_send![event, locationInWindow];
    let local: NSPoint =
        msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
    let clicks: usize = msg_send![event, clickCount];
    let mut y = lock();
    let Some(id) = hit_cat(&y, this as *const Object as usize, local) else {
        return;
    };
    let Some(cat) = y.cats.iter().find(|c| c.window_id == id) else {
        return;
    };
    // Click-or-drag is undecided until slop or up. Grab offsets keep the
    // sprite from snapping its center to the pointer on the first slide.
    let grab_dx = local.x - cat.x;
    let grab_dy = local.y - cat.y;
    // Slop anchor from the EVENT's own location (view-local flipped →
    // global bottom-left), never a fresh mouseLocation: a down that
    // dispatches late (busy hop) would otherwise anchor after the
    // drag's first pixels and the press would never cross the slop.
    let Some(panel) = y.panels.iter().find(|p| p.screen_id == cat.screen_id) else {
        return;
    };
    let start = NSPoint {
        x: panel.frame.origin.x + local.x,
        y: panel.frame.max_y() - local.y,
    };
    y.press = Some(CatPress {
        window_id: id,
        start,
        moved: 0.0,
        grab_dx,
        grab_dy,
        clicks,
    });
}

// mouseUp: click, double-click, or the drop

pub(crate) extern "C" fn mouse_up(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { mouse_up_inner(this, event) })) {
        eprintln!("yard: mouseUp panicked: {}", panic_msg(&*p));
    }
}

unsafe fn mouse_up_inner(this: &Object, event: *mut Object) {
    let loc: NSPoint = msg_send![event, locationInWindow];
    let local: NSPoint =
        msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
    let clicks: usize = msg_send![event, clickCount];
    let view_ptr = this as *const Object as usize;
    let mut focus: Option<String> = None;
    {
        let mut y = lock();
        let Some(mut press) = y.press.take() else {
            // press_tick already ran this release's verdict — first
            // taker won; there is nothing twice.
            return;
        };
        if press.moved >= DRAG_SLOP {
            // A real drag, not a click: drop where the pointer let go.
            release_press(&mut y, &press);
            return;
        }
        // Click: the up must still be on the SAME cat (the walker's
        // still-inside rule) — a slip off the sprite is nothing at all.
        let Some(id) = hit_cat(&y, view_ptr, local) else {
            return;
        };
        if id != press.window_id {
            return;
        }
        // The up event's own clickCount is the freshest count.
        if clicks > 0 {
            press.clicks = clicks;
        }
        focus = click_verdict(&mut y, &press);
    }
    if let Some(id) = focus {
        // The cat stays until the window is claimed/closed/scooped — a
        // focused unclaimed window is still unclaimed.
        let _ = crate::windows::restore_window(&id);
    }
}

/// The click act, shared by mouseUp and the hop's release catch (first
/// taker wins, same verdict either way). Single click says the app name
/// only — never a focus/restore/open. Double-click is the AX open; the
/// window id comes BACK so the caller restores after the lock drops.
fn click_verdict(y: &mut Yard, press: &CatPress) -> Option<String> {
    if press.clicks >= 2 {
        return Some(press.window_id.clone());
    }
    arm_caption(y, &press.window_id);
    None
}

/// Single-click feedback: the ~2s app-name chip above the sprite, then
/// nothing else. The chip text is the cat's app_name — the window's
/// app from the same list_windows the rail reads, never the title.
fn arm_caption(y: &mut Yard, window_id: &str) {
    if let Some(cat) = y.cats.iter().find(|c| c.window_id == window_id) {
        if let Some(p) = y.panels.iter().find(|p| p.screen_id == cat.screen_id) {
            unsafe {
                let view = p.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
    y.caption = Some((window_id.to_string(), Instant::now()));
}

// the hop's drag step

/// Runs from the 33ms hop (main thread): slides a dragged cat with the
/// pointer once past click slop, and catches a release whose mouseUp
/// hasn't dispatched yet. First taker wins against mouseUp — but the
/// verdict is SHARED: a real drag (≥4px) drops here; a sub-slop release
/// is a CLICK (chip, or focus when the down's clickCount was ≥2), never
/// a silent drop. Returns the double-click focus id so the AX restore
/// runs after the yard lock drops, same as mouseUp's own path.
pub(crate) fn press_tick(y: &mut Yard) -> Option<String> {
    if y.press.is_none() {
        return None;
    }
    unsafe {
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        if buttons & 1 == 0 {
            let press = y.press.take()?;
            if press.moved >= DRAG_SLOP {
                // A real drag: drop at the pointer's last spot.
                release_press(y, &press);
                return None;
            }
            // A click whose up event is still en route — the cat is
            // frozen while held, so a sub-slop release is still on it.
            return click_verdict(y, &press);
        }
        let (id, grab_dx, grab_dy) = {
            let Some(press) = y.press.as_mut() else {
                return None;
            };
            press.moved = press
                .moved
                .max((mouse.x - press.start.x).hypot(mouse.y - press.start.y));
            if press.moved < DRAG_SLOP {
                return None; // click slop — the sprite doesn't slide yet
            }
            (press.window_id.clone(), press.grab_dx, press.grab_dy)
        };
        let Some(idx) = y.cats.iter().position(|c| c.window_id == id) else {
            // Claimed/closed/scooped mid-drag — the press dies with it.
            y.press = None;
            return None;
        };
        let Some(panel) = y
            .panels
            .iter()
            .find(|p| p.screen_id == y.cats[idx].screen_id)
        else {
            return None;
        };
        let walk = panel.walk;
        let local = NSPoint {
            x: mouse.x - panel.frame.origin.x,
            y: panel.frame.max_y() - mouse.y,
        };
        let cat = &mut y.cats[idx];
        // A grabbed raindrop becomes a walker for the hold — uniform
        // clamp, and a clean amble/rain verdict on release.
        cat.gait = Gait::Walk;
        cat.vx = 0.0;
        cat.vy = 0.0;
        cat.move_until = None;
        cat.pause_until = None;
        cat.x = local.x - grab_dx;
        cat.y = local.y - grab_dy;
        clamp_cat(cat, &walk);
    }
    None
}

/// The drop verdict. Top fifth of the cat's OWN screen (the panel the
/// cat is on — never the desktop union): rain back down — the existing
/// rain gait falls VISIBLY from the drop point (never a teleport to the
/// floor), lands at the bottom, then ambles. Anywhere else: the cat
/// stays where it was let go and resumes its stroll from there.
fn release_press(y: &mut Yard, press: &CatPress) {
    let Some(idx) = y.cats.iter().position(|c| c.window_id == press.window_id) else {
        return;
    };
    let Some(panel) = y
        .panels
        .iter()
        .find(|p| p.screen_id == y.cats[idx].screen_id)
    else {
        return;
    };
    let walk = panel.walk;
    let top_band = panel.frame.size.height * RAIN_DROP_TOP;
    let mouse: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
    // Flipped view-local y of the pointer on THAT panel: 0 = screen top.
    let local_y = panel.frame.max_y() - mouse.y;
    let cat = &mut y.cats[idx];
    cat.vx = 0.0;
    cat.vy = 0.0;
    cat.move_until = None;
    if local_y <= top_band {
        // Rain-from-drop: y stays at the drop point and the Rain gait
        // falls from here — the visible fall is the whole point.
        cat.gait = Gait::Rain;
        cat.pause_until = None;
        cat.floor_y = (walk.max_y() - CAT - rand_range(0.0, 6.0)).max(walk.origin.y);
    } else {
        cat.gait = Gait::Walk;
        cat.pause_until = Some(Instant::now() + Duration::from_secs_f64(rand_range(0.2, 0.9)));
        clamp_cat(cat, &walk);
    }
}

// the app-name chip

/// Single-click feedback: dark rounded backer + white 11pt app name,
/// centered above the cat (flips below when the cat hugs the top edge).
/// The habitat caption pill's yard form — same recipe, cat-anchored.
pub(crate) unsafe fn draw_cat_chip(cat: &Cat, text: &str) {
    if text.is_empty() {
        return;
    }
    let ns_text = ns_string(text);
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let keys = [ns_string("NSFont"), ns_string("NSColor")];
    let vals = [font, white];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 2usize];
    if attrs.is_null() {
        return;
    }
    let size: super::CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let pad_x = 8.0;
    let pad_y = 4.0;
    let pill_w = size.width + pad_x * 2.0;
    let pill_h = size.height + pad_y * 2.0;
    let rect = cat_rect(cat);
    let mut pill_y = rect.origin.y - pill_h - 4.0;
    if pill_y < 2.0 {
        pill_y = rect.origin.y + CAT + 4.0;
    }
    let pill = NSRect::new(
        rect.origin.x + CAT / 2.0 - pill_w / 2.0,
        pill_y,
        pill_w,
        pill_h,
    );
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let bg: *mut Object = msg_send![black, colorWithAlphaComponent: 0.72f64];
    let _: () = msg_send![bg, set];
    let path: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithRoundedRect: pill
        xRadius: pill_h / 2.0
        yRadius: pill_h / 2.0];
    let _: () = msg_send![path, fill];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: pill.origin.x + pad_x,
            y: pill.origin.y + pad_y,
        }
        withAttributes: attrs];
}
