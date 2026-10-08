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

/// Who armed the pending claim. `User` is the tray, Dock menu, or app.
/// `Agent` is the control socket (MCP, orbittyctl, a raw client). The
/// string is the top-level request `caller`, not a tool argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimOrigin {
    User,
    Agent(String),
}

/// The open card `apply_claim` retargeted, with the fields to put back.
#[derive(Clone, Debug)]
struct CardUndo {
    id: String,
    prev_project_id: String,
    prev_window_id: Option<String>,
    prev_title: String,
    prev_last_touched_at: f64,
    /// Title and touch the claim itself wrote. Undo puts the previous
    /// values back only while the card still holds these.
    written_title: String,
    written_last_touched_at: f64,
}

/// One pending undo. Reverses that window's claim only — never a
/// whole-board snapshot — and only for the origin that armed it.
#[derive(Clone, Debug)]
struct PendingUndo {
    window_id: String,
    origin: ClaimOrigin,
    expires: SystemTime,
    prev_claim: Option<Claim>,
    written_project_id: String,
    prev_last_claim_project_id: Option<String>,
    card: Option<CardUndo>,
}

/// One slot per origin. An agent arm writes only the agent slot, so a
/// pending tray/dock/app undo is never overwritten.
struct PendingSlots {
    user: Option<PendingUndo>,
    agent: Option<PendingUndo>,
}

static PENDING: Mutex<PendingSlots> = Mutex::new(PendingSlots {
    user: None,
    agent: None,
});

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

fn capture_undo(
    before: &PersistedState,
    after: &PersistedState,
    window_id: &str,
    origin: ClaimOrigin,
    expires: SystemTime,
) -> PendingUndo {
    let prev_claim = before
        .claims
        .iter()
        .find(|c| c.window_id == window_id)
        .cloned();
    let written_project_id = after
        .claims
        .iter()
        .find(|c| c.window_id == window_id)
        .map(|c| c.project_id.clone())
        .unwrap_or_default();
    let card = before
        .kanban_cards
        .iter()
        .find(|c| c.window_id.as_deref() == Some(window_id) && c.status != "done")
        .map(|c| {
            let written = after.kanban_cards.iter().find(|a| a.id == c.id);
            CardUndo {
                id: c.id.clone(),
                prev_project_id: c.project_id.clone(),
                prev_window_id: c.window_id.clone(),
                prev_title: c.title.clone(),
                prev_last_touched_at: c.last_touched_at,
                written_title: written
                    .map(|a| a.title.clone())
                    .unwrap_or_else(|| c.title.clone()),
                written_last_touched_at: written
                    .map(|a| a.last_touched_at)
                    .unwrap_or(c.last_touched_at),
            }
        });
    PendingUndo {
        window_id: window_id.to_string(),
        origin,
        expires,
        prev_claim,
        written_project_id,
        prev_last_claim_project_id: before.last_claim_project_id.clone(),
        card,
    }
}

/// Expiry, then ownership. `User` may undo or change any pending claim.
/// An agent may touch only its own agent-origin claim.
fn check_pending(
    p: &PendingUndo,
    caller: &ClaimOrigin,
    now: SystemTime,
    verb: &str,
) -> Result<(), String> {
    if now > p.expires {
        return Err(format!("{verb} expired"));
    }
    match caller {
        ClaimOrigin::User => Ok(()),
        ClaimOrigin::Agent(a) => match &p.origin {
            ClaimOrigin::User => {
                Err(format!("claim was made from the tray/app; {verb} it there"))
            }
            ClaimOrigin::Agent(b) if a != b => Err("not your claim".into()),
            ClaimOrigin::Agent(_) => Ok(()),
        },
    }
}

/// Put back the claim row this pending armed. Unrelated state stays.
fn undo_claim_in(st: &mut PersistedState, p: &PendingUndo) -> Result<(), String> {
    let Some(idx) = st.claims.iter().position(|c| c.window_id == p.window_id) else {
        return Err("claim changed since; not undone".into());
    };
    if st.claims[idx].project_id != p.written_project_id {
        return Err("claim changed since; not undone".into());
    }
    match &p.prev_claim {
        Some(prev) => st.claims[idx] = prev.clone(),
        None => {
            st.claims.remove(idx);
        }
    }
    if st.last_claim_project_id.as_deref() == Some(p.written_project_id.as_str()) {
        st.last_claim_project_id = p.prev_last_claim_project_id.clone();
    }
    if let Some(cu) = &p.card {
        if let Some(card) = st.kanban_cards.iter_mut().find(|c| c.id == cu.id) {
            if card.project_id == p.written_project_id {
                card.project_id = cu.prev_project_id.clone();
                card.window_id = cu.prev_window_id.clone();
                if card.title == cu.written_title {
                    card.title = cu.prev_title.clone();
                }
                if card.last_touched_at == cu.written_last_touched_at {
                    card.last_touched_at = cu.prev_last_touched_at;
                }
            }
        }
    }
    Ok(())
}

/// Take the slot only when `check_pending` accepts. A refusal leaves it.
fn try_take(
    slot: &mut Option<PendingUndo>,
    caller: &ClaimOrigin,
    now: SystemTime,
) -> Result<PendingUndo, String> {
    let Some(p) = slot.as_ref() else {
        return Err("nothing to undo".into());
    };
    check_pending(p, caller, now, "undo")?;
    Ok(slot.take().expect("pending checked"))
}

enum PendingWhich {
    User,
    Agent,
}

/// User undo/change uses the user slot while it is unexpired, and falls
/// back to the agent slot only when the user slot is empty or expired.
/// An agent only ever sees the agent slot.
fn which_pending(slots: &PendingSlots, caller: &ClaimOrigin, now: SystemTime) -> PendingWhich {
    match caller {
        ClaimOrigin::Agent(_) => PendingWhich::Agent,
        ClaimOrigin::User => {
            let user_live = slots.user.as_ref().is_some_and(|p| now <= p.expires);
            if user_live {
                PendingWhich::User
            } else if slots.agent.as_ref().is_some_and(|p| now <= p.expires) {
                PendingWhich::Agent
            } else if slots.user.is_some() {
                PendingWhich::User
            } else {
                PendingWhich::Agent
            }
        }
    }
}

fn slot_mut(slots: &mut PendingSlots, which: PendingWhich) -> &mut Option<PendingUndo> {
    match which {
        PendingWhich::User => &mut slots.user,
        PendingWhich::Agent => &mut slots.agent,
    }
}

fn slot_ref(slots: &PendingSlots, which: PendingWhich) -> &Option<PendingUndo> {
    match which {
        PendingWhich::User => &slots.user,
        PendingWhich::Agent => &slots.agent,
    }
}

/// Arm into the origin's own slot. An agent arm never writes the user slot.
fn arm_into(slots: &mut PendingSlots, pending: PendingUndo) {
    match pending.origin {
        ClaimOrigin::User => slots.user = Some(pending),
        ClaimOrigin::Agent(_) => slots.agent = Some(pending),
    }
}

fn arm_undo(
    before: &PersistedState,
    after: &PersistedState,
    window_id: &str,
    origin: ClaimOrigin,
) {
    let pending = capture_undo(
        before,
        after,
        window_id,
        origin,
        SystemTime::now() + Duration::from_secs(3),
    );
    if let Ok(mut slots) = PENDING.lock() {
        arm_into(&mut slots, pending);
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

pub fn undo_last(app: &AppHandle, caller: &ClaimOrigin) -> Result<(), String> {
    let pending = {
        let mut slots = PENDING
            .lock()
            .map_err(|_| "nothing to undo".to_string())?;
        let now = SystemTime::now();
        let which = which_pending(&slots, caller, now);
        try_take(slot_mut(&mut slots, which), caller, now)?
    };
    let current = crate::state::load_state(app)?;
    let mut after = current.clone();
    undo_claim_in(&mut after, &pending)?;
    save_with_events(app, &current, &after)?;
    Ok(())
}

pub fn change_planet(
    app: &AppHandle,
    project_id: &str,
    caller: &ClaimOrigin,
) -> Result<(), String> {
    let (window_id, origin) = {
        let slots = PENDING
            .lock()
            .map_err(|_| "nothing to change".to_string())?;
        let now = SystemTime::now();
        let which = which_pending(&slots, caller, now);
        let p = slot_ref(&slots, which)
            .as_ref()
            .ok_or_else(|| "nothing to change".to_string())?;
        check_pending(p, caller, SystemTime::now(), "change")?;
        (p.window_id.clone(), p.origin.clone())
    };
    let list = crate::windows::list_windows()?;
    let w = list
        .iter()
        .find(|x| x.id == window_id)
        .cloned()
        .ok_or_else(|| "window gone".to_string())?;
    // Re-arm keeps the origin that made the original claim, so undo
    // after change still returns to the pre-change claim.
    claim_window(app, &w, Some(project_id), &list, origin)
}

fn claim_window(
    app: &AppHandle,
    w: &WindowInfo,
    project_id: Option<&str>,
    list: &[WindowInfo],
    origin: ClaimOrigin,
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
    arm_undo(&before, &after, &w.id, origin);
    save_with_events(app, &before, &after)?;
    emit_toast(app, &project, &w);
    crate::habitat::bring_planet_front(&project.id);
    Ok(())
}

pub fn claim_front(app: &AppHandle, origin: &ClaimOrigin) -> Result<(), String> {
    let list = crate::windows::list_windows()?;
    let Some(w) = crate::windows::frontmost_claimable()? else {
        return Err("no window to claim — click the window you want (any display), then Claim again".into());
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
        return claim_window(app, &w, Some(&project.id), &list, origin.clone());
    }
    claim_window(app, &w, None, &list, origin.clone())
}

pub(crate) fn claim_front_to_planet_origin(
    app: &AppHandle,
    project_id: &str,
    origin: &ClaimOrigin,
) -> Result<(), String> {
    let list = crate::windows::list_windows()?;
    let Some(w) = crate::windows::frontmost_claimable()? else {
        return Err("no window to claim — click the window you want (any display), then Claim again".into());
    };
    claim_window(app, &w, Some(project_id), &list, origin.clone())
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
    claim_front(&app, &ClaimOrigin::User)
}

#[tauri::command]
pub fn claim_front_undo(app: AppHandle) -> Result<(), String> {
    undo_last(&app, &ClaimOrigin::User)
}

#[tauri::command]
pub fn claim_front_change(app: AppHandle, project_id: String) -> Result<(), String> {
    change_planet(&app, &project_id, &ClaimOrigin::User)
}

#[tauri::command]
pub fn claim_front_to(app: AppHandle, project_id: String) -> Result<(), String> {
    claim_front_to_planet_origin(&app, &project_id, &ClaimOrigin::User)
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
                if let Err(e) = crate::claim_front::claim_front(app, &crate::claim_front::ClaimOrigin::User)
                {
                    eprintln!("dock-claim: {e}");
                }
            }
        }

        unsafe extern "C" fn dock_undo(_this: *mut Object, _cmd: Sel, _sender: *mut Object) {
            if let Some(app) = DOCK_APP.get() {
                if let Err(e) =
                    crate::claim_front::undo_last(app, &crate::claim_front::ClaimOrigin::User)
                {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Claim, KanbanCard, PersistedState, Project};
    use std::time::UNIX_EPOCH;

    fn future() -> SystemTime {
        SystemTime::now() + Duration::from_secs(30)
    }

    fn agent(name: &str) -> ClaimOrigin {
        ClaimOrigin::Agent(name.to_string())
    }

    fn win(id: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            pid: 1,
            app_name: "App".into(),
            title: title.into(),
            bundle_id: Some("app.bundle".into()),
            stable_key: format!("app.bundle|{title}|1"),
            icon_base64: None,
            is_frontmost: true,
            is_minimized: false,
            document_path: None,
        }
    }

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            trackers: None,
            workspace_root: None,
        }
    }

    fn claim_of(window_id: &str, project_id: &str, seed: u64, pack: &str) -> Claim {
        Claim {
            window_id: window_id.into(),
            project_id: project_id.into(),
            last_touched_at: 1.0,
            bundle_id: Some("app.bundle".into()),
            app_name: Some("App".into()),
            title_at_claim: Some("notes".into()),
            title_index: Some(1),
            sprite_pack_id: Some(pack.into()),
            char_seed: Some(seed),
            char_look: None,
            sprite_scale: None,
            walker_home: None,
            file_path: None,
            hermes_profile: None,
            hermes_session: None,
            action_id: None,
            command: None,
            cwd: None,
        }
    }

    fn card_of(
        id: &str,
        project_id: &str,
        title: &str,
        window_id: Option<&str>,
        status: &str,
        touched: f64,
    ) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: project_id.into(),
            window_id: window_id.map(str::to_string),
            kind: Some("todo".into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: touched,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: None,
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links: None,
        }
    }

    fn pending(origin: ClaimOrigin, expires: SystemTime) -> PendingUndo {
        PendingUndo {
            window_id: "W".into(),
            origin,
            expires,
            prev_claim: None,
            written_project_id: "P".into(),
            prev_last_claim_project_id: None,
            card: None,
        }
    }

    /// T1. Unrelated writes made after the claim survive a targeted undo.
    #[test]
    fn undo_keeps_unrelated_writes() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("P", "Planet"), project("Q", "Other")];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "P", &list);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());
        after.kanban_cards
            .push(card_of("c-new", "Q", "fresh card", None, "todo", 9.0));
        after.claims.push(claim_of("X", "Q", 7, "other-pack"));
        after
            .projects
            .iter_mut()
            .find(|p| p.id == "Q")
            .unwrap()
            .name = "Renamed".into();
        after.companion.last_user_line = "keep me".into();
        undo_claim_in(&mut after, &pending).unwrap();
        assert!(after.claims.iter().all(|c| c.window_id != "W"));
        assert!(after
            .claims
            .iter()
            .any(|c| c.window_id == "X" && c.project_id == "Q" && c.char_seed == Some(7)));
        assert!(after
            .kanban_cards
            .iter()
            .any(|c| c.id == "c-new" && c.title == "fresh card"));
        assert_eq!(
            after.projects.iter().find(|p| p.id == "Q").unwrap().name,
            "Renamed"
        );
        assert_eq!(after.companion.last_user_line, "keep me");
        assert_eq!(after.last_claim_project_id, None);
    }

    /// T2. Undo puts the previous claim row back, seed and pack included.
    #[test]
    fn undo_restores_previous_claim() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "Alpha"), project("B", "Beta")];
        before.claims = vec![claim_of("W", "A", 42, "pack-a")];
        before.last_claim_project_id = Some("A".into());
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let live = after.claims.iter().find(|c| c.window_id == "W").unwrap();
        assert_eq!(live.project_id, "B");
        assert_eq!(live.char_seed, Some(42));
        assert_eq!(live.sprite_pack_id.as_deref(), Some("pack-a"));
        let pending = capture_undo(&before, &after, "W", agent("mcp"), future());
        undo_claim_in(&mut after, &pending).unwrap();
        let restored = after.claims.iter().find(|c| c.window_id == "W").unwrap();
        assert_eq!(restored.project_id, "A");
        assert_eq!(restored.char_seed, Some(42));
        assert_eq!(restored.sprite_pack_id.as_deref(), Some("pack-a"));
        assert_eq!(after.last_claim_project_id.as_deref(), Some("A"));
    }

    /// T3. The card apply_claim retargeted comes back. A card moved since stays.
    #[test]
    fn undo_restores_card_retarget() {
        let w = win("W", "doc");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![
            project("A", "Alpha"),
            project("B", "Beta"),
            project("C", "Gamma"),
        ];
        before.kanban_cards = vec![card_of("c-open", "A", "", Some("W"), "doing", 1.5)];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let moved = after.kanban_cards.iter().find(|c| c.id == "c-open").unwrap();
        assert_eq!(moved.project_id, "B");
        assert_eq!(moved.title, "App — doc");
        assert_ne!(moved.last_touched_at, 1.5);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());
        undo_claim_in(&mut after, &pending).unwrap();
        let card = after.kanban_cards.iter().find(|c| c.id == "c-open").unwrap();
        assert_eq!(card.project_id, "A");
        assert_eq!(card.window_id.as_deref(), Some("W"));
        assert_eq!(card.title, "");
        assert_eq!(card.last_touched_at, 1.5);

        let mut moved_since = before.clone();
        apply_claim(&mut moved_since, &w, "B", &list);
        let pending = capture_undo(&before, &moved_since, "W", ClaimOrigin::User, future());
        {
            let card = moved_since
                .kanban_cards
                .iter_mut()
                .find(|c| c.id == "c-open")
                .unwrap();
            card.project_id = "C".into();
            card.title = "moved by hand".into();
            card.window_id = Some("elsewhere".into());
            card.last_touched_at = 99.0;
        }
        undo_claim_in(&mut moved_since, &pending).unwrap();
        let card = moved_since
            .kanban_cards
            .iter()
            .find(|c| c.id == "c-open")
            .unwrap();
        assert_eq!(card.project_id, "C");
        assert_eq!(card.title, "moved by hand");
        assert_eq!(card.window_id.as_deref(), Some("elsewhere"));
        assert_eq!(card.last_touched_at, 99.0);
    }

    /// T4. A claim re-pointed or dropped since the arm is not undone.
    #[test]
    fn undo_refuses_when_claim_changed_since() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("B", "Beta"), project("C", "Gamma")];
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let pending = capture_undo(&before, &after, "W", ClaimOrigin::User, future());

        let mut reclaimed = after.clone();
        reclaimed
            .claims
            .iter_mut()
            .find(|c| c.window_id == "W")
            .unwrap()
            .project_id = "C".into();
        let snap = serde_json::to_value(&reclaimed).unwrap();
        let err = undo_claim_in(&mut reclaimed, &pending).unwrap_err();
        assert!(err.contains("claim changed since"), "{err}");
        assert_eq!(serde_json::to_value(&reclaimed).unwrap(), snap);

        let mut gone = after.clone();
        gone.claims.retain(|c| c.window_id != "W");
        let snap = serde_json::to_value(&gone).unwrap();
        let err = undo_claim_in(&mut gone, &pending).unwrap_err();
        assert!(err.contains("claim changed since"), "{err}");
        assert_eq!(serde_json::to_value(&gone).unwrap(), snap);
    }

    /// T5. An agent undoes or changes only its own pending claim.
    #[test]
    fn agent_cannot_undo_or_change_others() {
        let exp = future();
        let now = SystemTime::now();
        for verb in ["undo", "change"] {
            let other = pending(agent("b"), exp);
            let err = check_pending(&other, &agent("a"), now, verb).unwrap_err();
            assert_eq!(err, "not your claim");

            let user_made = pending(ClaimOrigin::User, exp);
            let err = check_pending(&user_made, &agent("a"), now, verb).unwrap_err();
            assert_eq!(
                err,
                format!("claim was made from the tray/app; {verb} it there")
            );

            assert!(check_pending(&other, &ClaimOrigin::User, now, verb).is_ok());
            let mine = pending(agent("a"), exp);
            assert!(check_pending(&mine, &agent("a"), now, verb).is_ok());
        }
    }

    /// T6. change uses the same expiry window as undo.
    #[test]
    fn change_and_undo_refuse_expired() {
        let p = pending(agent("a"), UNIX_EPOCH);
        let now = SystemTime::now();
        assert_eq!(
            check_pending(&p, &agent("a"), now, "undo").unwrap_err(),
            "undo expired"
        );
        assert_eq!(
            check_pending(&p, &agent("a"), now, "change").unwrap_err(),
            "change expired"
        );
    }

    /// T7. A refused take leaves the pending slot. An accepted take clears it.
    #[test]
    fn refused_undo_keeps_pending() {
        let exp = future();
        let now = SystemTime::now();
        let mut slot = Some(pending(agent("b"), exp));
        let err = try_take(&mut slot, &agent("a"), now).unwrap_err();
        assert_eq!(err, "not your claim");
        assert!(slot.is_some());

        let mut user_slot = Some(pending(ClaimOrigin::User, exp));
        let err = try_take(&mut user_slot, &agent("a"), now).unwrap_err();
        assert!(err.contains("tray/app"), "{err}");
        assert!(user_slot.is_some());

        let mut expired = Some(pending(agent("a"), UNIX_EPOCH));
        let err = try_take(&mut expired, &agent("a"), now).unwrap_err();
        assert_eq!(err, "undo expired");
        assert!(expired.is_some());

        let taken = try_take(&mut slot, &agent("b"), now).unwrap();
        assert_eq!(taken.origin, agent("b"));
        assert!(slot.is_none());
    }

    #[test]
    fn v1_user_title_edit_on_retargeted_card_survives_undo() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "A"), project("B", "B")];
        before.claims.push(claim_of("W", "A", 5, "pack"));
        before.kanban_cards.push(card_of("c1", "A", "orig title", Some("W"), "doing", 1.0));
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let p = capture_undo(&before, &after, "W", agent("mcp:x:1"), future());
        let c = after.kanban_cards.iter_mut().find(|c| c.id == "c1").unwrap();
        c.title = "renamed by Matt".into();
        c.notes = Some("note added".into());
        undo_claim_in(&mut after, &p).unwrap();
        let c = after.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.project_id, "A", "retarget undone");
        assert_eq!(c.notes.as_deref(), Some("note added"), "note kept");
        assert_eq!(c.title, "renamed by Matt", "unrelated title edit in the window was reverted");
    }

    /// Mirror of V1: a user touch of `last_touched_at` survives; an untouched title still restores.
    #[test]
    fn v1_user_last_touched_edit_on_retargeted_card_survives_undo() {
        let w = win("W", "notes");
        let list = vec![w.clone()];
        let mut before = PersistedState::default();
        before.projects = vec![project("A", "A"), project("B", "B")];
        before
            .kanban_cards
            .push(card_of("c1", "A", "", Some("W"), "doing", 1.0));
        let mut after = before.clone();
        apply_claim(&mut after, &w, "B", &list);
        let p = capture_undo(&before, &after, "W", agent("mcp:x:1"), future());
        let c = after.kanban_cards.iter_mut().find(|c| c.id == "c1").unwrap();
        c.last_touched_at = 42.0;
        undo_claim_in(&mut after, &p).unwrap();
        let c = after.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.project_id, "A", "retarget undone");
        assert_eq!(c.window_id.as_deref(), Some("W"));
        assert_eq!(c.last_touched_at, 42.0, "user last_touched_at edit survives");
        assert_eq!(c.title, "", "untouched title still restores");
    }

    /// N1. An agent claim must not burn a pending tray/app undo.
    #[test]
    fn agent_claim_never_overwrites_pending_user_undo() {
        let (w1, w2) = (win("W1", "mine"), win("W2", "agent"));
        let list = vec![w1.clone(), w2.clone()];
        let mut s0 = PersistedState::default();
        s0.projects = vec![project("A", "A")];
        let mut s1 = s0.clone();
        apply_claim(&mut s1, &w1, "A", &list);
        let mut slots = PendingSlots {
            user: None,
            agent: None,
        };
        arm_into(
            &mut slots,
            capture_undo(&s0, &s1, "W1", ClaimOrigin::User, future()),
        );
        let mut s2 = s1.clone();
        apply_claim(&mut s2, &w2, "A", &list);
        arm_into(
            &mut slots,
            capture_undo(&s1, &s2, "W2", agent("mcp:x:1"), future()),
        );
        let now = SystemTime::now();
        let which = which_pending(&slots, &ClaimOrigin::User, now);
        let p = try_take(slot_mut(&mut slots, which), &ClaimOrigin::User, now).unwrap();
        assert_eq!(p.window_id, "W1");
        undo_claim_in(&mut s2, &p).unwrap();
        assert!(
            !s2.claims.iter().any(|c| c.window_id == "W1"),
            "Matt's own claim undone"
        );
        assert!(
            s2.claims.iter().any(|c| c.window_id == "W2"),
            "agent claim untouched"
        );
        let now = SystemTime::now();
        let which = which_pending(&slots, &agent("mcp:x:1"), now);
        let p2 = try_take(slot_mut(&mut slots, which), &agent("mcp:x:1"), now).unwrap();
        assert_eq!(p2.window_id, "W2");
        undo_claim_in(&mut s2, &p2).unwrap();
        assert!(!s2.claims.iter().any(|c| c.window_id == "W2"));
    }

    /// C1. Expiry is inclusive at `now == expires` (`now > expires`).
    #[test]
    fn check_pending_exact_expiry_is_inclusive() {
        let e = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let p = pending(agent("mcp:x:1"), e);
        for verb in ["change", "undo"] {
            assert!(
                check_pending(&p, &agent("mcp:x:1"), e, verb).is_ok(),
                "{verb} at expiry"
            );
            let err = check_pending(
                &p,
                &agent("mcp:x:1"),
                e + Duration::from_millis(1),
                verb,
            )
            .unwrap_err();
            assert_eq!(err, format!("{verb} expired"));
        }
    }

    /// C4. Caller compare is case-sensitive.
    #[test]
    fn agent_compare_is_case_sensitive() {
        let now = UNIX_EPOCH;
        let p = pending(agent("mcp:Cursor:1"), future());
        for other in ["MCP:CURSOR:1", "mcp:cursor:1"] {
            for verb in ["undo", "change"] {
                assert_eq!(
                    check_pending(&p, &agent(other), now, verb).unwrap_err(),
                    "not your claim",
                    "{other:?} {verb}"
                );
            }
        }
    }

    /// C5. Caller compare does not trim.
    #[test]
    fn agent_compare_does_not_trim() {
        let now = UNIX_EPOCH;
        let p = pending(agent("mcp:Cursor:1"), future());
        for verb in ["undo", "change"] {
            assert_eq!(
                check_pending(&p, &agent(" mcp:Cursor:1"), now, verb).unwrap_err(),
                "not your claim",
                "{verb}"
            );
        }
    }

    fn strip_rust_comments(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' {
                out.push('"');
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    out.push(c);
                    i += 1;
                    if c == '\\' && i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
        let marker = format!("fn {name}(");
        let start = src.find(&marker).unwrap_or_else(|| panic!("missing {name}"));
        let bytes = src.as_bytes();
        let mut i = start;
        while i < bytes.len() && bytes[i] != b'{' {
            i += 1;
        }
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escape = false;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
                i += 1;
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &src[start..=i];
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("unclosed {name}");
    }

    /// C11. change_planet checks the pending slot before it re-claims.
    #[test]
    fn change_planet_checks_pending_before_claim_window() {
        let stripped = strip_rust_comments(include_str!("claim_front.rs"));
        let body = fn_body(&stripped, "change_planet");
        let check = body
            .find("check_pending(p, caller, SystemTime::now(), \"change\")?")
            .expect("check_pending");
        let claim = body.find("claim_window(").expect("claim_window");
        assert!(
            check < claim,
            "C11 change_planet must check_pending before claim_window"
        );
    }

    /// C13. undo_last takes through try_take, never a bare slot.take().
    #[test]
    fn undo_last_uses_try_take() {
        let stripped = strip_rust_comments(include_str!("claim_front.rs"));
        let body = fn_body(&stripped, "undo_last");
        assert!(body.contains("try_take("), "C13 undo_last must call try_take");
        assert!(
            !body.contains(".take()"),
            "C13 undo_last must not bare-take the slot:\n{body}"
        );
    }
}
