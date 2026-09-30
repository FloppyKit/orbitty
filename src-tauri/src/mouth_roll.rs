//! Session roll: new backend session, same bubble. Never wipe turns.
//! Timeouts are per-send, never a max-turns cap.

use std::path::Path;

use crate::coordinator::{
    PlugState, capture_goose_session, goose_args, grok_args, is_uuid, mint_uuid, run_mouth_cli,
    save_plug,
};

/// Streaming kill matcher for a grok `--resume` spawn: the dead-resume
/// line (`session get failed: 404 Not Found`) prints in ~0.5s but grok
/// STAYS ALIVE, so the watched runner kills on this line instead of
/// waiting out the 180s for a corpse. Narrower than `grok_session_gone`
/// on purpose — a live generate's tool noise can say "no such file"
/// with the session perfectly healthy, so the line must name the
/// session AND a gone word.
pub fn grok_session_gone_line(line: &str) -> bool {
    let e = line.to_ascii_lowercase();
    e.contains("session")
        && (e.contains("404")
            || e.contains("not found")
            || e.contains("no such")
            || e.contains("invalid")
            || e.contains("expired"))
}

/// True when a grok failure says the resume target is GONE — the watched
/// resume surfaces it FAST (~0.5s: `session get failed: 404 Not Found`),
/// never a slow generate. Fail fast means: mint a fresh id, `-s`, save
/// plug.json — forking a corpse would 404 too, so the fork path is
/// skipped.
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
/// ~6 turns of the one companion transcript, then the already
/// pocket/desk-wrapped user line (mode + now.json + pocket already in
/// that wrap). Persisted nowhere — the send path saves only the raw
/// line + reply, so the handoff never becomes a you-row.
/// No history yet (a first send that died) → the wrapped line unchanged.
pub fn handoff_prompt(app: &tauri::AppHandle, wrapped_line: &str) -> String {
    const HANDOFF_TURNS: usize = 6;
    const HISTORY_CAP: usize = 1200;
    let (mode, turns) = crate::state::load_state(app)
        .map(|s| (s.companion.mode.clone(), s.companion.turns.clone()))
        .unwrap_or_else(|_| ("spark".into(), Vec::new()));
    if turns.is_empty() || crate::mouth_gist::is_gist(wrapped_line) {
        return wrapped_line.to_string();
    }
    let start = turns.len().saturating_sub(HANDOFF_TURNS);
    // Newest-first budget fill, then flip back to chronological.
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0usize;
    for t in turns[start..].iter().rev() {
        let text = crate::mouth_hop::strip_handoff_marks(&t.text).replace('\n', " ");
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

/// `grok --single=<handoff> --resume <old> --fork-session -s <new> …`: resume the
/// dying session but fork it into a fresh id — history carries, the old id
/// is left alone. (`-s` with `--resume` is valid only with --fork-session;
/// it names the fork.)
fn grok_fork_args(
    old_sid: &str,
    new_sid: &str,
    soul: &str,
    line: &str,
    tools_ok: bool,
) -> Vec<String> {
    // Attached: `-p <line>` rejects a line starting with `-`.
    let mut args: Vec<String> = vec![format!("--single={line}")];
    args.push("--resume".into());
    args.push(old_sid.to_string());
    args.push("--fork-session".into());
    args.push("-s".into());
    args.push(new_sid.to_string());
    args.extend([
        "--output-format".into(),
        "streaming-json".into(),
        "--always-approve".into(),
        "--no-subagents".into(),
        "--disable-web-search".into(),
    ]);
    if !tools_ok {
        args.push("--disallowed-tools".into());
        args.push("bash,shell,computer".into());
    }
    args.extend([
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
    app: &tauri::AppHandle,
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    let old_sid = plug.grok_session_id.clone().unwrap_or_default();
    let new_sid = mint_uuid()?;
    if is_uuid(&old_sid) {
        let args = grok_fork_args(
            &old_sid,
            &new_sid,
            soul,
            handoff,
            crate::coordinator::companion_mode_tools(app),
        );
        // The fork RESUMES the old id — under the watch a corpse 404s in
        // ~0.5s and falls through to the fresh rollover, instead of
        // hanging out the 180s first.
        if let Ok(text) = crate::mouth_blip::run_streamed(
            app,
            "Grok",
            bin,
            &args,
            180,
            Some(&grok_session_gone_line),
        ) {
            if !text.is_empty() {
                plug.grok_session_id = Some(new_sid);
                plug.grok_session_minted = true;
                save_plug(plug);
                return Ok(text);
            }
        }
        // Fork missed (resume target gone/thin) — fall through to fresh.
    }
    grok_fresh_rollover(app, bin, soul, plug, handoff)
}

/// Fresh `-s` rollover, no fork attempt: the resume target is known gone
/// (404 / not-found / invalid), so forking it would 404 too. Mint a new
/// id, send the handoff on it (the handoff IS the context), save
/// plug.json marked minted. One shot — a failure stays an honest Err.
pub fn grok_fresh_rollover(
    app: &tauri::AppHandle,
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    let new_sid = mint_uuid()?;
    let args = grok_args(
        &new_sid,
        false,
        soul,
        handoff,
        crate::coordinator::companion_mode_tools(app),
        None,
    );
    let text = crate::mouth_blip::run_streamed(app, "Grok", bin, &args, 180, None)?;
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
    app: &tauri::AppHandle,
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    plug.goose_session_id = None;
    save_plug(plug);
    let args = goose_args(
        soul,
        handoff,
        None,
        crate::coordinator::companion_mode_tools(app),
    );
    let reply = run_mouth_cli("Goose", bin, &args, &[("GOOSE_MODE", "always_allow")], 300)?;
    if reply.is_empty() {
        return Err("Goose sent back an empty reply.".into());
    }
    capture_goose_session(bin);
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_args_attach_a_dashed_line_to_single() {
        let line = "---\n--force not a flag";
        let args = grok_fork_args("old", "new", "soul", line, true);
        assert_eq!(args[0], format!("--single={line}"));
        assert!(!args.iter().any(|a| a == "-p" || a == line));
    }
}
