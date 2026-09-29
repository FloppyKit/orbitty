//! Them-row board list in code. Names the planet and bucket.
//! The Hang route calls this. No model.

use crate::mouth_tools::{collect_open, OpenBuckets};

/// Speak the open board for `raw_line`. Always returns a them-row.
pub fn speak(app: &tauri::AppHandle, raw_line: &str) -> String {
    let project = crate::mouth_tools::project_for_line(app, raw_line);
    match collect_open(app, &project) {
        Ok(b) => speak_from_buckets(&focus_bucket(b, raw_line)),
        Err(e) => e,
    }
}

/// A question that names one bucket speaks that bucket only.
fn focus_bucket(mut b: OpenBuckets, line: &str) -> OpenBuckets {
    let l = line.to_ascii_lowercase();
    let doing = l
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "doing");
    let todo = l
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "todo" || w == "todos");
    if doing && !todo {
        b.todo.clear();
        b.todo_n = 0;
        b.needed.clear();
        b.needed_n = 0;
    } else if todo && !doing {
        b.doing.clear();
        b.doing_n = 0;
        b.needed.clear();
        b.needed_n = 0;
    }
    b
}

/// Pure them-row. Names the planet and bucket. Never relabels them.
pub fn speak_from_buckets(b: &OpenBuckets) -> String {
    crate::mouth_tools::honest_snapshot(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mouth_tools::OpenBuckets;

    fn buckets(display: &str, todo: &[&str], doing: &[&str], needed: &[&str]) -> OpenBuckets {
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
        assert!(
            out.contains("Orbitty — Todo (2): GO-ORBITTY-SLEEP-PRESETS; GO-ORBITTY-DOCK-CENTER")
        );
        assert!(out.contains("Orbitty — Doing (1): Later: 1B harness pick skip|job|pocket|tight"));
        assert!(!out.contains("Needed"));
        assert!(!out.contains("Orbitty has"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn skips_empty_needed() {
        let out = speak_from_buckets(&buckets("Orbitty", &["GO-ORBITTY-SLEEP-PRESETS"], &[], &[]));
        assert_eq!(out, "Orbitty — Todo (1): GO-ORBITTY-SLEEP-PRESETS");
        assert!(!out.contains("Doing"));
        assert!(!out.contains("Needed"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn empty_board_one_sentence() {
        let out = speak_from_buckets(&buckets("Orbitty", &[], &[], &[]));
        assert_eq!(out, "Orbitty — no open cards.");
        assert!(!out.contains("Todo ("));
        assert!(!out.contains("Orbitty has"));
        assert!(!out.contains("Open tab"));
    }

    #[test]
    fn includes_needed_when_present() {
        let out = speak_from_buckets(&buckets("Orbitty", &["A"], &["B"], &["Who runs this?"]));
        assert!(out.contains("Orbitty — Doing (1): B"));
        assert!(out.contains("Orbitty — Needed (1): Who runs this?"));
        assert!(!out.contains("Orbitty has"));
    }
}
