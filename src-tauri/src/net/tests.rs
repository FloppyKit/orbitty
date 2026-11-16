//! Door tests. Pure file fixtures under the temp dir; nothing here connects.

use super::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(label: &str) -> (PathBuf, Scratch) {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("orbitty-net-{label}-{n}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Scratch(dir))
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn listed(dir: &Path, hosts: &[&str]) {
    let mut list = AllowList::empty();
    for h in hosts {
        list.add(h).unwrap();
    }
    write_allow(dir, &list).unwrap();
}

#[test]
fn flag_off_denies_a_listed_host() {
    let (dir, _keep) = scratch("flag-off");
    listed(&dir, &["a.example"]);
    let v = permit_at(&dir, false, "a.example", "test");
    assert!(!v.allow);
    assert_eq!(v.reason, "net_guard is off (flag net_guard)");
}

#[test]
fn flag_on_empty_allow_denies() {
    let (dir, _keep) = scratch("empty-allow");
    let v = permit_at(&dir, true, "a.example", "test");
    assert!(!v.allow);
    assert_eq!(v.reason, "a.example is not in net-allow.json");
    // A missing file is the same door: deny.
    assert!(read_allow(&dir).unwrap().is_empty());
}

#[test]
fn flag_on_listed_allows_that_host_only() {
    let (dir, _keep) = scratch("listed");
    listed(&dir, &["a.example"]);
    let v = permit_at(&dir, true, "a.example", "test");
    assert!(v.allow, "{}", v.reason);
    assert_eq!(
        v.reason,
        "a.example is listed in net-allow.json (purpose: test)"
    );
    // Case and surrounding space fold onto the stored entry.
    assert!(permit_at(&dir, true, " A.Example ", "test").allow);
}

#[test]
fn kill_file_denies_the_listed_host() {
    let (dir, _keep) = scratch("kill");
    listed(&dir, &["a.example"]);
    assert!(permit_at(&dir, true, "a.example", "test").allow);
    kill_on(&dir).unwrap();
    assert!(kill_set(&dir));
    let v = permit_at(&dir, true, "a.example", "test");
    assert!(!v.allow);
    assert_eq!(v.reason, "kill file is set (net-kill)");
    // Removing the file is the only clear.
    kill_off(&dir).unwrap();
    assert!(!kill_set(&dir));
    assert!(permit_at(&dir, true, "a.example", "test").allow);
    // Clearing twice is not an error.
    kill_off(&dir).unwrap();
}

#[test]
fn a_second_unlisted_host_is_denied() {
    let (dir, _keep) = scratch("second");
    listed(&dir, &["a.example"]);
    assert!(permit_at(&dir, true, "a.example", "test").allow);
    let v = permit_at(&dir, true, "b.example", "test");
    assert!(!v.allow);
    assert_eq!(v.reason, "b.example is not in net-allow.json");
}

#[test]
fn first_create_is_an_empty_list() {
    let (dir, _keep) = scratch("create");
    write_allow(&dir, &AllowList::empty()).unwrap();
    let text = fs::read_to_string(allow_path(&dir)).unwrap();
    assert!(text.contains("\"hosts\": []"), "{text}");
    assert_eq!(read_allow(&dir).unwrap().len(), 0);
    // The file carries host names only — no secret-shaped keys.
    assert!(!text.contains("token") && !text.contains("secret") && !text.contains("key"));
}

#[test]
fn allow_add_and_del_round_trip() {
    let (dir, _keep) = scratch("round-trip");
    assert!(read_allow(&dir).unwrap().is_empty());
    let mut list = read_allow(&dir).unwrap();
    assert!(list.add("a.example").unwrap());
    assert!(!list.add("a.example").unwrap(), "duplicate is a no-op");
    assert!(list.remove("a.example").unwrap());
    assert!(!list.remove("a.example").unwrap());
    assert!(list.is_empty());
}

#[test]
fn host_shapes_that_are_not_a_host_are_refused() {
    for bad in [
        "",
        "   ",
        "*",
        "*.example",
        "a.example/path",
        "user:pass@a.example",
        "a.example:443",
        "https://a.example",
        "a b.example",
        "-a.example",
        "a.example..b",
    ] {
        assert!(host_ok(bad).is_err(), "{bad:?} must be refused");
    }
    assert_eq!(host_ok(" A.Example ").unwrap(), "a.example");
    assert_eq!(host_ok("127.0.0.1").unwrap(), "127.0.0.1");
    let src = include_str!("mod.rs");
    assert!(!src.contains('*'), "no wildcard in the door source");
}

#[test]
fn corrupt_allow_denies_and_status_says_so() {
    let (dir, _keep) = scratch("corrupt");
    fs::write(allow_path(&dir), b"{ not json").unwrap();
    let v = permit_at(&dir, true, "a.example", "test");
    assert!(!v.allow);
    assert!(v.reason.contains("corrupt"), "{}", v.reason);
    let st = status(&dir, true);
    assert!(st["allow"].is_null());
    assert!(st["allowError"].as_str().unwrap().contains("corrupt"));
    // A wrong version is corrupt too — never a silent empty allow list.
    fs::write(allow_path(&dir), br#"{"v":2,"hosts":[]}"#).unwrap();
    assert!(read_allow(&dir).is_err());
}

#[test]
fn status_reports_flag_allow_and_kill() {
    let (dir, _keep) = scratch("status");
    let off = status(&dir, false);
    assert_eq!(off["flag"], "net_guard");
    assert_eq!(off["on"], false);
    assert_eq!(off["allow"], 0);
    assert_eq!(off["kill"], false);
    listed(&dir, &["a.example", "b.example"]);
    kill_on(&dir).unwrap();
    let on = status(&dir, true);
    assert_eq!(on["on"], true);
    assert_eq!(on["allow"], 2);
    assert_eq!(on["hosts"], json!(["a.example", "b.example"]));
    assert_eq!(on["kill"], true);
    assert!(on["allowFile"].as_str().unwrap().ends_with(ALLOW_FILE));
    assert!(on["killFile"].as_str().unwrap().ends_with(KILL_FILE));
}

#[test]
fn no_socket_no_thread_no_timer() {
    let src = include_str!("mod.rs");
    for banned in [
        "TcpStream",
        "UdpStream",
        "UnixStream",
        "std::net",
        "thread::spawn",
        "std::thread",
        "Instant::now",
        "SystemTime",
        "reqwest",
        "tungstenite",
        "tauri::",
        "use tauri",
    ] {
        assert!(!src.contains(banned), "net/mod.rs must not carry {banned}");
    }
    // No `connect` of any kind, in code or in a doc example.
    assert!(!src.contains("connect("), "no connect call");
}

#[test]
fn flag_row_is_off_manual_only_and_never_auto() {
    let flags = include_str!("../flags.rs");
    let at = flags
        .find("id: \"net_guard\",")
        .expect("net_guard row in FLAGS");
    let row = &flags[at..];
    let row = &row[..row.find("},").expect("row end")];
    assert!(row.contains("default: false"), "{row}");
    assert!(row.contains("auto_on_verify: false"), "{row}");
    assert!(row.contains("GO-ORBITTY-NET-GUARD"), "{row}");
    // It stays on the manual-only list, so no verify sweep flips it.
    let manual_at = flags.find("pub const MANUAL_ONLY").expect("MANUAL_ONLY");
    let manual = &flags[manual_at..];
    let manual = &manual[..manual.find("];").expect("MANUAL_ONLY end")];
    assert!(manual.contains("\"net_guard\""), "{manual}");
}
