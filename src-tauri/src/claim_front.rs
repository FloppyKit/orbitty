//! Layer-0 claim door: tray, Orbitty Dock menu, planet "Put front window here".
//! Writes through `board::save_with_events` so Rail/Inspector need not be open.
//! No minimize nag. Dock menu is Orbitty-only (applicationDockMenu).

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::board::{self, save_with_events};
use crate::state::{Claim, PersistedState, Project};
use crate::windows::WindowInfo;

#[derive(Clone)]
struct PendingUndo {
    before: PersistedState,
    window_id: String,
    expires: SystemTime,
}

static PENDING: Mutex<Option<PendingUndo>> = Mutex::new(None);

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}


fn last_used_project(st: &PersistedState) -> Option<&Project> {
    if let Some(id) = st.last_claim_project_id.as_deref() {
        if let Some(p) = st.projects.iter().find(|p| p.id == id) {
            return Some(p);
        }
    }
    let mut best: Option<(&str, f64)> = None;
    for c in &st.claims {
        if !st.projects.iter().any(|p| p.id == c.project_id) {
            continue;
        }
        if best.map(|(_, t)| c.last_touched_at >= t).unwrap_or(true) {
            best = Some((c.project_id.as_str(), c.last_touched_at));
        }
    }
    best.and_then(|(id, _)| st.projects.iter().find(|p| p.id == id))
        .or_else(|| st.projects.first())
}

fn title_index_for(list: &[WindowInfo], w: &WindowInfo) -> u32 {
    let bundle = w.bundle_id.as_deref().unwrap_or(w.app_name.as_str());
    let mut n = 0u32;
    for x in list {
        let b = x.bundle_id.as_deref().unwrap_or(x.app_name.as_str());
        if b == bundle && x.title == w.title {
            n += 1;
            if x.id == w.id {
                return n.max(1);
            }
        }
    }
    1
}

fn roll_char_seed() -> u64 {
    use std::io::Read;
    let mut b = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    u64::from_le_bytes(b)
}

fn apply_claim(st: &mut PersistedState, w: &WindowInfo, project_id: &str, list: &[WindowInfo]) {
    let now = now_secs();
    let prev = st.claims.iter().find(|c| c.window_id == w.id).cloned();
    st.claims.retain(|c| c.window_id != w.id);
    st.claims.push(Claim {
        window_id: w.id.clone(),
        project_id: project_id.to_string(),
        last_touched_at: now,
        bundle_id: w.bundle_id.clone(),
        app_name: Some(w.app_name.clone()),
        title_at_claim: Some(w.title.clone()),
        title_index: Some(title_index_for(list, w)),
        sprite_pack_id: prev.as_ref().and_then(|c| c.sprite_pack_id.clone()),
        char_seed: Some(
            prev.as_ref()
                .and_then(|c| c.char_seed)
                .unwrap_or_else(roll_char_seed),
        ),
        char_look: prev.as_ref().and_then(|c| c.char_look.clone()),
        sprite_scale: prev.as_ref().and_then(|c| c.sprite_scale),
        walker_home: None,
        file_path: w
            .document_path
            .clone()
            .or_else(|| prev.as_ref().and_then(|c| c.file_path.clone())),
        hermes_profile: prev.as_ref().and_then(|c| c.hermes_profile.clone()),
        hermes_session: prev.as_ref().and_then(|c| c.hermes_session.clone()),
        action_id: None,
        command: None,
        cwd: None,
    });
    st.last_claim_project_id = Some(project_id.to_string());

    let shown = if w.title.is_empty() {
        w.app_name.clone()
    } else {
        format!("{} — {}", w.app_name, w.title)
    };
    // Bare claim = walker on the grass only. A board card is born only
    // when a todo is assigned to this window (`kanban.assign --window`).
    // If a non-done card already names this window, keep the link and
    // retarget the project — never force status back to todo / Stand.
    if let Some(open) = st
        .kanban_cards
        .iter_mut()
        .find(|c| c.window_id.as_deref() == Some(w.id.as_str()) && c.status != "done")
    {
        open.project_id = project_id.to_string();
        open.window_id = Some(w.id.clone());
        if open.title.is_empty() {
            open.title = shown;
        }
        open.last_touched_at = now;
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaimToastPayload {
    pub project_id: String,
    pub project_name: String,
    pub window_id: String,
    /// Short label for the toast — "App — title" or just app name.
    pub window_title: String,
    pub seconds: u32,
}

fn arm_undo(before: PersistedState, window_id: &str) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(PendingUndo {
            before,
            window_id: window_id.to_string(),
            expires: SystemTime::now() + Duration::from_secs(3),
        });
    }
}

fn emit_toast(app: &AppHandle, project: &Project, w: &WindowInfo) {
    let window_title = if w.title.is_empty() {
        w.app_name.clone()
    } else {
        format!("{} — {}", w.app_name, w.title)
    };
    let _ = app.emit(
        "orbitty:claim-toast",
        ClaimToastPayload {
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            window_id: w.id.clone(),
            window_title,
            seconds: 3,
        },
    );
}

pub fn undo_last(app: &AppHandle) -> Result<(), String> {
    let pending = PENDING
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .ok_or_else(|| "nothing to undo".to_string())?;
    if SystemTime::now() > pending.expires {
        return Err("undo expired".into());
    }
    let current = crate::state::load_state(app)?;
    save_with_events(app, &current, &pending.before)?;
    Ok(())
}

pub fn change_planet(app: &AppHandle, project_id: &str) -> Result<(), String> {
    let window_id = PENDING
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.window_id.clone()))
        .ok_or_else(|| "nothing to change".to_string())?;
    let list = crate::windows::list_windows()?;
    let w = list
        .iter()
        .find(|x| x.id == window_id)
        .cloned()
        .ok_or_else(|| "window gone".to_string())?;
    claim_window(app, &w, Some(project_id), &list)
}

fn claim_window(
    app: &AppHandle,
    w: &WindowInfo,
    project_id: Option<&str>,
    list: &[WindowInfo],
) -> Result<(), String> {
    let before = crate::state::load_state(app)?;
    if before.projects.is_empty() {
        return Err("no planets".into());
    }
    let project = if let Some(id) = project_id {
        before
            .projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| format!("unknown planet {id}"))?
    } else {
        last_used_project(&before)
            .cloned()
            .ok_or_else(|| "no planets".to_string())?
    };
    let mut after = before.clone();
    apply_claim(&mut after, w, &project.id, list);
    arm_undo(before.clone(), &w.id);
    save_with_events(app, &before, &after)?;
    emit_toast(app, &project, &w);
    crate::habitat::bring_planet_front(&project.id);
    Ok(())
}

pub fn claim_front(app: &AppHandle) -> Result<(), String> {
    let list = crate::windows::list_windows()?;
    let Some(w) = crate::windows::frontmost_claimable()? else {
        return Err("no frontmost window to claim".into());
    };
    let st = crate::state::load_state(app)?;
    if st.projects.is_empty() {
        let name = ask_planet_name()?;
        let name = name.trim();
        if name.is_empty() {
            return Err("cancelled".into());
        }
        let project = board::project_add(app, name)?;
        let list = crate::windows::list_windows()?;
        return claim_window(app, &w, Some(&project.id), &list);
    }
    claim_window(app, &w, None, &list)
}

pub fn claim_front_to_planet(app: &AppHandle, project_id: &str) -> Result<(), String> {
    let list = crate::windows::list_windows()?;
    let Some(w) = crate::windows::frontmost_claimable()? else {
        return Err("no frontmost window to claim".into());
    };
    claim_window(app, &w, Some(project_id), &list)
}

#[cfg(target_os = "macos")]
fn ask_planet_name() -> Result<String, String> {
    use std::ffi::CString;
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    crate::main_thread_sync(move || {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSRect {
            origin: NSPoint,
            size: NSSize,
        }
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        unsafe {
            let alert: *mut Object = msg_send![class!(NSAlert), new];
            if alert.is_null() {
                let _ = tx.send(Err("alert alloc failed".into()));
                return;
            }
            let title = CString::new("Name this planet").unwrap();
            let info =
                CString::new("No planets yet — name one to claim the front window.").unwrap();
            let ns_title: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: title.as_ptr()];
            let ns_info: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: info.as_ptr()];
            let _: () = msg_send![alert, setMessageText: ns_title];
            let _: () = msg_send![alert, setInformativeText: ns_info];
            let ok = CString::new("Claim").unwrap();
            let cancel = CString::new("Cancel").unwrap();
            let ns_ok: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: ok.as_ptr()];
            let ns_cancel: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: cancel.as_ptr()];
            let _: () = msg_send![alert, addButtonWithTitle: ns_ok];
            let _: () = msg_send![alert, addButtonWithTitle: ns_cancel];
            let rect = NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize {
                    width: 260.0,
                    height: 24.0,
                },
            };
            let field: *mut Object = msg_send![class!(NSTextField), alloc];
            let field: *mut Object = msg_send![field, initWithFrame: rect];
            if !field.is_null() {
                let empty = CString::new("").unwrap();
                let ns_empty: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: empty.as_ptr()];
                let _: () = msg_send![field, setStringValue: ns_empty];
                let _: () = msg_send![alert, setAccessoryView: field];
            }
            let resp: isize = msg_send![alert, runModal];
            let mut out = String::new();
            if resp == 1000 && !field.is_null() {
                let val: *mut Object = msg_send![field, stringValue];
                if !val.is_null() {
                    let utf8: *const std::os::raw::c_char = msg_send![val, UTF8String];
                    if !utf8.is_null() {
                        out = std::ffi::CStr::from_ptr(utf8)
                            .to_string_lossy()
                            .into_owned();
                    }
                }
            }
            let _: () = msg_send![alert, release];
            if resp != 1000 {
                let _ = tx.send(Err("cancelled".into()));
            } else {
                let _ = tx.send(Ok(out));
            }
        }
    });
    rx.recv()
        .map_err(|_| "name sheet: no response".to_string())?
}

#[cfg(not(target_os = "macos"))]
fn ask_planet_name() -> Result<String, String> {
    Err("claim name sheet is macOS-only".into())
}

#[tauri::command]
pub fn claim_front_window(app: AppHandle) -> Result<(), String> {
    claim_front(&app)
}

#[tauri::command]
pub fn claim_front_undo(app: AppHandle) -> Result<(), String> {
    undo_last(&app)
}

#[tauri::command]
pub fn claim_front_change(app: AppHandle, project_id: String) -> Result<(), String> {
    change_planet(&app, &project_id)
}

#[tauri::command]
pub fn claim_front_to(app: AppHandle, project_id: String) -> Result<(), String> {
    claim_front_to_planet(&app, &project_id)
}


#[cfg(target_os = "macos")]
static DOCK_APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Orbitty Dock menu only — never other apps. Hooks `applicationDockMenu:`
/// onto the live NSApplication delegate class (Tauri's).
#[cfg(target_os = "macos")]
pub fn install_orbitty_dock_menu(app: AppHandle) {
    let _ = DOCK_APP.set(app);
    crate::main_thread_sync(|| unsafe {
        use objc::runtime::{Class, Object, Sel};
        use objc::{class, msg_send, sel, sel_impl};
        use std::os::raw::c_char;

        extern "C" {
            fn class_addMethod(
                cls: *const Class,
                name: Sel,
                imp: unsafe extern "C" fn(),
                types: *const c_char,
            ) -> bool;
            fn class_getInstanceMethod(cls: *const Class, name: Sel) -> *mut std::ffi::c_void;
            fn method_setImplementation(
                method: *mut std::ffi::c_void,
                imp: unsafe extern "C" fn(),
            ) -> unsafe extern "C" fn();
        }

        unsafe extern "C" fn dock_menu(
            _this: *mut Object,
            _cmd: Sel,
            _sender: *mut Object,
        ) -> *mut Object {
            use objc::runtime::Object;
            use objc::{class, msg_send, sel, sel_impl};
            unsafe {
                let menu: *mut Object = msg_send![class!(NSMenu), new];
                if menu.is_null() {
                    return std::ptr::null_mut();
                }
                let title = std::ffi::CString::new("Claim front window").unwrap();
                let ns_title: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: title.as_ptr()];
                let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
                let empty = std::ffi::CString::new("").unwrap();
                let ns_empty: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: empty.as_ptr()];
                let item: *mut Object = msg_send![item,
                    initWithTitle: ns_title
                    action: sel!(orbittyDockClaim:)
                    keyEquivalent: ns_empty];
                if !item.is_null() {
                    let _: () = msg_send![item, setTarget: _this];
                    let _: () = msg_send![menu, addItem: item];
                    let _: () = msg_send![item, release];
                }
                let title2 = std::ffi::CString::new("Undo last claim").unwrap();
                let ns_title2: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: title2.as_ptr()];
                let item2: *mut Object = msg_send![class!(NSMenuItem), alloc];
                let item2: *mut Object = msg_send![item2,
                    initWithTitle: ns_title2
                    action: sel!(orbittyDockUndo:)
                    keyEquivalent: ns_empty];
                if !item2.is_null() {
                    let _: () = msg_send![item2, setTarget: _this];
                    let _: () = msg_send![menu, addItem: item2];
                    let _: () = msg_send![item2, release];
                }
                // Autorelease so AppKit owns it for the popup.
                let _: *mut Object = msg_send![menu, autorelease];
                menu
            }
        }

        unsafe extern "C" fn dock_claim(_this: *mut Object, _cmd: Sel, _sender: *mut Object) {
            if let Some(app) = DOCK_APP.get() {
                if let Err(e) = crate::claim_front::claim_front(app) {
                    eprintln!("dock-claim: {e}");
                }
            }
        }

        unsafe extern "C" fn dock_undo(_this: *mut Object, _cmd: Sel, _sender: *mut Object) {
            if let Some(app) = DOCK_APP.get() {
                if let Err(e) = crate::claim_front::undo_last(app) {
                    eprintln!("dock-undo: {e}");
                }
            }
        }

        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if ns_app.is_null() {
            return;
        }
        let delegate: *mut Object = msg_send![ns_app, delegate];
        if delegate.is_null() {
            return;
        }
        let cls: *const Class = msg_send![delegate, class];
        if cls.is_null() {
            return;
        }
        // @"v@:@" for void methods; "@:@:" for dock menu returning id.
        let types_menu = std::ffi::CString::new("@@:@").unwrap();
        let types_void = std::ffi::CString::new("v@:@").unwrap();
        let add_or_replace = |name: Sel, imp: unsafe extern "C" fn(), types: &std::ffi::CString| {
            let existing = class_getInstanceMethod(cls, name);
            if existing.is_null() {
                let _ = class_addMethod(cls, name, imp, types.as_ptr());
            } else {
                let _ = method_setImplementation(existing, imp);
            }
        };
        add_or_replace(
            sel!(applicationDockMenu:),
            std::mem::transmute::<
                unsafe extern "C" fn(*mut Object, Sel, *mut Object) -> *mut Object,
                unsafe extern "C" fn(),
            >(dock_menu),
            &types_menu,
        );
        add_or_replace(
            sel!(orbittyDockClaim:),
            std::mem::transmute::<
                unsafe extern "C" fn(*mut Object, Sel, *mut Object),
                unsafe extern "C" fn(),
            >(dock_claim),
            &types_void,
        );
        add_or_replace(
            sel!(orbittyDockUndo:),
            std::mem::transmute::<
                unsafe extern "C" fn(*mut Object, Sel, *mut Object),
                unsafe extern "C" fn(),
            >(dock_undo),
            &types_void,
        );
    });
}

#[cfg(not(target_os = "macos"))]
pub fn install_orbitty_dock_menu(_app: AppHandle) {}


pub fn unclaim_window(app: &AppHandle, window_id: &str) -> Result<(), String> {
    let before = crate::state::load_state(app)?;
    if !before.claims.iter().any(|c| c.window_id == window_id) {
        return Err("not claimed".into());
    }
    let mut after = before.clone();
    after.claims.retain(|c| c.window_id != window_id);
    for card in after.kanban_cards.iter_mut() {
        if card.window_id.as_deref() == Some(window_id) {
            card.window_id = None;
            if card.kind.as_deref() == Some("window") {
                card.kind = Some("todo".into());
            }
        }
    }
    save_with_events(app, &before, &after)?;
    Ok(())
}

#[tauri::command]
pub fn claim_front_unclaim(app: AppHandle, window_id: String) -> Result<(), String> {
    unclaim_window(&app, &window_id)
}
