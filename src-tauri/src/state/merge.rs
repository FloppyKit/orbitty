use super::{CardTombstone, PersistedState};
use super::io::{ update };
use super::models::{AgentRun};
use super::models_board::{KanbanCard};
use super::models_companion::{CompanionState};

/// Counts from one window merge. Logged as
/// `state-save-merge {window, kept_disk, took_incoming}` — counts only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowMerge {
    pub kept_disk: usize,
    pub took_incoming: usize,
}

/// Window `save_state` merge. Per-card by `lastTouchedAt`: a strictly
/// newer disk card beats an older incoming one; equal touch takes the
/// incoming card (the save in hand). An omitted card stays unless the
/// incoming tombstone is at least as new as the disk card. Rust-owned
/// slices are copied from `disk` and never from the window payload.
pub fn merge_window_save(disk: &PersistedState, incoming: &PersistedState) -> (PersistedState, WindowMerge) {
    let mut out = incoming.clone();
    out.agent_runs = disk.agent_runs.clone();
    out.companion.hack_live = disk.companion.hack_live;
    out.companion.hack_who = disk.companion.hack_who.clone();
    out.companion.hack_project_id = disk.companion.hack_project_id.clone();
    out.companion.free_mode = disk.companion.free_mode;
    out.companion.conductor_on = disk.companion.conductor_on;
    out.companion.conductor_model = disk.companion.conductor_model.clone();
    out.companion.reply_lane = disk.companion.reply_lane.clone();
    out.companion.reply_model = disk.companion.reply_model.clone();
    out.companion.handoff_backend = disk.companion.handoff_backend.clone();
    out.companion.auto_handoff = disk.companion.auto_handoff;
    out.companion.backend = disk.companion.backend.clone();
    out.companion.direct_base_url = disk.companion.direct_base_url.clone();
    out.companion.direct_model = disk.companion.direct_model.clone();
    out.shift_on = disk.shift_on;
    out.peer_chrome = disk.peer_chrome;
    // Stage prefs are machine settings written by set_rail_stage_prefs.
    // A window save that omits them deserializes to the defaults and must
    // not wipe a saved edge or auto-hide choice.
    out.rail_edge = disk.rail_edge.clone();
    out.rail_auto_hide_on_blur = disk.rail_auto_hide_on_blur;
    out.starter_story = disk.starter_story.clone();
    if out.last_claim_project_id.is_none() {
        out.last_claim_project_id = disk.last_claim_project_id.clone();
    }
    // Carried when the window names a terminal. An omitted key
    // deserializes to "" and must not wipe a saved override.
    if !matches!(out.agent_terminal.trim(), "ghostty" | "terminal") {
        out.agent_terminal = disk.agent_terminal.clone();
    }
    let (cards, tombs, kept, took) = merge_cards(
        &disk.kanban_cards,
        &incoming.kanban_cards,
        &disk.card_tombstones,
        &incoming.card_tombstones,
    );
    out.kanban_cards = cards;
    out.card_tombstones = tombs;
    // taskId / taskReason are native-only (task::stamp_card / task::end);
    // no window ever edits them. A window copy's values are always a stale
    // echo, so disk wins regardless of lastTouchedAt — a real later window
    // edit (newer stamp) must not carry an old taskId over the new one.
    // A card that is not on disk (new, or a newer window edit that
    // resurrects a Rust delete) has no native stamp to keep — drop the
    // window's stale taskId / taskReason.
    for c in out.kanban_cards.iter_mut() {
        match disk.kanban_cards.iter().find(|d| d.id == c.id) {
            Some(d) => {
                c.task_id = d.task_id.clone();
                c.task_reason = d.task_reason.clone();
            }
            None => {
                c.task_id = None;
                c.task_reason = None;
            }
        }
    }
    // A rail copy of a tab claim can omit `tab` (a writer that rebuilds
    // the row field by field). Disk's tab wins when the incoming row for
    // the same id has none. An incoming tab is kept — url/title updates
    // ride that row.
    for c in out.claims.iter_mut() {
        if c.tab.is_some() {
            continue;
        }
        if let Some(d) = disk.claims.iter().find(|d| d.window_id == c.window_id) {
            if d.tab.is_some() {
                c.tab = d.tab.clone();
            }
        }
    }
    (
        out,
        WindowMerge {
            kept_disk: kept,
            took_incoming: took,
        },
    )
}

/// Apply a Rust writer's before→after delta onto the freshly loaded
/// state. Slices the writer did not change stay as `live` has them, so a
/// stale whole-state snapshot cannot drop an `agentRuns` row or a hack
/// flag it never touched.
pub fn apply_writer_delta(live: &mut PersistedState, before: &PersistedState, after: &PersistedState) {
    apply_card_delta(
        &mut live.kanban_cards,
        &before.kanban_cards,
        &after.kanban_cards,
        &mut live.card_tombstones,
    );
    apply_run_delta(&mut live.agent_runs, &before.agent_runs, &after.agent_runs);
    apply_tombstone_delta(
        &mut live.card_tombstones,
        &before.card_tombstones,
        &after.card_tombstones,
    );
    take_changed(&mut live.theme_id, &before.theme_id, &after.theme_id);
    take_changed(&mut live.window_opacity, &before.window_opacity, &after.window_opacity);
    take_changed(&mut live.projects, &before.projects, &after.projects);
    take_changed(&mut live.claims, &before.claims, &after.claims);
    take_changed(
        &mut live.last_claim_project_id,
        &before.last_claim_project_id,
        &after.last_claim_project_id,
    );
    take_changed(&mut live.tags, &before.tags, &after.tags);
    take_changed(&mut live.window_tags, &before.window_tags, &after.window_tags);
    take_changed(
        &mut live.collapsed_section_ids,
        &before.collapsed_section_ids,
        &after.collapsed_section_ids,
    );
    take_changed(
        &mut live.nudge_suppressed_window_ids,
        &before.nudge_suppressed_window_ids,
        &after.nudge_suppressed_window_ids,
    );
    take_changed(&mut live.points_ledger, &before.points_ledger, &after.points_ledger);
    take_changed(&mut live.title_overrides, &before.title_overrides, &after.title_overrides);
    take_changed(
        &mut live.autohide_row_chrome,
        &before.autohide_row_chrome,
        &after.autohide_row_chrome,
    );
    take_changed(&mut live.hide_titles, &before.hide_titles, &after.hide_titles);
    take_changed(&mut live.hide_app_icons, &before.hide_app_icons, &after.hide_app_icons);
    take_changed(
        &mut live.action_shortcuts,
        &before.action_shortcuts,
        &after.action_shortcuts,
    );
    take_changed(
        &mut live.agent_shortcuts,
        &before.agent_shortcuts,
        &after.agent_shortcuts,
    );
    take_changed(&mut live.sprite_scale, &before.sprite_scale, &after.sprite_scale);
    take_changed(&mut live.marble_drift, &before.marble_drift, &after.marble_drift);
    take_changed(&mut live.peer_chrome, &before.peer_chrome, &after.peer_chrome);
    take_changed(&mut live.yard_windows, &before.yard_windows, &after.yard_windows);
    take_changed(&mut live.yard_tabs, &before.yard_tabs, &after.yard_tabs);
    apply_companion_delta(&mut live.companion, &before.companion, &after.companion);
    take_changed(&mut live.stars, &before.stars, &after.stars);
    take_changed(&mut live.sky, &before.sky, &after.sky);
    take_changed(&mut live.shared_pane_mode, &before.shared_pane_mode, &after.shared_pane_mode);
    take_changed(&mut live.drawers, &before.drawers, &after.drawers);
    take_changed(&mut live.spend_cap_usd, &before.spend_cap_usd, &after.spend_cap_usd);
    take_changed(
        &mut live.spend_cap_enabled,
        &before.spend_cap_enabled,
        &after.spend_cap_enabled,
    );
    take_changed(&mut live.runner_cap, &before.runner_cap, &after.runner_cap);
    take_changed(&mut live.spend_day, &before.spend_day, &after.spend_day);
    take_changed(&mut live.spend_usd, &before.spend_usd, &after.spend_usd);
    take_changed(&mut live.needed_split, &before.needed_split, &after.needed_split);
    take_changed(&mut live.preview_width, &before.preview_width, &after.preview_width);
    take_changed(&mut live.tracker_split, &before.tracker_split, &after.tracker_split);
    take_changed(
        &mut live.open_subtask_ids,
        &before.open_subtask_ids,
        &after.open_subtask_ids,
    );
    take_changed(&mut live.todo_trees, &before.todo_trees, &after.todo_trees);
    take_changed(&mut live.tree_grow_days, &before.tree_grow_days, &after.tree_grow_days);
    take_changed(&mut live.signed_locally, &before.signed_locally, &after.signed_locally);
    take_changed(&mut live.private_relay, &before.private_relay, &after.private_relay);
    take_changed(
        &mut live.private_relay_url,
        &before.private_relay_url,
        &after.private_relay_url,
    );
    take_changed(&mut live.shift_on, &before.shift_on, &after.shift_on);
    take_changed(&mut live.planet_idle, &before.planet_idle, &after.planet_idle);
    take_changed(
        &mut live.agent_terminal,
        &before.agent_terminal,
        &after.agent_terminal,
    );
    take_changed(
        &mut live.rail_board_cols,
        &before.rail_board_cols,
        &after.rail_board_cols,
    );
    take_changed(&mut live.rail_edge, &before.rail_edge, &after.rail_edge);
    take_changed(
        &mut live.rail_auto_hide_on_blur,
        &before.rail_auto_hide_on_blur,
        &after.rail_auto_hide_on_blur,
    );
    // Named so writer_delta_lists_cover_every_field stays green. A writer
    // that did not change the slice leaves `live` alone. Window saves do
    // not come through here; merge_window_save copies starter_story from disk.
    take_changed(
        &mut live.starter_story,
        &before.starter_story,
        &after.starter_story,
    );
}

pub(crate) fn wall_now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub(crate) fn json_eq<T: serde::Serialize + ?Sized>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

pub(crate) fn take_changed<T: Clone + serde::Serialize>(live: &mut T, before: &T, after: &T) {
    if !json_eq(before, after) {
        *live = after.clone();
    }
}

pub(crate) fn apply_companion_delta(live: &mut CompanionState, before: &CompanionState, after: &CompanionState) {
    take_changed(&mut live.visible, &before.visible, &after.visible);
    take_changed(&mut live.bounds, &before.bounds, &after.bounds);
    take_changed(&mut live.image_pack_id, &before.image_pack_id, &after.image_pack_id);
    take_changed(&mut live.state, &before.state, &after.state);
    take_changed(&mut live.last_user_line, &before.last_user_line, &after.last_user_line);
    take_changed(&mut live.last_bot_line, &before.last_bot_line, &after.last_bot_line);
    take_changed(&mut live.agent_profile, &before.agent_profile, &after.agent_profile);
    take_changed(&mut live.expanded, &before.expanded, &after.expanded);
    take_changed(&mut live.turns, &before.turns, &after.turns);
    take_changed(&mut live.speak_replies, &before.speak_replies, &after.speak_replies);
    take_changed(&mut live.speak_volume, &before.speak_volume, &after.speak_volume);
    take_changed(&mut live.captions, &before.captions, &after.captions);
    take_changed(&mut live.planets_hidden, &before.planets_hidden, &after.planets_hidden);
    take_changed(
        &mut live.cheat_sheet_hint,
        &before.cheat_sheet_hint,
        &after.cheat_sheet_hint,
    );
    take_changed(&mut live.talk_source, &before.talk_source, &after.talk_source);
    take_changed(&mut live.view_collapsed, &before.view_collapsed, &after.view_collapsed);
    take_changed(&mut live.view_expanded, &before.view_expanded, &after.view_expanded);
    take_changed(&mut live.sprite, &before.sprite, &after.sprite);
    take_changed(&mut live.mode, &before.mode, &after.mode);
    take_changed(&mut live.threads, &before.threads, &after.threads);
    take_changed(&mut live.hack_live, &before.hack_live, &after.hack_live);
    take_changed(&mut live.hack_who, &before.hack_who, &after.hack_who);
    take_changed(
        &mut live.hack_project_id,
        &before.hack_project_id,
        &after.hack_project_id,
    );
    take_changed(&mut live.free_mode, &before.free_mode, &after.free_mode);
    take_changed(&mut live.conductor_on, &before.conductor_on, &after.conductor_on);
    take_changed(
        &mut live.conductor_model,
        &before.conductor_model,
        &after.conductor_model,
    );
    take_changed(&mut live.reply_lane, &before.reply_lane, &after.reply_lane);
    take_changed(&mut live.reply_model, &before.reply_model, &after.reply_model);
    take_changed(
        &mut live.handoff_backend,
        &before.handoff_backend,
        &after.handoff_backend,
    );
    take_changed(&mut live.auto_handoff, &before.auto_handoff, &after.auto_handoff);
    take_changed(&mut live.backend, &before.backend, &after.backend);
    take_changed(
        &mut live.direct_base_url,
        &before.direct_base_url,
        &after.direct_base_url,
    );
    take_changed(&mut live.direct_model, &before.direct_model, &after.direct_model);
}

pub(crate) fn upsert_tombstone(tombs: &mut Vec<CardTombstone>, id: &str, at: f64) {
    if let Some(t) = tombs.iter_mut().find(|t| t.id == id) {
        if at > t.last_touched_at {
            t.last_touched_at = at;
        }
    } else {
        tombs.push(CardTombstone {
            id: id.to_string(),
            last_touched_at: at,
        });
    }
}

pub(crate) fn tomb_at(tombs: &[CardTombstone], id: &str) -> Option<f64> {
    tombs.iter().find(|t| t.id == id).map(|t| t.last_touched_at)
}

pub(crate) fn merge_cards(
    disk: &[KanbanCard],
    incoming: &[KanbanCard],
    disk_tombs: &[CardTombstone],
    incoming_tombs: &[CardTombstone],
) -> (Vec<KanbanCard>, Vec<CardTombstone>, usize, usize) {
    let mut tombs = Vec::new();
    for t in disk_tombs.iter().chain(incoming_tombs.iter()) {
        upsert_tombstone(&mut tombs, &t.id, t.last_touched_at);
    }
    let disk_by: std::collections::HashMap<&str, &KanbanCard> =
        disk.iter().map(|c| (c.id.as_str(), c)).collect();
    let inc_by: std::collections::HashMap<&str, &KanbanCard> =
        incoming.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut ids: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in disk.iter().chain(incoming.iter()) {
        if seen.insert(c.id.as_str()) {
            ids.push(c.id.as_str());
        }
    }
    let mut out = Vec::new();
    let mut kept = 0usize;
    let mut took = 0usize;
    for id in ids {
        match (disk_by.get(id), inc_by.get(id)) {
            (Some(d), Some(i)) => {
                if d.last_touched_at > i.last_touched_at {
                    out.push((*d).clone());
                    kept += 1;
                } else {
                    out.push((*i).clone());
                    took += 1;
                }
            }
            (Some(d), None) => {
                let newer_delete = tomb_at(incoming_tombs, id).is_some_and(|t| t >= d.last_touched_at);
                if newer_delete {
                    took += 1;
                } else {
                    out.push((*d).clone());
                    kept += 1;
                }
            }
            (None, Some(i)) => {
                let blocked = tomb_at(disk_tombs, id).is_some_and(|t| t >= i.last_touched_at);
                if blocked {
                    kept += 1;
                } else {
                    out.push((*i).clone());
                    took += 1;
                }
            }
            (None, None) => {}
        }
    }
    // Lane order is the window's array, not disk-first. A drag that only
    // reorders never changes lastTouchedAt, so disk-first ids froze the list.
    out = order_like_incoming(out, incoming);
    tombs.retain(|t| match out.iter().find(|c| c.id == t.id) {
        Some(c) => t.last_touched_at > c.last_touched_at,
        None => true,
    });
    (out, tombs, kept, took)
}

/// Incoming order for cards the window still has. Disk-only survivors
/// (another writer added them) stay, after that order.
pub(crate) fn order_like_incoming(merged: Vec<KanbanCard>, incoming: &[KanbanCard]) -> Vec<KanbanCard> {
    let mut rest = merged;
    let mut out = Vec::with_capacity(rest.len());
    for c in incoming {
        if let Some(i) = rest.iter().position(|x| x.id == c.id) {
            out.push(rest.remove(i));
        }
    }
    out.append(&mut rest);
    out
}

pub(crate) fn apply_card_delta(
    live: &mut Vec<KanbanCard>,
    before: &[KanbanCard],
    after: &[KanbanCard],
    tombs: &mut Vec<CardTombstone>,
) {
    if json_eq(before, after) {
        return;
    }
    let before_by: std::collections::HashMap<&str, &KanbanCard> =
        before.iter().map(|c| (c.id.as_str(), c)).collect();
    let after_by: std::collections::HashMap<&str, &KanbanCard> =
        after.iter().map(|c| (c.id.as_str(), c)).collect();
    for c in before {
        if after_by.contains_key(c.id.as_str()) {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.id == c.id) {
            if live[idx].last_touched_at <= c.last_touched_at {
                let at = c.last_touched_at.max(live[idx].last_touched_at);
                live.remove(idx);
                upsert_tombstone(tombs, &c.id, at);
            }
        }
    }
    for c in after {
        let changed = match before_by.get(c.id.as_str()) {
            None => true,
            Some(prev) => !json_eq(*prev, c),
        };
        if !changed {
            continue;
        }
        if tomb_at(tombs, &c.id).is_some_and(|t| t >= c.last_touched_at) && before_by.get(c.id.as_str()).is_none()
        {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.id == c.id) {
            let saved = live[idx].last_touched_at;
            // touch() clamps to now+60, which sits below a far-future
            // saved stamp and would drop the edit (taskId included).
            // A saved stamp past now+60s is bogus — take the writer.
            if c.last_touched_at >= saved || saved > wall_now_secs() + 60.0 {
                live[idx] = c.clone();
            }
        } else if !tomb_at(tombs, &c.id).is_some_and(|t| t >= c.last_touched_at) {
            live.push(c.clone());
        }
    }
}

pub(crate) fn apply_run_delta(live: &mut Vec<AgentRun>, before: &[AgentRun], after: &[AgentRun]) {
    if json_eq(before, after) {
        return;
    }
    let before_by: std::collections::HashMap<&str, &AgentRun> =
        before.iter().map(|r| (r.card_id.as_str(), r)).collect();
    let after_by: std::collections::HashMap<&str, &AgentRun> =
        after.iter().map(|r| (r.card_id.as_str(), r)).collect();
    for r in before {
        if after_by.contains_key(r.card_id.as_str()) {
            continue;
        }
        live.retain(|x| x.card_id != r.card_id);
    }
    for r in after {
        let changed = match before_by.get(r.card_id.as_str()) {
            None => true,
            Some(prev) => !json_eq(*prev, r),
        };
        if !changed {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.card_id == r.card_id) {
            live[idx] = r.clone();
        } else {
            live.push(r.clone());
        }
    }
}

pub(crate) fn apply_tombstone_delta(
    live: &mut Vec<CardTombstone>,
    before: &[CardTombstone],
    after: &[CardTombstone],
) {
    if json_eq(before, after) {
        return;
    }
    for t in after {
        let same = before.iter().find(|b| b.id == t.id).is_some_and(|b| json_eq(b, t));
        if !same {
            upsert_tombstone(live, &t.id, t.last_touched_at);
        }
    }
}

/// FIVE one-time migration: legacy checkins are 0–4, the
/// scale is now 1–5. Every saved spoke shifts n → n+1 exactly once — the
/// `scale_v` flag guards it, never zero-sniffing (a migrated save has no
/// zeros) — then the migrated shape is written back so no window ever
/// re-reads the old scale. Best-effort: a load/save failure just retries
/// next boot. An empty day stays empty: only EXISTING checkins shift, no
/// day is ever invented here.
pub fn migrate_stars_scale(app: &tauri::AppHandle) {
    let _ = update(app, |st| {
        if st.stars.scale_v >= 2 {
            return Ok(());
        }
        for c in st.stars.checkins.values_mut() {
            c.sleep = c.sleep.saturating_add(1).min(5);
            c.terroir = c.terroir.saturating_add(1).min(5);
            c.athleticism = c.athleticism.saturating_add(1).min(5);
            c.relationships = c.relationships.saturating_add(1).min(5);
            c.self_ = c.self_.saturating_add(1).min(5);
        }
        st.stars.scale_v = 2;
        Ok(())
    });
}
