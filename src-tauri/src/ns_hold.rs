//! One string helper for AppKit keys and titles.
//!
//! A repeated constant returns the same pointer. A title, path, or caption
//! is released when the holder drops. No per-call `mem::forget`.

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::sync::Mutex;

const CONSTANTS: &[&str] = &[
    "",
    "NSScreenNumber",
    "NSFont",
    "NSColor",
    "NSShadow",
    "NSCalibratedRGBColorSpace",
    "NSImageHintInterpolation",
    "drawsBackground",
    "AppleActionOnDoubleClick",
    "new track",
    "Boombox music",
    "Assign to planet",
];

pub struct NsHold {
    ptr: *mut Object,
    release: bool,
}

// The pointer is a CoreFoundation ref. Holders stay on the thread that made them.
unsafe impl Send for NsHold {}

impl Drop for NsHold {
    fn drop(&mut self) {
        if self.release && !self.ptr.is_null() {
            unsafe {
                let _: () = objc::msg_send![self.ptr, release];
            }
        }
    }
}

impl NsHold {
    pub fn as_ptr(&self) -> *mut Object {
        self.ptr
    }
}

/// Constant keys are created once. Anything else is released with this value.
pub unsafe fn ns_string(s: &str) -> NsHold {
    if CONSTANTS.contains(&s) {
        return NsHold {
            ptr: intern(s),
            release: false,
        };
    }
    let cf = CFString::new(s);
    let ptr = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    NsHold {
        ptr,
        release: true,
    }
}

fn intern(s: &str) -> *mut Object {
    static CACHE: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(p) = map.get(s) {
        return *p as *mut Object;
    }
    let cf = CFString::new(s);
    let ptr = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    map.insert(s.to_string(), ptr as usize);
    ptr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_number_key_is_stable() {
        unsafe {
            let first = ns_string("NSScreenNumber").as_ptr();
            assert!(!first.is_null());
            for _ in 0..1000 {
                assert_eq!(ns_string("NSScreenNumber").as_ptr(), first);
            }
        }
    }
}
