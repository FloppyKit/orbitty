//! NSView + planet-menu target class registration (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs. Selector bodies stay in mouse /
//! draw / planet_menu / walker_menu / audio.

#![allow(unexpected_cfgs)]

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use super::audio;
use super::draw;
use super::input;
use super::mouse;
use super::avatar_popup;
use super::planet_menu;
use super::walker_menu;
use super::{panic_msg, NSRect};

/// None means the class couldn't be registered; callers skip planet
/// creation so a habitat failure leaves the rail usable.
pub(crate) fn world_view_class() -> Option<&'static Class> {
    static CLS: OnceLock<Option<&'static Class>> = OnceLock::new();
    CLS.get_or_init(|| {
        // add_method asserts on any mismatch — registration must never abort
        // the app, so it runs under catch_unwind.
        catch_unwind(AssertUnwindSafe(|| unsafe { register_world_view_class() }))
            .unwrap_or_else(|p| {
                eprintln!(
                    "habitat: world view class registration panicked: {}",
                    panic_msg(&*p)
                );
                None
            })
    })
    .as_ref()
    .copied()
}

unsafe fn register_world_view_class() -> Option<&'static Class> {
    // Already registered (unit tests, re-init) — reuse, never panic.
    if let Some(cls) = Class::get("OrbittyWorldView") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyWorldView", class!(NSView))?;
    // drawRect:'s dirty-rect arg is part of the signature (objc's add_method
    // asserts selector/fn arg counts match); the body ignores it and always
    // redraws the whole view.
    decl.add_method(
        sel!(drawRect:),
        draw::draw_rect as extern "C" fn(&Object, Sel, NSRect),
    );
    decl.add_method(
        sel!(mouseDown:),
        mouse::mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(mouseUp:),
        mouse::mouse_up as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(isFlipped),
        is_flipped as extern "C" fn(&Object, Sel) -> bool,
    );
    decl.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, *mut Object) -> bool,
    );
    decl.add_method(
        sel!(scrollWheel:),
        mouse::scroll_wheel as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(rightMouseDown:),
        input::right_mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

/// Target for the planet right-click menu. A single leaked instance lives
/// forever (NSMenuItem does not retain its target).
pub(crate) fn menu_target() -> Option<usize> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    TARGET
        .get_or_init(|| {
            catch_unwind(AssertUnwindSafe(|| unsafe {
                let cls = register_menu_target_class()?;
                let obj: *mut Object = msg_send![cls, new];
                if obj.is_null() {
                    None
                } else {
                    Some(obj as usize)
                }
            }))
            .unwrap_or_else(|p| {
                eprintln!(
                    "habitat: menu target registration panicked: {}",
                    panic_msg(&*p)
                );
                None
            })
        })
        .as_ref()
        .copied()
}

unsafe fn register_menu_target_class() -> Option<&'static Class> {
    if let Some(cls) = Class::get("OrbittyPlanetMenuTarget") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyPlanetMenuTarget", class!(NSObject))?;
    decl.add_method(
        sel!(planetMinimize:),
        planet_menu::menu_minimize as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRestore:),
        planet_menu::menu_restore as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleMarble:),
        planet_menu::menu_toggle_marble as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetTogglePlant:),
        planet_menu::menu_toggle_plant as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleMushroom:),
        planet_menu::menu_toggle_mushroom as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleBoombox:),
        planet_menu::menu_toggle_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetChooseMusic:),
        audio::menu_choose_music as extern "C" fn(&Object, Sel, *mut Object),
    );
    // boombox sprite menu + volume steps.
    decl.add_method(
        sel!(planetBoomboxPlayPause:),
        planet_menu::menu_boombox_play_pause as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRemoveBoombox:),
        planet_menu::menu_remove_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetBoomboxVolume:),
        planet_menu::menu_boombox_volume as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetBoomboxSlider:),
        planet_menu::menu_boombox_slider as extern "C" fn(&Object, Sel, *mut Object),
    );
    // per-planet beach-ball toggle.
    decl.add_method(
        sel!(planetToggleBeachBall:),
        planet_menu::menu_toggle_beach_ball as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetPlaceBuilding:),
        planet_menu::menu_place_building as extern "C" fn(&Object, Sel, *mut Object),
    );
    // walker right-click menu actions.
    decl.add_method(
        sel!(walkerUseImage:),
        walker_menu::walker_use_image as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerUseFolder:),
        walker_menu::walker_use_folder as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerSetPack:),
        walker_menu::walker_set_pack as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerOpenBeing:),
        walker_menu::walker_open_being as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerChangeAvatar:),
        avatar_popup::walker_change_avatar as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerAssign:),
        walker_menu::walker_assign as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> bool {
    true
}

extern "C" fn accepts_first_mouse(_this: &Object, _cmd: Sel, _event: *mut Object) -> bool {
    true
}
