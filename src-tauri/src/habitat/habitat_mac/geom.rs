//! Disc / view geometry for the macOS habitat.
//!
//! NSPoint/NSRect mirror AppKit layouts; Projection picks flat vs oblique
//! disc math. Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct NSPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct CGSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct NSRect {
    pub(crate) origin: NSPoint,
    pub(crate) size: CGSize,
}

impl NSRect {
    pub(crate) fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        NSRect {
            origin: NSPoint { x, y },
            size: CGSize {
                width: w,
                height: h,
            },
        }
    }
    pub(crate) fn mid_x(&self) -> f64 {
        self.origin.x + self.size.width / 2.0
    }
    pub(crate) fn mid_y(&self) -> f64 {
        self.origin.y + self.size.height / 2.0
    }
    pub(crate) fn max_x(&self) -> f64 {
        self.origin.x + self.size.width
    }
    pub(crate) fn max_y(&self) -> f64 {
        self.origin.y + self.size.height
    }
    pub(crate) fn inset(&self, d: f64) -> Self {
        NSRect::new(
            self.origin.x + d,
            self.origin.y + d,
            self.size.width - 2.0 * d,
            self.size.height - 2.0 * d,
        )
    }
    pub(crate) fn contains(&self, p: NSPoint) -> bool {
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Projection {
    Flat,
    Oblique,
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Oblique
    }
}

/// expanded planets are always 2.5D; marbles are
/// always 2D. Not a persisted per-planet switch.
pub(crate) fn locked_projection(is_marble: bool) -> Projection {
    if is_marble {
        Projection::Flat
    } else {
        Projection::Oblique
    }
}

pub(crate) fn locked_projection_str(is_marble: bool) -> &'static str {
    if is_marble {
        "flat"
    } else {
        "oblique"
    }
}

/// Vertical radius: oblique (2.5D) squashes the disc to an ellipse.
pub(crate) fn radius_y(radius: f64, projection: Projection) -> f64 {
    match projection {
        Projection::Flat => radius,
        Projection::Oblique => radius * 0.55,
    }
}

/// town at real scale — the walker shrinks so
/// 10–12 fit on a planet. (PROPS leftover: the igloo dropped the 1.6×
/// grow off this cap — a fixed disc-relative width keeps it on-disc.)
pub(crate) fn walker_fit(radius: f64, user_scale: f64) -> f64 {
    let scale = if user_scale <= 0.0 {
        1.0
    } else {
        user_scale.clamp(0.5, 1.25)
    };
    (radius * 0.18).clamp(40.0, 80.0) * scale
}

/// Disc-local unit coords (-1..1) → view-local point.
pub(crate) fn disc_to_view(center: NSPoint, radius: f64, projection: Projection, x: f64, y: f64) -> NSPoint {
    NSPoint {
        x: center.x + x * radius,
        y: center.y + y * radius_y(radius, projection),
    }
}

/// View-local point → disc-local unit coords.
pub(crate) fn view_to_disc(center: NSPoint, radius: f64, projection: Projection, p: NSPoint) -> (f64, f64) {
    (
        (p.x - center.x) / radius,
        (p.y - center.y) / radius_y(radius, projection),
    )
}
