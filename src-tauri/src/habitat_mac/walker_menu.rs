//! Walker right-click sprite / assign menu (DIET-HABITAT).
//!
//! Extracted from habitat_mac/input.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tauri::Emitter;

use super::input::{add_menu_item, add_separator, pop_context_menu};
use super::{app_handle, lock, menu_target, ns_string, sender_project_id};

// walker right-click menu (GO-ORBITTY-RAIL-ROW-MENU)

/// What the rail does with a walker-menu pick. `kind` = import first
/// ("image" / "folder"); `pack_id` = direct pick (None = Generated).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WalkerPackEvent {
    entry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    pack_id: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WalkerAssignEvent {
    entry_id: String,
    project_id: String,
}

fn emit_walker_pack(entry_id: String, kind: Option<&str>, pack_id: Option<String>) {
    if let Some(app) = app_handle() {
        let _ = app.emit_to(
            "main",
            "habitat:walker-pack",
            WalkerPackEvent {
                entry_id,
                kind: kind.map(|k| k.to_string()),
                pack_id,
            },
        );
    }
}

/// Walker right-click words. Assign is omitted when the rail has no planets.
pub(crate) fn walker_menu_labels(has_planets: bool) -> Vec<&'static str> {
    let mut labels = vec!["Change avatar…"];
    if has_planets {
        labels.push("Assign to planet");
    }
    labels
}

/// Change avatar… opens one flat popup. Assign to planet is the rail claim
/// (habitat:walker-assign). The native side never writes claims.
pub(crate) unsafe fn walker_context_menu(this: &Object, event: *mut Object, entry_id: &str) {
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    let names = {
        let h = lock();
        h.planet_names.clone()
    };
    let labels = walker_menu_labels(!names.is_empty());
    add_menu_item(
        menu,
        labels[0],
        sel!(walkerChangeAvatar:),
        target,
        entry_id,
        None,
        true,
    );
    // Assign to planet IS the rail claim — the pick emits
    // habitat:walker-assign and the rail rewrites projectId through the
    // same persist door. No occupancy teleport.
    if labels.len() > 1 {
        add_separator(menu);
        let sub: *mut Object = msg_send![class!(NSMenu), alloc];
        let sub: *mut Object = msg_send![sub, initWithTitle: ns_string("")];
        if !sub.is_null() {
            for (pid, pname) in &names {
                add_menu_item(
                    sub,
                    pname,
                    sel!(walkerAssign:),
                    target,
                    &format!("{entry_id}\u{1f}{pid}"),
                    None,
                    true,
                );
            }
            let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
            let item: *mut Object = msg_send![item,
                initWithTitle: ns_string(labels[1])
                action: Sel::from_ptr(std::ptr::null())
                keyEquivalent: ns_string("")];
            if !item.is_null() {
                let _: () = msg_send![item, setSubmenu: sub];
                let _: () = msg_send![menu, addItem: item];
                let _: () = msg_send![item, release];
            }
            let _: () = msg_send![sub, release];
        }
    }
    let view = this as *const Object as *mut Object;
    pop_context_menu(menu, event, view);
    let _: () = msg_send![menu, release];
}

pub(crate) extern "C" fn walker_use_image(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            emit_walker_pack(id, Some("image"), None);
        }
    }));
}

pub(crate) extern "C" fn walker_use_folder(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            emit_walker_pack(id, Some("folder"), None);
        }
    }));
}

pub(crate) extern "C" fn walker_set_pack(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(raw) = sender_project_id(sender) else {
            return;
        };
        let Some((entry_id, pack_id)) = raw.split_once('\u{1f}') else {
            return;
        };
        emit_walker_pack(
            entry_id.to_string(),
            None,
            if pack_id.is_empty() {
                None
            } else {
                Some(pack_id.to_string())
            },
        );
    }));
}

pub(crate) extern "C" fn walker_assign(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(raw) = sender_project_id(sender) else {
            return;
        };
        let Some((entry_id, project_id)) = raw.split_once('\u{1f}') else {
            return;
        };
        if project_id.is_empty() {
            return;
        }
        if let Some(app) = app_handle() {
            let _ = app.emit_to(
                "main",
                "habitat:walker-assign",
                WalkerAssignEvent {
                    entry_id: entry_id.to_string(),
                    project_id: project_id.to_string(),
                },
            );
        }
    }));
}

pub(crate) extern "C" fn walker_open_being(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            if let Some(app) = app_handle() {
                let _ = app.emit_to("main", "habitat:being", id);
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::walker_menu_labels;

    #[test]
    fn walker_menu_is_change_avatar_and_assign() {
        assert_eq!(walker_menu_labels(false), vec!["Change avatar…"]);
        let full = walker_menu_labels(true);
        assert_eq!(full, vec!["Change avatar…", "Assign to planet"]);
        let blob = full.join("\n").to_ascii_lowercase();
        assert!(!blob.contains("image"));
        assert!(!blob.contains("folder"));
        assert!(!blob.contains("look"));
        assert!(!blob.contains("being"));
        assert!(!blob.contains("pack"));
    }
}

