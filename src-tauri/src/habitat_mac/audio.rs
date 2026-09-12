//! Boombox AVAudioPlayer + choose-music picker.
//! Zoom fades volume in place — never restart the track.
//! Off stops the player. Marble (MIN) is silent.

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

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

// ---------- choose-music picker (GO-ORBITTY-BOOMBOX-KILL) ----------

/// One picker at a time. While up, the DisplayLink callback posts nothing
/// (motion paused) and cursor_tick skips boombox starts — the old
/// beachball was runModal from a non-activating app with the vsync clock
/// churning under it.
pub(crate) static MUSIC_PICKER_OPEN: AtomicBool = AtomicBool::new(false);
/// Panel being shown (retained until the completion handler releases it).
static MUSIC_PICKER_PANEL: AtomicUsize = AtomicUsize::new(0);

/// Project the open picker belongs to (a capture-less block can't carry
/// it; the picker is singleton-gated so one slot suffices).
fn music_picker_project() -> &'static Mutex<Option<String>> {
    static PROJECT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    PROJECT.get_or_init(|| Mutex::new(None))
}

/// Minimal block ABI for `beginWithCompletionHandler:`. Capture-less and
/// marked BLOCK_IS_GLOBAL, so AppKit's Block_copy is a no-op and the
/// leaked box from completion_block() stays valid forever.
#[repr(C)]
struct CompletionBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const CompletionBlock, isize),
    descriptor: *const CompletionBlockDescriptor,
}

#[repr(C)]
struct CompletionBlockDescriptor {
    reserved: std::os::raw::c_ulong,
    size: std::os::raw::c_ulong,
}

extern "C" {
    #[link_name = "_NSConcreteGlobalBlock"]
    static NS_CONCRETE_GLOBAL_BLOCK: c_void;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static COMPLETION_BLOCK_DESCRIPTOR: CompletionBlockDescriptor = CompletionBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<CompletionBlock>() as std::os::raw::c_ulong,
};

fn completion_block() -> *const CompletionBlock {
    // Stored as usize — the file's convention for raw pointers in statics.
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        Box::into_raw(Box::new(CompletionBlock {
            // SAFETY: referencing the libSystem global-block class.
            isa: unsafe { &NS_CONCRETE_GLOBAL_BLOCK as *const c_void },
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: choose_music_done,
            descriptor: &COMPLETION_BLOCK_DESCRIPTOR,
        })) as usize
    }) as *const CompletionBlock
}

/// NSOpenPanel completion (main thread): resume motion + starts, then
/// apply the pick. An unplayable file fails silent later in cursor_tick
/// (player init/play fails → pick cleared, caption "Choose music…").
extern "C" fn choose_music_done(_block: *const CompletionBlock, response: isize) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        MUSIC_PICKER_OPEN.store(false, Ordering::SeqCst);
        let panel = MUSIC_PICKER_PANEL.swap(0, Ordering::SeqCst) as *mut Object;
        let id = music_picker_project()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let mut picked = String::new();
        if !panel.is_null() {
            if response == 1 {
                // NSModalResponseOK
                let url: *mut Object = msg_send![panel, URL];
                if !url.is_null() {
                    let path_obj: *mut Object = msg_send![url, path];
                    if !path_obj.is_null() {
                        let utf8: *const std::os::raw::c_char = msg_send![path_obj, UTF8String];
                        if !utf8.is_null() {
                            picked = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
                        }
                    }
                }
            }
            let _: () = msg_send![panel, release];
        }
        let Some(id) = id else { return };
        if picked.is_empty() {
            return;
        }
        let mut h = lock();
        let (boombox, remembered) = {
            let world = world_entry(&mut h, &id);
            world.music = picked.clone();
            (world.boombox, world.boombox_volume)
        };
        h.worlds_dirty = true;
        let reduce = h.reduce_motion;
        if let Some(planet) = h.planets.get_mut(&id) {
            if boombox {
                let live = live_volume(remembered, planet.radius, reduce);
                start_audio(planet, &picked, live);
            }
            let file = picked.rsplit('/').next().unwrap_or(&picked);
            set_caption(planet, format!("Music: {file}"));
        }
    }));
}

pub(crate) extern "C" fn menu_choose_music(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(id) = unsafe { sender_project_id(sender) } {
            begin_choose_music(&id);
        }
    }));
}

/// Open the music picker as a sheet on the planet window when we have
/// one, otherwise a free-floating panel. Never holds the habitat lock
/// across the picker.
pub(crate) fn begin_choose_music(id: &str) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if MUSIC_PICKER_OPEN.swap(true, Ordering::SeqCst) {
            return;
        }
        let host = {
            let h = lock();
            h.planets.get(id).map(|p| p.panel).unwrap_or(0)
        };
        let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
        if panel.is_null() {
            MUSIC_PICKER_OPEN.store(false, Ordering::SeqCst);
            return;
        }
        let _: *mut Object = msg_send![panel, retain];
        let _: () = msg_send![panel, setCanChooseFiles: true];
        let _: () = msg_send![panel, setCanChooseDirectories: false];
        let _: () = msg_send![panel, setAllowsMultipleSelection: false];
        let _: () = msg_send![panel, setTitle: ns_string("Boombox music")];
        *music_picker_project()
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(id.to_string());
        MUSIC_PICKER_PANEL.store(panel as usize, Ordering::SeqCst);
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, activateWithOptions: 2usize];
        let hostp = host as *mut Object;
        if !hostp.is_null() && host != 0 {
            let _: () = msg_send![panel,
                beginSheetModalForWindow: hostp
                completionHandler: completion_block()];
        } else {
            let _: () = msg_send![panel, beginWithCompletionHandler: completion_block()];
        }
    }));
}
