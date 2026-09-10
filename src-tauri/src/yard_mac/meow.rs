//! Yard meows — GO-ORBITTY-HERDING-CATS. While the clowder is out, one
//! `brand/yard/meow_{0,1,2}.wav` (bundled to Resources/yard/) every
//! 2–6s, jittered, ONE voice at a time. Skipped while any planet boombox
//! is actually playing (never stack). Killed on every exit — the
//! boombox-kill law: no orphan audio. AVAudioPlayer, never afplay.
//!
//! Mute: playback rides the system output, so system volume/mute apply
//! as-is. There is no in-app master mute to mirror (boombox volume is a
//! per-planet fader and never reaches the yard).
//!
//! Everything here runs on the main thread (the yard hop); the player
//! pointer is a usize in the yard state like every other raw ObjC ptr.

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};
use tauri::Manager;

use super::{app_handle, ns_string, rand_range};

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

pub(crate) struct Meow {
    /// Live AVAudioPlayer, 0 = silent. One voice: a new meow never starts
    /// while this is playing.
    player: usize,
    next_at: Instant,
}

impl Default for Meow {
    fn default() -> Self {
        Meow {
            player: 0,
            next_at: Instant::now(),
        }
    }
}

impl Meow {
    /// First meow lands inside the first jitter window — the rain should
    /// be heard as it lands, not after a long silence.
    pub(crate) fn armed() -> Self {
        Meow {
            player: 0,
            next_at: Instant::now() + Duration::from_secs_f64(rand_range(1.0, 2.5)),
        }
    }
}

/// One bundled voice, picked at random. None when the resource is
/// missing (never re-read a absent file every hop — a miss is cheap and
/// the next jitter window tries again).
fn meow_path(n: usize) -> Option<std::path::PathBuf> {
    let app = app_handle()?;
    let dir = app.path().resource_dir().ok()?.join("yard");
    let p = dir.join(format!("meow_{n}.wav"));
    p.is_file().then_some(p)
}

/// Main-thread hop: release a finished player, then maybe voice the next
/// meow. `cats_out` false = silent (an empty overlay never meows).
pub(crate) fn tick(m: &mut Meow, cats_out: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if m.player != 0 {
            let player = m.player as *mut Object;
            let playing: bool = msg_send![player, isPlaying];
            if !playing {
                let _: () = msg_send![player, release];
                m.player = 0;
            }
        }
        if !cats_out || Instant::now() < m.next_at {
            return;
        }
        m.next_at = Instant::now() + Duration::from_secs_f64(rand_range(2.0, 6.0));
        if m.player != 0 {
            return; // one voice at a time
        }
        // Boombox owns the air — skip this window entirely (do not stack).
        if crate::habitat::boombox_playing() {
            return;
        }
        let pick = (rand_range(0.0, 3.0) as usize).min(2);
        // Fall back through the trio if one file is missing.
        let path = (0..3).find_map(|k| meow_path((pick + k) % 3));
        let Some(path) = path else {
            return;
        };
        let Some(path_str) = path.to_str() else {
            return;
        };
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_string(path_str)];
        if url.is_null() {
            return;
        }
        let mut err: *mut Object = std::ptr::null_mut();
        let player: *mut Object = msg_send![class!(AVAudioPlayer), alloc];
        let player: *mut Object = msg_send![player, initWithContentsOfURL: url error: &mut err];
        if player.is_null() {
            return;
        }
        let _: () = msg_send![player, setNumberOfLoops: 0i64];
        let ok: bool = msg_send![player, play];
        if !ok {
            let _: () = msg_send![player, release];
            return;
        }
        m.player = player as usize;
    }))
    .map_err(|p| eprintln!("yard: meow panicked: {}", super::panic_msg(&*p)));
}

/// Every exit path (scoop one-to-empty, scoop all, Esc, ⌘⇧Y, Settings
/// while on, app quit) kills the voice — no orphan audio, ever.
pub(crate) fn kill(m: &mut Meow) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if m.player != 0 {
            let player = m.player as *mut Object;
            let _: () = msg_send![player, stop];
            let _: () = msg_send![player, release];
            m.player = 0;
        }
    }))
    .map_err(|p| eprintln!("yard: meow kill panicked: {}", super::panic_msg(&*p)));
}
