//! Board write helpers. One save door, same as the UI.

use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;

use crate::card_links::{self, MAX_LINKS};
use crate::event_log;
use crate::state::{CardLink, KanbanCard, PersistedState, PointsEntry, Project};

/// Same daily points cap as the board (KanbanPage DAILY_CAP).
const DAILY_CAP: u32 = 12;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

const BASE36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(BASE36[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// `<prefix>-<base36 millis>-<6 base36 rand>` — the rail/board id shape
/// (`c-` todos, `n-` needed, `pt-` points rows, `a-` actions).
pub(crate) fn new_id(prefix: &str) -> String {
    use std::io::Read;
    let mut b = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    let mut x = u64::from_le_bytes(b);
    let mut rand = String::with_capacity(6);
    for _ in 0..6 {
        rand.push(BASE36[(x % 36) as usize] as char);
        x /= 36;
    }
    format!("{prefix}-{}-{rand}", base36(now_millis()))
}

/// Local calendar day `YYYY-MM-DD` — the pointsLedger daily-cap key, same
/// as the frontend todayKey().
fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// The one save door — same shape as the Tauri `save_state` command:
/// atomic write, claim diff (best-effort), `state:changed` emit.
pub(crate) fn save_with_events(
    app: &tauri::AppHandle,
    before: &PersistedState,
    after: &PersistedState,
) -> Result<(), String> {
    let (claims_before, claims_after) = crate::state::update(app, |live| {
        let claims_before = live.claims.clone();
        crate::state::apply_writer_delta(live, before, after);
        crate::todo_trees::reconcile(live);
        Ok((claims_before, live.claims.clone()))
    })?;
    event_log::diff_claims(app, &claims_before, &claims_after);
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// A non-empty projectId must name a real project — a typo would orphan
/// the card off every planet and the inbox. "" is the inbox.
fn check_project(state: &PersistedState, project_id: &str) -> Result<(), String> {
    if !project_id.is_empty() && !state.projects.iter().any(|p| p.id == project_id) {
        return Err(format!("unknown project id {project_id:?}"));
    }
    Ok(())
}

/// `kanban.add` projectId resolve — "" is the
/// inbox; an existing project id passes through; anything else is tried
/// as a trim / case-insensitive display name (`fkt`, `orbitty`,
/// `practice_desk`) and must match exactly one planet. Unknown or
/// ambiguous = honest error, no mint. Returns the id to store.
/// Shared with agent.add / agent.pin — `who`
/// labels the honest error with the calling tool.
pub(crate) fn resolve_project(
    state: &PersistedState,
    project_id: &str,
    who: &str,
) -> Result<String, String> {
    let raw = project_id.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    if state.projects.iter().any(|p| p.id == raw) {
        return Ok(raw.to_string());
    }
    let named: Vec<&Project> = state
        .projects
        .iter()
        .filter(|p| p.name.trim().eq_ignore_ascii_case(raw))
        .collect();
    match named.len() {
        0 => Err(format!(
            "{who}: unknown project {raw:?} (need a project id or a unique planet name)"
        )),
        1 => Ok(named[0].id.clone()),
        n => Err(format!(
            "{who}: ambiguous project name {raw:?} — {n} planets match; use the id"
        )),
    }
}

/// `kanban.add` — mint `kind: "todo"`, `actor: "agent"`. `status` is
/// `todo` (default) or `doing`; done is never minted (it is earned through
/// the complete door). `projectId` is an id OR a unique planet name
/// (the resolve above). Logs the mint's board proof — 420 for
/// todo, 421 for doing (never a 31200 board-add).
/// `notes` is the optional one-line runner summary
/// (first line paints as the board row's `.b-notes`); empty/None = none.
pub fn kanban_add(
    app: &tauri::AppHandle,
    title: &str,
    project_id: &str,
    status: &str,
    notes: Option<&str>,
    links: Vec<CardLink>,
) -> Result<KanbanCard, String> {
    kanban_add_as(app, title, project_id, status, "agent", notes, links)
}

/// Same door as `kanban.add`; `actor` is who minted ("agent" for MCP/ctl,
/// "user" for the composer `todo:` intercept).
pub fn kanban_add_as(
    app: &tauri::AppHandle,
    title: &str,
    project_id: &str,
    status: &str,
    actor: &str,
    notes: Option<&str>,
    links: Vec<CardLink>,
) -> Result<KanbanCard, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("kanban.add: empty title".into());
    }
    let status = match status.trim() {
        "" | "todo" => "todo",
        "doing" => "doing",
        other => {
            return Err(format!(
                "kanban.add: status must be todo|doing, got {other:?} (done is earned via kanban.move)"
            ))
        }
    };
    let links = normalize_new_links(links)?;
    let before = crate::state::load_state(app)?;
    let project_id = resolve_project(&before, project_id, "kanban.add")?;
    let now = now_secs();
    let card = KanbanCard {
        id: new_id("c"),
        title: title.to_string(),
        project_id: project_id.clone(),
        window_id: None,
        kind: Some("todo".into()),
        status: status.into(),
        created_at: now,
        last_touched_at: now,
        completed_at: None,
        goal_aligned: true,
        clinical_or_master: false,
        points_awarded: 0,
        starred: false,
        notes: notes
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        difficulty: None,
        subtasks: None,
        due_at: None,
        tag_ids: None,
        needed_state: None,
        needed_why: None,
        needed_answer: None,
        actor: Some(actor.trim().to_string()),
        assigned_agent_id: None,
        parent_id: None,
        rating: None,
        trackers: None,
        task_id: None,
        task_reason: None,
        links,
    };
    let mut after = before.clone();
    after.kanban_cards.push(card.clone());
    save_with_events(app, &before, &after)?;
    // A mint writes the target column's proof
    // (420 todo / 421 doing) with `t=orbitty`, not a 31200 board-add line.
    event_log::append_board_proof_best(
        app,
        &card.id,
        &card.status,
        None,
        Some(&card.project_id),
        Some(actor.trim()),
        &serde_json::json!({ "cardId": card.id, "title": card.title }),
    );
    Ok(card)
}

/// `kanban.move` — the board's column rules. `done` is the same complete
/// door as the checkbox (award once per card, 1 pt / 2 clinical|master,
/// daily cap, `completedAt`, actor "agent") and does NOT unclaim — the
/// walk-off still narrates. Moving a window-carded row out of Doing
/// unclaims its window, exactly like the board drag. Same-column = the
/// board's early return (no save, no log). Logs the target column's board
/// proof — 420/421/422 with `t=orbitty` (never a
/// 31200 board-move).
pub fn kanban_move(app: &tauri::AppHandle, id: &str, target: &str) -> Result<KanbanCard, String> {
    let target = match target.trim() {
        "todo" | "doing" | "done" => target.trim(),
        other => {
            return Err(format!(
                "kanban.move: status must be todo|doing|done, got {other:?}"
            ))
        }
    };
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| format!("kanban.move: unknown card id {id:?}"))?;
    let from = before.kanban_cards[idx].status.clone();
    if from == target {
        return Ok(before.kanban_cards[idx].clone());
    }
    let project_id = before.kanban_cards[idx].project_id.clone();
    let now = now_secs();
    let mut after = before.clone();
    if target == "done" {
        // Complete door — the points math reads the pre-move snapshot.
        let key = today_key();
        let today_pts: u32 = before
            .points_ledger
            .iter()
            .filter(|e| e.day == key)
            .map(|e| e.amount)
            .sum();
        let already = before.points_ledger.iter().any(|e| e.card_id == id);
        let base = if before.kanban_cards[idx].clinical_or_master {
            2
        } else {
            1
        };
        let award = if !already && today_pts + base <= DAILY_CAP {
            base
        } else {
            0
        };
        let c = &mut after.kanban_cards[idx];
        c.status = "done".into();
        c.last_touched_at = now;
        c.completed_at = Some(now);
        c.points_awarded += award;
        c.actor = Some("agent".into());
        if award > 0 {
            after.points_ledger.push(PointsEntry {
                id: new_id("pt"),
                card_id: id.to_string(),
                amount: award,
                day: key,
                at: now,
                actor: Some("agent".into()),
            });
        }
        // No unclaim: the walk-off narrates first; the rail's
        // habitat:walked-off handler unclaims (the existing Done eviction).
    } else {
        let window_id = before.kanban_cards[idx].window_id.clone();
        {
            let c = &mut after.kanban_cards[idx];
            c.status = target.into();
            c.completed_at = None;
            c.last_touched_at = now;
            c.window_id = if target == "doing" {
                window_id.clone()
            } else {
                None
            };
        }
        // Board drag rule: a window-carded row leaving Doing unclaims its
        // window — the board never lies about linkage. diff_claims logs it.
        if from == "doing" {
            if let Some(w) = window_id {
                after.claims.retain(|cl| cl.window_id != w);
            }
        }
    }
    save_with_events(app, &before, &after)?;
    if target == "done" || target == "todo" {
        crate::task::end_card(
            app,
            &after.kanban_cards[idx],
            if target == "done" { "done" } else { "cancelled" },
        );
    }
    event_log::append_board_proof_best(
        app,
        id,
        target,
        None,
        Some(&project_id),
        Some("agent"),
        &serde_json::json!({ "from": from, "to": target, "cardId": id }),
    );
    // Doing-fire + walker attach after this move returns (same work,
    // not on the drop's IPC wait).
    crate::doing_fire::spawn_fire_and_attach(app, before, after.clone());
    Ok(after.kanban_cards[idx].clone())
}

/// `kanban.notes` — set a card's `notes` (the first
/// line is the board row's one-line summary; the drawer edits the rest).
/// Empty-after-trim clears, exactly like the drawer (notes → None). Same
/// one save door as add/move; a no-change write is the board's early
/// return (no save, no log). Logs `board-notes` with the id + cleared
/// flag only — never the text (small JSON, no PHI).
pub fn kanban_notes(app: &tauri::AppHandle, id: &str, notes: &str) -> Result<KanbanCard, String> {
    let notes = notes.trim();
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == id.trim())
        .ok_or_else(|| format!("kanban.notes: unknown card id {id:?}"))?;
    if before.kanban_cards[idx].notes.as_deref().unwrap_or("") == notes {
        return Ok(before.kanban_cards[idx].clone());
    }
    let mut after = before.clone();
    after.kanban_cards[idx].notes = if notes.is_empty() {
        None
    } else {
        Some(notes.to_string())
    };
    after.kanban_cards[idx].last_touched_at = now_secs();
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "board-notes",
        None,
        Some(&after.kanban_cards[idx].project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "cardId": after.kanban_cards[idx].id, "cleared": notes.is_empty() }),
    );
    Ok(after.kanban_cards[idx].clone())
}

fn normalize_new_links(links: Vec<CardLink>) -> Result<Option<Vec<CardLink>>, String> {
    if links.is_empty() {
        return Ok(None);
    }
    if links.len() > MAX_LINKS {
        return Err(format!("kanban.add: a card holds at most {MAX_LINKS} links"));
    }
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        out.push(CardLink {
            url: card_links::normalize_link(&link.url).map_err(|e| format!("kanban.add: {e}"))?,
            label: card_links::clean_label(link.label).map_err(|e| format!("kanban.add: {e}"))?,
        });
    }
    Ok(Some(out))
}

/// `kanban.link` — add, remove, or clear. Same write door as `kanban.notes`.
/// Does not touch `notes` or `status`.
pub fn kanban_link_add(
    app: &tauri::AppHandle,
    id: &str,
    url: &str,
    label: Option<String>,
) -> Result<KanbanCard, String> {
    let url = card_links::normalize_link(url).map_err(|e| format!("kanban.link: {e}"))?;
    let label = card_links::clean_label(label).map_err(|e| format!("kanban.link: {e}"))?;
    let before = crate::state::load_state(app)?;
    let idx = card_index(&before, id, "kanban.link")?;
    let mut links = before.kanban_cards[idx].links.clone().unwrap_or_default();
    if let Some(existing) = links.iter_mut().find(|l| l.url == url) {
        if existing.label == label {
            return Ok(before.kanban_cards[idx].clone());
        }
        existing.label = label;
    } else {
        if links.len() >= MAX_LINKS {
            return Err(format!("kanban.link: a card holds at most {MAX_LINKS} links"));
        }
        links.push(CardLink { url, label });
    }
    write_links(app, &before, idx, Some(links), "add")
}

pub fn kanban_link_rm(app: &tauri::AppHandle, id: &str, url: &str) -> Result<KanbanCard, String> {
    let url = card_links::normalize_link(url).map_err(|e| format!("kanban.link: {e}"))?;
    let before = crate::state::load_state(app)?;
    let idx = card_index(&before, id, "kanban.link")?;
    let mut links = before.kanban_cards[idx].links.clone().unwrap_or_default();
    let n = links.len();
    links.retain(|l| l.url != url);
    if links.len() == n {
        return Err(format!("kanban.link: link not on card {id:?}"));
    }
    write_links(app, &before, idx, if links.is_empty() { None } else { Some(links) }, "rm")
}

pub fn kanban_link_clear(app: &tauri::AppHandle, id: &str) -> Result<KanbanCard, String> {
    let before = crate::state::load_state(app)?;
    let idx = card_index(&before, id, "kanban.link")?;
    if before.kanban_cards[idx].links.is_none() {
        return Ok(before.kanban_cards[idx].clone());
    }
    write_links(app, &before, idx, None, "clear")
}

/// Forwards `run` to the launcher. orbittyctl and the MCP tool both come
/// through here. Reads link-approvals.json and never writes it.
pub(crate) fn dispatch_kanban_open(
    url: &str,
    run: bool,
    approvals: &std::path::Path,
) -> Result<String, String> {
    let url = url.to_string();
    card_links::ctl_open_at(&url, run, approvals)
}

/// Open link `n` (default 0). Read-only — status stays put.
/// Documents, folders, images, and https open. An executable is revealed
/// in Finder unless `run` is set or that exact file is already approved.
/// Reads link-approvals.json and never writes it.
pub fn kanban_open_link(
    app: &tauri::AppHandle,
    id: &str,
    n: usize,
    run: bool,
) -> Result<serde_json::Value, String> {
    let before = crate::state::load_state(app)?;
    let idx = card_index(&before, id, "kanban.open_link")?;
    let card = &before.kanban_cards[idx];
    let links = card.links.as_deref().unwrap_or(&[]);
    let link = links.get(n).ok_or_else(|| {
        format!("kanban.open_link: card {id:?} has no link at n={n}")
    })?;
    let url = link.url.clone();
    let status = card.status.clone();
    let card_id = card.id.clone();
    let action = dispatch_kanban_open(&url, run, &card_links::approvals_path(app)?)?;
    Ok(serde_json::json!({
        "id": card_id,
        "n": n,
        "url": url,
        "status": status,
        "action": action,
    }))
}

fn card_index(state: &PersistedState, id: &str, who: &str) -> Result<usize, String> {
    state
        .kanban_cards
        .iter()
        .position(|c| c.id == id.trim())
        .ok_or_else(|| format!("{who}: unknown card id {id:?}"))
}

fn write_links(
    app: &tauri::AppHandle,
    before: &PersistedState,
    idx: usize,
    links: Option<Vec<CardLink>>,
    op: &str,
) -> Result<KanbanCard, String> {
    let mut after = before.clone();
    after.kanban_cards[idx].links = links;
    after.kanban_cards[idx].last_touched_at = now_secs();
    let card_id = after.kanban_cards[idx].id.clone();
    let project_id = after.kanban_cards[idx].project_id.clone();
    save_with_events(app, before, &after)?;
    event_log::append_best(
        app,
        "board-link",
        None,
        Some(&project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "cardId": card_id, "op": op }),
    );
    Ok(after.kanban_cards[idx].clone())
}

/// The locked one-liners for the GO-ORBITTY-*
/// runner todos — what the runner is and does, first line of `notes`.
const GO_CARD_BLURBS: &[(&str, &str)] = &[
    ("GO-ORBITTY-ATTACH", "One file chip on Spark, original path, gone after send. 25 MB cap."),
    ("GO-ORBITTY-SHIP-SLIM", "NES-tiny blessed app (strip, GLBs out, icns). Not habitat behavior."),
    ("GO-ORBITTY-DEAD-DRAWERS", "Diet #2: delete unused drawer/dock stubs. Behavior unchanged."),
    ("GO-ORBITTY-DEAD-INVOKE", "Diet #3: drop Tauri commands the UI never calls."),
    ("GO-ORBITTY-CHROME-CSS", "Diet #4: one shared wordmark/winx CSS. No restyle."),
    ("GO-ORBITTY-COMMENT-DIET", "Diet #5: strip GO history comments; keep constraint lines."),
    ("GO-ORBITTY-SLEEP-PRESETS", "Unused planets marble after 1h/4h/12h/Never. Default 4h."),
    ("GO-ORBITTY-STARS-FIVE", "STARS spokes 1–5. Empty day stays empty."),
    ("GO-ORBITTY-NEEDED-RULES-ANSWER", "After an answer, “if this again” without typing RULE:."),
    ("GO-ORBITTY-DOCK-CENTER", "Optional: dests sit on the dock after rail is quiet."),
    ("GO-ORBITTY-TASK-BOUNDARY", "Snapshot planet root before/after a file-writing task. Undo that task."),
    ("GO-ORBITTY-GATE-CAPS", "$20/day default, 2 runners (max 4). Needed is the gate."),
    ("GO-ORBITTY-CHEATSHEET", "⌘? one-screen keys after cockpit mails."),
    ("GO-ORBITTY-BOARD-PROOFS", "Todo/doing/done as Nostr 420/421/422 (`t=orbitty`). Journal stays 31200."),
    ("GO-ORBITTY-DOING-WALKER", "Drag doing → android on that planet. Needed/blocked → near rim."),
    ("GO-ORBITTY-DROID-GLANCE", "Droids read working / blocked / needs-you at a glance. NES walkers stay open/sleep/done."),
    ("GO-ORBITTY-HERDR-ADAPTER", "Herdr if present (skip+URL). OpenCode detect-only."),
    ("GO-ORBITTY-MOUTH-HOP", "Thin router: local → OpenRouter free → paid Grok. Same bubble."),
    ("GO-ORBITTY-KANBAN-GRAB", "Shift+Enter grab, arrows move, Enter drop, Esc ungrab."),
    ("GO-ORBITTY-CARD-BLURB", "Board row shows a one-line runner summary (`notes`)."),
    ("GO-ORBITTY-SHIFT", "Humming supervisor via ctl: watch board + agentRuns + AX, cap runners, park stuck, next pickup, Needed escalate. No LLM in habitat_mac."),
    ("GO-ORBITTY-ATTACH-MD", "MarkItDown sidecar for PDF/Word/Excel/PPT if on PATH. Path-only if missing. No nag."),
    ("GO-ORBITTY-ATTACH-VISION", "Photos via mouth vision first (Grok --prompt-json). OCR not every image."),
    ("GO-ORBITTY-RELAY-SIDECAR", "Loopback NIP-01 sidecar for t=orbitty 420/421/422. LaunchAgent. jsonl stays SoT. Not FKT/NCH."),
];

/// A backfillable GO title is exactly the token — `GO-ORBITTY-FOO`, never
/// `GO-ORBITTY-FOO leftover: …` (a suffixed row is Matt's own edit).
fn is_exact_go_token(title: &str) -> bool {
    title
        .strip_prefix("GO-ORBITTY-")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'))
}

/// Fallback for tokens outside the table: the first non-empty `Matt:` line
/// of the nest prompt file `~/.buzz/PLANS/_GO_<TOKEN>_PROMPT.md`
/// (dash→underscore slug, same PLANS dir needed_rules uses).
fn go_prompt_blurb(title: &str) -> Option<String> {
    let path = std::path::PathBuf::from(std::env::var("HOME").ok()?)
        .join(".buzz/PLANS")
        .join(format!("_{}_PROMPT.md", title.replace('-', "_")));
    let raw = std::fs::read_to_string(path).ok()?;
    raw.lines()
        .filter_map(|l| l.trim().strip_prefix("Matt:"))
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

/// One-time startup backfill — existing
/// `kind: "todo"` cards titled exactly `GO-ORBITTY-*` with EMPTY notes get
/// the one-liner (table first, then the nest prompt file's first `Matt:`
/// line, else left empty). A note Matt already typed is never overwritten.
/// Best-effort: a load/save failure only skips the backfill, never boot.

/// Bind a todo to a claimed window (or clear the bind).
/// Non-empty `window_id`: set `window_id`, move status to `doing`, ensure
/// a Layer-0 claim on the card's planet. Empty: clear `window_id` and,
/// if status was `doing`, drop back to `todo` (claim left alone — unclaim
/// is its own door).
pub fn kanban_assign_window(
    app: &tauri::AppHandle,
    card_id: &str,
    window_id: &str,
) -> Result<KanbanCard, String> {
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == card_id.trim())
        .ok_or_else(|| format!("kanban.assign_window: unknown card id {card_id:?}"))?;
    let card = &before.kanban_cards[idx];
    if card.kind.as_deref() == Some("needed") {
        return Err("kanban.assign_window: Needed cards cannot bind a window".into());
    }
    let window_id = window_id.trim();
    let mut after = before.clone();
    if window_id.is_empty() {
        let c = &mut after.kanban_cards[idx];
        c.window_id = None;
        if c.status == "doing" {
            c.status = "todo".into();
        }
        if c.kind.as_deref() == Some("window") {
            c.kind = Some("todo".into());
        }
        c.last_touched_at = now_secs();
    } else {
        // One open card per window.
        for (i, c) in after.kanban_cards.iter_mut().enumerate() {
            if i != idx && c.window_id.as_deref() == Some(window_id) && c.status != "done" {
                c.window_id = None;
                if c.status == "doing" {
                    c.status = "todo".into();
                }
            }
        }
        let project_id = after.kanban_cards[idx].project_id.clone();
        {
            let c = &mut after.kanban_cards[idx];
            c.window_id = Some(window_id.to_string());
            c.status = "doing".into();
            c.completed_at = None;
            c.last_touched_at = now_secs();
            // Real todo bind — not the legacy auto window ticket.
            if c.kind.as_deref() != Some("todo") {
                c.kind = Some("todo".into());
            }
        }
        // Window must already be claimed (bare claim → grass). Binding a
        // todo retargets that claim onto the card's planet.
        if !after.claims.iter().any(|cl| cl.window_id == window_id) {
            return Err(format!(
                "kanban.assign_window: window {window_id:?} is not claimed — claim it first"
            ));
        }
        if let Some(cl) = after.claims.iter_mut().find(|cl| cl.window_id == window_id) {
            cl.project_id = project_id.clone();
            cl.last_touched_at = now_secs();
        }
        after.last_claim_project_id = Some(project_id);
    }
    save_with_events(app, &before, &after)?;
    crate::event_log::append_best(
        app,
        "kanban-assign-window",
        None,
        Some(&after.kanban_cards[idx].project_id),
        Some("agent"),
        true,
        &serde_json::json!({
            "cardId": after.kanban_cards[idx].id,
            "windowId": after.kanban_cards[idx].window_id,
            "status": after.kanban_cards[idx].status,
        }),
    );
    Ok(after.kanban_cards[idx].clone())
}

pub fn backfill_go_card_notes(app: &tauri::AppHandle) {
    let before = match crate::state::load_state(app) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("card-blurb backfill: load_state: {e}");
            return;
        }
    };
    let mut after = before.clone();
    let mut filled = 0u32;
    for c in &mut after.kanban_cards {
        if c.kind.as_deref() != Some("todo") || !is_exact_go_token(&c.title) {
            continue;
        }
        if c.notes.as_deref().is_some_and(|n| !n.trim().is_empty()) {
            continue; // Matt's note wins — never overwrite
        }
        let blurb = GO_CARD_BLURBS
            .iter()
            .find(|(t, _)| *t == c.title)
            .map(|(_, b)| (*b).to_string())
            .or_else(|| go_prompt_blurb(&c.title));
        if let Some(b) = blurb {
            c.notes = Some(b);
            filled += 1;
        }
    }
    if filled == 0 {
        return;
    }
    if let Err(e) = save_with_events(app, &before, &after) {
        eprintln!("card-blurb backfill: save: {e}");
        return;
    }
    // Count only — card text stays out of the log (small JSON, no PHI).
    event_log::append_best(
        app,
        "board-notes-backfill",
        None,
        None,
        Some("agent"),
        true,
        &serde_json::json!({ "filled": filled }),
    );
}

/// `project.add` — mint a planet row
/// `{ id: new_id("p"), name }` (the rail's createProject id shape). Empty
/// name refused. Leftover: a same-name row (trim, case-insensitive) is
/// reused, never twinned — newest id wins if several — and either way the
/// disc is fronted + locate-glowed (a mint must surface through covering
/// windows). Logs `project-add` on a real mint only.
pub fn project_add(app: &tauri::AppHandle, name: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("project.add: empty name".into());
    }
    let before = crate::state::load_state(app)?;
    // Dedup: the display name (trim, case-insensitive) already has a
    // planet — return it instead of minting a twin. Newest id wins (ids
    // are `p-<base36 millis>-<rand>`, so max id = latest mint). Nothing
    // is saved; the existing disc still fronts + glows.
    if let Some(existing) = before
        .projects
        .iter()
        .filter(|p| p.name.trim().eq_ignore_ascii_case(name))
        .max_by(|a, b| a.id.cmp(&b.id))
    {
        let project = existing.clone();
        crate::habitat::bring_planet_front(&project.id);
        let _ = crate::habitat::locate_planet(app.clone(), project.id.clone());
        return Ok(project);
    }
    let project = Project {
        id: new_id("p"),
        name: name.to_string(),
        trackers: None,
        workspace_root: None,
    };
    let mut after = before.clone();
    after.projects.push(project.clone());
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-add",
        None,
        Some(&project.id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": project.id, "name": project.name }),
    );
    // Front + locate-glow the new disc. The panel may not exist until the
    // next habitat sync — bring is then a no-op and this arm only catches
    // a sync inside the TTL, so do_sync's new-panel path arms the glow
    // again on insert (first-blit guarantee).
    crate::habitat::bring_planet_front(&project.id);
    let _ = crate::habitat::locate_planet(app.clone(), project.id.clone());
    Ok(project)
}

/// `project.rm` — drop the row from `projects`.
/// Cards keep their `projectId`; live claims stay (walkers vanish with
/// the disc on the next habitat sync). Never auto-minted, never
/// auto-killed — ctl is the door. Unknown id = honest error. Logs
/// `project-rm`.
pub fn project_rm(app: &tauri::AppHandle, id: &str) -> Result<Project, String> {
    let before = crate::state::load_state(app)?;
    let idx = before
        .projects
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("project.rm: unknown project id {id:?}"))?;
    let gone = before.projects[idx].clone();
    let mut after = before.clone();
    after.projects.remove(idx);
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-rm",
        None,
        Some(&gone.id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": gone.id, "name": gone.name }),
    );
    Ok(gone)
}

/// `project.rename` — change `name`, keep `id`.
/// Empty name = error; unknown id = honest error. Logs `project-rename`.
pub fn project_rename(app: &tauri::AppHandle, id: &str, name: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("project.rename: empty name".into());
    }
    let before = crate::state::load_state(app)?;
    let idx = before
        .projects
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("project.rename: unknown project id {id:?}"))?;
    let mut after = before.clone();
    after.projects[idx].name = name.to_string();
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-rename",
        None,
        Some(id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": id, "name": name }),
    );
    Ok(after.projects[idx].clone())
}

/// `needed.open` — mint `kind: "needed"`, `neededState: "open"`,
/// `status: "todo"`, `windowId: null` (the Needed inbox shape). Logs
/// `needed-open`.
/// The user-minted standing rules get first
/// claim (needed_rules.rs — classify/match/can-run live there, not here).
/// One hit + can-run → the card is born `answered` in the SAME save,
/// `neededAnswer` = the rule's answer, `actor: "rule"`, and `needed-answer`
/// is emitted so the existing watch → nest clerk / writer dispatch acts.
/// Miss / collide / cannot-run / unreadable store → mint `open` as today.
pub fn needed_open(
    app: &tauri::AppHandle,
    title: &str,
    project_id: &str,
    why: Option<&str>,
) -> Result<KanbanCard, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("needed.open: empty title".into());
    }
    let before = crate::state::load_state(app)?;
    check_project(&before, project_id)?;
    if let Some(existing) = before.kanban_cards.iter().find(|c| {
        c.kind.as_deref() == Some("needed")
            && c.needed_state.as_deref() == Some("open")
            && c.title.trim() == title
    }) {
        return Ok(existing.clone());
    }
    let ruled =
        match crate::needed_rules::decide(title, project_id, why.unwrap_or(""), &before.projects)
        {
            crate::needed_rules::Decision::Apply(answer) => Some(answer),
            crate::needed_rules::Decision::Open => None,
        };
    let now = now_secs();
    let card = KanbanCard {
        id: new_id("n"),
        title: title.to_string(),
        project_id: project_id.to_string(),
        window_id: None,
        kind: Some("needed".into()),
        status: "todo".into(),
        created_at: now,
        last_touched_at: now,
        completed_at: None,
        goal_aligned: false,
        clinical_or_master: false,
        points_awarded: 0,
        starred: false,
        notes: None,
        difficulty: None,
        subtasks: None,
        due_at: None,
        tag_ids: None,
        needed_state: Some(if ruled.is_some() { "answered" } else { "open" }.into()),
        needed_why: why
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string),
        needed_answer: ruled.clone(),
        actor: Some(if ruled.is_some() { "rule" } else { "agent" }.into()),
        assigned_agent_id: None,
        parent_id: None,
        rating: None,
        trackers: None,
        task_id: None,
        task_reason: None,
        links: None,
    };
    let mut after = before.clone();
    after.kanban_cards.push(card.clone());
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "needed-open",
        None,
        Some(&card.project_id),
        card.actor.as_deref(),
        true,
        &serde_json::json!({ "cardId": card.id, "title": card.title }),
    );
    if ruled.is_some() {
        // The answer event rides the same save — the watch dispatches the
        // standing answer exactly as if Matt typed it. No walker sits, no
        // NEXT-CARD pops (neededState is answered), no companion send.
        event_log::append_best(
            app,
            "needed-answer",
            None,
            Some(&card.project_id),
            Some("rule"),
            true,
            &serde_json::json!({ "cardId": card.id }),
        );
    }
    Ok(card)
}

/// Answer a Needed from the companion composer. Logs `needed-answer` and
/// writes a short companion caption so the mouth shows the line without
/// pinging the model.
pub fn needed_answer(
    app: &tauri::AppHandle,
    id: &str,
    answer: &str,
) -> Result<KanbanCard, String> {
    let raw = answer.trim();
    if raw.is_empty() {
        return Err("needed.answer: empty answer".into());
    }
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == id.trim())
        .ok_or_else(|| format!("needed.answer: unknown card id {id:?}"))?;
    if before.kanban_cards[idx].kind.as_deref() != Some("needed") {
        return Err("needed.answer: that card is not a Needed".into());
    }
    let answer = crate::needed_rules::apply_composer_rule(&before.kanban_cards[idx], raw);
    if answer.is_empty() {
        return Err("needed.answer: empty answer".into());
    }
    let now = now_secs();
    let mut after = before.clone();
    {
        let c = &mut after.kanban_cards[idx];
        c.needed_answer = Some(answer.to_string());
        c.needed_state = Some("answered".into());
        c.last_touched_at = now;
    }
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "needed-answer",
        None,
        Some(&after.kanban_cards[idx].project_id),
        Some("user"),
        true,
        &serde_json::json!({ "cardId": after.kanban_cards[idx].id }),
    );
    Ok(after.kanban_cards[idx].clone())
}

#[cfg(test)]
mod tests {
    use super::dispatch_kanban_open;

    fn temp_case(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "orbitty-link-board-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn kanban_open_passes_the_run_flag() {
        let _hook = crate::card_links::OpenGuard::arm();
        let dir = temp_case("run");
        let approvals = dir.join("link-approvals.json");
        let seeded = b"{\"note\":\"n\",\"approvals\":[]}\n";
        std::fs::write(&approvals, seeded).unwrap();
        let cmd = dir.join("hello.command");
        std::fs::write(&cmd, "echo hi\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&cmd).unwrap().permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&cmd, perms).unwrap();
        }
        let before = std::fs::read(&approvals).unwrap();
        assert_eq!(
            dispatch_kanban_open(&cmd.to_string_lossy(), false, &approvals).unwrap(),
            "reveal"
        );
        assert_eq!(std::fs::read(&approvals).unwrap(), before);
        assert_eq!(
            dispatch_kanban_open(&cmd.to_string_lossy(), true, &approvals).unwrap(),
            "open"
        );
        assert_eq!(std::fs::read(&approvals).unwrap(), before);
        let src = include_str!("board.rs");
        assert!(src.contains("dispatch_kanban_open(&url, run,"));
        assert!(src.contains("card_links::ctl_open_at(&url, run,"));
        let add = src.find("pub fn kanban_link_add(").expect("kanban_link_add");
        let add_body = &src[add..add + src[add..].find("pub fn kanban_link_rm(").unwrap()];
        assert!(!add_body.contains("link-approvals"));
        assert!(!add_body.contains("remember("));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
