//! Town PNGs, user-pack decode, being previews (NSImage).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use std::ffi::c_void;
use objc::{class, msg_send, sel, sel_impl};
use tauri::Manager;

use super::geom::NSRect;
use super::{
    app_handle, lock, ns_color, ns_string, set_fill, Habitat, PackFrames,
};

// base64 (icons arrive as PNG base64 from the rail)

pub(crate) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in s.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' {
            continue;
        }
        buf = (buf << 6) | val(b)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// NSImage from encoded bytes — initWithData decodes png/gif/webp alike.
pub(crate) unsafe fn ns_image_from_bytes(bytes: &[u8]) -> *mut Object {
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let img: *mut Object = msg_send![class!(NSImage), alloc];
    let img: *mut Object = msg_send![img, initWithData: data];
    img
}


/// Bitmap rep for pixel dimensions / CGImage slicing. Returns null for
/// formats NSBitmapImageRep can't parse (some webp) — callers fall back to
/// whole-image NSImage then.
pub(crate) unsafe fn bitmap_rep_from_bytes(bytes: &[u8]) -> *mut Object {
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return std::ptr::null_mut();
    }
    msg_send![class!(NSBitmapImageRep), imageRepWithData: data]
}

// user-pack frames (GO-LS-TAURI-USER-PACKS)

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    // Crop in pixel space, origin top-left as displayed — exactly how sprite
    // sheets are laid out (row-major from the top).
    fn CGImageCreateWithImageInRect(image: *const c_void, rect: NSRect) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Called by pack import/remove (any thread): the next sync reloads.
pub fn reload_pack_frames() {
    lock().packs_loaded = false;
}

/// load the bundled town PNGs (Resources/town/*)
/// once into retained NSImages — same bundle mapping as the spirit, but
/// drawn by the habitat painter, not the webview. A missing file stays 0
/// so the glyph fallback draws instead of re-reading disk every frame.
/// Runs on the main thread inside drawRect (all AppKit stays main-thread).
pub(crate) fn ensure_town_art(h: &mut Habitat) {
    if h.town_art_loaded {
        return;
    }
    let Some(app) = app_handle() else {
        return;
    };
    let Ok(dir) = app.path().resource_dir() else {
        return;
    };
    h.town_art_loaded = true;
    let dir = dir.join("town");
    for (name, slot) in [
        ("igloo.png", &mut h.town_igloo),
        ("sign.png", &mut h.town_sign),
        ("boombox.png", &mut h.town_boombox),
        ("beachball.png", &mut h.town_ball),
    ] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            let img = unsafe { ns_image_from_bytes(&bytes) };
            *slot = img as usize;
        }
    }
}

/// Release any decoded pack images and re-enumerate the roster from
/// packs/*/ on disk — PATHS ONLY (GO-ORBITTY-PLANET-BUDGET): no bitmap
/// decodes here. A pack decodes when an expanded planet first wears it
/// (ensure_pack_loaded) and drops when the last expanded wearer collapses
/// (unload_dead_packs). Runs on the main thread inside do_sync (all AppKit
/// stays main-thread).
pub(crate) fn load_pack_frames(h: &mut Habitat) {
    for (_, pack) in h.pack_frames.drain(..) {
        for img in pack.walk.into_iter().chain(pack.idle) {
            unsafe {
                let _: () = msg_send![img as *mut Object, release];
            }
        }
    }
    let Some(app) = app_handle() else {
        return;
    };
    for (id, art) in crate::packs::pack_art(app) {
        h.pack_frames.push((
            id,
            PackFrames {
                walk: Vec::new(),
                idle: Vec::new(),
                art,
                loaded: false,
            },
        ));
    }
}

/// decode a pack's frames on first expanded
/// wear. Idempotent; a failed decode stays failed (walk empty → the
/// generated being draws) instead of re-reading disk every sync.
pub(crate) fn ensure_pack_loaded(h: &mut Habitat, idx: usize) {
    let Some((_, pf)) = h.pack_frames.get_mut(idx) else {
        return;
    };
    if pf.loaded {
        return;
    }
    pf.loaded = true;
    let walk = unsafe { load_source(&pf.art.walk) };
    let idle = match pf.art.idle.take() {
        Some(src) => unsafe { load_source(&src) },
        None => Vec::new(),
    };
    pf.walk = walk;
    pf.idle = idle;
}

/// drop a decoded pack's bitmaps once no
/// expanded planet wears it (the last wearer collapsed or left). The art
/// paths stay, so a re-expand re-decodes. Cheap: a per-sync scan of the
/// expanded walkers' pack indices.
pub(crate) fn unload_dead_packs(h: &mut Habitat) {
    let mut used: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for p in h.planets.values() {
        if p.is_marble {
            continue;
        }
        for b in &p.walkers {
            if let Some(idx) = b.pack {
                used.insert(idx);
            }
        }
    }
    for (idx, (_, pf)) in h.pack_frames.iter_mut().enumerate() {
        if pf.loaded && !used.contains(&idx) {
            for img in pf.walk.drain(..).chain(pf.idle.drain(..)) {
                unsafe {
                    let _: () = msg_send![img as *mut Object, release];
                }
            }
            pf.loaded = false;
        }
    }
}

unsafe fn load_source(src: &crate::packs::ArtSource) -> Vec<usize> {
    match src {
        crate::packs::ArtSource::Frames(paths) => load_frame_files(paths),
        crate::packs::ArtSource::Sheet { file, spec } => slice_sheet(file, spec),
    }
}

/// A walker's frame cap — the cycle never uses more than 8.
const MAX_WALK_FRAMES: usize = 8;

/// Load image files as walker frames. A horizontal strip sheet (width an
/// exact multiple of height — the classic itch layout, e.g. Hero Knight's
/// 1440×180 Run.png) is sliced into ≤8 square frames so it actually walks;
/// anything else is one static frame. Stops at MAX_WALK_FRAMES total.
unsafe fn load_frame_files(paths: &[std::path::PathBuf]) -> Vec<usize> {
    let mut out = Vec::new();
    for p in paths {
        if out.len() >= MAX_WALK_FRAMES {
            break;
        }
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let rep = bitmap_rep_from_bytes(&bytes);
        if !rep.is_null() {
            let width: isize = msg_send![rep, pixelsWide];
            let height: isize = msg_send![rep, pixelsHigh];
            if height > 0 && width > height && width % height == 0 {
                let remaining = (MAX_WALK_FRAMES - out.len()) as isize;
                let n = (width / height).min(remaining);
                out.extend(slice_rep_frames(rep, height, height, n));
                continue;
            }
        }
        let img = ns_image_from_bytes(&bytes);
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}

/// Slice a sprite sheet into retained NSImages, row-major from the top-left,
/// at most `spec.count` frames (fewer when the bitmap is short).
unsafe fn slice_sheet(path: &std::path::Path, spec: &crate::packs::SheetSpec) -> Vec<usize> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let rep = bitmap_rep_from_bytes(&bytes);
    if rep.is_null() {
        return Vec::new();
    }
    let fw = spec.frame_w as isize;
    let fh = spec.frame_h as isize;
    let width: isize = msg_send![rep, pixelsWide];
    let height: isize = msg_send![rep, pixelsHigh];
    if fw <= 0 || fh <= 0 || width < fw || height < fh {
        return Vec::new();
    }
    slice_rep_frames(rep, fw, fh, spec.count as isize)
}

/// Cut `total` frames of fw×fh out of a bitmap rep, row-major from the
/// top-left (fewer when the bitmap is short).
pub(crate) unsafe fn slice_rep_frames(rep: *mut Object, fw: isize, fh: isize, total: isize) -> Vec<usize> {
    let cg: *const c_void = msg_send![rep, CGImage];
    if cg.is_null() || fw <= 0 || fh <= 0 {
        return Vec::new();
    }
    let width: isize = msg_send![rep, pixelsWide];
    let height: isize = msg_send![rep, pixelsHigh];
    let cols = (width / fw).max(1);
    let rows = (height / fh).max(1);
    let total = total.min(cols * rows).max(0);
    let mut out = Vec::new();
    for i in 0..total {
        let x = (i % cols) * fw;
        let y = (i / cols) * fh;
        let sub = CGImageCreateWithImageInRect(
            cg,
            NSRect::new(x as f64, y as f64, fw as f64, fh as f64),
        );
        if sub.is_null() {
            continue;
        }
        let frame_rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
        let frame_rep: *mut Object = msg_send![frame_rep, initWithCGImage: sub];
        CFRelease(sub);
        if frame_rep.is_null() {
            continue;
        }
        let img: *mut Object = msg_send![class!(NSImage), alloc];
        let img: *mut Object = msg_send![img, init];
        let _: () = msg_send![img, addRepresentation: frame_rep];
        let _: () = msg_send![frame_rep, release];
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}


// being-editor preview (GO-ORBITTY-CHARS)

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

pub fn char_preview(look: crate::chars::CharLook, px: u32) -> Result<String, String> {
    let _ = px;
    if let Some(bytes) = bible_preview_png(&look) {
        return Ok(base64_encode(&bytes));
    }
    char_preview_grid(look, px)
}

fn bible_preview_png(look: &crate::chars::CharLook) -> Option<Vec<u8>> {
    let home = std::env::var("HOME").ok()?;
    let p = std::path::PathBuf::from(home)
        .join("Library/Application Support/space.orbitty.mac/packs")
        .join(crate::chars::default_pack(look))
        .join("walk_0.png");
    std::fs::read(p).ok()
}

/// Last-resort walker art when the resolved pack id isn't loaded (pack
/// removed between launches, seeding raced the first sync): the default
/// Boy bitty, so the planet never falls through to the glyph grid while
/// any bitty pack exists on disk.
pub(crate) fn fallback_pack_index(h: &Habitat) -> Option<usize> {
    h.pack_frames
        .iter()
        .position(|(id, _)| id == "orbitty-bitty-boy")
}

/// Rasterize a generated person to a base64 PNG for the Being editor's
/// preview — fallback if the Imagine pack is missing.
fn char_preview_grid(look: crate::chars::CharLook, px: u32) -> Result<String, String> {
    let px = px.clamp(2, 16) as usize;
    let width = crate::chars::W * px;
    let height = crate::chars::H * px;
    let grid = crate::chars::grid(&look, 0);
    let pal = crate::chars::palette(&look);
    unsafe {
        // Commands arrive on an IPC worker thread with no pool in place;
        // the AppKit objects below are autoreleased, so bracket the work.
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let out = (|| {
            let rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
            let rep: *mut Object = msg_send![rep,
                initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
                pixelsWide: width as isize
                pixelsHigh: height as isize
                bitsPerSample: 8isize
                samplesPerPixel: 4isize
                hasAlpha: true
                isPlanar: false
                colorSpaceName: ns_string("NSCalibratedRGBColorSpace")
                bytesPerRow: (width * 4) as isize
                bitsPerPixel: 32isize];
            if rep.is_null() {
                return Err("preview: bitmap alloc failed".into());
            }
            let data: *mut u8 = msg_send![rep, bitmapData];
            if data.is_null() {
                let _: () = msg_send![rep, release];
                return Err("preview: no bitmap buffer".into());
            }
            std::ptr::write_bytes(data, 0, width * height * 4);
            let ctx: *mut Object =
                msg_send![class!(NSGraphicsContext), graphicsContextWithBitmapImageRep: rep];
            if ctx.is_null() {
                let _: () = msg_send![rep, release];
                return Err("preview: no graphics context".into());
            }
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: ctx];
            // Bitmap reps are y-up (the world view is flipped): grid row 0 is
            // the person's TOP, so paint it at the highest y.
            for (row, line) in grid.iter().enumerate() {
                for (col, ch) in line.iter().enumerate() {
                    let Some(c) = crate::chars::glyph_color(*ch, &pal) else {
                        continue;
                    };
                    set_fill(ns_color(c.0, c.1, c.2, 1.0));
                    let cell = NSRect::new(
                        col as f64 * px as f64,
                        (crate::chars::H - 1 - row) as f64 * px as f64,
                        px as f64,
                        px as f64,
                    );
                    let _: () = msg_send![class!(NSBezierPath), fillRect: cell];
                }
            }
            let _: () = msg_send![ctx, flushGraphics];
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: std::ptr::null::<Object>()];
            // NSPNGFileType = 4.
            let png: *mut Object =
                msg_send![rep, representationUsingType: 4usize properties: std::ptr::null::<Object>()];
            let out = if png.is_null() {
                Err("preview: PNG encode failed".into())
            } else {
                let len: usize = msg_send![png, length];
                let ptr: *const u8 = msg_send![png, bytes];
                if ptr.is_null() || len == 0 {
                    Err("preview: empty PNG".into())
                } else {
                    Ok(base64_encode(std::slice::from_raw_parts(ptr, len)))
                }
            };
            let _: () = msg_send![rep, release];
            out
        })();
        let _: () = msg_send![pool, drain];
        out
    }
}
