//! Expanded / marble panel budgets for `do_sync` (DIET-HABITAT).
//!
//! Cap-3 expanded worlds + cap-12 marble NSPanels, peeled from `sync.rs`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::{HashMap, HashSet};
use tauri::Emitter;

use super::audio;
use super::{
    app_handle, now_epoch, Habitat, HabitatSnapshot, MAX_EXPANDED, MAX_MARBLE_PANELS,
};

/// Cap-3 expanded (emit `habitat:collapse` for victims) and cap-12 marble
/// NSPanels (tear down rail-last overflow). Returns project ids forced to
/// marble this sync — also used for effective collapse in `do_sync` —
/// plus the post-cap marble panel count (for create-slot gating).
pub(crate) fn apply_budgets(
    h: &mut Habitat,
    snapshot: &HabitatSnapshot,
) -> (HashSet<String>, usize) {
    // BUDGET cap 3 expanded: habitat:expand /
    // click-to-expand always succeeds — when a 4th world is expanded, the
    // least-recently focused of the rest takes the marble (stable snapshot
    // order breaks recency ties). The rail owns the collapsed flag: the
    // habitat:collapse echo makes it persist there, and this sync already
    // renders the victim as a marble so a 4th planet never shows. The
    // expand gesture itself is the focus — a marble→expanded transition
    // bumps the world's last_used ledger (drags and scroll-zooms already
    // stamp it), so recency survives restarts via worlds.json.
    let mut force_marble: HashSet<String> = HashSet::new();
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

    // BUDGET cap 12 marbles: at most 12 collapsed
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

    (force_marble, marble_panels)
}
