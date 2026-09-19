//! Marble drift / orbit membership (host + bary clusters).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use std::time::Instant;

use super::geom::NSPoint;

// Marbles: own slow track around nearest expanded planet, else a frozen
// barycenter. Circle at live radius + 2 marble-widths. Go around the
// disc, never through. Polar join, not a straight line. Orbit path
// never writes worlds.json. CVDisplayLink posts drift_pass and returns.
pub(crate) const DRIFT_JOIN_DIST: f64 = 220.0; // bary cluster join (marble↔marble)
pub(crate) const DRIFT_LEAVE_DIST: f64 = 280.0; // bary cluster leave
pub(crate) const DRIFT_PIN_SECS: f64 = 2.0;
pub(crate) const OVAL_RATIO: f64 = 0.88; // bary tracks only — mild oval, almost round
pub(crate) const JOIN_EASE_SECS: f64 = 6.0; // ease-IN onto your own ring, polar
pub(crate) const HOST_SWITCH_CLOSER: f64 = 80.0; // host hysteresis: switch only if ≥80pt closer
pub(crate) const HOST_JOIN_DIST: f64 = 280.0; // expanded-host join, off the LIVE disc edge
pub(crate) const HOST_LEAVE_DIST: f64 = 360.0; // edge distance — a zoomed chip keeps its ring
/// Front/back boolean hysteresis: the bit flips only outside ±deadband
/// on sin θ, so it never chatters at the equator.
pub(crate) const FRONT_DEADBAND: f64 = 0.08;
pub(crate) const BARY_R_MIN: f64 = 70.0;
pub(crate) const BARY_R_MAX: f64 = 140.0;
pub(crate) const TRACK_T_MIN: f64 = 90.0; // per-marble period ∈ 90–140s, hashed from id
pub(crate) const TRACK_T_MAX: f64 = 140.0;
/// sun_key prefixes: an expanded-planet host vs a frozen barycenter.
pub(crate) const HOST_PREFIX: &str = "h\n";
pub(crate) const BARY_PREFIX: &str = "b\n";


/// a marble's in-memory track membership.
/// sun_key names the sun: HOST_PREFIX + the expanded planet's id, or
/// BARY_PREFIX + the cluster's sorted member ids. A key change (host
/// switch, cluster recapture) re-arms the join ease from the live panel
/// position, so joins never teleport. The frozen barycenter homes live
/// in `Habitat::clusters` keyed by the bary sun_key; a host sun is just
/// the planet's live panel center (expanded planets never translate
/// under drift). Never persisted — worlds.json keeps only the live
/// position; sun, θ, and orientation live in memory only.
#[derive(Clone, Debug)]
pub(crate) struct MarbleOrbit {
    pub(crate) sun_key: String,
    pub(crate) lerp_start: Instant,
    pub(crate) lerp_from: NSPoint,
    /// Last applied z vs this marble's OWN host (true = in front).
    /// Flips only outside the equator deadband; orderWindow:relativeTo:
    /// fires on the flip only — never a per-tick restack.
    pub(crate) front: bool,
}

/// a frozen barycenter capture, keyed by
/// BARY_PREFIX + the sorted member ids. home is the cluster's live
/// centroid captured ONCE at (re)capture — membership change means a
/// new key and a new capture; the home is never recomputed from live
/// x/y and never walked. The screen keep-in may shrink radii / nudge
/// the sun per tick but never writes back.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClusterState {
    pub(crate) home: NSPoint,
}

