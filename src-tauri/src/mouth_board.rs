//! Them-row board list in code. One job: speak open cards as a
//! kind lead + bullets. Skips LM Studio / Grok for board reads.

use serde_json::json;

use crate::mouth_tools::{collect_open, OpenBuckets};

/// Speak the open board for `raw_line`. Always returns a them-row.
pub fn speak(app: &tauri::AppHandle, raw_line: &str) -> String {
    let project = crate::mouth_tools::project_from_line(raw_line);
    match collect_open(app, &project) {
        Ok(b) => speak_from_buckets(&b),
        Err(e) => e,
    }
}

/// Pure them-row. Unit-tested with canned titles (no AppHandle).
fn nice_planet(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return "Orbitty".into();
    }
    let mut chars = t.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
        None => "Orbitty".into(),
    }
}

pub fn speak_from_buckets(b: &OpenBuckets) -> String {
    let planet = nice_planet(&b.display);
    let total = b.todo_n + b.doing_n + b.needed_n;
    if total == 0 {
        return format!("Okay, {planet} has no open cards right now.");
    }

    let todo_phrase = if b.todo_n == 1 {
        "1 todo".to_string()
    } else {
        format!("{} todos", b.todo_n)
    };
    let mut lead = format!(
        "Okay, there are {} on {} right now",
        todo_phrase, planet
    );
    if b.doing_n > 0 {
        lead.push_str(&format!(", and {} in doing", b.doing_n));
    }
    if b.needed_n > 0 {
        lead.push_str(&format!(", and {} needed", b.needed_n));
    }
    lead.push('.');

    let mut out = lead;
    push_bucket(&mut out, "TODO", &b.todo);
    push_bucket(&mut out, "DOING", &b.doing);
    push_bucket(&mut out, "NEEDED", &b.needed);
    out
}

fn push_bucket(out: &mut String, name: &str, titles: &[String]) {
    if titles.is_empty() {
        return;
    }
    out.push('\n');
    out.push('\n');
    out.push_str(name);
    for t in titles {
        out.push('\n');
        out.push_str("- ");
        out.push_str(t);
    }
}

/// Log hop lane so dogfood can see code skipped LM.
pub fn log_code_hop(app: &tauri::AppHandle) {
    crate::event_log::append_best(
        app,
        "companion-hop",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "lane": "code", "boardList": true }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mouth_tools::OpenBuckets;

    fn buckets(
        display: &str,
        todo: &[&str],
        doing: &[&str],
        needed: &[&str],
    ) -> OpenBuckets {
        OpenBuckets {
            pid: "p-test".into(),
            display: display.into(),
            todo: todo.iter().map(|s| (*s).into()).collect(),
            todo_n: todo.len(),
            doing: doing.iter().map(|s| (*s).into()).collect(),
            doing_n: doing.len(),
            needed: needed.iter().map(|s| (*s).into()).collect(),
            needed_n: needed.len(),
        }
    }

    #[test]
    fn kind_lead_and_bullets() {
        let out = speak_from_buckets(&buckets(
            "Orbitty",
            &["GO-ORBITTY-SLEEP-PRESETS", "GO-ORBITTY-DOCK-CENTER"],
            &["Later: 1B harness pick skip|job|pocket|tight"],
            &[],
        ));
        assert!(out.starts_with(
            "Okay, there are 2 todos on Orbitty right now, and 1 in doing."
        ));
        assert!(out.contains("\n\nTODO\n- GO-ORBITTY-SLEEP-PRESETS\n- GO-ORBITTY-DOCK-CENTER"));
        assert!(out.contains(
            "\n\nDOING\n- Later: 1B harness pick skip|job|pocket|tight"
        ));
        assert!(!out.contains("NEEDED"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn skips_empty_needed() {
        let out = speak_from_buckets(&buckets(
            "Orbitty",
            &["GO-ORBITTY-SLEEP-PRESETS"],
            &[],
            &[],
        ));
        assert!(out.starts_with("Okay, there are 1 todo on Orbitty right now."));
        assert!(!out.contains("DOING"));
        assert!(!out.contains("NEEDED"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn empty_board_one_sentence() {
        let out = speak_from_buckets(&buckets("Orbitty", &[], &[], &[]));
        assert_eq!(out, "Okay, Orbitty has no open cards right now.");
        assert!(!out.contains("TODO"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn includes_needed_when_present() {
        let out = speak_from_buckets(&buckets(
            "Orbitty",
            &["A"],
            &["B"],
            &["Who runs this?"],
        ));
        assert!(out.contains(", and 1 in doing, and 1 needed."));
        assert!(out.contains("\n\nNEEDED\n- Who runs this?"));
    }
}
