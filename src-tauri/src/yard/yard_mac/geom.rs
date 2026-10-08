//! Yard panel geometry. Same POD rects the habitat overlay uses.

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use super::CAT;

// geometry (habitat stays crate-private; these are the same
// handful of POD helpers)

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

unsafe impl objc::Encode for CGSize {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGSize=dd}") }
    }
}

/// visibleFrame excludes the menu bar / Dock — the walk bounds, so a cat
/// never ambles under OS chrome. Rain still falls through the full frame.
pub(crate) unsafe fn visible_frame_for(s: *mut Object) -> NSRect {
    msg_send![s, visibleFrame]
}

/// Walk bounds in view-local flipped (y-down) coords for a display whose
/// global frame is `frame` and visibleFrame is `vis`.
pub(crate) fn walk_rect(frame: &NSRect, vis: &NSRect) -> NSRect {
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
