//! Loopback 1B mouth sidecar (Harsha constrained decode).
//! Not LM Studio, not Grok, not in the .app. Fail open → keywords.

use std::sync::Mutex;

use serde::Deserialize;
use serde_json::json;

#[cfg(test)]
use std::cell::{Cell, RefCell};

// Test double for the config file and `ORBITTY_MOUTH_SIDECAR_URL`.
// Outer `None` means "read the process". Inner `None` means "absent".
#[cfg(test)]
thread_local! {
    static TEST_ENV: RefCell<Option<Option<String>>> = RefCell::new(None);
    static TEST_FILE: RefCell<Option<Option<String>>> = RefCell::new(None);
    static HTTP_HITS: Cell<u32> = Cell::new(0);
}
/// Per-send ceiling. Slower than this keeps the keyword classifier.
pub const SWAP_IN_MS: f64 = 200.0;
/// Below this P, the pick is noise — keywords own it.
pub const MIN_P: f64 = 0.40;

#[derive(Debug, Clone)]
pub struct Classify {
    pub verb: &'static str,
    pub ack: String,
    pub prob: f64,
    pub elapsed_ms: f64,
    pub harness: Option<crate::mouth_gist::Harness>,
    pub harness_prob: f64,
}

static LAST_HARNESS: Mutex<Option<(crate::mouth_gist::Harness, f64)>> = Mutex::new(None);

pub fn last_harness() -> Option<(crate::mouth_gist::Harness, f64)> {
    *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn clear_harness() {
    *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[derive(Deserialize)]
struct SidecarOut {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    verb: String,
    #[serde(default)]
    ack: String,
    #[serde(default)]
    prob: f64,
    #[serde(default)]
    elapsed_ms: f64,
    #[serde(default)]
    verb_ms: f64,
    #[serde(default)]
    harness: String,
    #[serde(default)]
    harness_prob: f64,
}

pub fn splash_for(ack: &str, verb: &str) -> String {
    match ack.trim() {
        "on_it" => "On it.".into(),
        "heard" => "Heard you.".into(),
        "one_sec" => "One sec.".into(),
        "hand_off" => "Handing that off.".into(),
        "your_call" => "That call is yours.".into(),
        "got_file" => "Got the file.".into(),
        "nope" => "Not that door.".into(),
        _ => match verb {
            "none" => "Heard you.".into(),
            _ => "On it.".into(),
        },
    }
}

fn normalize_verb(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "chat" => Some("chat"),
        "do-go" | "do_go" | "dogo" => Some("do-go"),
        "run-action" | "run_action" => Some("run-action"),
        "ask-needed" | "ask_needed" => Some("ask-needed"),
        "none" => Some("none"),
        _ => None,
    }
}

/// Sidecar base URL when configured, else off. Env wins over the file.
/// Only `http://127.0.0.1:<port>` and `http://localhost:<port>` count;
/// anything else is off (logged once). No HTTP from the caller when this
/// is `None`.
fn enabled_url() -> Option<String> {
    if let Some(raw) = env_url() {
        return accept_loopback(&raw);
    }
    let text = config_text()?;
    let cfg: SidecarFile = serde_json::from_str(&text).ok()?;
    let url = cfg.url.trim();
    if url.is_empty() {
        return None;
    }
    accept_loopback(url)
}

fn env_url() -> Option<String> {
    let raw = env_url_raw()?;
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn env_url_raw() -> Option<String> {
    #[cfg(test)]
    {
        let forced = TEST_ENV.with(|c| c.borrow().clone());
        if let Some(v) = forced {
            return v;
        }
    }
    std::env::var("ORBITTY_MOUTH_SIDECAR_URL").ok()
}

fn config_text() -> Option<String> {
    #[cfg(test)]
    {
        let forced = TEST_FILE.with(|c| c.borrow().clone());
        if let Some(v) = forced {
            return v;
        }
    }
    let home = std::env::var("HOME").ok()?;
    if home.is_empty() {
        return None;
    }
    let path = std::path::PathBuf::from(home).join(".config/orbitty/mouth-sidecar.json");
    std::fs::read_to_string(path).ok()
}

#[derive(Deserialize)]
struct SidecarFile {
    #[serde(default)]
    url: String,
}

/// Loopback only. A trailing slash is fine; a path, query, or other host is not.
fn accept_loopback(raw: &str) -> Option<String> {
    match parse_loopback(raw.trim()) {
        Some(url) => Some(url),
        None => {
            if !raw.trim().is_empty() {
                log_rejected_once(raw.trim());
            }
            None
        }
    }
}

fn parse_loopback(s: &str) -> Option<String> {
    let rest = s.strip_prefix("http://")?;
    if rest.contains('@') || rest.contains('?') || rest.contains('#') {
        return None;
    }
    let (host, port) = rest.split_once(':')?;
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    let port = match port.strip_suffix('/') {
        Some(p) if !p.contains('/') => p,
        Some(_) => return None,
        None if port.contains('/') => return None,
        None => port,
    };
    if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let p: u16 = port.parse().ok()?;
    if p == 0 {
        return None;
    }
    Some(format!("http://{host}:{p}"))
}

/// Replace userinfo with `***` so a refused URL cannot log a password.
/// Everything between `://` (or the start, if there is no scheme) and the
/// last `@` in the authority becomes `***`. URLs with no `@` pass through.
fn redact_userinfo(url: &str) -> String {
    let scheme_end = match url.find("://") {
        Some(i) => i + 3,
        None => 0,
    };
    let rest = &url[scheme_end..];
    let authority_len = rest
        .find(|c: char| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
    let Some(at) = rest[..authority_len].rfind('@') else {
        return url.to_string();
    };
    let mut out = String::with_capacity(url.len());
    out.push_str(&url[..scheme_end]);
    out.push_str("***");
    out.push_str(&url[scheme_end + at..]);
    out
}

fn log_rejected_once(url: &str) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let shown = redact_userinfo(url);
    ONCE.call_once(|| {
        eprintln!("mouth-sidecar: non-loopback url refused ({shown}); sidecar off");
    });
}

/// One classify. None = off, down, slow, low-P, or junk — caller uses keywords.
/// Off (no config / bad url) returns before any HTTP.
pub fn classify(line: &str) -> Option<Classify> {
    let url = enabled_url()?;
    #[cfg(test)]
    HTTP_HITS.with(|n| n.set(n.get() + 1));
    let body = json!({ "line": line });
    let (status, text) =
        crate::mouth_hop::curl_json("POST", &format!("{url}/classify"), None, Some(&body), 1)
            .ok()?;
    if !(200..=299).contains(&status) {
        return None;
    }
    let out: SidecarOut = serde_json::from_str(&text).ok()?;
    if !out.ok {
        return None;
    }
    let verb_ms = if out.verb_ms > 0.0 {
        out.verb_ms
    } else {
        out.elapsed_ms
    };
    if verb_ms > SWAP_IN_MS {
        return None;
    }
    if out.prob < MIN_P {
        return None;
    }
    let verb = normalize_verb(&out.verb)?;
    // Harness is scored after the verb. Total elapsed often lands
    // 270–300ms while verb_ms stays under the 200ms swap-in gate —
    // keep harness on the verb clock so 1B can still break job/pocket.
    let harness = if verb_ms <= SWAP_IN_MS {
        crate::mouth_gist::parse_harness(&out.harness)
    } else {
        None
    };
    let harness_prob = out.harness_prob;
    if let Some(h) = harness {
        *LAST_HARNESS.lock().unwrap_or_else(|e| e.into_inner()) = Some((h, harness_prob));
    }
    Some(Classify {
        verb,
        ack: splash_for(&out.ack, verb),
        prob: out.prob,
        elapsed_ms: out.elapsed_ms,
        harness,
        harness_prob,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_table() {
        assert_eq!(splash_for("on_it", "chat"), "On it.");
        assert_eq!(splash_for("heard", "none"), "Heard you.");
        assert_eq!(splash_for("one_sec", "chat"), "One sec.");
        assert_eq!(splash_for("hand_off", "do-go"), "Handing that off.");
        assert_eq!(splash_for("your_call", "ask-needed"), "That call is yours.");
        assert_eq!(splash_for("got_file", "chat"), "Got the file.");
        assert_eq!(splash_for("nope", "run-action"), "Not that door.");
    }

    #[test]
    fn verb_allowlist() {
        assert_eq!(normalize_verb("CHAT"), Some("chat"));
        assert_eq!(normalize_verb("think"), None);
        assert_eq!(normalize_verb("do-go"), Some("do-go"));
    }

    struct CfgGuard;

    impl CfgGuard {
        fn set(env: Option<&str>, file: Option<&str>) -> Self {
            TEST_ENV.with(|c| *c.borrow_mut() = Some(env.map(str::to_owned)));
            TEST_FILE.with(|c| *c.borrow_mut() = Some(file.map(str::to_owned)));
            HTTP_HITS.with(|n| n.set(0));
            CfgGuard
        }
    }

    impl Drop for CfgGuard {
        fn drop(&mut self) {
            TEST_ENV.with(|c| *c.borrow_mut() = None);
            TEST_FILE.with(|c| *c.borrow_mut() = None);
            HTTP_HITS.with(|n| n.set(0));
        }
    }

    #[test]
    fn absent_file_is_off() {
        let _g = CfgGuard::set(None, None);
        assert!(enabled_url().is_none());
    }

    #[test]
    fn loopback_url_is_on_with_that_url() {
        let _g = CfgGuard::set(
            None,
            Some(r#"{"url":"http://127.0.0.1:13121","weights":"/w"}"#),
        );
        assert_eq!(enabled_url().as_deref(), Some("http://127.0.0.1:13121"));
        drop(_g);
        let _g = CfgGuard::set(None, Some(r#"{"url":"http://localhost:13121/"}"#));
        assert_eq!(enabled_url().as_deref(), Some("http://localhost:13121"));
    }

    #[test]
    fn non_loopback_is_off() {
        let _g = CfgGuard::set(None, Some(r#"{"url":"http://10.1.2.3:13121"}"#));
        assert!(enabled_url().is_none());
        drop(_g);
        let _g = CfgGuard::set(None, Some(r#"{"url":"https://127.0.0.1:13121"}"#));
        assert!(enabled_url().is_none());
    }

    #[test]
    fn env_override_wins() {
        let _g = CfgGuard::set(
            Some("http://127.0.0.1:4242"),
            Some(r#"{"url":"http://localhost:13121"}"#),
        );
        assert_eq!(enabled_url().as_deref(), Some("http://127.0.0.1:4242"));
        drop(_g);
        let _g = CfgGuard::set(
            Some("http://example.com:13121"),
            Some(r#"{"url":"http://127.0.0.1:13121"}"#),
        );
        assert!(enabled_url().is_none());
    }

    #[test]
    fn malformed_json_is_off() {
        let _g = CfgGuard::set(None, Some("{not json"));
        assert!(enabled_url().is_none());
    }

    #[test]
    fn redact_userinfo_hides_password() {
        let out = redact_userinfo("http://user:pw@127.0.0.1:13121");
        assert!(!out.contains("pw"), "{out}");
        assert!(!out.contains("user"), "{out}");
        assert!(out.contains("***@127.0.0.1"), "{out}");
    }

    #[test]
    fn redact_userinfo_leaves_plain_url() {
        let url = "http://127.0.0.1:13121";
        assert_eq!(redact_userinfo(url), url);
    }

    #[test]
    fn redact_userinfo_hides_at_evil() {
        let url = "http://127.0.0.1:13121@evil.com:80";
        let out = redact_userinfo(url);
        assert_ne!(out, url);
        assert!(out.contains("***@evil"), "{out}");
        assert!(!out.contains("127.0.0.1"), "{out}");
        assert_eq!(redact_userinfo("http://@evil"), "http://***@evil");
    }

    #[test]
    fn absent_config_classify_returns_none_without_http() {
        let _g = CfgGuard::set(None, None);
        assert!(classify("ship the gate").is_none());
        let hits = HTTP_HITS.with(|n| n.get());
        assert_eq!(hits, 0, "classify must not HTTP when the sidecar is off");
    }
}
