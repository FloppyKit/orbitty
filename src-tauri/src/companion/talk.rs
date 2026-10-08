//! Companion Talk.
//!
//! Default path: macOS Speech.framework (press-to-start / press-to-stop).
//! Optional path: Handy PTT chord (Ctrl+Space) when Settings → Talk is
//! Hotkey. `ORBITTY_TALK=handy|system` overrides for Pixel, not shipping UI.
//!
//! webkitSpeechRecognition dies in WKWebView after a few seconds — never
//! the shipping engine. Never always-on. Never a wake word. Never auto-send.

use std::ffi::{c_char, CStr};
use std::os::raw::c_void;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

static HELD: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

#[cfg(target_os = "macos")]
mod keys {
    use super::*;

    const HID_SOURCE: u32 = 1; // kCGEventSourceStateHIDSystemState
    const HID_TAP: u32 = 0; // kCGHIDEventTap
    const FLAG_CONTROL: u64 = 0x0004_0000; // kCGEventFlagMaskControl
    const VK_SPACE: u16 = 0x31;
    const VK_CONTROL: u16 = 0x3B; // left control — Handy binding is ctrl_left+space

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state_id: u32) -> *mut c_void;
        fn CGEventCreateKeyboardEvent(
            source: *mut c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    fn post(source: *mut c_void, vk: u16, down: bool, flags: u64) {
        unsafe {
            let ev = CGEventCreateKeyboardEvent(source, vk, down);
            if ev.is_null() {
                return;
            }
            CGEventSetFlags(ev, flags);
            CGEventPost(HID_TAP, ev);
            CFRelease(ev);
        }
    }

    pub fn down() {
        unsafe {
            let src = CGEventSourceCreate(HID_SOURCE);
            if src.is_null() {
                return;
            }
            post(src, VK_CONTROL, true, FLAG_CONTROL);
            post(src, VK_SPACE, true, FLAG_CONTROL);
            CFRelease(src);
        }
    }

    pub fn up() {
        unsafe {
            let src = CGEventSourceCreate(HID_SOURCE);
            if src.is_null() {
                return;
            }
            post(src, VK_SPACE, false, FLAG_CONTROL);
            post(src, VK_CONTROL, false, 0);
            CFRelease(src);
        }
    }
}

#[cfg(target_os = "macos")]
mod engine {
    use super::*;

    type TalkCb = Option<unsafe extern "C" fn(*const c_char, *const c_char)>;

    #[link(name = "orbitty_talk_mac", kind = "static")]
    extern "C" {
        fn orbitty_talk_engine_start(cb: TalkCb) -> i32;
        fn orbitty_talk_engine_stop();
    }

    pub unsafe extern "C" fn on_event(event: *const c_char, text: *const c_char) {
        if event.is_null() {
            return;
        }
        let Some(app) = APP.get() else {
            return;
        };
        let ev = unsafe { CStr::from_ptr(event) }.to_string_lossy();
        let payload = if text.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        };
        match ev.as_ref() {
            "partial" => {
                let _ = app.emit("talk:partial", payload);
            }
            "ended" => {
                let _ = app.emit("talk:ended", payload);
            }
            "error" => {
                let _ = app.emit("talk:error", payload);
            }
            _ => {}
        }
    }

    pub fn start() -> Result<(), String> {
        let rc = unsafe { orbitty_talk_engine_start(Some(on_event)) };
        match rc {
            0 => Ok(()),
            1 => Err("Mic or speech permission denied — type instead.".into()),
            2 => Err("System speech isn’t available — type instead.".into()),
            4 => Err("Allow Mic and Speech, then click Talk again.".into()),
            _ => Err("System speech failed to start — type instead.".into()),
        }
    }

    pub fn stop() {
        unsafe { orbitty_talk_engine_stop() };
    }
}

fn handy_running() -> bool {
    std::process::Command::new("/usr/bin/pgrep")
        .args(["-xq", "handy"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn focus_companion(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("companion") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn bind_app(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

fn talk_source(app: &AppHandle) -> String {
    if let Ok(v) = std::env::var("ORBITTY_TALK") {
        if !v.trim().is_empty() {
            return crate::state::normalize_talk_source(&v);
        }
    }
    crate::state::load_state(app)
        .map(|s| crate::state::normalize_talk_source(&s.companion.talk_source))
        .unwrap_or_else(|_| "system".into())
}

fn release_handy() {
    if HELD.swap(false, Ordering::SeqCst) {
        #[cfg(target_os = "macos")]
        keys::up();
    }
}

fn stop_system() {
    #[cfg(target_os = "macos")]
    engine::stop();
}

#[tauri::command]
pub fn talk_start(app: AppHandle) -> Result<(), String> {
    bind_app(&app);
    let source = talk_source(&app);
    if source == "hotkey" {
        stop_system();
        if !handy_running() {
            return Err("Handy isn't running — type instead.".into());
        }
        focus_companion(&app);
        if HELD.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        keys::down();
        #[cfg(not(target_os = "macos"))]
        {
            HELD.store(false, Ordering::SeqCst);
            return Err("Talk needs macOS.".into());
        }
        return Ok(());
    }

    release_handy();
    #[cfg(not(target_os = "macos"))]
    {
        return Err("Talk needs macOS.".into());
    }
    #[cfg(target_os = "macos")]
    {
        focus_companion(&app);
        engine::start()
    }
}

#[tauri::command]
pub fn talk_stop() -> Result<(), String> {
    release_handy();
    stop_system();
    Ok(())
}

/// Companion hide / destroy — never leave Ctrl+Space stuck, never leave
/// the recognizer running.
pub fn stop_if_any() {
    release_handy();
    stop_system();
}
