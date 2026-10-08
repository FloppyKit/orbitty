use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    pub(crate) static OPEN_LOG: RefCell<Option<Vec<Vec<String>>>> = const { RefCell::new(None) };
}

/// Test hook. While armed, macOS `open` is not spawned; calls are recorded.
#[cfg(test)]
pub struct OpenGuard;
#[cfg(test)]
impl OpenGuard {
    pub fn arm() -> Self {
        OPEN_LOG.with(|l| *l.borrow_mut() = Some(Vec::new()));
        OpenGuard
    }
    pub fn log() -> Vec<Vec<String>> {
        OPEN_LOG.with(|l| l.borrow().clone().unwrap_or_default())
    }
    pub fn clear() {
        OPEN_LOG.with(|l| {
            if let Some(v) = l.borrow_mut().as_mut() {
                v.clear();
            }
        });
    }
}
#[cfg(test)]
impl Drop for OpenGuard {
    fn drop(&mut self) {
        OPEN_LOG.with(|l| *l.borrow_mut() = None);
    }
}


    use super::*;
    use crate::state::CardLink;
    use std::path::{Path, PathBuf};

    #[test]
    fn rejects_hostile_schemes() {
        for raw in [
            "javascript:alert(1)",
            "JAVASCRIPT:alert(1)",
            "data:text/html,hi",
            "http://example.com",
            "HTTP://example.com",
            "vbscript:msg",
            "blob:https://example.com/x",
        ] {
            let err = normalize_link(raw).unwrap_err();
            assert!(
                err.contains("not allowed") || err.contains("link rejected"),
                "{raw} -> {err}"
            );
            assert!(
                err.contains(ACCEPTED),
                "{raw} error should say what is accepted: {err}"
            );
            let scheme = raw.split(':').next().unwrap().to_ascii_lowercase();
            assert!(
                err.contains(&scheme),
                "{raw} error {err} should name {scheme}"
            );
        }
    }

    #[test]
    fn allows_https_and_file() {
        assert_eq!(
            normalize_link("https://example.com/a?b=1").unwrap(),
            "https://example.com/a?b=1"
        );
        assert_eq!(
            normalize_link("HTTPS://Example.com/Path").unwrap(),
            "https://Example.com/Path"
        );
        let go = "/Users/dude/Orbitty GOs/GO-ORBITTY-CARD-LINKS_2026-09-25.md";
        let want = "file:///Users/dude/Orbitty%20GOs/GO-ORBITTY-CARD-LINKS_2026-09-25.md";
        assert_eq!(normalize_link(go).unwrap(), want);
        assert_eq!(normalize_link(want).unwrap(), want);
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty GOs/x.md").unwrap(),
            "file:///Users/dude/Orbitty%20GOs/x.md"
        );
    }

    #[test]
    fn rejects_relative_and_dotdot() {
        assert!(normalize_link("notes.md").is_err());
        assert!(normalize_link("file://relative").is_err());
        assert!(normalize_link("file:///Users/dude/../../etc/passwd").is_err());
        assert!(normalize_link("https://").is_err());
        assert!(normalize_link("https://user@example.com").is_err());
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = std::env::var("HOME").unwrap();
        let want = format!(
            "file://{}",
            encode_path(&format!("{home}/Orbitty GOs/x.md"))
        );
        assert_eq!(normalize_link("~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(normalize_link("  ~/Orbitty GOs/x.md ").unwrap(), want);
        assert_eq!(normalize_link("file://~/Orbitty GOs/x.md").unwrap(), want);
        assert_eq!(
            normalize_link("~").unwrap(),
            format!("file://{}", encode_path(&home))
        );
        // ~user is not expanded, and the message says what works.
        let err = normalize_link("~root/x").unwrap_err();
        assert!(err.contains(ACCEPTED), "{err}");
        assert!(normalize_link("~/../../etc/passwd").is_err());
    }

    #[test]
    fn file_url_forms_normalize() {
        let want = "file:///Users/dude/Orbitty%20GOs/x.md";
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("file:///Users/dude/Orbitty%20GOs/x.md").unwrap(),
            want
        );
        // Two-slash typo: the root folder landed in the host slot.
        assert_eq!(
            normalize_link("file://Users/dude/Orbitty GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("FILE://Users/dude/Orbitty%20GOs/x.md").unwrap(),
            want
        );
        assert_eq!(
            normalize_link("file://Volumes/Disk/a b.pdf").unwrap(),
            "file:///Volumes/Disk/a%20b.pdf"
        );
        assert_eq!(
            normalize_link("file://localhost/tmp/a.md").unwrap(),
            "file:///tmp/a.md"
        );
        assert_eq!(
            normalize_link("/Users/dude/x.md").unwrap(),
            "file:///Users/dude/x.md"
        );
        // Idempotent on its own output.
        let once = normalize_link("file://Users/dude/Orbitty GOs/x.md").unwrap();
        assert_eq!(normalize_link(&once).unwrap(), once);
        // Still rejected: unknown host, relative, dot-dot via the typo form.
        assert!(normalize_link("file://example.com/x").is_err());
        assert!(normalize_link("file://relative").is_err());
        assert!(normalize_link("file://Users/dude/../../etc/passwd").is_err());
    }

    #[test]
    fn rejections_say_what_is_accepted() {
        for raw in ["notes.md", "file://relative", "ftp://x.org/a", ""] {
            let err = normalize_link(raw).unwrap_err();
            assert!(err.contains(ACCEPTED), "{raw:?} -> {err}");
        }
    }

    #[test]
    fn empty_links_stay_absent() {
        assert_eq!(canonicalize_links(None).unwrap(), None);
        assert_eq!(canonicalize_links(Some(vec![])).unwrap(), None);
    }

    #[test]
    fn old_card_json_omits_links() {
        let raw = r#"{"id":"c1","title":"T","projectId":"p","status":"todo","createdAt":1.0,"lastTouchedAt":1.0,"goalAligned":true,"clinicalOrMaster":false,"pointsAwarded":0}"#;
        let card: crate::state::KanbanCard = serde_json::from_str(raw).unwrap();
        assert!(card.links.is_none());
        let v = serde_json::to_value(&card).unwrap();
        assert!(v.get("links").is_none());
    }

    #[test]
    fn missing_file_is_a_clear_error() {
        let err = open_url("file:///tmp/orbitty-card-links-no-such-file-zz.md").unwrap_err();
        assert_eq!(err, "file not found");
    }

    #[test]
    fn cap_is_eight() {
        let links = (0..9)
            .map(|i| CardLink {
                url: format!("https://example.com/{i}"),
                label: None,
            })
            .collect();
        let err = canonicalize_links(Some(links)).unwrap_err();
        assert!(err.contains("8"));
    }

    fn temp_case(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "orbitty-link-safe-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn set_mode(path: &Path, mode: u32) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(mode);
            std::fs::set_permissions(path, perms).unwrap();
        }
        let _ = (path, mode);
    }

    #[test]
    fn preview_kind_table() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("kinds");
        let md = dir.join("note.md");
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("line {i}\n"));
        }
        std::fs::write(&md, &body).unwrap();
        let folder = dir.join("papers");
        std::fs::create_dir(&folder).unwrap();
        let png = dir.join("dot.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nnot-really").unwrap();
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho hi\n").unwrap();
        set_mode(&cmd, 0o755);
        let cmd_nox = dir.join("plain.command");
        std::fs::write(&cmd_nox, "echo hi\n").unwrap();
        set_mode(&cmd_nox, 0o644);
        let sh = dir.join("run.sh");
        std::fs::write(&sh, "echo hi\n").unwrap();
        set_mode(&sh, 0o644);
        let plus = dir.join("plain.txt");
        std::fs::write(&plus, "text\n").unwrap();
        set_mode(&plus, 0o755);
        let app = dir.join("Demo.app");
        std::fs::create_dir(&app).unwrap();
        let bundle = dir.join("Widget");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), b"<plist/>").unwrap();
        let pdf = dir.join("a.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let big = dir.join("big.md");
        let mut big_body = vec![b'a'; EXCERPT_BYTES + 100];
        big_body.push(b'\n');
        big_body.extend_from_slice(b"TAIL\n");
        std::fs::write(&big, &big_body).unwrap();
        let mut run_exts = Vec::new();
        for ext in [
            "fileloc",
            "inetloc",
            "webloc",
            "jar",
            "mpkg",
            "shortcut",
            "mobileconfig",
            "osax",
            "action",
            "csh",
            "py",
        ] {
            let p = dir.join(format!("sample.{ext}"));
            std::fs::write(&p, b"not a document\n").unwrap();
            set_mode(&p, 0o644);
            run_exts.push(p);
        }

        let mut cases = vec![
            (md.to_string_lossy().to_string(), "document"),
            (folder.to_string_lossy().to_string(), "folder"),
            (png.to_string_lossy().to_string(), "image"),
            (cmd.to_string_lossy().to_string(), "executable"),
            (cmd_nox.to_string_lossy().to_string(), "executable"),
            (sh.to_string_lossy().to_string(), "executable"),
            (plus.to_string_lossy().to_string(), "executable"),
            (app.to_string_lossy().to_string(), "executable"),
            (bundle.to_string_lossy().to_string(), "executable"),
            (pdf.to_string_lossy().to_string(), "document"),
            (
                "/tmp/orbitty-link-safe-no-such-file-zz.md".into(),
                "missing",
            ),
            ("https://example.com/a?b=1".into(), "https"),
        ];
        for p in &run_exts {
            cases.push((p.to_string_lossy().to_string(), "executable"));
        }
        for (raw, kind) in cases {
            let p = preview_link(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(p.kind, kind, "{raw}");
            assert!(
                OpenGuard::log().is_empty(),
                "preview spawned open for {raw}"
            );
        }

        let doc = preview_link(&md.to_string_lossy()).unwrap();
        let excerpt = doc.excerpt.clone().unwrap();
        let lines: Vec<_> = excerpt.lines().collect();
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[0], "line 0");
        assert_eq!(doc.target, md.canonicalize().unwrap().to_string_lossy());
        assert!(doc.size.is_some() && doc.modified.is_some());

        let img = preview_link(&png.to_string_lossy()).unwrap();
        assert!(img.thumbnail.unwrap().starts_with("data:image/png;base64,"));

        let exe = preview_link(&cmd.to_string_lossy()).unwrap();
        assert_eq!(exe.warning.as_deref(), Some(RUN_WARNING));
        assert!(exe.excerpt.is_none());
        assert!(exe.content_sha256.is_some());
        let nox = preview_link(&cmd_nox.to_string_lossy()).unwrap();
        assert_eq!(nox.kind, "executable");
        assert_eq!(nox.warning.as_deref(), Some(RUN_WARNING));
        for p in &run_exts {
            let row = preview_link(&p.to_string_lossy()).unwrap();
            assert_eq!(row.warning.as_deref(), Some(RUN_WARNING), "{}", p.display());
        }

        let web = preview_link("https://example.com/a?b=1").unwrap();
        assert_eq!(web.target, "https://example.com/a?b=1");
        assert_eq!(web.host.as_deref(), Some("example.com"));
        assert!(web.size.is_none() && web.excerpt.is_none());

        let capped = preview_link(&big.to_string_lossy()).unwrap();
        let excerpt = capped.excerpt.unwrap();
        assert!(!excerpt.contains("TAIL"));
        assert!(excerpt.len() <= EXCERPT_BYTES);

        let pdf_p = preview_link(&pdf.to_string_lossy()).unwrap();
        assert!(pdf_p.excerpt.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn click_previews_and_run_does_not_execute_fixture() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("run");
        let approvals = dir.join("link-approvals.json");
        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let md = dir.join("note.md");
        std::fs::write(&md, "hello\nsecond\n").unwrap();
        let folder = dir.join("dir");
        std::fs::create_dir(&folder).unwrap();

        let first = click_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        assert_eq!(first.action, "preview");
        assert_eq!(first.preview.unwrap().kind, "executable");
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        let md_click = click_link_at(&md.to_string_lossy(), &approvals).unwrap();
        assert_eq!(md_click.action, "preview");
        let web = click_link_at("https://example.com/docs", &approvals).unwrap();
        assert_eq!(web.action, "preview");
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        let canon = md.canonicalize().unwrap().to_string_lossy().to_string();
        assert_eq!(OpenGuard::log(), vec![vec![canon.clone()]]);
        OpenGuard::clear();
        reveal(&preview_link(&folder.to_string_lossy()).unwrap()).unwrap();
        assert_eq!(
            OpenGuard::log(),
            vec![vec![
                "-R".into(),
                folder.canonicalize().unwrap().to_string_lossy().to_string()
            ]]
        );
        OpenGuard::clear();
        open_link_at("https://example.com/docs", &approvals).unwrap();
        assert_eq!(
            OpenGuard::log(),
            vec![vec!["https://example.com/docs".to_string()]]
        );

        OpenGuard::clear();
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let cmd_canon = cmd.canonicalize().unwrap().to_string_lossy().to_string();
        assert_eq!(OpenGuard::log(), vec![vec![cmd_canon]]);
        assert!(!sentinel.exists(), "fixture .command must not execute");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remembered_approval_is_exact_and_stale_executable_asks_again() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("memory");
        let approvals = dir.join("link-approvals.json");
        let md = dir.join("note.md");
        std::fs::write(&md, "one\n").unwrap();
        let other = dir.join("other.md");
        std::fs::write(&other, "two\n").unwrap();
        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );
        assert_eq!(OpenGuard::log().len(), 1);

        OpenGuard::clear();
        assert_eq!(
            click_link_at(&other.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        let https = "https://example.com/a?b=1";
        open_link_at(https, &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(click_link_at(https, &approvals).unwrap().action, "opened");
        OpenGuard::clear();
        assert_eq!(
            click_link_at("https://example.com/a?b=2", &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        #[cfg(unix)]
        {
            let same = dir.join("same.md");
            std::os::unix::fs::symlink(&md, &same).unwrap();
            OpenGuard::clear();
            assert_eq!(
                click_link_at(&same.to_string_lossy(), &approvals)
                    .unwrap()
                    .action,
                "opened"
            );
            let elsewhere = dir.join("else.md");
            std::os::unix::fs::symlink(&other, &elsewhere).unwrap();
            OpenGuard::clear();
            assert_eq!(
                click_link_at(&elsewhere.to_string_lossy(), &approvals)
                    .unwrap()
                    .action,
                "preview"
            );
            assert!(OpenGuard::log().is_empty());
        }

        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );
        assert!(!sentinel.exists());

        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran-more > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let f = std::fs::File::options().write(true).open(&cmd).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(180))
            .unwrap();
        drop(f);
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        assert!(!sentinel.exists());

        let canon = md.canonicalize().unwrap().to_string_lossy().to_string();
        remove_approval(&approvals, &canon).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        open_link_at(&md.to_string_lossy(), &approvals).unwrap();
        clear_approvals(&approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&md.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        let file = load_approvals(&approvals).unwrap();
        assert!(file.approvals.is_empty());
        assert!(file.note.contains("not synced"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ctl_reveals_executables_unless_run_or_approved() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("ctl");
        let approvals = dir.join("link-approvals.json");
        let sentinel = dir.join("sentinel");
        let cmd = dir.join("hello.command");
        std::fs::write(
            &cmd,
            format!("#!/bin/sh\necho ran > {}\n", sentinel.display()),
        )
        .unwrap();
        set_mode(&cmd, 0o755);
        let md = dir.join("note.md");
        std::fs::write(&md, "doc\n").unwrap();

        assert_eq!(
            ctl_open_at(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            OpenGuard::log()[0],
            vec![md.canonicalize().unwrap().to_string_lossy().to_string()]
        );

        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "reveal"
        );
        assert_eq!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());

        let fresh = dir.join("fresh.json");
        let fresh_before = snap(&fresh);
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), true, &fresh).unwrap(),
            "open"
        );
        assert_ne!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());
        assert_eq!(snap(&fresh), fresh_before, "--run must not write approvals");
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &fresh).unwrap(),
            "reveal"
        );
        assert_eq!(OpenGuard::log()[0][0], "-R");
        assert!(!sentinel.exists());

        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let approved = snap(&approvals);
        assert!(!approved.is_empty());
        OpenGuard::clear();
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            snap(&approvals),
            approved,
            "honoring an approval must not rewrite it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn snap(path: &Path) -> Vec<u8> {
        match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => panic!("read {}: {e}", path.display()),
        }
    }

    fn plist_xml(exe: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <plist version=\"1.0\"><dict>\n\
             <key>CFBundleExecutable</key><string>{exe}</string>\n\
             </dict></plist>\n"
        )
    }

    fn write_app(dir: &Path, body: &[u8]) -> PathBuf {
        let app = dir.join("Demo.app");
        let macos = app.join("Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::write(app.join("Contents/Info.plist"), plist_xml("Demo")).unwrap();
        let bin = macos.join("Demo");
        std::fs::write(&bin, body).unwrap();
        set_mode(&bin, 0o755);
        app
    }

    #[test]
    fn open_card_link_previews_not_opens() {
        let src = concat!(include_str!("norm.rs"), include_str!("preview.rs"), include_str!("tests.rs"));
        let start = src.find("pub fn open_card_link(").expect("open_card_link");
        let rest = &src[start..];
        let end = rest
            .find("\npub fn click_link(")
            .expect("click_link follows");
        let body = &rest[..end];
        assert!(body.contains("click_link_at("), "{body}");
        assert!(!body.contains("open_link_at("), "{body}");
        assert!(!body.contains("open_url("), "{body}");
    }

    #[test]
    fn ctl_and_mcp_open_leave_approvals_byte_identical() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("bytes");
        let approvals = dir.join("link-approvals.json");
        let seeded = b"{\n  \"note\": \"seed\",\n  \"approvals\": []\n}\n";
        std::fs::write(&approvals, seeded).unwrap();
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho hi\n").unwrap();
        set_mode(&cmd, 0o644);
        let md = dir.join("note.md");
        std::fs::write(&md, "doc\n").unwrap();

        let before = snap(&approvals);
        assert_eq!(
            ctl_open_at(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            ctl_open_at(&cmd.to_string_lossy(), true, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&md.to_string_lossy(), false, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&cmd.to_string_lossy(), true, &approvals).unwrap(),
            "open"
        );
        assert_eq!(
            crate::board::dispatch_kanban_open(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "reveal"
        );
        assert_eq!(snap(&approvals), before);

        let click = click_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        assert_eq!(click.action, "preview");
        assert_eq!(click.preview.unwrap().kind, "executable");
        assert_eq!(snap(&approvals), before);
        assert!(OpenGuard::log()
            .iter()
            .any(|c| c.first().map(String::as_str) == Some("-R")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundle_fingerprint_is_main_executable_and_plist() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("bundle");
        let approvals = dir.join("link-approvals.json");
        let app = write_app(&dir, b"#!/bin/sh\necho one\n");
        let bin = app.join("Contents/MacOS/Demo");
        let plist = app.join("Contents/Info.plist");
        let preview = preview_link(&app.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_some());
        assert!(preview.plist_sha256.is_some());
        assert_eq!(preview.size, Some(std::fs::metadata(&bin).unwrap().len()));

        open_link_at(&app.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        let folder = std::fs::File::open(&app).unwrap();
        folder
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(900))
            .unwrap();
        drop(folder);
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "opened"
        );

        let old_mtime = std::fs::metadata(&bin).unwrap().modified().unwrap();
        let old_len = std::fs::metadata(&bin).unwrap().len();
        let mut swapped = std::fs::read(&bin).unwrap();
        assert!(!swapped.is_empty());
        swapped[0] ^= 0x01;
        assert_eq!(swapped.len() as u64, old_len);
        std::fs::write(&bin, &swapped).unwrap();
        set_mode(&bin, 0o755);
        let f = std::fs::File::options().write(true).open(&bin).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        let meta = std::fs::metadata(&bin).unwrap();
        assert_eq!(meta.len(), old_len);
        assert_eq!(
            meta.modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            old_mtime
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());

        std::fs::write(&bin, b"#!/bin/sh\necho one\n").unwrap();
        set_mode(&bin, 0o755);
        let f = std::fs::File::options().write(true).open(&bin).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        open_link_at(&app.to_string_lossy(), &approvals).unwrap();
        std::fs::write(&plist, format!("{}<!-- edited -->\n", plist_xml("Demo"))).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&app.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_plist_names_the_main_executable() {
        let dir = temp_case("bplist");
        let app = write_app(&dir, b"#!/bin/sh\necho bin\n");
        let plist = app.join("Contents/Info.plist");
        let status = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "binary1"])
            .arg(&plist)
            .status()
            .unwrap();
        assert!(status.success());
        let bytes = std::fs::read(&plist).unwrap();
        assert!(bytes.starts_with(b"bplist00"));
        assert_eq!(cfbundle_executable(&bytes).as_deref(), Some("Demo"));
        let preview = preview_link(&app.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_some());
        assert!(preview.plist_sha256.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_size_rewrite_with_mtime_restored_asks_again() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("hash");
        let approvals = dir.join("link-approvals.json");
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "#!/bin/sh\necho one\n").unwrap();
        set_mode(&cmd, 0o644);
        open_link_at(&cmd.to_string_lossy(), &approvals).unwrap();
        let row = load_approvals(&approvals)
            .unwrap()
            .approvals
            .into_iter()
            .find(|a| a.kind == "executable")
            .unwrap();
        let old_mtime = std::fs::metadata(&cmd).unwrap().modified().unwrap();
        let mut body = std::fs::read(&cmd).unwrap();
        body[8] ^= 0x01;
        std::fs::write(&cmd, &body).unwrap();
        set_mode(&cmd, 0o644);
        let f = std::fs::File::options().write(true).open(&cmd).unwrap();
        f.set_modified(old_mtime).unwrap();
        drop(f);
        let meta = std::fs::metadata(&cmd).unwrap();
        assert_eq!(Some(meta.len()), row.size);
        assert_eq!(
            Some(
                meta.modified()
                    .unwrap()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            row.modified_secs
        );
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&cmd.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn over_cap_executable_always_previews() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("cap");
        let approvals = dir.join("link-approvals.json");
        let big = dir.join("big.command");
        let f = std::fs::File::create(&big).unwrap();
        f.set_len(HASH_CAP + 1).unwrap();
        drop(f);
        set_mode(&big, 0o644);
        let preview = preview_link(&big.to_string_lossy()).unwrap();
        assert_eq!(preview.kind, "executable");
        assert!(preview.content_sha256.is_none());
        open_link_at(&big.to_string_lossy(), &approvals).unwrap();
        OpenGuard::clear();
        assert_eq!(
            click_link_at(&big.to_string_lossy(), &approvals)
                .unwrap()
                .action,
            "preview"
        );
        assert!(OpenGuard::log().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_link_refuses_when_target_changed_since_preview() {
        let _hook = OpenGuard::arm();
        let dir = temp_case("stale");
        let approvals = dir.join("link-approvals.json");
        std::fs::write(&approvals, b"{\"note\":\"n\",\"approvals\":[]}\n").unwrap();
        let md = dir.join("note.md");
        std::fs::write(&md, "hello\n").unwrap();
        let shown = preview_link(&md.to_string_lossy()).unwrap();
        let expect = LinkFingerprint::from_preview(&shown);
        std::fs::write(&md, "hello\nchanged\n").unwrap();
        let before = snap(&approvals);
        let res = open_link_checked(&md.to_string_lossy(), &approvals, &expect).unwrap();
        assert_eq!(res.action, "preview");
        assert_eq!(res.preview.unwrap().kind, "document");
        assert!(OpenGuard::log().is_empty());
        assert_eq!(snap(&approvals), before);

        let shown = preview_link(&md.to_string_lossy()).unwrap();
        let expect = LinkFingerprint::from_preview(&shown);
        set_mode(&md, 0o755);
        let res = open_link_checked(&md.to_string_lossy(), &approvals, &expect).unwrap();
        assert_eq!(res.action, "preview");
        assert_eq!(res.preview.as_ref().unwrap().kind, "executable");
        assert!(OpenGuard::log().is_empty());
        assert_eq!(snap(&approvals), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
