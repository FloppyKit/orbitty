//! After a GO writer exits, one local Verify Bot dogfoods.
//! Pass closes the card and leaves one Spark line. A fail or a human
//! click leaves the card in Doing and opens one Needed.
//! The model comes from the saved default runner. Not the cloud bot.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::AppHandle;

static ARMED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Pass,
    Human(String),
    Fail(String),
}

pub(crate) fn parse_verdict(text: &str) -> Verdict {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let rest = |prefix: &str| {
        line.get(prefix.len()..)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("no sentence")
            .to_string()
    };
    if line.len() >= 4 && line[..4].eq_ignore_ascii_case("PASS") {
        Verdict::Pass
    } else if line.len() >= 6 && line[..6].eq_ignore_ascii_case("HUMAN:") {
        Verdict::Human(rest("HUMAN:"))
    } else if line.len() >= 5 && line[..5].eq_ignore_ascii_case("FAIL:") {
        Verdict::Fail(rest("FAIL:"))
    } else {
        Verdict::Fail("Verify Bot did not print PASS, FAIL, or HUMAN.".into())
    }
}

fn claim(card_id: &str) -> bool {
    let mut g = ARMED.lock().unwrap_or_else(|e| e.into_inner());
    let set = g.get_or_insert_with(HashSet::new);
    set.insert(card_id.to_string())
}

fn release(card_id: &str) {
    if let Ok(mut g) = ARMED.lock() {
        if let Some(set) = g.as_mut() {
            set.remove(card_id);
        }
    }
}

/// One launch per card. `pid_file` is the writer’s shell pid. None means
/// the writer has already exited.
pub fn arm(app: AppHandle, card_id: String, pid_file: Option<PathBuf>) {
    if card_id.trim().is_empty() || !claim(&card_id) {
        return;
    }
    let _ = thread::Builder::new()
        .name("orbitty-verify-close".into())
        .spawn(move || {
            let card = card_id.clone();
            if let Err(why) = wait_writer(pid_file.as_deref()) {
                finish(&app, &card, Verdict::Fail(why));
            } else {
                let verdict = run_bot(&app, &card);
                finish(&app, &card, verdict);
            }
            release(&card);
        });
}

fn wait_writer(pid: Option<&Path>) -> Result<(), String> {
    let Some(pid) = pid else {
        return Ok(());
    };
    let start = Instant::now();
    while !pid.exists() && start.elapsed() < Duration::from_secs(45) {
        thread::sleep(Duration::from_secs(1));
    }
    if !pid.exists() {
        return Err("writer never started".into());
    }
    while pid.exists() && start.elapsed() < Duration::from_secs(6 * 3600) {
        thread::sleep(Duration::from_secs(2));
    }
    if pid.exists() {
        return Err("writer still running after 6h".into());
    }
    Ok(())
}

fn model_arg() -> Result<Option<String>, String> {
    let spec = crate::plans::chosen_runner(&crate::plans::Roots::real())
        .ok_or("Choose a default runner in Settings. Verify uses that model.")?;
    let model = spec.model.trim();
    if model.is_empty() {
        Ok(None)
    } else {
        Ok(Some(model.to_string()))
    }
}

fn run_bot(app: &AppHandle, card_id: &str) -> Verdict {
    let model = match model_arg() {
        Ok(m) => m,
        Err(why) => return Verdict::Fail(why),
    };
    let Some(bin) = crate::coordinator::find_on_path("cursor-agent") else {
        return Verdict::Fail("cursor-agent is not on PATH".into());
    };
    let title = crate::state::load_state(app)
        .ok()
        .and_then(|st| {
            st.kanban_cards
                .iter()
                .find(|c| c.id == card_id)
                .map(|c| c.title.clone())
        })
        .unwrap_or_else(|| card_id.to_string());
    let used = model.as_deref().unwrap_or("cursor-agent default");
    let prompt = format!(
        "You are the local Verify Bot on this Mac. Not the cloud Public Witness bot.\n\
         Do not edit, commit, push, install, or move cards.\n\
         Model for this run is {used}. That comes from Settings and can change later.\n\
         Card: {title} ({card_id}). Repo: ~/orbitty.\n\
         Check orbittyctl status (Nightly must be up) and git log -1 --oneline.\n\
         First line only, then stop:\n\
         PASS\n\
         or FAIL: one sentence\n\
         or HUMAN: one sentence if only a person can click it.\n"
    );
    let mut cmd = Command::new(bin);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("text")
        .arg("--trust")
        .arg("--force")
        .arg("--workspace")
        .arg(crate::plans::Roots::real().home.join("orbitty"));
    if let Some(model) = &model {
        cmd.arg("--model").arg(model);
    }
    cmd.arg(prompt);
    match cmd.output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            parse_verdict(&text)
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            let line = err.lines().next().unwrap_or("cursor-agent failed");
            Verdict::Fail(line.to_string())
        }
        Err(e) => Verdict::Fail(format!("could not start Verify Bot: {e}")),
    }
}

fn finish(app: &AppHandle, card_id: &str, verdict: Verdict) {
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let Some(card) = st.kanban_cards.iter().find(|c| c.id == card_id) else {
        return;
    };
    let project = card.project_id.clone();
    let title = card.title.clone();
    match verdict {
        Verdict::Pass => {
            let _ = crate::board::kanban_move(app, card_id, "done");
            let _ = crate::coordinator::companion_note(
                app.clone(),
                format!("Verify passed. {title} is done."),
                Some("spark".into()),
            );
        }
        Verdict::Human(why) | Verdict::Fail(why) => {
            let _ = crate::board::kanban_note_append(app, card_id, &why);
            let _ = crate::board::needed_open(
                app,
                &format!("Verify close: {title}"),
                &project,
                Some(&why),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_verdict;
    use super::Verdict;

    #[test]
    fn verdict_lines() {
        assert_eq!(parse_verdict("PASS\nmore"), Verdict::Pass);
        assert_eq!(
            parse_verdict("  fail: socket down\n"),
            Verdict::Fail("socket down".into())
        );
        assert_eq!(
            parse_verdict("HUMAN: click the tray"),
            Verdict::Human("click the tray".into())
        );
        assert!(matches!(parse_verdict("hello"), Verdict::Fail(_)));
    }
}
