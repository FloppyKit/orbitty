use super::{rows, run_fire, SHIMS};
use crate::ledger::{self, CountRow, TSV_HEADER};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("orbitty-shim-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn shim_fired_counts_once() {
    let dir = Scratch::new("once");
    run_fire(dir.path(), true, "ack_fallback", None, true);
    let counts = ledger::load_counts(&ledger::counts_path(dir.path()));
    assert_eq!(counts["ack_fallback"].total, 1);
    assert_eq!(counts["ack_fallback"].by_day.len(), 1);
    assert_eq!(counts["ack_fallback"].by_day.values().sum::<u64>(), 1);
    let fires = fs::read_to_string(ledger::fires_path(dir.path())).unwrap();
    let line: serde_json::Value = serde_json::from_str(fires.lines().next().unwrap()).unwrap();
    assert_eq!(line["count"], 1);
    assert_eq!(fires.lines().count(), 1);
    run_fire(dir.path(), true, "ack_fallback", None, true);
    let counts = ledger::load_counts(&ledger::counts_path(dir.path()));
    assert_eq!(counts["ack_fallback"].total, 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            ledger::counts_path(dir.path()),
            ledger::fires_path(dir.path()),
        ] {
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{}", path.display());
        }
    }
}

#[test]
fn shim_unknown_id() {
    let dir = Scratch::new("unknown");
    run_fire(dir.path(), true, "not-a-shim", None, false);
    let counts = ledger::load_counts(&ledger::counts_path(dir.path()));
    assert_eq!(counts.get("unknown").map(|c| c.total), Some(1));
    assert!(counts.get("not-a-shim").is_none());
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_fire(dir.path(), true, "not-a-shim", None, true);
    }));
    let err = caught.expect_err("debug panics on an unknown id");
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or("");
    assert!(msg.contains("unknown shim id not-a-shim"), "{msg}");
}

#[test]
fn shim_stale_rule_fixed_dates() {
    let today = "2026-10-30";
    let none = BTreeMap::new();
    assert_eq!(ledger::status("2026-10-27", &none, today), "quiet");
    assert_eq!(
        ledger::status("2026-10-16", &none, today),
        "delete_candidate"
    );
    assert_eq!(
        ledger::status("2026-10-01", &none, today),
        "delete_candidate"
    );
    let mut yesterday = BTreeMap::new();
    yesterday.insert("2026-10-29".to_string(), 1);
    assert_eq!(ledger::status("2026-10-01", &yesterday, today), "active");
    let mut edge = BTreeMap::new();
    edge.insert("2026-10-16".to_string(), 1);
    assert_eq!(
        ledger::status("2026-10-01", &edge, today),
        "delete_candidate"
    );
}

#[test]
fn tsv_header_exact() {
    let parsed = ledger::parse_shims(include_str!("shims.rs")).unwrap();
    assert_eq!(parsed, rows());
    assert_eq!(
        TSV_HEADER,
        "shim_id\tsymptom\tpatches\tfile\tadded\tfirst_fired\tlast_fired\tfires_7d\tfires_30d\tfires_total\tmodels_seen\tstatus"
    );
    let mut by = BTreeMap::new();
    by.insert("2026-10-29".to_string(), 2);
    by.insert("2026-10-01".to_string(), 5);
    let mut counts = BTreeMap::new();
    counts.insert(
        "ack_fallback".to_string(),
        CountRow {
            total: 7,
            first: "2026-10-01T00:00:00-05:00".into(),
            last: "2026-10-29T00:00:00-05:00".into(),
            by_day: by,
        },
    );
    let tsv = ledger::render_tsv(&parsed, &counts, "2026-10-30");
    let mut lines = tsv.lines();
    assert_eq!(lines.next().unwrap(), TSV_HEADER);
    let cols: Vec<&str> = TSV_HEADER.split('\t').collect();
    assert_eq!(cols.len(), 12);
    assert_eq!(cols[10], "models_seen");
    let ack = lines.next().unwrap();
    let fields: Vec<&str> = ack.split('\t').collect();
    assert_eq!(fields.len(), 12, "{ack}");
    assert_eq!(fields[0], "ack_fallback");
    assert_eq!(fields[7], "2");
    assert_eq!(fields[8], "7");
    assert_eq!(fields[9], "7");
    assert_eq!(fields[10], "");
    assert_eq!(fields[11], "active");
    for line in tsv.lines().skip(1) {
        assert_eq!(line.split('\t').count(), 12, "{line}");
        assert!(!line.contains("Users"));
    }
    for row in &parsed {
        assert!(row.note.chars().count() <= 80, "{}", row.id);
        assert!(!row.note.contains('\t') && !row.note.contains('"'));
    }
}

#[test]
fn fires_keyset_no_text() {
    let dir = Scratch::new("keys");
    run_fire(dir.path(), true, "goose_context_dead", Some("goose"), true);
    run_fire(dir.path(), true, "handoff_strip", None, true);
    let allowed: BTreeSet<&str> = ["ts", "id", "seat", "count"].into_iter().collect();
    let text = fs::read_to_string(ledger::fires_path(dir.path())).unwrap();
    let mut saw_seat = false;
    let mut saw_bare = false;
    for line in text.lines() {
        let v: serde_json::Map<String, serde_json::Value> = serde_json::from_str(line).unwrap();
        for key in v.keys() {
            assert!(allowed.contains(key.as_str()), "{key} in {line}");
        }
        assert!(v.contains_key("ts") && v.contains_key("id") && v.contains_key("count"));
        assert_eq!(v["count"], 1);
        for banned in ["text", "prompt", "args", "path", "lane", "model", "reply"] {
            assert!(!v.contains_key(banned), "{line}");
        }
        if v.get("seat").and_then(|s| s.as_str()) == Some("goose") {
            saw_seat = true;
        }
        if !v.contains_key("seat") {
            saw_bare = true;
        }
    }
    assert!(saw_seat && saw_bare);
}

#[test]
fn export_out_outside_home_refused() {
    let home = Scratch::new("home");
    let err = ledger::resolve_out(home.path(), "/tmp/orbitty-shim-ledger-out.tsv").unwrap_err();
    assert!(err.contains("HOME"), "{err}");
    let escaped = format!("{}/../orbitty-shim-escape.tsv", home.path().display());
    assert!(ledger::resolve_out(home.path(), &escaped).is_err());
    let dest = ledger::resolve_out(home.path(), "~/ok.tsv").unwrap();
    assert_eq!(dest, home.path().canonicalize().unwrap().join("ok.tsv"));
    let body = ledger::export_at(
        home.path(),
        home.path(),
        &rows(),
        Some("~/ok.tsv"),
        "2026-10-30",
    )
    .unwrap();
    assert!(body.starts_with(TSV_HEADER));
    let written = fs::read_to_string(home.path().join("ok.tsv")).unwrap();
    assert_eq!(written, body);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(home.path().join("ok.tsv"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn flag_off_noop_no_files() {
    let dir = Scratch::new("off");
    run_fire(dir.path(), false, "ack_fallback", Some("goose"), true);
    run_fire(dir.path(), false, "not-a-shim", None, true);
    assert!(!dir.path().join("ledger").exists());
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn shims_scan_both_ways() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut calls: BTreeSet<String> = BTreeSet::new();
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        calls.extend(fired_ids(&text));
    }
    let known: BTreeSet<&str> = SHIMS.iter().map(|s| s.id).collect();
    for id in &known {
        assert!(calls.contains(*id), "no call site for {id}");
    }
    for id in &calls {
        assert!(known.contains(id.as_str()), "fired id {id} is not in SHIMS");
    }
}

#[test]
fn goldens_unchanged() {
    let intercept = include_str!("coordinator/intercept.rs");
    assert!(intercept.contains("let stripped = crate::mouth_hop::strip_handoff_marks(them);"));
    let send = include_str!("coordinator/send.rs");
    assert!(send.contains("let reply = crate::mouth_hop::strip_handoff_marks(&reply);"));
    assert!(send.contains("let spoken = crate::mouth_hop::strip_handoff_marks(&spoken);"));
    let hop = include_str!("companion/mouth_hop/handoff.rs");
    assert_eq!(
        hop.matches("let answer = strip_handoff_marks(&answer);")
            .count(),
        2
    );
    let local = include_str!("companion/mouth_hop/local.rs");
    assert!(local.contains("let text = strip_handoff_marks(&t.text);"));
    let ack = include_str!("companion/mouth_ack.rs");
    assert!(ack.contains("let (v, a) = fallback_classify(line);"));
}

fn fired_ids(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let key = if line.contains("shims::fired(") {
            "shims::fired("
        } else if line.contains("shims::fired_bound(") {
            "shims::fired_bound("
        } else {
            continue;
        };
        let from = line.find(key).unwrap() + key.len();
        let mut window = line[from..].to_string();
        for extra in lines.iter().skip(i + 1).take(6) {
            if window.contains(");") {
                break;
            }
            window.push(' ');
            window.push_str(extra);
        }
        let Some(start) = window.find('"') else {
            continue;
        };
        let rest = &window[start + 1..];
        let Some(end) = rest.find('"') else { continue };
        let id = &rest[..end];
        if id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && !id.is_empty()
        {
            out.push(id.to_string());
        }
    }
    out
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let rd = fs::read_dir(dir).unwrap();
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            rust_files(&path, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "shims_tests.rs" || name == "tests.rs" || name.ends_with("_tests.rs") {
            continue;
        }
        out.push(path);
    }
}
