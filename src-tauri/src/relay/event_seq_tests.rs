use super::event_log;
use super::event_seq;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_N: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    path: PathBuf,
}

impl Tmp {
    fn new(name: &str) -> Self {
        let n = TMP_N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "orbitty-event-seq-{name}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct Force;

impl Force {
    fn on() -> Self {
        event_log::force_event_seq(Some(true));
        Force
    }

    fn off() -> Self {
        event_log::force_event_seq(Some(false));
        Force
    }
}

impl Drop for Force {
    fn drop(&mut self) {
        event_log::force_event_seq(None);
    }
}

fn write_at(path: &Path, n: u64) -> String {
    event_log::write_event(
        path,
        1_700_000_000 + n,
        event_log::KIND,
        vec![["t".into(), "local-only".into()]],
        format!("{{\"n\":{n}}}"),
        (format!("id{n:04}"), String::new(), String::new()),
    )
    .unwrap()
}

fn lines_of(path: &Path) -> Vec<Value> {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn seqs_of(path: &Path) -> Vec<u64> {
    lines_of(path)
        .into_iter()
        .filter_map(|v| v.get("seq").and_then(Value::as_u64))
        .collect()
}

/// Flag off: the bytes are the pre-seq line. No seq file.
#[test]
fn flag_off_bytes_identical() {
    let tmp = Tmp::new("off");
    let _f = Force::off();
    let path = tmp.path.join("events.jsonl");
    event_log::write_event(
        &path,
        1_700_000_000,
        31200,
        vec![["t".into(), "local-only".into()]],
        "{}".into(),
        ("abc".into(), String::new(), String::new()),
    )
    .unwrap();
    let got = std::fs::read_to_string(&path).unwrap();
    let golden = "{\"id\":\"abc\",\"pubkey\":\"\",\"created_at\":1700000000,\"kind\":31200,\"tags\":[[\"t\",\"local-only\"]],\"content\":\"{}\",\"sig\":\"\"}\n";
    assert_eq!(got, golden);
    assert!(!tmp.path.join("events.seq").exists());
    assert!(!tmp.path.join("events.seq.lock").exists());
}

#[test]
fn seq_increments_and_persists() {
    let tmp = Tmp::new("inc");
    let _f = Force::on();
    let path = tmp.path.join("events.jsonl");
    for n in 1..=3 {
        write_at(&path, n);
    }
    assert_eq!(seqs_of(&path), vec![1, 2, 3]);
    assert_eq!(
        std::fs::read_to_string(tmp.path.join("events.seq")).unwrap(),
        "3\n"
    );
}

#[test]
fn seq_outside_signature() {
    let tmp = Tmp::new("sig");
    let _f = Force::on();
    let key = crate::event_keys::load_or_create_in(&tmp.path).unwrap();
    let created_at = 1_700_000_000u64;
    let kind = 420u64;
    let tags = vec![["t".into(), "orbitty".into()], ["d".into(), "c-1".into()]];
    let content = "{}";
    let signed = crate::event_keys::sign_with(&key, created_at, kind, &tags, content).unwrap();
    let path = tmp.path.join("events.jsonl");
    event_log::write_event(
        &path,
        created_at,
        kind,
        tags.clone(),
        content.into(),
        (signed.id.clone(), signed.pubkey.clone(), signed.sig.clone()),
    )
    .unwrap();
    let line = lines_of(&path).pop().unwrap();
    assert_eq!(line["seq"], 1);
    let got_id = line["id"].as_str().unwrap();
    let again = crate::event_keys::nip01_id(
        line["pubkey"].as_str().unwrap(),
        line["created_at"].as_u64().unwrap(),
        line["kind"].as_u64().unwrap(),
        &tags,
        line["content"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(got_id, again);
    assert_eq!(got_id, signed.id);
    assert_eq!(
        crate::event_keys::classify_value(&line),
        crate::event_keys::LineClass::Signed
    );
}

#[test]
fn recover_from_missing_seq_file() {
    let tmp = Tmp::new("missing");
    let _f = Force::on();
    let path = tmp.path.join("events.jsonl");
    for n in 1..=5 {
        write_at(&path, n);
    }
    std::fs::remove_file(tmp.path.join("events.seq")).unwrap();
    write_at(&path, 6);
    assert_eq!(seqs_of(&path), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(
        std::fs::read_to_string(tmp.path.join("events.seq")).unwrap(),
        "6\n"
    );
}

#[test]
fn recover_ignores_legacy_lines() {
    let tmp = Tmp::new("legacy");
    let path = tmp.path.join("events.jsonl");
    {
        let _f = Force::off();
        for n in 0..10 {
            write_at(&path, n);
        }
    }
    let legacy_plus = "{\"id\":\"old\",\"pubkey\":\"\",\"created_at\":1,\"kind\":31200,\"tags\":[],\"content\":\"{}\",\"sig\":\"\",\"seq\":7}\n";
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(legacy_plus.as_bytes())
        .unwrap();
    let _f = Force::on();
    write_at(&path, 99);
    let seqs = seqs_of(&path);
    assert_eq!(seqs.last().copied(), Some(8));
    assert_eq!(seqs.iter().filter(|s| **s == 8).count(), 1);
}

#[test]
fn never_reuses_after_commit_failure() {
    let tmp = Tmp::new("fail");
    let _f = Force::on();
    let path = tmp.path.join("events.jsonl");
    write_at(&path, 1);
    event_seq::fail_commit(true);
    let err = event_log::write_event(
        &path,
        1_700_000_002,
        event_log::KIND,
        vec![["t".into(), "local-only".into()]],
        "{\"n\":2}".into(),
        ("id0002".into(), String::new(), String::new()),
    )
    .unwrap_err();
    event_seq::fail_commit(false);
    assert!(err.contains("injected commit failure"), "{err}");
    assert_eq!(
        std::fs::read_to_string(tmp.path.join("events.seq")).unwrap(),
        "1\n"
    );
    assert_eq!(seqs_of(&path), vec![1, 2]);
    write_at(&path, 3);
    assert_eq!(seqs_of(&path), vec![1, 2, 3]);
    assert_eq!(
        std::fs::read_to_string(tmp.path.join("events.seq")).unwrap(),
        "3\n"
    );
}

#[test]
fn concurrent_threads_no_gaps_no_dups() {
    let tmp = Tmp::new("threads");
    let path = tmp.path.join("events.jsonl");
    let mut joins = Vec::new();
    for t in 0..8 {
        let path = path.clone();
        joins.push(std::thread::spawn(move || {
            let _f = Force::on();
            for i in 0..500 {
                event_log::write_event(
                    &path,
                    1_700_000_000,
                    event_log::KIND,
                    vec![["t".into(), "local-only".into()]],
                    format!("{{\"t\":{t},\"i\":{i}}}"),
                    (format!("t{t}-{i}"), String::new(), String::new()),
                )
                .unwrap();
            }
        }));
    }
    for j in joins {
        j.join().unwrap();
    }
    let mut seqs = seqs_of(&path);
    seqs.sort_unstable();
    let expect: Vec<u64> = (1..=4000).collect();
    assert_eq!(seqs, expect);
}

const WORKER: &str = "relay::event_seq_tests::cross_process_worker";

#[test]
fn cross_process_worker() {
    let Ok(dir) = std::env::var("EVENT_SEQ_DIR") else {
        return;
    };
    let n: u64 = std::env::var("EVENT_SEQ_N").unwrap().parse().unwrap();
    let worker = std::env::var("EVENT_SEQ_WORKER").unwrap_or_else(|_| "0".into());
    let _f = Force::on();
    let path = PathBuf::from(dir).join("events.jsonl");
    for i in 0..n {
        event_log::write_event(
            &path,
            1_700_000_000,
            event_log::KIND,
            vec![["t".into(), "local-only".into()]],
            format!("{{\"w\":{worker},\"i\":{i}}}"),
            (format!("w{worker}-{i}"), String::new(), String::new()),
        )
        .unwrap();
    }
}

#[test]
fn cross_process_flock() {
    let tmp = Tmp::new("flock");
    let exe = std::env::current_exe().unwrap();
    let mut kids = Vec::new();
    for worker in 0..2 {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["--exact", WORKER, "--test-threads=1"])
            .env("EVENT_SEQ_DIR", &tmp.path)
            .env("EVENT_SEQ_N", "200")
            .env("EVENT_SEQ_WORKER", worker.to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        kids.push(cmd.spawn().unwrap());
    }
    for kid in kids {
        let out = kid.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "worker status {}\nstdout {}\nstderr {}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let mut seqs = seqs_of(&tmp.path.join("events.jsonl"));
    seqs.sort_unstable();
    let expect: Vec<u64> = (1..=400).collect();
    assert_eq!(seqs, expect);
}

#[test]
fn since_returns_after_in_order_with_limit() {
    let tmp = Tmp::new("since");
    let _f = Force::on();
    let path = tmp.path.join("events.jsonl");
    for n in 1..=6 {
        write_at(&path, n);
    }
    let (lines, next_after) = event_seq::since(&path, 3, 2).unwrap();
    let seqs: Vec<u64> = lines.iter().map(|v| v["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, vec![4, 5]);
    assert_eq!(next_after, 5);
}

#[test]
fn since_on_legacy_only_file_is_empty() {
    let tmp = Tmp::new("since-legacy");
    let path = tmp.path.join("events.jsonl");
    {
        let _f = Force::off();
        for n in 0..5 {
            write_at(&path, n);
        }
    }
    let (lines, next_after) = event_seq::since(&path, 0, 500).unwrap();
    assert!(lines.is_empty());
    assert_eq!(next_after, 0);
}

#[test]
fn publish_strips_seq() {
    let line = json!({
        "seq": 7,
        "id": "ab".repeat(32),
        "pubkey": "cd".repeat(32),
        "created_at": 1,
        "kind": 420,
        "tags": [["t", "orbitty"], ["d", "c-1"]],
        "content": "{}",
        "sig": "ef".repeat(64),
    });
    assert!(super::event_relay::publishable(&line));
    let sent = super::event_relay::relay_payload(&line);
    assert!(sent.get("seq").is_none());
    assert!(line.get("seq").is_some());
    assert_eq!(sent["id"], line["id"]);
    assert_eq!(sent["sig"], line["sig"]);
    assert!(super::event_relay::publishable(&sent));
    let mut bare = line.clone();
    bare.as_object_mut().unwrap().remove("seq");
    assert_eq!(
        super::event_relay::publishable(&line),
        super::event_relay::publishable(&bare)
    );
}

#[test]
#[ignore]
fn soak_10k_p99() {
    let tmp = Tmp::new("soak");
    let _f = Force::on();
    let path = tmp.path.join("events.jsonl");
    let n = 10_000usize;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t0 = std::time::Instant::now();
        event_log::write_event(
            &path,
            1_700_000_000,
            event_log::KIND,
            vec![["t".into(), "local-only".into()]],
            format!("{{\"i\":{i}}}"),
            (format!("s{i}"), String::new(), String::new()),
        )
        .unwrap();
        samples.push(t0.elapsed().as_micros());
    }
    samples.sort_unstable();
    let p50 = samples[n / 2];
    let p99 = samples[(n * 99 / 100) - 1];
    eprintln!("soak_10k_p99 p50_us={p50} p99_us={p99} n={n}");
    assert!(p99 < 1000, "p99 {p99} µs >= 1000 (p50 {p50})");
}
