//! GO-ORBITTY-ACTION-FILE — an action is a named shell recipe plus an
//! optional key combo. Spark (or a human) writes rows through the control
//! plane (`orbittyctl action add`); the face is the Settings list, not a
//! Launchpad well. SoT stays whole-object `state.json`.
//!
//! Combos ride the same `tauri_plugin_global_shortcut` as the ⌘⇧O summon.
//! Registrations re-sync on load and on every persist of
//! `actionShortcuts` (the `save_state` command and the `action.*` control
//! tools all call `sync_combos`). Reserved chords (⌘⇧O summon, ⌘Space
//! Spotlight, the ⌘⌥1/2/3 + ⌘⌥←/→ tile chords) are refused; an
//! OS/other-app collision skips registration
//! and marks the row `conflict` in the list — never a crash. Empty combo
//! = Spirit-only (chat intercept or a Settings name click).

use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::state::ActionShortcut;

/// Parse the locked combo format ("cmd+alt+m") through the plugin's own
/// parser (case-insensitive tokens: cmd/command/super, alt/option, ctrl,
/// shift, then one key). A bare key with no modifier is refused — a
/// global bare letter would eat the user's typing.
fn parse_combo(combo: &str) -> Result<tauri_plugin_global_shortcut::Shortcut, String> {
    let raw = combo.trim();
    if raw.is_empty() {
        return Err("empty combo".into());
    }
    let s: tauri_plugin_global_shortcut::Shortcut = raw
        .parse()
        .map_err(|e| format!("bad combo {raw:?}: {e}"))?;
    if s.mods.is_empty() {
        return Err(format!(
            "combo {raw:?} needs a modifier (cmd/alt/ctrl/shift)"
        ));
    }
    Ok(s)
}

/// Never register these — a row carrying one shows `conflict`.
/// ⌘⇧O = companion summon. ⌘Space = Spotlight. Super-only letters below
/// are Mac menu equivalents (Hide, Minimize, Quit, Close, …): Carbon
/// RegisterEventHotKey succeeds, then the menu swallows the press while
/// Orbitty is frontmost, so the action looks dead. ⌘⌥+letter is fine.
/// GO-ORBITTY-TILE-CHORDS: ⌘⌥1/2/3 = dest tiles, ⌘⌥←/→ = ring yaw,
/// ⌘⌥↓ = dismiss the hosted tile — the tile chord wins, so a row
/// carrying one shows `conflict`.
#[cfg(target_os = "macos")]
fn is_reserved(s: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    use tauri_plugin_global_shortcut::{Code, Modifiers};
    if s.mods == Modifiers::SUPER | Modifiers::SHIFT && s.key == Code::KeyO {
        return true;
    }
    if s.mods == Modifiers::SUPER | Modifiers::ALT
        && matches!(
            s.key,
            Code::Digit1
                | Code::Digit2
                | Code::Digit3
                | Code::ArrowLeft
                | Code::ArrowRight
                | Code::ArrowDown
        )
    {
        return true;
    }
    if s.mods == Modifiers::SUPER && matches!(s.key, Code::Space | Code::Tab | Code::Backquote) {
        return true;
    }
    s.mods == Modifiers::SUPER
        && matches!(
            s.key,
            Code::KeyH
                | Code::KeyM
                | Code::KeyQ
                | Code::KeyW
                | Code::KeyN
                | Code::KeyO
                | Code::KeyS
                | Code::KeyP
                | Code::KeyA
                | Code::KeyZ
                | Code::KeyX
                | Code::KeyC
                | Code::KeyV
                | Code::Comma
        )
}

#[cfg(target_os = "macos")]
mod reg {
    use super::{is_reserved, parse_combo};
    use crate::state::ActionShortcut;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::{Mutex, MutexGuard};
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    /// action id → the shortcut we registered for it (unregister needs the
    /// same value handed back).
    static REGISTERED: Mutex<Option<HashMap<String, Shortcut>>> = Mutex::new(None);
    /// action id → "ok" | "conflict", for every row that carries a combo.
    static STATUS: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);

    fn lock<T>(m: &Mutex<Option<T>>) -> MutexGuard<'_, Option<T>> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn statuses() -> BTreeMap<String, String> {
        lock(&STATUS).clone().unwrap_or_default()
    }

    /// A combo press runs the action's CURRENT command (looked up fresh by
    /// id, so an edit without a chord change never goes stale) on a worker
    /// thread — the handler itself never blocks on /bin/zsh. Emit so
    /// Settings can show the same run note as a name click (`echo hi`
    /// otherwise looks like the chord did nothing).
    fn fire(app: tauri::AppHandle, id: String) {
        std::thread::spawn(move || {
            use serde_json::{json, Value};
            use tauri::Emitter;
            let Ok(st) = crate::state::load_state(&app) else {
                return;
            };
            let Some(a) = st.action_shortcuts.iter().find(|a| a.id == id) else {
                return;
            };
            let name = a.name.clone();
            let ran = crate::coordinator::run_action_logged(
                &app,
                a.command.clone(),
                a.cwd.clone(),
            );
            let payload = match ran {
                Ok(r) => json!({
                    "id": id,
                    "name": name,
                    "ok": r.ok,
                    "code": r.code,
                    "stdout": r.stdout,
                    "stderr": r.stderr,
                }),
                Err(e) => json!({
                    "id": id,
                    "name": name,
                    "ok": false,
                    "code": Value::Null,
                    "stdout": "",
                    "stderr": e,
                }),
            };
            let _ = app.emit("orbitty:action-ran", payload);
        });
    }

    /// Reconcile registrations with the persisted list. Best-effort: every
    /// failure lands as a `conflict` mark on the row, never an Err/panic.
    pub fn sync(app: &tauri::AppHandle, shortcuts: &[ActionShortcut]) {
        let mut status: BTreeMap<String, String> = BTreeMap::new();
        let mut desired: Vec<(String, Shortcut)> = Vec::new();
        for a in shortcuts {
            let Some(c) = a.combo.as_deref().map(str::trim).filter(|c| !c.is_empty()) else {
                continue;
            };
            match parse_combo(c) {
                Ok(s) if is_reserved(&s) => {
                    status.insert(a.id.clone(), "conflict".into());
                }
                Ok(s) => desired.push((a.id.clone(), s)),
                Err(_) => {
                    status.insert(a.id.clone(), "conflict".into());
                }
            }
        }

        let gs = app.global_shortcut();
        let mut guard = lock(&REGISTERED);
        let reg = guard.get_or_insert_with(HashMap::new);

        // Unregister rows that are gone or whose chord changed.
        let stale: Vec<String> = reg
            .iter()
            .filter(|(id, s)| !desired.iter().any(|(d, ds)| d == *id && ds == *s))
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            if let Some(s) = reg.remove(&id) {
                let _ = gs.unregister(s);
            }
        }

        // Register the rest. First row wins a duplicated chord; the loser
        // and any OS/other-app collision land as `conflict`.
        for (id, s) in desired {
            if reg.get(&id) == Some(&s) {
                status.insert(id, "ok".into());
                continue;
            }
            let handle = app.clone();
            let aid = id.clone();
            match gs.on_shortcut(s, move |_app, _s, ev| {
                if ev.state() != ShortcutState::Pressed {
                    return;
                }
                fire(handle.clone(), aid.clone());
            }) {
                Ok(()) => {
                    reg.insert(id.clone(), s);
                    status.insert(id, "ok".into());
                }
                Err(_) => {
                    status.insert(id, "conflict".into());
                }
            }
        }

        *lock(&STATUS) = Some(status);
    }
}

/// Re-sync combo registrations with a persisted list. No-op off macOS.
#[cfg(target_os = "macos")]
pub fn sync_combos(app: &tauri::AppHandle, shortcuts: &[ActionShortcut]) {
    reg::sync(app, shortcuts);
}

/// Re-sync combo registrations with a persisted list. No-op off macOS.
#[cfg(not(target_os = "macos"))]
pub fn sync_combos(_app: &tauri::AppHandle, _shortcuts: &[ActionShortcut]) {}

#[cfg(target_os = "macos")]
fn combo_statuses() -> BTreeMap<String, String> {
    reg::statuses()
}

#[cfg(not(target_os = "macos"))]
fn combo_statuses() -> BTreeMap<String, String> {
    BTreeMap::new()
}

/// Settings → Actions reads this to mark conflicts. Keyed by action id; a
/// Spirit-only row (no combo) has no entry.
#[tauri::command]
pub fn action_combo_status() -> BTreeMap<String, String> {
    combo_statuses()
}

// ---------- control plane (`action.*` tools; unix socket only) ----------

/// `action.list` — the persisted rows plus runtime combo status
/// ("ok" | "conflict" | null = Spirit-only). Read-profile safe.
#[cfg(unix)]
pub fn action_list(app: &tauri::AppHandle) -> Result<Value, String> {
    let st = crate::state::load_state(app)?;
    let status = combo_statuses();
    let rows: Vec<Value> = st
        .action_shortcuts
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "command": a.command,
                "cwd": a.cwd,
                "combo": a.combo,
                "status": status.get(&a.id).cloned(),
            })
        })
        .collect();
    Ok(Value::Array(rows))
}

/// `action.add` — mint a row through the one save door. A present `combo`
/// must parse (garbage never reaches the list); a reserved chord mints
/// fine and simply shows `conflict` (registration is refused, never an
/// error here). The companion profile may call this — Spark's mint door.
#[cfg(unix)]
pub fn action_add(
    app: &tauri::AppHandle,
    name: &str,
    cmd: &str,
    cwd: Option<&str>,
    combo: Option<&str>,
) -> Result<ActionShortcut, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("action.add: empty name".into());
    }
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return Err("action.add: empty cmd".into());
    }
    let combo = combo.map(str::trim).filter(|c| !c.is_empty());
    if let Some(c) = combo {
        parse_combo(c)?;
    }
    let before = crate::state::load_state(app)?;
    let action = ActionShortcut {
        id: crate::board::new_id("a"),
        name: name.to_string(),
        command: cmd.to_string(),
        cwd: cwd
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string),
        combo: combo.map(str::to_string),
    };
    let mut after = before.clone();
    after.action_shortcuts.push(action.clone());
    crate::board::save_with_events(app, &before, &after)?;
    sync_combos(app, &after.action_shortcuts);
    crate::event_log::append_best(
        app,
        "action-add",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "actionId": action.id, "name": action.name }),
    );
    Ok(action)
}

/// `action.rm` — remove by id; the chord unregisters on the re-sync.
#[cfg(unix)]
pub fn action_rm(app: &tauri::AppHandle, id: &str) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    if !before.action_shortcuts.iter().any(|a| a.id == id) {
        return Err(format!("action.rm: unknown action id {id:?}"));
    }
    let mut after = before.clone();
    after.action_shortcuts.retain(|a| a.id != id);
    crate::board::save_with_events(app, &before, &after)?;
    sync_combos(app, &after.action_shortcuts);
    crate::event_log::append_best(
        app,
        "action-rm",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "actionId": id }),
    );
    Ok(json!({ "ok": true, "id": id }))
}

/// `action.run` — by `id` (exact) or `name` (exact, then a unique
/// case-insensitive match). The run goes through the same logged
/// `/bin/zsh -lc` 120s door as a Settings list click. Spark may run what
/// it minted; Spark never gets a raw shell.
#[cfg(unix)]
pub fn action_run(
    app: &tauri::AppHandle,
    id: Option<&str>,
    name: Option<&str>,
) -> Result<crate::coordinator::ActionResult, String> {
    let st = crate::state::load_state(app)?;
    let found = if let Some(id) = id.map(str::trim).filter(|s| !s.is_empty()) {
        st.action_shortcuts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("action.run: unknown action id {id:?}"))?
    } else if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(a) = st.action_shortcuts.iter().find(|a| a.name == name) {
            a
        } else {
            let lower = name.to_lowercase();
            let matches: Vec<&ActionShortcut> = st
                .action_shortcuts
                .iter()
                .filter(|a| a.name.to_lowercase() == lower)
                .collect();
            match matches.len() {
                1 => matches[0],
                0 => return Err(format!("action.run: no action named {name:?}")),
                _ => {
                    return Err(format!(
                        "action.run: {name:?} names {} actions — use --id",
                        matches.len()
                    ))
                }
            }
        }
    } else {
        return Err("action.run: need --id ID or --name N".into());
    };
    crate::coordinator::run_action_logged(app, found.command.clone(), found.cwd.clone())
}
