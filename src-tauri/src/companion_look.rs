//! Look-at contract for the Spirit flipbook. Webview `src/lib/lookAt.ts`
//! is the live math — keep the numbers in lockstep. Pointer tracking
//! stays in the webview (window-local). This module names the cells
//! and holds the math tests.
#![allow(dead_code)]

pub const LOOK_DIRS: [&str; 8] = ["e", "ne", "n", "nw", "w", "sw", "s", "se"];
pub const OVERLAY_KINDS: [&str; 6] = [
    "blink",
    "poke",
    "sleepy",
    "sparkle",
    "delighted",
    "dizzy",
];

pub const OVERLAY_MS: u32 = 500;
pub const DEAD_PX: f64 = 56.0;
pub const DEAD_HYST_PX: f64 = 20.0;
pub const HALF_SECTOR_DEG: f64 = 22.5;
pub const HYST_DEG: f64 = 10.0;
pub const POKE_WINDOW_MS: i64 = 1400;
pub const POKE_DIZZY: usize = 4;

pub fn look_file(i: usize) -> String {
    format!("look_{i}.png")
}
pub fn overlay_file(kind: &str) -> String {
    format!("overlay_{kind}.png")
}
pub fn bust_look_file(i: usize) -> String {
    format!("bust_{i}.png")
}
pub fn bust_overlay_file(kind: &str) -> String {
    format!("overlay_bust_{kind}.png")
}

pub fn look_enabled(fine_pointer: bool, reduced_motion: bool) -> bool {
    fine_pointer && !reduced_motion
}

/// Screen delta → degrees, 0 = east, 90 = north (y-down inverted).
pub fn vector_deg(dx: f64, dy: f64) -> f64 {
    let mut a = (-dy).atan2(dx).to_degrees();
    if a < 0.0 {
        a += 360.0;
    }
    a
}

pub fn sector_index(deg: f64) -> usize {
    ((deg / 45.0).round() as i32).rem_euclid(8) as usize
}

fn angle_delta_deg(a: f64, b: f64) -> f64 {
    let mut d = a - b;
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

/// `current` `-1` is center idle. `0..7` match `LOOK_DIRS`.
pub fn pick_look_cell(dx: f64, dy: f64, current: i32, in_dead: bool) -> (i32, bool) {
    let dist = dx.hypot(dy);
    if in_dead {
        if dist < DEAD_PX + DEAD_HYST_PX {
            return (-1, true);
        }
    } else if dist < DEAD_PX {
        return (-1, true);
    }
    let next = sector_index(vector_deg(dx, dy)) as i32;
    if current < 0 || current == next {
        return (next, false);
    }
    let cur_deg = current as f64 * 45.0;
    if angle_delta_deg(vector_deg(dx, dy), cur_deg).abs() <= HALF_SECTOR_DEG + HYST_DEG {
        return (current, false);
    }
    (next, false)
}

pub fn record_poke(times: &[i64], now: i64) -> (Vec<i64>, &'static str) {
    let mut kept: Vec<i64> = times
        .iter()
        .copied()
        .filter(|t| now - t <= POKE_WINDOW_MS)
        .collect();
    kept.push(now);
    if kept.len() >= POKE_DIZZY {
        (Vec::new(), "dizzy")
    } else {
        (kept, "poke")
    }
}

#[derive(Clone, Debug)]
pub struct CardSnap {
    pub id: String,
    pub status: String,
    pub due_at: Option<f64>,
}

pub fn overlay_from_board(
    prev_ready: bool,
    prev_ids: &[String],
    prev_done: &[String],
    next: &[CardSnap],
    now_sec: f64,
) -> (Option<&'static str>, bool) {
    let overdue = next.iter().any(|c| {
        c.status != "done" && c.due_at.is_some_and(|d| d < now_sec)
    });
    if !prev_ready {
        return (None, overdue);
    }
    for c in next {
        if c.status == "done" && !prev_done.iter().any(|id| id == &c.id) {
            return (Some("delighted"), overdue);
        }
    }
    for c in next {
        if c.status != "done" && !prev_ids.iter().any(|id| id == &c.id) {
            return (Some("sparkle"), overdue);
        }
    }
    (None, overdue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_dirs_and_overlays() {
        assert_eq!(LOOK_DIRS.len(), 8);
        assert_eq!(OVERLAY_KINDS.len(), 6);
        assert_eq!(OVERLAY_MS, 500);
        assert_eq!(look_file(0), "look_0.png");
        assert_eq!(overlay_file("blink"), "overlay_blink.png");
        assert_eq!(bust_look_file(2), "bust_2.png");
        assert_eq!(bust_overlay_file("dizzy"), "overlay_bust_dizzy.png");
        assert_eq!(LOOK_DIRS[2], "n");
    }

    #[test]
    fn sectors_ccw_from_east() {
        assert_eq!(sector_index(0.0), 0); // E
        assert_eq!(sector_index(45.0), 1); // NE
        assert_eq!(sector_index(90.0), 2); // N
        assert_eq!(sector_index(180.0), 4); // W
        assert_eq!(sector_index(270.0), 6); // S
        assert_eq!(sector_index(359.0), 0);
        assert!((vector_deg(10.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((vector_deg(0.0, -10.0) - 90.0).abs() < 1e-6);
    }

    #[test]
    fn dead_zone_and_hysteresis() {
        assert_eq!(pick_look_cell(200.0, 0.0, -1, true), (0, false));
        assert_eq!(pick_look_cell(10.0, 0.0, 0, false), (-1, true));
        assert_eq!(
            pick_look_cell(DEAD_PX + 4.0, 0.0, -1, true),
            (-1, true)
        );
        assert_eq!(
            pick_look_cell(DEAD_PX + DEAD_HYST_PX + 4.0, 0.0, -1, true),
            (0, false)
        );
        let line = HALF_SECTOR_DEG + 1.0;
        let rad = 200.0;
        let x = rad * line.to_radians().cos();
        let y = -rad * line.to_radians().sin();
        assert_eq!(pick_look_cell(x, y, 0, false).0, 0);
        let past = HALF_SECTOR_DEG + HYST_DEG + 4.0;
        let x2 = rad * past.to_radians().cos();
        let y2 = -rad * past.to_radians().sin();
        assert_eq!(pick_look_cell(x2, y2, 0, false).0, 1);
    }

    #[test]
    fn reduced_motion_and_coarse_pointer() {
        assert!(look_enabled(true, false));
        assert!(!look_enabled(false, false));
        assert!(!look_enabled(true, true));
    }

    #[test]
    fn four_pokes_dizzy() {
        let (t, k) = record_poke(&[], 1000);
        assert_eq!(k, "poke");
        let (t, k) = record_poke(&t, 1100);
        assert_eq!(k, "poke");
        let (t, k) = record_poke(&t, 1200);
        assert_eq!(k, "poke");
        let (t, k) = record_poke(&t, 1300);
        assert_eq!(k, "dizzy");
        assert!(t.is_empty());
        let (t, k) = record_poke(&[], 0);
        assert_eq!(k, "poke");
        let (_, k) = record_poke(&t, 2000);
        assert_eq!(k, "poke");
    }

    #[test]
    fn board_binds() {
        let a = CardSnap {
            id: "a".into(),
            status: "todo".into(),
            due_at: None,
        };
        let b = CardSnap {
            id: "b".into(),
            status: "todo".into(),
            due_at: None,
        };
        let done_a = CardSnap {
            id: "a".into(),
            status: "done".into(),
            due_at: None,
        };
        let (cue, _) = overlay_from_board(false, &[], &[], &[a.clone()], 10.0);
        assert_eq!(cue, None);
        let (cue, _) = overlay_from_board(
            true,
            &["a".into()],
            &[],
            &[a.clone(), b.clone()],
            11.0,
        );
        assert_eq!(cue, Some("sparkle"));
        let (cue, _) = overlay_from_board(
            true,
            &["a".into(), "b".into()],
            &[],
            &[done_a, b],
            12.0,
        );
        assert_eq!(cue, Some("delighted"));
        let overdue = CardSnap {
            id: "c".into(),
            status: "todo".into(),
            due_at: Some(1.0),
        };
        let (cue, is_overdue) =
            overlay_from_board(true, &["c".into()], &[], &[overdue], 99.0);
        assert_eq!(cue, None);
        assert!(is_overdue);
    }
}
