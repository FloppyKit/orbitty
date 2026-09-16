//! Unbound todos grow a tree on their planet.

use crate::state::{KanbanCard, PersistedState, TodoTree};

pub const DEFAULT_GROW_DAYS: f64 = 14.0;

// Grove: east of the grass west edge, north of disc center (y < 0).
// Never west into the cabin. May nibble the board when grown.
const GROVE_WEST: f64 = -0.364; // YARD_WEST
const GROVE_EAST: f64 = 0.50;
const GROVE_NORTH: f64 = -0.70; // toward the board; negative y is north
const GROVE_SOUTH: f64 = -0.02;

/// Unbound = a real todo, no window, no assigned agent, not Needed.
pub fn is_unbound(c: &KanbanCard) -> bool {
    if c.kind.as_deref() == Some("needed") {
        return false;
    }
    if c.kind.as_deref() == Some("window") {
        return false;
    }
    if c.window_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_some() {
        return false;
    }
    if c.assigned_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return false;
    }
    if c.project_id.trim().is_empty() {
        return false;
    }
    c.status == "todo" || c.status == "doing"
}

fn hash_str(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn default_spot(card_id: &str) -> (f64, f64) {
    let h = hash_str(card_id);
    let xw = GROVE_EAST - GROVE_WEST;
    let yh = GROVE_SOUTH - GROVE_NORTH;
    let x = GROVE_WEST + ((h % 40) as f64) / 39.0 * xw;
    let y = GROVE_NORTH + (((h >> 8) % 28) as f64) / 27.0 * yh;
    (
        x.clamp(GROVE_WEST, GROVE_EAST),
        y.clamp(GROVE_NORTH, GROVE_SOUTH),
    )
}

fn in_grove(x: f64, y: f64) -> bool {
    x >= GROVE_WEST && x <= GROVE_EAST && y >= GROVE_NORTH && y <= GROVE_SOUTH
}

pub fn grow_days(st: &PersistedState) -> f64 {
    let d = st.tree_grow_days;
    if d.is_finite() && d > 0.0 {
        d.clamp(1.0, 365.0)
    } else {
        DEFAULT_GROW_DAYS
    }
}

/// 0 sprout … 1 full. No due → full at treeGrowDays. Due set → full by due.
pub fn growth_t(planted_at: f64, due_at: Option<f64>, now: f64, grow_days: f64) -> f64 {
    let span = if let Some(due) = due_at {
        (due - planted_at).max(60.0)
    } else {
        grow_days.max(1.0) * 86400.0
    };
    ((now - planted_at) / span).clamp(0.0, 1.0)
}

pub fn phase(t: f64) -> usize {
    ((t * 7.0).floor() as usize).min(7)
}

/// Mint missing trees; drop trees whose card is gone, done, or now bound.
pub fn reconcile(st: &mut PersistedState) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let live: std::collections::HashSet<String> = st
        .kanban_cards
        .iter()
        .filter(|c| is_unbound(c))
        .map(|c| c.id.clone())
        .collect();
    st.todo_trees.retain(|t| live.contains(&t.card_id));
    for t in &mut st.todo_trees {
        if !in_grove(t.x, t.y) {
            let (x, y) = default_spot(&t.card_id);
            t.x = x;
            t.y = y;
        }
    }
    let have: std::collections::HashSet<String> =
        st.todo_trees.iter().map(|t| t.card_id.clone()).collect();
    for c in &st.kanban_cards {
        if !is_unbound(c) || have.contains(&c.id) {
            continue;
        }
        let (x, y) = default_spot(&c.id);
        st.todo_trees.push(TodoTree {
            card_id: c.id.clone(),
            project_id: c.project_id.clone(),
            x,
            y,
            planted_at: now,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::KanbanCard;

    fn card(id: &str, status: &str) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: "Thing".into(),
            project_id: "p-1".into(),
            window_id: None,
            kind: Some("todo".into()),
            status: status.into(),
            created_at: 1.0,
            last_touched_at: 1.0,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: None,
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
            rating: None,
            trackers: None,
        }
    }

    #[test]
    fn unbound_is_loose_todo() {
        assert!(is_unbound(&card("c-1", "todo")));
        let mut w = card("c-2", "todo");
        w.window_id = Some("1:2".into());
        assert!(!is_unbound(&w));
        let mut a = card("c-3", "todo");
        a.assigned_agent_id = Some("a-1".into());
        assert!(!is_unbound(&a));
        assert!(!is_unbound(&card("c-4", "done")));
    }

    #[test]
    fn phase_ends_at_seven() {
        assert_eq!(phase(0.0), 0);
        assert_eq!(phase(1.0), 7);
        assert_eq!(phase(0.5), 3);
    }

    #[test]
    fn grove_is_east_of_grass_west_and_north_of_center() {
        for id in ["c-1", "c-2", "c-grove", "c-mu3hzb2l-us8rqx"] {
            let (x, y) = default_spot(id);
            assert!(x >= GROVE_WEST && x <= GROVE_EAST, "x={x}");
            assert!(y >= GROVE_NORTH && y <= GROVE_SOUTH, "y={y}");
            assert!(y < 0.0, "north of center y={y}");
        }
    }
}
