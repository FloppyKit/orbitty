//! Boombox AVAudioPlayer + choose-music picker.
//! Zoom fades volume in place — never restart the track.
//! Off stops the player. Marble (MIN) is silent.

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

use super::{
    lock, max_radius, ns_string, sender_project_id, set_caption, world_entry, Planet,
    EXPANDED_RADIUS, MIN_RADIUS,
};

/// Zoom fader across the whole legal radius — no dead zone at the top.
/// AVAudioPlayer caps at 1.0. 0 at marble (MIN), ~0.55 at EXPANDED,
/// 1.0 at max_radius. Shrinking a large world is audible immediately;
/// the last stretch to marble still goes silent.
pub(crate) fn zoom_gain(radius: f64) -> f32 {
    let r = radius.max(0.0);
    if r <= MIN_RADIUS {
        return 0.0;
    }
    let max_r = max_radius().max(EXPANDED_RADIUS + 1.0);
    if r >= max_r {
        return 1.0;
    }
    if r >= EXPANDED_RADIUS {
        let t = (r - EXPANDED_RADIUS) / (max_r - EXPANDED_RADIUS);
        return (0.55 + 0.45 * t) as f32;
    }
    let t = (r - MIN_RADIUS) / (EXPANDED_RADIUS - MIN_RADIUS).max(1.0);
    (0.55 * t) as f32
}

/// Live output = remembered step × zoom gain. Reduce Motion skips the
/// gain (remembered step only). 0.0 is the Off step — a real pick, kept;
/// only a negative (hand-edited save) falls back to full.
pub(crate) fn live_volume(remembered: f32, radius: f64, reduce_motion: bool) -> f32 {
    let remembered = if remembered >= 0.0 { remembered } else { 1.0 };
    if reduce_motion {
        remembered
    } else {
        remembered * zoom_gain(radius)
    }
}

/// Live fader: set AVAudioPlayer.volume in place. Zoom/Reduce Motion
/// must never restart the file. Volume 0 (marble) is mute, player kept
/// so zooming back out continues the track. Off still goes through
/// start_audio(volume<=0) / stop_audio and releases the player.
pub(crate) fn fade_audio(planet: &mut Planet, volume: f32) {
    planet.audio_volume = volume;
    let Some(ptr) = planet.audio else {
        return;
    };
    unsafe {
        let player = ptr as *mut Object;
        if player.is_null() {
            return;
        }
        let _: () = msg_send![player, setVolume: volume.max(0.0)];
    }
}

fn player_path(player: *mut Object) -> Option<String> {
    unsafe {
        if player.is_null() {
            return None;
        }
        let url: *mut Object = msg_send![player, url];
        if url.is_null() {
            return None;
        }
        let path: *mut Object = msg_send![url, path];
        if path.is_null() {
            return None;
        }
        let utf8: *const c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Boombox: one AVAudioPlayer per planet. Same file + already loaded →
/// fade only (play/pause, volume step, zoom). New file / first play →
/// alloc + play, loop forever. Volume 0 (Off step) releases the player
/// — never a silent spawn. Audio failure is silent — the rail and the
/// planet never block on it.
pub(crate) fn start_audio(planet: &mut Planet, music: &str, volume: f32) -> bool {
    if volume <= 0.0 {
        stop_audio(planet);
        planet.audio_volume = volume;
        return true;
    }
    if music.is_empty() || !std::path::Path::new(music).is_file() {
        stop_audio(planet);
        return false;
    }
    if let Some(ptr) = planet.audio {
        let player = ptr as *mut Object;
        if player_path(player).as_deref() == Some(music) {
            fade_audio(planet, volume);
            unsafe {
                let playing: bool = msg_send![player, isPlaying];
                if !playing {
                    let ok: bool = msg_send![player, play];
                    if !ok {
                        stop_audio(planet);
                        return false;
                    }
                }
            }
            return true;
        }
    }
    stop_audio(planet);
    planet.audio_volume = volume;
    unsafe {
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_string(music)];
        if url.is_null() {
            return false;
        }
        let mut err: *mut Object = std::ptr::null_mut();
        let player: *mut Object = msg_send![class!(AVAudioPlayer), alloc];
        let player: *mut Object = msg_send![player, initWithContentsOfURL: url error: &mut err];
        if player.is_null() {
            return false;
        }
        let _: () = msg_send![player, setVolume: volume.max(0.0)];
        let _: () = msg_send![player, setNumberOfLoops: -1i64];
        let ok: bool = msg_send![player, play];
        if !ok {
            let _: () = msg_send![player, release];
            return false;
        }
        planet.audio = Some(player as usize);
        true
    }
}

pub(crate) fn stop_audio(planet: &mut Planet) {
    if let Some(ptr) = planet.audio.take() {
        unsafe {
            let player = ptr as *mut Object;
            if !player.is_null() {
                let _: () = msg_send![player, stop];
                let _: () = msg_send![player, release];
            }
        }
    }
}

// choose-music picker (GO-ORBITTY-BOOMBOX-KILL)

/// While true, DisplayLink posts nothing and cursor_tick skips boombox
/// starts. The old beachball was runModal *during* vsync setFrameOrigin.
pub(crate) static MUSIC_PICKER_OPEN: AtomicBool = AtomicBool::new(false);

struct PickerFlag;
impl Drop for PickerFlag {
    fn drop(&mut self) {
        MUSIC_PICKER_OPEN.store(false, Ordering::SeqCst);
    }
}

extern "C" {
    #[link_name = "_dispatch_main_q"]
    static DISPATCH_MAIN_Q: c_void;
    fn dispatch_async_f(
        queue: *mut c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

/// Always hop to the *next* main-queue turn — even if we are already on
/// main. `main_thread_async` runs inline on main, which is still inside
/// `popUpContextMenu`'s nested loop; NSOpenPanel never becomes visible
/// from there (the "Choose music does nothing" bug). The boombox HUD's
/// event monitors hop through the same door.
pub(crate) fn defer_main<F: FnOnce() + Send + 'static>(f: F) {
    extern "C" fn trampoline<F: FnOnce()>(ctx: *mut c_void) {
        let f = unsafe { Box::from_raw(ctx as *mut F) };
        let _ = catch_unwind(AssertUnwindSafe(f));
    }
    let ctx = Box::into_raw(Box::new(f)) as *mut c_void;
    unsafe {
        dispatch_async_f(
            std::ptr::addr_of!(DISPATCH_MAIN_Q) as *mut c_void,
            ctx,
            trampoline::<F>,
        );
    }
}

pub(crate) extern "C" fn menu_choose_music(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(id) = unsafe { sender_project_id(sender) } {
            begin_choose_music(&id);
        }
    }));
}

/// Open the music picker. Never a sheet on the planet (non-activating
/// NSPanel — that abort is the right-click crash). Never holds the
/// habitat lock across the panel. Never presents from inside an NSMenu
/// tracking loop.
pub(crate) fn begin_choose_music(id: &str) {
    MUSIC_PICKER_OPEN.store(true, Ordering::SeqCst);
    let id = id.to_string();
    defer_main(move || {
        let _hold = PickerFlag;
        present_music_picker(&id);
    });
}

fn present_music_picker(id: &str) {
    {
        let mut h = lock();
        if let Some(planet) = h.planets.get_mut(id) {
            set_caption(planet, "Choose music…".into());
        }
    }
    let picked = objc_try_open_panel();
    let Some(picked) = picked else {
        return;
    };
    if picked.is_empty() {
        return;
    }
    let mut h = lock();
    let (remembered, muted) = {
        let world = world_entry(&mut h, id);
        world.music = picked.clone();
        // BOOMBOX click leftover: the HUD carries no play/pause — the
        // pick IS the play door (old flow: menu Play after Choose
        // music). Never faked: start_audio below owns the outcome.
        world.boombox = true;
        (world.boombox_volume, world.boombox_muted)
    };
    h.worlds_dirty = true;
    let reduce = h.reduce_motion;
    if let Some(planet) = h.planets.get_mut(id) {
        // Mirror the world playing flag onto the live planet — draw uses
        // planet.boombox for full vs greyed alpha. start_audio alone left
        // the sprite dim while the track already played.
        planet.boombox = true;
        // start_audio owns every case: same file fades, new file swaps,
        // live 0 (hard mute / marble) stops — never a silent spawn; the
        // cursor_tick fader door brings it up on unmute / growth.
        let live = if muted {
            0.0
        } else {
            live_volume(remembered, planet.radius, reduce)
        };
        start_audio(planet, &picked, live);
        let file = picked.rsplit('/').next().unwrap_or(&picked);
        set_caption(planet, format!("Music: {file}"));
        unsafe {
            let view = planet.view as *mut Object;
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

extern "C" {
    fn orbitty_objc_try(fn_: extern "C" fn(*mut c_void), ctx: *mut c_void) -> i32;
}

fn objc_try_open_panel() -> Option<String> {
    let mut slot: Option<String> = None;
    extern "C" fn go(ctx: *mut c_void) {
        let slot = unsafe { &mut *(ctx as *mut Option<String>) };
        *slot = catch_unwind(AssertUnwindSafe(|| unsafe { run_open_panel() }))
            .ok()
            .flatten();
    }
    let ok = unsafe { orbitty_objc_try(go, &mut slot as *mut _ as *mut c_void) };
    if ok != 0 {
        slot
    } else {
        None
    }
}

/// macOS 14+ dropped NSApplication.activateWithOptions:. Sending it on
/// 26 aborts (unrecognized selector) — that was the 16:43 boombox crash.
unsafe fn activate_orbitty(app: *mut Object) {
    if app.is_null() {
        return;
    }
    let sel_activate = sel!(activate);
    let can_activate: bool = msg_send![app, respondsToSelector: sel_activate];
    if can_activate {
        let _: () = msg_send![app, activate];
    }
    let current: *mut Object = msg_send![class!(NSRunningApplication), currentApplication];
    if current.is_null() {
        return;
    }
    let sel_from = sel!(activateFromApplication:);
    let can_from: bool = msg_send![current, respondsToSelector: sel_from];
    if can_from {
        let _: bool = msg_send![current, activateFromApplication: current];
    }
}

/// Same door as walker "Use my image…" (`packs.rs` runModal), after the
/// menu has closed and Orbitty is actually the active app.
unsafe fn run_open_panel() -> Option<String> {
    let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
    if panel.is_null() {
        return None;
    }
    // No setAllowedFileTypes: any file; AVAudioPlayer fails silent on
    // the unplayable. The audio-extension filter was part of the beachball.
    let _: () = msg_send![panel, setCanChooseFiles: true];
    let _: () = msg_send![panel, setCanChooseDirectories: false];
    let _: () = msg_send![panel, setAllowsMultipleSelection: false];
    let _: () = msg_send![panel, setTitle: ns_string("Boombox music")];

    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    let old_policy: i64 = msg_send![app, activationPolicy];
    // Regular (0): NSOpenPanel cannot become key as Accessory/Prohibited.
    if old_policy != 0 {
        let _: bool = msg_send![app, setActivationPolicy: 0i64];
    }
    activate_orbitty(app);
    let _: () = msg_send![panel, center];
    let response: isize = msg_send![panel, runModal];
    if old_policy != 0 {
        let _: bool = msg_send![app, setActivationPolicy: old_policy];
    }
    if response != 1 {
        return None;
    }
    let url: *mut Object = msg_send![panel, URL];
    if url.is_null() {
        return None;
    }
    let path_obj: *mut Object = msg_send![url, path];
    if path_obj.is_null() {
        return None;
    }
    let utf8: *const c_char = msg_send![path_obj, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}
