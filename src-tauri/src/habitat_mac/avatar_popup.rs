//! Flat avatar picker popped from the walker menu's "Change avatar…".
//!
//! Generated + walker packs, then import and the being editor. Pack picks
//! still ride `entryId␟packId` into `walkerSetPack:` → habitat:walker-pack.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::input::{add_menu_item, add_separator};
use super::{lock, menu_target, ns_string, sender_project_id, NSPoint};

/// Walker art only. Companion bodies and habitat tiles stay out of the picker.
pub(crate) fn is_walker_pack(frame_count: usize, role: &str) -> bool {
    frame_count > 0
        && !role.eq_ignore_ascii_case("companion")
        && !role.eq_ignore_ascii_case("habitat")
}

/// Pack rows. Payload is `entryId␟packId` (empty pack id = Generated).
/// `packs` is `(name, id)` in display order. Generated is checked when
/// `cur_pack` is empty.
pub(crate) fn avatar_items(
    entry_id: &str,
    cur_pack: &str,
    packs: &[(&str, &str)],
) -> Vec<(String, String, bool)> {
    let mut rows = Vec::with_capacity(1 + packs.len());
    rows.push((
        "Generated".to_string(),
        format!("{entry_id}\u{1f}"),
        cur_pack.is_empty(),
    ));
    for (name, id) in packs {
        rows.push((
            (*name).to_string(),
            format!("{entry_id}\u{1f}{id}"),
            cur_pack == *id,
        ));
    }
    rows
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AvatarVerb {
    Pack,
    AddImage,
    AddFolder,
    EditBeing,
}

impl AvatarVerb {
    fn selector_name(self) -> &'static str {
        match self {
            AvatarVerb::Pack => "walkerSetPack:",
            AvatarVerb::AddImage => "walkerUseImage:",
            AvatarVerb::AddFolder => "walkerUseFolder:",
            AvatarVerb::EditBeing => "walkerOpenBeing:",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AvatarLine {
    Item {
        label: String,
        payload: String,
        checked: bool,
        verb: AvatarVerb,
    },
    Separator,
}

/// The whole popup: pack rows, a gap, then Add new icon…, Add icon folder…,
/// Edit being…. Action payloads are the entry id (the import / being doors).
pub(crate) fn avatar_popup_lines(
    entry_id: &str,
    cur_pack: &str,
    packs: &[(&str, &str)],
) -> Vec<AvatarLine> {
    let mut lines = Vec::new();
    for (label, payload, checked) in avatar_items(entry_id, cur_pack, packs) {
        lines.push(AvatarLine::Item {
            label,
            payload,
            checked,
            verb: AvatarVerb::Pack,
        });
    }
    lines.push(AvatarLine::Separator);
    for (label, verb) in [
        ("Add new icon…", AvatarVerb::AddImage),
        ("Add icon folder…", AvatarVerb::AddFolder),
        ("Edit being…", AvatarVerb::EditBeing),
    ] {
        lines.push(AvatarLine::Item {
            label: label.to_string(),
            payload: entry_id.to_string(),
            checked: false,
            verb,
        });
    }
    lines
}

/// "Change avatar…" — hop off the context-menu tracking loop, then pop
/// one flat menu anchored on the walker.
pub(crate) extern "C" fn walker_change_avatar(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(entry_id) = (unsafe { sender_project_id(sender) }) else {
            return;
        };
        super::audio::defer_main(move || unsafe {
            pop_avatar_menu(&entry_id);
        });
    }));
}

struct MenuQuiet;
impl MenuQuiet {
    fn arm() -> Self {
        crate::companion_clear::set_menu_quiet(true);
        Self
    }
}
impl Drop for MenuQuiet {
    fn drop(&mut self) {
        crate::companion_clear::set_menu_quiet(false);
    }
}

unsafe fn pop_avatar_menu(entry_id: &str) {
    let (cur_pack, anchor) = {
        let h = lock();
        let cur = h
            .planets
            .values()
            .flat_map(|p| p.walkers.iter())
            .find(|b| b.entry_id == entry_id)
            .map(|b| b.sprite_pack_id.clone())
            .unwrap_or_default();
        let anchor = h.planets.values().find_map(|p| {
            p.walkers
                .iter()
                .find(|b| b.entry_id == entry_id)
                .map(|b| {
                    (
                        p.view,
                        NSPoint {
                            x: b.frame.mid_x(),
                            y: b.frame.mid_y(),
                        },
                    )
                })
        });
        (cur, anchor)
    };
    let packs: Vec<(String, String)> = super::app_handle()
        .and_then(|app| crate::packs::list_packs_sync(app.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| is_walker_pack(p.frame_count, &p.role))
        .map(|p| (p.name, p.id))
        .collect();
    let refs: Vec<(&str, &str)> = packs.iter().map(|(n, id)| (n.as_str(), id.as_str())).collect();
    let lines = avatar_popup_lines(entry_id, &cur_pack, &refs);
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    for line in &lines {
        match line {
            AvatarLine::Separator => add_separator(menu),
            AvatarLine::Item {
                label,
                payload,
                checked,
                verb,
            } => {
                let action = Sel::register(verb.selector_name());
                let state = match verb {
                    AvatarVerb::Pack => Some(*checked),
                    _ => None,
                };
                add_menu_item(menu, label, action, target, payload, state, true);
            }
        }
    }
    let _quiet = MenuQuiet::arm();
    let (view, point) = match anchor {
        Some((view, point)) => (view as *mut Object, point),
        None => {
            let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            (std::ptr::null_mut(), point)
        }
    };
    let _: bool = msg_send![menu,
        popUpMenuPositioningItem: std::ptr::null_mut::<Object>()
        atLocation: point
        inView: view];
    let _: () = msg_send![menu, release];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_is_checked_when_pack_is_empty() {
        let rows = avatar_items("win-1", "", &[("Cats", "cats"), ("Dogs", "dogs")]);
        assert_eq!(rows[0].0, "Generated");
        assert_eq!(rows[0].1, "win-1\u{1f}");
        assert!(rows[0].2);
        assert_eq!(rows[1], ("Cats".into(), "win-1\u{1f}cats".into(), false));
        assert_eq!(rows[2], ("Dogs".into(), "win-1\u{1f}dogs".into(), false));
    }

    #[test]
    fn current_pack_is_the_checked_row() {
        let rows = avatar_items("e", "dogs", &[("Cats", "cats"), ("Dogs", "dogs")]);
        assert!(!rows[0].2, "generated");
        assert!(!rows[1].2, "cats");
        assert!(rows[2].2, "dogs");
        assert_eq!(rows[2].1, "e\u{1f}dogs");
    }

    #[test]
    fn popup_lists_packs_then_import_and_being() {
        let lines = avatar_popup_lines("e", "", &[("Cats", "cats")]);
        assert_eq!(lines.len(), 6);
        assert!(matches!(
            &lines[0],
            AvatarLine::Item { label, verb: AvatarVerb::Pack, checked: true, .. } if label == "Generated"
        ));
        assert!(matches!(&lines[2], AvatarLine::Separator));
        assert!(matches!(
            &lines[3],
            AvatarLine::Item { label, payload, verb: AvatarVerb::AddImage, .. }
                if label == "Add new icon…" && payload == "e"
        ));
        assert!(matches!(
            &lines[4],
            AvatarLine::Item { label, verb: AvatarVerb::AddFolder, .. } if label == "Add icon folder…"
        ));
        assert!(matches!(
            &lines[5],
            AvatarLine::Item { label, verb: AvatarVerb::EditBeing, .. } if label == "Edit being…"
        ));
    }

    #[test]
    fn avatar_verbs_name_their_selectors() {
        assert_eq!(AvatarVerb::AddImage.selector_name(), "walkerUseImage:");
        assert_eq!(AvatarVerb::AddFolder.selector_name(), "walkerUseFolder:");
        assert_eq!(AvatarVerb::Pack.selector_name(), "walkerSetPack:");
        assert_eq!(AvatarVerb::EditBeing.selector_name(), "walkerOpenBeing:");
    }

    #[test]
    fn companion_and_habitat_roles_are_not_walker_packs() {
        assert!(is_walker_pack(4, ""));
        assert!(is_walker_pack(1, "walker"));
        assert!(!is_walker_pack(0, "walker"));
        assert!(!is_walker_pack(3, "companion"));
        assert!(!is_walker_pack(3, "Habitat"));
    }
}
