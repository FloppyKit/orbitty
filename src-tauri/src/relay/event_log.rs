//! Append-only events.jsonl. A log failure never fails the user action.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

/// One kind for every Orbitty runtime event this GO.
pub const KIND: u64 = 31200;

/// Board column proofs. Same three numbers as
/// NCH compute, different `t` tag (`orbitty` here, `nch` there).
pub const KIND_BOARD_TODO: u64 = 420;
pub const KIND_BOARD_DOING: u64 = 421;
pub const KIND_BOARD_DONE: u64 = 422;

/// Serializes appends so two command threads can't interleave a line.
static LOG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static WARNED: AtomicBool = AtomicBool::new(false);

// Tests pin the flag per thread. None follows flags::on_in_dir.
#[cfg(test)]
thread_local! {
    static SEQ_FORCE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn force_event_seq(on: Option<bool>) {
    SEQ_FORCE.with(|c| c.set(on));
}

fn event_seq_enabled(path: &Path) -> bool {
    #[cfg(test)]
    if let Some(v) = SEQ_FORCE.with(|c| c.get()) {
        return v;
    }
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    crate::flags::on_in_dir(dir, "event_seq")
}

fn warn_once(why: &str) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("orbitty: event log append failed (further failures silent): {why}");
    }
}

pub(crate) fn events_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("events.jsonl"))
}

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fill_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(buf).is_ok() {
            return;
        }
    }
    // Fallback stir: time ^ pid ^ counter, xorshifted per byte. Only reached
    // when /dev/urandom is unreadable; uniqueness is best-effort.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut x = unix_now().wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0)
        ^ ((std::process::id() as u64) << 32)
        ^ COUNTER.fetch_add(1, Ordering::Relaxed);
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
}

/// uuid-v4 as 32 lowercase hex chars (no dashes, no 0x).
pub(crate) fn uuid_v4_hex() -> String {
    let mut b = [0u8; 16];
    fill_random(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10
    let mut s = String::with_capacity(32);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

#[derive(Serialize)]
struct EventLine {
    /// Local cursor. Omitted when the flag is off, so the bytes match
    /// the pre-seq line. Not part of the NIP-01 id hash. `write_line`
    /// inserts it under the log lock when the flag is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    seq: Option<u64>,
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u64,
    tags: Vec<[String; 2]>,
    /// Nostr content is a string; ours holds one small JSON object.
    content: String,
    sig: String,
}

pub struct Appended {
    pub id: String,
    pub created_at: u64,
    pub trace_id: String,
}

/// One event line through the shared writer, same shape as `append`.
/// `ident` is (id, pubkey, sig). Returns the id.
pub(crate) fn write_event(
    path: &Path,
    created_at: u64,
    kind: u64,
    tags: Vec<[String; 2]>,
    content: String,
    ident: (String, String, String),
) -> Result<String, String> {
    let (id, pubkey, sig) = ident;
    let line = EventLine {
        seq: None,
        id,
        pubkey,
        created_at,
        kind,
        tags,
        content,
        sig,
    };
    let text = serde_json::to_string(&line).map_err(|e| format!("serialize event: {e}"))?;
    write_line(path, &text).map_err(|e| format!("append {}: {e}", path.display()))?;
    Ok(line.id)
}

fn write_line(path: &Path, line: &str) -> std::io::Result<()> {
    let _guard = LOG_LOCK
        .lock()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("log lock: {e}")))?;
    // append and append_board_proof also land here (S0). History uses
    // write_event, which calls this. One lock, one seq, every writer.
    if event_seq_enabled(path) {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let seq = super::event_seq::SeqFile::new(dir).next(path)?;
        let text = insert_seq(line, seq.seq)?;
        append_bytes(path, text.as_bytes(), true)?;
        seq.commit()?;
        return Ok(());
    }
    append_bytes(path, line.as_bytes(), false)
}

/// `"seq":N` is the first field. The rest of the line is unchanged, so
/// id/pubkey/sig stay the bytes that were signed.
fn insert_seq(line: &str, seq: u64) -> std::io::Result<String> {
    let line = line.trim_end_matches(['\n', '\r']);
    let rest = line.strip_prefix('{').ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "event line is not a JSON object",
        )
    })?;
    Ok(format!("{{\"seq\":{seq},{rest}"))
}

fn append_bytes(path: &Path, bytes: &[u8], sync: bool) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.write_all(b"\n")?;
    if sync {
        f.sync_data()?;
    }
    Ok(())
}

/// Core append. `trace_id` None mints one; `project_id` None/empty tags
/// "void"; `actor` None/empty tags "user". Unknown e-types are written as
/// given — never a crash. Err only on IO/serialize failure.
pub fn append(
    app: &tauri::AppHandle,
    e_type: &str,
    trace_id: Option<&str>,
    project_id: Option<&str>,
    actor: Option<&str>,
    local_only: bool,
    content: &serde_json::Value,
) -> Result<Appended, String> {
    let path = events_path(app)?;
    let trace = trace_id
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(uuid_v4_hex);
    let project = project_id
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or("void")
        .to_string();
    let actor = actor
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .unwrap_or("user")
        .to_string();
    let created_at = unix_now();
    let tags = vec![
        // t=publishable stays illegal — local file only.
        ["t".into(), "local-only".into()],
        ["trace".into(), trace.clone()],
        ["e-type".into(), e_type.to_string()],
        ["project".into(), project],
        ["actor".into(), actor],
    ];
    let content = serde_json::to_string(content).unwrap_or_else(|_| "{}".into());
    let (id, pubkey, sig) = identity(app, created_at, KIND, &tags, &content);
    let _ = local_only;
    let line = EventLine {
        seq: None,
        id,
        pubkey,
        created_at,
        kind: KIND,
        tags,
        content,
        sig,
    };
    let text = serde_json::to_string(&line).map_err(|e| format!("serialize event: {e}"))?;
    write_line(&path, &text).map_err(|e| format!("append {}: {e}", path.display()))?;
    Ok(Appended {
        id: line.id,
        created_at: line.created_at,
        trace_id: trace,
    })
}

/// Best-effort append for native hooks: one stderr line per process on
/// failure, never an Err bubbling into the caller's real work.
pub fn append_best(
    app: &tauri::AppHandle,
    e_type: &str,
    trace_id: Option<&str>,
    project_id: Option<&str>,
    actor: Option<&str>,
    local_only: bool,
    content: &serde_json::Value,
) -> Option<Appended> {
    match append(
        app, e_type, trace_id, project_id, actor, local_only, content,
    ) {
        Ok(a) => Some(a),
        Err(e) => {
            warn_once(&e);
            None
        }
    }
}

/// Card tag names in tag-id order, plus the project display name.
/// The name is passed through so the planet slug can ignore a rename.
/// A missing card or a failed load yields no names; the proof still writes.
fn board_card_tags(
    app: &tauri::AppHandle,
    card_id: &str,
    project_id: &str,
) -> (Vec<String>, String) {
    let Ok(st) = crate::state::load_state(app) else {
        return (Vec::new(), String::new());
    };
    let display = st
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let names = st
        .kanban_cards
        .iter()
        .find(|c| c.id == card_id)
        .and_then(|c| c.tag_ids.as_ref())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| st.tags.iter().find(|t| t.id == *id).map(|t| t.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    (names, display)
}

/// One board column proof: a mint or move to
/// todo/doing/done is kind 420/421/422 with `t=orbitty` (never a 31200
/// board-move/board-add line). `d` = card id; doing chains `e` → the
/// card's newest 420, done chains `e` → the newest 420 and 421. New
/// lines fill NIP-01 id/pubkey/sig when Settings → signed locally is
/// on; a sign failure writes unsigned instead of failing the board.
pub fn append_board_proof(
    app: &tauri::AppHandle,
    card_id: &str,
    to: &str,
    trace_id: Option<&str>,
    project_id: Option<&str>,
    actor: Option<&str>,
    content: &serde_json::Value,
) -> Result<Appended, String> {
    let kind = match to.trim() {
        "todo" => KIND_BOARD_TODO,
        "doing" => KIND_BOARD_DOING,
        "done" => KIND_BOARD_DONE,
        other => return Err(format!("board proof: unknown column {other:?}")),
    };
    let path = events_path(app)?;
    let trace = trace_id
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(uuid_v4_hex);
    let project = project_id
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or("void")
        .to_string();
    let actor = actor
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .unwrap_or("user")
        .to_string();
    let mut tags = vec![
        ["t".into(), "orbitty".into()],
        ["d".into(), card_id.to_string()],
    ];
    match kind {
        KIND_BOARD_DOING => {
            tags.extend(board_chain_links(&path, card_id, &[KIND_BOARD_TODO]));
        }
        KIND_BOARD_DONE => {
            tags.extend(board_chain_links(
                &path,
                card_id,
                &[KIND_BOARD_TODO, KIND_BOARD_DOING],
            ));
        }
        _ => {}
    }
    tags.push(["project".into(), project.clone()]);
    tags.push(["actor".into(), actor]);
    tags.push(["trace".into(), trace.clone()]);
    if crate::flags::on(app, "microtags") {
        let (names, display) = board_card_tags(app, card_id, &project);
        let refused =
            crate::microtags::apply_board_microtags(&mut tags, true, &names, &project, &display);
        for raw in &refused {
            eprintln!("orbitty: microtag refused: {raw}");
        }
    }
    let created_at = unix_now();
    let content = serde_json::to_string(content).unwrap_or_else(|_| "{}".into());
    let (id, pubkey, sig) = identity(app, created_at, kind, &tags, &content);
    let line = EventLine {
        seq: None,
        id,
        pubkey,
        created_at,
        kind,
        tags,
        content,
        sig,
    };
    let text = serde_json::to_string(&line).map_err(|e| format!("serialize event: {e}"))?;
    write_line(&path, &text).map_err(|e| format!("append {}: {e}", path.display()))?;
    // Signed 420/421/422 may EVENT to the sidecar. Never 31200.
    if let Ok(v) = serde_json::from_str(&text) {
        crate::event_relay::publish_best(app, &v);
    }
    Ok(Appended {
        id: line.id,
        created_at: line.created_at,
        trace_id: trace,
    })
}

/// Best-effort twin of [`append_board_proof`] for the native board
/// writers: one stderr line per process on failure, never an Err bubbling
/// into the save path.
pub fn append_board_proof_best(
    app: &tauri::AppHandle,
    card_id: &str,
    to: &str,
    trace_id: Option<&str>,
    project_id: Option<&str>,
    actor: Option<&str>,
    content: &serde_json::Value,
) -> Option<Appended> {
    match append_board_proof(app, card_id, to, trace_id, project_id, actor, content) {
        Ok(a) => Some(a),
        Err(e) => {
            warn_once(&e);
            None
        }
    }
}

/// Chain links for a proof: the newest event id per wanted kind (420
/// before 421) for this card, scanned from the tail of the log. Only
/// `t=orbitty` lines with this exact `d` tag count — NCH's same-numbered
/// `t=nch` events are a different chain. A miss or an unreadable file is
/// an empty chain, never an error: the proof still writes.
fn board_chain_links(path: &Path, card_id: &str, want: &[u64]) -> Vec<[String; 2]> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let d_needle = format!("\"d\",\"{card_id}\"");
    let mut found: Vec<(u64, String)> = Vec::new();
    for line in raw.lines().rev() {
        if found.len() == want.len() {
            break;
        }
        if !line.contains("\"t\",\"orbitty\"") || !line.contains(&d_needle) {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // Signed lines must verify; unsigned pre-keys lines still chain.
        // Corrupt never fails the board write — skip the link.
        if crate::event_keys::classify_value(&v) == crate::event_keys::LineClass::Corrupt {
            warn_once("event line corrupt, skipped");
            continue;
        };
        let Some(kind) = v.get("kind").and_then(|k| k.as_u64()) else {
            continue;
        };
        if !want.contains(&kind) || found.iter().any(|(k, _)| *k == kind) {
            continue;
        }
        // Substring hits are only a prefilter — confirm the real `d` tag.
        let tagged = v
            .get("tags")
            .and_then(|t| t.as_array())
            .is_some_and(|tags| {
                tags.iter().any(|t| {
                    t.as_array().is_some_and(|a| {
                        a.len() == 2 && a[0].as_str() == Some("d") && a[1].as_str() == Some(card_id)
                    })
                })
            });
        if !tagged {
            continue;
        }
        if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
            found.push((kind, id.to_string()));
        }
    }
    want.iter()
        .filter_map(|k| found.iter().find(|(fk, _)| fk == k))
        .map(|(_, id)| ["e".into(), id.clone()])
        .collect()
}

/// What the UI gets back. `traceId` is always populated (minted when the
/// caller didn't thread one) so a drag can be stitched later. On IO error
/// the command still returns Ok with `skipped: true` — never an Err that
/// bubbles into save_state / send.
#[derive(Serialize)]
pub struct LogEventAck {
    pub id: String,
    pub created_at: u64,
    #[serde(rename = "traceId")]
    pub trace_id: String,
    pub skipped: bool,
}

#[tauri::command]
pub fn log_event(
    app: tauri::AppHandle,
    e_type: String,
    trace_id: Option<String>,
    project_id: Option<String>,
    actor: Option<String>,
    local_only: Option<bool>,
    content: Option<serde_json::Value>,
) -> LogEventAck {
    let trace = trace_id
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(uuid_v4_hex);
    let content = content.unwrap_or_else(|| serde_json::json!({}));
    // A board column transition (mint or move to
    // todo/doing/done) is kind 420/421/422 with `t=orbitty`, never a 31200
    // board-move/board-add line — one match arm on the e-type. A shape we
    // don't recognize falls through to the generic 31200 write.
    let proof_to = match e_type.as_str() {
        "board-move" => content.get("to").and_then(|v| v.as_str()),
        "board-add" => content.get("to").and_then(|v| v.as_str()).or(Some("todo")),
        _ => None,
    };
    if let (Some(to), Some(card_id)) = (proof_to, content.get("cardId").and_then(|v| v.as_str())) {
        if matches!(to, "todo" | "doing" | "done") {
            return match append_board_proof(
                &app,
                card_id,
                to,
                Some(&trace),
                project_id.as_deref(),
                actor.as_deref(),
                &content,
            ) {
                Ok(a) => LogEventAck {
                    id: a.id,
                    created_at: a.created_at,
                    trace_id: a.trace_id,
                    skipped: false,
                },
                Err(e) => {
                    warn_once(&e);
                    LogEventAck {
                        id: String::new(),
                        created_at: 0,
                        trace_id: trace,
                        skipped: true,
                    }
                }
            };
        }
    }
    match append(
        &app,
        &e_type,
        Some(&trace),
        project_id.as_deref(),
        actor.as_deref(),
        local_only.unwrap_or(true),
        &content,
    ) {
        Ok(a) => LogEventAck {
            id: a.id,
            created_at: a.created_at,
            trace_id: a.trace_id,
            skipped: false,
        },
        Err(e) => {
            warn_once(&e);
            LogEventAck {
                id: String::new(),
                created_at: 0,
                trace_id: trace,
                skipped: true,
            }
        }
    }
}

/// Claim/unclaim events, emitted from the one native writer of `claims`
/// (the save_state command) by diffing disk-before against the incoming
/// snapshot — catches rail, board, and companion writes with no second
/// frontend guess. A reboot rematch rewrites window_id but keeps the
/// claim's stable fingerprint; those pairs are adoption, not claim churn,
/// and stay silent.
pub fn diff_claims(
    app: &tauri::AppHandle,
    before: &[crate::state::Claim],
    after: &[crate::state::Claim],
) {
    let removed: Vec<&crate::state::Claim> = before
        .iter()
        .filter(|b| !after.iter().any(|a| a.window_id == b.window_id))
        .collect();
    let mut added: Vec<&crate::state::Claim> = after
        .iter()
        .filter(|a| !before.iter().any(|b| b.window_id == a.window_id))
        .collect();
    if removed.is_empty() && added.is_empty() {
        return;
    }
    let fingerprint = |c: &crate::state::Claim| -> Option<String> {
        let bundle = c.bundle_id.clone().or_else(|| c.app_name.clone())?;
        let title = c.title_at_claim.clone()?;
        let index = c.title_index?;
        Some(format!("{bundle}|{title}|{index}"))
    };
    let mut true_removed: Vec<&crate::state::Claim> = Vec::new();
    for r in removed {
        let adopted = fingerprint(r).and_then(|fp| {
            added
                .iter()
                .position(|a| fingerprint(a).as_ref() == Some(&fp))
        });
        match adopted {
            Some(i) => {
                added.remove(i);
            }
            None => true_removed.push(r),
        }
    }
    for c in true_removed {
        append_best(
            app,
            "unclaim",
            None,
            Some(&c.project_id),
            Some("user"),
            true,
            &serde_json::json!({ "windowId": c.window_id, "projectId": c.project_id }),
        );
    }
    for c in added {
        append_best(
            app,
            "claim",
            None,
            Some(&c.project_id),
            Some("user"),
            true,
            &serde_json::json!({ "windowId": c.window_id, "projectId": c.project_id }),
        );
    }
}

/// id/pubkey/sig for a new line. Off or a sign failure → uuid-v4,
/// empty pubkey/sig, same as pre-keys. Never fails the caller.
fn identity(
    app: &tauri::AppHandle,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
) -> (String, String, String) {
    identity_as(
        app,
        crate::event_keys::signed_locally(app),
        created_at,
        kind,
        tags,
        content,
    )
}

/// [`identity`] with the Settings flag already in hand, so a caller that
/// holds the loaded state does not re-read state.json per line.
pub(crate) fn identity_as(
    app: &tauri::AppHandle,
    signed: bool,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
) -> (String, String, String) {
    if !signed {
        return (uuid_v4_hex(), String::new(), String::new());
    }
    match crate::event_keys::sign_new(app, created_at, kind, tags, content) {
        Ok(s) => (s.id, s.pubkey, s.sig),
        Err(e) => {
            warn_once(&format!("sign failed, writing unsigned: {e}"));
            (uuid_v4_hex(), String::new(), String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn builder_emits_microtags_only_when_the_flag_is_on() {
        let src = include_str!("event_log.rs");
        let start = src.find("pub fn append_board_proof(").expect("builder");
        let end = src[start..]
            .find("\npub fn ")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        let flag = body.find("flags::on(app, \"microtags\")").expect("flag");
        let lookup = body.find("board_card_tags(").expect("lookup");
        let call = body.find("apply_board_microtags(").expect("apply");
        assert!(
            flag < lookup && lookup < call,
            "flag gates lookup and append"
        );
        let args = &body[call..];
        let project = args.find("&project").expect("project id");
        let display = args.find("&display").expect("display name");
        assert!(project < display, "slug input is the project id");
        assert!(body.contains("microtag refused"));
    }
}
