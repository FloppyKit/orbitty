#![allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[allow(unused_imports)]
use super::{CardTombstone, PersistedState};
#[allow(unused_imports)]
use super::models::*;
#[allow(unused_imports)]
use super::models_companion::*;
#[allow(unused_imports)]
use super::models_board::*;
#[allow(unused_imports)]
use super::io::*;
#[allow(unused_imports)]
use super::merge::*;

pub(crate) fn needed_split_default() -> f64 {
    0.28
}

pub(crate) fn preview_width_default() -> f64 {
    280.0
}

pub(crate) fn tracker_split_default() -> f64 {
    0.20
}

pub(crate) fn tree_grow_days_default() -> f64 {
    14.0
}

pub(crate) fn signed_locally_default() -> bool {
    true
}

pub(crate) fn planet_idle_default() -> String {
    "4h".into()
}

pub(crate) fn rail_board_cols_default() -> Vec<String> {
    vec!["todo".into()]
}

pub(crate) fn rail_edge_default() -> String {
    "left".into()
}

pub(crate) fn rail_auto_hide_default() -> bool {
    true
}

/// "left" | "right" | "top" | "bottom". Anything else is left.
pub fn normalize_rail_edge(raw: &str) -> &'static str {
    match raw.trim() {
        "right" => "right",
        "top" => "top",
        "bottom" => "bottom",
        "left" => "left",
        _ => "left",
    }
}

/// Window origin on a top-left visible frame. Hidden keeps a 12px sliver
/// on `edge`. Revealed sits 8px in from that same edge.
pub fn rail_stage_frame(
    edge: &str,
    revealed: bool,
    vx: f64,
    vy: f64,
    vw: f64,
    vh: f64,
    ww: f64,
    wh: f64,
) -> (f64, f64, f64, f64) {
    let sliver = 12.0;
    let inset = 8.0;
    let w = if ww > 0.0 { ww } else { 360.0 };
    let h = if wh > 0.0 { wh } else { 420.0 };
    let (x, y) = match normalize_rail_edge(edge) {
        "right" => {
            let x = if revealed {
                vx + vw - w - inset
            } else {
                vx + vw - sliver
            };
            let y = vy + ((vh - h) / 2.0).max(0.0);
            (x, y)
        }
        "top" => {
            let y = if revealed { vy + inset } else { vy - h + sliver };
            let x = vx + ((vw - w) / 2.0).max(0.0);
            (x, y)
        }
        "bottom" => {
            let y = if revealed {
                vy + vh - h - inset
            } else {
                vy + vh - sliver
            };
            let x = vx + ((vw - w) / 2.0).max(0.0);
            (x, y)
        }
        _ => {
            let x = if revealed { vx + inset } else { vx - w + sliver };
            let y = vy + ((vh - h) / 2.0).max(0.0);
            (x, y)
        }
    };
    (x, y, w, h)
}

/// Revealed cap: wheel column plus the board beside it.
/// `stageLook.ts` `WHEEL_COL` 320 + `PREVIEW_MARGIN` + ~690px board.
pub(crate) const RAIL_SPIN_W: f64 = 1040.0;
pub(crate) const RAIL_SPIN_H: f64 = 760.0;

/// Revealed window hugs the wheel and the board. LEFT/RIGHT inset the width
/// by the 8px `rail_stage_frame` uses on the parked edge; TOP/BOTTOM inset
/// the height. Parked stays the compact 360×420 sliver. Unknown edges are left.
pub fn rail_wheel_size(edge: &str, revealed: bool, vw: f64, vh: f64) -> (f64, f64) {
    if !revealed {
        return (360.0, 420.0);
    }
    match normalize_rail_edge(edge) {
        "top" | "bottom" => (vw.min(RAIL_SPIN_W), (vh - 8.0).max(0.0).min(RAIL_SPIN_H)),
        _ => ((vw - 8.0).max(0.0).min(RAIL_SPIN_W), vh.min(RAIL_SPIN_H)),
    }
}
