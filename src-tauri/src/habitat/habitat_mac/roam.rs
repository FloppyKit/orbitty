//! Per-walker roam state (position, facing, check-in flags).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use std::time::Instant;

/// Per-walker roam state (ORBITTY_PLANET_PLACE_GROW): walkers walk the rim
/// under their own angular speed, pausing now and then — kept outside
/// `Being` so a re-sync doesn't teleport everyone back to the start.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RoamState {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) tx: f64,
    pub(crate) ty: f64,
    pub(crate) pause_end: Option<Instant>,
    pub(crate) facing_left: bool,
    /// 0 east, 1 south (toward camera), 2 west, 3 north. Droids pick a
    /// 4-dir sheet from this; NES walkers still only read facing_left.
    pub(crate) facing_dir: u8,
    /// the walked-off emit fired (walker hidden off the
    /// north rim). Reset whenever the pose leaves WalkOff so a re-claimed
    /// walker's next Done emits again.
    pub(crate) off_emitted: bool,
    /// WORLD-TOUCH / WORLD-STOP: a Done walker has reached the near lip
    /// of the board (south edge of `board_column_rects`, not STATION_BOARD).
    /// Lifts the blit hold (tick may move to done) and arms the lip dwell.
    /// Reset whenever the pose leaves WalkOff.
    pub(crate) via_board: bool,
    /// WORLD-HOME: after the lip dwell, the walker has reached the igloo.
    /// Emit only after this. Reset whenever the pose leaves WalkOff.
    /// Never a left-rim curtain (`HOME_OFF` stays dead).
    pub(crate) via_home: bool,
    /// "user dropped on the board, don't
    /// auto-flip" — set ONLY by the drag-drop column paths (a drop on
    /// Todo parks the walker at the lip with no auto re-check-in; a drop
    /// on Doing spends the manual check-in). The walked auto check-in
    /// never sets it (PROPS leftover: it retries on a throttle instead).
    /// Re-arms on a fresh claim journey — a new roam entry, or the
    /// return-from-hide teleport in anim_tick.
    pub(crate) via_checkin: bool,
    /// PROPS leftover (check-in retry): when the walked auto check-in
    /// last fired. A Stand walker at the board lip re-fires
    /// habitat:board-checkin every ~0.5s while the snapshot still says
    /// todo — a missed rail (or AX-untrusted no-op sync) used to park
    /// the walker at the lip forever. None = not fired yet this roam.
    pub(crate) checkin_emit_at: Option<Instant>,
    /// the Doing wander stream — 0 = not armed
    /// (never Doing yet this roam); seeded from the entry id on the
    /// first Doing tick, advanced on every grass-point / dwell pick so
    /// two Doing bodies never conga. Kept across pose flips.
    pub(crate) wander_seed: u64,
}

