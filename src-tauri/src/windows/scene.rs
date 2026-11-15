//! Planet scenes (GO 42b): the one frame convention the scene's AX reads
//! and writes share.
//!
//! The plan itself — which windows fall away, which go back, which frames
//! are written — is pure TypeScript in `src/lib/rail/habitat.ts`
//! (`sceneEnter` / `sceneLeave`), because the claims, the saved layout and
//! the pre-scene snapshot all live in the rail's memory. What must match
//! the OS exactly, and is decided here, is the box:
//!
//! AX reports a window's position as a **top-left-origin** CGPoint (y grows
//! down) plus a CGSize. That is not AppKit's bottom-left NSPoint: a write
//! that flips y moves the window down by its own height. `Frame` keeps the
//! top-left convention on both sides, and `frame_top_left_origin` pins it.

/// A window box in AX coordinates: points, top-left origin, y grows down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

impl Frame {
    /// Straight from AXPosition + AXSize — no flip, no screen height.
    pub(crate) fn from_ax(x: f64, y: f64, w: f64, h: f64) -> Frame {
        Frame { x, y, w, h }
    }

    /// The point and size handed straight back to AXPosition / AXSize.
    pub(crate) fn to_ax(&self) -> ((f64, f64), (f64, f64)) {
        ((self.x, self.y), (self.w, self.h))
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    const SCREEN_H: f64 = 1080.0;

    #[test]
    fn frame_top_left_origin() {
        // AX position is top-left origin; the write hands it back unchanged.
        let f = Frame::from_ax(320.0, 180.0, 800.0, 600.0);
        let ((x, y), (w, h)) = f.to_ax();
        assert_eq!((x, y), (320.0, 180.0));
        assert_eq!((w, h), (800.0, 600.0));
        // The AppKit (bottom-left) reading of the same box is a different
        // number — 300, not 180 — so a flipped write cannot pass as this one.
        let appkit_y = SCREEN_H - 180.0 - 600.0;
        assert_eq!(appkit_y, 300.0);
        assert_ne!(appkit_y, y);
        // A frame round-trips through the pair AX actually stores.
        assert_eq!(Frame::from_ax(x, y, w, h), f);
    }
}
