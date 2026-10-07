//! Native traffic lights for the Board, Stars and the expanded companion.
//! Pure decisions live here so the cargo tests do not need AppKit.
//! `main` never gets lights (the rail stays POLISH's borderless window).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::Manager;

pub const STYLE_TITLED: usize = 1;
pub const STYLE_CLOSABLE: usize = 2;
pub const STYLE_MINI: usize = 4;
pub const STYLE_RESIZABLE: usize = 8;
pub const STYLE_FULLSIZE: usize = 1 << 15;

pub const FS_PRIMARY: usize = 1 << 7;
pub const FS_NONE: usize = 1 << 9;

pub const ROW_H: f64 = 38.0;
pub const BUTTON_H: f64 = 14.0;
pub const ROW_CENTRE: f64 = 19.0;
/// Close-button left edge. Runtime `show_and_position_lights` then
/// centres every button on `ROW_CENTRE`.
pub const TL_X: f64 = 14.0;
/// `traffic_light_y(38, 14)` — button top so the centre is row/2.
pub const TL_Y: f64 = traffic_light_y(ROW_H, BUTTON_H);

#[derive(Clone, Copy, Debug)]
pub struct LightsCtx {
    pub mac: bool,
    pub companion_expanded: bool,
    /// Persisted companion.visible. Hidden (not minimized) stays dark so
    /// a page-mount `ensure_glass` cannot turn the buttons back on.
    pub companion_visible: bool,
}

pub fn wants_traffic_lights(label: &str, ctx: LightsCtx) -> bool {
    match label {
        "kanban" | "stars" => ctx.mac,
        "companion" => companion_wants_chrome(ctx.companion_expanded, ctx.companion_visible, ctx.mac),
        _ => false,
    }
}

pub fn companion_wants_chrome(expanded: bool, visible: bool, platform_mac: bool) -> bool {
    platform_mac && expanded && visible
}

pub fn companion_mask_on(current: usize) -> usize {
    (current & STYLE_RESIZABLE) | STYLE_TITLED | STYLE_CLOSABLE | STYLE_MINI | STYLE_FULLSIZE
}

pub fn companion_mask_off(current: usize) -> usize {
    let _ = current;
    STYLE_RESIZABLE
}

pub fn companion_frame_locked(minimized: bool, zoomed: bool) -> bool {
    minimized || zoomed
}

pub fn frame_write_skipped(minimized: bool, zoomed: bool, _kind: &str) -> bool {
    companion_frame_locked(minimized, zoomed)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlainRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A zoomed companion persists the pre-zoom stash, never the zoomed frame.
pub fn frame_for_persist(zoomed: bool, stash: Option<PlainRect>, live: PlainRect) -> PlainRect {
    if zoomed {
        stash.unwrap_or(live)
    } else {
        live
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomStep {
    Unzoom,
    Proceed,
}

pub fn collapse_or_host_steps(zoomed: bool) -> &'static [ZoomStep] {
    if zoomed {
        &[ZoomStep::Unzoom, ZoomStep::Proceed]
    } else {
        &[ZoomStep::Proceed]
    }
}

pub fn fullscreen_none_bits(behavior: usize) -> usize {
    (behavior & !FS_PRIMARY) | FS_NONE
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DblClick {
    Maximize,
    Minimize,
    None,
}

/// Missing and anything other than Minimize / None → Maximize (macOS default).
pub fn dblclick_pref_parse(raw: Option<&str>) -> DblClick {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        Some("Minimize") => DblClick::Minimize,
        Some("None") => DblClick::None,
        _ => DblClick::Maximize,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneVis {
    Shown,
    Minimized,
    Hidden,
}

pub fn pane_vis_state(visible: bool, minimized: bool) -> PaneVis {
    if minimized {
        PaneVis::Minimized
    } else if visible {
        PaneVis::Shown
    } else {
        PaneVis::Hidden
    }
}

pub fn pane_is_lost(state: PaneVis) -> bool {
    matches!(state, PaneVis::Hidden)
}

/// Button top. Centre = y + button_h/2 = row_h/2.
pub const fn traffic_light_y(row_h: f64, button_h: f64) -> f64 {
    (row_h - button_h) / 2.0
}

pub fn lights_ctx(app: &tauri::AppHandle) -> LightsCtx {
    let st = crate::state::load_state(app).unwrap_or_default();
    LightsCtx {
        mac: cfg!(target_os = "macos"),
        companion_expanded: st.companion.expanded,
        companion_visible: st.companion.visible,
    }
}

pub fn apply_lights(window: &tauri::WebviewWindow, label: &str, ctx: LightsCtx) {
    if label == "companion" {
        let minimized =
            window.is_minimized().unwrap_or(false) || crate::glass::is_miniaturized(window);
        if minimized {
            eprintln!("orbitty:companion-chrome deferred minimized=true");
            return;
        }
    }
    // A shown companion is on screen before `companion.visible` is persisted
    // (show_drawer, then the flag). Lights follow the live window then.
    let mut ctx = ctx;
    if label == "companion" && window.is_visible().unwrap_or(false) {
        ctx.companion_visible = true;
    }
    if wants_traffic_lights(label, ctx) {
        let _ = window.set_minimizable(true);
        let _ = window.set_maximizable(true);
        let _ = window.set_closable(true);
        if label == "companion" {
            let cur = crate::glass::style_mask(window);
            let next = companion_mask_on(cur);
            if cur != next {
                let (dx, dy, dw, dh) = crate::glass::apply_companion_mask(window, true);
                eprintln!(
                    "orbitty:companion-chrome on=true dx={dx:.2} dy={dy:.2} dw={dw:.2} dh={dh:.2} mask={next}"
                );
            }
            COMPANION_CHROME.store(true, Ordering::SeqCst);
        }
        crate::glass::show_and_position_lights(window, TL_X, ROW_CENTRE);
        if label == "kanban" || label == "stars" || label == "companion" {
            crate::glass::apply_fullscreen_none(window);
        }
    } else {
        if label == "companion" && COMPANION_CHROME.load(Ordering::SeqCst) {
            let (dx, dy, dw, dh) = crate::glass::apply_companion_mask(window, false);
            COMPANION_CHROME.store(false, Ordering::SeqCst);
            eprintln!(
                "orbitty:companion-chrome on=false dx={dx:.2} dy={dy:.2} dw={dw:.2} dh={dh:.2}"
            );
        }
        crate::glass::hide_traffic_lights(window);
    }
}

static COMPANION_CHROME: AtomicBool = AtomicBool::new(false);
static ZOOM_SUPPRESS: AtomicBool = AtomicBool::new(false);

struct ZoomMem {
    zoomed: bool,
    stash: Option<PlainRect>,
    last_normal: Option<PlainRect>,
}

fn zoom_mem() -> &'static Mutex<ZoomMem> {
    static ZOOM: std::sync::OnceLock<Mutex<ZoomMem>> = std::sync::OnceLock::new();
    ZOOM.get_or_init(|| {
        Mutex::new(ZoomMem {
            zoomed: false,
            stash: None,
            last_normal: None,
        })
    })
}

pub fn companion_zoomed() -> bool {
    zoom_mem().lock().map(|z| z.zoomed).unwrap_or(false)
}

fn near(a: PlainRect, b: PlainRect) -> bool {
    (a.x - b.x).abs() <= 1.0
        && (a.y - b.y).abs() <= 1.0
        && (a.w - b.w).abs() <= 1.0
        && (a.h - b.h).abs() <= 1.0
}

fn live_frame(window: &tauri::WebviewWindow) -> Option<PlainRect> {
    crate::drawers::live_points_rect(window).map(|r| PlainRect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    })
}

pub fn skip_companion_frame(minimized: bool, reason: &str) -> bool {
    let zoomed = companion_zoomed();
    let locked = companion_frame_locked(minimized, zoomed);
    if locked {
        eprintln!(
            "orbitty:companion-frame-skip {reason} minimized={minimized} zoomed={zoomed}"
        );
    }
    let _ = frame_write_skipped(minimized, zoomed, reason);
    locked
}

fn place_stash(window: &tauri::WebviewWindow, rect: PlainRect) {
    let r = crate::state::CompanionBounds {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    };
    crate::drawers::place_logical_rect(window, "companion", &r);
}

/// Un-zoom before collapse, host enter, or hide. The stash is the frame
/// that gets persisted — never the zoomed visibleFrame.
pub fn unzoom_companion(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("companion") else {
        return;
    };
    let run = move || {
        let zoomed = crate::glass::is_zoomed(&window) || companion_zoomed();
        let steps = collapse_or_host_steps(zoomed);
        if !steps.contains(&ZoomStep::Unzoom) {
            return;
        }
        let live = live_frame(&window).unwrap_or(PlainRect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        });
        let stash = zoom_mem().lock().ok().and_then(|z| z.stash);
        let chosen = frame_for_persist(true, stash, live);
        ZOOM_SUPPRESS.store(true, Ordering::SeqCst);
        if crate::glass::is_zoomed(&window) {
            crate::glass::perform_zoom(&window);
        }
        if let Some(now) = live_frame(&window) {
            if !near(now, chosen) {
                place_stash(&window, chosen);
            }
        } else {
            place_stash(&window, chosen);
        }
        if let Ok(mut z) = zoom_mem().lock() {
            z.zoomed = false;
            z.stash = None;
            z.last_normal = Some(chosen);
        }
        ZOOM_SUPPRESS.store(false, Ordering::SeqCst);
        eprintln!(
            "orbitty:companion-unzoom x={:.1} y={:.1} w={:.1} h={:.1}",
            chosen.x, chosen.y, chosen.w, chosen.h
        );
    };
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(run);
    #[cfg(not(target_os = "macos"))]
    run();
}

pub fn companion_chrome(app: &tauri::AppHandle, on: bool) {
    let Some(window) = app.get_webview_window("companion") else {
        return;
    };
    let minimized = window.is_minimized().unwrap_or(false)
        || crate::glass::is_miniaturized(&window);
    if on && minimized {
        eprintln!("orbitty:companion-chrome deferred minimized=true");
        return;
    }
    let ctx = LightsCtx {
        mac: cfg!(target_os = "macos"),
        companion_expanded: on,
        companion_visible: true,
    };
    let run = move || {
        apply_lights(&window, "companion", ctx);
        let behavior = crate::glass::collection_behavior(&window);
        eprintln!("orbitty:companion-chrome-behavior {behavior}");
    };
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(run);
    #[cfg(not(target_os = "macos"))]
    run();
}

pub fn sync_companion_chrome(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("companion") else {
        return;
    };
    let ctx = lights_ctx(app);
    let minimized = window.is_minimized().unwrap_or(false)
        || crate::glass::is_miniaturized(&window);
    if ctx.companion_expanded && minimized {
        eprintln!("orbitty:companion-chrome deferred minimized=true");
        return;
    }
    let run = move || apply_lights(&window, "companion", ctx);
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(run);
    #[cfg(not(target_os = "macos"))]
    run();
}

/// True when `frame` is the screen visible frame (a finished zoom).
fn frame_is_zoomed_size(window: &tauri::WebviewWindow, frame: PlainRect) -> bool {
    let Some((vw, vh)) = crate::glass::visible_frame_size(window) else {
        return false;
    };
    ((frame.w - vw).abs() < 8.0 && (frame.h - vh).abs() < 8.0)
        || (frame.w > vw * 0.92 && frame.h > vh * 0.92)
}

/// Zoom animation frames arrive with isZoomed still false. They grow in
/// both axes toward the visible frame. A host grow changes height (or a
/// single axis) and must still update the pre-zoom frame.
fn zoom_animation_frame(
    window: &tauri::WebviewWindow,
    last: Option<PlainRect>,
    frame: PlainRect,
) -> bool {
    if frame_is_zoomed_size(window, frame) {
        return true;
    }
    let Some((vw, vh)) = crate::glass::visible_frame_size(window) else {
        return false;
    };
    let Some(prev) = last else {
        return false;
    };
    let grew = frame.w > prev.w + 40.0 && frame.h > prev.h + 40.0;
    let short = frame.w < vw - 8.0 || frame.h < vh - 8.0;
    grew && short
}

pub fn on_resized(window: &tauri::WebviewWindow, label: &str) {
    if ZOOM_SUPPRESS.load(Ordering::SeqCst) {
        return;
    }
    if label == "kanban" || label == "stars" {
        crate::glass::show_and_position_lights(window, TL_X, ROW_CENTRE);
        crate::glass::apply_fullscreen_none(window);
        return;
    }
    if label != "companion" {
        return;
    }
    let zoomed_now = crate::glass::is_zoomed(window);
    let frame = live_frame(window);
    let mut entering = false;
    let mut leaving: Option<PlainRect> = None;
    if let Ok(mut z) = zoom_mem().lock() {
        if zoomed_now && !z.zoomed {
            // Stash the last frame that was not a zoom animation frame.
            // `or(frame)` poisoned the stash with the visible frame while
            // isZoomed was still false, and the leave path then re-zoomed.
            z.stash = z.last_normal;
            z.zoomed = true;
            entering = true;
        } else if !zoomed_now && z.zoomed {
            z.zoomed = false;
            leaving = z.stash.take();
        } else if !zoomed_now {
            if let Some(frame) = frame {
                if !zoom_animation_frame(window, z.last_normal, frame) {
                    z.last_normal = Some(frame);
                }
            }
        }
    }
    if entering {
        if let Ok(z) = zoom_mem().lock() {
            if let Some(s) = z.stash {
                eprintln!(
                    "orbitty:companion-zoom stash x={:.1} y={:.1} w={:.1} h={:.1}",
                    s.x, s.y, s.w, s.h
                );
            }
        }
    }
    if let Some(stash) = leaving {
        if let Some(now) = live_frame(window) {
            // Pull back only when AppKit left us on the zoomed size and
            // the stash is the smaller pre-zoom frame. Placing a poisoned
            // (zoomed-size) stash is what bounced the window back.
            let now_zoomed = frame_is_zoomed_size(window, now);
            let stash_zoomed = frame_is_zoomed_size(window, stash);
            if now_zoomed && !stash_zoomed && !near(now, stash) {
                place_stash(window, stash);
            }
            if let Ok(mut z) = zoom_mem().lock() {
                z.last_normal = Some(if stash_zoomed { now } else { stash });
            }
        }
        eprintln!(
            "orbitty:companion-unzoom-edge x={:.1} y={:.1} w={:.1} h={:.1}",
            stash.x, stash.y, stash.w, stash.h
        );
    }
    if COMPANION_CHROME.load(Ordering::SeqCst) {
        let mask = crate::glass::style_mask(window);
        let want = companion_mask_on(mask);
        if mask & STYLE_TITLED == 0 || mask & STYLE_FULLSIZE == 0 {
            let (dx, dy, dw, dh) = crate::glass::apply_companion_mask(window, true);
            eprintln!(
                "orbitty:companion-mask-reassert dx={dx:.2} dy={dy:.2} dw={dw:.2} dh={dh:.2} want={want}"
            );
        }
        crate::glass::show_and_position_lights(window, TL_X, ROW_CENTRE);
    }
}

#[tauri::command]
pub fn window_appearance(window: tauri::WebviewWindow, scheme: String) -> Result<(), String> {
    let theme = if scheme == "light" {
        tauri::utils::Theme::Light
    } else {
        tauri::utils::Theme::Dark
    };
    window.set_theme(Some(theme)).map_err(|e| e.to_string())?;
    let label = window.label();
    if label == "kanban" || label == "stars" || label == "companion" {
        crate::glass::show_and_position_lights(&window, TL_X, ROW_CENTRE);
    }
    Ok(())
}

#[tauri::command]
pub fn window_title_dblclick(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if label == "companion" || (label != "kanban" && label != "stars") {
        return Ok(());
    }
    let Some(window) = app.get_webview_window(&label) else {
        return Err(format!("no window labeled {label}"));
    };
    let pref = crate::glass::apple_action_on_double_click();
    let action = dblclick_pref_parse(pref.as_deref());
    eprintln!(
        "orbitty:title-dblclick label={label} pref={} action={action:?}",
        pref.as_deref().unwrap_or("<missing>")
    );
    match action {
        DblClick::Maximize => crate::glass::perform_zoom(&window),
        DblClick::Minimize => crate::glass::perform_miniaturize(&window),
        DblClick::None => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac_exp() -> LightsCtx {
        LightsCtx {
            mac: true,
            companion_expanded: true,
            companion_visible: true,
        }
    }

    #[test]
    fn lights_main_never() {
        let mut ctx = mac_exp();
        assert!(!wants_traffic_lights("main", ctx));
        ctx.companion_expanded = false;
        assert!(!wants_traffic_lights("main", ctx));
        ctx.mac = false;
        assert!(!wants_traffic_lights("main", ctx));
    }

    #[test]
    fn lights_kanban_stars_mac_only() {
        assert!(wants_traffic_lights("kanban", mac_exp()));
        assert!(wants_traffic_lights("stars", mac_exp()));
        let other = LightsCtx {
            mac: false,
            companion_expanded: true,
            companion_visible: true,
        };
        assert!(!wants_traffic_lights("kanban", other));
        assert!(!wants_traffic_lights("stars", other));
    }

    #[test]
    fn lights_companion_only_expanded() {
        assert!(wants_traffic_lights("companion", mac_exp()));
        assert!(!wants_traffic_lights(
            "companion",
            LightsCtx {
                mac: true,
                companion_expanded: false,
                companion_visible: true,
            }
        ));
        assert!(!wants_traffic_lights(
            "companion",
            LightsCtx {
                mac: true,
                companion_expanded: true,
                companion_visible: false,
            }
        ));
        assert!(!wants_traffic_lights(
            "companion",
            LightsCtx {
                mac: false,
                companion_expanded: true,
                companion_visible: true,
            }
        ));
    }

    #[test]
    fn lights_other_panes_never() {
        for label in ["settings", "sky", "cheatsheet", "next-card", "yard-warn", "nope"] {
            assert!(!wants_traffic_lights(label, mac_exp()), "{label}");
        }
    }

    #[test]
    fn companion_mask_on_has_titled_closable_mini_fullsize() {
        let on = companion_mask_on(STYLE_RESIZABLE | (1 << 14));
        assert_eq!(on & STYLE_TITLED, STYLE_TITLED);
        assert_eq!(on & STYLE_CLOSABLE, STYLE_CLOSABLE);
        assert_eq!(on & STYLE_MINI, STYLE_MINI);
        assert_eq!(on & STYLE_FULLSIZE, STYLE_FULLSIZE);
        assert_eq!(on & STYLE_RESIZABLE, STYLE_RESIZABLE);
        assert_eq!(on & (1 << 14), 0);
    }

    #[test]
    fn companion_mask_off_borderless_keeps_resizable() {
        let on = companion_mask_on(STYLE_RESIZABLE);
        let off = companion_mask_off(on);
        assert_eq!(off & STYLE_RESIZABLE, STYLE_RESIZABLE);
        assert_eq!(off & STYLE_TITLED, 0);
        assert_eq!(off & STYLE_CLOSABLE, 0);
        assert_eq!(off & STYLE_MINI, 0);
        assert_eq!(off & STYLE_FULLSIZE, 0);
        assert_eq!(companion_mask_off(0), STYLE_RESIZABLE);
    }

    #[test]
    fn companion_frame_locked_minimized_or_zoomed() {
        assert!(companion_frame_locked(true, false));
        assert!(companion_frame_locked(false, true));
        assert!(companion_frame_locked(true, true));
        assert!(!companion_frame_locked(false, false));
        for kind in ["host", "host-hug", "hug", "grow", "dock"] {
            assert!(frame_write_skipped(true, false, kind), "{kind}");
            assert!(frame_write_skipped(false, true, kind), "{kind}");
            assert!(!frame_write_skipped(false, false, kind), "{kind}");
        }
    }

    #[test]
    fn unzoom_before_persist_uses_stash() {
        let live = PlainRect {
            x: 0.0,
            y: 0.0,
            w: 1400.0,
            h: 900.0,
        };
        let stash = PlainRect {
            x: 40.0,
            y: 80.0,
            w: 619.0,
            h: 647.0,
        };
        assert_eq!(frame_for_persist(true, Some(stash), live), stash);
        assert_eq!(frame_for_persist(false, Some(stash), live), live);
        assert_eq!(frame_for_persist(true, None, live), live);
    }

    #[test]
    fn collapse_or_host_while_zoomed_unzooms_first() {
        assert_eq!(
            collapse_or_host_steps(true),
            &[ZoomStep::Unzoom, ZoomStep::Proceed]
        );
        assert_eq!(collapse_or_host_steps(false), &[ZoomStep::Proceed]);
    }

    #[test]
    fn fullscreen_none_bits() {
        let out = super::fullscreen_none_bits(273 | FS_PRIMARY);
        assert_eq!(out & FS_NONE, FS_NONE);
        assert_eq!(out & FS_PRIMARY, 0);
        assert_eq!(out & 273, 273);
        assert_eq!(super::fullscreen_none_bits(0) & FS_NONE, FS_NONE);
    }

    #[test]
    fn dblclick_pref_parse() {
        assert_eq!(super::dblclick_pref_parse(Some("Maximize")), DblClick::Maximize);
        assert_eq!(super::dblclick_pref_parse(Some("Minimize")), DblClick::Minimize);
        assert_eq!(super::dblclick_pref_parse(Some("None")), DblClick::None);
        assert_eq!(super::dblclick_pref_parse(None), DblClick::Maximize);
        assert_eq!(super::dblclick_pref_parse(Some("")), DblClick::Maximize);
        assert_eq!(super::dblclick_pref_parse(Some("other")), DblClick::Maximize);
    }

    #[test]
    fn pane_vis_state_minimized_not_lost() {
        assert_eq!(pane_vis_state(false, true), PaneVis::Minimized);
        assert_eq!(pane_vis_state(true, true), PaneVis::Minimized);
        assert!(!pane_is_lost(PaneVis::Minimized));
        assert!(!pane_is_lost(pane_vis_state(false, true)));
        assert!(pane_is_lost(pane_vis_state(false, false)));
        assert_eq!(pane_vis_state(true, false), PaneVis::Shown);
        assert_eq!(pane_vis_state(false, false), PaneVis::Hidden);
    }

    #[test]
    fn traffic_light_y_centres_on_row() {
        let y = traffic_light_y(38.0, 14.0);
        let centre = y + 14.0 / 2.0;
        assert!((centre - 19.0).abs() <= 2.0, "centre {centre} y {y}");
        assert!((TL_Y - y).abs() < 0.01);
    }
}
