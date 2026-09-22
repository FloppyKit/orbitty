//! World disc painting (DIET-HABITAT).
//!
//! Extracted from habitat_mac/draw.rs — draw_world paint body.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::super::decor_art::{
    plant_metrics, shroom_count, BALL, BOOMBOX, BOOMBOX_SCALE, GRASS_TUFTS, HOUSE, IGLOO, MUSHROOM,
    SHROOM_OFFSETS, SHROOM_SCALE, TUFT, TUFT_GREENS, TUFT_SMALL,
};
use super::super::{
    boombox_hud, fill_oval, now_epoch, ns_color, radius_y, set_fill, walker_fit, CGSize,
    DecorKind, NSPoint, NSRect, STATION_COMSTACK, STATION_FIELD, STATION_HOUSE,
    STATION_SUNDIAL, YARD_EAST, YARD_NORTH, YARD_SOUTH, YARD_WEST,
};

#[path = "draw_data.rs"]
mod draw_data;
pub(crate) use draw_data::{DecorDraw, DrawData, WalkerDraw};
use draw_data::{
    draw_nsimage, shaded, COMSTACK_W, IGLOO_DOOR, IGLOO_IMG_H, IGLOO_IMG_W, IGLOO_W, SUNDIAL_W,
};

#[path = "draw_fx.rs"]
mod draw_fx;
use draw_fx::*;

#[path = "draw_board.rs"]
mod draw_board;

pub(crate) unsafe fn draw_world(this: &Object, d: &DrawData) {
    let bounds: NSRect = msg_send![this, bounds];
    let center = NSPoint {
        x: bounds.mid_x(),
        y: bounds.mid_y(),
    };
    let r = d.radius * d.marble_scale;
    let ry = radius_y(r, d.projection);
    let rect = NSRect::new(center.x - r, center.y - ry, r * 2.0, ry * 2.0);
    let base = d.color;
    let dark = shaded(base, 0.6);
    let light = shaded(base, 1.3);

    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: 3.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: if d.is_marble { 4.0 } else { 8.0 }];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object = msg_send![black, colorWithAlphaComponent: 0.35f64];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let _: () = msg_send![shadow, set];
    set_fill(ns_color(base.0, base.1, base.2, 1.0));
    fill_oval(rect);
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let clip: *mut Object = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![clip, addClip];

    if !d.is_marble {
        let band_y: [f64; 3] = [-0.34, 0.04, 0.38];
        for i in 0..3 {
            let phase = d.rotation + (i as f64) * (std::f64::consts::TAU / 3.0);
            let fx = center.x + phase.sin() * r * 0.55;
            let fw = r * 0.46 * phase.cos().abs().max(0.18);
            let fh = ry * 0.22;
            let fy = center.y + band_y[i] * ry;
            let c = if i % 2 == 0 { light } else { dark };
            set_fill(ns_color(c.0, c.1, c.2, 0.30));
            fill_oval(NSRect::new(fx - fw / 2.0, fy - fh / 2.0, fw, fh));
        }
    }

    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let shade: *mut Object = msg_send![black, colorWithAlphaComponent: 0.28f64];
    let pair = [clear, shade];
    let colors: *mut Object =
        msg_send![class!(NSArray), arrayWithObjects: pair.as_ptr() count: 2usize];
    let locations: [f64; 2] = [0.4, 1.0];
    let space: *mut Object = msg_send![class!(NSColorSpace), deviceRGBColorSpace];
    let grad: *mut Object = msg_send![class!(NSGradient), alloc];
    let grad: *mut Object = msg_send![grad,
        initWithColors: colors
        atLocations: locations.as_ptr()
        colorSpace: space];
    if !grad.is_null() {
        let start = NSPoint {
            x: rect.origin.x + r * 0.3,
            y: rect.origin.y + ry * 0.3,
        };
        let end = NSPoint {
            x: rect.max_x() - r * 0.15,
            y: rect.max_y() - ry * 0.15,
        };
        let _: () = msg_send![grad, drawFromPoint: start toPoint: end options: 0usize];
    }

    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let hl: *mut Object =
        msg_send![white, colorWithAlphaComponent: if d.is_marble { 0.25 } else { 0.16 }];
    set_fill(hl);
    fill_oval(NSRect::new(
        center.x - r * 0.62,
        center.y - ry * 0.66,
        r * 0.6,
        ry * 0.34,
    ));

    if !d.is_marble {
        let gx = center.x + YARD_WEST * r;
        let gy = center.y + YARD_NORTH * ry;
        let gw = (YARD_EAST - YARD_WEST) * r;
        let gh = (YARD_SOUTH - YARD_NORTH) * ry;
        let corner = 0.10 * r;
        let dirt: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRoundedRect: NSRect::new(gx, gy, gw, gh)
            xRadius: corner
            yRadius: corner];
        set_fill(ns_color(0.24, 0.36, 0.20, 0.45));
        let _: () = msg_send![dirt, fill];
        let tuft_scale = (0.012 * r).clamp(1.5, 4.0);
        for (i, &(tx, ty)) in GRASS_TUFTS.iter().enumerate() {
            let art = if i % 2 == 0 { &TUFT[..] } else { &TUFT_SMALL[..] };
            draw_pixel_art(
                center.x + (STATION_FIELD.0 + tx) * r,
                center.y + (STATION_FIELD.1 + ty) * ry,
                art,
                tuft_scale,
                &TUFT_GREENS[..],
                0.95,
            );
        }
    }

    for decor in &d.decor {
        let cx = decor.frame.mid_x();
        let base_y = decor.frame.max_y();
        match decor.kind {
            DecorKind::Plant => {
                let ph = ((decor.t * 7.0).floor() as usize).min(7);
                if !super::super::todo_tree::paint_phase(cx, base_y, d.radius, ph) {
                    let (art, scale) = plant_metrics(decor.t);
                    draw_pixel_art(
                        cx,
                        base_y,
                        art,
                        scale,
                        &[
                            (b'L', (0.30, 0.62, 0.32)),
                            (b'S', (0.16, 0.45, 0.22)),
                            (b'G', (0.30, 0.62, 0.32)),
                            (b'T', (0.35, 0.22, 0.12)),
                        ],
                        1.0,
                    );
                }
            }
            DecorKind::Mushroom => {
                let n = shroom_count(decor.t);
                let mini = 7.0 * SHROOM_SCALE;
                for (dx, dy) in SHROOM_OFFSETS.iter().take(n) {
                    draw_pixel_art(
                        cx + dx,
                        base_y + dy,
                        &MUSHROOM[..],
                        SHROOM_SCALE,
                        &[(b'R', (0.75, 0.22, 0.22)), (b'W', (0.95, 0.93, 0.88))],
                        1.0,
                    );
                }
                let _ = mini;
            }
            DecorKind::Building => {
                let scale = 1.6 + decor.t * 1.4;
                draw_pixel_art(
                    cx,
                    base_y,
                    &HOUSE[..],
                    scale,
                    &[
                        (b'R', (0.62, 0.22, 0.18)),
                        (b'W', (0.86, 0.78, 0.62)),
                        (b'D', (0.28, 0.16, 0.10)),
                    ],
                    1.0,
                );
            }
            DecorKind::Boombox => {
                let alpha = if decor.on { 1.0 } else { 0.55 };
                if d.town_boombox != 0 {
                    draw_nsimage(
                        d.town_boombox as *mut Object,
                        decor.frame,
                        alpha,
                    );
                } else {
                    draw_pixel_art(
                        cx,
                        base_y,
                        &BOOMBOX[..],
                        BOOMBOX_SCALE,
                        &[
                            (b'H', (0.55, 0.55, 0.60)),
                            (b'B', (0.18, 0.18, 0.22)),
                            (b'K', (0.05, 0.05, 0.07)),
                            (b'W', (0.85, 0.85, 0.90)),
                        ],
                        alpha,
                    );
                }
            }
        }
    }
    if !d.is_marble {
        super::super::todo_tree::paint_ticks(
            &d.todo_trees,
            d.radius,
            d.projection,
            d.tree_grow_days,
            now_epoch(),
        );
    }

    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

    let stroke_color = ns_color(dark.0, dark.1, dark.2, 0.8);
    let _: () = msg_send![stroke_color, setStroke];
    let ring: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(0.75)];
    let _: () = msg_send![ring, setLineWidth: if d.is_marble { 1.0 } else { 1.5 }];
    let _: () = msg_send![ring, stroke];

    if !d.is_marble {
        draw_board::paint(d);
        if d.town_igloo != 0 {
            let blit_w = IGLOO_W * r;
            let blit_h = blit_w * IGLOO_IMG_H / IGLOO_IMG_W;
            let door_x = center.x + STATION_HOUSE.0 * r;
            let door_y = center.y + STATION_HOUSE.1 * ry;
            draw_nsimage(
                d.town_igloo as *mut Object,
                NSRect::new(
                    door_x - IGLOO_DOOR.0 * blit_w,
                    door_y - IGLOO_DOOR.1 * blit_h,
                    blit_w,
                    blit_h,
                ),
                1.0,
            );
        } else {
            let town = (r / 72.0).clamp(0.7, 2.0);
            let ix = center.x + STATION_HOUSE.0 * r;
            let iy = center.y + STATION_HOUSE.1 * ry;
            draw_pixel_art(
                ix,
                iy + 4.0 * town,
                &IGLOO[..],
                2.8 * town,
                &[(b'I', (0.88, 0.93, 0.97)), (b'D', (0.20, 0.24, 0.30))],
                1.0,
            );
        }
        // Sundial: south of igloo, west of grass. Bottom-pinned at the station.
        if d.town_sundial != 0 {
            let w = SUNDIAL_W * r;
            let cx = center.x + STATION_SUNDIAL.0 * r;
            let cy = center.y + STATION_SUNDIAL.1 * ry;
            draw_nsimage(
                d.town_sundial as *mut Object,
                NSRect::new(cx - w / 2.0, cy - w, w, w),
                1.0,
            );
        }
        // Comms: east of grass, south of the grove L, north of the south lip.
        if d.town_comstack != 0 {
            let w = COMSTACK_W * r;
            let cx = center.x + STATION_COMSTACK.0 * r;
            let cy = center.y + STATION_COMSTACK.1 * ry;
            draw_nsimage(
                d.town_comstack as *mut Object,
                NSRect::new(cx - w / 2.0, cy - w, w, w),
                1.0,
            );
        }
    }

    for frame in &d.dots {
        let white85: *mut Object = msg_send![white, colorWithAlphaComponent: 0.85f64];
        set_fill(white85);
        fill_oval(frame.inset(0.5));
        set_fill(ns_color(base.0, base.1, base.2, 1.0));
        fill_oval(frame.inset(3.0));
    }

    for w in &d.walkers {
        let frame = &w.frame;
        let mut sprite_rect = *frame;
        if w.pack_frame != 0 {
            let img = w.pack_frame as *mut Object;
            let size: CGSize = msg_send![img, size];
            let aspect = if size.width > 0.0 && size.height > 0.0 {
                size.width / size.height
            } else {
                1.0
            };
            let fit = walker_fit(d.radius, d.sprite_scale * w.user_scale) * w.scale;
            let (dw, dh) = if aspect >= 1.0 {
                (fit, fit / aspect)
            } else {
                (fit * aspect, fit)
            };
            let cx = frame.mid_x();
            let base_y = frame.max_y() - 2.0 + w.bob;
            let art = NSRect::new(cx - dw / 2.0, base_y - dh, dw, dh);
            sprite_rect = art;
            let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
            let ctx: *mut Object = msg_send![class!(NSGraphicsContext), currentContext];
            if !ctx.is_null() {
                let _: () = msg_send![ctx, setImageInterpolation: 1usize];
            }
            if w.flip {
                let tf: *mut Object = msg_send![class!(NSAffineTransform), transform];
                let _: () = msg_send![tf, translateXBy: cx yBy: 0.0];
                let _: () = msg_send![tf, scaleXBy: -1.0 yBy: 1.0];
                let _: () = msg_send![tf, translateXBy: -cx yBy: 0.0];
                let _: () = msg_send![tf, concat];
            }
            draw_nsimage(img, art, w.dim);
            let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        } else {
            let fit = walker_fit(d.radius, d.sprite_scale * w.user_scale) * w.scale;
            let px = (fit / crate::chars::H as f64).clamp(1.5, 8.0);
            let grid = crate::chars::grid(&w.look, w.walk_frame);
            let pal = crate::chars::palette(&w.look);
            let cx = frame.mid_x();
            let base_y = frame.max_y() - 2.0 + w.bob;
            let ox = cx - (crate::chars::W as f64 * px) / 2.0;
            let oy = base_y - crate::chars::H as f64 * px;
            sprite_rect = NSRect::new(ox, oy, crate::chars::W as f64 * px, crate::chars::H as f64 * px);
            for (row, line) in grid.iter().enumerate() {
                for (col, ch) in line.iter().enumerate() {
                    let Some(c) = crate::chars::glyph_color(*ch, &pal) else {
                        continue;
                    };
                    let draw_col = if w.flip { crate::chars::W - 1 - col } else { col };
                    set_fill(ns_color(c.0, c.1, c.2, w.dim));
                    let cell = NSRect::new(
                        ox + draw_col as f64 * px,
                        oy + row as f64 * px,
                        px,
                        px,
                    );
                    let _: () = msg_send![class!(NSBezierPath), fillRect: cell];
                }
            }
        }
        if w.icon != 0 {
            let bs = 14.0;
            let badge = NSRect::new(
                sprite_rect.max_x() - bs + 2.0,
                sprite_rect.max_y() - bs + 2.0,
                bs,
                bs,
            );
            let white85: *mut Object =
                msg_send![white, colorWithAlphaComponent: 0.85f64 * w.dim];
            set_fill(white85);
            fill_oval(badge.inset(-1.0));
            draw_nsimage(w.icon as *mut Object, badge, w.dim);
        }
        if w.locate {
            let ring_color: *mut Object =
                msg_send![white, colorWithAlphaComponent: 0.85f64];
            let _: () = msg_send![ring_color, setStroke];
            let ring: *mut Object = msg_send![class!(NSBezierPath),
                bezierPathWithRoundedRect: frame.inset(-4.0)
                xRadius: 6.0f64
                yRadius: 6.0f64];
            let _: () = msg_send![ring, setLineWidth: 2.5f64];
            let _: () = msg_send![ring, stroke];
        }
    }

    if let Some((bx, by)) = d.ball {
        if d.town_ball != 0 {
            let s = (0.028 * d.radius).clamp(16.0, 20.0);
            draw_nsimage(
                d.town_ball as *mut Object,
                NSRect::new(bx - s / 2.0, by - s, s, s),
                1.0,
            );
        } else {
            let scale = (0.022 * d.radius).clamp(1.5, 3.0);
            draw_pixel_art(
                bx,
                by,
                &BALL[..],
                scale,
                &[(b'O', (0.95, 0.52, 0.14)), (b'W', (0.96, 0.95, 0.90))],
                1.0,
            );
        }
    }

    let pill = d.caption.as_ref().or(if d.hover_name {
        Some(&d.name)
    } else {
        None
    });
    if let Some(text) = pill {
        draw_caption(center, ry, text);
    }

    if !d.is_marble {
        draw_under_label(center, ry, &d.name);
    }

    draw_planet_locate(d, center, r, ry, rect);

    if let Some(hud) = &d.hud {
        boombox_hud::draw(hud);
    }
}
