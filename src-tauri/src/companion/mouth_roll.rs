//! Session roll: new backend session, same bubble. Never wipe turns.
//! Timeouts are per-send, never a max-turns cap.

use std::path::Path;
use std::time::{Duration, Instant};

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

/// Why a backend session is being replaced. Dead reasons keep today's
/// handoff sentence. Planned reasons do not say the session died.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetReason {
    ContextDead,
    SessionGone,
    EmptyResume,
    Budget,
    Thin,
    ManualTest,
}

impl ResetReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ResetReason::ContextDead => "context_dead",
            ResetReason::SessionGone => "session_gone",
            ResetReason::EmptyResume => "empty_resume",
            ResetReason::Budget => "budget",
            ResetReason::Thin => "thin",
            ResetReason::ManualTest => "manual_test",
        }
    }

    pub fn is_dead(self) -> bool {
        matches!(
            self,
            ResetReason::ContextDead | ResetReason::SessionGone | ResetReason::EmptyResume
        )
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "context_dead" => Some(ResetReason::ContextDead),
            "session_gone" => Some(ResetReason::SessionGone),
            "empty_resume" => Some(ResetReason::EmptyResume),
            "budget" => Some(ResetReason::Budget),
            "thin" => Some(ResetReason::Thin),
            "manual_test" => Some(ResetReason::ManualTest),
            _ => None,
        }
    }
}

/// Handoff block for the first prompt on a rolled-over session: the last
/// ~6 turns of the one companion transcript, then the already
/// pocket/desk-wrapped user line (mode + now.json + pocket already in
/// that wrap). Persisted nowhere — the send path saves only the raw
/// line + reply, so the handoff never becomes a you-row.
/// No history yet (a first send that died) → the wrapped line unchanged.
/// What `coordinator/send.rs` calls after `arm_manual_roll`.
/// Flag-off sends never reach this (they stay on the every-16 path).
/// Only this function can mint an `ArmedRoll`. The flag-on handoff seam
/// takes that token, so a test that hardcodes `ResetReason` cannot load rules.
#[derive(Clone, Copy, Debug)]
pub struct ArmedRoll {
    reason: ResetReason,
}

impl ArmedRoll {
    pub fn reason(self) -> ResetReason {
        self.reason
    }
}

pub fn consume_armed_reason(
    pending: Option<&str>,
    chars: u64,
    budget: u64,
) -> Option<ArmedRoll> {
    let name = crate::mouth_archive::planned_reason(pending, chars, budget)?;
    Some(ArmedRoll {
        reason: ResetReason::parse(name).unwrap_or(ResetReason::Budget),
    })
}

pub fn handoff_prompt(
    app: &tauri::AppHandle,
    reason: ResetReason,
    wrapped_line: &str,
) -> String {
    if crate::companion::memory::enabled(app) && !crate::mouth_gist::is_gist(wrapped_line) {
        return crate::companion::memory::handoff_line(app, reason, wrapped_line);
    }
    let (mode, turns) = crate::state::load_state(app)
        .map(|s| (s.companion.mode.clone(), s.companion.turns.clone()))
        .unwrap_or_else(|_| ("spark".into(), Vec::new()));
    handoff_from_turns(reason, &mode, &turns, wrapped_line)
}

pub fn handoff_from_turns(
    reason: ResetReason,
    mode: &str,
    turns: &[crate::state::CompanionTurn],
    wrapped_line: &str,
) -> String {
    const HANDOFF_TURNS: usize = 6;
    const HISTORY_CAP: usize = 1200;
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
    let head = if reason.is_dead() {
        format!(
            "[handoff — the prior backend session died; this is the same desk thread (mode: {mode}) \
             on a fresh session. The transcript so far is below. Answer the USER line at the end; \
             do not greet, do not summarize.]"
        )
    } else {
        format!(
            "[handoff — fresh backend session ({why}); this is the same desk thread (mode: {mode}) \
             on a fresh session. The transcript so far is below. Answer the USER line at the end; \
             do not greet, do not summarize.]",
            why = reason.as_str()
        )
    };
    format!("{head}\n{}\n\n{wrapped_line}", lines.join("\n"))
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

/// One post-roll reply waits at most this long. A fork timeout must
/// not start a second full wait (that was 2 x 180 s).
pub const POST_ROLL_SECS: u64 = 180;
const POST_ROLL_SLACK_SECS: u64 = 5;
pub const POST_ROLL_TIMEOUT: &str = "Grok took longer than 180s after a roll";

#[derive(Debug)]
enum RollStep {
    ForkOk(String),
    /// Fast miss (404 / empty). Fresh may run, but only for the seconds
    /// still inside the one budget.
    NeedFresh(u64),
    Stop,
}

fn is_stream_timeout(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("took longer") || e.contains("after a roll")
}

/// What the fork result is allowed to do. A timeout, or a fork that
/// already used the budget, stops. It does not start another 180 s wait.
fn step_after_fork(fork: Result<String, String>, elapsed: Duration, budget_secs: u64) -> RollStep {
    let budget = Duration::from_secs(budget_secs);
    let bound = budget + Duration::from_secs(POST_ROLL_SLACK_SECS);
    match fork {
        Ok(text) if !text.is_empty() => RollStep::ForkOk(text),
        Err(e) if is_stream_timeout(&e) => RollStep::Stop,
        _ if elapsed >= budget || elapsed >= bound => RollStep::Stop,
        _ => {
            let left = budget_secs.saturating_sub(elapsed.as_secs()).max(1);
            if elapsed + Duration::from_secs(left) > bound {
                RollStep::Stop
            } else {
                RollStep::NeedFresh(left)
            }
        }
    }
}

fn log_post_roll_timeout(app: &tauri::AppHandle, elapsed_ms: u128, handoff_chars: usize) {
    crate::event_log::append_best(
        app,
        "companion-timeout",
        None,
        None,
        Some("agent"),
        true,
        &serde_json::json!({
            "phase": "post-roll",
            "elapsed_ms": elapsed_ms,
            "handoff_chars": handoff_chars,
        }),
    );
}

/// Roll a dead grok mouth onto a fresh session id. Fork when the old id
/// still resumes (history carries; the handoff re-aims the reply), else a
/// fresh `-s` where the handoff IS the context. plug.json always lands the
/// new id, marked minted. One shot — a failing rollover stays an honest Err.
/// A fork that hits the timeout does not fall through to a second 180 s wait.
pub fn grok_rollover(
    app: &tauri::AppHandle,
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
) -> Result<String, String> {
    let started = Instant::now();
    let old_sid = plug.grok_session_id.clone().unwrap_or_default();
    let new_sid = mint_uuid()?;
    let step = if is_uuid(&old_sid) {
        let args = grok_fork_args(
            &old_sid,
            &new_sid,
            soul,
            handoff,
            crate::coordinator::companion_mode_tools(app),
        );
        // The fork RESUMES the old id — under the watch a corpse 404s in
        // ~0.5s and may still fresh-roll inside the remaining budget.
        // A slow fork (the 180s kill) stops here. No second wait.
        let fork = crate::mouth_blip::run_streamed(
            app,
            "Grok",
            bin,
            &args,
            POST_ROLL_SECS,
            Some(&grok_session_gone_line),
        );
        step_after_fork(fork, started.elapsed(), POST_ROLL_SECS)
    } else {
        RollStep::NeedFresh(POST_ROLL_SECS)
    };
    match step {
        RollStep::ForkOk(text) => {
            plug.grok_session_id = Some(new_sid);
            plug.grok_session_minted = true;
            save_plug(plug);
            Ok(text)
        }
        RollStep::Stop => {
            log_post_roll_timeout(app, started.elapsed().as_millis(), handoff.chars().count());
            Err(POST_ROLL_TIMEOUT.into())
        }
        RollStep::NeedFresh(secs) => grok_fresh_within(app, bin, soul, plug, handoff, secs),
    }
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
    grok_fresh_within(app, bin, soul, plug, handoff, POST_ROLL_SECS)
}

fn grok_fresh_within(
    app: &tauri::AppHandle,
    bin: &Path,
    soul: &str,
    plug: &mut PlugState,
    handoff: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let started = Instant::now();
    let new_sid = mint_uuid()?;
    let args = grok_args(
        &new_sid,
        false,
        soul,
        handoff,
        crate::coordinator::companion_mode_tools(app),
        None,
    );
    let text = match crate::mouth_blip::run_streamed(app, "Grok", bin, &args, timeout_secs, None) {
        Err(e) if is_stream_timeout(&e) => {
            log_post_roll_timeout(app, started.elapsed().as_millis(), handoff.chars().count());
            return Err(POST_ROLL_TIMEOUT.into());
        }
        other => other?,
    };
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

    /// Stub fork that already slept past the timeout. Scaled clock: the
    /// 180s kill is the whole budget, so a second roll would be 360s.
    #[test]
    fn post_roll_single_wait_bound() {
        let slept = Duration::from_secs(POST_ROLL_SECS);
        let step = step_after_fork(
            Err(format!(
                "Grok took longer than {POST_ROLL_SECS}s — killed. Try again."
            )),
            slept,
            POST_ROLL_SECS,
        );
        assert!(matches!(step, RollStep::Stop), "fork timeout must not fresh-roll");
        let immediate = step_after_fork(
            Err(format!(
                "Grok took longer than {POST_ROLL_SECS}s — killed. Try again."
            )),
            Duration::from_millis(10),
            POST_ROLL_SECS,
        );
        assert!(
            matches!(immediate, RollStep::Stop),
            "a timeout error stops even before the clock hits 180s"
        );
        let scaled_ms = slept.as_millis();
        assert!(
            scaled_ms <= (POST_ROLL_SECS as u128 + POST_ROLL_SLACK_SECS as u128) * 1000,
            "scaled elapsed {scaled_ms} ms exceeds 185s"
        );
        // A fast 404 still may fresh, but only inside the leftover budget.
        let fast = step_after_fork(
            Err("Grok: session get failed: 404 Not Found".into()),
            Duration::from_millis(500),
            POST_ROLL_SECS,
        );
        match fast {
            RollStep::NeedFresh(secs) => {
                assert!(secs <= POST_ROLL_SECS);
                assert!(secs < POST_ROLL_SECS || POST_ROLL_SECS == 180);
            }
            other => panic!("fast miss should fresh inside budget, got {other:?}"),
        }
        let body = include_str!("mouth_roll.rs");
        let start = body.find("pub fn grok_rollover(").expect("rollover");
        let rest = &body[start..];
        let end = rest.find("\npub fn grok_fresh_rollover(").expect("fresh");
        let rollover = &rest[..end];
        assert!(rollover.contains("step_after_fork"), "{rollover}");
        assert!(rollover.contains("RollStep::Stop"));
        assert_eq!(
            rollover.matches("grok_fresh_within").count(),
            1,
            "fork timeout must not add a second full wait"
        );
        assert!(!rollover.contains("grok_fresh_rollover("));
    }
}
