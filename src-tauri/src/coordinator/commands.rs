#![allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[allow(unused_imports)]
use super::hermes::*;
#[allow(unused_imports)]
use super::allowlist::*;
#[allow(unused_imports)]
use super::plug::*;
#[allow(unused_imports)]
use super::mouth_cli::*;
#[allow(unused_imports)]
use super::intercept::*;
#[allow(unused_imports)]
use super::send::*;
#[allow(unused_imports)]
use super::soul_wake::*;
#[allow(unused_imports)]
use super::action::*;
#[allow(unused_imports)]
use super::paths::*;
#[allow(unused_imports)]
use super::cursor_perm::*;
#[allow(unused_imports)]
use super::launch_agent::*;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use super::mac;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionAgents {
    /// Berd agent stems (or Hermes profiles when backend=hermes).
    pub agents: Vec<String>,
    /// Effective pick after the unset/stale fallback rules.
    pub selected: String,
}

/// Agent picker roster for the companion window. goose/grok/berd share
/// the four-cast stems; hermes keeps its profile dirs; an unknown backend
/// is an honest error (the page degrades to a single "default" row).
#[tauri::command]
pub fn companion_agents(app: tauri::AppHandle) -> Result<CompanionAgents, String> {
    let (backend, source) = crate::companion_backend::require_send_backend(&app)?;
    let (agents, selected) = match backend.as_str() {
        "hermes" => {
            let mut a = vec!["default".to_string()];
            a.extend(hermes_profile_dirs());
            let persisted = crate::state::load_state(&app)
                .map(|s| s.companion.agent_profile)
                .unwrap_or_default();
            let selected = resolve_agent(&persisted, &a);
            (a, selected)
        }
        "goose" | "grok" | "berd" | "direct" => {
            let (agents, prefer) = berd_companion_stems();
            let persisted = crate::state::load_state(&app)
                .map(|s| s.companion.agent_profile)
                .unwrap_or_default();
            let selected = resolve_agent_prefer(&persisted, &agents, &prefer);
            (agents, selected)
        }
        other => {
            return Err(crate::companion_backend::unknown_backend_message(
                other, source,
            ))
        }
    };
    Ok(CompanionAgents { agents, selected })
}

/// Persist the companion body pack. Empty / "spirit" → blessed spirit.
/// Only `role: companion` packs (or a still-valid existing id) are allowed.
#[tauri::command]
pub fn set_companion_pack(app: tauri::AppHandle, pack_id: String) -> Result<Option<String>, String> {
    let raw = pack_id.trim();
    let id = if raw.is_empty() || raw.eq_ignore_ascii_case("spirit") {
        None
    } else {
        Some(raw.to_string())
    };
    if let Some(ref pid) = id {
        // female is the reserved 3D body
        // bundled at companion/3d — never pack folders, so the pack-art
        // check is skipped for them.
        let reserved_3d = pid == "female";
        if !reserved_3d && crate::packs::companion_pack_art(&app, pid).is_none() {
            return Err(format!("companion pack {pid:?} missing"));
        }
    }
    crate::state::update(&app, |st| {
        st.companion.image_pack_id = id.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(id)
}

/// Persist the companion's agent pick (soul voice label).
/// goose/grok/berd: the four-cast stems; the soul pointer updates like
/// Berd (goose/grok read that md as --system/--rules on the next send).
/// Hermes path: next send continues that profile's orbitty-companion thread.
#[tauri::command]
pub fn set_companion_agent(app: tauri::AppHandle, profile: String) -> Result<String, String> {
    let (backend, source) = crate::companion_backend::require_send_backend(&app)?;
    let p = profile.trim().to_string();
    match backend.as_str() {
        "hermes" => {
            let dirs = hermes_profile_dirs();
            if p != "default" && !dirs.iter().any(|d| d == &p) {
                return Err(format!("No Hermes profile named {p:?}."));
            }
        }
        "goose" | "grok" | "berd" | "direct" => {
            let stems = berd_agent_stems();
            if !stems.iter().any(|d| d == &p) {
                return Err(format!("No agent named {p:?} under ~/.agents/agents."));
            }
            if let Some(al) = load_companion_allowlist() {
                if !al.personas.iter().any(|d| d == &p) {
                    return Err(format!("{p} is not on the Orbitty four-cast."));
                }
            }
            update_soul_pointer(&p);
        }
        other => {
            return Err(crate::companion_backend::unknown_backend_message(
                other, source,
            ))
        }
    }
    crate::state::update(&app, |st| {
        st.companion.agent_profile = p.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(p)
}

/// Persist the active companion mode (spark |
/// chat | work | hold). Job law only — same transcript. Unknown mode =
/// honest error, fail closed — never a silent spark.
#[tauri::command]
pub fn set_companion_mode(app: tauri::AppHandle, mode: String) -> Result<String, String> {
    let m = crate::state::normalize_companion_mode(&mode).ok_or_else(|| {
        format!(
            "unknown companion mode {:?} — spark | chat | work | hold",
            mode.trim()
        )
    })?;
    crate::state::update(&app, |st| {
        st.companion.switch_mode(&m);
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(m)
}

/// Drop a them-line onto the live transcript — NO
/// LLM, no send path. `mode` is ignored (one companion thread). Returns
/// the active mode.
pub fn companion_note(
    app: tauri::AppHandle,
    line: String,
    mode: Option<String>,
) -> Result<String, String> {
    let line = line.trim().replace('\n', " ");
    if line.is_empty() {
        return Err("companion note needs a line.".into());
    }
    let target = crate::state::update(&app, |st| {
        // load_state healed the mode, so st.companion.mode is allowlist-clean.
        let target = match mode.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => crate::state::normalize_companion_mode(raw).ok_or_else(|| {
                format!("unknown companion mode {raw:?} — spark | chat | work | hold")
            })?,
            None => st.companion.mode.clone(),
        };
        // One transcript: notes always land on the live thread.
        let _ = target;
        st.companion.last_bot_line = line.clone();
        st.companion.push_turn("them", &line);
        Ok(target)
    })?;
    let _ = app.emit("state:changed", ());
    Ok(target)
}

/// Human "this chat is thin / new session needed". Dev-only.
/// One nod. Does not copy the last user line into now.json, and does
/// not wipe `turns`. The ⌘⌥T shortcut enters through this same function.
#[tauri::command]
pub fn companion_thin(app: tauri::AppHandle) -> Result<String, String> {
    // Gate = cfg(any(debug_assertions, feature = "dev-thin")), via the shared const.
    // One nod for the whole attempt (inside thin_flow). The stamp script does not play one.
    crate::thin::thin_flow(
        crate::thin::release_gate(crate::thin::THIN_ENABLED),
        || crate::thin::stamp(&app),
        || {
            let _ = crate::companion::companion_play(&app, "nod");
        },
        |line| companion_note(app.clone(), line, None).map(|_| ()),
    )
}

/// Overwrite fields of now.json (current job packet). Empty string
/// clears that field. Missing args leave the field.
#[tauri::command]
pub fn companion_now(
    job: Option<String>,
    last_error: Option<String>,
    last_green: Option<String>,
    do_not_retry: Option<String>,
    files: Option<Vec<String>>,
) -> Result<String, String> {
    let mut n = load_now();
    if let Some(v) = job {
        n.job = v;
    }
    if let Some(v) = last_error {
        n.last_error = v;
    }
    if let Some(v) = last_green {
        n.last_green = v;
    }
    if let Some(v) = do_not_retry {
        n.do_not_retry = v;
    }
    if let Some(v) = files {
        n.files = v;
    }
    save_now(&n);
    Ok(serde_json::to_string_pretty(&n).unwrap_or_else(|_| "{}".into()))
}

/// The ctl-lit half of the dock LED. Any harness
/// opts in (`orbittyctl companion hack -- on [--who cursor]`); off
/// clears the who. Persisted on the companion slice (a flag, never a
/// transcript); state:changed re-paints the dock. Off when every source
/// is idle — a stuck on-light is a leftover, not a feature.
/// `project_id` is the live agent's
/// pinned planet (pickup passes it) so the dest/planet menus glow that
/// one planet; cleared on off like the who.
pub fn set_companion_hack(
    app: tauri::AppHandle,
    live: bool,
    who: Option<String>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let (hack_live, hack_who, hack_project_id) = crate::state::update(&app, |st| {
        apply_hack(st, live, who, project_id);
        Ok((
            st.companion.hack_live,
            st.companion.hack_who.clone(),
            st.companion.hack_project_id.clone(),
        ))
    })?;
    let _ = app.emit("state:changed", ());
    Ok(serde_json::json!({
        "live": hack_live,
        "who": hack_who,
        "projectId": hack_project_id,
    }))
}

/// Hack fields only. Callers must already be inside `state::update` or
/// go through `set_companion_hack`, which is.
pub(crate) fn apply_hack(
    st: &mut crate::state::PersistedState,
    live: bool,
    who: Option<String>,
    project_id: Option<String>,
) {
    st.companion.hack_live = live;
    st.companion.hack_who = if live {
        who.map(|w| w.trim().to_string())
            .filter(|w| !w.is_empty())
            .unwrap_or_default()
    } else {
        String::new()
    };
    st.companion.hack_project_id = if live {
        project_id
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
    } else {
        None
    };
}

pub(crate) fn run_is_seated(r: &crate::state::AgentRun) -> bool {
    !r.agent_shortcut_id.trim().is_empty()
        || r.herdr
            .as_ref()
            .is_some_and(|h| !h.agent_name.trim().is_empty())
}

pub(crate) fn seated_live<'a>(st: &'a crate::state::PersistedState) -> Option<&'a crate::state::AgentRun> {
    st.agent_runs.iter().find(|r| {
        run_is_seated(r) && matches!(r.state.as_str(), "working" | "starting" | "blocked")
    })
}

/// Who lights the dock: the shortcut's harness, else the herdr kind.
pub(crate) fn who_from_run(st: &crate::state::PersistedState, r: &crate::state::AgentRun) -> Option<String> {
    if !r.agent_shortcut_id.trim().is_empty() {
        if let Some(s) = st
            .agent_shortcuts
            .iter()
            .find(|s| s.id == r.agent_shortcut_id)
        {
            let h = s.harness.trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    r.herdr.as_ref().and_then(|h| {
        let k = h.kind.trim();
        if k.is_empty() {
            None
        } else {
            Some(k.to_string())
        }
    })
}

/// Pickup lights the dock before the harness is up; a dead process
/// that never ran its off-hook leaves hackLive stuck. Both ways: seated
/// working|starting|blocked and dark LED → on (who from shortcut
/// harness or herdr kind); no seated run and lit LED → off.
pub fn heal_stuck_hack(app: &tauri::AppHandle) {
    let _ = crate::state::update(app, |st| {
        let seated = seated_live(st).cloned();
        match (seated.as_ref(), st.companion.hack_live) {
            (Some(r), false) => {
                let who = who_from_run(st, r);
                let project_id = if r.planet_id.trim().is_empty() {
                    None
                } else {
                    Some(r.planet_id.clone())
                };
                apply_hack(st, true, who, project_id);
            }
            (None, true) => apply_hack(st, false, None, None),
            _ => {}
        }
        Ok(())
    });
}

/// Persist Talk source (`system` | `hotkey`).
#[tauri::command]
pub fn set_companion_talk_source(app: tauri::AppHandle, source: String) -> Result<String, String> {
    let src = crate::state::normalize_talk_source(&source);
    crate::state::update(&app, |st| {
        st.companion.talk_source = src.clone();
        Ok(())
    })?;
    crate::talk::stop_if_any();
    let _ = app.emit("state:changed", ());
    Ok(src)
}

/// Persist expand/collapse so the window reopens the way it was left.
#[tauri::command]
pub fn set_companion_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    if !expanded {
        crate::window_chrome::unzoom_companion(&app);
    }
    crate::state::update(&app, |st| {
        st.companion.expanded = expanded;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    // Expanding while hugged restores the tall
    // idle FIRST — the transcript is never a 120px slab.
    if expanded {
        crate::panes::unhug_companion(&app);
        // Nor a composer-grown frame — shed the
        // sprite-on grow extra with the hug.
        crate::panes::ungrow_companion_composer(&app);
    }
    // Expand forces the slab (mouse on) NOW,
    // before the JS re-render; collapse reapplies the ghost from kept hits.
    crate::companion_clear::expanded_changed(&app, expanded);
    Ok(())
}

/// Persist the Full/Head view for ONE
/// surface (`surface` = "collapsed" | "expanded", `view` = "full" |
/// "head"). Two prefs, never one global bool — the other surface's key
/// is untouched. Returns the value actually written.
#[tauri::command]
pub fn set_companion_view(
    app: tauri::AppHandle,
    surface: String,
    view: String,
) -> Result<String, String> {
    let written = crate::state::update(&app, |st| {
        let written = match surface.trim().to_ascii_lowercase().as_str() {
            "collapsed" => {
                let v = crate::state::normalize_companion_view(&view, "full");
                st.companion.view_collapsed = v.clone();
                v
            }
            "expanded" => {
                let v = crate::state::normalize_companion_view(&view, "head");
                st.companion.view_expanded = v.clone();
                v
            }
            "all" => {
                let v = crate::state::normalize_companion_view(&view, "full");
                st.companion.view_collapsed = v.clone();
                st.companion.view_expanded = v.clone();
                v
            }
            other => return Err(format!("bad companion surface: {other:?}")),
        };
        Ok(written)
    })?;
    let _ = app.emit("state:changed", ());
    Ok(written)
}

/// Persist the one sprite flag (default on; a
/// missing key loads true). Same shape as set_companion_speak. R-click on
/// the Full/Head button is the only writer; the per-surface Full/Head
/// prefs are never touched here — they survive Off.
/// Idle collapsed + sprite off hugs the window to
/// the JS-measured CC+dock bar (`hug_height`), keeping the bottom edge;
/// sprite on restores the tall idle. Expanded/hosted never hug. A call
/// with the flag unchanged is the page's hug-height re-report — no
/// persist, no emit, just the bar re-fit.
#[tauri::command]
pub fn set_companion_sprite(
    app: tauri::AppHandle,
    enabled: bool,
    hug_height: Option<f64>,
) -> Result<(), String> {
    let expanded = crate::state::update(&app, |st| {
        let changed = st.companion.sprite != enabled;
        if changed {
            st.companion.sprite = enabled;
        }
        Ok((changed, st.companion.expanded))
    })?;
    if expanded.0 {
        let _ = app.emit("state:changed", ());
    }
    if enabled {
        crate::panes::unhug_companion(&app);
    } else if !expanded.1 {
        // Shed any sprite-on composer grow FIRST
        // — the hug stash must be the true tall idle, never grown.
        crate::panes::ungrow_companion_composer(&app);
        crate::panes::hug_companion(&app, hug_height);
    }
    Ok(())
}

// S2.7 speak replies: macOS `say`, default system voice

/// The in-flight `say` child (at most one). A finished child stays in the
/// slot until the next kill — killing a dead child is a harmless Err.
pub(crate) static SAY_CHILD: std::sync::OnceLock<std::sync::Mutex<Option<std::process::Child>>> =
    std::sync::OnceLock::new();

pub(crate) fn say_slot() -> &'static std::sync::Mutex<Option<std::process::Child>> {
    SAY_CHILD.get_or_init(|| std::sync::Mutex::new(None))
}

/// Kill the in-flight `say`, if any. Child-handle only — never a
/// system-wide pkill, so a user's own `say` in a terminal is untouched.
pub(crate) fn stop_say() {
    if let Ok(mut slot) = say_slot().lock() {
        if let Some(mut child) = slot.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Persist the Speak toggle. Turning it OFF kills any in-flight `say`.
#[tauri::command]
pub fn set_companion_speak(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if !enabled {
        stop_say();
    }
    crate::state::update(&app, |st| {
        st.companion.speak_replies = enabled;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Companion `say` volume 0.0–1.0. Volume 0 mutes the next speak.
#[tauri::command]
pub fn set_companion_speak_volume(app: tauri::AppHandle, volume: f32) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    crate::state::update(&app, |st| {
        st.companion.speak_volume = volume;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Draw / hide the CC + transcript. Composer stays.
/// Captions off while expanded also collapses — empty big box is never
/// a destination (one save, one state:changed, no JS race).
#[tauri::command]
pub fn set_companion_captions(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let collapsed = crate::state::update(&app, |st| {
        st.companion.captions = enabled;
        let collapsed = !enabled && st.companion.expanded;
        if collapsed {
            st.companion.expanded = false;
        }
        Ok(collapsed)
    })?;
    let _ = app.emit("state:changed", ());
    if collapsed {
        crate::companion_clear::expanded_changed(&app, false);
    }
    Ok(())
}

// STARS prompts opt-in + strip ack
// The companion window never writes state.json wholesale; these two
// load-modify-save commands are its only STARS writes. The rail's STARS
// pane writes checkins through its own full persist.

/// Persist the Prompts opt-in (default off). Mirrored by the rail's STARS
/// pane toggle and the companion's Prompts toggle — same key either way.
#[tauri::command]
pub fn set_stars_prompts(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.stars.prompts_opt_in = enabled;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Dismiss the companion's prompt strip: `ack` is `"{day}:{sig}"` — the
/// strip stays hidden until the day rolls or a fresh check-in changes the
/// sig. One show per day, no loop.
#[tauri::command]
pub fn ack_stars_prompt(app: tauri::AppHandle, ack: String) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.stars.prompts_ack = ack;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Read one bot reply aloud with the DEFAULT system voice (`say "<text>"`
/// — no -v, no voice listing). Kills the previous `say` first, so a new
/// reply always wins. macOS only; one honest Err line, never a crash.
/// Volume rides `[[volm 0–1]]` so we don't touch system output volume.
#[tauri::command]
pub fn speak_reply(app: tauri::AppHandle, text: String) -> Result<(), String> {
    stop_say();
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    let st = crate::state::load_state(&app).ok();
    if st.as_ref().is_some_and(|s| !s.companion.speak_replies) {
        return Ok(());
    }
    let vol = st
        .as_ref()
        .map(|s| s.companion.speak_volume.clamp(0.0, 1.0))
        .unwrap_or(1.0);
    if vol <= 0.0 {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let spoken = if (vol - 1.0).abs() < 0.01 {
            text
        } else {
            format!("[[volm {vol:.2}]] {text}")
        };
        let child = Command::new("say")
            .arg(&spoken)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Couldn't speak the reply — `say` failed to start: {e}"))?;
        if let Ok(mut slot) = say_slot().lock() {
            *slot = Some(child);
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("Speak replies needs macOS `say`.".into())
    }
}

