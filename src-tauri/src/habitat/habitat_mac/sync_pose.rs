//! Pose-only sync helpers (DIET-HABITAT).
//!
//! Field comparison for the board-drag fast path and off-lock pose-event
//! flushing, peeled from `sync.rs`.

use super::pose::{pose_name, Pose};
use super::{app_handle, HabitatEntry};

/// CADENCE-FIX: one event per pose transition, appended off the AppKit
/// thread and off the habitat lock. Best-effort: a missed pose line beats
/// a stalled UI.
pub(crate) fn flush_pose_events(events: Vec<(String, String, Pose, Pose)>) {
    if events.is_empty() {
        return;
    }
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        for (project_id, entry_id, from, to) in events {
            crate::event_log::append_best(
                &app,
                "walker-pose",
                None,
                Some(&project_id),
                Some("habitat"),
                true,
                &serde_json::json!({
                    "entryId": entry_id,
                    "from": pose_name(from),
                    "to": pose_name(to),
                }),
            );
        }
    });
}

/// Being-defining fields: everything except the board card fields
/// (card_status / card_kind / needed_state — those only move the pose)
/// and `minimized`, which CADENCE-FIX made pose-irrelevant (a claimed
/// window is a walker either way). The pose-only fast path compares these
/// to skip a full Being rebuild on board drags.
pub(crate) fn being_fields_eq(a: &HabitatEntry, b: &HabitatEntry) -> bool {
    a.id == b.id
        && a.app_name == b.app_name
        && a.title == b.title
        && a.bundle_id == b.bundle_id
        && a.icon_base64 == b.icon_base64
        && a.age == b.age
        && a.sprite_pack_id == b.sprite_pack_id
        && a.char_seed == b.char_seed
        && a.char_look == b.char_look
        && a.sprite_scale == b.sprite_scale
        && a.asleep == b.asleep
        && a.agent_seed == b.agent_seed
}
