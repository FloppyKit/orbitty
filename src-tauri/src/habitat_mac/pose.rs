//! Walker poses, stations, and yard wander helpers.
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use crate::habitat::HabitatEntry;

// Board → station. Disc +y is screen-down (front/south).
// Igloo left, board far/back, field middle, Needed front rim.
// Walkers never enter the igloo (x ≤ -0.50).
pub(crate) const STATION_HOUSE: (f64, f64) = (-0.611, 0.05); // igloo door
pub(crate) const STATION_BOARD: (f64, f64) = (0.0, -0.72); // Todo / sign post
pub(crate) const STATION_FIELD: (f64, f64) = (0.0, 0.05); // grass amble
pub(crate) const STATION_RIM: (f64, f64) = (0.0, 0.40); // Needed
/// Sleeper camp (closed claimed windows only — NOT minimized).
/// North of the igloo door, west of the grass pad: the gap between the
/// igloo (x ≤ -0.50) and YARD_WEST. Dim + still. Minimized stays full-
/// color on the green with everyone else.
pub(crate) const SLEEPER_WEST: f64 = -0.46;
pub(crate) const SLEEPER_EAST: f64 = -0.37; // just west of YARD_WEST (−0.364)
pub(crate) const SLEEPER_NORTH: f64 = -0.36;
pub(crate) const SLEEPER_SOUTH: f64 = -0.08;
// Grass AABB — west pinned so the igloo gap stays. Not a centered square.
pub(crate) const YARD_WEST: f64 = -0.364; // PINNED
pub(crate) const YARD_EAST: f64 = 0.54;
pub(crate) const YARD_NORTH: f64 = -0.45;
pub(crate) const YARD_SOUTH: f64 = 0.42;
/// interior inset so sprites stay on the green —
/// the wander AABB is the pad shrunk by this inward on every edge:
/// x ∈ [-0.324, 0.50], y ∈ [-0.41, 0.38]. The box never reaches the
/// igloo (x ≤ -0.50) or the sign footprint (the face sits at
/// y ≈ -0.8), so an in-box target can never walk a body there.
pub(crate) const YARD_INSET: f64 = 0.04;
/// Keep-up cadence (WORLD-YARD leftover): with the beach ball ON a Doing
/// walker DARTS — this is the arrive radius on a grass point (the exact
/// pre-amble 0.04): arrive → dwell standing → pick a new point → dart.
/// Ball OFF the smooth amble keeps its own wider handoff (YARD_SAIL).
pub(crate) const YARD_ARRIVE: f64 = 0.04;
/// Dwell range on a reached grass point (ms), hashed off the walker's
/// stream — the ball-on dart cadence only.
pub(crate) const YARD_DWELL_MIN_MS: u64 = 600;
pub(crate) const YARD_DWELL_MAX_MS: u64 = 1400;
/// Smooth wander (WORLD-YARD leftover, ball OFF): a Doing walker within
/// this of its grass point has ALREADY picked the next one — the
/// pre-arrive handoff distance. No dwell, no snap-turn.
pub(crate) const YARD_SAIL: f64 = 0.12;
// Done — home: past the LEFT rim onto the panel (GO-ORBITTY-WORLD-TOWN,
// replacing CADENCE-EXIT's north exit). Hide sits clearly past the
// visual rim (LEFTOVER: no 1–2px curtain) so the walker is seen
// stepping off the left edge, then vanishes.
// Never a rim waypoint (that yoyo'd).
pub(crate) const HOME_OFF: (f64, f64) = (-1.55, 0.05);
/// Past this line (roam.x < HOME_HIDE_X) the walked-off walker hides
/// until the rail's Done eviction (unclaim) lands — cadence never
/// deletes the claim itself.
pub(crate) const HOME_HIDE_X: f64 = -1.40;

/// Walker cadence, resolved from the snapshot's card fields. Cap = these
/// four plus the ambient default; no fifth pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pose {
    /// No card: the existing ambient amble — which dies planet-wide while
    /// any walker here works or sits.
    Roam,
    /// Todo: stand at the west board (walk from the house, then idle
    /// frame 0, bob 0).
    Stand,
    /// Doing: amble the grass paddock — walk from the house / board lip
    /// onto the green, then random points in the inset yard AABB. Ball
    /// OFF (WORLD-YARD leftover): a CONTINUOUS slow wander — the next
    /// point is picked BEFORE arrival, never a reverse heading, the walk
    /// cycle never parks on the green. Ball ON (keep-up cadence): the
    /// exact pre-amble dart returns — 0.16·dt to the point, arrive
    /// (YARD_ARRIVE), dwell 600–1400 ms STANDING, pick, dart. That
    /// darting IS the keep-up footwork.
    Work,
    /// Needed (open/later): sit at the front rim — bad news skips the
    /// queue (snap, never a scenic route). Idle frame 0, no bob, FULL
    /// brightness: sit is not a sleeper.
    Sit,
    /// Done: walk to the near lip of the board, touch (blit flips),
    /// dwell, walk home to the igloo, then unclaim. No off-disc curtain.
    WalkOff,
}

/// The board → pose mapping (one place). Needed wins over Doing for a
/// WINDOW walker; a Done card walks even a sleeper off (the complete
/// ritual closes the window — the walk-off narrates the eviction, then
/// the rail's unclaim lands). DOING-WALKER leftover (doing orbit): an
/// agent walker's needed_state is its own run's blocked signal — a
/// working droid never reports a planet Needed, so it keeps the disc.
pub(crate) fn pose_for(entry: &HabitatEntry) -> Pose {
    let droid = !entry.agent_seed.is_empty();
    if droid {
        // sit blocked and needs-you; work
        // while writing; walk off when the run is done. NES walkers
        // keep the table below (Needed wins over Doing).
        if entry.needed_state == "open"
            || entry.needed_state == "later"
            || entry.needed_state == "blocked"
        {
            return Pose::Sit;
        }
        return match entry.card_status.as_str() {
            "done" => Pose::WalkOff,
            _ => Pose::Work,
        };
    }
    if entry.needed_state == "open" || entry.needed_state == "later" {
        return Pose::Sit;
    }
    match entry.card_status.as_str() {
        "todo" => Pose::Stand,
        "doing" => Pose::Work,
        "done" => Pose::WalkOff,
        _ => Pose::Roam,
    }
}

/// 0 working, 1 blocked, 2 needs-you. Window walkers stay 0.
pub(crate) fn droid_glance(entry: &HabitatEntry) -> u8 {
    if entry.agent_seed.is_empty() {
        return 0;
    }
    match entry.needed_state.as_str() {
        "blocked" => 1,
        "open" | "later" => 2,
        _ => 0,
    }
}

pub(crate) fn pose_name(p: Pose) -> &'static str {
    match p {
        Pose::Roam => "roam",
        Pose::Stand => "stand",
        Pose::Work => "work",
        Pose::Sit => "sit",
        Pose::WalkOff => "walkoff",
    }
}

pub(crate) fn station_anchor(pose: Pose) -> Option<(f64, f64)> {
    match pose {
        Pose::Stand => Some(STATION_BOARD),
        Pose::Work => Some(STATION_FIELD),
        Pose::Sit => Some(STATION_RIM),
        Pose::WalkOff => Some(STATION_BOARD),
        Pose::Roam => None,
    }
}

/// The walker's disc-local target: its station anchor plus a small stable
/// radial offset (hashed from the entry id) so a column of walkers fans
/// out around the anchor instead of stacking on one pixel.
pub(crate) fn station_target(pose: Pose, entry_id: &str) -> (f64, f64) {
    let Some((ax, ay)) = station_anchor(pose) else {
        return (0.0, 0.0);
    };
    let h = hash_str(entry_id);
    let a = (h % 6283) as f64 / 1000.0; // 0..2π
    let r = 0.02 + (((h >> 16) % 100) as f64 / 100.0) * 0.06; // 0.02..0.08
    (ax + a.cos() * r, ay + a.sin() * r)
}

/// Stable point in the sleeper camp AABB (north of igloo, west of grass).
/// Hashed so a stack of closed apps fans across the strip, never the green.
pub(crate) fn sleeper_target(entry_id: &str) -> (f64, f64) {
    let h = hash_str(entry_id);
    let fx = ((h >> 8) & 0xffff) as f64 / 65535.0;
    let fy = ((h >> 24) & 0xffff) as f64 / 65535.0;
    (
        SLEEPER_WEST + fx * (SLEEPER_EAST - SLEEPER_WEST),
        SLEEPER_NORTH + fy * (SLEEPER_SOUTH - SLEEPER_NORTH),
    )
}

/// FNV-1a for stable per-walker offsets (tick.rs has its own for tracks).
/// also hashes the droid's bake seed string.
pub(crate) fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// per-walker wander stream seed — FNV-1a over
/// the entry id, never 0 (0 is the roam entry's "not armed yet"
/// sentinel). Two Doing bodies get different streams: no shared pixel,
/// no conga line.
pub(crate) fn yard_seed(entry_id: &str) -> u64 {
    let h = hash_str(entry_id);
    if h == 0 { 0x9e37_79b9_7f4a_7c15 } else { h }
}

/// xorshift64* step — the stream advances on every pick so points and
/// dwells never repeat in lockstep.
pub(crate) fn yard_step(s: u64) -> u64 {
    let mut x = s;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

/// A random point in the inset yard AABB off the walker's stream
/// (advances it). Always on the green, never the igloo or the sign.
pub(crate) fn yard_pick(seed: &mut u64) -> (f64, f64) {
    *seed = yard_step(*seed);
    let fx = ((*seed >> 11) & 0xffff) as f64 / 65535.0;
    let fy = ((*seed >> 37) & 0xffff) as f64 / 65535.0;
    (
        YARD_WEST + YARD_INSET + fx * (YARD_EAST - YARD_WEST - 2.0 * YARD_INSET),
        YARD_NORTH + YARD_INSET + fy * (YARD_SOUTH - YARD_NORTH - 2.0 * YARD_INSET),
    )
}

/// the next grass point, preferring
/// a heading that is NOT a reverse of the walker's current one — a pick
/// whose dot with (dx, dy) is ≤ −0.5 (a ≥ 120° about-face) is skipped,
/// up to 3 stream steps; the stream always advances, so a cornered
/// walker still gets out. No reverse darts, no pause.
pub(crate) fn yard_pick_sailing(seed: &mut u64, x: f64, y: f64, dx: f64, dy: f64) -> (f64, f64) {
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-9 {
        return yard_pick(seed);
    }
    let (ux, uy) = (dx / dist, dy / dist);
    let mut pick = yard_pick(seed);
    let mut tries = 0;
    while tries < 3 {
        let (nx, ny) = (pick.0 - x, pick.1 - y);
        let nd = (nx * nx + ny * ny).sqrt();
        if nd < 1e-9 || (nx / nd) * ux + (ny / nd) * uy > -0.5 {
            break;
        }
        pick = yard_pick(seed);
        tries += 1;
    }
    pick
}

/// Dwell on a reached grass point (ball-on dart cadence): 600–1400 ms
/// STANDING, hashed off the walker's stream so two bodies never park in
/// lockstep.
pub(crate) fn yard_dwell_ms(seed: &mut u64) -> u64 {
    *seed = yard_step(*seed);
    YARD_DWELL_MIN_MS + (*seed % (YARD_DWELL_MAX_MS - YARD_DWELL_MIN_MS + 1))
}

/// Inside the inset yard AABB? (Four edges, inset inward on every side —
/// the west edge stays pinned at YARD_WEST.)
pub(crate) fn yard_contains(x: f64, y: f64) -> bool {
    x >= YARD_WEST + YARD_INSET
        && x <= YARD_EAST - YARD_INSET
        && y >= YARD_NORTH + YARD_INSET
        && y <= YARD_SOUTH - YARD_INSET
}

/// Hard wall (law 6): clamp a position to the inset yard AABB — a Doing
/// walker on the grass never leaves it; the igloo and the sign footprint
/// stay unreachable.
pub(crate) fn yard_clamp(x: f64, y: f64) -> (f64, f64) {
    (
        x.clamp(YARD_WEST + YARD_INSET, YARD_EAST - YARD_INSET),
        y.clamp(YARD_NORTH + YARD_INSET, YARD_SOUTH - YARD_INSET),
    )
}

/// A walk-off in flight never flickers back to Roam on a blank snapshot
/// (unlinked-card / doneCard race): only a real column — or the claim
/// leaving the snapshot — cancels it. Without this a mid-walk `""` drops
/// the walker to Roam and it idles wherever it was: the disc middle.
pub(crate) fn sticky_pose(prev: Pose, next: Pose) -> Pose {
    if prev == Pose::WalkOff && next == Pose::Roam {
        Pose::WalkOff
    } else {
        next
    }
}

/// WalkOff always aims past the LEFT rim onto
/// the panel — Done goes home to the igloo, then off the west edge.
/// Hashed y (±0.12) so two Done walkers don't overlap. `x` is clamped
/// so a target is never more EAST than the walker already is — x only
/// ever decreases, so there is no yoyo.
pub(crate) fn walkoff_target(entry_id: &str, x: f64, _y: f64) -> (f64, f64) {
    let h = hash_str(entry_id);
    let ty = if (h >> 16) & 1 == 0 { -0.12 } else { 0.12 };
    (HOME_OFF.0.min(x), ty)
}

