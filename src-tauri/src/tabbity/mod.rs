//! Tab claims. Pure checks live here. The claim door and the socket arm
//! call in; this file does not reach other product slices.

use crate::state::{Claim, PersistedState, TabRef};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::merge_window_save;

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
        let guard = body.find("is_tab_owned").expect("guard");
        let rematch = body.find("1b.").expect("1b");
        assert!(guard > rematch, "guard wraps 1b, so it is named after the heading");
        let open = body.find("if !crate::tabbity::is_tab_owned").expect("open");
        let step2 = body.find("// 2. File sleeper").expect("step2");
        assert!(open < step2);
        let mut c = claim("tab:11111111-1111-4111-8111-111111111111", "p", Some(tab("https://a")));
        c.bundle_id = Some("com.brave.Browser".into());
        assert!(is_tab_owned(&c));
    }
}
