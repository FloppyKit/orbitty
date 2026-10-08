//! Cat colorway frames. Paths only; habitat loaders stay crate-private.

use super::{CGSize, CatArt, NSRect, CAT_PACKS};
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use tauri::AppHandle;

// cat art (pack frames; habitat's loaders stay crate-private)

/// NSImage from encoded bytes — initWithData decodes png/gif/webp alike.
/// Returns a retained image (caller releases).
unsafe fn ns_image_from_bytes(bytes: &[u8]) -> *mut Object {
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
    if img.is_null() {
        return std::ptr::null_mut();
    }
    // initWithData can leave NSSizeZero until a rep is queried; drawInRect
    // of a zero-size image is a silent no-op (meows, no cats).
    let size: CGSize = msg_send![img, size];
    if size.width < 1.0 || size.height < 1.0 {
        let reps: *mut Object = msg_send![img, representations];
        let n: usize = if reps.is_null() {
            0
        } else {
            msg_send![reps, count]
        };
        if n > 0 {
            let rep: *mut Object = msg_send![reps, objectAtIndex: 0usize];
            if !rep.is_null() {
                let w: isize = msg_send![rep, pixelsWide];
                let h: isize = msg_send![rep, pixelsHigh];
                if w > 0 && h > 0 {
                    let _: () = msg_send![img, setSize: CGSize {
                        width: w as f64,
                        height: h as f64,
                    }];
                }
            }
        }
    }
    img
}

/// Decode the four yard colorways from the seeded packs dir. Paths only
/// from packs::pack_art, decoded here — a failed file just drops out of
/// its cycle. Empty across all four = no art, and start refuses (never
/// rain invisible cats).
pub(crate) fn load_cat_art(app: &AppHandle) -> Vec<CatArt> {
    let all = crate::packs::pack_art(app);
    let mut out = Vec::new();
    for slug in CAT_PACKS {
        let Some((_, art)) = all.iter().find(|(id, _)| id == slug) else {
            continue;
        };
        let walk = unsafe { load_source(&art.walk) };
        if walk.is_empty() {
            continue;
        }
        let idle = match &art.idle {
            Some(src) => unsafe { load_source(src) },
            None => Vec::new(),
        };
        out.push(CatArt { walk, idle });
    }
    out
}

unsafe fn load_source(src: &crate::packs::ArtSource) -> Vec<usize> {
    match src {
        crate::packs::ArtSource::Frames(paths) => load_frame_files(paths),
        crate::packs::ArtSource::Sheet { file, spec } => slice_sheet(file, spec),
    }
}

const MAX_WALK_FRAMES: usize = 8;

unsafe fn load_frame_files(paths: &[std::path::PathBuf]) -> Vec<usize> {
    let mut out = Vec::new();
    for p in paths {
        if out.len() >= MAX_WALK_FRAMES {
            break;
        }
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let img = ns_image_from_bytes(&bytes);
        if !img.is_null() {
            out.push(img as usize);
        }
    }
    out
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGImageCreateWithImageInRect(image: *const c_void, rect: NSRect) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Bitty packs ship loose frames; the sheet path only exists so an
/// imported sheet-style recolor still walks.
unsafe fn slice_sheet(path: &std::path::Path, spec: &crate::packs::SheetSpec) -> Vec<usize> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let data: *mut Object = msg_send![class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()];
    if data.is_null() {
        return Vec::new();
    }
    let rep: *mut Object = msg_send![class!(NSBitmapImageRep), imageRepWithData: data];
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
    let cg: *const c_void = msg_send![rep, CGImage];
    if cg.is_null() {
        return Vec::new();
    }
    let cols = (width / fw).max(1);
    let rows = (height / fh).max(1);
    let total = (spec.count as isize).min(cols * rows).max(0);
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
