//! GO-ORBITTY-MOUTH-FOREVER: the companion mouth never tuckers.
//!
//! The bubble is ONE continuous thread (tmux); a backend session may still
//! die (context-dead / session-dead / empty-after-resume). When it does,
//! the mouth ROLLS OVER: a fresh session id lands in plug.json and the
//! first prompt on the new session is a handoff block — the last ~6 turns
//! of the ACTIVE mode plus the pocket/desk-wrapped user line. The reply
//! still appends to the same `turns` / `threads[mode]`; the handoff is
//! never persisted as a you-row and `companion.turns` is never wiped.
//!
//! Grok: `--fork-session` (resume the old id, name the fork) when the old
//! session may still exist, else a fresh `-s` UUID. Goose: a new `--name`
//! session, the real id re-captured from `goose session list`. The
//! 180s/300s guards stay PER-SEND timeouts — never a `--max-turns` cap.

use std::path::Path;

use crate::coordinator::{
    PlugState, capture_goose_session, goose_args, grok_args, grok_reply_text, is_uuid, mint_uuid,
    run_mouth_cli, save_plug,
};

/// True when a grok failure says the resume target is GONE — grok fails
/// FAST here (~0.5s: `session get failed: 404 Not Found`), never a slow
/// generate. Fail fast means: mint a fresh id, `-s`, save plug.json —
/// forking a corpse would 404 too, so the fork path is skipped.
pub fn grok_session_gone(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("not found")
        || e.contains("no such")
        || e.contains("404")
        || e.contains("invalid")
        || e.contains("expired")
}

/// True when a grok failure reads as context/session death — never a
/// normal model error. Consulted ONLY on the resume path (a first-send
/// failure stays an honest Err). The 180s kill is a SLOW GENERATE, not
/// death: "took longer" / "killed" never match here, so a live-but-slow
/// send stays an honest timeout Err instead of rolling over with a fat
/// handoff, and a first `-s` create that dies at 180s is never treated
/// as "resume the corpse" on the next send.
pub fn grok_context_dead(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("session")
        || e.contains("context")
        || e.contains("not found")
        || e.contains("expired")
        || e.contains("max turns")
}

/// True when a goose failure reads as session death. The 300s timeout is
/// NOT death here — with full tools loaded a long reply can just be slow;
/// only resume-of-a-gone-session and context complaints roll over.
pub fn goose_context_dead(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("session")
        || e.contains("not found")
        || e.contains("no such")
        || e.contains("context")
        || e.contains("expired")
        || e.contains("invalid")
}

/// Handoff block for the first prompt on a rolled-over session: the last
/// ~20 turns of the ACTIVE mode (`companion.turns` IS the active thread —
/// MODE-SPLIT mirrors it into `threads[mode]`), then the already
/// pocket/desk-wrapped user line. Persisted nowhere — the send path saves
/// only the raw line + reply, so the handoff never becomes a you-row.
/// No history yet (a first send that died) → the wrapped line unchanged.
pub fn handoff_prompt(app: &tauri::AppHandle, wrapped_line: &str) -> String {
    const HANDOFF_TURNS: usize = 6;
    const HISTORY_CAP: usize = 1200;
    let (mode, turns) = crate::state::load_state(app)
        .map(|s| (s.companion.mode.clone(), s.companion.turns.clone()))
        .unwrap_or_else(|_| ("spark".into(), Vec::new()));
    if turns.is_empty() {
        return wrapped_line.to_string();
    }
    let start = turns.len().saturating_sub(HANDOFF_TURNS);
    // Newest-first budget fill, then flip back to chronological.
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0usize;
    for t in turns[start..].iter().rev() {
        let text = t.text.replace('\n', " ");
        let low = text.to_ascii_lowercase();
        if low.starts_with("noted ")
            || low.starts_with("heard you")
            || low.contains("companion stays the mouth")
            || low.starts_with("dogfood ") && low.contains("is in needed")
        {
            continue;
        }
        let who = if t.role == "you" { "you" } else { "them" };
        let row = format!("{who}: {text}");
        let n = row.chars().count();
        if !lines.is_empty() && used + n > HISTORY_CAP {
            break;
        }
        used += n;
        lines.push(row);
    }
    lines.reverse();
    format!(
        "[handoff — the prior backend session died; this is the same desk thread (mode: {mode}) \
         on a fresh session. The transcript so far is below. Answer the USER line at the end; \
         do not greet, do not summarize.]\n{}\n\n{wrapped_line}",
        lines.join("\n")
    )
}

/// `grok -p <handoff> --resume <old> --fork-session -s <new> …`: resume the
/// dying session but fork it into a fresh id — history carries, the old id
/// is left alone. (`-s` with `--resume` is valid only with --fork-session;
/// it names the fork.)
fn grok_fork_args(old_sid: &str, new_sid: &str, soul: &str, line: &str) -> Vec<String> {
    let mut args: Vec<String> = vec!["-p".into(), line.to_string()];
    args.push("--resume".into());
    args.push(old_sid.to_string());
    args.push("--fork-session".into());
    args.push("-s".into());
    args.push(new_sid.to_string());
    args.extend([
        "--output-format".into(),
        "json".into(),
        "--always-approve".into(),
        "--no-subagents".into(),
        "--disable-web-search".into(),
        // Equals form, one argv — a dash-leading soul stays a value.
        format!("--rules={soul}"),
    ]);
    args
}

/// Roll a dead grok mouth onto a fresh session id. Fork when the old id
/// still resumes (history carries; the handoff re-aims the reply), else a
/// fresh `-s` where the handoff IS the context. plug.json always lands the
/// new id, marked minted. One shot — a failing rollover stays an honest Err.
pub fn grok_rollover(
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    let old_sid = plug.grok_session_id.clone().unwrap_or_default();
    let new_sid = mint_uuid()?;
    if is_uuid(&old_sid) {
        let args = grok_fork_args(&old_sid, &new_sid, soul, handoff);
        if let Ok(raw) = run_mouth_cli("Grok", bin, &args, &[], 180) {
            let text = grok_reply_text(&raw);
            if !text.is_empty() {
                plug.grok_session_id = Some(new_sid);
                plug.grok_session_minted = true;
                save_plug(plug);
                return Ok(text);
            }
        }
        // Fork missed (resume target gone/thin) — fall through to fresh.
    }
    let args = grok_args(&new_sid, false, soul, handoff);
    let raw = run_mouth_cli("Grok", bin, &args, &[], 180)?;
    let text = grok_reply_text(&raw);
    if text.is_empty() {
        return Err("Grok sent back an empty reply.".into());
    }
    plug.grok_session_id = Some(new_sid);
    plug.grok_session_minted = true;
    save_plug(plug);
    Ok(text)
}

/// Roll a dead goose mouth: drop the dead id, send the handoff on a fresh
/// `--name` session, then re-capture the real YYYYMMDD_N id into plug.json
/// (`--name` is not a unique resume key — same capture as the first send).
pub fn goose_rollover(
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    plug.goose_session_id = None;
    save_plug(plug);
    let args = goose_args(soul, handoff, None);
    let reply = run_mouth_cli("Goose", bin, &args, &[("GOOSE_MODE", "always_allow")], 300)?;
    if reply.is_empty() {
        return Err("Goose sent back an empty reply.".into());
    }
    capture_goose_session(bin);
    Ok(reply)
}
