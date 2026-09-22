//! Board sign and column ticks on an expanded planet (DIET-HABITAT).
//!
//! Peeled from `draw_world`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::super::super::board_geom::{board_column_rects, sign_blit_rect};
use super::super::super::{ns_color, set_fill, NSRect};
use super::draw_data::draw_nsimage;
use super::DrawData;

pub(crate) unsafe fn paint(d: &DrawData) {
    let blit = sign_blit_rect(d.radius, d.projection, d.marble_scale);
    if d.town_sign != 0 {
        draw_nsimage(d.town_sign as *mut Object, blit, 1.0);
    }
    let cols = board_column_rects(d.radius, d.projection, d.marble_scale);
    let rail_tints = [
        (1.0, 1.0, 1.0, 0.16),
        (1.0, 0.75, 0.35, 0.22),
        (0.45, 0.85, 0.50, 0.20),
    ];
    let col_w = cols[0].size.width;
    let col_h = cols[0].size.height;
    let tick = (col_w * 0.55).clamp(2.0, 6.0);
    let tick_step = tick * 1.4;
    let max_ticks = (((col_h - tick * 0.7) / tick_step).floor() as usize).max(1);
    for (ci, ticks) in d.board.iter().enumerate() {
        let col = cols[ci];
        let (tr, tg, tb, ta) = rail_tints[ci];
        set_fill(ns_color(tr, tg, tb, ta));
        let rail: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRoundedRect: col
            xRadius: 2.0f64
            yRadius: 2.0f64];
        let _: () = msg_send![rail, fill];
        for (ti, &(cr, cg, cb)) in ticks.iter().take(max_ticks).enumerate() {
            set_fill(ns_color(cr, cg, cb, 0.95));
            let t: *mut Object = msg_send![class!(NSBezierPath),
                bezierPathWithRoundedRect: NSRect::new(
                    col.origin.x + (col_w - tick) / 2.0,
                    col.origin.y + tick * 0.35 + ti as f64 * tick_step,
                    tick,
                    tick,
                )
                xRadius: 1.0f64
                yRadius: 1.0f64];
            let _: () = msg_send![t, fill];
        }
    }
}
