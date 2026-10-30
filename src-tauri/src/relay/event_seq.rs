//! Monotonic `seq` beside `events.jsonl`. The number sits outside the
//! NIP-01 id hash. Flag `event_seq` off: the writer never calls this,
//! and no `events.seq` file is created.

use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Recovery reads this much of the tail. A missing or behind `events.seq`
/// continues from the max `seq` found here, plus one.
pub(crate) const TAIL_BYTES: u64 = 256 * 1024;

/// `events.since` never returns more than this.
pub(crate) const SINCE_LIMIT: usize = 500;

pub(crate) struct SeqFile {
    dir: PathBuf,
}

pub(crate) struct SeqGuard {
    pub seq: u64,
    lock: File,
    dir: PathBuf,
}

impl SeqFile {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Exclusive flock on `events.seq.lock`, then `last+1`. The caller
    /// holds the in-process log mutex for the whole write and commit.
    pub(crate) fn next(&self, log_path: &Path) -> std::io::Result<SeqGuard> {
        let lock_path = self.dir.join("events.seq.lock");
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let lock = opts.open(&lock_path)?;
        lock.lock()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600));
        }
        let last = read_or_recover(&self.dir, log_path)?;
        let seq = last
            .checked_add(1)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "seq overflow"))?;
        Ok(SeqGuard {
            seq,
            lock,
            dir: self.dir.clone(),
        })
    }
}

impl SeqGuard {
    /// `events.seq` is `N\n`, written via tmp+rename. The flock drops
    /// with this guard. A failure leaves the line's seq on disk; the
    /// next `next` recovers it and does not reuse the number.
    pub(crate) fn commit(self) -> std::io::Result<()> {
        if injected_commit_failure() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "injected commit failure",
            ));
        }
        write_seq_atomic(&self.dir, self.seq)
    }
}

impl Drop for SeqGuard {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}

fn injected_commit_failure() -> bool {
    #[cfg(test)]
    {
        FAIL_COMMIT.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        false
    }
}

#[cfg(test)]
thread_local! {
    static FAIL_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_commit(on: bool) {
    FAIL_COMMIT.with(|c| c.set(on));
}

fn read_seq_value(dir: &Path) -> u64 {
    let Ok(text) = std::fs::read_to_string(dir.join("events.seq")) else {
        return 0;
    };
    text.trim().parse().unwrap_or(0)
}

fn read_or_recover(dir: &Path, log_path: &Path) -> std::io::Result<u64> {
    let seq_path = dir.join("events.seq");
    if !seq_path.exists() {
        // Missing file: the max may sit behind a legacy tail line.
        return Ok(recover_max_seq(log_path, TAIL_BYTES));
    }
    // Present file. A failed commit leaves the new seq on the last line
    // and the file behind it. A legacy line after that hides the number,
    // so a seq-less tail falls back to the 256 KiB scan.
    let file_n = read_seq_value(dir);
    let last = last_line_seq(log_path);
    if last == 0 {
        return Ok(file_n.max(recover_max_seq(log_path, TAIL_BYTES)));
    }
    Ok(file_n.max(last))
}

/// Seq on the last complete line, or 0 when that line is legacy or absent.
fn last_line_seq(log_path: &Path) -> u64 {
    let Ok(mut file) = File::open(log_path) else {
        return 0;
    };
    let Ok(meta) = file.metadata() else {
        return 0;
    };
    let len = meta.len();
    if len == 0 {
        return 0;
    }
    let window = 16 * 1024u64;
    let start = len.saturating_sub(window);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return 0;
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return 0;
    }
    if start > 0 {
        if let Some(i) = buf.iter().position(|b| *b == b'\n') {
            buf.drain(..=i);
        } else {
            return 0;
        }
    }
    buf.split(|b| *b == b'\n')
        .rev()
        .find(|line| !line.is_empty())
        .and_then(seq_of_line)
        .unwrap_or(0)
}

fn write_seq_atomic(dir: &Path, seq: u64) -> std::io::Result<()> {
    let dest = dir.join("events.seq");
    let tmp = dir.join("events.seq.tmp");
    let wrote = (|| -> std::io::Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        writeln!(f, "{seq}")?;
        drop(f);
        std::fs::rename(&tmp, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    })();
    if wrote.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    wrote
}

/// Max `"seq"` in the last `tail_bytes` of the log. A missing file is 0.
/// The first partial line of a mid-file tail is skipped.
pub(crate) fn recover_max_seq(log_path: &Path, tail_bytes: u64) -> u64 {
    let Ok(mut file) = File::open(log_path) else {
        return 0;
    };
    let Ok(meta) = file.metadata() else {
        return 0;
    };
    let len = meta.len();
    let start = len.saturating_sub(tail_bytes);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return 0;
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return 0;
    }
    if start > 0 {
        if let Some(i) = buf.iter().position(|b| *b == b'\n') {
            buf.drain(..=i);
        } else {
            return 0;
        }
    }
    let mut max = 0u64;
    for line in buf.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Some(seq) = seq_of_line(line) {
            max = max.max(seq);
        }
    }
    max
}

fn seq_of_line(line: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(line).ok()?;
    let v: Value = serde_json::from_str(text).ok()?;
    v.get("seq").and_then(Value::as_u64)
}

/// Lines with `seq > after`, file order, at most `limit` (capped at 500).
/// `next_after` is the last returned seq, or `after` when nothing matched.
/// Legacy lines (no seq) are skipped and never stop the scan.
pub fn since(log_path: &Path, after: u64, limit: usize) -> Result<(Vec<Value>, u64), String> {
    let limit = limit.min(SINCE_LIMIT);
    if limit == 0 {
        return Ok((Vec::new(), after));
    }
    let raw = match std::fs::read(log_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), after)),
        Err(e) => return Err(format!("read {}: {e}", log_path.display())),
    };
    let lines: Vec<&[u8]> = raw
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let mut matched = Vec::new();
    for line in lines.iter().rev() {
        let Ok(text) = std::str::from_utf8(line) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        match v.get("seq").and_then(Value::as_u64) {
            Some(seq) if seq <= after => break,
            Some(_) => matched.push(v),
            None => {}
        }
    }
    matched.reverse();
    if matched.len() > limit {
        matched.truncate(limit);
    }
    let next_after = matched
        .last()
        .and_then(|v| v.get("seq").and_then(Value::as_u64))
        .unwrap_or(after);
    Ok((matched, next_after))
}
