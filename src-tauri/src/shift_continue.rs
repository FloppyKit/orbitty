//! After an agent walker walks off, Spark
//! gets a one-line note and, in Work/Hold, pickup fires the next
//! assigned todo on that agent. Spark/Chat do not auto-continue.
//! No LLM in this file.

use crate::state::PersistedState;

fn one_line(s: &str) -> String {
    s.replace('\n', " ").trim().to_string()
}

fn clip_title(s: &str) -> String {
    let t = one_line(s);
    if t.chars().count() > 48 {
        format!("{}…", t.chars().take(47).collect::<String>())
    } else {
        t
    }
}

fn should_continue(st: &PersistedState) -> bool {
    matches!(st.companion.mode.to_ascii_lowercase().as_str(), "work" | "hold")
}

/// Fire-and-forget from the walk-off emit. Never fails the habitat tick.
pub fn on_agent_walked_off(app: &tauri::AppHandle, card_id: &str) {
    let card_id = card_id.trim();
    if card_id.is_empty() {
        return;
    }
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let card = st.kanban_cards.iter().find(|c| c.id == card_id);
    let title = card
        .map(|c| clip_title(&c.title))
        .unwrap_or_else(|| "run".into());
    let agent_id = card
        .and_then(|c| c.assigned_agent_id.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            st.agent_runs
                .iter()
                .find(|r| r.card_id == card_id)
                .map(|r| r.agent_shortcut_id.clone())
                .filter(|s| !s.is_empty())
        });

    if !should_continue(&st) {
        let _ = crate::coordinator::companion_note(
            app.clone(),
            format!("Walked off {title}."),
            Some("spark".into()),
        );
        return;
    }

    let Some(agent_id) = agent_id else {
        let _ = crate::coordinator::companion_note(
            app.clone(),
            format!("Walked off {title}. Nothing assigned to pick up."),
            Some("work".into()),
        );
        return;
    };

    match crate::agents::agent_pickup(app, Some(&agent_id), None, None) {
        Ok(v) if v.get("picked").and_then(|x| x.as_bool()) == Some(true) => {
            let next = v
                .get("title")
                .and_then(|t| t.as_str())
                .map(clip_title)
                .unwrap_or_else(|| "next".into());
            let _ = crate::coordinator::companion_note(
                app.clone(),
                format!("Walked off {title}. Picking up {next}."),
                Some("work".into()),
            );
        }
        Ok(_) => {
            let _ = crate::coordinator::companion_note(
                app.clone(),
                format!("Walked off {title}. Nothing else assigned."),
                Some("work".into()),
            );
        }
        Err(e) => {
            let _ = crate::coordinator::companion_note(
                app.clone(),
                format!("Walked off {title}. Next pickup paused ({e})."),
                Some("work".into()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PersistedState;

    #[test]
    fn work_and_hold_continue_spark_does_not() {
        let mut st = PersistedState::default();
        st.companion.mode = "work".into();
        assert!(should_continue(&st));
        st.companion.mode = "hold".into();
        assert!(should_continue(&st));
        st.companion.mode = "spark".into();
        assert!(!should_continue(&st));
        st.companion.mode = "chat".into();
        assert!(!should_continue(&st));
    }
}
