// Included from glass.rs `imp`. Gated by GLASS_FR3_DIAG=1. Removed before the last commit.
// Dumps view trees and card rects. The per-card and mask phases are diag-only probes.

use tauri::Manager;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorGetNumberOfComponents(color: *mut std::ffi::c_void) -> usize;
    fn CGColorGetComponents(color: *mut std::ffi::c_void) -> *const f64;
    fn CGPathCreateMutable() -> *mut std::ffi::c_void;
    fn CGPathAddRoundedRect(
        path: *mut std::ffi::c_void,
        transform: *const std::ffi::c_void,
        rect: NSRect,
        corner_width: f64,
        corner_height: f64,
    );
    fn CGPathRelease(path: *mut std::ffi::c_void);
}

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {
    fn CACurrentMediaTime() -> f64;
}

const FR3_MAX_NODES: usize = 320;
const FR3_MAX_DEPTH: usize = 8;

struct Fr3Geom {
    rects: Vec<[f64; 4]>,
}

static FR3_GEOM: std::sync::Mutex<Vec<(String, Fr3Geom)>> = std::sync::Mutex::new(Vec::new());

struct Fr3Walk {
    label: String,
    nodes: usize,
    truncated: bool,
    effects: usize,
    win_w: f64,
    win_h: f64,
}

fn fr3_near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 2.0
}

unsafe fn fr3_fmt_cg(cg: *mut std::ffi::c_void) -> String {
    if cg.is_null() {
        return "none".to_string();
    }
    let n = CGColorGetNumberOfComponents(cg);
    let comps = CGColorGetComponents(cg);
    if comps.is_null() || n == 0 {
        return "empty".to_string();
    }
    if n == 2 {
        return format!("ga({:.3},{:.3})", *comps, *comps.add(1));
    }
    if n >= 4 {
        return format!(
            "rgba({:.3},{:.3},{:.3},{:.3})",
            *comps,
            *comps.add(1),
            *comps.add(2),
            *comps.add(3)
        );
    }
    "other".to_string()
}

unsafe fn fr3_fmt_nscolor(color: *mut Object) -> String {
    if color.is_null() {
        return "null".to_string();
    }
    let cg: *mut std::ffi::c_void = msg_send![color, CGColor];
    fr3_fmt_cg(cg)
}

unsafe fn fr3_class_name(obj: *mut Object) -> String {
    if obj.is_null() {
        return "null".to_string();
    }
    let cls: *const Class = msg_send![obj, class];
    if cls.is_null() {
        return "noclass".to_string();
    }
    (*cls).name().to_string()
}

fn fr3_fmt_rect(r: NSRect) -> String {
    format!(
        "{:.1},{:.1},{:.1}x{:.1}",
        r.origin.x, r.origin.y, r.size.width, r.size.height
    )
}

unsafe fn fr3_layer(view: *mut Object) -> (bool, String, String, String, String) {
    let wants: bool = msg_send![view, wantsLayer];
    let layer: *mut Object = msg_send![view, layer];
    if layer.is_null() {
        return (
            wants,
            "none".to_string(),
            "none".to_string(),
            "none".to_string(),
            "none".to_string(),
        );
    }
    let bg: *mut std::ffi::c_void = msg_send![layer, backgroundColor];
    let opaque: bool = msg_send![layer, isOpaque];
    let mask: *mut Object = msg_send![layer, mask];
    let mask_s = if mask.is_null() {
        "none".to_string()
    } else {
        fr3_class_name(mask)
    };
    let mtb: bool = msg_send![layer, masksToBounds];
    (
        wants,
        fr3_fmt_cg(bg),
        if opaque { "true" } else { "false" }.to_string(),
        mask_s,
        if mtb { "true" } else { "false" }.to_string(),
    )
}

unsafe fn fr3_walk(view: *mut Object, path: &str, depth: usize, walk: &mut Fr3Walk) {
    if view.is_null() || walk.truncated {
        return;
    }
    if walk.nodes >= FR3_MAX_NODES || depth > FR3_MAX_DEPTH {
        walk.truncated = true;
        return;
    }
    walk.nodes += 1;
    let name = fr3_class_name(view);
    let frame: NSRect = msg_send![view, frame];
    let hidden: bool = msg_send![view, isHidden];
    let view_opaque: bool = msg_send![view, isOpaque];
    let (wants, layer_bg, layer_opaque, mask, mtb) = fr3_layer(view);
    eprintln!(
        "GLASS_FR3_DIAG view label={} path={path} class={name} frame={} hidden={hidden} wantsLayer={wants} viewOpaque={view_opaque} layerBg={layer_bg} layerOpaque={layer_opaque} mask={mask} masksToBounds={mtb}",
        walk.label,
        fr3_fmt_rect(frame)
    );
    if let Some(effect_cls) = Class::get("NSVisualEffectView") {
        let is_effect: bool = msg_send![view, isKindOfClass: effect_cls];
        if is_effect {
            walk.effects += 1;
            let material: isize = msg_send![view, material];
            let state: isize = msg_send![view, state];
            let blending: isize = msg_send![view, blendingMode];
            let sized =
                fr3_near(frame.size.width, walk.win_w) && fr3_near(frame.size.height, walk.win_h);
            eprintln!(
                "GLASS_FR3_DIAG effect label={} path={path} class={name} material={material} state={state} blending={blending} frame={} windowSized={sized}",
                walk.label,
                fr3_fmt_rect(frame)
            );
        }
    }
    if let Some(wk_cls) = Class::get("WKWebView") {
        let is_wk: bool = msg_send![view, isKindOfClass: wk_cls];
        if is_wk {
            let draws_obj: *mut Object = msg_send![view, valueForKey: ns_string("drawsBackground")];
            let draws: bool = if draws_obj.is_null() {
                false
            } else {
                msg_send![draws_obj, boolValue]
            };
            eprintln!(
                "GLASS_FR3_DIAG webview label={} path={path} drawsBackground={draws}",
                walk.label
            );
        }
    }
    let subs: *mut Object = msg_send![view, subviews];
    if subs.is_null() {
        return;
    }
    let count: usize = msg_send![subs, count];
    for i in 0..count {
        if walk.truncated {
            return;
        }
        let sub: *mut Object = msg_send![subs, objectAtIndex: i];
        let child = if path.is_empty() {
            format!("{i}")
        } else {
            format!("{path}.{i}")
        };
        fr3_walk(sub, &child, depth + 1, walk);
    }
}

unsafe fn fr3_reduce() -> &'static str {
    let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    if ws.is_null() {
        return "null";
    }
    let ok: bool = msg_send![
        ws,
        respondsToSelector: sel!(accessibilityDisplayShouldReduceTransparency)
    ];
    if !ok {
        return "unsupported";
    }
    let on: bool = msg_send![ws, accessibilityDisplayShouldReduceTransparency];
    if on {
        "true"
    } else {
        "false"
    }
}

fn fr3_windows(app: &tauri::AppHandle) -> Vec<(String, tauri::WebviewWindow)> {
    let mut wins: Vec<_> = app.webview_windows().into_iter().collect();
    wins.sort_by(|a, b| a.0.cmp(&b.0));
    wins
}

pub fn fr3_dump_all(app: &tauri::AppHandle) {
    let conf = include_str!("../../tauri.conf.json");
    let transparent = conf.contains("\"transparent\": true");
    let private_api = conf.contains("macOSPrivateApi");
    let (enabled, light) = crate::vibrancy_mac::wanted();
    let reduce = unsafe { fr3_reduce() };
    eprintln!(
        "GLASS_FR3_DIAG config transparent={transparent} macOSPrivateApi={} wantedEnabled={enabled} wantedLight={light} reduceTransparency={reduce}",
        if private_api { "present" } else { "absent" }
    );
    let wins = fr3_windows(app);
    if wins.is_empty() {
        eprintln!("GLASS_FR3_DIAG windows=0");
    }
    for (label, window) in wins {
        eprintln!("GLASS_FR3_DIAG open label={label}");
        unsafe { fr3_dump_one(&window) };
    }
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

unsafe fn fr3_dump_one(window: &tauri::WebviewWindow) {
    let Some(ns_window) = ns_window(window) else {
        eprintln!(
            "GLASS_FR3_DIAG begin label={} window=missing",
            window.label()
        );
        return;
    };
    let label = window.label().to_string();
    let opaque: bool = msg_send![ns_window, isOpaque];
    let bg: *mut Object = msg_send![ns_window, backgroundColor];
    let bg_s = fr3_fmt_nscolor(bg);
    let frame: NSRect = msg_send![ns_window, frame];
    let visible: bool = msg_send![ns_window, isVisible];
    let content: *mut Object = msg_send![ns_window, contentView];
    let content_frame = if content.is_null() {
        "null".to_string()
    } else {
        let r: NSRect = msg_send![content, frame];
        fr3_fmt_rect(r)
    };
    eprintln!(
        "GLASS_FR3_DIAG begin label={label} isOpaque={opaque} backgroundColor={bg_s} isVisible={visible} frame={} contentFrame={content_frame}",
        fr3_fmt_rect(frame)
    );
    let mut walk = Fr3Walk {
        label: label.clone(),
        nodes: 0,
        truncated: false,
        effects: 0,
        win_w: frame.size.width,
        win_h: frame.size.height,
    };
    let root = if content.is_null() {
        std::ptr::null_mut()
    } else {
        let parent: *mut Object = msg_send![content, superview];
        if parent.is_null() {
            content
        } else {
            parent
        }
    };
    fr3_walk(root, "0", 0, &mut walk);
    eprintln!(
        "GLASS_FR3_DIAG end label={label} nodes={} truncated={} effects={}",
        walk.nodes, walk.truncated, walk.effects
    );
}

fn fr3_parse_page(payload: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(payload.trim()).ok()?;
    if let Some(s) = v.as_str() {
        return serde_json::from_str(s).ok();
    }
    Some(v)
}

fn fr3_store_page(label: &str, payload: &str) {
    let Some(v) = fr3_parse_page(payload) else {
        eprintln!(
            "GLASS_FR3_DIAG page label={label} parse-failed bytes={}",
            payload.len()
        );
        return;
    };
    if v.get("err").is_some() {
        let code = v.get("err").and_then(|e| e.as_str()).unwrap_or("");
        let code = if code == "no-collect" || code == "eval-failed" {
            code
        } else {
            "other"
        };
        eprintln!("GLASS_FR3_DIAG page label={label} err={code}");
        return;
    }
    eprintln!("GLASS_FR3_DIAG page label={label} {payload}");
    let mut rects = Vec::new();
    if let Some(arr) = v.get("place").and_then(|x| x.as_array()) {
        for item in arr.iter().take(24) {
            let x = item.get("x").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let y = item.get("y").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let w = item.get("w").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let h = item.get("h").and_then(|n| n.as_f64()).unwrap_or(0.0);
            if w >= 40.0 && h >= 24.0 {
                rects.push([x, y, w, h]);
            }
        }
    }
    eprintln!(
        "GLASS_FR3_DIAG place-stored label={label} n={}",
        rects.len()
    );
    if let Ok(mut g) = FR3_GEOM.lock() {
        g.retain(|(k, _)| k != label);
        g.push((label.to_string(), Fr3Geom { rects }));
    }
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

fn fr3_eval_pages(app: &tauri::AppHandle) {
    let js = r#"(function(){try{if(typeof window.__glassFr3Collect==="function")return window.__glassFr3Collect();return{err:"no-collect"};}catch(e){return{err:"eval-failed"};}})()"#;
    for (label, window) in fr3_windows(app) {
        let tag = label.clone();
        if let Err(_err) = window.eval_with_callback(js, move |payload| {
            fr3_store_page(&tag, &payload);
        }) {
            eprintln!("GLASS_FR3_DIAG page label={label} eval-error");
        }
    }
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

pub fn fr3_baseline(app: &tauri::AppHandle) {
    fr3_dump_all(app);
    fr3_eval_pages(app);
}

fn fr3_geom(label: &str) -> Option<Fr3Geom> {
    let Ok(g) = FR3_GEOM.lock() else {
        return None;
    };
    g.iter().find(|(k, _)| k == label).map(|(_, geom)| Fr3Geom {
        rects: geom.rects.clone(),
    })
}

unsafe fn fr3_host(content: *mut Object) -> (*mut Object, *mut Object) {
    let host: *mut Object = msg_send![content, superview];
    if host.is_null() {
        (content, std::ptr::null_mut())
    } else {
        (host, content)
    }
}

unsafe fn fr3_remove_effects(root: *mut Object) {
    let Some(cls) = Class::get("NSVisualEffectView") else {
        return;
    };
    let subs: *mut Object = msg_send![root, subviews];
    if subs.is_null() {
        return;
    }
    let count: usize = msg_send![subs, count];
    let mut hit = Vec::new();
    for i in 0..count {
        let sub: *mut Object = msg_send![subs, objectAtIndex: i];
        if sub.is_null() {
            continue;
        }
        let is_effect: bool = msg_send![sub, isKindOfClass: cls];
        if is_effect {
            hit.push(sub);
        }
    }
    for v in hit {
        let _: () = msg_send![v, removeFromSuperview];
    }
}

unsafe fn fr3_map(content: *mut Object, r: [f64; 4]) -> NSRect {
    let cf: NSRect = msg_send![content, frame];
    NSRect {
        origin: NSPoint {
            x: cf.origin.x + r[0],
            y: cf.origin.y + cf.size.height - r[1] - r[3],
        },
        size: NSSize {
            width: r[2],
            height: r[3],
        },
    }
}

unsafe fn fr3_rects(content: *mut Object, label: &str) -> (Vec<NSRect>, &'static str) {
    if let Some(g) = fr3_geom(label) {
        if !g.rects.is_empty() {
            let mapped = g
                .rects
                .iter()
                .copied()
                .map(|r| fr3_map(content, r))
                .collect();
            return (mapped, "dom");
        }
    }
    let b: NSRect = msg_send![content, bounds];
    let vw = b.size.width;
    let vh = b.size.height;
    let w = 180.0;
    let h = 100.0;
    let mut dom = Vec::new();
    for row in 0..2 {
        for col in 0..3 {
            let x = 24.0 + col as f64 * (w + 16.0);
            let y = 48.0 + row as f64 * (h + 20.0);
            if x + w <= vw - 8.0 && y + h <= vh - 8.0 {
                dom.push([x, y, w, h]);
            }
        }
    }
    let mapped = dom.iter().copied().map(|r| fr3_map(content, r)).collect();
    (mapped, "grid")
}

unsafe fn fr3_add_effect(parent: *mut Object, anchor: *mut Object, frame: NSRect) -> *mut Object {
    let cls = class!(NSVisualEffectView);
    let view: *mut Object = msg_send![cls, alloc];
    if view.is_null() {
        return std::ptr::null_mut();
    }
    let view: *mut Object = msg_send![view, initWithFrame: frame];
    if view.is_null() {
        return std::ptr::null_mut();
    }
    let _: () = msg_send![view, setWantsLayer: true];
    let _: () = msg_send![
        parent,
        addSubview: view
        positioned: -1isize
        relativeTo: anchor
    ];
    let _: () = msg_send![view, release];
    let _: () = msg_send![view, setMaterial: 13isize];
    let _: () = msg_send![view, setBlendingMode: 0isize];
    let _: () = msg_send![view, setState: 1isize];
    let _: () = msg_send![view, setAutoresizingMask: 0usize];
    view
}

unsafe fn fr3_apply_mask(view: *mut Object, rects: &[NSRect]) {
    let _ = CACurrentMediaTime();
    let Some(cls) = Class::get("CAShapeLayer") else {
        eprintln!("GLASS_FR3_DIAG mask class-missing");
        return;
    };
    let layer: *mut Object = msg_send![view, layer];
    if layer.is_null() {
        eprintln!("GLASS_FR3_DIAG mask layer-missing");
        return;
    }
    let path = CGPathCreateMutable();
    if path.is_null() {
        eprintln!("GLASS_FR3_DIAG mask path-missing");
        return;
    }
    for r in rects {
        // Mask path is in the effect view's bounds. The probe view fills the
        // host from (0,0), so host-space frames match that space.
        let local = NSRect {
            origin: r.origin,
            size: r.size,
        };
        CGPathAddRoundedRect(path, std::ptr::null(), local, 12.0, 12.0);
    }
    let shape: *mut Object = msg_send![cls, layer];
    if shape.is_null() {
        CGPathRelease(path);
        eprintln!("GLASS_FR3_DIAG mask shape-missing");
        return;
    }
    let _: () = msg_send![shape, setPath: path];
    CGPathRelease(path);
    let _: () = msg_send![layer, setMask: shape];
}

unsafe fn fr3_content(window: &tauri::WebviewWindow) -> Option<*mut Object> {
    let ns_window = ns_window(window)?;
    let content: *mut Object = msg_send![ns_window, contentView];
    if content.is_null() {
        None
    } else {
        Some(content)
    }
}

pub fn fr3_experiment_cards(app: &tauri::AppHandle) {
    for (label, window) in fr3_windows(app) {
        unsafe {
            let Some(content) = fr3_content(&window) else {
                eprintln!("GLASS_FR3_DIAG probe kind=per-card label={label} content=missing");
                continue;
            };
            let (host, anchor) = fr3_host(content);
            fr3_remove_effects(host);
            let (rects, source) = fr3_rects(content, &label);
            eprintln!(
                "GLASS_FR3_DIAG probe kind=per-card label={label} source={source} n={}",
                rects.len()
            );
            for (i, frame) in rects.iter().copied().enumerate() {
                let placed = fr3_add_effect(host, anchor, frame);
                eprintln!(
                    "GLASS_FR3_DIAG probe-rect label={label} i={i} frame={} placed={}",
                    fr3_fmt_rect(frame),
                    !placed.is_null()
                );
            }
        }
    }
    fr3_dump_all(app);
}

pub fn fr3_experiment_mask(app: &tauri::AppHandle) {
    for (label, window) in fr3_windows(app) {
        unsafe {
            let Some(content) = fr3_content(&window) else {
                eprintln!("GLASS_FR3_DIAG probe kind=mask label={label} content=missing");
                continue;
            };
            let (host, anchor) = fr3_host(content);
            fr3_remove_effects(host);
            let (rects, source) = fr3_rects(content, &label);
            let bounds: NSRect = msg_send![host, bounds];
            let view = fr3_add_effect(host, anchor, bounds);
            if view.is_null() {
                eprintln!("GLASS_FR3_DIAG probe kind=mask label={label} placed=false");
                continue;
            }
            fr3_apply_mask(view, &rects);
            eprintln!(
                "GLASS_FR3_DIAG probe kind=mask label={label} source={source} n={} frame={}",
                rects.len(),
                fr3_fmt_rect(bounds)
            );
        }
    }
    fr3_dump_all(app);
}

pub fn fr3_experiment_restore(app: &tauri::AppHandle) {
    for (label, window) in fr3_windows(app) {
        unsafe {
            if let Some(content) = fr3_content(&window) {
                let (host, _) = fr3_host(content);
                fr3_remove_effects(host);
            }
        }
        let _ = crate::vibrancy_mac::sync_one(&window);
        eprintln!("GLASS_FR3_DIAG restore label={label}");
    }
    fr3_dump_all(app);
}
