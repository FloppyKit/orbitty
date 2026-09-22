//! First panel for a project that is not on screen yet (DIET-HABITAT).
//!
//! Over the marble cap the project stays on the rail with no panel.
//! A missing world-view class skips the planet and leaves the rail usable.
//! Peeled from `sync.rs`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::time::Instant;

use super::audio;
use super::todo_tree;
use crate::habitat::{BoardCardTick, HabitatEntry, TodoTreeTick};
use super::{
    default_state, layout_beings, locked_projection, make_panel, ns_string, panel_frame,
    panel_set_frame, world_view_class, Being, Habitat,
    MAX_MARBLE_PANELS, NSRect, Planet,
};

pub(crate) enum Fresh {
    /// No panel this sync. Caller skips the projection heal.
    Skip,
    /// Panel is on screen. `marble` counts against the cap.
    Panel { marble: bool },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize(
    h: &mut Habitat,
    project_id: &str,
    name: &str,
    collapsed_eff: bool,
    marble_panels: usize,
    primary_id: &str,
    radius: f64,
    side: f64,
    color: (f64, f64, f64),
    built: Option<(Vec<Being>, Vec<Being>)>,
    walker_entries: &[HabitatEntry],
    dot_entries: &[HabitatEntry],
    board_cards: &[BoardCardTick],
    todo_trees: &[TodoTreeTick],
    tree_grow_days: f64,
) -> Fresh {
    if collapsed_eff && marble_panels >= MAX_MARBLE_PANELS {
        // 13th+ marble: the project stays on the rail list with NO panel.
        // A slot frees when a marble expands or is unclaimed; that sync
        // materializes the waitlist in rail order.
        return Fresh::Skip;
    }
    if !h.worlds.contains_key(project_id) {
        // Middle display, clear of the inspector. Step aside when taken.
        let nudge = super::spawn::fresh_anchor()
            .map(|(sid, cx, cy)| {
                let mut n = 0usize;
                while h.worlds.values().any(|w| {
                    let x = cx + n as f64 * 40.0;
                    let y = cy + n as f64 * 28.0;
                    w.screen_id == sid && (w.x - x).abs() < 36.0 && (w.y - y).abs() < 36.0
                }) {
                    n += 1;
                    if n > 16 {
                        break;
                    }
                }
                n
            })
            .unwrap_or(0);
        h.worlds
            .insert(project_id.to_string(), default_state(primary_id, nudge));
    }
    let state = h.worlds[project_id].clone();
    // A planet that didn't exist above can't have taken the pose-only
    // fast path (its gate requires the planet), so the beings are always
    // built here.
    let Some((walkers, dots)) = built else {
        return Fresh::Skip;
    };
    let mut planet = Planet {
        panel: 0,
        view: 0,
        name: name.to_string(),
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
        beach_ball: state.beach_ball,
        ball: None,
        audio: None,
        audio_volume: 1.0,
        decor_frames: Vec::new(),
        roam: HashMap::new(),
        walker_entries: walker_entries.to_vec(),
        dot_entries: dot_entries.to_vec(),
        board_cards: board_cards.to_vec(),
        todo_trees: todo_trees.to_vec(),
        todo_tree_frames: Vec::new(),
        tree_grow_days,
        tree_chores: Vec::new(),
    };
    layout_beings(&mut planet);
    todo_tree::layout(&mut planet);
    let Some(cls) = world_view_class() else {
        // Class registration failed — skip this planet and leave the
        // rail (list/focus/stow) fully usable.
        eprintln!("habitat: no world view class; skipping planet {project_id}");
        return Fresh::Skip;
    };
    unsafe {
        let view: *mut Object = msg_send![cls, alloc];
        let view: *mut Object = msg_send![view, initWithFrame: NSRect::new(0.0, 0.0, side, side)];
        let panel = make_panel();
        let _: () = msg_send![panel, setContentView: view];
        let _: () = msg_send![view, release]; // panel owns it now
        // Borderless panels never draw a title, but the name shows up
        // in window lists / screen-recording prompts.
        let _: () = msg_send![panel, setTitle: ns_string(&format!("Orbitty — {name}"))];
        panel_set_frame(panel, panel_frame(&state, side), false);
        if h.planets_hidden {
            let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
        } else {
            let _: () = msg_send![panel, orderFrontRegardless];
        }
        planet.panel = panel as usize;
        planet.view = view as usize;
    }
    // Boombox persisted on: start playing once the planet exists.
    // A hard-muted save starts nothing — the cursor_tick fader
    // door brings it up on unmute.
    if state.boombox && !state.music.is_empty() {
        let live = if state.boombox_muted {
            0.0
        } else {
            audio::live_volume(state.boombox_volume, planet.radius, h.reduce_motion)
        };
        audio::start_audio(&mut planet, &state.music, live);
    }
    h.planets.insert(project_id.to_string(), planet);
    // A fresh panel must not debut behind the user's windows —
    // orderFrontRegardless ran above; arm the locate glow so the first
    // blits ring the disc (project_add arms pre-sync too; this is the
    // guarantee).
    h.locate_planet = Some((project_id.to_string(), Instant::now()));
    Fresh::Panel {
        marble: collapsed_eff,
    }
}
