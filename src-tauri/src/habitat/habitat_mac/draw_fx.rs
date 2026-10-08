//! Blit/saturn/caption/pixel helpers for draw_world (DIET-HABITAT).
#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::DrawData;
use super::super::super::{
    ns_color, ns_string, set_fill, CGSize, NSPoint, NSRect,
};

pub(crate) unsafe fn draw_planet_locate(
    d: &DrawData,
    center: NSPoint,
    r: f64,
    ry: f64,
    rect: NSRect,
) {
    let Some(t) = d.locate_t else { return };
    let fade = (1.0 - t).clamp(0.0, 1.0);
    if fade <= 0.02 {
        return;
    }
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let base = d.color;
    if d.is_marble {
        let tilt = -42.0f64;
        let rw = r * 2.70;
        let rh = ry * 0.90;
        let ring_rect = NSRect::new(center.x - rw / 2.0, center.y - rh / 2.0, rw, rh);
        let pad = r * 6.0;
        let spin = t * 360.0;
        // Ears: the ring outside the disc (front and back tips).
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        clip_outside_oval(rect, pad);
        concat_tilt(center, tilt);
        stroke_saturn_ellipse(ring_rect, fade, base);
        stroke_saturn_highlight(center, ring_rect, spin, fade);
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        // Front crossing: inside the disc, only the near long edge
        // (lower half of the unrotated ellipse, then tilted).
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let disc: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
        let _: () = msg_send![disc, addClip];
        concat_tilt(center, tilt);
        let front: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRect: NSRect::new(center.x - pad, center.y, pad * 2.0, pad)];
        let _: () = msg_send![front, addClip];
        stroke_saturn_ellipse(ring_rect, fade, base);
        stroke_saturn_highlight(center, ring_rect, spin, fade);
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
    } else {
        let glow: *mut Object = msg_send![white, colorWithAlphaComponent: 0.5f64 * fade];
        let _: () = msg_send![glow, setStroke];
        let outer: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(-3.5)];
        let _: () = msg_send![outer, setLineWidth: 5.5f64];
        let _: () = msg_send![outer, stroke];
        let ink: *mut Object =
            msg_send![white, colorWithAlphaComponent: 0.95f64 * fade];
        let _: () = msg_send![ink, setStroke];
        let rim: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(-0.5)];
        let _: () = msg_send![rim, setLineWidth: 2.6f64];
        let _: () = msg_send![rim, stroke];
        let tint = ns_color(base.0, base.1, base.2, 0.7 * fade);
        let _: () = msg_send![tint, setStroke];
        let color_ring: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
        let _: () = msg_send![color_ring, setLineWidth: 1.8f64];
        let _: () = msg_send![color_ring, stroke];
        draw_center_flash(center, r, &d.name, fade);
    }
}

pub(crate) unsafe fn concat_tilt(center: NSPoint, degrees: f64) {
    let tf: *mut Object = msg_send![class!(NSAffineTransform), transform];
    let _: () = msg_send![tf, translateXBy: center.x yBy: center.y];
    let _: () = msg_send![tf, rotateByDegrees: degrees];
    let _: () = msg_send![tf, translateXBy: -center.x yBy: -center.y];
    let _: () = msg_send![tf, concat];
}

pub(crate) unsafe fn clip_outside_oval(rect: NSRect, pad: f64) {
    let hole: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
    let c = NSPoint {
        x: rect.mid_x(),
        y: rect.mid_y(),
    };
    let _: () = msg_send![hole,
        appendBezierPathWithRect: NSRect::new(c.x - pad, c.y - pad, pad * 2.0, pad * 2.0)];
    let disc: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![hole, appendBezierPath: disc];
    let _: () = msg_send![hole, setWindingRule: 1usize];
    let _: () = msg_send![hole, addClip];
}

pub(crate) unsafe fn stroke_saturn_ellipse(
    ring_rect: NSRect,
    fade: f64,
    base: (f64, f64, f64),
) {
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let glow: *mut Object = msg_send![white, colorWithAlphaComponent: 0.32f64 * fade];
    let _: () = msg_send![glow, setStroke];
    let outer: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![outer, setLineWidth: 5.0f64];
    let _: () = msg_send![outer, stroke];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: 0.78f64 * fade];
    let _: () = msg_send![ink, setStroke];
    let inner: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![inner, setLineWidth: 2.0f64];
    let _: () = msg_send![inner, stroke];
    let tint = ns_color(base.0, base.1, base.2, 0.5 * fade);
    let _: () = msg_send![tint, setStroke];
    let color_ring: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect.inset(1.5)];
    let _: () = msg_send![color_ring, setLineWidth: 1.4f64];
    let _: () = msg_send![color_ring, stroke];
}

/// Bright chunk traveling around the ellipse — the ring spinning in its
/// plane, not the oval tumbling around the planet.
pub(crate) unsafe fn stroke_saturn_highlight(
    center: NSPoint,
    ring_rect: NSRect,
    spin_deg: f64,
    fade: f64,
) {
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let pie: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
    let _: () = msg_send![pie, moveToPoint: center];
    let reach = ring_rect.size.width.max(ring_rect.size.height);
    let _: () = msg_send![pie,
        appendBezierPathWithArcWithCenter: center
        radius: reach
        startAngle: spin_deg
        endAngle: spin_deg + 70.0
        clockwise: 0u8];
    let _: () = msg_send![pie, closePath];
    let _: () = msg_send![pie, addClip];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: fade];
    let _: () = msg_send![ink, setStroke];
    let hi: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![hi, setLineWidth: 3.2f64];
    let _: () = msg_send![hi, stroke];
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
}

pub(crate) unsafe fn draw_center_flash(center: NSPoint, r: f64, text: &str, fade: f64) {
    let ns_text = ns_string(text);
    let max_w = (r * 2.0 * 0.78).max(24.0);
    let probe = 48.0;
    let probe_font: *mut Object = msg_send![class!(NSFont), boldSystemFontOfSize: probe];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let probe_keys = [ns_string("NSFont"), ns_string("NSColor")];
    let probe_vals = [probe_font, white];
    let probe_attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: probe_vals.as_ptr()
        forKeys: probe_keys.as_ptr()
        count: 2usize];
    if probe_attrs.is_null() {
        return;
    }
    let probe_size: CGSize = msg_send![ns_text, sizeWithAttributes: probe_attrs];
    let font_size = if probe_size.width > 1.0 {
        (probe * max_w / probe_size.width).clamp(16.0, r * 0.80)
    } else {
        28.0
    };
    let font: *mut Object = msg_send![class!(NSFont), boldSystemFontOfSize: font_size];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: fade];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: -1.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: 6.0f64];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object =
        msg_send![black, colorWithAlphaComponent: 0.8f64 * fade];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let keys = [ns_string("NSFont"), ns_string("NSColor"), ns_string("NSShadow")];
    let vals = [font, ink, shadow];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 3usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: center.x - size.width / 2.0,
            y: center.y - size.height / 2.0,
        }
        withAttributes: attrs];
}

/// Expanded-planet under-label: the planet name in 11pt white, centered
/// just below the oval, soft shadow instead of the pill backer so it reads
/// on any wallpaper without looking like a second caption.
pub(crate) unsafe fn draw_under_label(center: NSPoint, r: f64, text: &str) {
    let ns_text = ns_string(text);
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: -1.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: 2.0f64];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object = msg_send![black, colorWithAlphaComponent: 0.6f64];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let keys = [ns_string("NSFont"), ns_string("NSColor"), ns_string("NSShadow")];
    let vals = [font, white, shadow];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 3usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: center.x - size.width / 2.0,
            y: center.y + r + 4.0,
        }
        withAttributes: attrs];
}

/// Tiny pixel-art painter for decor: `art` rows of glyph chars, each glyph
/// mapped to an rgb color, bottom-anchored at (cx, base_y) — plants grow
/// upward from the point they were planted at.
pub(crate) unsafe fn draw_pixel_art(
    cx: f64,
    base_y: f64,
    art: &[&str],
    scale: f64,
    colors: &[(u8, (f64, f64, f64))],
    alpha: f64,
) {
    let ox = cx - (art.first().map(|r| r.len()).unwrap_or(0) as f64 * scale) / 2.0;
    let oy = base_y - art.len() as f64 * scale;
    for (row, line) in art.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let Some(&(_, c)) = colors.iter().find(|(k, _)| *k == ch) else {
                continue;
            };
            set_fill(ns_color(c.0, c.1, c.2, alpha));
            let _: () = msg_send![class!(NSBezierPath),
                fillRect: NSRect::new(
                    ox + col as f64 * scale,
                    oy + row as f64 * scale,
                    scale,
                    scale,
                )];
        }
    }
}

/// Caption pill: dark rounded backer + white 11pt text, centered above the
/// disc (VIEW_PADDING leaves headroom). NSString drawing is flipped-context
/// aware, so the point is the text's top-left in this view's coords.
pub(crate) unsafe fn draw_caption(center: NSPoint, r: f64, text: &str) {
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
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let pad_x = 8.0;
    let pad_y = 4.0;
    let pill_w = size.width + pad_x * 2.0;
    let pill_h = size.height + pad_y * 2.0;
    let pill = NSRect::new(
        center.x - pill_w / 2.0,
        (center.y - r - pill_h - 6.0).max(1.0),
        pill_w,
        pill_h,
    );
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let bg: *mut Object = msg_send![black, colorWithAlphaComponent: 0.72f64];
    set_fill(bg);
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
