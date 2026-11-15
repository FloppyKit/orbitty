//! Pinned walker pose table. Empty `outcome` is the base table.
//! A non-empty door word moves only a droid, and never by itself to WalkOff.

use super::{droid_glance, pose_for, Pose};
use crate::habitat::HabitatEntry;

const FLAG_OFF: &[(bool, &str, &str, &str, Pose, u8)] = &[
    (true, "", "todo", "", Pose::Work, 0),
    (true, "", "todo", "open", Pose::Sit, 2),
    (true, "", "todo", "later", Pose::Sit, 2),
    (true, "", "todo", "blocked", Pose::Sit, 1),
    (true, "", "window", "", Pose::Work, 0),
    (true, "", "window", "open", Pose::Sit, 2),
    (true, "", "window", "later", Pose::Sit, 2),
    (true, "", "window", "blocked", Pose::Sit, 1),
    (true, "todo", "todo", "", Pose::Work, 0),
    (true, "todo", "todo", "open", Pose::Sit, 2),
    (true, "todo", "todo", "later", Pose::Sit, 2),
    (true, "todo", "todo", "blocked", Pose::Sit, 1),
    (true, "todo", "window", "", Pose::Work, 0),
    (true, "todo", "window", "open", Pose::Sit, 2),
    (true, "todo", "window", "later", Pose::Sit, 2),
    (true, "todo", "window", "blocked", Pose::Sit, 1),
    (true, "doing", "todo", "", Pose::Work, 0),
    (true, "doing", "todo", "open", Pose::Sit, 2),
    (true, "doing", "todo", "later", Pose::Sit, 2),
    (true, "doing", "todo", "blocked", Pose::Sit, 1),
    (true, "doing", "window", "", Pose::Work, 0),
    (true, "doing", "window", "open", Pose::Sit, 2),
    (true, "doing", "window", "later", Pose::Sit, 2),
    (true, "doing", "window", "blocked", Pose::Sit, 1),
    (true, "done", "todo", "", Pose::WalkOff, 0),
    (true, "done", "todo", "open", Pose::Sit, 2),
    (true, "done", "todo", "later", Pose::Sit, 2),
    (true, "done", "todo", "blocked", Pose::Sit, 1),
    (true, "done", "window", "", Pose::WalkOff, 0),
    (true, "done", "window", "open", Pose::Sit, 2),
    (true, "done", "window", "later", Pose::Sit, 2),
    (true, "done", "window", "blocked", Pose::Sit, 1),
    (false, "", "todo", "", Pose::Roam, 0),
    (false, "", "todo", "open", Pose::Sit, 0),
    (false, "", "todo", "later", Pose::Sit, 0),
    (false, "", "todo", "blocked", Pose::Roam, 0),
    (false, "", "window", "", Pose::Roam, 0),
    (false, "", "window", "open", Pose::Sit, 0),
    (false, "", "window", "later", Pose::Sit, 0),
    (false, "", "window", "blocked", Pose::Roam, 0),
    (false, "todo", "todo", "", Pose::Stand, 0),
    (false, "todo", "todo", "open", Pose::Sit, 0),
    (false, "todo", "todo", "later", Pose::Sit, 0),
    (false, "todo", "todo", "blocked", Pose::Stand, 0),
    (false, "todo", "window", "", Pose::Roam, 0),
    (false, "todo", "window", "open", Pose::Sit, 0),
    (false, "todo", "window", "later", Pose::Sit, 0),
    (false, "todo", "window", "blocked", Pose::Roam, 0),
    (false, "doing", "todo", "", Pose::Work, 0),
    (false, "doing", "todo", "open", Pose::Sit, 0),
    (false, "doing", "todo", "later", Pose::Sit, 0),
    (false, "doing", "todo", "blocked", Pose::Work, 0),
    (false, "doing", "window", "", Pose::Roam, 0),
    (false, "doing", "window", "open", Pose::Sit, 0),
    (false, "doing", "window", "later", Pose::Sit, 0),
    (false, "doing", "window", "blocked", Pose::Roam, 0),
    (false, "done", "todo", "", Pose::WalkOff, 0),
    (false, "done", "todo", "open", Pose::Sit, 0),
    (false, "done", "todo", "later", Pose::Sit, 0),
    (false, "done", "todo", "blocked", Pose::WalkOff, 0),
    (false, "done", "window", "", Pose::WalkOff, 0),
    (false, "done", "window", "open", Pose::Sit, 0),
    (false, "done", "window", "later", Pose::Sit, 0),
    (false, "done", "window", "blocked", Pose::WalkOff, 0),
];

fn entry(
    droid: bool,
    card_status: &str,
    card_kind: &str,
    needed_state: &str,
    outcome: &str,
) -> HabitatEntry {
    HabitatEntry {
        id: "id".into(),
        app_name: "app".into(),
        title: "title".into(),
        bundle_id: None,
        minimized: false,
        icon_base64: None,
        age: "fresh".into(),
        sprite_pack_id: String::new(),
        char_seed: 0,
        char_look: None,
        sprite_scale: 0.0,
        asleep: false,
        card_status: card_status.into(),
        card_kind: card_kind.into(),
        needed_state: needed_state.into(),
        agent_seed: if droid { "planet:card".into() } else { String::new() },
        outcome: outcome.into(),
    }
}

fn base_pinned(droid: bool, status: &str, kind: &str, needed: &str) -> (Pose, u8) {
    for row in FLAG_OFF {
        if row.0 == droid && row.1 == status && row.2 == kind && row.3 == needed {
            return (row.4, row.5);
        }
    }
    panic!("unpinned row {droid} {status} {kind} {needed}");
}

#[test]
fn pose_flag_off_table_unchanged() {
    for &(droid, status, kind, needed, pose, glance) in FLAG_OFF {
        let e = entry(droid, status, kind, needed, "");
        assert_eq!(pose_for(&e), pose, "pose {droid} {status} {kind} {needed}");
        assert_eq!(droid_glance(&e), glance, "glance {droid} {status} {kind} {needed}");
    }
}

fn expect_door(status: &str, kind: &str, needed: &str, outcome: &str) -> (Pose, u8) {
    let (today_pose, today_glance) = base_pinned(true, status, kind, needed);
    if status == "done" {
        return (Pose::WalkOff, today_glance);
    }
    match outcome {
        "waiting_on_you" => (Pose::Sit, 2),
        "failed" => (Pose::Sit, 1),
        "working" => (Pose::Work, 0),
        "idle" | "done" => (today_pose, 0),
        _ => (today_pose, today_glance),
    }
}

#[test]
fn droid_outcome_pose_table() {
    let words = ["working", "waiting_on_you", "idle", "failed", "done", "laterish"];
    let statuses = ["", "todo", "doing", "done"];
    let kinds = ["todo", "window"];
    let neededs = ["", "open", "later", "blocked"];
    for outcome in words {
        for status in statuses {
            for kind in kinds {
                for needed in neededs {
                    let e = entry(true, status, kind, needed, outcome);
                    let (pose, glance) = expect_door(status, kind, needed, outcome);
                    assert_eq!(pose_for(&e), pose, "pose {outcome} {status} {kind} {needed}");
                    assert_eq!(
                        droid_glance(&e),
                        glance,
                        "glance {outcome} {status} {kind} {needed}"
                    );
                }
            }
        }
    }
}

#[test]
fn door_done_never_walks_off() {
    for outcome in ["done", "idle"] {
        let e = entry(true, "doing", "todo", "", outcome);
        assert_ne!(pose_for(&e), Pose::WalkOff, "{outcome}");
    }
    let off = entry(true, "done", "todo", "", "working");
    assert_eq!(pose_for(&off), Pose::WalkOff);
}

#[test]
fn window_walker_ignores_outcome() {
    for outcome in ["working", "waiting_on_you", "idle", "failed", "done", "nope"] {
        for &(droid, status, kind, needed, pose, glance) in FLAG_OFF {
            if droid {
                continue;
            }
            let e = entry(false, status, kind, needed, outcome);
            assert_eq!(pose_for(&e), pose, "pose {outcome} {status} {kind} {needed}");
            assert_eq!(droid_glance(&e), glance, "glance {outcome} {status} {kind} {needed}");
        }
    }
}

#[test]
fn habitat_entry_outcome_defaults_empty() {
    let raw = r#"{
        "id":"agent:c1",
        "appName":"agent",
        "title":"Seat",
        "bundleId":null,
        "minimized":false,
        "iconBase64":null,
        "age":"fresh",
        "spritePackId":"",
        "charSeed":0,
        "charLook":null,
        "spriteScale":0,
        "asleep":false,
        "cardStatus":"doing",
        "cardKind":"todo",
        "neededState":"blocked",
        "agentSeed":"p1:c1"
    }"#;
    let entry: HabitatEntry = serde_json::from_str(raw).unwrap();
    assert_eq!(entry.outcome, "");
}
