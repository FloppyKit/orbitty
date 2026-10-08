#![allow(unused_imports)]
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use super::dirs::*;
#[allow(unused_imports)]
use super::locate::*;
#[allow(unused_imports)]
use super::resolve::*;
#[allow(unused_imports)]
use super::parse::*;
#[allow(unused_imports)]
use super::rules::*;

/// `None` means sealed and launchable.
pub fn seal_note(roots: &Roots, go: &ParsedGo) -> Option<String> {
    let id = if go.legacy {
        legacy_token_from_path(&go.path).unwrap_or_else(|| go.id())
    } else {
        go.id()
    };
    if go.legacy {
        if legacy_allowed(roots, &id) {
            return None;
        }
        return Some(format!("{id} is unsealed (legacy prompt); not launching."));
    }
    let status = nonblank(go.header.status.as_deref())
        .unwrap_or_else(|| "draft".into())
        .to_ascii_lowercase();
    if status != "sealed" {
        return Some(format!("{id} is {status}, not sealed; not launching."));
    }
    if nonblank(go.header.sealed_at.as_deref()).is_none()
        || nonblank(go.header.sealed_by.as_deref()).is_none()
    {
        return Some(format!(
            "{id} is sealed but missing sealed_at or sealed_by; not launching."
        ));
    }
    let Some(want) = nonblank(go.header.seal_hash.as_deref()) else {
        return Some(format!(
            "{id} is unsealed (no seal_hash; run go seal); not launching."
        ));
    };
    // A trailing comment is not part of the parsed seal value, and the hash
    // leaves the seal key line out. Refuse it so the comment cannot launch.
    if let Some(key) = seal_line_trailing_comment(&go.yaml) {
        return Some(format!(
            "{id} has a trailing comment on {key}; not launching."
        ));
    }
    match seal_hash_of(roots, &go.path, &go.yaml, &go.header, &go.body) {
        Ok(got) if want.eq_ignore_ascii_case(&got) => None,
        Ok(_) => Some(format!(
            "{id} changed since seal (body, header or lock); not launching."
        )),
        Err(e) => Some(format!("{id} seal check failed ({e}); not launching.")),
    }
}

/// Header keys the seal writes, so the seal hash leaves them out.
pub(crate) const SEAL_KEYS: &[&str] = &["status", "sealed_at", "sealed_by", "seal_hash"];

/// The seal covers every header field except the seal fields themselves
/// (so push_allowed, tools, runner, harness, model, lock, workspace ...),
/// the body, and the lock. Header comments and key order don't count in
/// the header part: it is hashed as canonical JSON with sorted keys.
///
/// A separate lock file is hashed as its raw bytes under the part name
/// `lock` (or `(no lock)` when the header has no lock). When the lock
/// resolves to this GO file, the lock part is named `lock-self` instead:
/// the GO's own header with only the `status` / `sealed_at` / `sealed_by` /
/// `seal_hash` key lines left out, then the body. Indented continuation
/// lines under those keys stay hashed. Edits to the seal values do not
/// change the hash; a continuation, any other header text (including
/// comments), a header value, or a body byte does. A trailing comment on
/// a seal line is refused at launch.
pub fn seal_hash_of(
    roots: &Roots,
    go_path: &Path,
    yaml: &str,
    header: &GoHeader,
    body: &str,
) -> Result<String, String> {
    let doc: serde_yaml::Value = if yaml.trim().is_empty() {
        serde_yaml::Value::Mapping(Default::default())
    } else {
        serde_yaml::from_str(yaml).map_err(|e| format!("malformed header: {e}"))?
    };
    let mut j = serde_json::to_value(&doc).map_err(|e| format!("malformed header: {e}"))?;
    let obj = j
        .as_object_mut()
        .ok_or_else(|| "malformed header: not a map".to_string())?;
    for k in SEAL_KEYS {
        obj.remove(*k);
    }
    let head = canonical_json(&j);
    let (lock_name, lock) = match nonblank(header.lock.as_deref()) {
        None => ("lock", "(no lock)".to_string()),
        Some(l) => {
            let p = lock_path(roots, &l);
            if lock_is_self(&p, go_path) {
                ("lock-self", self_lock_text(yaml, body))
            } else {
                let text = std::fs::read_to_string(&p)
                    .map_err(|e| format!("lock unreadable {}: {e}", p.display()))?;
                ("lock", text)
            }
        }
    };
    let mut h = Sha256::new();
    h.update(b"orbitty-seal-v2\n");
    for (name, part) in [("header", head.as_str()), ("body", body), (lock_name, lock.as_str())] {
        h.update(format!("{name} {}\n", part.len()).as_bytes());
        h.update(part.as_bytes());
        h.update(b"\n");
    }
    Ok(hex_encode(&h.finalize()))
}

/// True when `lock` and `go` are the same file. Follows symlinks (dev+ino).
/// Equal canonical paths count too, so a case-only path difference on APFS
/// still matches. A missing lock is not self: the caller keeps the
/// "lock unreadable" error.
pub(crate) fn lock_is_self(lock: &Path, go: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(lock_meta) = std::fs::metadata(lock) else {
        return false;
    };
    if let Ok(go_meta) = std::fs::metadata(go) {
        if (lock_meta.dev(), lock_meta.ino()) == (go_meta.dev(), go_meta.ino()) {
            return true;
        }
    }
    match (std::fs::canonicalize(lock), std::fs::canonicalize(go)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Lock-part text for a GO that is its own lock. Only the top-level seal
/// key line itself (`status`, `sealed_at`, `sealed_by`, or `seal_hash`)
/// is left out. Indented continuation lines and block-scalar payloads stay
/// in the hashed text, so a multiline `|` or `>-` value, or an indented
/// comment under a seal key, breaks the seal. Every other header line
/// stays, `\r` stripped. Then a `---` fence and the body bytes.
pub(crate) fn self_lock_text(yaml: &str, body: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    for raw_line in yaml.split('\n') {
        let line = raw_line.replace('\r', "");
        if top_key(&line).is_some_and(|k| SEAL_KEYS.contains(&k)) {
            continue;
        }
        kept.push(line);
    }
    // `yaml` from the front matter ends with a newline, which split leaves
    // as a final empty piece. That piece is the terminator, not a blank line.
    if kept.last().is_some_and(|l| l.is_empty()) {
        kept.pop();
    }
    let header = kept.join("\n");
    if header.is_empty() {
        format!("---\n{body}")
    } else {
        format!("{header}\n---\n{body}")
    }
}

pub(crate) fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", Value::String((*k).clone()), canonical_json(&m[*k])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(canonical_json).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

pub(crate) fn lock_path(roots: &Roots, raw: &str) -> PathBuf {
    let plans = plans_dir(roots).display().to_string();
    let repo = repo_root(roots).display().to_string();
    expand_home(roots, &expand_text(raw, &repo, &plans))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_encode(&h.finalize())
}

pub fn seal_file(path: &Path, by: &str) -> Result<ParsedGo, String> {
    seal_file_in(&Roots::real(), path, by)
}

/// Seal in place. Only the `status`, `sealed_at`, `sealed_by` and
/// `seal_hash` lines change (added at the end of the header when absent);
/// comments, key order and the body stay byte-for-byte. The result must
/// verify before anything is written.
pub fn seal_file_in(roots: &Roots, path: &Path, by: &str) -> Result<ParsedGo, String> {
    let by = by.trim();
    if by.is_empty() {
        return Err("go seal needs --by".into());
    }
    if under_buzz(roots, path) {
        return Err(format!(
            "go seal: {} is inside ~/.buzz; refused (nothing is written under .buzz)",
            path.display()
        ));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let bom = raw.starts_with('\u{feff}');
    let text = raw.trim_start_matches('\u{feff}');
    let (yaml, body) = split_front(text)?;
    let header: GoHeader =
        serde_yaml::from_str(&yaml).map_err(|e| format!("malformed header: {e}"))?;
    let header = normalize_header(header);
    let hash = seal_hash_of(roots, path, &yaml, &header, &body)?;
    let nl = if text.starts_with("---\r\n") { "\r\n" } else { "\n" };
    let quote = |v: &str| serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into());
    let new_yaml = set_header_keys(
        &yaml,
        &[
            ("status", "sealed".to_string()),
            ("sealed_at", quote(&seal_timestamp())),
            ("sealed_by", quote(by)),
            ("seal_hash", hash),
        ],
        nl,
    );
    let out = format!(
        "{}---{nl}{new_yaml}---{nl}{body}",
        if bom { "\u{feff}" } else { "" }
    );
    // Verify the bytes about to be written. A self-lock's lock part comes
    // from this yaml and body (only the seal key lines left out), not from
    // the pre-seal file still on disk.
    let parsed = parse_text(path, out.clone())?;
    if let Some(note) = seal_note(roots, &parsed) {
        return Err(format!("seal did not verify: {note}"));
    }
    write_sealed(path, &out)?;
    Ok(parsed)
}

/// Replace the GO file without writing into a shared inode: refuse a file
/// with more than one hard link (the other name could be inside ~/.buzz),
/// then write a temp file in the same folder and rename it over the real
/// (symlink-resolved) path. Mode bits are kept.
pub(crate) fn write_sealed(path: &Path, out: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::MetadataExt;
    let real = std::fs::canonicalize(path).map_err(|e| format!("resolve {}: {e}", path.display()))?;
    let meta = std::fs::metadata(&real).map_err(|e| format!("stat {}: {e}", real.display()))?;
    if meta.nlink() > 1 {
        return Err(format!(
            "go seal: {} has {} hard links; refused (a link could be inside ~/.buzz)",
            path.display(),
            meta.nlink()
        ));
    }
    let dir = real.parent().ok_or_else(|| format!("no folder for {}", real.display()))?;
    let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{name}.seal-{}-{nanos}.tmp", std::process::id()));
    let res = (|| -> Result<(), String> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(out.as_bytes()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().map_err(|e| format!("sync {}: {e}", tmp.display()))?;
        std::fs::set_permissions(&tmp, meta.permissions())
            .map_err(|e| format!("chmod {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &real).map_err(|e| format!("rename → {}: {e}", real.display()))
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// True when `line` has a YAML comment (`#` outside quotes, at the start
/// of the line or after whitespace). `#` inside a quoted seal value stays
/// part of that value.
pub(crate) fn has_yaml_trailing_comment(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if b[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'\'' => {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\'' {
                        if i + 1 < b.len() && b[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'#' if i == 0 || b[i - 1].is_ascii_whitespace() => return true,
            _ => i += 1,
        }
    }
    false
}

/// Seal key whose line carries a trailing comment, if any. The parsed
/// value does not include that comment, so launch refuses the line itself.
pub(crate) fn seal_line_trailing_comment(yaml: &str) -> Option<String> {
    for raw_line in yaml.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(k) = top_key(line).filter(|k| SEAL_KEYS.contains(k)) {
            if has_yaml_trailing_comment(line) {
                return Some(k.to_string());
            }
        }
    }
    None
}

/// Top-level YAML key on this line, if it is a plain `key: ...` line.
pub(crate) fn top_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (k, _) = line.split_once(':')?;
    let k = k.trim().trim_matches(['"', '\'']);
    if k.is_empty() || k.contains(' ') {
        None
    } else {
        Some(k)
    }
}

/// Replace (or append) top-level keys in header text, keeping every other
/// line, comments included. A replaced key's indented continuation lines go
/// with it; a repeated key keeps only the first, rewritten line.
pub(crate) fn set_header_keys(yaml: &str, pairs: &[(&str, String)], nl: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut done = vec![false; pairs.len()];
    let mut skipping = false;
    for line in yaml.lines() {
        if skipping {
            if line.starts_with([' ', '\t']) {
                continue;
            }
            skipping = false;
        }
        if let Some(i) = top_key(line).and_then(|k| pairs.iter().position(|(p, _)| *p == k)) {
            if !done[i] {
                out.push(format!("{}: {}", pairs[i].0, pairs[i].1));
                done[i] = true;
            }
            skipping = true;
            continue;
        }
        out.push(line.to_string());
    }
    for (i, (k, v)) in pairs.iter().enumerate() {
        if !done[i] {
            out.push(format!("{k}: {v}"));
        }
    }
    let mut joined = out.join(nl);
    joined.push_str(nl);
    joined
}

pub fn status_value(
    file: Option<&str>,
    go: Option<&str>,
    card: Option<&str>,
    title: Option<&str>,
) -> Result<Value, String> {
    let roots = Roots::real();
    if let Some(f) = nonblank(file) {
        let path = PathBuf::from(&f);
        let parsed = parse_file(&path)?;
        return Ok(status_json(&roots, &parsed));
    }
    let go_id = nonblank(go).unwrap_or_default();
    let card_id = nonblank(card).unwrap_or_default();
    let title = if let Some(t) = nonblank(title) {
        t
    } else if go_id.is_empty() {
        return Err("go status needs --file or --go".into());
    } else {
        go_id.clone()
    };
    match locate(&roots, &card_id, &title, &[]) {
        Locate::Missing => Ok(json!({
            "sealed": false,
            "found": false,
            "id": go_id,
            "note": format!("{go_id} is not sealed; not launching."),
        })),
        Locate::Bad { note } => Ok(json!({
            "sealed": false,
            "found": true,
            "id": go_id,
            "note": note,
        })),
        Locate::Found(parsed) => Ok(status_json(&roots, &parsed)),
    }
}

/// `query` (sealed only) is the exact text a launch hands the agent: the
/// GO with paths expanded, the lock appended, and the launcher rules. The
/// orbitty-needed clerk / cursor-go routes use it, so every route gets the
/// same text. `launchNote` set = sealed but still not launchable.
pub(crate) fn status_json(roots: &Roots, go: &ParsedGo) -> Value {
    let note = seal_note(roots, go);
    let mut v = json!({
        "sealed": note.is_none(),
        "found": true,
        "id": go.id(),
        "file": go.path.display().to_string(),
        "status": go.header.status,
        "legacy": go.legacy,
        "sha256": sha256_hex(go.raw.as_bytes()),
        "workspace": workspace_in(roots, go).display().to_string(),
        "note": note.clone().unwrap_or_default(),
    });
    if note.is_none() {
        let run = resolve_run_in(roots, &go.header, &go.id(), "", herdr_bin_present());
        match push_decision(&run) {
            Err(e) => v["launchNote"] = json!(format!("{} {e}; not launching.", go.id())),
            Ok(push) => {
                let q = launch_text_in(roots, go, push);
                match enforce_cap(&q) {
                    Err(e) => v["launchNote"] = json!(e),
                    Ok(()) => {
                        v["queryChars"] = json!(q.chars().count());
                        v["query"] = json!(q);
                    }
                }
            }
        }
    }
    v
}

