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

/// The walker's sprite-assignment menu: import (image/folder), Generated,
/// one item per walker pack, and the Being editor door. All verbs emit to
/// the rail — the native side never writes claims.
pub(crate) unsafe fn walker_context_menu(this: &Object, event: *mut Object, entry_id: &str) {
    // Current pick for the check states (raw claim value; "" = Generated).
    let cur_pack = {
        let h = lock();
        h.planets
            .values()
            .flat_map(|p| p.walkers.iter())
            .find(|b| b.entry_id == entry_id)
            .map(|b| b.sprite_pack_id.clone())
            .unwrap_or_default()
    };
    // Walker packs only — companion bodies and habitat tiles stay out.
    let packs = app_handle()
        .and_then(|app| crate::packs::list_packs_sync(app.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            p.frame_count > 0
                && !p.role.eq_ignore_ascii_case("companion")
                && !p.role.eq_ignore_ascii_case("habitat")
        })
        .collect::<Vec<_>>();
    // Lock dropped: popUpContextMenu runs modally and the target's actions
    // re-lock on selection.
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    add_menu_item(
        menu,
        "Use my image…",
        sel!(walkerUseImage:),
        target,
        entry_id,
        None,
        true,
    );
    add_menu_item(
        menu,
        "Folder…",
        sel!(walkerUseFolder:),
        target,
        entry_id,
        None,
        true,
    );
    add_separator(menu);
    // Pack list is a submenu — same shape as Assign to planet. The pick
    // rides the represented object as `entryId␟packId` (unit separator).
    let look: *mut Object = msg_send![class!(NSMenu), alloc];
    let look: *mut Object = msg_send![look, initWithTitle: ns_string("")];
    if !look.is_null() {
        add_menu_item(
            look,
            "Generated",
            sel!(walkerSetPack:),
            target,
            &format!("{entry_id}\u{1f}"),
            Some(cur_pack.is_empty()),
            true,
        );
        for p in &packs {
            add_menu_item(
                look,
                &p.name,
                sel!(walkerSetPack:),
                target,
                &format!("{entry_id}\u{1f}{}", p.id),
                Some(cur_pack == p.id),
                true,
            );
        }
        let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let item: *mut Object = msg_send![item,
            initWithTitle: ns_string("Look")
            action: Sel::from_ptr(std::ptr::null())
            keyEquivalent: ns_string("")];
        if !item.is_null() {
            let _: () = msg_send![item, setSubmenu: look];
            let _: () = msg_send![menu, addItem: item];
            let _: () = msg_send![item, release];
        }
        let _: () = msg_send![look, release];
    }
    add_separator(menu);
    // Assign to planet IS the rail claim — the pick
    // emits habitat:walker-assign and the rail rewrites projectId through
    // the same persist door (one windowId, one planet; the open kanban
    // card and the sprite follow the claim). No occupancy teleport.
    let names = {
        let h = lock();
        h.planet_names.clone()
    };
    if !names.is_empty() {
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
                initWithTitle: ns_string("Assign to planet")
                action: Sel::from_ptr(std::ptr::null())
                keyEquivalent: ns_string("")];
            if !item.is_null() {
                let _: () = msg_send![item, setSubmenu: sub];
                let _: () = msg_send![menu, addItem: item];
                let _: () = msg_send![item, release];
            }
            let _: () = msg_send![sub, release];
        }
        add_separator(menu);
    }
    add_menu_item(
        menu,
        "Open being…",
        sel!(walkerOpenBeing:),
        target,
        entry_id,
        None,
        true,
    );
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

