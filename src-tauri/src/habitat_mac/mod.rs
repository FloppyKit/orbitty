//! macOS habitat — port of OverlayController + HabitatViews.swift to Rust/objc.
//!
//! One borderless NSPanel per claimed project, at the NORMAL window level:
//! planets are peer surfaces — click/drag orders front, other apps can cover
//! them later. Panels default to ignoresMouseEvents=true; a cursor watcher
//! flips interactivity on only while the pointer is over the planet disc or
//! one of its sprites/dots, so the transparent corners stay click-through.
//!
//! Claimed home windows walk the planet as pixel sprites — open or
//! minimized, a claim is a walker (CADENCE-FIX v1.1: open-vs-minimized no
//! longer decides walker vs dot); only age-`park` zombies sit as occupancy
//! dots on the inner ring. Click matrix (NSEvent
//! clickCount): walker single = locate stroke (~500ms ring around the
//! being, GO-ORBITTY-RAIL-ROW-MENU), double = restore + focus; dot single =
//! focus + caption, double = stow back to the planet. The Being editor's
//! door is the walker right-click menu's "Open being…". Captions
//! are small native pills on the planet (~2s, titles only). Marble
//! (collapsed rail section) click expands via `habitat:expand` to the rail.
//!
//! GO-ORBITTY-CHARS: the default walker is a generated Orbitty person
//! (chars.rs layers drawn straight in drawRect) — no itch zip needed. A
//! picked image pack still wins the draw.
//!
//! GO-ORBITTY-SLEEPERS: a claimed window that closes stays on its planet as
//! a sleeping walker (idle frame 0, dim, no bounce/roam) until Done or
//! Unclaim — close is not complete. Sleeper single-click = caption +
//! locate stroke; double-click = wake_sleeper (live → file → Hermes session → recipe →
//! app), never a bare restore_window on a dead pid:cgid.
//!
//! All AppKit objects live on the main thread: the sync command hops through
//! `run_on_main_thread`; the 16ms tick thread and the CVDisplayLink vsync
//! clock both POST to the main queue via `dispatch_async_f` and return at
//! once (neither worker ever blocks on the UI thread), each coalesced by its
//! own pending flag. Raw ObjC pointers are stored as usize inside the mutex
//! and only dereferenced on the main thread.
//!
//! GO-ORBITTY-PLANET-BUDGET: the desk runs ≤3 expanded planets (a 4th
//! expand collapses the least-recently focused of the rest — never refused)
//! and ≤12 marble panels (13th+ collapsed projects stay on the rail list,
//! no panel). Sprite packs decode lazily — only while an expanded planet
//! wears them — and an expanded planet with no motion (all sleepers, no
//! caption/locate) never dirties its view on the anim tick.

// objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that newer
// rustc flags; harmless upstream noise.
#![allow(unexpected_cfgs)]

use super::{HabitatEntry, HabitatPlanet, HabitatSnapshot};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object};
use objc::{class, msg_send, sel, sel_impl};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

mod audio;
mod tick;
mod input;
mod draw;

pub use tick::{persist_now, start};

// ---------- geometry ----------

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct NSRect {
    origin: NSPoint,
    size: CGSize,
}

impl NSRect {
    fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        NSRect {
            origin: NSPoint { x, y },
            size: CGSize {
                width: w,
                height: h,
            },
        }
    }
    fn mid_x(&self) -> f64 {
        self.origin.x + self.size.width / 2.0
    }
    fn mid_y(&self) -> f64 {
        self.origin.y + self.size.height / 2.0
    }
    fn max_x(&self) -> f64 {
        self.origin.x + self.size.width
    }
    fn max_y(&self) -> f64 {
        self.origin.y + self.size.height
    }
    fn inset(&self, d: f64) -> Self {
        NSRect::new(
            self.origin.x + d,
            self.origin.y + d,
            self.size.width - 2.0 * d,
            self.size.height - 2.0 * d,
        )
    }
    fn contains(&self, p: NSPoint) -> bool {
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

// ---------- pixel sprite art ----------
// GO-ORBITTY-CHARS: walkers are generated people (chars.rs 12×12 glyph
// grids, composited body/clothes/eyes/mouth/hat + palette) drawn per frame
// below. The old blob/cat/robot templates are retired — an empty pack id
// means "generated being", never a blank.

const DOT_SIZE: f64 = 12.0;
const VIEW_PADDING: f64 = 30.0;
pub(crate) const EXPANDED_RADIUS: f64 = 72.0;
const MARBLE_RADIUS: f64 = 18.0;
/// GO-ORBITTY-PLANET-BUDGET desk budget: at most 3 expanded planets (a 4th
/// expand collapses the least-recently focused of the rest) and at most 12
/// collapsed marble panels (13th+ projects stay on the rail list, no panel).
const MAX_EXPANDED: usize = 3;
const MAX_MARBLE_PANELS: usize = 12;
// Scroll-zoom clamp for an expanded planet's radius.
pub(crate) const MIN_RADIUS: f64 = 18.0;
const MAX_RADIUS_FLOOR: f64 = 220.0;
const BASE_SPIN: f64 = std::f64::consts::TAU / 45.0; // one turn per 45s

// GO-ORBITTY-MARBLE-RIM (part 1j, rewrites MARBLE-GLASS hosted track +
// hitch clock): collapsed marbles on the same screen take SLOW WIDE
// TRACKS around a sun that is not them — planets floating in a solar
// system, not water-blobs. They do not seek each other, never kiss,
// never rush. The sun is, in priority: (1) the nearest EXPANDED planet
// on that screen (a sitting disc; expanded planets never translate under
// drift) — each in-range marble (join/leave measured off the LIVE disc
// edge: center dist − live radius vs HOST_JOIN_DIST/HOST_LEAVE_DIST, so
// a zoomed chip keeps its ring at any size) gets its OWN phase and
// direction and a hashed 90–140s period from its id, with ONE hashed
// in-plane rotation per sun; an established host holds unless another
// expanded planet on that screen is ≥ HOST_SWITCH_CLOSER pt closer to
// the marble (hysteresis, so the sun never flickers). The hosted track
// is a CIRCLE around the planet center at the LIVE (scroll-zoomed)
// radius + two marble-widths + the marble radius — bigger chip →
// bigger ring, always ~2 widths off the live edge. Constant r: no
// accordion, no back tuck, no graze dip. The disc itself is a COLLIDER
// LAW, never a window and never drawn: a marble center may not sit
// closer than live radius + MARBLE_RADIUS from an expanded planet
// center — an unconstrained point inside is pushed out along the
// bearing from the planet, so marbles go AROUND the disc, never
// through (join leftovers and bary clips included). Joining eases
// POLAR along that marble's own ring (θ and r from the armed live
// frame) over ~6s ease-IN — never a cartesian straight line through
// the chip. Z is a per-marble BOOLEAN vs its OWN host only: front/back
// from sin θ with a deadband at the equator, applied with
// orderWindow:relativeTo: only when the bit flips — never a group
// restack. (2) Else the INVISIBLE FROZEN BARYCENTER of the nearby
// marble cluster (bary hysteresis 220/280, marble↔marble): home = the
// live centroid captured ONCE at (re)capture and never walked; each
// member gets an independent radius 70–140pt and phase0 hashed from its
// id — large independent rings, NOT opposite ends of a stick, NOT one
// shared θ — on a mild almost-round oval (OVAL_RATIO 0.88). All tracks
// run at even speed (constant dθ — no Kepler). Drawn scale stays 1.0
// (no throb). Drag wins: after a drop the marble pins for
// DRIFT_PIN_SECS, then eases back onto its own ring. Screen keep-in
// nudges the SUN inward per tick (never written back) and only shrinks
// the figure when the screen can't fit it — a host ring never below
// one marble-width off the live face; marbles are never clamped
// independently and (0,0)/visibleFrame.origin is never a target.
// Isolates with no sun sit. A lone marble with an expanded planet in
// range still orbits that planet. Clicks land on the disc. The orbit
// path NEVER dirties worlds.json — live marble x/y persist on
// pin/drop/toggle/quit only. Motion is clocked by CVDisplayLink (Core
// Video — NOT CADisplayLink): the vsync callback POSTS drift_pass to
// the main queue and returns (never waits for the UI thread; at most
// one hop queued-or-running, so a stall drops a refresh instead of
// bursting late); the 16ms thread keeps drag/anim/cursor and never
// also drives motion (two clocks = jitter). If the link fails to
// create, the 16ms thread runs drift_pass EVERY tick — a move is never
// skipped (the old n % 6 hole WAS the 1-2-3-pause).
// GO-ORBITTY-MARBLE-HITCH (part 1k): that post-and-return + coalesce,
// and the hosted standoff cut from three marble-widths to two.
const DRIFT_JOIN_DIST: f64 = 220.0; // bary cluster join (marble↔marble)
const DRIFT_LEAVE_DIST: f64 = 280.0; // bary cluster leave
const DRIFT_PIN_SECS: f64 = 2.0;
const OVAL_RATIO: f64 = 0.88; // bary tracks only — mild oval, almost round
const JOIN_EASE_SECS: f64 = 6.0; // ease-IN onto your own ring, polar
const HOST_SWITCH_CLOSER: f64 = 80.0; // host hysteresis: switch only if ≥80pt closer
const HOST_JOIN_DIST: f64 = 280.0; // expanded-host join, off the LIVE disc edge
const HOST_LEAVE_DIST: f64 = 360.0; // edge distance — a zoomed chip keeps its ring
/// Front/back boolean hysteresis: the bit flips only outside ±deadband
/// on sin θ, so it never chatters at the equator.
const FRONT_DEADBAND: f64 = 0.08;
const BARY_R_MIN: f64 = 70.0;
const BARY_R_MAX: f64 = 140.0;
const TRACK_T_MIN: f64 = 90.0; // per-marble period ∈ 90–140s, hashed from id
const TRACK_T_MAX: f64 = 140.0;
/// sun_key prefixes: an expanded-planet host vs a frozen barycenter.
const HOST_PREFIX: &str = "h\n";
const BARY_PREFIX: &str = "b\n";

// ---------- GO-ORBITTY-CADENCE: the board drives the walkers ----------

/// The one board → station → pose table. Stations are disc-local anchors,
/// never Kenney buildings. The world view is flipped (isFlipped = true:
/// disc +y renders screen-DOWN), so +y = front/south, −y = far/back/top
/// of the oval, −x = screen LEFT.
///
/// GO-ORBITTY-WORLD-TOWN compass: igloo LEFT, board far/BACK, field
/// middle, Needed front rim. A board-bound walker (Stand/Work) COMES
/// FROM the igloo (STATION_HOUSE — user-placed buildings stay sandbox
/// décor, never a spawn) and walks to its station. Sit snaps to the
/// front rim (bad news skips the queue). Done walks to the near lip
/// of the board, dwells, walks home to the igloo, then unclaims —
/// never past the left rim into a hide curtain (the last walker lets
/// the planet shrink).
// WORLD-PROPS leftover: nudged east (-0.78 → -0.50) so the fixed-width
// igloo blit sits fully on the disc (west edge ≈ -0.94); spawn/home is
// still the DOOR, walkers exit on-planet. Matches the yard law "never
// enter the igloo (x ≤ -0.50)".
// Leftover #2: the igloo shrank 25% with the WEST EDGE HELD at
// ≈ -0.9446, so the door/station moved west (-0.50 → -0.611); east
// edge ≈ -0.52, still clear of the x > -0.35 fan.
pub(crate) const STATION_HOUSE: (f64, f64) = (-0.611, 0.05); // igloo — left interior
// Leftover #2: -0.58 → -0.72 — the blit rides up until the whiteboard
// top sits ~3px past the back rim (r = 72); the post base stays planted
// on-disc, and x = 0 pins the wood post (SIGN_POST_X), not the face.
pub(crate) const STATION_BOARD: (f64, f64) = (0.0, -0.72); // Todo — far/back board
// Doing — the open interior field, off the front lip (the fanned target
// keeps |y| < 0.22 and x > -0.35, so the fan never enters the igloo):
// the walk cycle keeps running here after arrival (CADENCE-PATH stilled
// it only because the old workbench station sat on the front rim and
// read as rim-strolling; the rim is Needed's now).
pub(crate) const STATION_FIELD: (f64, f64) = (0.0, 0.05);
pub(crate) const STATION_RIM: (f64, f64) = (0.0, 0.40); // Needed — front/south rim
/// GO-ORBITTY-WORLD-PROPS: the grass paddock around STATION_FIELD —
/// half-extent in disc units (x ∈ [-0.364, 0.364], y ∈ [-0.314, 0.414]).
/// A painted grass patch, never fence posts; west open toward the igloo
/// (leftover #2: 0.28 → 0.364, +30%; west edge ≈ -0.364 keeps a gap to
/// the igloo's east edge ≈ -0.52), south open toward the Needed rim.
/// WORLD-YARD wanders it; PROPS only paints it.
pub(crate) const YARD_HALF: f64 = 0.364;
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
    /// Doing: work the open interior field — walk from the house, then
    /// KEEP the walk cycle after arrival (WORLD-PLACES; CADENCE-PATH's
    /// idle-in-place was only because the station sat on the front rim).
    Work,
    /// Needed (open/later): sit at the front rim — bad news skips the
    /// queue (snap, never a scenic route). Idle frame 0, no bob, FULL
    /// brightness: sit is not a sleeper.
    Sit,
    /// Done: walk to the near lip of the board, touch (blit flips),
    /// dwell, walk home to the igloo, then unclaim. No off-disc curtain.
    WalkOff,
}

/// The board → pose mapping (one place). Needed wins over Doing; a Done
/// card walks even a sleeper off (the complete ritual closes the window —
/// the walk-off narrates the eviction, then the rail's unclaim lands).
fn pose_for(entry: &HabitatEntry) -> Pose {
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

pub(crate) fn pose_name(p: Pose) -> &'static str {
    match p {
        Pose::Roam => "roam",
        Pose::Stand => "stand",
        Pose::Work => "work",
        Pose::Sit => "sit",
        Pose::WalkOff => "walkoff",
    }
}

fn station_anchor(pose: Pose) -> Option<(f64, f64)> {
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

/// FNV-1a for stable per-walker offsets (tick.rs has its own for tracks).
fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// GO-ORBITTY-WORLD-TOWN: where a board-bound walker comes FROM — the
/// igloo station, always. User-placed `planet.buildings` stay sandbox
/// décor and are never a spawn. Stand/Work roams (and returns from
/// hide) start here and walk to the station; Sit never spawns here
/// (bad news snaps to the front rim).
/// PROPS leftover (town usable): the doorstep is IN FRONT of the east
/// door, just outside the igloo blit (door x + 0.12; the blit's east
/// edge is ≈ -0.517) — a claimed walker appears on open disc,
/// clickable (right-click menu / locate) the moment it spawns, never
/// inside the PNG. Same y as the door.
pub(crate) fn house_spawn(_planet: &Planet, _entry_id: &str) -> (f64, f64) {
    (STATION_HOUSE.0 + 0.12, STATION_HOUSE.1)
}

/// A walk-off in flight never flickers back to Roam on a blank snapshot
/// (unlinked-card / doneCard race): only a real column — or the claim
/// leaving the snapshot — cancels it. Without this a mid-walk `""` drops
/// the walker to Roam and it idles wherever it was: the disc middle.
fn sticky_pose(prev: Pose, next: Pose) -> Pose {
    if prev == Pose::WalkOff && next == Pose::Roam {
        Pose::WalkOff
    } else {
        next
    }
}

/// GO-ORBITTY-WORLD-TOWN: WalkOff always aims past the LEFT rim onto
/// the panel — Done goes home to the igloo, then off the west edge.
/// Hashed y (±0.12) so two Done walkers don't overlap. `x` is clamped
/// so a target is never more EAST than the walker already is — x only
/// ever decreases, so there is no yoyo.
pub(crate) fn walkoff_target(entry_id: &str, x: f64, _y: f64) -> (f64, f64) {
    let h = hash_str(entry_id);
    let ty = if (h >> 16) & 1 == 0 { -0.12 } else { 0.12 };
    (HOME_OFF.0.min(x), ty)
}

/// A Done walker past the hide line: not drawn, not hit-tested — it waits
/// off-screen for the rail's eviction, never a vanish mid-disc.
pub(crate) fn walker_hidden(planet: &Planet, b: &Being) -> bool {
    b.pose == Pose::WalkOff
        && planet
            .roam
            .get(&b.entry_id)
            .is_some_and(|r| r.x < HOME_HIDE_X)
}

/// The walk-off finished: tell the rail so its existing Done eviction
/// (unclaim) lands. Fire-and-forget; the rail's handler is idempotent.
/// Emits are once per walk-off (anim_tick's off_emitted, do_sync's
/// walkoff_fast for walker-less Done entries); do_sync also re-delivers
/// while a walked-off WALKER lingers past the hide line, so a missed
/// emit there heals on the next sync.
pub(crate) fn emit_walked_off(entry_id: &str) {
    if let Some(app) = app_handle() {
        let _ = app.emit_to("main", "habitat:walked-off", entry_id.to_string());
    }
}

/// GO-ORBITTY-WORLD-CHECKIN: a Todo walker touched the near lip of the
/// board — the rail's listener flips that window's card todo→doing
/// (only if still todo; a human drag always wins). Fire-and-forget;
/// PROPS leftover: anim_tick re-fires on a ~0.5s throttle while the
/// snapshot still says todo (the listener is idempotent), so a missed
/// emit heals instead of parking the walker at the lip. Broadcast to
/// ALL webviews (app.emit), not only main. Not a WalkOff path —
/// via_board/via_home stay Done-only.
pub(crate) fn emit_board_checkin(entry_id: &str) {
    if let Some(app) = app_handle() {
        let _ = app.emit("habitat:board-checkin", entry_id.to_string());
    }
}


/// Decor budget: 3 plants + 3 mushrooms at first sight, +1 each every 2
/// days, capped at 5+5; the boombox is always a single placement.
const DECOR_BASE: usize = 3;
const DECOR_CAP: usize = 5;
const DECOR_STEP_SECS: f64 = 2.0 * 86400.0;


/// (plants, mushrooms) the world may hold right now.
fn decor_budget(first_seen: f64, now: f64) -> (usize, usize) {
    let extra = ((now - first_seen).max(0.0) / DECOR_STEP_SECS) as usize;
    (
        (DECOR_BASE + extra).min(DECOR_CAP),
        (DECOR_BASE + extra).min(DECOR_CAP),
    )
}

// ---------- projection geometry ----------

/// Vertical radius: oblique (2.5D) squashes the disc to an ellipse.
fn radius_y(radius: f64, projection: Projection) -> f64 {
    match projection {
        Projection::Flat => radius,
        Projection::Oblique => radius * 0.55,
    }
}

pub(crate) fn max_radius() -> f64 {
    let all = unsafe { screens() };
    let (w, h) = all
        .first()
        .map(|(_, f)| (f.size.width, f.size.height))
        .unwrap_or((1440.0, 900.0));
    (w.min(h) * 1.35 / 2.0).max(MAX_RADIUS_FLOOR)
}

/// GO-ORBITTY-WORLD-PROPS: town at real scale — the walker shrinks so
/// 10–12 fit on a planet. (PROPS leftover: the igloo dropped the 1.6×
/// grow off this cap — a fixed disc-relative width keeps it on-disc.)
fn walker_fit(radius: f64, user_scale: f64) -> f64 {
    let scale = if user_scale <= 0.0 {
        1.0
    } else {
        user_scale.clamp(0.5, 1.25)
    };
    (radius * 0.18).clamp(40.0, 80.0) * scale
}

/// Disc-local unit coords (-1..1) → view-local point.
fn disc_to_view(center: NSPoint, radius: f64, projection: Projection, x: f64, y: f64) -> NSPoint {
    NSPoint {
        x: center.x + x * radius,
        y: center.y + y * radius_y(radius, projection),
    }
}

/// View-local point → disc-local unit coords.
fn view_to_disc(center: NSPoint, radius: f64, projection: Projection, p: NSPoint) -> (f64, f64) {
    (
        (p.x - center.x) / radius,
        (p.y - center.y) / radius_y(radius, projection),
    )
}

/// Keep a placed/dragged decor point inside the disc, with a small margin.
fn clamp_disc(x: f64, y: f64) -> (f64, f64) {
    let len = (x * x + y * y).sqrt();
    if len > 0.85 {
        (x / len * 0.85, y / len * 0.85)
    } else {
        (x, y)
    }
}


// ---------- persisted world positions ----------

/// One placed plant/mushroom: disc-local unit coords (-1..1) + birth time
/// for the 7-day growth curve.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
struct DecorItem {
    x: f64,
    y: f64,
    planted_at: f64,
}

/// Boombox placement (no growth — it just sits there and plays).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
struct DecorPoint {
    x: f64,
    y: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
enum Projection {
    Flat,
    Oblique,
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Oblique
    }
}

/// GO-ORBITTY-PROJ-LOCK: expanded planets are always 2.5D; marbles are
/// always 2D. Not a persisted per-planet switch.
fn locked_projection(is_marble: bool) -> Projection {
    if is_marble {
        Projection::Flat
    } else {
        Projection::Oblique
    }
}

fn locked_projection_str(is_marble: bool) -> &'static str {
    if is_marble {
        "flat"
    } else {
        "oblique"
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct WorldState {
    screen_id: String,
    /// Screen-local, flipped (y down) center coords.
    x: f64,
    y: f64,
    last_used: f64,
    /// Scroll-zoomed expanded radius; 0 = never zoomed (EXPANDED_RADIUS).
    radius: f64,
    /// Pre-GROW toggles, migrated into `plants`/`mushrooms` placements on
    /// load (true + empty list → one default placement, then cleared).
    plant: bool,
    mushroom: bool,
    /// Boombox playing flag (kept): on + `music` set → AVAudioPlayer loop
    /// while the planet lives. Placement lives in `boombox_pos`.
    boombox: bool,
    music: String,
    /// GO-ORBITTY-BOOMBOX remembered volume step (Quiet .25 / Low .5 /
    /// Med .75 / High 1.0). Field-level default keeps pre-fader saves at
    /// full volume — the container `default` would give 0.0.
    #[serde(default = "default_boombox_volume")]
    boombox_volume: f32,
    /// Legacy field. Live disc ignores this — GO-ORBITTY-PROJ-LOCK derives
    /// projection from marble vs expanded. Rewritten on sync to heal.
    projection: String,
    /// First time this world was seen; 0 = pre-GROW save (set to now on
    /// load). The decor budget grows from this.
    first_seen: f64,
    plants: Vec<DecorItem>,
    mushrooms: Vec<DecorItem>,
    buildings: Vec<DecorItem>,
    boombox_pos: Option<DecorPoint>,
}

fn default_boombox_volume() -> f32 {
    1.0
}

/// Additive migration for pre-GROW saves; returns true when it changed
/// anything (caller persists). Idempotent.
fn migrate_world(w: &mut WorldState, now: f64) -> bool {
    let mut dirty = false;
    if w.first_seen <= 0.0 {
        w.first_seen = now;
        dirty = true;
    }
    if w.plant {
        if w.plants.is_empty() {
            w.plants.push(DecorItem {
                x: -0.42,
                y: 0.34,
                planted_at: now,
            });
        }
        w.plant = false;
        dirty = true;
    }
    if w.mushroom {
        if w.mushrooms.is_empty() {
            w.mushrooms.push(DecorItem {
                x: 0.42,
                y: 0.34,
                planted_at: now,
            });
        }
        w.mushroom = false;
        dirty = true;
    }
    // An "on" boombox from before placements becomes a placed boombox that
    // keeps playing.
    if w.boombox && w.boombox_pos.is_none() {
        w.boombox_pos = Some(DecorPoint { x: 0.0, y: 0.62 });
        dirty = true;
    }
    dirty
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct WorldsFile {
    worlds: HashMap<String, WorldState>,
}

// ---------- habitat state (main thread only) ----------

pub(crate) struct Being {
    entry_id: String,
    /// "App — Title" for the click caption (titles only, truncated).
    label: String,
    /// View-local, flipped coords; recomputed on layout.
    frame: NSRect,
    bob: f64,
    /// Retained NSImage, or 0. Owned by the session icon cache.
    icon: usize,
    /// GO-ORBITTY-CHARS: the generated person's layer picks (resolved at
    /// sync from the claim's charLook, else derived from charSeed).
    look: crate::chars::CharLook,
    /// Per-walker size multiplier from the claim (1.0 = follow the gear's
    /// global spriteScale).
    user_scale: f64,
    /// Index into Habitat.pack_frames when user packs supply frames; None =
    /// draw the generated person.
    pack: Option<usize>,
    /// Zombie fade (TS-7): 1.0 fresh, dimmed for 7+ days untouched. Parked
    /// entries never become walkers — they occupy like dots (see do_sync).
    dim: f64,
    /// GO-ORBITTY-SLEEPERS: the claim's window is closed. The walker sleeps
    /// (idle frame 0, dimmed, no roam/bounce) and double-click wakes it via
    /// coordinator::wake_sleeper_with instead of a dead restore_window.
    asleep: bool,
    /// The claim's raw spritePackId ("" = generated being) — the walker
    /// right-click menu checks the current pick against this.
    sprite_pack_id: String,
    /// GO-ORBITTY-CADENCE: the board-driven pose, resolved from the
    /// snapshot's card fields at sync. Rebuilt every sync; the roam state
    /// (position) lives outside so a re-sync never teleports anyone.
    pose: Pose,
}

/// Which decor item a frame belongs to (plants[i], mushrooms[i], boombox).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecorKind {
    Plant,
    Mushroom,
    Building,
    Boombox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DecorRef {
    kind: DecorKind,
    index: usize,
}

/// Per-walker roam state (ORBITTY_PLANET_PLACE_GROW): walkers walk the rim
/// under their own angular speed, pausing now and then — kept outside
/// `Being` so a re-sync doesn't teleport everyone back to the start.
#[derive(Clone, Copy, Debug)]
struct RoamState {
    x: f64,
    y: f64,
    tx: f64,
    ty: f64,
    pause_end: Option<Instant>,
    facing_left: bool,
    /// GO-ORBITTY-CADENCE: the walked-off emit fired (walker hidden off the
    /// north rim). Reset whenever the pose leaves WalkOff so a re-claimed
    /// walker's next Done emits again.
    off_emitted: bool,
    /// WORLD-TOUCH / WORLD-STOP: a Done walker has reached the near lip
    /// of the board (south edge of `board_column_rects`, not STATION_BOARD).
    /// Lifts the blit hold (tick may move to done) and arms the lip dwell.
    /// Reset whenever the pose leaves WalkOff.
    via_board: bool,
    /// WORLD-HOME: after the lip dwell, the walker has reached the igloo.
    /// Emit only after this. Reset whenever the pose leaves WalkOff.
    /// Never a left-rim curtain (`HOME_OFF` stays dead).
    via_home: bool,
    /// GO-ORBITTY-WORLD-CHECKIN: "user dropped on the board, don't
    /// auto-flip" — set ONLY by the drag-drop column paths (a drop on
    /// Todo parks the walker at the lip with no auto re-check-in; a drop
    /// on Doing spends the manual check-in). The walked auto check-in
    /// never sets it (PROPS leftover: it retries on a throttle instead).
    /// Re-arms on a fresh claim journey — a new roam entry, or the
    /// return-from-hide teleport in anim_tick.
    via_checkin: bool,
    /// PROPS leftover (check-in retry): when the walked auto check-in
    /// last fired. A Stand walker at the board lip re-fires
    /// habitat:board-checkin every ~0.5s while the snapshot still says
    /// todo — a missed rail (or AX-untrusted no-op sync) used to park
    /// the walker at the lip forever. None = not fired yet this roam.
    checkin_emit_at: Option<Instant>,
}

/// GO-ORBITTY-MARBLE-SOLAR: a marble's in-memory track membership.
/// sun_key names the sun: HOST_PREFIX + the expanded planet's id, or
/// BARY_PREFIX + the cluster's sorted member ids. A key change (host
/// switch, cluster recapture) re-arms the join ease from the live panel
/// position, so joins never teleport. The frozen barycenter homes live
/// in `Habitat::clusters` keyed by the bary sun_key; a host sun is just
/// the planet's live panel center (expanded planets never translate
/// under drift). Never persisted — worlds.json keeps only the live
/// position; sun, θ, and orientation live in memory only.
#[derive(Clone, Debug)]
struct MarbleOrbit {
    sun_key: String,
    lerp_start: Instant,
    lerp_from: NSPoint,
    /// Last applied z vs this marble's OWN host (true = in front).
    /// Flips only outside the equator deadband; orderWindow:relativeTo:
    /// fires on the flip only — never a per-tick restack.
    front: bool,
}

/// GO-ORBITTY-MARBLE-SOLAR: a frozen barycenter capture, keyed by
/// BARY_PREFIX + the sorted member ids. home is the cluster's live
/// centroid captured ONCE at (re)capture — membership change means a
/// new key and a new capture; the home is never recomputed from live
/// x/y and never walked. The screen keep-in may shrink radii / nudge
/// the sun per tick but never writes back.
#[derive(Clone, Copy, Debug)]
struct ClusterState {
    home: NSPoint,
}

pub(crate) struct Planet {
    panel: usize, // NSPanel
    view: usize,  // LiveSpriteWorldView
    /// Display name from the rail snapshot — walker Assign-to-planet menu.
    name: String,
    radius: f64,
    sprite_scale: f64,
    is_marble: bool,
    color: (f64, f64, f64),
    rotation: f64,
    projection: Projection,
    walkers: Vec<Being>,
    dots: Vec<Being>,
    /// Transient native caption pill (single-click feedback), ~2s.
    caption: Option<(String, Instant)>,
    /// GO-ORBITTY-PLANET-MARK: the cursor is over a marble's disc — draw
    /// the planet name as the caption pill whenever no click caption is
    /// live. Set by the cursor watcher (panels ignore mouse events
    /// off-disc, so AppKit tracking areas would never see the enter).
    hover_name: bool,
    /// GO-ORBITTY-MARBLE-SOLAR: sun membership + join ease; cleared by
    /// isolation and when motion is stilled. The track itself (radius,
    /// phase, period) is hashed from the marble's id on the fly — these
    /// fields only carry the sun key and the ease-in window.
    orbit: Option<MarbleOrbit>,
    /// Drawn radius multiplier. GO-ORBITTY-MARBLE-SOLAR killed the fake-Z
    /// scale-throb — this stays 1.0; the field remains so the draw and
    /// hit-test paths keep one expression. In-memory only.
    marble_scale: f64,
    /// GO-ORBITTY-MARBLE-ORBIT: the dropped marble sits until this
    /// instant, then may rejoin its ring. Drag wins.
    pin_until: Option<Instant>,
    /// Decor placements, mirrored from the world state at sync/mutation.
    plants: Vec<DecorItem>,
    mushrooms: Vec<DecorItem>,
    buildings: Vec<DecorItem>,
    boombox_pos: Option<DecorPoint>,
    /// Boombox playing flag, mirrored from the world state.
    boombox: bool,
    /// Live AVAudioPlayer while the boombox is on; stopped + released on
    /// toggle-off, planet teardown, or re-toggle. usize = retained
    /// ObjC pointer; only touched on the main thread.
    audio: Option<usize>,
    /// Last live volume applied (remembered step × zoom gain). Zoom
    /// fades this in place — never restart the file.
    audio_volume: f32,
    /// Last laid-out decor frames (view-local) for hit-testing.
    decor_frames: Vec<(DecorRef, NSRect)>,
    /// Roam state per walker entry id.
    roam: HashMap<String, RoamState>,
    /// The snapshot entries the current walkers/dots were built from
    /// (CADENCE-FIX pose-only fast path): a sync whose being-defining
    /// fields are unchanged updates poses in place instead of rebuilding
    /// every Being — no icon re-decode, roam state untouched.
    walker_entries: Vec<HabitatEntry>,
    dot_entries: Vec<HabitatEntry>,
    /// GO-ORBITTY-WORLD-TOWN: the back-board's ticks (id + column, no
    /// titles), mirrored from the snapshot. Drawn as the three-column
    /// board blit; marbles never draw it.
    board_cards: Vec<crate::habitat::BoardCardTick>,
}

/// Any Planet drop (stale teardown, marble-budget cap, map clear) ends its
/// song — stop + release, never an orphaned player.
impl Drop for Planet {
    fn drop(&mut self) {
        audio::stop_audio(self);
    }
}

/// What a mouse-down grabbed: the whole planet (move it) or one decor item
/// (drag it somewhere else on the disc).
enum Drag {
    Panel {
        project_id: String,
        grab_dx: f64,
        grab_dy: f64,
        start: NSPoint,
        moved: f64,
    },
    Decor {
        project_id: String,
        decor: DecorRef,
        start: NSPoint,
        moved: f64,
    },
    Walker {
        project_id: String,
        entry_id: String,
        start: NSPoint,
        moved: f64,
    },
}

/// After "Place plant/mushroom/boombox" the next disc click drops it.
#[derive(Clone, Debug)]
struct Placement {
    project_id: String,
    kind: DecorKind,
}

/// Sprite/dot mouse-down pending its mouse-up click check. `walker`
/// distinguishes stowed sprites from open-window dots — single vs double
/// click mean different things on each.
struct PendingClick {
    project_id: String,
    entry_id: String,
    walker: bool,
}

#[derive(Default)]
pub(crate) struct Habitat {
    planets: HashMap<String, Planet>,
    worlds: HashMap<String, WorldState>,
    worlds_loaded: bool,
    /// Set by scroll-zoom / decor / boombox mutations; cursor_tick persists
    /// at 10Hz so a scroll gesture can't thrash the disk.
    worlds_dirty: bool,
    drag: Option<Drag>,
    pending_click: Option<PendingClick>,
    /// WORLD-TOUCH leftover: mouse-down on the planet board's Done column.
    pending_board_done: Option<String>,
    /// GO-ORBITTY-RAIL-ROW-MENU: the locate stroke — (entry id, armed at).
    /// Drawn as a ~500ms ring around the walker's frame; cursor_tick
    /// expires it (10Hz, so Reduce Motion can't pin it on).
    locate: Option<(String, Instant)>,
    /// Dest-menu find: (project id, armed at). Marble = diagonal saturn
    /// ring; expanded = rim glow + center name fade.
    locate_planet: Option<(String, Instant)>,
    /// Armed by the Place menu items; the next empty-disc click drops it.
    placement: Option<Placement>,
    bob_phase: f64,
    last_anim: Option<Instant>,
    reduce_motion: bool,
    /// GO-ORBITTY-MARBLE-DRIFT: Settings → Appearance toggle, mirrored
    /// from the rail's snapshot (default false = marbles sit).
    marble_drift: bool,
    /// GO-ORBITTY-MARBLE-SOLAR: frozen barycenter homes, keyed by the
    /// bary sun_key (BARY_PREFIX + sorted member ids). In-memory only;
    /// entries are dropped the tick after their cluster dissolves.
    clusters: HashMap<String, ClusterState>,
    /// Shared drift clock (seconds), advanced EVERY habitat tick (~16ms)
    /// while motion is on so drifting marbles don't step at the 3-tick
    /// walker cadence. bob_phase stays the walker clock.
    drift_phase: f64,
    last_drift: Option<Instant>,
    /// Gear walker-size slider (0.5–1.25, WORLD-PROPS cap). 0 = treat as 1.0.
    sprite_scale: f64,
    /// GO-ORBITTY-WALKER-CONTEXT: named destinations for Assign to planet.
    planet_names: Vec<(String, String)>,
    icon_cache: HashMap<String, usize>, // bundleId/appName → NSImage
    /// User-pack art (GO-LS-TAURI-USER-PACKS): (pack folder id, frames) per
    /// pack — walk cycle plus optional idle cycle for paused walkers.
    /// Walkers reference a pack by id via the claim's spritePackId;
    /// empty/unknown id = the generated person. The roster is re-enumerated
    /// on first sync and after any import/remove, but frames stay UNDECODED
    /// until an expanded planet wears the pack (GO-ORBITTY-PLANET-BUDGET).
    pack_frames: Vec<(String, PackFrames)>,
    packs_loaded: bool,
    /// GO-ORBITTY-WORLD-PROPS: bundled town art (Resources/town/*),
    /// loaded once into retained NSImages on the first world draw.
    /// 0 = missing → the cheap glyph fallback draws instead.
    town_igloo: usize,
    town_sign: usize,
    town_art_loaded: bool,
    /// Global walk-cycle counter, +1 per anim tick (20Hz); walkers advance
    /// pack frames at half that rate.
    anim_frame: u64,
    /// GO-ORBITTY-CADENCE-FIX: Done entries with no walker to walk off
    /// (marble planet, parked dot) already fast-forward-emitted
    /// `habitat:walked-off` — once per Done spell, never per sync. Pruned
    /// when the entry leaves Done so a re-Done emits again.
    walkoff_fast: std::collections::HashSet<String>,
}

struct PackFrames {
    walk: Vec<usize>,
    idle: Vec<usize>,
    /// GO-ORBITTY-PLANET-BUDGET lazy packs: the on-disk art source (paths
    /// only). No bitmap is decoded until an expanded planet wears the pack
    /// (`loaded` flips on first wear); the bitmap drops when the last
    /// expanded wearer collapses. Marbles never load a pack.
    art: crate::packs::PackArt,
    loaded: bool,
}

/// xorshift64 for roam pauses (no rand crate on the tree); global so the
/// anim loop can draw numbers while planets are mutably borrowed.
fn rand_unit() -> f64 {
    static RNG: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut g = RNG
        .get_or_init(|| Mutex::new(now_epoch().to_bits() ^ 0x9e37_79b9_7f4a_7c15))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut x = *g;
    if x == 0 {
        x = 0x2545_f491_4f6c_dd1d;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *g = x;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn habitat() -> &'static Mutex<Habitat> {
    static H: OnceLock<Mutex<Habitat>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(Habitat::default()))
}

pub(crate) fn lock() -> std::sync::MutexGuard<'static, Habitat> {
    habitat().lock().unwrap_or_else(|e| e.into_inner())
}

fn app_handle_cell() -> &'static OnceLock<AppHandle> {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    &APP
}

fn app_handle() -> Option<&'static AppHandle> {
    app_handle_cell().get()
}

// ---------- small objc helpers ----------

/// Toll-free-bridged NSString, intentionally leaked (used as a constant).
pub(crate) unsafe fn ns_string(s: &str) -> *mut Object {
    let cf = CFString::new(s);
    let raw = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    raw
}

unsafe fn ns_color(r: f64, g: f64, b: f64, a: f64) -> *mut Object {
    msg_send![class!(NSColor), colorWithCalibratedRed: r green: g blue: b alpha: a]
}

unsafe fn set_fill(color: *mut Object) {
    let _: () = msg_send![color, setFill];
}

unsafe fn fill_oval(rect: NSRect) {
    let path: *mut Object = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![path, fill];
}


/// Stable per-project planet color: golden-angle hue scatter, pastel-leaning.
fn color_for(project_id: &str) -> (f64, f64, f64) {
    let sum: u32 = project_id.bytes().map(|b| b as u32).sum();
    let hue = ((sum as f64) * 137.508) % 360.0 / 360.0;
    hsv(hue, 0.55, 0.80)
}

/// GO-ORBITTY-WORLD-TOWN: stable per-card tick color for the board blit —
/// the same golden-angle idea as color_for, one hue per card id.
pub(crate) fn card_color(card_id: &str) -> (f64, f64, f64) {
    let sum: u32 = card_id.bytes().map(|b| b as u32).sum();
    let hue = ((sum as f64) * 137.508) % 360.0 / 360.0;
    hsv(hue, 0.60, 0.92)
}

fn hsv(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

// ---------- base64 (icons arrive as PNG base64 from the rail) ----------

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in s.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' {
            continue;
        }
        buf = (buf << 6) | val(b)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// NSImage from encoded bytes — initWithData decodes png/gif/webp alike.
unsafe fn ns_image_from_bytes(bytes: &[u8]) -> *mut Object {
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let img: *mut Object = msg_send![class!(NSImage), alloc];
    let img: *mut Object = msg_send![img, initWithData: data];
    img
}


/// Bitmap rep for pixel dimensions / CGImage slicing. Returns null for
/// formats NSBitmapImageRep can't parse (some webp) — callers fall back to
/// whole-image NSImage then.
unsafe fn bitmap_rep_from_bytes(bytes: &[u8]) -> *mut Object {
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return std::ptr::null_mut();
    }
    msg_send![class!(NSBitmapImageRep), imageRepWithData: data]
}

// ---------- user-pack frames (GO-LS-TAURI-USER-PACKS) ----------

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    // Crop in pixel space, origin top-left as displayed — exactly how sprite
    // sheets are laid out (row-major from the top).
    fn CGImageCreateWithImageInRect(image: *const c_void, rect: NSRect) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Called by pack import/remove (any thread): the next sync reloads.
pub fn reload_pack_frames() {
    lock().packs_loaded = false;
}

/// GO-ORBITTY-WORLD-PROPS: load the bundled town PNGs (Resources/town/*)
/// once into retained NSImages — same bundle mapping as the spirit, but
/// drawn by the habitat painter, not the webview. A missing file stays 0
/// so the glyph fallback draws instead of re-reading disk every frame.
/// Runs on the main thread inside drawRect (all AppKit stays main-thread).
pub(crate) fn ensure_town_art(h: &mut Habitat) {
    if h.town_art_loaded {
        return;
    }
    let Some(app) = app_handle() else {
        return;
    };
    let Ok(dir) = app.path().resource_dir() else {
        return;
    };
    h.town_art_loaded = true;
    let dir = dir.join("town");
    for (name, slot) in [
        ("igloo.png", &mut h.town_igloo),
        ("sign.png", &mut h.town_sign),
    ] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            let img = unsafe { ns_image_from_bytes(&bytes) };
            *slot = img as usize;
        }
    }
}

/// Release any decoded pack images and re-enumerate the roster from
/// packs/*/ on disk — PATHS ONLY (GO-ORBITTY-PLANET-BUDGET): no bitmap
/// decodes here. A pack decodes when an expanded planet first wears it
/// (ensure_pack_loaded) and drops when the last expanded wearer collapses
/// (unload_dead_packs). Runs on the main thread inside do_sync (all AppKit
/// stays main-thread).
fn load_pack_frames(h: &mut Habitat) {
    for (_, pack) in h.pack_frames.drain(..) {
        for img in pack.walk.into_iter().chain(pack.idle) {
            unsafe {
                let _: () = msg_send![img as *mut Object, release];
            }
        }
    }
    let Some(app) = app_handle() else {
        return;
    };
    for (id, art) in crate::packs::pack_art(app) {
        h.pack_frames.push((
            id,
            PackFrames {
                walk: Vec::new(),
                idle: Vec::new(),
                art,
                loaded: false,
            },
        ));
    }
}

/// GO-ORBITTY-PLANET-BUDGET: decode a pack's frames on first expanded
/// wear. Idempotent; a failed decode stays failed (walk empty → the
/// generated being draws) instead of re-reading disk every sync.
fn ensure_pack_loaded(h: &mut Habitat, idx: usize) {
    let Some((_, pf)) = h.pack_frames.get_mut(idx) else {
        return;
    };
    if pf.loaded {
        return;
    }
    pf.loaded = true;
    let walk = unsafe { load_source(&pf.art.walk) };
    let idle = match pf.art.idle.take() {
        Some(src) => unsafe { load_source(&src) },
        None => Vec::new(),
    };
    pf.walk = walk;
    pf.idle = idle;
}

/// GO-ORBITTY-PLANET-BUDGET: drop a decoded pack's bitmaps once no
/// expanded planet wears it (the last wearer collapsed or left). The art
/// paths stay, so a re-expand re-decodes. Cheap: a per-sync scan of the
/// expanded walkers' pack indices.
fn unload_dead_packs(h: &mut Habitat) {
    let mut used: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for p in h.planets.values() {
        if p.is_marble {
            continue;
        }
        for b in &p.walkers {
            if let Some(idx) = b.pack {
                used.insert(idx);
            }
        }
    }
    for (idx, (_, pf)) in h.pack_frames.iter_mut().enumerate() {
        if pf.loaded && !used.contains(&idx) {
            for img in pf.walk.drain(..).chain(pf.idle.drain(..)) {
                unsafe {
                    let _: () = msg_send![img as *mut Object, release];
                }
            }
            pf.loaded = false;
        }
    }
}

unsafe fn load_source(src: &crate::packs::ArtSource) -> Vec<usize> {
    match src {
        crate::packs::ArtSource::Frames(paths) => load_frame_files(paths),
        crate::packs::ArtSource::Sheet { file, spec } => slice_sheet(file, spec),
    }
}

/// A walker's frame cap — the cycle never uses more than 8.
const MAX_WALK_FRAMES: usize = 8;

/// Load image files as walker frames. A horizontal strip sheet (width an
/// exact multiple of height — the classic itch layout, e.g. Hero Knight's
/// 1440×180 Run.png) is sliced into ≤8 square frames so it actually walks;
/// anything else is one static frame. Stops at MAX_WALK_FRAMES total.
unsafe fn load_frame_files(paths: &[std::path::PathBuf]) -> Vec<usize> {
    let mut out = Vec::new();
    for p in paths {
        if out.len() >= MAX_WALK_FRAMES {
            break;
        }
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let rep = bitmap_rep_from_bytes(&bytes);
        if !rep.is_null() {
            let width: isize = msg_send![rep, pixelsWide];
            let height: isize = msg_send![rep, pixelsHigh];
            if height > 0 && width > height && width % height == 0 {
                let remaining = (MAX_WALK_FRAMES - out.len()) as isize;
                let n = (width / height).min(remaining);
                out.extend(slice_rep_frames(rep, height, height, n));
                continue;
            }
        }
        let img = ns_image_from_bytes(&bytes);
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}

/// Slice a sprite sheet into retained NSImages, row-major from the top-left,
/// at most `spec.count` frames (fewer when the bitmap is short).
unsafe fn slice_sheet(path: &std::path::Path, spec: &crate::packs::SheetSpec) -> Vec<usize> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let rep = bitmap_rep_from_bytes(&bytes);
    if rep.is_null() {
        return Vec::new();
    }
    let fw = spec.frame_w as isize;
    let fh = spec.frame_h as isize;
    let width: isize = msg_send![rep, pixelsWide];
    let height: isize = msg_send![rep, pixelsHigh];
    if fw <= 0 || fh <= 0 || width < fw || height < fh {
        return Vec::new();
    }
    slice_rep_frames(rep, fw, fh, spec.count as isize)
}

/// Cut `total` frames of fw×fh out of a bitmap rep, row-major from the
/// top-left (fewer when the bitmap is short).
unsafe fn slice_rep_frames(rep: *mut Object, fw: isize, fh: isize, total: isize) -> Vec<usize> {
    let cg: *const c_void = msg_send![rep, CGImage];
    if cg.is_null() || fw <= 0 || fh <= 0 {
        return Vec::new();
    }
    let width: isize = msg_send![rep, pixelsWide];
    let height: isize = msg_send![rep, pixelsHigh];
    let cols = (width / fw).max(1);
    let rows = (height / fh).max(1);
    let total = total.min(cols * rows).max(0);
    let mut out = Vec::new();
    for i in 0..total {
        let x = (i % cols) * fw;
        let y = (i / cols) * fh;
        let sub = CGImageCreateWithImageInRect(
            cg,
            NSRect::new(x as f64, y as f64, fw as f64, fh as f64),
        );
        if sub.is_null() {
            continue;
        }
        let frame_rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
        let frame_rep: *mut Object = msg_send![frame_rep, initWithCGImage: sub];
        CFRelease(sub);
        if frame_rep.is_null() {
            continue;
        }
        let img: *mut Object = msg_send![class!(NSImage), alloc];
        let img: *mut Object = msg_send![img, init];
        let _: () = msg_send![img, addRepresentation: frame_rep];
        let _: () = msg_send![frame_rep, release];
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}


// ---------- being-editor preview (GO-ORBITTY-CHARS) ----------

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

pub fn char_preview(look: crate::chars::CharLook, px: u32) -> Result<String, String> {
    let _ = px;
    if let Some(bytes) = bible_preview_png(&look) {
        return Ok(base64_encode(&bytes));
    }
    char_preview_grid(look, px)
}

fn bible_preview_png(look: &crate::chars::CharLook) -> Option<Vec<u8>> {
    let home = std::env::var("HOME").ok()?;
    let p = std::path::PathBuf::from(home)
        .join("Library/Application Support/space.orbitty.mac/packs")
        .join(crate::chars::default_pack(look))
        .join("walk_0.png");
    std::fs::read(p).ok()
}

/// Last-resort walker art when the resolved pack id isn't loaded (pack
/// removed between launches, seeding raced the first sync): the default
/// Boy bitty, so the planet never falls through to the glyph grid while
/// any bitty pack exists on disk.
fn fallback_pack_index(h: &Habitat) -> Option<usize> {
    h.pack_frames
        .iter()
        .position(|(id, _)| id == "orbitty-bitty-boy")
}

/// Rasterize a generated person to a base64 PNG for the Being editor's
/// preview — fallback if the Imagine pack is missing.
fn char_preview_grid(look: crate::chars::CharLook, px: u32) -> Result<String, String> {
    let px = px.clamp(2, 16) as usize;
    let width = crate::chars::W * px;
    let height = crate::chars::H * px;
    let grid = crate::chars::grid(&look, 0);
    let pal = crate::chars::palette(&look);
    unsafe {
        // Commands arrive on an IPC worker thread with no pool in place;
        // the AppKit objects below are autoreleased, so bracket the work.
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let out = (|| {
            let rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
            let rep: *mut Object = msg_send![rep,
                initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
                pixelsWide: width as isize
                pixelsHigh: height as isize
                bitsPerSample: 8isize
                samplesPerPixel: 4isize
                hasAlpha: true
                isPlanar: false
                colorSpaceName: ns_string("NSCalibratedRGBColorSpace")
                bytesPerRow: (width * 4) as isize
                bitsPerPixel: 32isize];
            if rep.is_null() {
                return Err("preview: bitmap alloc failed".into());
            }
            let data: *mut u8 = msg_send![rep, bitmapData];
            if data.is_null() {
                let _: () = msg_send![rep, release];
                return Err("preview: no bitmap buffer".into());
            }
            std::ptr::write_bytes(data, 0, width * height * 4);
            let ctx: *mut Object =
                msg_send![class!(NSGraphicsContext), graphicsContextWithBitmapImageRep: rep];
            if ctx.is_null() {
                let _: () = msg_send![rep, release];
                return Err("preview: no graphics context".into());
            }
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: ctx];
            // Bitmap reps are y-up (the world view is flipped): grid row 0 is
            // the person's TOP, so paint it at the highest y.
            for (row, line) in grid.iter().enumerate() {
                for (col, ch) in line.iter().enumerate() {
                    let Some(c) = crate::chars::glyph_color(*ch, &pal) else {
                        continue;
                    };
                    set_fill(ns_color(c.0, c.1, c.2, 1.0));
                    let cell = NSRect::new(
                        col as f64 * px as f64,
                        (crate::chars::H - 1 - row) as f64 * px as f64,
                        px as f64,
                        px as f64,
                    );
                    let _: () = msg_send![class!(NSBezierPath), fillRect: cell];
                }
            }
            let _: () = msg_send![ctx, flushGraphics];
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: std::ptr::null::<Object>()];
            // NSPNGFileType = 4.
            let png: *mut Object =
                msg_send![rep, representationUsingType: 4usize properties: std::ptr::null::<Object>()];
            let out = if png.is_null() {
                Err("preview: PNG encode failed".into())
            } else {
                let len: usize = msg_send![png, length];
                let ptr: *const u8 = msg_send![png, bytes];
                if ptr.is_null() || len == 0 {
                    Err("preview: empty PNG".into())
                } else {
                    Ok(base64_encode(std::slice::from_raw_parts(ptr, len)))
                }
            };
            let _: () = msg_send![rep, release];
            out
        })();
        let _: () = msg_send![pool, drain];
        out
    }
}

// ---------- screens ----------

unsafe fn screens() -> Vec<(String, NSRect)> {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![arr, count];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, frame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: ns_string("NSScreenNumber")]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        out.push((id, frame));
    }
    out
}

/// visibleFrame excludes the menu bar / Dock — drift clamps to it so a
/// marble never wanders under OS chrome. (`screens()` keeps returning the
/// full frame; persisted coords round-trip through that.)
pub(crate) unsafe fn visible_frames() -> Vec<(String, NSRect)> {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![arr, count];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, visibleFrame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: ns_string("NSScreenNumber")]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        out.push((id, frame));
    }
    out
}

fn primary_screen_id() -> Option<String> {
    unsafe { screens().into_iter().next().map(|(id, _)| id) }
}

/// Stored coords are screen-local, flipped (y down) — clamp on-screen.
fn panel_frame(state: &WorldState, side: f64) -> NSRect {
    let all = unsafe { screens() };
    let fallback = NSRect::new(0.0, 0.0, 1440.0, 900.0);
    let (_, frame) = all
        .iter()
        .find(|(id, _)| id == &state.screen_id)
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))
        .unwrap_or_else(|| (String::new(), fallback));
    let lo = side / 2.0 + 8.0;
    let cx = state.x.clamp(lo, (frame.size.width - side / 2.0 - 8.0).max(lo));
    let cy = state.y.clamp(lo, (frame.size.height - side / 2.0 - 8.0).max(lo));
    NSRect::new(
        frame.origin.x + cx - side / 2.0,
        frame.origin.y + frame.size.height - cy - side / 2.0,
        side,
        side,
    )
}

// ---------- spawn ring (GO-ORBITTY-PLANET-SPAWN) ----------
//
// New worlds appear on a ring around the rail, not mid-right of the
// display. Center = the `main` window's outer-frame center, converted into
// the same screen-local flipped (y-down) coords WorldState.x/y already
// use. Radius = half the rail's outer width + 80pt. First slot is
// 3 o'clock, then counterclockwise in 45° steps; if the rail sits on the
// right third of its screen (or 3 o'clock would clip the visibleFrame)
// the walk mirrors — 9 o'clock first, then clockwise — so the first
// visible slot is always inboard. A slot is skipped when the expanded
// disc (EXPANDED_RADIUS, not the marble) would clip that screen's
// visibleFrame. Existing worlds never move and do not occupy slots; only
// worlds already sitting on the ring (spawned here this session) count as
// claimed. Rail missing (headless test) → mid-right fallback, no panic.

const RING_SLOTS: usize = 8;
const RING_CLEARANCE: f64 = 80.0;

struct SpawnRing {
    screen_id: String,
    cx: f64,
    cy: f64,
    radius: f64,
    mirrored: bool,
    visible: [bool; RING_SLOTS],
}

fn ring_slot(ring: &SpawnRing, k: usize) -> (f64, f64) {
    let step = std::f64::consts::FRAC_PI_4 * (k as f64);
    // y is down, so visual up is negative y: both walks go up first.
    let (dx, dy) = if ring.mirrored {
        (-step.cos(), -step.sin()) // 9 o'clock, then clockwise
    } else {
        (step.cos(), -step.sin()) // 3 o'clock, then counterclockwise
    };
    (ring.cx + ring.radius * dx, ring.cy + ring.radius * dy)
}

fn spawn_ring() -> Option<SpawnRing> {
    let window = app_handle()?.get_webview_window("main")?;
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    // Tauri reports physical pixels from the primary screen's top-left
    // (y down); AppKit frames are points from its bottom-left (y up).
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    let cx_t = pos.x as f64 / scale + w / 2.0;
    let cy_t = pos.y as f64 / scale + h / 2.0;
    let all = unsafe { screens() };
    let primary_h = all.first().map(|(_, f)| f.size.height)?;
    let center = NSPoint {
        x: cx_t,
        y: primary_h - cy_t,
    };
    let (screen_id, sf) = all
        .iter()
        .find(|(_, f)| f.contains(center))
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))?;
    let cx = center.x - sf.origin.x;
    let cy = sf.size.height - (center.y - sf.origin.y);
    let visible = unsafe { visible_frames() };
    let vf = visible
        .iter()
        .find(|(id, _)| id == &screen_id)
        .map(|(_, f)| *f)
        .unwrap_or(sf);
    // visibleFrame as a y-down rect in screen-local coords.
    let vx0 = vf.origin.x - sf.origin.x;
    let vy0 = sf.size.height - (vf.origin.y + vf.size.height - sf.origin.y);
    let clips = |sx: f64, sy: f64| {
        sx - EXPANDED_RADIUS < vx0
            || sx + EXPANDED_RADIUS > vx0 + vf.size.width
            || sy - EXPANDED_RADIUS < vy0
            || sy + EXPANDED_RADIUS > vy0 + vf.size.height
    };
    let radius = w / 2.0 + RING_CLEARANCE;
    let mirrored = cx > sf.size.width * 2.0 / 3.0 || clips(cx + radius, cy);
    let mut ring = SpawnRing {
        screen_id,
        cx,
        cy,
        radius,
        mirrored,
        visible: [false; RING_SLOTS],
    };
    for k in 0..RING_SLOTS {
        let (sx, sy) = ring_slot(&ring, k);
        ring.visible[k] = !clips(sx, sy);
    }
    Some(ring)
}

/// Worlds already sitting on a visible ring slot — only those occupy
/// slots; pre-existing (mid-right or dragged) worlds are not on the ring.
fn ring_claimed(ring: &SpawnRing, worlds: &HashMap<String, WorldState>) -> usize {
    worlds
        .values()
        .filter(|w| {
            w.screen_id == ring.screen_id
                && (0..RING_SLOTS).any(|k| {
                    if !ring.visible[k] {
                        return false;
                    }
                    let (sx, sy) = ring_slot(ring, k);
                    (w.x - sx).abs() < 1.0 && (w.y - sy).abs() < 1.0
                })
        })
        .count()
}

/// New worlds land on the spawn ring around the rail; `existing_on_screen`
/// is the count of ring slots already claimed this session. Headless
/// fallback: mid-right, clearly on-screen, staggered per existing world.
fn default_state(screen_id: &str, existing_on_screen: usize) -> WorldState {
    let placed = spawn_ring().and_then(|ring| {
        let open: Vec<usize> = (0..RING_SLOTS).filter(|&k| ring.visible[k]).collect();
        if open.is_empty() {
            return None;
        }
        let lap = existing_on_screen / open.len();
        let (sx, sy) = ring_slot(&ring, open[existing_on_screen % open.len()]);
        // Past a full lap, ride the same slots a touch outward so a ninth
        // world never lands exactly on the first.
        let grow = lap as f64 * 96.0;
        let dx = (sx - ring.cx) / ring.radius;
        let dy = (sy - ring.cy) / ring.radius;
        Some((ring.screen_id.clone(), sx + dx * grow, sy + dy * grow))
    });
    let (screen_id, x, y) = match placed {
        Some(p) => p,
        None => {
            let all = unsafe { screens() };
            let size = all
                .iter()
                .find(|(id, _)| id == screen_id)
                .or_else(|| all.first())
                .map(|(_, f)| f.size)
                .unwrap_or(CGSize {
                    width: 1440.0,
                    height: 900.0,
                });
            (
                screen_id.to_string(),
                size.width * 0.72 + (existing_on_screen as f64) * 40.0,
                size.height * 0.45,
            )
        }
    };
    WorldState {
        screen_id,
        x,
        y,
        last_used: now_epoch(),
        radius: 0.0,
        plant: false,
        mushroom: false,
        boombox: false,
        music: String::new(),
        boombox_volume: 1.0,
        projection: "oblique".into(),
        first_seen: now_epoch(),
        plants: Vec::new(),
        mushrooms: Vec::new(),
        buildings: Vec::new(),
        boombox_pos: None,
    }
}

fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ---------- worlds.json ----------

fn worlds_path() -> Option<std::path::PathBuf> {
    let app = app_handle()?;
    let dir = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("worlds.json"))
}

fn load_worlds(h: &mut Habitat) {
    if h.worlds_loaded {
        return;
    }
    h.worlds_loaded = true;
    if let Some(path) = worlds_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(file) = serde_json::from_str::<WorldsFile>(&text) {
                h.worlds = file.worlds;
            }
        }
    }
    // Pre-GROW saves: bool toggles → placements, first_seen stamped.
    let now = now_epoch();
    let mut dirty = false;
    for w in h.worlds.values_mut() {
        dirty |= migrate_world(w, now);
    }
    if dirty {
        save_worlds(h);
    }
}

fn save_worlds(h: &Habitat) {
    let Some(path) = worlds_path() else { return };
    let file = WorldsFile {
        worlds: h.worlds.clone(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Expanded radius: the world's scroll-zoomed value when set, else the
/// default. Marbles are always MARBLE_RADIUS regardless of zoom.
fn effective_radius(world: Option<&WorldState>, collapsed: bool) -> f64 {
    if collapsed {
        return MARBLE_RADIUS;
    }
    match world {
        Some(w) if w.radius > 0.0 => w.radius.clamp(MIN_RADIUS, max_radius()),
        _ => EXPANDED_RADIUS,
    }
}

// ---------- the world view class ----------

/// None means the class couldn't be registered; callers skip planet
/// creation so a habitat failure leaves the rail usable.
fn world_view_class() -> Option<&'static Class> {
    static CLS: OnceLock<Option<&'static Class>> = OnceLock::new();
    CLS.get_or_init(|| {
        // add_method asserts on any mismatch — registration must never abort
        // the app, so it runs under catch_unwind.
        catch_unwind(AssertUnwindSafe(|| unsafe { register_world_view_class() }))
            .unwrap_or_else(|p| {
                eprintln!("habitat: world view class registration panicked: {}", panic_msg(&*p));
                None
            })
    })
    .as_ref()
    .copied()
}

unsafe fn register_world_view_class() -> Option<&'static Class> {
    // Already registered (unit tests, re-init) — reuse, never panic.
    if let Some(cls) = Class::get("LiveSpriteWorldView") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("LiveSpriteWorldView", class!(NSView))?;
    // drawRect:'s dirty-rect arg is part of the signature (objc's add_method
    // asserts selector/fn arg counts match); the body ignores it and always
    // redraws the whole view.
    decl.add_method(
        sel!(drawRect:),
        draw::draw_rect as extern "C" fn(&Object, Sel, NSRect),
    );
    decl.add_method(
        sel!(mouseDown:),
        input::mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(mouseUp:),
        input::mouse_up as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(sel!(isFlipped), is_flipped as extern "C" fn(&Object, Sel) -> bool);
    decl.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, *mut Object) -> bool,
    );
    decl.add_method(
        sel!(scrollWheel:),
        input::scroll_wheel as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(rightMouseDown:),
        input::right_mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

/// Target for the planet right-click menu. A single leaked instance lives
/// forever (NSMenuItem does not retain its target).
fn menu_target() -> Option<usize> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    TARGET
        .get_or_init(|| {
            catch_unwind(AssertUnwindSafe(|| unsafe {
                let cls = register_menu_target_class()?;
                let obj: *mut Object = msg_send![cls, new];
                if obj.is_null() {
                    None
                } else {
                    Some(obj as usize)
                }
            }))
            .unwrap_or_else(|p| {
                eprintln!("habitat: menu target registration panicked: {}", panic_msg(&*p));
                None
            })
        })
        .as_ref()
        .copied()
}

unsafe fn register_menu_target_class() -> Option<&'static Class> {
    if let Some(cls) = Class::get("OrbittyPlanetMenuTarget") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyPlanetMenuTarget", class!(NSObject))?;
    decl.add_method(
        sel!(planetMinimize:),
        input::menu_minimize as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRestore:),
        input::menu_restore as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetTogglePlant:),
        input::menu_toggle_plant as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleMushroom:),
        input::menu_toggle_mushroom as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleBoombox:),
        input::menu_toggle_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetChooseMusic:),
        audio::menu_choose_music as extern "C" fn(&Object, Sel, *mut Object),
    );
    // GO-ORBITTY-BOOMBOX: boombox sprite menu + volume steps.
    decl.add_method(
        sel!(planetBoomboxPlayPause:),
        input::menu_boombox_play_pause as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRemoveBoombox:),
        input::menu_remove_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetBoomboxVolume:),
        input::menu_boombox_volume as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetPlaceBuilding:),
        input::menu_place_building as extern "C" fn(&Object, Sel, *mut Object),
    );
    // GO-ORBITTY-RAIL-ROW-MENU: walker right-click menu actions.
    decl.add_method(
        sel!(walkerUseImage:),
        input::walker_use_image as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerUseFolder:),
        input::walker_use_folder as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerSetPack:),
        input::walker_set_pack as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerOpenBeing:),
        input::walker_open_being as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerAssign:),
        input::walker_assign as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

use objc::runtime::Sel;

extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> bool {
    true
}

extern "C" fn accepts_first_mouse(_this: &Object, _cmd: Sel, _event: *mut Object) -> bool {
    true
}


// drawRect body lives in draw.rs (GO-ORBITTY-CLEAN-HABITAT-DRAW).
const CAPTION_TTL: Duration = Duration::from_secs(2);

/// Locate stroke lifetime (GO-ORBITTY-RAIL-ROW-MENU): rail row left-click
/// and walker left-click both glow ~500ms, then the ring draws out.
const LOCATE_TTL: Duration = Duration::from_millis(500);

/// Planet find glow (dest-menu click). Longer than walker locate so the
/// expanded name can flash and fade.
const PLANET_LOCATE_TTL: Duration = Duration::from_millis(1400);


fn plant_cap(first_seen: f64, now: f64) -> usize {
    let extra = ((now - first_seen).max(0.0) / 172_800.0).floor() as usize;
    (3 + extra).min(5)
}

fn building_cap(first_seen: f64, now: f64) -> usize {
    let extra = ((now - first_seen).max(0.0) / 259_200.0).floor() as usize;
    (1 + extra).min(3)
}

fn layout_decor(planet: &mut Planet) {
    planet.decor_frames.clear();
    if planet.is_marble {
        return;
    }
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let size = (planet.radius * 0.22).clamp(14.0, 28.0);
    for (i, d) in planet.plants.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Plant,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    for (i, d) in planet.mushrooms.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Mushroom,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    for (i, d) in planet.buildings.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Building,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    if let Some(b) = planet.boombox_pos {
        let p = disc_to_view(center, planet.radius, planet.projection, b.x, b.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Boombox,
                index: 0,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
}

/// Native caption pill on the planet for ~2s (titles only), drawn on the
/// next display pass.
pub(crate) fn set_caption(planet: &mut Planet, text: String) {
    planet.caption = Some((text, Instant::now()));
    unsafe {
        let view = planet.view as *mut Object;
        let _: () = msg_send![view, setNeedsDisplay: true];
    }
}

pub(crate) unsafe fn sender_project_id(sender: *mut Object) -> Option<String> {
    let obj: *mut Object = msg_send![sender, representedObject];
    if obj.is_null() {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![obj, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
}


/// World entry for menu/scroll mutations; created defensively if missing.
pub(crate) fn world_entry<'a>(h: &'a mut Habitat, project_id: &str) -> &'a mut WorldState {
    h.worlds
        .entry(project_id.to_string())
        .or_insert_with(|| default_state(&primary_screen_id().unwrap_or_default(), 0))
}


// ---------- panels ----------

unsafe fn make_panel() -> *mut Object {
    let panel: *mut Object = msg_send![class!(NSPanel), alloc];
    // NSWindowStyleMaskBorderless (0) | NSWindowStyleMaskNonactivatingPanel (1<<7)
    let panel: *mut Object = msg_send![panel,
        initWithContentRect: NSRect::new(0.0, 0.0, 10.0, 10.0)
        styleMask: 128usize
        backing: 2usize // NSBackingStoreBuffered
        defer: false];
    // Peer surface at the normal window level: other apps can cover the
    // planet later; it neither floats forever nor hides under everything.
    let _: () = msg_send![panel, setLevel: 0i64];
    // canJoinAllSpaces (1) | stationary (1<<4) | fullScreenAuxiliary (1<<8)
    let _: () = msg_send![panel, setCollectionBehavior: 273usize];
    let _: () = msg_send![panel, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: false]; // draw_world paints its own
    // Click-through by default; the cursor watcher flips this off only while
    // the pointer is over the planet disc or a sprite/dot.
    let _: () = msg_send![panel, setIgnoresMouseEvents: true];
    let _: () = msg_send![panel, setAcceptsMouseMovedEvents: true];
    let _: () = msg_send![panel, setReleasedWhenClosed: false];
    let _: () = msg_send![panel, setHidesOnDeactivate: false];
    // Show Desktop stay-visible: planets are not hidable app chrome.
    let _: () = msg_send![panel, setCanHide: false];
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: true];
    panel
}

/// Walkers orbit with the planet's spin — this is the "walking the surface."
/// Dots sit on a static inner ring. View-local flipped coords.
fn layout_beings(planet: &mut Planet) {
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let ids: Vec<String> = planet.walkers.iter().map(|b| b.entry_id.clone()).collect();
    let lip = draw::board_near_lip(planet.radius, planet.projection, planet.marble_scale);
    for (i, id) in ids.iter().enumerate() {
        let pose = planet.walkers.get(i).map(|b| b.pose).unwrap_or(Pose::Roam);
        // WORLD-PLACES: a fresh board-pose roam starts at the house.
        let house = house_spawn(planet, id);
        planet.roam.entry(id.clone()).or_insert_with(|| {
            // GO-ORBITTY-WORLD-TOWN: a fresh roam for a board pose seeds
            // AT THE IGLOO (STATION_HOUSE — user buildings stay décor)
            // and walks to the station — the world narrates
            // igloo → board/field instead of popping in at the column.
            // Sit still seeds AT the front rim (bad news skips the
            // queue). A Done walker seeds AT THE IGLOO TOO and walks
            // to the near lip of the board (WORLD-STOP). Live card
            // moves keep the existing roam entry, so those walks stay
            // visible.
            match pose {
                Pose::Stand | Pose::Work => {
                    let (tx, ty) = station_target(pose, id);
                    RoamState {
                        x: house.0,
                        y: house.1,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                    }
                }
                Pose::Sit => {
                    let (tx, ty) = station_target(pose, id);
                    RoamState {
                        x: tx,
                        y: ty,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                    }
                }
                Pose::WalkOff => RoamState {
                    x: house.0,
                    y: house.1,
                    tx: lip.0,
                    ty: lip.1,
                    pause_end: None,
                    facing_left: false,
                    off_emitted: false,
                    via_board: false,
                    via_home: false,
                    via_checkin: false,
                    checkin_emit_at: None,
                },
                Pose::Roam => {
                    let a = i as f64 * 2.3;
                    let r = 0.32;
                    RoamState {
                        x: a.cos() * r,
                        y: a.sin() * r,
                        tx: a.cos() * r,
                        ty: a.sin() * r,
                        pause_end: None,
                        facing_left: false,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                    }
                }
            }
        });
    }
    let live: std::collections::HashSet<String> = ids.iter().cloned().collect();
    planet.roam.retain(|k, _| live.contains(k));
    for b in planet.walkers.iter_mut() {
        // Per-claim size (GO-ORBITTY-CHARS) multiplies the gear's global
        // slider; the hit box matches the drawn size.
        let fit = walker_fit(planet.radius, planet.sprite_scale * b.user_scale);
        let (x, y) = planet
            .roam
            .get(&b.entry_id)
            .map(|r| (r.x, r.y))
            .unwrap_or((0.0, 0.0));
        let pt = disc_to_view(center, planet.radius, planet.projection, x, y);
        b.frame = NSRect::new(pt.x - fit / 2.0, pt.y - fit / 2.0, fit, fit);
    }
    let dot_ring = planet.radius * 0.55;
    let dcount = planet.dots.len().max(1) as f64;
    for (i, b) in planet.dots.iter_mut().enumerate() {
        let angle = -std::f64::consts::FRAC_PI_2
            + (i as f64) * (std::f64::consts::TAU / dcount)
            + std::f64::consts::PI / 7.0;
        b.frame = NSRect::new(
            center.x + angle.cos() * dot_ring - DOT_SIZE / 2.0,
            center.y + angle.sin() * dot_ring - DOT_SIZE / 2.0,
            DOT_SIZE,
            DOT_SIZE,
        );
    }
    layout_decor(planet);
}

fn make_being(h: &mut Habitat, entry: &HabitatEntry) -> Being {
    let mut icon = 0usize;
    if let Some(b64) = entry.icon_base64.as_deref() {
        let key = entry
            .bundle_id
            .clone()
            .unwrap_or_else(|| entry.app_name.clone());
        if let Some(&ptr) = h.icon_cache.get(&key) {
            icon = ptr;
        } else if let Some(bytes) = base64_decode(b64) {
            let img = unsafe { ns_image_from_bytes(&bytes) };
            if !img.is_null() {
                h.icon_cache.insert(key, img as usize);
                icon = img as usize;
            }
        }
    }
    let mut label = if entry.title.is_empty() {
        entry.app_name.clone()
    } else {
        format!("{} — {}", entry.app_name, entry.title)
    };
    // Captions stay one short line.
    if label.chars().count() > 56 {
        label = label.chars().take(55).collect::<String>() + "…";
    }
    // GO-ORBITTY-CHARS: explicit editor picks win; otherwise the persisted
    // seed derives the same person every launch.
    let look = entry
        .char_look
        .map(|mut l| {
            crate::chars::normalize(&mut l);
            l
        })
        .unwrap_or_else(|| crate::chars::look_from_seed(entry.char_seed));
    // Sprite picker: a user-picked pack wins. Otherwise the Season 1 bitty
    // for this look's Kind+Palette (GO-ORBITTY-S1-BITTYS) — the planet's
    // default population is the blessed bitmap crew, never the glyph grid.
    let pack_id = if entry.sprite_pack_id.is_empty() {
        crate::chars::default_pack(&look)
    } else {
        entry.sprite_pack_id.clone()
    };
    let pack = h
        .pack_frames
        .iter()
        .position(|(id, _)| id == &pack_id)
        .or_else(|| fallback_pack_index(h));
    let user_scale = if entry.sprite_scale <= 0.0 {
        1.0
    } else {
        entry.sprite_scale.clamp(0.5, 1.25)
    };
    // Rail Done AX-closes the window, so the walker is asleep during
    // WalkOff. Sleeper dim (0.45) is for a closed window standing still —
    // checkout stays full-bright until they actually leave.
    let pose = pose_for(entry);
    Being {
        entry_id: entry.id.clone(),
        label,
        frame: NSRect::default(),
        bob: 0.0,
        icon,
        look,
        user_scale,
        pack,
        dim: if pose == Pose::WalkOff {
            1.0
        } else if entry.asleep || entry.age == "fade" || entry.age == "gone" {
            0.45
        } else {
            1.0
        },
        asleep: entry.asleep,
        sprite_pack_id: entry.sprite_pack_id.clone(),
        pose,
    }
}

// ---------- sync (port of OverlayController.relayout) ----------

/// True on the AppKit main thread. The Tauri command handler for
/// `sync_habitat` runs there, so this is the common path.
fn on_main_thread() -> bool {
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

pub fn sync(app: &AppHandle, snapshot: HabitatSnapshot) -> Result<(), String> {
    // CADENCE-FIX: already on main → run inline. The old code queued the
    // closure with run_on_main_thread and then sat in recv_timeout(2s) —
    // on the main thread that is main waiting for a closure queued behind
    // itself, so every board drag beach-balled for the full 2s before the
    // sync landed.
    if on_main_thread() {
        return match catch_unwind(AssertUnwindSafe(|| do_sync(&snapshot))) {
            Ok(()) => Ok(()),
            Err(p) => {
                let msg = panic_msg(&*p);
                eprintln!("habitat: sync panicked (caught): {msg}");
                Err(format!("habitat sync failed: {msg}"))
            }
        };
    }
    let app = app.clone();
    // Off main (hypothetical IPC caller): keep the hop, but never block
    // main — recv_timeout on a worker thread is harmless.
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = catch_unwind(AssertUnwindSafe(|| do_sync(&snapshot)));
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(p)) => {
            let msg = panic_msg(&*p);
            eprintln!("habitat: sync panicked (caught): {msg}");
            Err(format!("habitat sync failed: {msg}"))
        }
        Err(_) => Err("habitat sync: no result from main thread".to_string()),
    }
}

fn do_sync(snapshot: &HabitatSnapshot) {
    // Pose transitions are collected under the lock and appended to the
    // event log AFTER the lock drops, on a background thread — jsonl IO
    // never runs on the AppKit thread inside the habitat lock (that was
    // the second half of the board-drag beach ball). A missed pose line
    // beats a stalled UI.
    let mut pose_events: Vec<(String, String, Pose, Pose)> = Vec::new();
    let mut h = lock();
    load_worlds(&mut h);
    // Re-arm the once-only Done fast-forward: the set tracks entries that
    // already emitted; an entry whose card leaves Done (uncheck, new open
    // card) drops out so its next Done emits again.
    {
        let done_ids: std::collections::HashSet<&str> = snapshot
            .planets
            .iter()
            .flat_map(|p| p.entries.iter())
            .filter(|e| pose_for(e) == Pose::WalkOff)
            .map(|e| e.id.as_str())
            .collect();
        h.walkoff_fast.retain(|id| done_ids.contains(id.as_str()));
    }
    let scale = if snapshot.sprite_scale <= 0.0 {
        1.0
    } else {
        snapshot.sprite_scale.clamp(0.5, 1.25)
    };
    h.sprite_scale = scale;
    // A drift toggle flip persists live marble x/y at once — the orbit
    // path never dirties, so a toggle would otherwise lose the drifted
    // positions until quit.
    if h.marble_drift != snapshot.marble_drift {
        h.worlds_dirty = true;
    }
    h.marble_drift = snapshot.marble_drift;
    h.planet_names = if snapshot.all_planets.is_empty() {
        snapshot
            .planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    } else {
        snapshot
            .all_planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    };
    for planet in h.planets.values_mut() {
        if (planet.sprite_scale - scale).abs() > 0.001 {
            planet.sprite_scale = scale;
            layout_beings(planet);
        }
    }
    // User packs: first sync and every import/remove reloads frame art.
    // A reload re-indexes pack_frames, so Being.pack indices built before
    // it are stale — the pose-only fast path below must not reuse them.
    let packs_reloaded = !h.packs_loaded;
    if !h.packs_loaded {
        h.packs_loaded = true;
        load_pack_frames(&mut h);
    }

    // Tear down panels for projects that left the habitat.
    let incoming: Vec<&str> = snapshot.planets.iter().map(|p| p.project_id.as_str()).collect();
    let stale: Vec<String> = h
        .planets
        .keys()
        .filter(|k| !incoming.contains(&k.as_str()))
        .cloned()
        .collect();
    for project_id in stale {
        if let Some(mut planet) = h.planets.remove(&project_id) {
            audio::stop_audio(&mut planet);
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                let _: () = msg_send![panel, release];
            }
        }
    }

    let Some(primary_id) = primary_screen_id() else {
        return;
    };

    // GO-ORBITTY-PLANET-BUDGET cap 3 expanded: habitat:expand /
    // click-to-expand always succeeds — when a 4th world is expanded, the
    // least-recently focused of the rest takes the marble (stable snapshot
    // order breaks recency ties). The rail owns the collapsed flag: the
    // habitat:collapse echo makes it persist there, and this sync already
    // renders the victim as a marble so a 4th planet never shows. The
    // expand gesture itself is the focus — a marble→expanded transition
    // bumps the world's last_used ledger (drags and scroll-zooms already
    // stamp it), so recency survives restarts via worlds.json.
    let mut force_marble: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let now = now_epoch();
        let mut expanded: Vec<(String, f64, usize)> = Vec::new();
        for (idx, p) in snapshot.planets.iter().enumerate() {
            if p.collapsed {
                continue;
            }
            let just_expanded = h
                .planets
                .get(&p.project_id)
                .is_some_and(|pl| pl.is_marble);
            let recency = if just_expanded || !h.worlds.contains_key(&p.project_id) {
                // A brand-new claim arrives focused; default_state stamps
                // it too when the world entry is created below.
                if let Some(w) = h.worlds.get_mut(&p.project_id) {
                    w.last_used = now;
                    h.worlds_dirty = true;
                }
                now
            } else {
                h.worlds
                    .get(&p.project_id)
                    .map(|w| w.last_used)
                    .unwrap_or(now)
            };
            expanded.push((p.project_id.clone(), recency, idx));
        }
        if expanded.len() > MAX_EXPANDED {
            expanded.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.2.cmp(&b.2))
            });
            for (pid, _, _) in expanded.into_iter().skip(MAX_EXPANDED) {
                force_marble.insert(pid.clone());
                if let Some(app) = app_handle() {
                    let _ = app.emit_to("main", "habitat:collapse", pid);
                }
            }
        }
    }

    // GO-ORBITTY-PLANET-BUDGET cap 12 marbles: at most 12 collapsed
    // NSPanels. Extra collapsed projects stay on the rail list with no
    // panel; a slot frees when one expands or is unclaimed (the stale
    // teardown above). The count uses the EFFECTIVE collapse state (post
    // cap-3), so a marble expanding this sync frees its slot for a waiting
    // project in the same pass.
    let eff_collapsed: HashMap<&str, bool> = snapshot
        .planets
        .iter()
        .map(|p| {
            (
                p.project_id.as_str(),
                p.collapsed || force_marble.contains(&p.project_id),
            )
        })
        .collect();
    let mut marble_panels = h
        .planets
        .keys()
        .filter(|pid| eff_collapsed.get(pid.as_str()).copied().unwrap_or(false))
        .count();
    if marble_panels > MAX_MARBLE_PANELS {
        // Pre-cap worlds overflow: tear down rail-last marble panels first;
        // a world cap-3 just collapsed keeps its panel while any other
        // marble exists (the user just watched it shrink — it must not
        // vanish). Torn-down projects stay on the rail list, no panel.
        let order: HashMap<&str, usize> = snapshot
            .planets
            .iter()
            .enumerate()
            .map(|(i, p)| (p.project_id.as_str(), i))
            .collect();
        let mut victims: Vec<String> = h
            .planets
            .keys()
            .filter(|pid| eff_collapsed.get(pid.as_str()).copied().unwrap_or(false))
            .cloned()
            .collect();
        victims.sort_by(|a, b| {
            force_marble
                .contains(a)
                .cmp(&force_marble.contains(b))
                .then_with(|| {
                    let ia = order.get(a.as_str()).copied().unwrap_or(usize::MAX);
                    let ib = order.get(b.as_str()).copied().unwrap_or(usize::MAX);
                    ib.cmp(&ia) // rail-last first
                })
        });
        for pid in victims.into_iter().take(marble_panels - MAX_MARBLE_PANELS) {
            if let Some(mut planet) = h.planets.remove(&pid) {
                audio::stop_audio(&mut planet);
                unsafe {
                    let panel = planet.panel as *mut Object;
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                    let _: () = msg_send![panel, release];
                }
            }
        }
        marble_panels = MAX_MARBLE_PANELS;
    }

    for HabitatPlanet {
        project_id,
        name,
        collapsed,
        entries,
        board_cards,
    } in &snapshot.planets
    {
        // Effective collapse: the snapshot's flag, or a cap-3 victim this
        // sync. Collapsed layout stays empty — marbles are bare.
        let collapsed_eff = *collapsed || force_marble.contains(project_id);
        let radius = effective_radius(h.worlds.get(project_id), collapsed_eff);
        let side = (radius + VIEW_PADDING) * 2.0;
        let color = color_for(project_id);

        // CADENCE-FIX v1.1: a CLAIMED window is a walker — open or
        // minimized no longer decides. Dots are parked zombies only.
        // Marbles stay bare until expanded (click to expand).
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| {
            (a.app_name.to_lowercase() + &a.title.to_lowercase())
                .cmp(&(b.app_name.to_lowercase() + &b.title.to_lowercase()))
        });
        // Zombie aging (TS-7): 14+ days untouched parks the sprite down to
        // marble-scale occupancy; faded ones still walk but draw dimmed.
        // GO-ORBITTY-SLEEPERS: sleepers (closed claimed windows) always
        // walk — asleep and dim — and are never parked into dots or culled
        // for age `gone`; only Done/Unclaim evicts them.
        let (parked, walking): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) = sorted
            .iter()
            .partition(|e| e.age == "park" && !e.asleep);
        let (walker_entries, dot_entries): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) =
            if collapsed_eff {
                (Vec::new(), Vec::new())
            } else {
                (walking, parked)
            };

        // GO-ORBITTY-CADENCE: a Done entry that gets no walker to walk off
        // (marble planet, or a parked dot) evicts immediately — fast-
        // forward, never a phantom walk. Walkers holding a Done card walk
        // off visibly; anim_tick emits at the rim. CADENCE-FIX: the fast-
        // forward fires ONCE per Done spell (walkoff_fast), not on every
        // sync while the rail's unclaim round-trips.
        let walking_ids: std::collections::HashSet<&str> =
            walker_entries.iter().map(|e| e.id.as_str()).collect();
        for e in entries.iter() {
            if pose_for(e) == Pose::WalkOff
                && !walking_ids.contains(e.id.as_str())
                && h.walkoff_fast.insert(e.id.clone())
            {
                emit_walked_off(&e.id);
            }
        }

        // CADENCE-FIX pose-only fast path: same walker/dot set, same
        // being-defining fields — only the board card fields moved (a
        // board drag). Skip the Being rebuild entirely: no make_being, no
        // icon re-decode, no pack re-lookup; poses update in place below
        // and roam state (positions) stays.
        let pose_only = !packs_reloaded
            && h.planets.get(project_id).is_some_and(|planet| {
                planet.walker_entries.len() == walker_entries.len()
                    && planet.dot_entries.len() == dot_entries.len()
                    && planet
                        .walker_entries
                        .iter()
                        .zip(walker_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
                    && planet
                        .dot_entries
                        .iter()
                        .zip(dot_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
            });

        // Beings are built before borrowing the planet: make_being needs the
        // shared icon cache.
        let built = if pose_only {
            None
        } else {
            let walkers: Vec<Being> = walker_entries
                .iter()
                .map(|e| make_being(&mut h, e))
                .collect();
            let dots: Vec<Being> = dot_entries.iter().map(|e| make_being(&mut h, e)).collect();
            // Lazy packs (GO-ORBITTY-PLANET-BUDGET): decode a sprite pack only
            // when an EXPANDED planet wears it this sync. Marbles never load a
            // pack — their being lists are empty, so this never fires for them.
            if !collapsed_eff {
                for idx in walkers.iter().filter_map(|b| b.pack) {
                    ensure_pack_loaded(&mut h, idx);
                }
            }
            Some((walkers, dots))
        };

        let decor = h.worlds.get(project_id).cloned();
        if let Some(planet) = h.planets.get_mut(project_id) {
            planet.name = name.clone();
            // WORLD-TOWN: the board blit's ticks ride the snapshot; a
            // board drag changes them without touching any being field,
            // so this dirties the view on its own (pose-only path too).
            let board_changed = planet.board_cards != *board_cards;
            planet.board_cards = board_cards.clone();
            let want_proj = locked_projection(collapsed_eff);
            let proj_changed = planet.projection != want_proj;
            planet.projection = want_proj;
            if let Some(w) = decor {
                // LEFTOVER hitch: layout_decor only when the décor itself
                // moved — a board drag / tick sync re-runs the frame math
                // for nothing otherwise.
                let decor_changed = planet.plants != w.plants
                    || planet.mushrooms != w.mushrooms
                    || planet.buildings != w.buildings
                    || planet.boombox_pos != w.boombox_pos
                    || planet.boombox != w.boombox;
                if decor_changed {
                    planet.plants = w.plants;
                    planet.mushrooms = w.mushrooms;
                    planet.buildings = w.buildings;
                    planet.boombox_pos = w.boombox_pos;
                    planet.boombox = w.boombox;
                    layout_decor(planet);
                }
            }
            let geometry_changed = planet.radius != radius || planet.is_marble != collapsed_eff;
            if geometry_changed {
                // Marble ↔ planet swap: resize the panel around its center so
                // the world stays where the user put it (keeps stack order).
                unsafe {
                    let panel = planet.panel as *mut Object;
                    let view = planet.view as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    panel_set_frame(
                        panel,
                        NSRect::new(
                            frame.mid_x() - side / 2.0,
                            frame.mid_y() - side / 2.0,
                            side,
                            side,
                        ),
                        true,
                    );
                    let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
                }
                planet.radius = radius;
                planet.is_marble = collapsed_eff;
                // Marble ↔ planet swap drops any fake-Z orbit state; an
                // expanded planet never translates or scales.
                planet.orbit = None;
                planet.marble_scale = 1.0;
            }
            match built {
                // Pose-only fast path: update poses in place (sticky
                // WalkOff), keep every Being and all roam state.
                None => {
                    let mut any_pose = false;
                    for (b, e) in planet.walkers.iter_mut().zip(walker_entries.iter()) {
                        let next = sticky_pose(b.pose, pose_for(e));
                        if next != b.pose {
                            pose_events.push((
                                project_id.clone(),
                                b.entry_id.clone(),
                                b.pose,
                                next,
                            ));
                            b.pose = next;
                            any_pose = true;
                        }
                    }
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    if geometry_changed || proj_changed {
                        layout_beings(planet);
                    }
                    if any_pose || geometry_changed || proj_changed || board_changed {
                        unsafe {
                            let view = planet.view as *mut Object;
                            let _: () = msg_send![view, setNeedsDisplay: true];
                        }
                    }
                }
                Some((mut walkers, dots)) => {
                    // GO-ORBITTY-CADENCE: one event per pose transition (first
                    // appearances stay silent — the claim event already exists, and
                    // a restart never replays a backlog into the log).
                    let prev_poses: HashMap<String, Pose> = planet
                        .walkers
                        .iter()
                        .map(|b| (b.entry_id.clone(), b.pose))
                        .collect();
                    // CADENCE-FIX: a walk-off in flight survives a blank
                    // card-status blip — never Roam mid-walk.
                    for b in walkers.iter_mut() {
                        if let Some(&prev) = prev_poses.get(&b.entry_id) {
                            b.pose = sticky_pose(prev, b.pose);
                        }
                    }
                    planet.color = color;
                    planet.walkers = walkers;
                    planet.dots = dots;
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    layout_beings(planet);
                    for b in &planet.walkers {
                        if let Some(prev) = prev_poses.get(&b.entry_id) {
                            if *prev != b.pose {
                                pose_events.push((
                                    project_id.clone(),
                                    b.entry_id.clone(),
                                    *prev,
                                    b.pose,
                                ));
                            }
                        }
                    }
                    unsafe {
                        let view = planet.view as *mut Object;
                        let _: () = msg_send![view, setNeedsDisplay: true];
                    }
                }
            }
        } else {
            if collapsed_eff && marble_panels >= MAX_MARBLE_PANELS {
                // 13th+ marble (GO-ORBITTY-PLANET-BUDGET): the project
                // stays on the rail list with NO panel. A slot frees when
                // a marble expands or is unclaimed; that sync's sig change
                // materializes the waitlist in rail order.
                continue;
            }
            if !h.worlds.contains_key(project_id) {
                // Slot index = ring slots already claimed this session;
                // existing worlds are not on the ring and do not occupy.
                let on_screen = match spawn_ring() {
                    Some(ring) => ring_claimed(&ring, &h.worlds),
                    None => h
                        .worlds
                        .values()
                        .filter(|w| w.screen_id == primary_id)
                        .count(),
                };
                h.worlds
                    .insert(project_id.clone(), default_state(&primary_id, on_screen));
            }
            let state = h.worlds[project_id].clone();
            // A planet that didn't exist above can't have taken the
            // pose-only fast path (its gate requires the planet), so the
            // beings are always built here.
            let Some((walkers, dots)) = built else {
                continue;
            };
            let mut planet = Planet {
                panel: 0,
                view: 0,
                name: name.clone(),
                radius,
                sprite_scale: h.sprite_scale,
                is_marble: collapsed_eff,
                color,
                rotation: 0.0,
                projection: locked_projection(collapsed_eff),
                walkers,
                dots,
                caption: None,
                hover_name: false,
                orbit: None,
                marble_scale: 1.0,
                pin_until: None,
                plants: state.plants.clone(),
                mushrooms: state.mushrooms.clone(),
                buildings: state.buildings.clone(),
                boombox_pos: state.boombox_pos,
                boombox: state.boombox,
                audio: None,
                audio_volume: 1.0,
                decor_frames: Vec::new(),
                roam: HashMap::new(),
                walker_entries: walker_entries.iter().map(|e| (*e).clone()).collect(),
                dot_entries: dot_entries.iter().map(|e| (*e).clone()).collect(),
                board_cards: board_cards.clone(),
            };
            layout_beings(&mut planet);
            let Some(cls) = world_view_class() else {
                // Class registration failed — skip this planet and leave the
                // rail (list/focus/stow) fully usable.
                eprintln!("habitat: no world view class; skipping planet {project_id}");
                continue;
            };
            unsafe {
                let view: *mut Object = msg_send![cls, alloc];
                let view: *mut Object =
                    msg_send![view, initWithFrame: NSRect::new(0.0, 0.0, side, side)];
                let panel = make_panel();
                let _: () = msg_send![panel, setContentView: view];
                let _: () = msg_send![view, release]; // panel owns it now
                // Borderless panels never draw a title, but the name shows up
                // in window lists / screen-recording prompts.
                let _: () = msg_send![panel, setTitle: ns_string(&format!("Orbitty — {name}"))];
                panel_set_frame(panel, panel_frame(&state, side), false);
                let _: () = msg_send![panel, orderFrontRegardless];
                planet.panel = panel as usize;
                planet.view = view as usize;
            }
            // Boombox persisted on: start playing once the planet exists.
            if state.boombox && !state.music.is_empty() {
                let live = audio::live_volume(state.boombox_volume, planet.radius, h.reduce_motion);
                audio::start_audio(&mut planet, &state.music, live);
            }
            h.planets.insert(project_id.clone(), planet);
            if collapsed_eff {
                marble_panels += 1;
            }
        }
        // GO-ORBITTY-PROJ-LOCK: heal worlds.json so a leftover "flat"
        // expanded save does not linger. Live disc never reads this.
        if let Some(w) = h.worlds.get_mut(project_id) {
            let want = locked_projection_str(collapsed_eff);
            if w.projection != want {
                w.projection = want.to_string();
                h.worlds_dirty = true;
            }
        }
    }
    // GO-ORBITTY-CADENCE: walked-off re-delivery + Reduce Motion
    // fast-forward. anim_tick emits when a Done walker crosses the hide
    // line; this pass re-emits on every sync while one lingers hidden (the
    // rail's handler is idempotent, so a missed first emit heals here).
    // Under Reduce Motion there is no walk to narrate — Done walkers snap
    // past the hide line at once instead of freezing mid-disc.
    let reduce_motion = h.reduce_motion;
    for planet in h.planets.values_mut() {
        if planet.is_marble {
            continue;
        }
        // Under Reduce Motion anim_tick never steps, so a posed walker
        // would freeze wherever it was — snap every station-posed walker
        // to its station here instead. The world stays truthful, just
        // still: positions are the board, motion is what's reduced.
        if reduce_motion {
            let posed: Vec<(String, Pose)> = planet
                .walkers
                .iter()
                .filter(|b| !b.asleep || b.pose == Pose::WalkOff)
                .map(|b| (b.entry_id.clone(), b.pose))
                .collect();
            let mut snapped = false;
            for (id, pose) in posed {
                let Some((tx, ty)) = (match pose {
                    // PROPS leftover (town usable): Stand snaps to the
                    // board's south lip too — never the post foot
                    // (STATION_BOARD sits behind the sign PNG).
                    Pose::Stand => Some(draw::board_near_lip(
                        planet.radius,
                        planet.projection,
                        planet.marble_scale,
                    )),
                    Pose::WalkOff | Pose::Work | Pose::Sit => {
                        Some(station_target(pose, &id))
                    }
                    Pose::Roam => None,
                }) else {
                    continue;
                };
                if let Some(r) = planet.roam.get_mut(&id) {
                    if (r.x - tx).abs() > 1e-9 || (r.y - ty).abs() > 1e-9 {
                        r.x = tx;
                        r.y = ty;
                        r.tx = tx;
                        r.ty = ty;
                        r.pause_end = None;
                        snapped = true;
                    }
                }
            }
            if snapped {
                layout_beings(planet);
                unsafe {
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
        let done_ids: Vec<String> = planet
            .walkers
            .iter()
            .filter(|b| b.pose == Pose::WalkOff)
            .map(|b| b.entry_id.clone())
            .collect();
        for id in done_ids {
            if planet.roam.get(&id).is_some_and(|r| r.x < HOME_HIDE_X) {
                emit_walked_off(&id);
            }
        }
    }
    // Lazy packs (GO-ORBITTY-PLANET-BUDGET): a pack whose last expanded
    // wearer collapsed (or left) drops its decoded bitmaps; the art paths
    // stay so a re-expand re-decodes.
    unload_dead_packs(&mut h);
    // The habitat lock drops BEFORE the event-log appends — jsonl IO runs
    // on a background thread, never on the AppKit thread under the lock.
    drop(h);
    flush_pose_events(pose_events);
}

/// CADENCE-FIX: one event per pose transition, appended off the AppKit
/// thread and off the habitat lock. Best-effort: a missed pose line beats
/// a stalled UI.
fn flush_pose_events(events: Vec<(String, String, Pose, Pose)>) {
    if events.is_empty() {
        return;
    }
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        for (project_id, entry_id, from, to) in events {
            crate::event_log::append_best(
                &app,
                "walker-pose",
                None,
                Some(&project_id),
                Some("habitat"),
                true,
                &serde_json::json!({
                    "entryId": entry_id,
                    "from": pose_name(from),
                    "to": pose_name(to),
                }),
            );
        }
    });
}

/// Being-defining fields: everything except the board card fields
/// (card_status / card_kind / needed_state — those only move the pose)
/// and `minimized`, which CADENCE-FIX made pose-irrelevant (a claimed
/// window is a walker either way). The pose-only fast path compares these
/// to skip a full Being rebuild on board drags.
fn being_fields_eq(a: &HabitatEntry, b: &HabitatEntry) -> bool {
    a.id == b.id
        && a.app_name == b.app_name
        && a.title == b.title
        && a.bundle_id == b.bundle_id
        && a.icon_base64 == b.icon_base64
        && a.age == b.age
        && a.sprite_pack_id == b.sprite_pack_id
        && a.char_seed == b.char_seed
        && a.char_look == b.char_look
        && a.sprite_scale == b.sprite_scale
        && a.asleep == b.asleep
}

unsafe fn panel_set_frame(panel: *mut Object, frame: NSRect, display: bool) {
    let _: () = msg_send![panel, setFrame: frame display: display];
}

/// GO-ORBITTY-RAIL-ROW-MENU: the rail's `habitat:locate` — stroke the
/// walker's frame for ~500ms. Glow only: no focus, no raise, no wake.
pub fn locate_walker(app: AppHandle, entry_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate = Some((entry_id.clone(), Instant::now()));
        for p in h.planets.values() {
            if p.walkers.iter().any(|b| b.entry_id == entry_id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Dest-menu find: glow that planet ~1.4s. Raises nothing — caller may
/// also `bring_planet_front`. Marble = diagonal ring; expanded = rim + name.
pub fn locate_planet(app: AppHandle, project_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate_planet = Some((project_id.clone(), Instant::now()));
        if let Some(p) = h.planets.get(&project_id) {
            unsafe {
                let view = p.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- rail Home button ----------

/// Order every planet panel front. Peer surfaces keep their relative order;
/// this just lifts the whole set above whatever covered it.
pub fn bring_all_front() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let h = lock();
        for planet in h.planets.values() {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

/// Order one planet's panel front by project id — that disc only. An
/// unknown or panel-less id is a no-op, never a bring_all_front fallback
/// (the user picked one name; lifting every disc would lie).
pub fn bring_planet_front(project_id: &str) {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let id = project_id.to_string();
    let _ = app.run_on_main_thread(move || {
        let h = lock();
        if let Some(planet) = h.planets.get(&id) {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

// ---------- tray escape hatch ----------

/// Bring back every minimized window that has a home (Swift restoreAll).
/// Parked zombies sit in `dots` (marble-scale occupancy) but are still real
/// minimized windows, so they come back too; deminiaturize is a no-op on
/// already-open dot windows. Sleepers (closed windows) are skipped — their
/// door is wake, not deminiaturize.
pub fn restore_all() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let ids: Vec<String> = {
            let h = lock();
            h.planets
                .values()
                .flat_map(|p| {
                    p.walkers
                        .iter()
                        .chain(p.dots.iter())
                        // Sleepers are closed windows — deminiaturize on a
                        // dead id can only fail; wake is their door.
                        .filter(|b| !b.asleep)
                        .map(|b| b.entry_id.clone())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for id in ids {
            let _ = crate::windows::deminiaturize_window(&id);
        }
    });
}
