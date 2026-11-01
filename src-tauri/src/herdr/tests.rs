//! Read-side gates. Fake hc only — locate() in tests never reaches a real core.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::MutexGuard;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::client;
use super::core;
use super::health::{self, Source};
use super::peek;
use super::redact;
use super::tui;

fn gate() -> MutexGuard<'static, ()> {
    client::TEST_GATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn hc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fake_hc/hc")
}

fn scratch(label: &str) -> PathBuf {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("orbitty-herdr-{label}-{n}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn arm(mode: &str, log: &Path) {
    let mut perms = fs::metadata(hc_path()).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(hc_path(), perms).unwrap();
    client::force_flag(Some(true));
    core::set_bin_override(Some(hc_path()));
    client::set_child_env(vec![
        ("FAKE_HC_MODE".into(), mode.into()),
        ("FAKE_HC_LOG".into(), log.display().to_string()),
    ]);
}

struct Clean(PathBuf);
impl Drop for Clean {
    fn drop(&mut self) {
        client::reset_overrides();
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn read_gate_flag_on() {
    let _g = gate();
    let dir = scratch("gate");
    let _c = Clean(dir.clone());
    let hlog = dir.join("herdr.log");
    let clog = dir.join("hc.log");
    let herdr = dir.join("herdr");
    let script = format!("#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n", hlog.display());
    fs::write(&herdr, script).unwrap();
    let mut perms = fs::metadata(&herdr).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&herdr, perms).unwrap();
    tui::set_bin_override(Some(herdr));
    arm("live", &clog);
    health::prove_up();
    let _ = client::alive("t1");
    let _ = client::status_value();
    let _ = peek::view(&[], "t1", 40, true);
    let hers = fs::read_to_string(&hlog).unwrap_or_default();
    assert!(hers.is_empty(), "flag on still called herdr: {hers}");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut bad = Vec::new();
    visit(&root, &mut bad);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

fn visit(dir: &Path, bad: &mut Vec<String>) {
    let needles = [
        "&[\"pane\"",
        "&[\"workspace\"",
        "&[\"agent\"",
        ".args([\"agent\"",
        ".args([\"pane\"",
        ".args([\"workspace\"",
        "\"pane\",\"close\"",
        "\"workspace\",\"close\"",
        "\"workspace\",\"create\"",
        "\"workspace\",\"focus\"",
        "\"workspace\",\"list\"",
        "\"agent\",\"prompt\"",
        "\"agent\",\"wait\"",
        "\"agent\",\"get\"",
        "\"agent\",\"focus\"",
        "\"agent\",\"read\"",
    ];
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            visit(&p, bad);
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&p).unwrap_or_default();
        let compact: String = text.split_whitespace().collect();
        let rel = p.display().to_string();
        let in_tui = rel.ends_with("herdr/tui.rs");
        let in_herdr = rel.contains("/herdr/");
        if !in_tui
            && (text.contains("find_on_path(\"herdr\")") || text.contains("which(\"herdr\")"))
        {
            bad.push(format!("herdr lookup outside tui.rs: {rel}"));
        }
        if !in_tui
            && (text.contains("Command::new(\"herdr\")") || text.contains("Command::new(\"hc\")"))
        {
            bad.push(format!("herdr/hc command outside tui.rs: {rel}"));
        }
        if !in_herdr
            && (text.contains("Command::new(\"herdr\")") || text.contains("Command::new(\"hc\")"))
        {
            bad.push(format!("herdr/hc command outside herdr/: {rel}"));
        }
        if !in_tui {
            for n in needles {
                if compact.contains(n) {
                    bad.push(format!("tui argv outside tui.rs: {rel} {n}"));
                    break;
                }
            }
        }
    }
}

#[test]
fn flag_off_goldens_identical() {
    let _g = gate();
    let dir = scratch("golden");
    let _c = Clean(dir.clone());
    let log = dir.join("argv.log");
    let bin = dir.join("herdr");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\nif [ \"$1\" = \"workspace\" ]; then echo '{{\"result\":{{}}}}'; exit 0; fi\necho '{{\"result\":{{\"agent\":{{\"agent_status\":\"working\"}}}}}}'\nexit 0\n",
        log.display()
    );
    fs::write(&bin, script).unwrap();
    let mut perms = fs::metadata(&bin).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&bin, perms).unwrap();
    client::force_flag(Some(false));
    tui::set_bin_override(Some(bin));
    let alive = tui::agent_alive("GOLD");
    assert_eq!(alive, Some(true));
    let got = fs::read_to_string(&log).unwrap();
    assert_eq!(got, "workspace list\nagent get GOLD\n");
    fs::write(&log, "").unwrap();
    let bin = dir.join("herdr");
    let _ = client::ensure_server(&bin);
    let _ = client::workspace_create(&bin, "/tmp/orbitty-golden", "orbitty");
    let _ = client::agent_start(&bin, "c-1", "grok", "p1", &[]);
    let rec = crate::state::AgentRunHerdr {
        agent_name: "c-1".into(),
        pane: "p1".into(),
        workspace: "ws1".into(),
        backend: "tui".into(),
        ..Default::default()
    };
    let _ = client::prompt_with(&bin, &rec, "hello");
    let _ = client::wait_run(&bin, &rec, None);
    client::stop(&rec);
    let got = fs::read_to_string(&log).unwrap();
    assert_eq!(
        got,
        "\
workspace list
workspace create --cwd /tmp/orbitty-golden --label orbitty
agent start c-1 --kind grok --pane p1 --timeout 30000
agent prompt c-1 hello
agent wait c-1 --until working --until blocked --until done --timeout 60000 --until idle
pane close p1
workspace close ws1
"
    );
}

#[test]
fn core_probe_maps_exit_codes() {
    let _g = gate();
    let dir = scratch("probe");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::prove_up();
    assert_eq!(health::source(), Source::Herdr);
    arm("not_found", &log);
    assert_eq!(client::alive("nosuch"), Some(false));
    health::assume_up();
    arm("socket", &log);
    assert_eq!(client::alive("t1"), None);
    health::assume_up();
    arm("live", &log);
    assert_eq!(client::alive("t1"), Some(true));
}

#[test]
fn hc_missing_uses_fallback() {
    let _g = gate();
    let _c = Clean(scratch("miss"));
    client::force_flag(Some(true));
    core::set_bin_override(None);
    health::reset();
    assert_eq!(client::alive("t1"), None);
    assert_eq!(health::source(), Source::Fallback);
}

#[test]
fn hc_crash_midstream_down_then_up() {
    let _g = gate();
    let dir = scratch("crash");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("crash", &log);
    health::reset();
    health::assume_up();
    health::set_clock_ms(Some(10_000));
    health::poll();
    assert_eq!(health::source(), Source::Fallback);
    assert_eq!(health::events(), vec!["fallback"]);
    health::set_clock_ms(Some(20_000));
    health::poll();
    assert_eq!(health::events(), vec!["fallback"]);
    arm("live", &log);
    health::set_clock_ms(Some(40_000));
    health::poll();
    assert_eq!(health::source(), Source::Fallback, "one healthy check is not back");
    health::set_clock_ms(Some(70_000));
    health::poll();
    assert_eq!(health::source(), Source::Herdr);
    assert_eq!(health::events(), vec!["fallback", "herdr"]);
}

#[test]
fn down_never_marks_died() {
    let _g = gate();
    let dir = scratch("down");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("not_found", &log);
    health::reset();
    health::force_fallback(1_000, "transport");
    assert_eq!(client::alive("nosuch"), None);
}

#[test]
fn server_start_never_on_hot_path() {
    let _g = gate();
    let dir = scratch("hot");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::prove_up();
    let _ = fs::write(&log, "");
    let bin = hc_path();
    let _ = client::ensure_server(&bin);
    let _ = client::status_value();
    let _ = client::alive("t1");
    let text = fs::read_to_string(&log).unwrap();
    assert!(!text.contains("server start"), "{text}");
    assert!(!text.lines().any(|l| l == "server" || l.starts_with("server start")));
    for line in text.lines() {
        if line.starts_with("server") {
            assert!(line.starts_with("server status"), "{line}");
        }
    }
}

#[test]
fn peek_caps_lines() {
    let _g = gate();
    let dir = scratch("caps");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("huge", &log);
    health::prove_up();
    let v = peek::view(&[], "t1", 40, true);
    let lines: Vec<&str> = v.text.lines().collect();
    assert!(lines.len() <= 200, "{}", lines.len());
    assert!(v.text.len() <= 16 * 1024, "{}", v.text.len());
    assert!(lines.iter().all(|l| l.chars().count() <= 400));
    assert!(v.text.contains("lines hidden"), "{text}", text = v.text);
    let short = (0..250).map(|i| format!("n{i}")).collect::<Vec<_>>().join("\n");
    let capped = peek::cap_preview(&short);
    assert_eq!(capped.lines().count(), 200);
    assert!(capped.contains("… 51 lines hidden"), "{capped}");
}

#[test]
fn peek_redacts_secrets() {
    let raw = include_str!("../../tests/fake_hc/fixtures/secrets.txt");
    let out = redact::redact(raw);
    for needle in [
        "sk-or-v1-",
        "sk-abc",
        "ghp_",
        "gho_",
        "github_pat_",
        "xoxb-",
        "AKIAIOSFODNN7EXAMPLE",
        "BEGIN OPENSSH PRIVATE KEY",
        "supersecretline",
        "Bearer abcdefghijklmnop",
        "abcdEFGH123456",
        "zzzzzzzzzz",
        "s3cret",
        "0123456789abcdef0123456789abcdef",
    ] {
        assert!(!out.contains(needle), "{needle} survived:\n{out}");
    }
    assert!(out.contains("hello world"), "{out}");
    assert!(out.contains("the quick brown fox"), "{out}");
    assert_eq!(out, GOLDEN_REDACT);
}

const GOLDEN_REDACT: &str = "\
hello world
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
[redacted]
the quick brown fox
";

#[test]
fn peek_unknown_when_down() {
    let _g = gate();
    let dir = scratch("unk");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::prove_up();
    let open = peek::view(&[], "t1", 40, true);
    assert_ne!(open.status, "unknown");
    assert!(!open.text.is_empty(), "{}", open.text);
    arm("socket", &log);
    let down = peek::view(&[], "t1", 40, true);
    assert_eq!(down.status, "unknown");
    assert!(down.text.is_empty(), "{}", down.text);
    arm("live", &log);
    let now = health::now_ms();
    health::set_clock_ms(Some(now + 30_000));
    health::poll();
    assert!(health::is_down(), "one healthy check stays down");
    health::set_clock_ms(Some(now + 60_000));
    health::poll();
    assert!(!health::is_down());
    let back = peek::view(&[], "t1", 40, true);
    assert_ne!(back.status, "unknown");
    assert!(!back.text.is_empty());
}

#[test]
fn peek_read_only() {
    let _g = gate();
    let dir = scratch("ro");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::prove_up();
    let _ = fs::write(&log, "");
    peek::exercise_open("t1", 60_000);
    peek::hover_get("t1");
    let text = fs::read_to_string(&log).unwrap();
    let allowed = ["ls", "get", "read", "events"];
    let banned = ["send", "keys", "attach", "interrupt", "put", "kill", "rm", "run"];
    for line in text.lines().filter(|l| !l.is_empty()) {
        let cmd = line.split_whitespace().next().unwrap();
        assert!(allowed.contains(&cmd), "{line}");
        assert!(!banned.contains(&cmd), "{line}");
    }
    assert!(text.lines().any(|l| l.starts_with("read")), "{text}");
    let src = include_str!("peek.rs");
    for bad in [
        "\"send\"",
        "\"keys\"",
        "\"attach\"",
        "\"interrupt\"",
        "\"put\"",
        "\"kill\"",
        "\"rm\"",
        "\"run\"",
    ] {
        assert!(!src.contains(bad), "{bad}");
    }
}

#[test]
fn peek_strips_ansi() {
    let raw = "plain\n\u{1b}[31mred\u{1b}[0m\n\u{1b}]7501;working\u{7}\n\u{1b}]8;;http://example.test\u{7}link\u{1b}]8;;\u{7}\n<b>tag</b>\n";
    let out = peek::strip_terminal(raw);
    assert!(!out.contains('\u{1b}'), "{out}");
    assert!(!out.contains('<'), "{out}");
    assert!(out.contains("plain"));
    assert!(out.contains("red"));
    assert!(out.contains("link"));
    assert!(out.contains("&lt;b&gt;tag&lt;/b&gt;"), "{out}");
    assert!(!out.contains("7501"));
    assert!(!out.contains("http://example.test"));
}

#[test]
fn read_gate_script_passes() {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-no-tui-herdr.sh");
    let out = Command::new("sh").arg(&script).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn logging_herdr(dir: &Path, log: &Path) -> PathBuf {
    let bin = dir.join("herdr");
    let script = format!("#!/bin/sh\necho \"$@\" >> \"{}\"\necho '{{}}'\nexit 0\n", log.display());
    fs::write(&bin, script).unwrap();
    let mut perms = fs::metadata(&bin).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&bin, perms).unwrap();
    bin
}

fn core_rec(terminal: &str) -> crate::state::AgentRunHerdr {
    crate::state::AgentRunHerdr {
        backend: "core".into(),
        terminal: terminal.into(),
        agent_name: "c1".into(),
        pane: "should-not-close".into(),
        workspace: "should-not-close".into(),
        ..Default::default()
    }
}

#[test]
fn tui_grep_gate_flag_on() {
    let _g = gate();
    let dir = scratch("tui-gate");
    let _c = Clean(dir.clone());
    let hlog = dir.join("herdr.log");
    let clog = dir.join("hc.log");
    let herdr = logging_herdr(&dir, &hlog);
    tui::set_bin_override(Some(herdr));
    arm("live", &clog);
    health::prove_up();
    fs::write(&hlog, "").unwrap();
    fs::write(&clog, "").unwrap();
    let bin = hc_path();
    let addr = client::launch_core(&bin, "/tmp/orbitty-golden", "c1", "p1", "grok", &[]).unwrap();
    assert_eq!(addr, "t1");
    let rec = core_rec(&addr);
    client::prompt_with(&bin, &rec, "hello").unwrap();
    client::wait_run(&bin, &rec, None).unwrap();
    client::stop(&rec);
    client::focus(&rec);
    let _ = crate::droid_inspect::tool_line(&rec);
    let hers = fs::read_to_string(&hlog).unwrap_or_default();
    assert!(hers.is_empty(), "flag on still called herdr: {hers}");
    let hc = fs::read_to_string(&clog).unwrap();
    assert!(hc.contains("run --bg"), "{hc}");
    assert!(!hc.contains("pane close") && !hc.contains("workspace"), "{hc}");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-no-tui-herdr.sh");
    let out = Command::new("sh").arg(&script).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn launch_via_hc_run_bg_labels() {
    let _g = gate();
    let dir = scratch("run");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    let addr = core::launch_once(
        &hc_path(),
        "/tmp/orbitty-golden",
        "c1",
        "p1",
        "grok",
        &[],
    )
    .unwrap();
    assert_eq!(addr, "t1");
    let got = fs::read_to_string(&log).unwrap();
    assert_eq!(
        got,
        "run --bg --json --cwd /tmp/orbitty-golden --label orbitty=1 --label card=c1 --label planet=p1 -- grok\n"
    );
}

#[test]
fn prompt_via_hc_send() {
    let _g = gate();
    let dir = scratch("send");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    core::send(&hc_path(), "t1", "hello").unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), "send --json t1 hello\n");
}

#[test]
fn wait_via_hc_wait() {
    let _g = gate();
    let dir = scratch("wait");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    core::wait(&hc_path(), "t1").unwrap();
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "wait --json --timeout 60 t1\n"
    );
}

#[test]
fn stop_kills_then_rm() {
    let _g = gate();
    let dir = scratch("stop");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    core::stop(&hc_path(), "t1");
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "kill --json t1\nrm --json t1\n"
    );
}

#[test]
fn droid_read_via_hc_read() {
    let _g = gate();
    let dir = scratch("read");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    let line = crate::droid_inspect::tool_line(&core_rec("t1"));
    assert!(line.is_some(), "fixture text should yield a line");
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "read --json --lines 16 t1\n"
    );
}

#[test]
fn run_sticks_to_its_backend() {
    let _g = gate();
    let dir = scratch("stick");
    let _c = Clean(dir.clone());
    let hlog = dir.join("herdr.log");
    let clog = dir.join("hc.log");
    let herdr = logging_herdr(&dir, &hlog);
    tui::set_bin_override(Some(herdr));
    arm("live", &clog);

    // Core run, flag now off: still kill then rm. No TUI close.
    client::force_flag(Some(false));
    client::stop(&core_rec("t1"));
    assert_eq!(
        fs::read_to_string(&clog).unwrap(),
        "kill --json t1\nrm --json t1\n"
    );
    assert!(fs::read_to_string(&hlog).unwrap_or_default().is_empty());

    // TUI run, flag now on: still pane close + workspace close. No hc kill.
    fs::write(&clog, "").unwrap();
    fs::write(&hlog, "").unwrap();
    client::force_flag(Some(true));
    core::set_bin_override(Some(hc_path()));
    let tui_rec = crate::state::AgentRunHerdr {
        backend: "tui".into(),
        pane: "p9".into(),
        workspace: "w9".into(),
        agent_name: "c9".into(),
        ..Default::default()
    };
    client::stop(&tui_rec);
    assert_eq!(
        fs::read_to_string(&hlog).unwrap(),
        "pane close p9\nworkspace close w9\n"
    );
    assert!(fs::read_to_string(&clog).unwrap_or_default().is_empty());
}

#[test]
fn old_save_loads_as_tui() {
    let raw = r#"{"planetId":"p","workspace":"w","tab":"tb","pane":"pn","agentName":"a","kind":"grok","mystery":"kept"}"#;
    let h: crate::state::AgentRunHerdr = serde_json::from_str(raw).unwrap();
    assert_eq!(h.backend, "tui");
    assert!(h.terminal.is_empty());
    let back = serde_json::to_value(&h).unwrap();
    assert_eq!(back["backend"], "tui");
    assert_eq!(back["mystery"], "kept");
    assert_eq!(back["agentName"], "a");
    assert!(back.get("terminal").is_none());
}

#[test]
fn hc_missing_launch_uses_fallback_launcher() {
    let _g = gate();
    let dir = scratch("miss-launch");
    let _c = Clean(dir.clone());
    let hlog = dir.join("herdr.log");
    let herdr = logging_herdr(&dir, &hlog);
    client::force_flag(Some(true));
    core::set_bin_override(None);
    tui::set_bin_override(Some(herdr));
    let err = crate::habitat::herdr::require_bin().unwrap_err();
    let line = crate::habitat::herdr::missing_launcher_line();
    assert_eq!(err, line);
    assert!(line.contains("herdr not on PATH"), "{line}");
    assert!(line.contains("the old launcher fires"), "{line}");
    assert!(fs::read_to_string(&hlog).unwrap_or_default().is_empty());
}

#[test]
fn hc_crash_during_launch_no_duplicate() {
    let _g = gate();
    let dir = scratch("die-launch");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let stamp = dir.join("stamp");
    arm("live", &log);
    health::prove_up();
    client::set_child_env(vec![
        ("FAKE_HC_MODE".into(), "die_after_run".into()),
        ("FAKE_HC_LOG".into(), log.display().to_string()),
        ("FAKE_HC_STAMP".into(), stamp.display().to_string()),
    ]);
    fs::write(&log, "").unwrap();
    let addr = core::launch_once(&hc_path(), "/tmp/orbitty-golden", "c1", "p1", "grok", &[]).unwrap();
    assert_eq!(addr, "t1");
    let text = fs::read_to_string(&log).unwrap();
    let runs = text.lines().filter(|l| l.starts_with("run ")).count();
    assert_eq!(runs, 1, "second launch after the id:\n{text}");
    assert_eq!(client::alive("t1"), None, "dead core is unknown, not died");
    let text = fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().filter(|l| l.starts_with("run ")).count(), 1, "{text}");
    arm("live", &log);
    health::prove_up();
    assert_eq!(client::alive("t1"), Some(true));
}

fn herdr_run(card: &str, backend: &str, terminal: &str, agent_name: &str) -> crate::state::AgentRun {
    crate::state::AgentRun {
        card_id: card.into(),
        planet_id: "p".into(),
        agent_shortcut_id: "a".into(),
        state: "working".into(),
        herdr: Some(crate::state::AgentRunHerdr {
            planet_id: "p".into(),
            workspace: "w".into(),
            tab: "tb".into(),
            pane: "p9".into(),
            agent_name: agent_name.into(),
            kind: "grok".into(),
            backend: backend.into(),
            terminal: terminal.into(),
            extra: Default::default(),
        }),
    }
}

#[test]
fn peek_core_run_reads_terminal_not_card() {
    let run = herdr_run("card-9", "core", "t1", "card-9");
    let (use_core, target) = peek::resolve_peek(std::slice::from_ref(&run), "card-9", false);
    assert!(use_core, "a core run is read through hc even when the flag is off");
    assert_eq!(target, "t1");
    assert_ne!(target, "card-9");
    let empty = herdr_run("card-9", "core", "  ", "card-9");
    let (use_core, target) = peek::resolve_peek(std::slice::from_ref(&empty), "card-9", false);
    assert!(!use_core && target == "card-9", "an empty terminal falls back, it is not sent to hc");
    let (use_core, target) = peek::resolve_peek(&[], "card-9", true);
    assert!(use_core && target == "card-9", "no row keeps today's flag path");
    let src = include_str!("peek.rs");
    let start = src.find("pub(crate) fn view(").expect("view");
    let body = &src[start..start + src[start..].find("\n}\n").expect("view end")];
    assert!(body.contains("resolve_peek("), "view follows the run row");
}

#[test]
fn peek_tui_run_ignores_flag() {
    let run = herdr_run("card-9", "tui", "", "card-9");
    let (use_core, target) = peek::resolve_peek(std::slice::from_ref(&run), "card-9", true);
    assert!(!use_core, "a tui run stays on the pane path when the core flag is on");
    assert_eq!(target, "card-9");
    let named = herdr_run("card-9", "tui", "t1", "seat-name");
    let (use_core, target) = peek::resolve_peek(std::slice::from_ref(&named), "card-9", true);
    assert!(!use_core);
    assert_eq!(target, "seat-name");
}

#[test]
fn grok_argv_read_only_is_plan_mode() {
    let extra = crate::agents::grok_permission_args("read-only");
    let argv = tui::start_argv("c-1", "grok", "p1", &extra);
    assert!(
        argv.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "plan"),
        "{argv:?}"
    );
    assert!(!argv.iter().any(|a| a.contains("always-approve")), "{argv:?}");
    let ww = crate::agents::grok_permission_args("workspace-write");
    let argv = tui::start_argv("c-1", "grok", "p1", &ww);
    assert!(
        argv.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "acceptEdits"),
        "{argv:?}"
    );
    let fa = crate::agents::grok_permission_args("full-auto");
    let argv = tui::start_argv("c-1", "grok", "p1", &fa);
    assert!(!argv.iter().any(|a| a == "--permission-mode" || a.contains("always-approve")), "{argv:?}");
    let src = include_str!("../habitat/herdr.rs");
    assert!(src.contains("grok_permission_args"), "both herdr doors take the grok extra");
    let core = include_str!("core.rs");
    assert!(core.contains("args.extend(extra"), "hc run forwards the same extra");
}

fn core_seat(card: &str, terminal: &str) -> crate::state::AgentRun {
    herdr_run(card, "core", terminal, card)
}

#[test]
fn peek_reports_herdr_on_first_call_after_start() {
    let _g = gate();
    let dir = scratch("peek-first");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::reset();
    assert!(health::is_down(), "cold start is down until the first sample");
    let run = core_seat("card-9", "t2");
    let v = peek::view(std::slice::from_ref(&run), "card-9", 40, true);
    assert_eq!(v.source, "herdr", "{v:?}");
    assert!(v.up, "{v:?}");
    assert!(!v.text.is_empty(), "{v:?}");
}

#[test]
fn status_reports_herdr_on_first_call_after_start() {
    let _g = gate();
    let dir = scratch("status-first");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::reset();
    let st = client::status_value();
    assert_eq!(st["up"], true, "{st}");
    assert_eq!(st["source"], "herdr", "{st}");
    assert_eq!(st["version"], "0.1.14", "{st}");
}

#[test]
fn status_and_peek_agree_when_faulted() {
    let _g = gate();
    let _c = Clean(scratch("agree"));
    client::force_flag(Some(true));
    core::set_bin_override(Some(PathBuf::from("/tmp/orbitty-herdr-fault-held")));
    health::reset();
    health::fault_held(5_000);
    health::set_clock_ms(Some(5_000));
    let st = client::status_value();
    assert_eq!(st["up"], false, "{st}");
    assert_eq!(st["source"], "fallback", "{st}");
    let run = core_seat("card-9", "t2");
    let v = peek::view(std::slice::from_ref(&run), "card-9", 40, true);
    assert!(!v.up, "{v:?}");
    assert_eq!(v.source, "fallback", "{v:?}");
    assert_eq!(v.status, "unknown", "{v:?}");
}

#[test]
fn peek_recovers_without_status_call() {
    let _g = gate();
    let dir = scratch("peek-recover");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("socket", &log);
    health::reset();
    health::assume_up();
    health::set_clock_ms(Some(10_000));
    let run = core_seat("card-9", "t2");
    let down = peek::view(std::slice::from_ref(&run), "card-9", 40, true);
    assert_eq!(down.source, "fallback", "{down:?}");
    assert!(!down.up, "one failed read forces fallback");
    arm("live", &log);
    health::set_clock_ms(Some(10_000 + 30_000));
    let back = peek::view(std::slice::from_ref(&run), "card-9", 40, true);
    assert_eq!(back.source, "herdr", "{back:?}");
    assert!(back.up, "{back:?}");
    assert!(!back.text.is_empty(), "{back:?}");
}

#[test]
fn force_fallback_logs_kind() {
    let _g = gate();
    let dir = scratch("kind");
    let _c = Clean(dir.clone());
    let script = dir.join("hc");
    fs::write(
        &script,
        "#!/bin/sh\ncase \" $* \" in\n  *\" read \"*)\n    echo '{\"error\":{\"kind\":\"timeout\",\"message\":\"read\"}}' >&2\n    exit 1\n    ;;\nesac\necho '{\"address\":\"t2\",\"state\":\"live\",\"status\":\"working\"}'\nexit 0\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    client::force_flag(Some(true));
    core::set_bin_override(Some(script));
    health::reset();
    health::assume_up();
    health::set_clock_ms(Some(2_000));
    let _ = health::take_unlogged();
    let run = core_seat("card-9", "t2");
    let v = peek::view(std::slice::from_ref(&run), "card-9", 40, true);
    assert_eq!(v.source, "fallback", "{v:?}");
    let notes = health::take_unlogged();
    assert!(
        notes.iter().any(|n| n.source == "fallback" && n.kind == "timeout"),
        "{notes:?}"
    );
    let flush = include_str!("mod.rs");
    assert!(flush.contains("\"kind\": ev.kind"), "herdr-health event carries kind");
}

#[test]
fn flag_off_peek_and_status_unchanged() {
    let _g = gate();
    let dir = scratch("flag-off");
    let _c = Clean(dir.clone());
    let script = dir.join("herdr");
    fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = agent ] && [ \"$2\" = get ]; then\n  echo '{\"result\":{\"agent\":{\"agent_status\":\"working\"}}}'\n  exit 0\nfi\nif [ \"$1\" = agent ] && [ \"$2\" = read ]; then\n  printf '%s\n' 'seat screen'\n  exit 0\nfi\necho '{}'\nexit 0\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    tui::set_bin_override(Some(script));
    client::force_flag(Some(false));
    health::reset();
    let st = client::status_value();
    assert_eq!(st["source"], "tui", "{st}");
    assert_eq!(st["version"], "", "{st}");
    assert_eq!(st["up"], true, "{st}");
    assert_eq!(st["status"], "none", "{st}");
    let v = peek::view(&[], "card-9", 40, true);
    assert_eq!(v.source, "tui", "{v:?}");
    assert!(v.up, "{v:?}");
    assert!(v.text.contains("seat screen"), "{v:?}");
}

#[test]
fn grok_trust_on_all_three_doors() {
    let _g = gate();
    let dir = scratch("trust-doors");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::prove_up();
    let extra = crate::agents::grok_permission_args("read-only");
    let argv = tui::start_argv("c-1", "grok", "p1", &extra);
    assert!(argv.iter().any(|a| a == "--trust"), "tui door: {argv:?}");
    let _ = fs::write(&log, "");
    let addr = client::launch_core(&hc_path(), "/tmp/orbitty-golden", "c1", "p1", "grok", &extra).unwrap();
    assert_eq!(addr, "t1");
    let text = fs::read_to_string(&log).unwrap();
    assert!(text.contains("--trust"), "hc run door:\n{text}");
    for preset in ["read-only", "workspace-write", "full-auto"] {
        let line = crate::agents::grok_exec_line_for("/usr/bin/grok", preset);
        assert!(line.contains("'--trust'"), "{preset}: {line}");
        assert!(!line.contains("always-approve"), "{line}");
    }
    let habitat = include_str!("../habitat/herdr.rs");
    assert!(habitat.contains("grok_permission_args"), "both herdr doors take the grok extra");
}

fn write_boot_script_body(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

fn write_boot_script(path: &Path) {
    write_boot_script_body(
        path,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HC_LOG\"\nif [ \"$1\" = server ] && [ \"$2\" = start ]; then\n  : > \"$HC_LOG.ready\"\n  exit 0\nfi\nif [ \"$1\" = server ] && [ \"$2\" = status ]; then\n  if [ \"$HC_UP\" = 1 ] || [ -f \"$HC_LOG.ready\" ]; then\n    printf '%s\\n' '{\"version\":\"0.1.14\"}'\n    exit 0\n  fi\n  printf '%s\\n' '{\"error\":{\"kind\":\"unavailable\",\"message\":\"down\"}}' >&2\n  exit 1\nfi\nexit 0\n",
    );
}

fn arm_boot(path: &Path, log: &Path, up: bool) {
    client::reset_overrides();
    client::force_flag(Some(true));
    core::set_bin_override(Some(path.to_path_buf()));
    client::set_child_env(vec![
        ("HC_LOG".into(), log.display().to_string()),
        ("HC_UP".into(), if up { "1".into() } else { "0".into() }),
    ]);
}

fn wait_log_contains(log: &Path, needle: &str) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let text = fs::read_to_string(log).unwrap_or_default();
        if text.contains(needle) || std::time::Instant::now() >= deadline {
            return text;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn between<'a>(src: &'a str, start: &str, end: &str) -> &'a str {
    let at = src.find(start).unwrap_or_else(|| panic!("missing {start}"));
    let rest = &src[at..];
    let end_at = rest.find(end).unwrap_or_else(|| panic!("missing {end} after {start}"));
    &rest[..end_at]
}

#[test]
fn boot_check_one_sample_cold_up() {
    let _g = gate();
    let dir = scratch("boot-up");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    arm("live", &log);
    health::reset();
    health::set_clock_ms(Some(1_000));
    let _ = fs::write(&log, "");
    health::boot_check(None);
    assert_eq!(health::source(), Source::Herdr);
    assert!(!health::is_down());
    assert_eq!(health::events(), vec!["herdr"]);
    let text = fs::read_to_string(&log).unwrap();
    assert_eq!(
        text.lines().filter(|l| l.starts_with("server status")).count(),
        1,
        "{text}"
    );
    assert_eq!(text.lines().filter(|l| l.starts_with("events")).count(), 1, "{text}");
    health::boot_check(None);
    let text = fs::read_to_string(&log).unwrap();
    assert_eq!(
        text.lines().filter(|l| l.starts_with("server status")).count(),
        1,
        "second boot_check sampled again:\n{text}"
    );
    assert_eq!(health::events(), vec!["herdr"]);
    let src = include_str!("health.rs");
    assert!(src.contains("const RECHECK_MS: u64 = 30_000;"));
    assert!(src.contains("let need = if cold { 1 } else { HEALTHY_NEED };"));
    let at = src.find("pub(crate) fn boot_check").expect("boot_check");
    let body = &src[at..];
    assert!(body.contains("herdr-health"), "{body}");
    assert!(body.contains("append_best"), "{body}");
}

#[test]
fn boot_check_missing_bin_fallback_event() {
    let _g = gate();
    let _c = Clean(scratch("boot-miss"));
    client::force_flag(Some(true));
    core::set_bin_override(None);
    health::reset();
    health::set_clock_ms(Some(2_000));
    health::boot_check(None);
    assert_eq!(health::source(), Source::Fallback);
    assert!(health::is_down());
    assert_eq!(health::events(), vec!["fallback"]);
    let notes = health::take_unlogged();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].source, "fallback");
    assert_eq!(notes[0].kind, "transport");
    health::boot_check(None);
    assert!(health::take_unlogged().is_empty(), "boot_check sampled twice");
}

#[test]
fn boot_server_only_for_bundled_sibling() {
    let _g = gate();
    let dir = scratch("boot-bundle");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let bin = dir.join("Nightly.app/Contents/MacOS/herdr-core");
    write_boot_script(&bin);
    arm_boot(&bin, &log, false);
    client::boot_server();
    let text = wait_log_contains(&log, "server start");
    assert_eq!(
        text.lines().filter(|l| l.contains("server start")).count(),
        1,
        "{text}"
    );
    assert!(text.contains("server status"), "{text}");
    let _ = fs::write(&log, "");
    client::force_flag(Some(false));
    client::boot_server();
    let off = fs::read_to_string(&log).unwrap_or_default();
    assert!(off.is_empty(), "flag off started a server: {off}");
    client::force_flag(Some(true));
    let bad = dir.join("Other.app/Contents/MacOS/herdr-core");
    fs::create_dir_all(bad.parent().unwrap()).unwrap();
    fs::write(&bad, "not a binary").unwrap();
    let mut perms = fs::metadata(&bad).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o644);
    fs::set_permissions(&bad, perms).unwrap();
    core::set_bin_override(Some(bad));
    health::reset();
    client::boot_server();
    let notes = health::take_unlogged();
    assert!(
        notes.iter().any(|n| n.source == "fallback"),
        "start failure wrote no fallback event: {notes:?}"
    );
}

#[test]
fn boot_server_noop_when_status_ok() {
    let _g = gate();
    let dir = scratch("boot-ok");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let bin = dir.join("App.app/Contents/MacOS/herdr-core");
    write_boot_script(&bin);
    arm_boot(&bin, &log, true);
    client::boot_server();
    // Start is a detached spawn. Read the log the way the bundled-start
    // test does, or a start-when-status-ok mutant still looks clean.
    let text = wait_log_contains(&log, "server start");
    assert!(text.contains("server status"), "{text}");
    assert!(!text.contains("server start"), "{text}");
    let src = include_str!("client.rs");
    let boot = between(src, "pub(crate) fn boot_server", "pub(crate) fn alive");
    let early = boot.find("is_ok()").expect("status ok return");
    let spawn = boot.find("[\"server\", \"start\"]").expect("start");
    assert!(early < spawn, "is_ok return must stay before start");
}

#[test]
fn boot_server_waits_until_status_ok_then_one_up_sample() {
    let _g = gate();
    let dir = scratch("boot-wait");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let bin = dir.join("Nightly.app/Contents/MacOS/herdr-core");
    write_boot_script_body(
        &bin,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HC_LOG\"\n\
if [ \"$1\" = server ] && [ \"$2\" = start ]; then\n  sleep 0.4\n  : > \"$HC_LOG.ready\"\n  exit 0\nfi\n\
if [ \"$1\" = server ] && [ \"$2\" = status ]; then\n  \
if [ -f \"$HC_LOG.ready\" ]; then\n    printf '%s\\n' 'status-ok' >> \"$HC_LOG\"\n    \
printf '%s\\n' '{\"version\":\"0.1.14\"}'\n    exit 0\n  fi\n  \
printf '%s\\n' 'status-down' >> \"$HC_LOG\"\n  \
printf '%s\\n' '{\"error\":{\"kind\":\"unavailable\",\"message\":\"down\"}}' >&2\n  exit 1\nfi\n\
printf 'x'\nexit 0\n",
    );
    arm_boot(&bin, &log, false);
    health::reset();
    let started = Instant::now();
    client::boot_server();
    let elapsed = started.elapsed();
    let text = fs::read_to_string(&log).unwrap_or_default();
    let start_at = text.find("server start").expect(&text);
    let ok_at = text.find("status-ok").unwrap_or_else(|| panic!("returned before status was ok:\n{text}"));
    assert!(start_at < ok_at, "status ok before start:\n{text}");
    assert!(
        elapsed >= Duration::from_millis(300),
        "boot_server returned before the delayed ready file: {elapsed:?}\n{text}"
    );
    assert!(health::events().is_empty(), "wait took a health sample: {:?}", health::events());
    health::set_clock_ms(Some(5_000));
    health::boot_check(None);
    assert_eq!(health::source(), Source::Herdr);
    assert!(!health::is_down());
    assert_eq!(health::events(), vec!["herdr"]);
    let after = fs::read_to_string(&log).unwrap_or_default();
    let ok_lines = after.lines().filter(|l| *l == "status-ok").count();
    health::boot_check(None);
    let again = fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(
        again.lines().filter(|l| *l == "status-ok").count(),
        ok_lines,
        "second boot_check sampled again:\n{again}"
    );
    assert_eq!(health::events(), vec!["herdr"]);
}

#[test]
fn boot_server_bound_expiry_one_fallback_sample() {
    let _g = gate();
    let dir = scratch("boot-bound");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let bin = dir.join("Nightly.app/Contents/MacOS/herdr-core");
    write_boot_script_body(
        &bin,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HC_LOG\"\n\
if [ \"$1\" = server ] && [ \"$2\" = status ]; then\n  \
printf '%s\\n' '{\"error\":{\"kind\":\"unavailable\",\"message\":\"down\"}}' >&2\n  exit 1\nfi\n\
printf 'x'\nexit 0\n",
    );
    arm_boot(&bin, &log, false);
    health::reset();
    health::set_clock_ms(Some(8_000));
    let started = Instant::now();
    client::boot_server();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(client::START_READY_MS),
        "bound not waited: {elapsed:?}"
    );
    assert!(health::events().is_empty(), "bound wait sampled: {:?}", health::events());
    let text = fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(
        text.lines().filter(|l| l.contains("server start")).count(),
        1,
        "{text}"
    );
    health::boot_check(None);
    assert_eq!(health::source(), Source::Fallback);
    assert!(health::is_down());
    let notes = health::take_unlogged();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].source, "fallback");
    assert_eq!(notes[0].kind, "status");
    health::boot_check(None);
    assert!(health::take_unlogged().is_empty(), "second boot_check sampled");
}

#[test]
fn boot_server_never_for_local_bin_or_override() {
    let _g = gate();
    let dir = scratch("boot-rig");
    let _c = Clean(dir.clone());
    let log = dir.join("hc.log");
    let _ = fs::write(&log, "");
    let local = dir.join(".local/bin/herdr-core");
    write_boot_script(&local);
    arm_boot(&local, &log, false);
    client::boot_server();
    let text = fs::read_to_string(&log).unwrap_or_default();
    assert!(text.is_empty(), "local bin was touched: {text}");
    let bundled = dir.join("Nightly.app/Contents/MacOS/herdr-core");
    write_boot_script(&bundled);
    arm_boot(&bundled, &log, false);
    client::force_located_override(Some(bundled.clone()));
    client::boot_server();
    let text = fs::read_to_string(&log).unwrap_or_default();
    assert!(text.is_empty(), "ORBITTY_HC override was touched: {text}");
}

#[test]
fn ensure_server_status_only_scan() {
    let src = include_str!("client.rs");
    let ensure = between(src, "pub(crate) fn ensure_server", "fn found_via_override");
    assert!(!ensure.contains("server start"), "{ensure}");
    assert!(!ensure.contains("[\"server\", \"start\"]"), "{ensure}");
    assert!(ensure.contains("server_status"), "{ensure}");
    assert!(!ensure.contains("boot_server"), "{ensure}");
    let boot = between(src, "pub(crate) fn boot_server", "pub(crate) fn alive");
    assert!(boot.contains("server start"), "{boot}");
    assert!(boot.contains("[\"server\", \"start\"]"), "{boot}");
    assert_eq!(src.matches("[\"server\", \"start\"]").count(), 1);
    let lib = include_str!("../lib.rs");
    let at = lib.find("health::boot_check").expect("boot_check spawn");
    let window = &lib[at.saturating_sub(500)..at];
    assert!(window.contains("std::thread::spawn"), "{window}");
    assert!(window.contains("client::boot_server"), "{window}");
}

#[test]
fn missing_core_line_plain_text() {
    let _g = gate();
    let _c = Clean(scratch("missing-line"));
    client::force_flag(Some(true));
    core::set_bin_override(None);
    health::reset();
    let line = client::missing_core_line();
    assert_eq!(
        line,
        "Agent terminals are off: herdr-core isn't installed. Orbitty will use its own launcher."
    );
    for ban in ["rail", "hc", "fallback"] {
        assert!(!line.to_ascii_lowercase().contains(ban), "{ban} in {line}");
    }
    let st = client::status_value();
    assert_eq!(st["line"], line, "{st}");
    assert_eq!(st["up"], false, "{st}");
    let v = peek::view(&[], "card-9", 40, true);
    assert_eq!(v.status, line, "{v:?}");
    assert!(v.text.is_empty(), "{v:?}");
    for ban in ["rail", "hc", "fallback"] {
        assert!(!v.status.to_ascii_lowercase().contains(ban), "{v:?}");
    }
    let peek_src = include_str!("peek.rs");
    assert!(peek_src.contains("missing_core_line"));
    let print_src = include_str!("../bin/orbittyctl/imp/print.rs");
    assert!(print_src.contains("data[\"line\"]"));
    client::force_flag(Some(false));
    health::reset();
    let off = client::status_value();
    assert_eq!(off["source"], "tui", "{off}");
    assert!(off.get("line").is_none(), "{off}");
}
