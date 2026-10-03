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

struct LaterShow {
    panel: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Paint after this sync drops the habitat lock. drawRect takes that lock,
/// so display here would deadlock. The deferred pass also re-applies the
/// marble frame: a panel born at 10×10 stays a clear speck until something
/// else resizes it, and that resize is the expand click.
fn show_later(panel: *mut Object, frame: NSRect) {
    let job = Box::new(LaterShow {
        panel: panel as usize,
        x: frame.origin.x,
        y: frame.origin.y,
        w: frame.size.width,
        h: frame.size.height,
    });
    extern "C" {
        #[link_name = "_dispatch_main_q"]
        static DISPATCH_MAIN_Q: std::ffi::c_void;
        fn dispatch_async_f(
            queue: *mut std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
    }
    extern "C" fn lift(ctx: *mut std::ffi::c_void) {
        if ctx.is_null() {
            return;
        }
        let job = unsafe { Box::from_raw(ctx as *mut LaterShow) };
        let panel = job.panel as *mut Object;
        if panel.is_null() {
            return;
        }
        unsafe {
            let frame = NSRect::new(job.x, job.y, job.w, job.h);
            super::panel_set_frame(panel, frame, true);
            let view: *mut Object = msg_send![panel, contentView];
            if !view.is_null() {
                let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, job.w, job.h)];
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
            let _: () = msg_send![panel, orderFrontRegardless];
        }
    }
    unsafe {
        dispatch_async_f(
            std::ptr::addr_of!(DISPATCH_MAIN_Q) as *mut std::ffi::c_void,
            Box::into_raw(job) as *mut std::ffi::c_void,
            lift,
        );
    }
}

/// Cap is 12 marble panels. A new mint must still appear, so the least
/// recently used other marble loses its panel and stays on the rail.
fn evict_one_marble(h: &mut Habitat, keep: &str) -> bool {
    let ids: Vec<String> = h
        .planets
        .iter()
        .filter(|(id, p)| p.is_marble && id.as_str() != keep)
        .map(|(id, _)| id.clone())
        .collect();
    let Some(victim) = ids.into_iter().min_by(|a, b| {
        let ta = h.worlds.get(a).map(|w| w.last_used).unwrap_or(0.0);
        let tb = h.worlds.get(b).map(|w| w.last_used).unwrap_or(0.0);
        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
    }) else {
        return false;
    };
    let Some(mut planet) = h.planets.remove(&victim) else {
        return false;
    };
    audio::stop_audio(&mut planet);
    unsafe {
        let panel = planet.panel as *mut Object;
        if !panel.is_null() {
            let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
            let _: () = msg_send![panel, release];
        }
    }
    true
}

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
    if collapsed_eff && marble_panels >= MAX_MARBLE_PANELS && !evict_one_marble(h, project_id)
    {
        // Cap full and nothing else to retire. Rail list only.
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
        // The panel is born 10×10. setContentView fits the view to that
        // rect, and the marble is drawn at the view center — so a view
        // left at 10×10 clips the disc to nothing. Width+height sizable
        // (2|16) keeps later resizes filling the panel.
        let _: () = msg_send![view, setAutoresizingMask: 18usize];
        let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
        // display:false while the habitat lock is held. The paint is
        // order_front_later, after this planet is inserted.
        let frame = panel_frame(&state, side);
        panel_set_frame(panel, frame, false);
        let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
        if h.planets_hidden {
            let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
        } else {
            let _: () = msg_send![panel, orderFrontRegardless];
            // Lift and paint once this sync has dropped the lock. Same
            // marble frame — do not expand.
            show_later(panel, frame);
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
