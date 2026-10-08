    use super::*;
    use std::path::{Path, PathBuf};

    fn row() -> crate::state::AgentShortcut {
        crate::state::AgentShortcut {
            id: "a1".into(),
            name: "orbitty-cursor".into(),
            harness: "cursor".into(),
            profile: String::new(),
            model: None,
            md_paths: vec![],
            project_id: Some("p1".into()),
            extra: None,
            permissions: "read-only".into(),
        }
    }

    #[test]
    fn query_file_carries_the_full_go_text() {
        let titles = vec!["GO-ORBITTY-X".to_string()];
        let go = "---\nid: GO-ORBITTY-X\n---\nBODY-SENTINEL\n\n## Launcher rules\n\n- git push: NOT allowed";
        let (body, _) = agent_query_body(&row(), Some("orbitty"), &titles, Some(go));
        assert!(body.contains("## GO\n\n---\nid: GO-ORBITTY-X"), "{body}");
        assert!(body.contains("BODY-SENTINEL"), "{body}");
        assert!(body.contains("git push: NOT allowed"), "{body}");
        assert!(!body.contains("titles only"), "{body}");
        // GO text comes before the planet titles.
        assert!(body.find("BODY-SENTINEL").unwrap() < body.find("- GO-ORBITTY-X").unwrap());
        let (plain, _) = agent_query_body(&row(), Some("orbitty"), &titles, None);
        assert!(!plain.contains("## GO"), "{plain}");
        assert!(plain.contains("titles only"), "{plain}");
    }

    #[test]
    fn terminal_setting_picks_ghostty_when_empty_and_installed() {
        assert_eq!(resolve_agent_terminal("", true), "Ghostty");
        assert_eq!(resolve_agent_terminal("", false), "Terminal");
        assert_eq!(resolve_agent_terminal("ghostty", false), "Ghostty");
        assert_eq!(resolve_agent_terminal("terminal", true), "Terminal");
        assert_eq!(resolve_agent_terminal("nope", true), "Ghostty");
        assert_eq!(resolve_agent_terminal("nope", false), "Terminal");
    }

    #[test]
    fn command_script_uses_bundled_ctl_and_sock() {
        let ctl = PathBuf::from("/Applications/Orbitty Nightly.app/Contents/MacOS/orbittyctl");
        let sock = PathBuf::from("/tmp/nightly/control.sock");
        let body = agent_command_body(
            Path::new("/tmp/q.md"),
            Path::new("/tmp/run.command"),
            Path::new("/tmp/run.pid"),
            "\"/usr/local/bin/cursor-agent\" --model \"grok-4.7-high\" \"$prompt\"",
            &ctl,
            &sock,
            Some("/tmp/workspace"),
        );
        assert!(body.contains(ctl.to_str().unwrap()));
        assert!(body.contains("--sock"));
        assert!(body.contains(sock.to_str().unwrap()));
        assert!(body.contains("cd '/tmp/workspace'"));
        assert!(body.contains("trap orbitty_cleanup EXIT"));
        assert!(!body.contains(".local/bin"));
        let argv = open_terminal_argv("Ghostty", Path::new("/tmp/run.command"));
        assert_eq!(
            argv,
            vec![
                "-a".to_string(),
                "Ghostty".to_string(),
                "/tmp/run.command".to_string()
            ]
        );
    }

    #[test]
    fn cursor_permissions_land_in_the_worktree_exclude() {
        let dir = std::env::temp_dir().join(format!(
            "orbitty-cursor-perm-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let init = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&dir)
            .status()
            .expect("git init");
        assert!(init.success(), "git init");
        install_cursor_permissions(&dir, "read-only").unwrap();
        let cli_path = project_cli_path(&dir);
        assert!(cli_path.ends_with(".cursor/cli.json"));
        assert!(!cli_path.to_string_lossy().contains("cli-config.json"));
        let raw = std::fs::read_to_string(&cli_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let allow = v["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|t| t == "Shell(find)"));
        assert!(allow.iter().any(|t| t == "Shell(head)"));
        assert!(allow.iter().all(|t| !t.as_str().unwrap_or("").starts_with("Write(")));
        assert!(v["permissions"]["deny"].as_array().unwrap().iter().any(|t| t == "Write(**)"));
        let exclude = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert!(exclude.lines().any(|l| l.trim() == "**/.cursor/cli.json"));

        let mut root: serde_json::Value = serde_json::from_str(&raw).unwrap();
        root["editor"] = serde_json::json!({ "vimMode": true });
        std::fs::write(&cli_path, serde_json::to_string_pretty(&root).unwrap()).unwrap();
        install_cursor_permissions(&dir, "workspace-write").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_path).unwrap()).unwrap();
        assert_eq!(v["editor"]["vimMode"], true);
        assert!(v["permissions"]["allow"].as_array().unwrap().iter().any(|t| t == "Write(**)"));
        assert!(v["permissions"]["allow"].as_array().unwrap().iter().all(|t| {
            !t.as_str().unwrap_or("").starts_with("Write(/")
        }));
        assert!(v["permissions"]["deny"].as_array().unwrap().is_empty());

        install_cursor_permissions(&dir, "full-auto").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_path).unwrap()).unwrap();
        assert!(v["permissions"]["deny"].as_array().unwrap().is_empty());

        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            assert!(install_cursor_permissions(Path::new(&home), "read-only").is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "orbitty-cl-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn repo_with_commit(dir: &Path) {
        git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("README"), "x\n").unwrap();
        git(dir, &["add", "README"]);
        git(dir, &["commit", "-q", "-m", "init", "--no-verify"]);
    }

    fn arr(v: &serde_json::Value, key: &str) -> Vec<String> {
        v["permissions"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn linked_worktree_cli_json_is_ignored() {
        let base = tmp("wt");
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        repo_with_commit(&main);
        let wt = base.join("wt");
        git(&main, &["worktree", "add", "-q", "-b", "wt", wt.to_str().unwrap()]);
        install_cursor_permissions_with_home(&wt, "read-only", None).unwrap();
        assert!(project_cli_path(&wt).is_file());
        assert_eq!(git(&wt, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&wt, &["check-ignore", "-q", ".cursor/cli.json"]);
        let common = std::fs::read_to_string(main.join(".git/info/exclude")).unwrap();
        assert!(common.lines().any(|l| l == "**/.cursor/cli.json"));
        // Idempotent: a second install doesn't duplicate the pattern.
        install_cursor_permissions_with_home(&wt, "read-only", None).unwrap();
        let common = std::fs::read_to_string(main.join(".git/info/exclude")).unwrap();
        assert_eq!(common.lines().filter(|l| *l == "**/.cursor/cli.json").count(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn subdirectory_workspace_cli_json_is_ignored() {
        let base = tmp("sub");
        repo_with_commit(&base);
        let sub = base.join("pkg").join("app");
        std::fs::create_dir_all(&sub).unwrap();
        install_cursor_permissions_with_home(&sub, "read-only", None).unwrap();
        assert!(sub.join(".cursor/cli.json").is_file());
        assert_eq!(git(&base, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&sub, &["check-ignore", "-q", ".cursor/cli.json"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn tracked_cli_json_is_left_untouched() {
        let base = tmp("tracked");
        repo_with_commit(&base);
        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        let theirs = "{\n  \"version\": 1,\n  \"permissions\": {\"allow\": [\"Shell(npm)\"], \"deny\": [\"Shell(rm)\"]}\n}\n";
        std::fs::write(base.join(".cursor/cli.json"), theirs).unwrap();
        git(&base, &["add", ".cursor/cli.json"]);
        git(&base, &["commit", "-q", "-m", "cfg", "--no-verify"]);
        let err = install_cursor_permissions_with_home(&base, "read-only", None).err().unwrap();
        assert!(err.contains("tracked .cursor/cli.json left untouched"), "{err}");
        assert_eq!(std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap(), theirs);
        assert!(!base.join(".cursor/orbitty-managed.json").exists());
        assert_eq!(git(&base, &["status", "--porcelain", "-uall"]).trim(), "");
        // Same from a subdirectory of the repo root? Not applicable: the
        // tracked file is the root's; a sub workspace has its own path.
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn untracked_cli_json_keeps_user_lists_across_presets() {
        let base = tmp("merge");
        let user = serde_json::json!({
            "version": 1,
            "editor": {"vimMode": true},
            "permissions": {"allow": ["Shell(npm)"], "deny": ["Shell(rm)", "Write(**)"]}
        });
        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        std::fs::write(base.join(".cursor/cli.json"), user.to_string()).unwrap();
        install_cursor_permissions_with_home(&base, "read-only", None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap()).unwrap();
        let (allow, deny) = (arr(&v, "allow"), arr(&v, "deny"));
        assert_eq!(allow[0], "Shell(npm)");
        assert!(allow.contains(&"Shell(find)".to_string()));
        assert_eq!(&deny[..2], &["Shell(rm)".to_string(), "Write(**)".to_string()]);
        assert!(deny.contains(&"Shell(find:*-delete*)".to_string()));
        assert_eq!(deny.iter().filter(|d| *d == "Write(**)").count(), 1);
        assert_eq!(v["editor"]["vimMode"], true);

        // workspace-write: our read-only denies go, the user's stay —
        // including their own Write(**) deny.
        install_cursor_permissions_with_home(&base, "workspace-write", None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap()).unwrap();
        let (allow, deny) = (arr(&v, "allow"), arr(&v, "deny"));
        assert_eq!(deny, vec!["Shell(rm)".to_string(), "Write(**)".to_string()]);
        assert!(allow.contains(&"Shell(npm)".to_string()));
        assert!(allow.contains(&"Write(**)".to_string()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn failed_launch_rolls_the_permission_file_back() {
        let base = tmp("undo");
        let undo = install_cursor_permissions_with_home(&base, "read-only", None).unwrap();
        assert!(project_cli_path(&base).is_file());
        undo.rollback();
        assert!(!base.join(".cursor").exists());

        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        let mine = "{\"version\":1,\"permissions\":{\"allow\":[\"Shell(npm)\"]}}";
        std::fs::write(project_cli_path(&base), mine).unwrap();
        let undo = install_cursor_permissions_with_home(&base, "full-auto", None).unwrap();
        assert_ne!(std::fs::read_to_string(project_cli_path(&base)).unwrap(), mine);
        undo.rollback();
        assert_eq!(std::fs::read_to_string(project_cli_path(&base)).unwrap(), mine);
        assert!(!base.join(".cursor/orbitty-managed.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn refuses_home_and_global_cursor_dir() {
        let home = tmp("home");
        let sub = home.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let h = Some(home.as_path());
        assert!(cursor_perm_refusal(&home, h).is_some());
        assert!(cursor_perm_refusal(&sub.join(".."), h).is_some(), "$HOME/sub/.. must be $HOME");
        assert!(cursor_perm_refusal(&home.join("."), h).is_some());
        #[cfg(unix)]
        {
            let link = home.join("sub/homelink");
            std::os::unix::fs::symlink(&home, &link).unwrap();
            assert!(cursor_perm_refusal(&link, h).is_some(), "symlink to $HOME");
        }
        let gcur = home.join(".cursor/projects");
        std::fs::create_dir_all(&gcur).unwrap();
        assert!(cursor_perm_refusal(&home.join(".cursor"), h).is_some());
        assert!(cursor_perm_refusal(&gcur, h).is_some());
        assert!(cursor_perm_refusal(&sub, h).is_none());
        assert!(cursor_perm_refusal(Path::new("/"), h).is_some());
        let _ = std::fs::remove_dir_all(&home);
    }

    /// N4 decision: the main (non-linked) checkout — a temp `$HOME/orbitty`
    /// stands in for `~/orbitty` via the injected home — gets the preset,
    /// kept out of commits by its own info/exclude.
    #[test]
    fn main_checkout_gets_preset_and_stays_uncommitted() {
        let home = tmp("mainco");
        let orb = home.join("orbitty");
        std::fs::create_dir_all(&orb).unwrap();
        repo_with_commit(&orb);
        // Non-linked: git dir == common dir.
        assert_eq!(
            git(&orb, &["rev-parse", "--git-dir"]).trim(),
            git(&orb, &["rev-parse", "--git-common-dir"]).trim()
        );
        let h = Some(home.as_path());
        assert!(cursor_perm_refusal(&orb, h).is_none());
        install_cursor_permissions_with_home(&orb, "read-only", h).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project_cli_path(&orb)).unwrap()).unwrap();
        assert!(arr(&v, "allow").contains(&"Shell(find)".to_string()));
        assert!(arr(&v, "deny").contains(&"Write(**)".to_string()));
        let exclude = std::fs::read_to_string(orb.join(".git/info/exclude")).unwrap();
        assert!(exclude.lines().any(|l| l == "**/.cursor/cli.json"));
        assert_eq!(git(&orb, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&orb, &["check-ignore", "-q", ".cursor/cli.json"]);
        let _ = std::fs::remove_dir_all(&home);
    }

    /// N1 source guard: the launch doors validate their row values FIRST.
    /// Cursor: `launch_model(&shortcut)…?` before permissions, herdr and the
    /// terminal. Hermes: `hermes_args(&shortcut, &query)…?` before herdr
    /// and the spawn. Byte offsets inside each `launch_agent` arm.
    fn launch_arm<'a>(src: &'a str, start: &str, next: &str) -> &'a str {
        let body = &src[src.find("pub fn launch_agent(").expect("launch_agent")..];
        let a = body.find(start).unwrap_or_else(|| panic!("no arm {start}"));
        let b = a + body[a..].find(next).unwrap_or_else(|| panic!("no arm {next}"));
        &body[a..b]
    }

    /// Offset of `call`, asserting its statement ends in `?;`.
    fn checked_call(arm: &str, call: &str) -> usize {
        let at = arm.find(call).unwrap_or_else(|| panic!("missing {call}"));
        let stmt_end = at + arm[at..].find(';').unwrap();
        assert!(arm[..stmt_end].ends_with('?'), "{call} must end in `?;`");
        at
    }

    #[test]
    fn launch_doors_validate_before_herdr_and_spawn() {
        let src = concat!(
include_str!("hermes.rs"),
include_str!("allowlist.rs"),
include_str!("plug.rs"),
include_str!("mouth_cli.rs"),
include_str!("commands.rs"),
include_str!("intercept.rs"),
include_str!("send.rs"),
include_str!("soul_wake.rs"),
include_str!("action.rs"),
include_str!("paths.rs"),
include_str!("cursor_perm.rs"),
include_str!("launch_agent.rs"),
include_str!("mac.rs"),
include_str!("launch_tests.rs")
);
        let cursor = launch_arm(src, "\n        \"cursor\" => {", "\n        \"goose\" => {");
        let at = checked_call(cursor, "crate::agents::launch_model(&shortcut)");
        for later in ["install_cursor_permissions(", "crate::herdr::branch(", "open_agent_terminal("] {
            let pos = cursor.find(later).unwrap_or_else(|| panic!("cursor arm lost {later}"));
            assert!(at < pos, "launch_model must come before {later}");
        }
        assert!(!cursor.contains("cursor_model("), "cursor arm resolves the model via launch_model");

        let hermes = launch_arm(src, "\n        \"hermes\" => {", "\n        \"opencode\" =>");
        let at = checked_call(hermes, "crate::agents::hermes_args(&shortcut, &query)");
        for later in ["crate::herdr::branch(", "Command::new(\"hermes\")"] {
            let pos = hermes.find(later).unwrap_or_else(|| panic!("hermes arm lost {later}"));
            assert!(at < pos, "hermes_args must come before {later}");
        }
        assert!(!hermes.contains("\"-m\"") && !hermes.contains("\"-p\""), "hermes argv comes from hermes_args");
    }

    #[test]
    fn sq_escapes_single_quotes() {
        assert_eq!(sq("a b"), "'a b'");
        assert_eq!(sq("it's"), "'it'\\''s'");
        assert_eq!(sq(""), "''");
    }

    fn find_pwned(dir: &Path, hits: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            // The hostile names contain "PWNED" mid-name; a marker that
            // actually ran is a file NAMED PWNED_*.
            if p.file_name().unwrap().to_string_lossy().starts_with("PWNED") {
                hits.push(p.clone());
            }
            if p.is_dir() && !p.is_symlink() {
                find_pwned(&p, hits);
            }
        }
    }

    /// Run a generated terminal door under zsh with a stub that records its
    /// argv NUL-separated; returns that argv.
    fn run_door_with_prompt(tag: &str, name: &str, exec_for: impl Fn(&str) -> String, prompt: &str) -> (String, Vec<String>) {
        use std::os::unix::fs::PermissionsExt;
        let base = tmp(tag);
        let exe = |n: &str| {
            let p = base.join(n);
            std::fs::write(&p, "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\000' \"$a\"; done > \"$0.args\"\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let bin = exe(name);
        let ctl = exe("orbittyctl");
        let query = base.join("q.md");
        std::fs::write(&query, prompt).unwrap();
        let exec = exec_for(bin.to_str().unwrap());
        let script = base.join("run.command");
        let body = agent_command_body(&query, &script, &base.join("run.pid"), &exec, &ctl, &base.join("sock"), Some(base.to_str().unwrap()));
        std::fs::write(&script, &body).unwrap();
        let run = std::process::Command::new("/bin/zsh").arg(&script).current_dir(&base).output().unwrap();
        assert!(run.status.success(), "run: {}", String::from_utf8_lossy(&run.stderr));
        let raw = std::fs::read_to_string(format!("{}.args", bin.display())).unwrap();
        let args = raw.trim_end_matches('\0').split('\0').map(String::from).collect();
        let _ = std::fs::remove_dir_all(&base);
        (exec, args)
    }

    const DASHED: &str = "---\nid: GO-DASHES\n---\n-t and -p and --force are text here $(touch PWNED_D)";

    #[test]
    fn goose_prompt_starting_with_dashes_is_the_attached_text_value() {
        if !Path::new("/bin/zsh").exists() {
            return;
        }
        let (exec, args) = run_door_with_prompt("goose", "goose", crate::agents::goose_exec_line, DASHED);
        assert!(exec.ends_with(" --text=\"$prompt\""), "{exec}");
        assert!(!exec.contains(" -t "), "{exec}");
        assert_eq!(args, vec!["run".to_string(), "--with-builtin=developer,computercontroller".into(), format!("--text={DASHED}")]);
        let mouth = goose_args("soul", DASHED, None, true);
        assert_eq!(mouth.last().unwrap(), &format!("--text={DASHED}"));
        assert!(!mouth.iter().any(|a| a == "-t" || a == DASHED));
    }

    #[test]
    fn grok_prompt_starting_with_dashes_is_the_attached_single_value() {
        if !Path::new("/bin/zsh").exists() {
            return;
        }
        let (exec, args) = run_door_with_prompt("grok", "grok", crate::agents::grok_exec_line, DASHED);
        assert!(exec.ends_with(" --single=\"$prompt\""), "{exec}");
        assert!(!exec.contains(" -p "), "{exec}");
        assert_eq!(args, vec![format!("--single={DASHED}")]);
        let mouth = grok_args("sid", false, "soul", DASHED, true, None);
        assert_eq!(mouth[0], format!("--single={DASHED}"));
        assert!(!mouth.iter().any(|a| a == "-p" || a == DASHED));
    }

    /// A GO passed as the prompt starts with `---`; the generated cursor
    /// command must put `--` right before it, and the prompt must arrive
    /// as one exact argument.
    #[test]
    fn cursor_prompt_starting_with_dashes_follows_end_of_options() {
        if !Path::new("/bin/zsh").exists() {
            eprintln!("no /bin/zsh; skipped");
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let base = tmp("dashes");
        let exe = |name: &str| {
            let p = base.join(name);
            std::fs::write(&p, "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\000' \"$a\"; done > \"$0.args\"\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let bin = exe("cursor-agent");
        let ctl = exe("orbittyctl");
        let prompt = "---\nid: GO-DASHES\ntools: [shell]\n---\n--force is not a flag here";
        let query = base.join("q.md");
        std::fs::write(&query, prompt).unwrap();
        let exec = crate::agents::cursor_exec_line(bin.to_str().unwrap(), "grok-4.7-high", "full-auto");
        assert!(exec.ends_with(" -- \"$prompt\""), "{exec}");
        let script = base.join("run.command");
        let body = agent_command_body(
            &query,
            &script,
            &base.join("run.pid"),
            &exec,
            &ctl,
            &base.join("sock"),
            Some(base.to_str().unwrap()),
        );
        std::fs::write(&script, &body).unwrap();
        let run = std::process::Command::new("/bin/zsh").arg(&script).current_dir(&base).output().unwrap();
        assert!(run.status.success(), "run: {}", String::from_utf8_lossy(&run.stderr));
        let raw = std::fs::read_to_string(format!("{}.args", bin.display())).unwrap();
        let args: Vec<&str> = raw.trim_end_matches('\0').split('\0').collect();
        assert_eq!(args, vec!["--model=grok-4.7-high", "--force", "--trust", "--", prompt]);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// B2: hostile dir / model / query / pid / script / bin / ctl / sock
    /// values. `zsh -n` must parse the body, and running it must execute
    /// nothing from any value, pass each value through verbatim, and still
    /// run the cleanup trap.
    #[test]
    fn command_body_survives_hostile_values_under_zsh() {
        if !Path::new("/bin/zsh").exists() {
            eprintln!("no /bin/zsh; skipped");
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let base = tmp("zsh");
        let evil = "x 'q' \"dq\" $(touch PWNED_S) `touch PWNED_B`; touch PWNED_C\nnl & | < >";
        let dir = base.join(format!("dir {evil}"));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = |name: String, body: &str| {
            let p = base.join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let rec = "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n";
        let bin = exe(format!("bin {evil}"), rec);
        let ctl = exe(format!("ctl {evil}"), rec);
        let sock = base.join(format!("sock {evil}"));
        let query = base.join(format!("q {evil}.md"));
        std::fs::write(&query, "hello $(touch PWNED_PROMPT)").unwrap();
        let pidf = base.join(format!("pid {evil}.pid"));
        let script = base.join(format!("run {evil}.command"));
        let model = format!("m {evil}");
        let exec = crate::agents::cursor_exec_line(bin.to_str().unwrap(), &model, "read-only");
        let body = agent_command_body(
            &query,
            &script,
            &pidf,
            &exec,
            &ctl,
            &sock,
            Some(dir.to_str().unwrap()),
        );
        std::fs::write(&script, &body).unwrap();
        let n = std::process::Command::new("/bin/zsh").arg("-n").arg(&script).output().unwrap();
        assert!(n.status.success(), "zsh -n: {}\n{body}", String::from_utf8_lossy(&n.stderr));
        let run = std::process::Command::new("/bin/zsh")
            .arg(&script)
            .current_dir(&base)
            .output()
            .unwrap();
        assert!(run.status.success(), "run: {}", String::from_utf8_lossy(&run.stderr));
        let mut hits = Vec::new();
        find_pwned(&base, &mut hits);
        assert!(hits.is_empty(), "executed: {hits:?}");
        let bin_args = std::fs::read_to_string(format!("{}.args", bin.display())).unwrap();
        assert_eq!(
            bin_args,
            format!("--model={model}\n--\nhello $(touch PWNED_PROMPT)\n")
        );
        let ctl_args = std::fs::read_to_string(format!("{}.args", ctl.display())).unwrap();
        assert_eq!(
            ctl_args,
            format!("--sock\n{}\ncompanion\nhack\n--\noff\n", sock.display())
        );
        assert!(!pidf.exists(), "cleanup removed the pid file");
        assert!(!script.exists(), "cleanup removed the script");
        let _ = std::fs::remove_dir_all(&base);
    }

