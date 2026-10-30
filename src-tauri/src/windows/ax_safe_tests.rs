use super::super::WindowInfo;

struct ForceAx {
    prev: Option<bool>,
}

impl ForceAx {
    fn set(on: bool) -> Self {
        Self {
            prev: crate::flags::force_ax_safe(Some(on)),
        }
    }
}

impl Drop for ForceAx {
    fn drop(&mut self) {
        crate::flags::force_ax_safe(self.prev);
    }
}

fn win(bundle: Option<&str>, name: &str, pid: i32) -> WindowInfo {
    WindowInfo {
        id: format!("{pid}:1"),
        pid,
        app_name: name.to_string(),
        title: String::new(),
        bundle_id: bundle.map(str::to_string),
        stable_key: String::new(),
        icon_base64: None,
        is_frontmost: true,
        is_minimized: false,
        document_path: None,
    }
}

fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src.find(name).unwrap_or_else(|| panic!("missing {name}"));
    let rest = &src[start..];
    let open = rest
        .find('{')
        .unwrap_or_else(|| panic!("no brace in {name}"));
    let open_at = start + open;
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    let mut i = open_at;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open_at + 1..i];
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unbalanced {name}");
}

fn create_call_sites(src: &str) -> Vec<usize> {
    let needle = "AXUIElementCreateApplication(";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = src[from..].find(needle) {
        let at = from + i;
        let line_start = src[..at].rfind('\n').map(|n| n + 1).unwrap_or(0);
        let line_end = src[at..].find('\n').map(|n| at + n).unwrap_or(src.len());
        // The extern declaration is not a call site.
        if !src[line_start..line_end].contains("fn AXUIElementCreateApplication") {
            out.push(at);
        }
        from = at + needle.len();
    }
    out
}

fn fn_at(src: &str, at: usize) -> usize {
    let head = &src[..at];
    let mut best = 0usize;
    for (i, _) in head.match_indices('\n') {
        let line = &head[i + 1..];
        let t = line.trim_start();
        let is_fn = t.starts_with("pub(crate) unsafe fn ")
            || t.starts_with("pub unsafe fn ")
            || t.starts_with("pub(crate) fn ")
            || t.starts_with("pub fn ")
            || t.starts_with("unsafe fn ")
            || t.starts_with("fn ");
        if is_fn {
            best = i;
        }
    }
    best
}

/// A raw create is allowed in `AxApp::for_pid` and in an `else` of `ax_safe_on()`.
fn create_is_allowed(src: &str, at: usize) -> bool {
    let start = fn_at(src, at);
    let sig_end = src[start..].find('{').map(|n| start + n).unwrap_or(at);
    if src[start..sig_end].contains("fn for_pid") {
        return true;
    }
    let until = &src[start..at];
    let Some(else_rel) = until.rfind(" else {") else {
        return false;
    };
    let else_abs = start + else_rel;
    src[start..else_abs].contains("if ax_safe_on()")
}

fn sync_args(body: &str) -> Vec<&str> {
    let needle = "main_thread_sync(";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = body[from..].find(needle) {
        let open = from + i + needle.len() - 1;
        let bytes = body.as_bytes();
        let mut depth = 0usize;
        let mut j = open;
        while j < bytes.len() {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(&body[open + 1..j]);
                        from = j + 1;
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
    }
    out
}

#[test]
fn is_self_app_table() {
    assert!(super::is_self_app(
        Some("space.orbitty.mac"),
        Some("Mail"),
        1,
        2
    ));
    assert!(super::is_self_app(
        Some("space.orbitty.mac.nightly"),
        Some("Mail"),
        1,
        2
    ));
    assert!(super::is_self_app(
        Some("space.orbitty.alpha"),
        Some("Mail"),
        1,
        2
    ));
    assert!(super::is_self_app(
        Some("com.apple.TextEdit"),
        Some("Orbitty Nightly"),
        1,
        2
    ));
    assert!(super::is_self_app(
        Some("com.apple.TextEdit"),
        Some("Orbitty"),
        1,
        2
    ));
    assert!(super::is_self_app(
        Some("com.apple.TextEdit"),
        Some("TextEdit"),
        4,
        4
    ));
    assert!(!super::is_self_app(
        Some("com.apple.TextEdit"),
        Some("TextEdit"),
        5,
        2
    ));
    assert!(!super::is_self_app(
        Some("com.example.Orbittyx"),
        Some("Orbittyx"),
        1,
        2
    ));
}

#[test]
fn axapp_sets_timeout_when_flag_on() {
    let _flag = ForceAx::set(true);
    super::test_seam::clear();
    let app = super::AxApp::for_pid(std::process::id() as i32);
    assert!(
        app.is_some(),
        "AXUIElementCreateApplication returned null for this process"
    );
    assert_eq!(super::AX_TIMEOUT_S, 0.25);
    assert_eq!(super::test_seam::recorded(), vec![super::AX_TIMEOUT_S]);
    let src = include_str!("windows_mac.rs");
    let body = fn_body(src, "fn set_ax_timeout");
    assert!(
        body.contains("AXUIElementSetMessagingTimeout(el, AX_TIMEOUT_S)"),
        "timeout call missing from set_ax_timeout"
    );
    let rt = include_str!("drawers/rt.rs");
    assert!(
        rt.contains("AXUIElementSetMessagingTimeout(ax_app, AX_TIMEOUT_S)"),
        "own-pid site missing the timeout"
    );
    assert!(
        rt.contains("const AX_TIMEOUT_S: f32 = 0.25"),
        "rt timeout drifted from 0.25"
    );
    drop(app);
}

#[test]
fn axapp_no_timeout_when_flag_off() {
    let _flag = ForceAx::set(false);
    super::test_seam::clear();
    let app = super::AxApp::for_pid(std::process::id() as i32);
    assert!(
        app.is_some(),
        "AXUIElementCreateApplication returned null for this process"
    );
    assert!(
        super::test_seam::recorded().is_empty(),
        "flag off recorded {:?}",
        super::test_seam::recorded()
    );
    drop(app);
}

#[test]
fn tracker_closure_has_no_ax() {
    let src = include_str!("windows_mac.rs");
    let body = fn_body(src, "pub fn install_frontmost_tracker");
    let args = sync_args(body);
    assert!(
        args.len() >= 2,
        "tracker should sync the flag-on read and the flag-off path, got {}",
        args.len()
    );
    for arg in &args {
        assert!(
            !arg.contains("ax_copy") && !arg.contains("AXUIElement"),
            "AX inside tracker main_thread_sync closure: {arg}"
        );
        assert!(
            !arg.contains("ax_focused_window_id") && !arg.contains("ax_app_ref"),
            "AX helper inside tracker main_thread_sync closure: {arg}"
        );
    }
    let front = fn_body(src, "fn read_workspace_front");
    assert!(!front.contains("ax_copy") && !front.contains("AXUIElement"));
    assert!(body.contains("read_workspace_front"));
    assert!(body.contains("ax_focused_window_id"));
}

#[test]
fn all_create_sites_use_axapp() {
    let mac = include_str!("windows_mac.rs");
    let ax = include_str!("windows_mac_ax.rs");
    let mac_sites = create_call_sites(mac);
    assert_eq!(
        mac_sites.len(),
        3,
        "expected for_pid, the flag-off helper, and the flag-off tracker body"
    );
    for at in mac_sites {
        assert!(
            create_is_allowed(mac, at),
            "raw create outside for_pid and flag-off arms: {}",
            &mac[at..at + 40]
        );
    }
    assert!(
        create_call_sites(ax).is_empty(),
        "windows_mac_ax.rs still calls AXUIElementCreateApplication"
    );
    assert!(ax.contains("ax_app_ref"));
    assert!(mac.contains("fn for_pid"));
}

#[test]
fn is_orbitty_win_covers_nightly() {
    let _on = ForceAx::set(true);
    assert!(super::is_orbitty_win(&win(
        Some("space.orbitty.mac.nightly"),
        "Mail",
        9
    )));
    assert!(super::is_orbitty_win(&win(
        Some("com.apple.TextEdit"),
        "Orbitty Nightly",
        9
    )));
    assert!(!super::is_orbitty_win(&win(
        Some("com.apple.TextEdit"),
        "TextEdit",
        9
    )));
    assert!(!super::is_orbitty_win(&win(
        Some("com.example.Orbittyx"),
        "Orbittyx",
        9
    )));
    drop(_on);
    let _off = ForceAx::set(false);
    assert!(!super::is_orbitty_win(&win(
        Some("space.orbitty.mac.nightly"),
        "Orbitty Nightly",
        9
    )));
    assert!(super::is_orbitty_win(&win(
        Some("space.orbitty.mac"),
        "TextEdit",
        9
    )));
    assert!(super::is_orbitty_win(&win(
        Some("com.apple.TextEdit"),
        "orbitty",
        9
    )));
}
