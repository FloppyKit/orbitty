//! Global chords for the dest tiles, riding the
//! same `tauri_plugin_global_shortcut` as the ⌘⇧O summon and the action
//! combos. These are NOT user `actionShortcuts` — reserved like summon
//! (actions.rs `is_reserved` refuses them for action rows, so a
//! collision marks the action `conflict` and the tile chord wins).
//!
//!   ⌘⌥1 / ⌘⌥2 / ⌘⌥3   Launchpad / STARS / Planets — the same move as
//!                     the dest chips: drop from idle, yaw from a hosted
//!                     sibling, dismiss when that dest is already up.
//!                     The page owns `hosted`, so the toggle lives there.
//!   ⌘⌥← / ⌘⌥→         While hosted: yaw the ring (Launchpad → STARS →
//!                     Planets → Launchpad). Idle: no-op — never summon.
//!   ⌘⌥↓               Dismiss whichever dest is hosted (Launchpad,
//!                     STARS, or Planets). Idle: no-op — never summon,
//!                     same as the yaws.
//!   ⌘⌥↑               Raise the last dest that was lowered (Launchpad
//!                     if none this session). This one DOES summon from
//!                     idle — same posture as ⌘⌥1/2/3 — but no-ops while
//!                     a dest is already up (a menu is already up).
//!
//! ⌘Space stays Spotlight; Super-only Mac menu chords stay refused
//! (actions.rs reserved list); ⌘⌥+letter stays the action lane — dests
//! are never registered as letters.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

/// Arrow keys repeat. One Pressed + a repeat on a 3-dest ring is the
/// other neighbor (looks inverted). Ignore dest chords inside the yaw.
const DEST_DEBOUNCE_MS: u64 = 450;
static LAST_DEST_MS: AtomicU64 = AtomicU64::new(0);

fn dest_chord_ok() -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_DEST_MS.load(Ordering::SeqCst);
    if now.saturating_sub(last) < DEST_DEBOUNCE_MS {
        return false;
    }
    LAST_DEST_MS.store(now, Ordering::SeqCst);
    true
}

const DESTS: [(Code, &str); 3] = [
    (Code::Digit1, "launchpad"),
    (Code::Digit2, "stars"),
    (Code::Digit3, "inspector"),
];
const YAWS: [(Code, &str); 2] = [
    (Code::ArrowLeft, "prev"),
    (Code::ArrowRight, "next"),
];

/// Register the seven tile chords. Best-effort per chord (same philosophy
/// as actions.rs — an OS/other-app collision is logged, never a crash):
/// a chord that fails to register simply stays dead.
#[cfg(target_os = "macos")]
pub fn register(app: &tauri::AppHandle) {
    let gs = app.global_shortcut();
    for (code, mode) in DESTS {
        let chord = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), code);
        let handle = app.clone();
        if let Err(e) = gs.on_shortcut(chord, move |_app, _s, ev| {
            if ev.state() != ShortcutState::Pressed {
                return;
            }
            if !dest_chord_ok() {
                return;
            }
            // A global chord must work from a hidden companion: raise it
            // native-side first (same as the tray dest items), then ask
            // the page to toggle — enterHost / dismissHost own the host
            // frame from there (drop, yaw, or dismiss-when-same).
            let _ = crate::panes::show_companion(&handle);
            let _ = handle.emit_to("companion", "orbitty:toggle-host", mode);
        }) {
            eprintln!("tile-chords: ⌘⌥{mode} register failed: {e}");
        }
    }
    for (code, dir) in YAWS {
        let chord = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), code);
        let handle = app.clone();
        if let Err(e) = gs.on_shortcut(chord, move |_app, _s, ev| {
            if ev.state() != ShortcutState::Pressed {
                return;
            }
            if !dest_chord_ok() {
                return;
            }
            // No show_companion here — idle is a no-op, never a summon.
            // The page no-ops when nothing is hosted.
            let _ = handle.emit_to("companion", "orbitty:yaw-host", dir);
        }) {
            eprintln!("tile-chords: ⌘⌥{dir} register failed: {e}");
        }
    }
    // ⌘⌥↓ dismisses whichever dest is hosted. Same posture as the yaws:
    // no show_companion — idle is a no-op, never a summon. The page
    // no-ops when nothing is hosted.
    let chord = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowDown);
    let handle = app.clone();
    if let Err(e) = gs.on_shortcut(chord, move |_app, _s, ev| {
        if ev.state() != ShortcutState::Pressed {
            return;
        }
        if !dest_chord_ok() {
            return;
        }
        let _ = handle.emit_to("companion", "orbitty:dismiss-host", ());
    }) {
        eprintln!("tile-chords: ⌘⌥↓ register failed: {e}");
    }
    // ⌘⌥↑ raises the last lowered dest (the page remembers; Launchpad if
    // none). Opposite posture from ↓: this one DOES show_companion first
    // — same as the ⌘⌥1/2/3 dest chords — because idle must work. The
    // page no-ops while a dest is already hosted.
    let chord = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowUp);
    let handle = app.clone();
    if let Err(e) = gs.on_shortcut(chord, move |_app, _s, ev| {
        if ev.state() != ShortcutState::Pressed {
            return;
        }
        if !dest_chord_ok() {
            return;
        }
        let _ = crate::panes::show_companion(&handle);
        let _ = handle.emit_to("companion", "orbitty:raise-last-host", ());
    }) {
        eprintln!("tile-chords: ⌘⌥↑ register failed: {e}");
    }
    // ⌘? (cmd+shift+slash) — own cheat-sheet window (click-off hides).
    let chord = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Slash);
    let handle = app.clone();
    if let Err(e) = gs.on_shortcut(chord, move |_app, _s, ev| {
        if ev.state() != ShortcutState::Pressed {
            return;
        }
        let _ = crate::panes::toggle_cheatsheet(handle.clone());
    }) {
        eprintln!("tile-chords: ⌘? register failed: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn register(_app: &tauri::AppHandle) {}
