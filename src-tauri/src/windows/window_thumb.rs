//! In-memory window thumbnails for the rail's centred layer-2 card.
//! JPEG bytes never leave this process: not state.json, not logs, not events.

use std::collections::{HashMap, VecDeque};

pub const THUMB_TTL_MS: u64 = 15_000;
pub const THUMB_MIN_INTERVAL_MS: u64 = 2_000;
pub const THUMB_CAP_COUNT: usize = 24;
pub const THUMB_CAP_BYTES: usize = 6 * 1024 * 1024;
pub const THUMB_MAX_PX: i32 = 480;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbPerm {
    Granted,
    #[cfg_attr(not(test), allow(dead_code))]
    Denied,
    #[cfg_attr(not(test), allow(dead_code))]
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbDecision {
    Use,
    Capture,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbMeta {
    pub at_ms: u64,
    pub focus_epoch: u64,
}

#[derive(Clone, Debug)]
pub struct ThumbBlob {
    pub bytes: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub at_ms: u64,
    pub focus_epoch: u64,
}

#[derive(Clone, Debug)]
pub struct ThumbCache {
    order: VecDeque<u32>,
    map: HashMap<u32, ThumbBlob>,
    bytes: usize,
}

impl ThumbCache {
    pub fn new() -> Self {
        Self {
            order: VecDeque::new(),
            map: HashMap::new(),
            bytes: 0,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn byte_len(&self) -> usize {
        self.bytes
    }

    pub fn get(&mut self, id: u32) -> Option<&ThumbBlob> {
        if !self.map.contains_key(&id) {
            return None;
        }
        if let Some(pos) = self.order.iter().position(|k| *k == id) {
            self.order.remove(pos);
        }
        self.order.push_back(id);
        self.map.get(&id)
    }

    pub fn meta(&self, id: u32) -> Option<ThumbMeta> {
        self.map.get(&id).map(|b| ThumbMeta {
            at_ms: b.at_ms,
            focus_epoch: b.focus_epoch,
        })
    }

    pub fn insert(&mut self, id: u32, blob: ThumbBlob) {
        if let Some(old) = self.map.remove(&id) {
            self.bytes = self.bytes.saturating_sub(old.bytes.len());
            if let Some(pos) = self.order.iter().position(|k| *k == id) {
                self.order.remove(pos);
            }
        }
        self.bytes += blob.bytes.len();
        self.map.insert(id, blob);
        self.order.push_back(id);
        self.evict();
    }

    fn evict(&mut self) {
        while self.map.len() > THUMB_CAP_COUNT || self.bytes > THUMB_CAP_BYTES {
            let Some(old_id) = self.order.pop_front() else {
                break;
            };
            if let Some(old) = self.map.remove(&old_id) {
                self.bytes = self.bytes.saturating_sub(old.bytes.len());
            }
        }
    }
}

impl Default for ThumbCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Capture when missing, stale while centred (TTL), or the focus epoch moved.
/// One capture in flight. At least `THUMB_MIN_INTERVAL_MS` between captures
/// of the same window. Denied / unsupported never capture.
pub fn thumb_policy(
    now: u64,
    entry: Option<ThumbMeta>,
    permission: ThumbPerm,
    in_flight: bool,
    focus_epoch: u64,
) -> ThumbDecision {
    if permission != ThumbPerm::Granted {
        return ThumbDecision::Fallback;
    }
    if in_flight {
        return if entry.is_some() {
            ThumbDecision::Use
        } else {
            ThumbDecision::Fallback
        };
    }
    let Some(entry) = entry else {
        return ThumbDecision::Capture;
    };
    let ttl = now.saturating_sub(entry.at_ms) >= THUMB_TTL_MS;
    let focus = entry.focus_epoch != focus_epoch;
    if !ttl && !focus {
        return ThumbDecision::Use;
    }
    if now.saturating_sub(entry.at_ms) < THUMB_MIN_INTERVAL_MS {
        return ThumbDecision::Use;
    }
    ThumbDecision::Capture
}

/// `pid:123` → 123. Title ids (`pid:t:…`) have no CG window id.
pub fn parse_cg_window_id(id: &str) -> Option<u32> {
    let rest = id.split_once(':')?.1;
    if rest.is_empty() || rest.starts_with('t') {
        return None;
    }
    rest.parse().ok()
}

pub fn skip_request_env() -> bool {
    std::env::var("ORBITTY_THUMB_SKIP_REQUEST").ok().as_deref() == Some("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(n: usize, at: u64) -> ThumbBlob {
        ThumbBlob {
            bytes: vec![1u8; n],
            w: 8,
            h: 8,
            at_ms: at,
            focus_epoch: 1,
        }
    }

    #[test]
    fn cache_lru_evicts_oldest() {
        let mut c = ThumbCache::new();
        for i in 0..THUMB_CAP_COUNT + 1 {
            c.insert(i as u32, blob(16, i as u64));
        }
        assert_eq!(c.len(), THUMB_CAP_COUNT);
        assert!(c.meta(0).is_none(), "oldest evicted");
        assert!(c.meta(1).is_some());
        assert!(c.meta(THUMB_CAP_COUNT as u32).is_some());
    }

    #[test]
    fn cache_byte_cap() {
        let mut c = ThumbCache::new();
        let chunk = 1024 * 1024;
        for i in 0..8 {
            c.insert(i, blob(chunk, i as u64));
        }
        assert!(c.byte_len() <= THUMB_CAP_BYTES);
        assert!(c.len() < 8);
        assert!(c.meta(7).is_some());
    }

    #[test]
    fn policy_ttl_refresh() {
        let now = 100_000u64;
        let fresh = ThumbMeta {
            at_ms: now - THUMB_TTL_MS + 1,
            focus_epoch: 3,
        };
        assert_eq!(
            thumb_policy(now, Some(fresh), ThumbPerm::Granted, false, 3),
            ThumbDecision::Use
        );
        let stale = ThumbMeta {
            at_ms: now - THUMB_TTL_MS,
            focus_epoch: 3,
        };
        assert_eq!(
            thumb_policy(now, Some(stale), ThumbPerm::Granted, false, 3),
            ThumbDecision::Capture
        );
    }

    #[test]
    fn policy_focus_epoch_refresh() {
        let now = 50_000u64;
        let entry = ThumbMeta {
            at_ms: now - 5_000,
            focus_epoch: 1,
        };
        assert_eq!(
            thumb_policy(now, Some(entry), ThumbPerm::Granted, false, 2),
            ThumbDecision::Capture
        );
    }

    #[test]
    fn policy_single_in_flight() {
        let now = 10_000u64;
        assert_eq!(
            thumb_policy(now, None, ThumbPerm::Granted, true, 1),
            ThumbDecision::Fallback
        );
        let entry = ThumbMeta {
            at_ms: 0,
            focus_epoch: 1,
        };
        assert_eq!(
            thumb_policy(now, Some(entry), ThumbPerm::Granted, true, 9),
            ThumbDecision::Use
        );
    }

    #[test]
    fn policy_min_interval() {
        let now = 20_000u64;
        let entry = ThumbMeta {
            at_ms: now - (THUMB_MIN_INTERVAL_MS - 1),
            focus_epoch: 1,
        };
        assert_eq!(
            thumb_policy(now, Some(entry), ThumbPerm::Granted, false, 4),
            ThumbDecision::Use
        );
    }

    #[test]
    fn policy_denied_fallback() {
        assert_eq!(
            thumb_policy(0, None, ThumbPerm::Denied, false, 0),
            ThumbDecision::Fallback
        );
        assert_eq!(
            thumb_policy(0, None, ThumbPerm::Unsupported, false, 0),
            ThumbDecision::Fallback
        );
    }

    #[test]
    fn parse_cg_window_id() {
        assert_eq!(crate::window_thumb::parse_cg_window_id("pid:123"), Some(123));
        assert_eq!(crate::window_thumb::parse_cg_window_id("pid:t:x#1"), None);
        assert_eq!(crate::window_thumb::parse_cg_window_id("nope"), None);
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use super::*;
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    extern "C" {
        fn orbitty_thumb_preflight() -> i32;
        fn orbitty_thumb_request() -> i32;
        fn orbitty_thumb_capture(
            window_id: u32,
            pid: i32,
            title: *const c_char,
            max_px: i32,
            ctx: *mut c_void,
            cb: extern "C" fn(*mut c_void, i32, *const c_char, i32, i32),
        );
    }

    struct WaitSlot {
        tx: Mutex<Option<mpsc::Sender<RawThumb>>>,
    }

    struct RawThumb {
        status: i32,
        b64: String,
        w: i32,
        h: i32,
    }

    extern "C" fn on_thumb(ctx: *mut c_void, status: i32, data: *const c_char, w: i32, h: i32) {
        if ctx.is_null() {
            return;
        }
        let slot = unsafe { std::sync::Arc::from_raw(ctx as *const WaitSlot) };
        let b64 = if data.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(data) }.to_string_lossy().into_owned()
        };
        let tx = slot.tx.lock().ok().and_then(|mut guard| guard.take());
        if let Some(tx) = tx {
            let _ = tx.send(RawThumb { status, b64, w, h });
        }
    }

    fn cache() -> &'static Mutex<ThumbCache> {
        static C: OnceLock<Mutex<ThumbCache>> = OnceLock::new();
        C.get_or_init(|| Mutex::new(ThumbCache::new()))
    }

    fn in_flight() -> &'static std::sync::atomic::AtomicBool {
        static F: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        &F
    }

    fn requested() -> &'static std::sync::atomic::AtomicBool {
        static R: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        &R
    }

    pub fn permission_granted() -> bool {
        unsafe { orbitty_thumb_preflight() == 1 }
    }

    fn status_name(code: i32) -> &'static str {
        match code {
            0 => "ok",
            1 => "denied",
            2 => "not_found",
            3 => "unsupported",
            _ => "error",
        }
    }

    pub fn capture_thumb(id: &str, title: &str, pid: i32, focus_epoch: u64) -> serde_json::Value {
        let our = std::process::id() as i32;
        if pid == our {
            return fallback("not_found");
        }
        if !permission_granted() {
            if !skip_request_env()
                && !requested().swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                unsafe {
                    orbitty_thumb_request();
                }
            }
            eprintln!("orbitty:thumb state=denied request_skipped={}", skip_request_env());
            return fallback("denied");
        }
        let Some(cg) = parse_cg_window_id(id).or_else(|| if pid > 0 { Some(0) } else { None }) else {
            return fallback("not_found");
        };
        let now = now_ms();
        let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
        let meta = if cg == 0 { None } else { guard.meta(cg) };
        let flying = in_flight().load(std::sync::atomic::Ordering::Relaxed);
        let decision = thumb_policy(now, meta, ThumbPerm::Granted, flying, focus_epoch);
        if decision != ThumbDecision::Capture {
            if decision == ThumbDecision::Use {
                if let Some(blob) = guard.get(cg) {
                    let bytes = blob.bytes.clone();
                    let w = blob.w;
                    let h = blob.h;
                    let at = blob.at_ms;
                    return ok_json(&bytes, w, h, at);
                }
            }
            return fallback("fallback");
        }
        drop(guard);
        if in_flight().swap(true, std::sync::atomic::Ordering::Relaxed) {
            return fallback("error");
        }
        let raw = call_capture(cg, pid, title);
        in_flight().store(false, std::sync::atomic::Ordering::Relaxed);
        if raw.status != 0 || raw.b64.is_empty() {
            eprintln!("orbitty:thumb state={}", status_name(raw.status));
            return fallback(status_name(raw.status));
        }
        let bytes = raw.b64.into_bytes();
        let at = now_ms();
        let blob = ThumbBlob {
            w: raw.w.max(0) as u32,
            h: raw.h.max(0) as u32,
            at_ms: at,
            focus_epoch,
            bytes: bytes.clone(),
        };
        if cg != 0 {
            if let Ok(mut g) = cache().lock() {
                g.insert(cg, blob);
            }
        }
        ok_json(&bytes, raw.w.max(0) as u32, raw.h.max(0) as u32, at)
    }

    fn call_capture(cg: u32, pid: i32, title: &str) -> RawThumb {
        let (tx, rx) = mpsc::channel();
        let slot = std::sync::Arc::new(WaitSlot {
            tx: Mutex::new(Some(tx)),
        });
        let ctx = std::sync::Arc::into_raw(slot) as *mut c_void;
        let c_title = CString::new(title).unwrap_or_else(|_| CString::new("").unwrap());
        unsafe {
            orbitty_thumb_capture(cg, pid, c_title.as_ptr(), THUMB_MAX_PX, ctx, on_thumb);
        }
        rx.recv_timeout(Duration::from_secs(2)).unwrap_or(RawThumb {
            status: 4,
            b64: String::new(),
            w: 0,
            h: 0,
        })
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn ok_json(bytes: &[u8], w: u32, h: u32, at_ms: u64) -> serde_json::Value {
        serde_json::json!({
            "state": "ok",
            "jpeg": String::from_utf8_lossy(bytes),
            "w": w,
            "h": h,
            "atMs": at_ms,
        })
    }

    fn fallback(state: &str) -> serde_json::Value {
        serde_json::json!({
            "state": state,
            "jpeg": null,
            "w": 0,
            "h": 0,
            "atMs": 0,
        })
    }
}

#[cfg(target_os = "macos")]
pub fn capture_thumb(id: &str, title: &str, pid: i32, focus_epoch: u64) -> serde_json::Value {
    ffi::capture_thumb(id, title, pid, focus_epoch)
}

#[cfg(not(target_os = "macos"))]
pub fn capture_thumb(id: &str, title: &str, pid: i32, focus_epoch: u64) -> serde_json::Value {
    let _ = (id, title, pid, focus_epoch);
    serde_json::json!({
        "state": "unsupported",
        "jpeg": null,
        "w": 0,
        "h": 0,
        "atMs": 0,
    })
}

#[cfg(target_os = "macos")]
pub fn permission_granted() -> bool {
    ffi::permission_granted()
}

#[cfg(not(target_os = "macos"))]
pub fn permission_granted() -> bool {
    false
}
