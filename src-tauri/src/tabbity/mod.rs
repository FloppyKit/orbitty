//! Tab claims. Pure checks live here. The claim door and the socket arm
//! call in; this file does not reach other product slices.

use crate::state::{Claim, PersistedState};

pub const URL_CAP: usize = 4096;
pub const TITLE_CAP: usize = 512;
pub const FAVICON_CAP: usize = 1024;
pub const CLAIM_CAP: usize = 500;
pub const BATCH_CAP: usize = 50;

pub fn is_tab_id(id: &str) -> bool {
    id.starts_with("tab:")
}

pub fn is_tab_owned(c: &Claim) -> bool {
    c.tab.is_some() || is_tab_id(&c.window_id)
}

pub fn valid_tab_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("tab:") else {
        return false;
    };
    let b = rest.as_bytes();
    if b.len() != 36 {
        return false;
    }
    let hex = |i: usize| b[i].is_ascii_hexdigit();
    let dash = |i: usize| b[i] == b'-';
    (0..8).all(hex)
        && dash(8)
        && (9..13).all(hex)
        && dash(13)
        && b[14] == b'4'
        && (15..18).all(hex)
        && dash(18)
        && matches!(b[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
        && (20..23).all(hex)
        && dash(23)
        && (24..36).all(hex)
}

pub fn known_browser(browser: &str) -> bool {
    matches!(browser, "chrome" | "brave")
}

/// http(s) only, and only under the cap. Anything else is dropped, not stored.
pub fn favicon(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.len() > FAVICON_CAP {
        return None;
    }
    if s.starts_with("https://") || s.starts_with("http://") {
        Some(s.to_string())
    } else {
        None
    }
}

pub fn check_text(url: &str, title: &str) -> Result<(), String> {
    if url.chars().count() > URL_CAP {
        return Err(format!("url longer than {URL_CAP}"));
    }
    if title.chars().count() > TITLE_CAP {
        return Err(format!("title longer than {TITLE_CAP}"));
    }
    Ok(())
}

pub struct IncomingTab {
    pub id: String,
    pub browser: String,
    pub url: String,
    pub title: String,
    pub fav_icon_url: Option<String>,
}

pub fn tab_count(st: &PersistedState, browser: &str) -> usize {
    st.claims
        .iter()
        .filter(|c| c.tab.as_ref().is_some_and(|t| t.browser == browser))
        .count()
}

/// Url / title / favicon on an existing tab claim. Never projectId.
/// Returns whether the durable row changed.
pub fn apply_update(st: &mut PersistedState, inc: &IncomingTab, now: f64) -> Result<bool, String> {
    if !valid_tab_id(&inc.id) {
        return Err("bad tab id".into());
    }
    check_text(&inc.url, &inc.title)?;
    if !known_browser(&inc.browser) {
        return Err("unknown browser".into());
    }
    let Some(c) = st.claims.iter_mut().find(|c| c.window_id == inc.id) else {
        return Err("unknown tab".into());
    };
    if !is_tab_owned(c) {
        return Err("not a tab claim".into());
    }
    let Some(tab) = c.tab.as_mut() else {
        return Err("not a tab claim".into());
    };
    if tab.browser != inc.browser {
        return Err("wrong browser".into());
    }
    let fav = favicon(inc.fav_icon_url.as_deref());
    let changed = tab.url != inc.url || tab.title != inc.title || tab.fav_icon_url != fav;
    if changed {
        tab.url = inc.url.clone();
        tab.title = inc.title.clone();
        tab.fav_icon_url = fav;
        tab.last_seen_at = now;
        c.last_touched_at = now;
    }
    Ok(changed)
}

pub fn browser_bundle(browser: &str) -> Option<&'static str> {
    match browser {
        "brave" => Some("com.brave.Browser"),
        "chrome" => Some("com.google.Chrome"),
        _ => None,
    }
}

pub fn browser_label(browser: &str) -> &'static str {
    match browser {
        "brave" => "Brave",
        "chrome" => "Chrome",
        _ => "Browser",
    }
}

pub const POLL_SECS: u64 = 20;
const TTL: std::time::Duration = std::time::Duration::from_secs(10);
const STALE: std::time::Duration = std::time::Duration::from_secs(30);
const OUT_CAP: usize = 16;

#[derive(Clone, Debug)]
struct Cmd {
    id: String,
    kind: String,
    claim_id: String,
    url: String,
    at: std::time::Instant,
}

struct BrowserBox {
    last_poll: Option<std::time::Instant>,
    waiting: bool,
    queue: std::collections::VecDeque<Cmd>,
}

struct Outbox {
    boxes: std::collections::HashMap<String, BrowserBox>,
    mem: std::collections::HashMap<String, MemTab>,
}

#[derive(Clone, Debug)]
struct MemTab {
    browser: String,
    url: String,
    title: String,
    live: bool,
}

fn outbox() -> &'static std::sync::Mutex<Outbox> {
    use std::sync::OnceLock;
    static BOX: OnceLock<std::sync::Mutex<Outbox>> = OnceLock::new();
    BOX.get_or_init(|| {
        std::sync::Mutex::new(Outbox {
            boxes: std::collections::HashMap::new(),
            mem: std::collections::HashMap::new(),
        })
    })
}

fn wake() -> &'static std::sync::Condvar {
    use std::sync::OnceLock;
    static CV: OnceLock<std::sync::Condvar> = OnceLock::new();
    CV.get_or_init(std::sync::Condvar::new)
}

fn box_mut<'a>(ob: &'a mut Outbox, browser: &str) -> &'a mut BrowserBox {
    ob.boxes.entry(browser.to_string()).or_insert_with(|| BrowserBox {
        last_poll: None,
        waiting: false,
        queue: std::collections::VecDeque::new(),
    })
}

fn connected(b: &BrowserBox) -> bool {
    b.waiting || b.last_poll.is_some_and(|t| t.elapsed() < STALE)
}

pub fn remember_claimed(spec: &IncomingTab) {
    if let Ok(mut ob) = outbox().lock() {
        ob.mem.insert(
            spec.id.clone(),
            MemTab {
                browser: spec.browser.clone(),
                url: spec.url.clone(),
                title: spec.title.clone(),
                live: true,
            },
        );
    }
}

fn remember_state(id: &str, browser: &str, url: &str, title: &str, live: bool) {
    if let Ok(mut ob) = outbox().lock() {
        let entry = ob.mem.entry(id.to_string()).or_insert(MemTab {
            browser: browser.to_string(),
            url: url.to_string(),
            title: title.to_string(),
            live,
        });
        entry.browser = browser.to_string();
        entry.url = url.to_string();
        entry.title = title.to_string();
        entry.live = live;
    }
}

pub fn note_unclaimed(id: &str) {
    let browser = outbox().lock().ok().and_then(|ob| ob.mem.get(id).map(|m| m.browser.clone()));
    if let Ok(mut ob) = outbox().lock() {
        ob.mem.remove(id);
    }
    if let Some(browser) = browser {
        let _ = push_cmd(&browser, "unclaimed", id, "");
    }
}

fn push_cmd(browser: &str, kind: &str, claim_id: &str, url: &str) -> Result<(), String> {
    let mut ob = outbox().lock().map_err(|_| "outbox locked".to_string())?;
    let b = box_mut(&mut ob, browser);
    if !connected(b) {
        return Err(format!("{} isn't connected", browser_label(browser)));
    }
    while b.queue.len() >= OUT_CAP {
        b.queue.pop_front();
    }
    let n = b.queue.len() + 1;
    let id = format!(
        "c-{}-{n}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    b.queue.push_back(Cmd {
        id,
        kind: kind.to_string(),
        claim_id: claim_id.to_string(),
        url: url.to_string(),
        at: std::time::Instant::now(),
    });
    drop(ob);
    wake().notify_all();
    Ok(())
}

fn raise_browser(browser: &str) {
    let Some(bundle) = browser_bundle(browser) else {
        return;
    };
    let _ = std::process::Command::new("open")
        .args(["-b", bundle])
        .spawn();
}

/// `Some` when `id` is a tab claim and this call handled it (never AX).
pub fn dispatch_window(id: &str, op: &str) -> Option<Result<(), String>> {
    if !is_tab_id(id) {
        return None;
    }
    Some(match op {
        "focus" | "restore" => enqueue_show(id),
        "close" => enqueue_kind(id, "close"),
        "minimize" | "locate" => Err(
            "a tab claim can't be minimized or located — click the row to focus the tab".into(),
        ),
        _ => Err("unsupported tab action".into()),
    })
}

fn enqueue_kind(id: &str, kind: &str) -> Result<(), String> {
    let mem = outbox()
        .lock()
        .ok()
        .and_then(|ob| ob.mem.get(id).cloned())
        .ok_or_else(|| "that tab isn't claimed".to_string())?;
    push_cmd(&mem.browser, kind, id, &mem.url)
}

fn enqueue_show(id: &str) -> Result<(), String> {
    let mem = outbox()
        .lock()
        .ok()
        .and_then(|ob| ob.mem.get(id).cloned())
        .ok_or_else(|| "that tab isn't claimed".to_string())?;
    let kind = if mem.live { "focus" } else { "reopen" };
    push_cmd(&mem.browser, kind, id, &mem.url)?;
    raise_browser(&mem.browser);
    Ok(())
}

fn take_cmd(browser: &str, timeout: std::time::Duration) -> Option<Cmd> {
    let guard = match outbox().lock() {
        Ok(g) => g,
        Err(_) => return None,
    };
    let (mut guard, result) = wake()
        .wait_timeout_while(guard, timeout, |ob| {
            let b = box_mut(ob, browser);
            b.waiting = true;
            b.last_poll = Some(std::time::Instant::now());
            b.queue.retain(|c| c.at.elapsed() < TTL);
            b.queue.is_empty()
        })
        .unwrap_or_else(|e| e.into_inner());
    let b = box_mut(&mut guard, browser);
    b.waiting = false;
    b.last_poll = Some(std::time::Instant::now());
    b.queue.retain(|c| c.at.elapsed() < TTL);
    let cmd = b.queue.pop_front();
    let _ = result;
    cmd
}

fn cmd_json(c: &Cmd) -> serde_json::Value {
    serde_json::json!({
        "cmdId": c.id,
        "cmd": c.kind,
        "claimId": c.claim_id,
        "url": c.url,
    })
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TabLive {
    pub id: String,
    pub title: String,
    pub browser: String,
    pub live: bool,
}

pub fn live_snapshot() -> Vec<TabLive> {
    let Ok(ob) = outbox().lock() else {
        return Vec::new();
    };
    ob.mem
        .iter()
        .map(|(id, m)| TabLive {
            id: id.clone(),
            title: m.title.clone(),
            browser: m.browser.clone(),
            live: m.live,
        })
        .collect()
}

fn arg_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("missing {key}"))
}

fn parse_spec(v: &serde_json::Value) -> Result<IncomingTab, String> {
    Ok(IncomingTab {
        id: arg_str(v, "id")?,
        browser: arg_str(v, "browser")?,
        url: v.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        title: v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        fav_icon_url: v
            .get("favIconUrl")
            .and_then(|x| x.as_str())
            .map(str::to_string),
    })
}

/// Popup click is the user. The socket caller stays an agent and cannot undo this.
pub fn claim(app: &tauri::AppHandle, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    if let Some(msg) = crate::flags::blocked(app, "tabbity_bridge", "tab claim") {
        return Err(msg);
    }
    let origin = crate::claim_front::ClaimOrigin::User;
    let project_id = arg_str(args, "projectId")?;
    let arr = args
        .get("claims")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing claims".to_string())?;
    if arr.len() > BATCH_CAP {
        return Err("batch too large".into());
    }
    let specs = arr.iter().map(parse_spec).collect::<Result<Vec<_>, _>>()?;
    crate::claim_front::claim_tabs(app, &specs, &project_id, origin)?;
    Ok(serde_json::json!({"ok": true}))
}

fn hello(app: &tauri::AppHandle, browser: &str) -> Result<serde_json::Value, String> {
    if !known_browser(browser) {
        return Err("unknown browser".into());
    }
    let st = crate::state::load_state(app)?;
    let mut claims = Vec::new();
    for c in &st.claims {
        let Some(t) = c.tab.as_ref() else {
            continue;
        };
        if t.browser != browser {
            continue;
        }
        remember_state(&c.window_id, &t.browser, &t.url, &t.title, false);
        claims.push(serde_json::json!({
            "id": c.window_id,
            "browser": t.browser,
            "url": t.url,
            "title": t.title,
            "favIconUrl": t.fav_icon_url,
            "lastSeenAt": t.last_seen_at,
            "projectId": c.project_id,
        }));
    }
    Ok(serde_json::json!({"ok": true, "version": "1", "claims": claims}))
}

fn planets(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    let st = crate::state::load_state(app)?;
    let rows: Vec<serde_json::Value> = st
        .projects
        .iter()
        .map(|p| serde_json::json!({"id": p.id, "name": p.name}))
        .collect();
    Ok(serde_json::Value::Array(rows))
}

fn unclaim(app: &tauri::AppHandle, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let browser = arg_str(args, "browser")?;
    let ids = args
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing ids".to_string())?;
    for id in ids {
        let id = id.as_str().ok_or_else(|| "bad id".to_string())?;
        if !is_tab_id(id) {
            return Err("not a tab claim".into());
        }
        let st = crate::state::load_state(app)?;
        if let Some(c) = st.claims.iter().find(|c| c.window_id == id) {
            if c.tab.as_ref().is_some_and(|t| t.browser != browser) {
                return Err("wrong browser".into());
            }
        }
        crate::claim_front::unclaim_window(app, id)?;
    }
    Ok(serde_json::json!({"ok": true}))
}

fn update(app: &tauri::AppHandle, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let arr = args
        .get("claims")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing claims".to_string())?;
    let mut parsed = Vec::new();
    for v in arr {
        let spec = parse_spec(v)?;
        let live = v.get("live").and_then(|x| x.as_object()).is_some();
        parsed.push((spec, live));
    }
    let mut st = crate::state::load_state(app)?;
    let before = st.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let mut changed = false;
    for (spec, _) in &parsed {
        // Refuse the whole batch before any save if one id is unknown.
        if !st.claims.iter().any(|c| c.window_id == spec.id && is_tab_owned(c)) {
            return Err("unknown tab".into());
        }
        if apply_update(&mut st, spec, now)? {
            changed = true;
        }
    }
    if changed {
        crate::board::save_with_events(app, &before, &st)?;
    }
    for (spec, live) in &parsed {
        remember_state(&spec.id, &spec.browser, &spec.url, &spec.title, *live);
    }
    Ok(serde_json::json!({"ok": true, "saved": changed}))
}

fn poll(app: &tauri::AppHandle, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let browser = arg_str(args, "browser")?;
    if !known_browser(&browser) {
        return Err("unknown browser".into());
    }
    let cmd = take_cmd(&browser, std::time::Duration::from_secs(POLL_SECS));
    let st = crate::state::load_state(app)?;
    let claim_ids: Vec<&str> = st
        .claims
        .iter()
        .filter(|c| c.tab.as_ref().is_some_and(|t| t.browser == browser))
        .map(|c| c.window_id.as_str())
        .collect();
    let commands: Vec<serde_json::Value> = cmd.iter().map(cmd_json).collect();
    Ok(serde_json::json!({"commands": commands, "claimIds": claim_ids}))
}

fn ack(app: &tauri::AppHandle, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let ok = args.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let text = args
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("tab command failed");
        use tauri::Emitter;
        let _ = app.emit(
            "tabbity:note",
            serde_json::json!({"kind": "err", "text": text}),
        );
    }
    Ok(serde_json::json!({"ok": true}))
}

#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub fn reset_for_test() {
    if let Ok(mut ob) = outbox().lock() {
        ob.boxes.clear();
        ob.mem.clear();
    }
}

#[cfg(test)]
pub fn mark_connected(browser: &str) {
    let mut ob = outbox().lock().expect("box");
    let b = box_mut(&mut ob, browser);
    b.last_poll = Some(std::time::Instant::now());
    b.waiting = false;
}

#[cfg(test)]
pub fn queued_kinds(browser: &str) -> Vec<String> {
    let mut ob = outbox().lock().expect("box");
    let b = box_mut(&mut ob, browser);
    b.queue.iter().map(|c| c.kind.clone()).collect()
}

#[cfg(test)]
pub fn poll_kind(browser: &str, timeout: std::time::Duration) -> Option<String> {
    take_cmd(browser, timeout).map(|c| c.kind)
}

#[cfg(test)]
pub fn push_stale(browser: &str) {
    let mut ob = outbox().lock().expect("box");
    let b = box_mut(&mut ob, browser);
    b.last_poll = Some(std::time::Instant::now());
    b.queue.push_back(Cmd {
        id: "old".into(),
        kind: "focus".into(),
        claim_id: "tab:stale".into(),
        url: "https://stale.example".into(),
        at: std::time::Instant::now() - std::time::Duration::from_secs(11),
    });
}

#[cfg_attr(not(feature = "dev-thin"), allow(dead_code))]
pub fn handle(
    app: &tauri::AppHandle,
    tool: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match tool {
        "tabbity.hello" => hello(app, &arg_str(args, "browser")?),
        "tabbity.planets" => planets(app),
        "tabbity.claim" => claim(app, args),
        "tabbity.unclaim" => unclaim(app, args),
        "tabbity.update" => update(app, args),
        "tabbity.poll" => poll(app, args),
        "tabbity.ack" => ack(app, args),
        other => Err(format!("unknown tool {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{merge_window_save, TabRef};

    fn tab(url: &str) -> TabRef {
        TabRef {
            browser: "brave".into(),
            url: url.into(),
            title: "t".into(),
            fav_icon_url: None,
            last_seen_at: 1.0,
        }
    }

    fn claim(id: &str, project: &str, tab: Option<TabRef>) -> Claim {
        serde_json::from_value(serde_json::json!({
            "windowId": id,
            "projectId": project,
            "tab": tab,
        }))
        .unwrap()
    }

    #[test]
    fn old_save_loads_without_tab() {
        let c: Claim = serde_json::from_str(r#"{"windowId":"w","projectId":"p"}"#).unwrap();
        assert!(c.tab.is_none());
        assert!(!is_tab_owned(&c));
        let raw = serde_json::to_string(&c).unwrap();
        assert!(!raw.contains("\"tab\""));
    }

    #[test]
    fn window_save_without_tab_keeps_disk() {
        let mut disk = PersistedState::default();
        disk.claims.push(claim(
            "tab:11111111-1111-4111-8111-111111111111",
            "p",
            Some(tab("https://kept.example")),
        ));
        let mut incoming = disk.clone();
        incoming.claims[0].tab = None;
        let (out, _) = merge_window_save(&disk, &incoming);
        assert_eq!(
            out.claims[0].tab.as_ref().map(|t| t.url.as_str()),
            Some("https://kept.example")
        );

        let mut newer = disk.clone();
        newer.claims[0].tab = Some(tab("https://fresh.example"));
        let (out, _) = merge_window_save(&disk, &newer);
        assert_eq!(
            out.claims[0].tab.as_ref().map(|t| t.url.as_str()),
            Some("https://fresh.example")
        );
    }

    #[test]
    fn update_cannot_change_project_and_caps() {
        let mut st = PersistedState::default();
        let id = "tab:11111111-1111-4111-8111-111111111111";
        st.claims.push(claim(id, "planet-a", Some(tab("https://a.example"))));
        let project_before = st.claims[0].project_id.clone();
        let changed = apply_update(
            &mut st,
            &IncomingTab {
                id: id.into(),
                browser: "brave".into(),
                url: "https://b.example".into(),
                title: "next".into(),
                fav_icon_url: Some("https://icon.example/i.png".into()),
            },
            9.0,
        )
        .unwrap();
        assert!(changed);
        assert_eq!(st.claims[0].project_id, project_before);
        assert_eq!(st.claims[0].tab.as_ref().unwrap().url, "https://b.example");
        assert!(apply_update(
            &mut st,
            &IncomingTab {
                id: "tab:nope".into(),
                browser: "brave".into(),
                url: "https://x".into(),
                title: "t".into(),
                fav_icon_url: None,
            },
            1.0,
        )
        .is_err());
        assert!(check_text(&"u".repeat(URL_CAP + 1), "t").is_err());
        assert!(check_text("https://ok", &"t".repeat(TITLE_CAP + 1)).is_err());
        assert!(favicon(Some("chrome://favicon/x")).is_none());
        assert!(favicon(Some("https://ok.example/a.png")).is_some());
        assert!(!valid_tab_id("tab:not-a-uuid"));
        assert!(valid_tab_id(id));
    }

    #[test]
    fn wake_rematch_guard_is_before_bundle_pick() {
        let src = include_str!("../coordinator/soul_wake.rs");
        let wake = src.find("fn wake_sleeper_inner").expect("wake");
        let body = &src[wake..];
        let open = body.find("if !crate::tabbity::is_tab_owned").expect("open");
        let step2 = body.find("// 2. File sleeper").expect("step2");
        assert!(open < step2);
        let early = body.find("restore_window(id)").expect("early restore");
        assert!(early < body.find("// 1. Live window").expect("step1"));
        let mut c = claim("tab:11111111-1111-4111-8111-111111111111", "p", Some(tab("https://a")));
        c.bundle_id = Some("com.brave.Browser".into());
        assert!(is_tab_owned(&c));
    }

    #[test]
    fn restore_enqueues_focus_and_does_not_say_bad_window_id() {
        let _hold = super::test_lock();
        super::reset_for_test();
        let id = "tab:11111111-1111-4111-8111-111111111111";
        super::mark_connected("brave");
        super::remember_claimed(&IncomingTab {
            id: id.into(),
            browser: "brave".into(),
            url: "https://example.test".into(),
            title: "Ex".into(),
            fav_icon_url: None,
        });
        let err = crate::windows::restore_window(id);
        assert!(err.is_ok(), "{err:?}");
        assert_eq!(super::queued_kinds("brave"), vec!["focus".to_string()]);
        let asleep = "tab:22222222-2222-4222-8222-222222222222";
        super::remember_state(asleep, "brave", "https://sleep.example", "S", false);
        assert!(crate::windows::restore_window(asleep).is_ok());
        assert!(super::queued_kinds("brave").contains(&"reopen".to_string()));
        let min = crate::windows::minimize_window(id).unwrap_err();
        assert!(!min.contains("bad window id"), "{min}");
        let src = include_str!("../windows/windows.rs");
        let body = src
            .split("pub fn restore_window")
            .nth(1)
            .expect("restore");
        let dispatch = body.find("dispatch_window").expect("dispatch");
        let platform = body.find("platform::restore_window").expect("platform");
        assert!(dispatch < platform);
    }

    #[test]
    fn outbox_poll_ttl_routing_and_not_connected() {
        let _hold = super::test_lock();
        super::reset_for_test();
        let missing = crate::windows::restore_window("tab:33333333-3333-4333-8333-333333333333");
        let msg = missing.unwrap_err();
        assert!(msg.contains("isn't connected") || msg.contains("isn't claimed"), "{msg}");
        super::mark_connected("chrome");
        super::remember_claimed(&IncomingTab {
            id: "tab:44444444-4444-4444-8444-444444444444".into(),
            browser: "chrome".into(),
            url: "https://c.example".into(),
            title: "C".into(),
            fav_icon_url: None,
        });
        assert!(crate::windows::restore_window("tab:44444444-4444-4444-8444-444444444444").is_ok());
        assert!(super::poll_kind("brave", std::time::Duration::from_millis(30)).is_none());
        assert_eq!(
            super::poll_kind("chrome", std::time::Duration::from_millis(200)).as_deref(),
            Some("focus")
        );
        super::push_stale("chrome");
        assert!(super::poll_kind("chrome", std::time::Duration::from_millis(40)).is_none());
        let started = std::time::Instant::now();
        let handle = std::thread::spawn(|| super::poll_kind("brave", std::time::Duration::from_secs(2)));
        std::thread::sleep(std::time::Duration::from_millis(40));
        super::mark_connected("brave");
        super::remember_claimed(&IncomingTab {
            id: "tab:55555555-5555-4555-8555-555555555555".into(),
            browser: "brave".into(),
            url: "https://wake.example".into(),
            title: "W".into(),
            fav_icon_url: None,
        });
        assert!(crate::windows::close_window("tab:55555555-5555-4555-8555-555555555555").is_ok());
        assert_eq!(handle.join().unwrap().as_deref(), Some("close"));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(POLL_SECS, 20);
    }

    #[test]
    fn user_origin_only_inside_claim_and_fence() {
        let src = include_str!("mod.rs");
        let src = src.split("mod tests").next().expect("prod");
        let claim_at = src.find("pub fn claim(").expect("claim");
        let next = src[claim_at + 1..]
            .find("\npub fn ")
            .map(|i| claim_at + 1 + i)
            .unwrap_or(src.len());
        let claim_fn = &src[claim_at..next];
        assert!(claim_fn.contains("ClaimOrigin::User"));
        let rest = format!("{}{}", &src[..claim_at], &src[next..]);
        assert!(!rest.contains("ClaimOrigin::User"));
        let win = include_str!("../windows/windows.rs");
        assert!(!win.contains("ClaimOrigin::User"));
    }
}
