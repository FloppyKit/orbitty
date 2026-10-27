//! Skirt todo-tree discs on the way to an outpost. Straight when the line is clear.

/// Disc radius in planet units. A tree this wide blocks a straight walk.
pub(crate) const TREE_BLOCK_R: f64 = 0.10;

pub(crate) fn tree_discs(trees: &[(f64, f64)]) -> Vec<(f64, f64, f64)> {
    trees.iter().map(|&(x, y)| (x, y, TREE_BLOCK_R)).collect()
}

/// Next aim. `to` when the segment misses every disc. Otherwise a point
/// south of the nearest blocking disc (disc +y is south, and Comms sits
/// south of the grove).
pub(crate) fn skirt_aim(from: (f64, f64), to: (f64, f64), discs: &[(f64, f64, f64)]) -> (f64, f64) {
    if !segment_hits(from, to, discs) {
        return to;
    }
    let Some((cx, cy, r)) = nearest_hit(from, to, discs) else {
        return to;
    };
    let clear = r + 0.04;
    let south = (cx, (cy + clear).min(1.05));
    if dist(from, south) > 0.02 && !segment_hits(from, south, discs) {
        return south;
    }
    let east = ((cx + clear).min(1.05), (cy + clear).min(1.05));
    if dist(from, east) > 0.02 && !segment_hits(from, east, discs) {
        return east;
    }
    (from.0, (from.1 + clear).min(1.05))
}

fn nearest_hit(
    from: (f64, f64),
    to: (f64, f64),
    discs: &[(f64, f64, f64)],
) -> Option<(f64, f64, f64)> {
    let mut best: Option<(f64, (f64, f64, f64))> = None;
    for &(cx, cy, r) in discs {
        let d = dist_point_seg((cx, cy), from, to);
        if d >= r {
            continue;
        }
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, (cx, cy, r)));
        }
    }
    best.map(|(_, disc)| disc)
}

fn segment_hits(a: (f64, f64), b: (f64, f64), discs: &[(f64, f64, f64)]) -> bool {
    discs
        .iter()
        .any(|&(cx, cy, r)| dist_point_seg((cx, cy), a, b) < r)
}

fn dist_point_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-12 {
        return dist(p, a);
    }
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0);
    dist(p, (a.0 + abx * t, a.1 + aby * t))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_straight_line_still_reaches_comms() {
        let goal = (0.86, 0.15);
        let tree = (0.40, 0.08);
        let discs = [(tree.0, tree.1, TREE_BLOCK_R)];
        assert!(
            segment_hits((0.0, 0.05), goal, &discs),
            "fixture must block the straight line"
        );
        let mut pos = (0.0, 0.05);
        let mut min_tree = dist(pos, tree);
        for _ in 0..80 {
            let aim = skirt_aim(pos, goal, &discs);
            let dx = aim.0 - pos.0;
            let dy = aim.1 - pos.1;
            let step_len = (dx * dx + dy * dy).sqrt();
            if step_len < 1e-9 {
                pos.1 += 0.02;
            } else {
                let step = 0.16_f64.min(step_len);
                pos.0 += dx / step_len * step;
                pos.1 += dy / step_len * step;
            }
            min_tree = min_tree.min(dist(pos, tree));
            if dist(pos, goal) < 0.04 {
                break;
            }
        }
        assert!(
            dist(pos, goal) < 0.04,
            "did not reach comms, ended at {pos:?}"
        );
        assert!(
            min_tree > TREE_BLOCK_R * 0.85,
            "walked through the tree (min {min_tree})"
        );
    }
}
