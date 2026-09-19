//! Deterministic planet / card colors.
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

pub(crate) fn color_for(project_id: &str) -> (f64, f64, f64) {
    let sum: u32 = project_id.bytes().map(|b| b as u32).sum();
    let hue = ((sum as f64) * 137.508) % 360.0 / 360.0;
    hsv(hue, 0.55, 0.80)
}

/// stable per-card tick color for the board blit —
/// the same golden-angle idea as color_for, one hue per card id.
pub(crate) fn card_color(card_id: &str) -> (f64, f64, f64) {
    let sum: u32 = card_id.bytes().map(|b| b as u32).sum();
    let hue = ((sum as f64) * 137.508) % 360.0 / 360.0;
    hsv(hue, 0.60, 0.92)
}

pub(crate) fn hsv(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
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
