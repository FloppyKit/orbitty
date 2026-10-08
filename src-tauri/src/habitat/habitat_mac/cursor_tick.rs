//! Cursor / click-through / boombox audio / locate / caption tick (DIET-HABITAT).
//!
//! Extracted from habitat_mac/tick.rs. Runs ~10Hz on the habitat tick hop.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use super::audio;
use super::boombox_hud;
use super::input::{contains_interactive, over_disc};
use super::{
    lock, panel_frame, panel_set_frame, save_worlds, screens, set_caption, Habitat, NSPoint, NSRect,
    CAPTION_TTL, LOCATE_TTL, PLANET_LOCATE_TTL, VIEW_PADDING,
};

pub(crate) fn cursor_tick() {
    let mut h = lock();
    // Boombox: AVAudioPlayer loops while on. Zoom is the fader — set
    // volume in place, never restart the file. Off / unplayable still
    // stop. Runs even mid-drag so music never gaps while moving a planet.
    let audio_state: HashMap<String, (String, f32, bool)> = h
        .worlds
        .iter()
        .map(|(id, w)| {
            (
                id.clone(),
                (w.music.clone(), w.boombox_volume, w.boombox_muted),
            )
        })
        .collect();
    let reduce = h.reduce_motion;
    // Choose-music panel up: no start until it closes (beachball guard).
    let picker_up = audio::MUSIC_PICKER_OPEN.load(Ordering::SeqCst);
    let mut failed: Vec<String> = Vec::new();
    if !picker_up {
        for (project_id, planet) in h.planets.iter_mut() {
            if !planet.boombox {
                continue;
            }
            let (music, remembered, muted) = audio_state
                .get(project_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), 1.0, false));
            // HUD speaker hard mute: the kept player fades to 0 — never
            // the Off end's stop/release, so unmute continues mid-track.
            let live = if muted {
                0.0
            } else {
                audio::live_volume(remembered, planet.radius, reduce)
            };
            if planet.audio.is_some() {
                // Same player: fade even at 0 (marble mute, keep the
                // playhead so zooming back continues the track).
                audio::fade_audio(planet, live);
            } else if live > 0.0 {
                if !audio::start_audio(planet, &music, live) {
                    failed.push(project_id.clone());
                    set_caption(planet, "Choose music…".into());
                }
            }
        }
    }
    for id in failed {
        if let Some(world) = h.worlds.get_mut(&id) {
            world.music.clear();
            h.worlds_dirty = true;
        }
    }
    // Scroll-zoom / menu mutations persist through here (10Hz), never per
    // scroll event.
    if h.worlds_dirty {
        h.worlds_dirty = false;
        save_worlds(&h);
    }
    // HUD self-heal: the box left the world (Remove, a sync, collapse,
    // teardown) → the HUD dies with it, whichever door did it.
    if h.boombox_hud.is_some() {
        let gone = {
            let pid = h.boombox_hud.as_deref().unwrap_or_default();
            h.planets
                .get(pid)
                .map_or(true, |p| p.is_marble || boombox_hud::rects(p).is_none())
        };
        if gone {
            boombox_hud::close(&mut h);
        }
    }
    // Locate stroke expiry (GO-ORBITTY-RAIL-ROW-MENU): 10Hz sweep, so the
    // ring draws out even with Reduce Motion on (anim_tick is parked then).
    if h
        .locate
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed() >= LOCATE_TTL)
    {
        let id = h.locate.take().map(|(id, _)| id).unwrap_or_default();
        for p in h.planets.values() {
            if p.walkers.iter().any(|b| b.entry_id == id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    }
    if let Some((id, at)) = h.locate_planet.clone() {
        if at.elapsed() < PLANET_LOCATE_TTL {
            if let Some(p) = h.planets.get(&id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        } else {
            h.locate_planet = None;
            if let Some(p) = h.planets.get(&id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    }
    if h.drag.is_some() {
        return; // a drag holds interactivity even if the cursor leaves the disc
    }
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: bool = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        h.reduce_motion = reduce;

        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let all = screens();
        let primary = all.first().map(|(id, _)| id.clone());
        let hud_open = h.boombox_hud.clone();
        let Habitat { planets, worlds, .. } = &mut *h;
        for (project_id, planet) in planets.iter_mut() {
            let panel = planet.panel as *mut Object;
            // Display unplugged/rearranged: fold onto the primary screen.
            if let Some(state) = worlds.get_mut(project_id) {
                if !all.iter().any(|(id, _)| id == &state.screen_id) {
                    if let Some(p) = &primary {
                        state.screen_id = p.clone();
                        let side = (planet.radius + VIEW_PADDING) * 2.0;
                        let frame = panel_frame(state, side);
                        panel_set_frame(panel, frame, true);
                    }
                }
            }
            let frame: NSRect = msg_send![panel, frame];
            let local = NSPoint {
                x: mouse.x - frame.origin.x,
                y: frame.max_y() - mouse.y,
            };
            let over =
                contains_interactive(planet, local, hud_open.as_deref() == Some(project_id));
            let _: () = msg_send![panel, setIgnoresMouseEvents: !over];
            // marble hover names the planet. Done
            // here in the cursor watcher — the panel ignores mouse events
            // off-disc, so a tracking area would never see the enter. The
            // draw falls back to the name only when no click caption is
            // live, so placed/walker/boombox pills are never stomped.
            let hover = planet.is_marble && over_disc(planet, local);
            if hover != planet.hover_name {
                planet.hover_name = hover;
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
            // Caption expiry (10Hz here so Reduce Motion can't pin it on).
            if planet
                .caption
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() >= CAPTION_TTL)
            {
                planet.caption = None;
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
}

