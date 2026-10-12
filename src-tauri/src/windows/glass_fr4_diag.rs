#![allow(unexpected_cfgs)]
// Gated by GLASS_FR4_DIAG=1. Observation only. Removed before the last commit.
// No titles, paths, or document text.

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorGetNumberOfComponents(color: *mut std::ffi::c_void) -> usize;
    fn CGColorGetComponents(color: *mut std::ffi::c_void) -> *const f64;
}

#[derive(Clone, Copy)]
struct Fact {
    enabled: bool,
    light: bool,
    reduce: bool,
    reach_apply: bool,
    frames: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsPoint {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NsSize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NsRect {
    origin: NsPoint,
    size: NsSize,
}

struct Walk {
    effects: Vec<*mut Object>,
    wk: *mut Object,
    nodes: usize,
    truncated: bool,
}

fn diag_on() -> bool {
    std::env::var("GLASS_FR4_DIAG").ok().as_deref() == Some("1")
}

fn flush_err() {
    let _ = std::io::stderr().flush();
}

fn facts() -> &'static Mutex<HashMap<String, Fact>> {
    static FACTS: OnceLock<Mutex<HashMap<String, Fact>>> = OnceLock::new();
    FACTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn css_once() -> &'static Mutex<HashSet<String>> {
    static ONCE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ONCE.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn on_rects(label: &str, rects: &[super::DomRect]) {
    if !diag_on() {
        return;
    }
    let sample = match rects.first() {
        Some(r) => format!("{:.1},{:.1},{:.1}x{:.1}", r.x, r.y, r.w, r.h),
        None => "none".to_string(),
    };
    eprintln!(
        "GLASS_FR4_DIAG rects label={label} count={} sample={sample}",
        rects.len()
    );
    flush_err();
}

pub fn note(
    label: &str,
    enabled: bool,
    light: bool,
    reduce: bool,
    reach_apply: bool,
    frames: usize,
) {
    if !diag_on() {
        return;
    }
    if let Ok(mut g) = facts().lock() {
        g.insert(
            label.to_string(),
            Fact {
                enabled,
                light,
                reduce,
                reach_apply,
                frames,
            },
        );
    }
}

pub fn after_sync(window: &tauri::WebviewWindow) {
    if !diag_on() {
        return;
    }
    let label = window.label().to_string();
    let fact = facts()
        .lock()
        .ok()
        .and_then(|g| g.get(&label).copied());
    unsafe { dump_window(window, &label, fact) };
    eval_css(window);
    schedule_css(window);
}

fn schedule_css(window: &tauri::WebviewWindow) {
    let label = window.label().to_string();
    let fresh = css_once()
        .lock()
        .ok()
        .map(|mut g| g.insert(label))
        .unwrap_or(false);
    if !fresh {
        return;
    }
    let late = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        eval_css(&late);
        std::thread::sleep(std::time::Duration::from_millis(5500));
        eval_css(&late);
    });
}

fn eval_css(window: &tauri::WebviewWindow) {
    let label = window.label().to_string();
    let js = r#"(function(){try{var fn=window.__glassFr4Collect;if(typeof fn==="function")return fn();return{err:"no-collect"};}catch(e){return{err:"eval-failed"};}})()"#;
    let tag = label.clone();
    if window
        .eval_with_callback(js, move |payload| print_css(&tag, &payload))
        .is_err()
    {
        eprintln!("GLASS_FR4_DIAG css label={label} eval-error");
        flush_err();
    }
}

fn safe_line(s: &str) -> bool {
    s.starts_with("GLASS_FR4_DIAG ")
        && s.len() < 800
        && !s.contains("Users/")
        && !s.contains("http")
        && !s.contains("file:")
}

fn print_css(label: &str, payload: &str) {
    let trimmed = payload.trim();
    let parsed = match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v) => {
            if let Some(inner) = v.as_str() {
                serde_json::from_str(inner).unwrap_or(v)
            } else {
                v
            }
        }
        Err(_) => {
            eprintln!(
                "GLASS_FR4_DIAG css label={label} err=parse bytes={}",
                trimmed.len()
            );
            flush_err();
            return;
        }
    };
    if let Some(err) = parsed.get("err").and_then(|e| e.as_str()) {
        let code = if err == "no-collect" || err == "eval-failed" {
            err
        } else {
            "other"
        };
        eprintln!("GLASS_FR4_DIAG css label={label} err={code}");
        flush_err();
        return;
    }
    let Some(lines) = parsed.get("lines").and_then(|v| v.as_array()) else {
        eprintln!("GLASS_FR4_DIAG css label={label} err=no-lines");
        flush_err();
        return;
    };
    let mut n = 0;
    for line in lines {
        if n >= 40 {
            break;
        }
        let Some(s) = line.as_str() else { continue };
        if safe_line(s) {
            eprintln!("{s}");
            n += 1;
        }
    }
    flush_err();
}

unsafe fn ns_string(s: &str) -> *mut Object {
    let cf = CFString::new(s);
    let raw = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    raw
}

unsafe fn fmt_cg(cg: *mut std::ffi::c_void) -> String {
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

unsafe fn fmt_nscolor(color: *mut Object) -> String {
    if color.is_null() {
        return "null".to_string();
    }
    let cg: *mut std::ffi::c_void = msg_send![color, CGColor];
    fmt_cg(cg)
}

unsafe fn class_name(obj: *mut Object) -> String {
    if obj.is_null() {
        return "null".to_string();
    }
    let cls: *const Class = msg_send![obj, class];
    if cls.is_null() {
        return "noclass".to_string();
    }
    (*cls)
        .name()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(64)
        .collect()
}

fn fmt_rect(r: NsRect) -> String {
    format!(
        "{:.1},{:.1},{:.1}x{:.1}",
        r.origin.x, r.origin.y, r.size.width, r.size.height
    )
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 2.0
}

unsafe fn is_effect(view: *mut Object) -> bool {
    let Some(cls) = Class::get("OrbittyVisualEffectView") else {
        return false;
    };
    msg_send![view, isKindOfClass: cls]
}

unsafe fn layer_bits(view: *mut Object) -> (bool, String, String, String) {
    let wants: bool = msg_send![view, wantsLayer];
    let layer: *mut Object = msg_send![view, layer];
    if layer.is_null() {
        return (
            wants,
            "none".to_string(),
            "none".to_string(),
            "none".to_string(),
        );
    }
    let op: f32 = msg_send![layer, opacity];
    let bg: *mut std::ffi::c_void = msg_send![layer, backgroundColor];
    let layer_opaque: bool = msg_send![layer, isOpaque];
    (
        wants,
        format!("{op:.3}"),
        fmt_cg(bg),
        if layer_opaque { "true" } else { "false" }.to_string(),
    )
}

unsafe fn sibling_index(view: *mut Object) -> (usize, usize) {
    let sv: *mut Object = msg_send![view, superview];
    if sv.is_null() {
        return (0, 0);
    }
    let subs: *mut Object = msg_send![sv, subviews];
    if subs.is_null() {
        return (0, 0);
    }
    let count: usize = msg_send![subs, count];
    let mut idx = 0;
    for i in 0..count {
        let sub: *mut Object = msg_send![subs, objectAtIndex: i];
        if sub == view {
            idx = i;
        }
    }
    (idx, count)
}

unsafe fn walk(view: *mut Object, depth: usize, found: &mut Walk) {
    if view.is_null() || found.truncated {
        return;
    }
    if found.nodes >= 480 || depth > 16 {
        found.truncated = true;
        return;
    }
    found.nodes += 1;
    if found.wk.is_null() {
        if let Some(cls) = Class::get("WKWebView") {
            let is_wk: bool = msg_send![view, isKindOfClass: cls];
            if is_wk {
                found.wk = view;
            }
        }
    }
    if is_effect(view) && found.effects.len() < 48 {
        found.effects.push(view);
    }
    let subs: *mut Object = msg_send![view, subviews];
    if subs.is_null() {
        return;
    }
    let count: usize = msg_send![subs, count];
    for i in 0..count {
        if found.truncated {
            return;
        }
        let sub: *mut Object = msg_send![subs, objectAtIndex: i];
        walk(sub, depth + 1, found);
    }
}

unsafe fn host_ancestor_index(wk: *mut Object, host: *mut Object) -> usize {
    let mut cur = wk;
    while !cur.is_null() {
        let parent: *mut Object = msg_send![cur, superview];
        if parent == host {
            return sibling_index(cur).0;
        }
        cur = parent;
    }
    0
}

fn closest(
    frame: NsRect,
    doms: &[super::DomRect],
    cx: f64,
    cy: f64,
    ch: f64,
) -> Option<(super::DomRect, super::ViewFrame, f64)> {
    let mut best: Option<(super::DomRect, super::ViewFrame, f64)> = None;
    for d in doms {
        let flipped = super::dom_to_view_frame(*d, cx, cy, ch);
        let delta = (frame.origin.x - flipped.x).abs()
            + (frame.origin.y - flipped.y).abs()
            + (frame.size.width - flipped.w).abs()
            + (frame.size.height - flipped.h).abs();
        let replace = best.map(|b| delta < b.2).unwrap_or(true);
        if replace {
            best = Some((*d, flipped, delta));
        }
    }
    best
}

unsafe fn dump_window(window: &tauri::WebviewWindow, label: &str, fact: Option<Fact>) {
    let Ok(raw) = window.ns_window() else {
        eprintln!("GLASS_FR4_DIAG window label={label} window=missing");
        flush_err();
        return;
    };
    let ns_window = raw as *mut Object;
    if ns_window.is_null() {
        eprintln!("GLASS_FR4_DIAG window label={label} window=missing");
        flush_err();
        return;
    }
    let alpha: f64 = msg_send![ns_window, alphaValue];
    let opaque: bool = msg_send![ns_window, isOpaque];
    let bg: *mut Object = msg_send![ns_window, backgroundColor];
    let bg_s = fmt_nscolor(bg);
    let visible: bool = msg_send![ns_window, isVisible];
    let scale: f64 = msg_send![ns_window, backingScaleFactor];
    let win_frame: NsRect = msg_send![ns_window, frame];
    let content: *mut Object = msg_send![ns_window, contentView];
    let content_s = if content.is_null() {
        "null".to_string()
    } else {
        let r: NsRect = msg_send![content, frame];
        fmt_rect(r)
    };
    let host: *mut Object = if content.is_null() {
        std::ptr::null_mut()
    } else {
        let parent: *mut Object = msg_send![content, superview];
        if parent.is_null() { content } else { parent }
    };
    let mut found = Walk {
        effects: Vec::new(),
        wk: std::ptr::null_mut(),
        nodes: 0,
        truncated: false,
    };
    if !host.is_null() {
        walk(host, 0, &mut found);
    }
    let (enabled, light, reduce, reach, frames) = match fact {
        Some(f) => (
            f.enabled.to_string(),
            f.light.to_string(),
            f.reduce.to_string(),
            f.reach_apply.to_string(),
            f.frames.to_string(),
        ),
        None => (
            "missing".to_string(),
            "missing".to_string(),
            "missing".to_string(),
            "missing".to_string(),
            "missing".to_string(),
        ),
    };
    eprintln!(
        "GLASS_FR4_DIAG sync label={label} enabled={enabled} light={light} reduce={reduce} reachApply={reach} frames={frames} effects={} truncated={}",
        found.effects.len(),
        found.truncated
    );
    eprintln!(
        "GLASS_FR4_DIAG window label={label} alpha={alpha:.3} opaque={opaque} bg={bg_s} visible={visible} scale={scale:.3} win={} content={content_s}",
        fmt_rect(win_frame)
    );
    dump_webview(label, found.wk, host);
    dump_siblings(label, host, win_frame.size.width, win_frame.size.height);
    let doms = super::dom_rects_for(label);
    let (cx, cy, ch) = if content.is_null() {
        (0.0, 0.0, 0.0)
    } else {
        let r: NsRect = msg_send![content, frame];
        (r.origin.x, r.origin.y, r.size.height)
    };
    for effect in &found.effects {
        dump_effect(label, *effect, host, scale, &doms, cx, cy, ch, win_frame);
    }
    flush_err();
}

unsafe fn dump_webview(label: &str, wk: *mut Object, host: *mut Object) {
    if wk.is_null() {
        eprintln!("GLASS_FR4_DIAG webview label={label} present=false");
        return;
    }
    let draws_obj: *mut Object = msg_send![wk, valueForKey: ns_string("drawsBackground")];
    let draws: bool = if draws_obj.is_null() {
        false
    } else {
        msg_send![draws_obj, boolValue]
    };
    let under_ok: bool = msg_send![wk, respondsToSelector: sel!(underPageBackgroundColor)];
    let under = if under_ok {
        let color: *mut Object = msg_send![wk, underPageBackgroundColor];
        fmt_nscolor(color)
    } else {
        "unsupported".to_string()
    };
    let (wk_i, wk_n) = sibling_index(wk);
    let wk_sv: *mut Object = msg_send![wk, superview];
    let same = !host.is_null() && wk_sv == host;
    let host_z = if host.is_null() {
        0
    } else {
        host_ancestor_index(wk, host)
    };
    let (wants, op, layer_bg, layer_opaque) = layer_bits(wk);
    eprintln!(
        "GLASS_FR4_DIAG webview label={label} present=true drawsBackground={draws} underPage={under} wkZ={wk_i}/{wk_n} wkSame={same} hostZ={host_z} wantsLayer={wants} layerOpacity={op} layerBg={layer_bg} layerOpaque={layer_opaque}"
    );
}

unsafe fn dump_siblings(label: &str, host: *mut Object, win_w: f64, win_h: f64) {
    if host.is_null() {
        eprintln!("GLASS_FR4_DIAG siblings label={label} n=0");
        return;
    }
    let subs: *mut Object = msg_send![host, subviews];
    if subs.is_null() {
        eprintln!("GLASS_FR4_DIAG siblings label={label} n=0");
        return;
    }
    let count: usize = msg_send![subs, count];
    let show = count.min(40);
    eprintln!("GLASS_FR4_DIAG siblings label={label} n={count} shown={show}");
    for i in 0..show {
        let sub: *mut Object = msg_send![subs, objectAtIndex: i];
        let frame: NsRect = msg_send![sub, frame];
        let hidden: bool = msg_send![sub, isHidden];
        let view_opaque: bool = msg_send![sub, isOpaque];
        let alpha: f64 = msg_send![sub, alphaValue];
        let (wants, op, layer_bg, layer_opaque) = layer_bits(sub);
        let sized = near(frame.size.width, win_w) && near(frame.size.height, win_h);
        eprintln!(
            "GLASS_FR4_DIAG sibling label={label} i={i}/{count} class={} frame={} hidden={hidden} opaque={view_opaque} alpha={alpha:.3} wantsLayer={wants} layerOpacity={op} layerBg={layer_bg} layerOpaque={layer_opaque} windowSized={sized}",
            class_name(sub),
            fmt_rect(frame)
        );
    }
}

unsafe fn dump_effect(
    label: &str,
    effect: *mut Object,
    host: *mut Object,
    scale: f64,
    doms: &[super::DomRect],
    cx: f64,
    cy: f64,
    ch: f64,
    win_frame: NsRect,
) {
    let frame: NsRect = msg_send![effect, frame];
    let (z, zn) = sibling_index(effect);
    let sv: *mut Object = msg_send![effect, superview];
    let host_match = !host.is_null() && sv == host;
    let hidden: bool = msg_send![effect, isHidden];
    let alpha: f64 = msg_send![effect, alphaValue];
    let material: isize = msg_send![effect, material];
    let state: isize = msg_send![effect, state];
    let blending: isize = msg_send![effect, blendingMode];
    let (wants, op, _layer_bg, _layer_opaque) = layer_bits(effect);
    let sized =
        near(frame.size.width, win_frame.size.width) && near(frame.size.height, win_frame.size.height);
    let (dom_s, flip_s, delta_s) = match closest(frame, doms, cx, cy, ch) {
        Some((dom, flipped, delta)) => (
            format!("{:.1},{:.1},{:.1}x{:.1}", dom.x, dom.y, dom.w, dom.h),
            format!("{:.1},{:.1},{:.1}x{:.1}", flipped.x, flipped.y, flipped.w, flipped.h),
            format!("{delta:.1}"),
        ),
        None => ("none".to_string(), "none".to_string(), "na".to_string()),
    };
    eprintln!(
        "GLASS_FR4_DIAG effect label={label} frame={} dom={dom_s} flipped={flip_s} delta={delta_s} scale={scale:.3} z={z}/{zn} hostMatch={host_match} windowSized={sized} material={material} state={state} blending={blending} hidden={hidden} alpha={alpha:.3} wantsLayer={wants} layerOpacity={op} sv={}",
        fmt_rect(frame),
        class_name(sv)
    );
}
