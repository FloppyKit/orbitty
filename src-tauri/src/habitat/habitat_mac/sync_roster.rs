//! Walker / dot split for one planet sync (DIET-HABITAT).
//!
//! A claimed window is a walker whether it is open or minimized.
//! Only a closed claim is asleep. Parked zombies (age "park", not
//! asleep) are dots. A marble has neither. Peeled from `sync.rs`.

use super::HabitatEntry;

pub(crate) fn classify(
    entries: &[HabitatEntry],
    collapsed: bool,
) -> (Vec<HabitatEntry>, Vec<HabitatEntry>) {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        (a.app_name.to_lowercase() + &a.title.to_lowercase())
            .cmp(&(b.app_name.to_lowercase() + &b.title.to_lowercase()))
    });
    // Marbles are bare — no walkers, no dots.
    if collapsed {
        return (Vec::new(), Vec::new());
    }
    let mut walkers = Vec::new();
    let mut dots = Vec::new();
    for entry in sorted {
        // Sleepers always walk, dim. Only Done/Unclaim evicts them.
        // 14+ days untouched, and not asleep, parks down to a dot.
        if entry.age == "park" && !entry.asleep {
            dots.push(entry);
        } else {
            walkers.push(entry);
        }
    }
    (walkers, dots)
}
