//! Named shell recipes plus optional combos. Reserved chords refused.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

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
/// ⌘⇧O = rail reveal. ⌘⇧Y = Herding Cats confirm.
/// ⌘⇧N = next-card window (leftover).
/// ⌘Space = Spotlight. Super-only letters below
/// are Mac menu equivalents (Hide, Minimize, Quit, Close, …): Carbon
/// RegisterEventHotKey succeeds, then the menu swallows the press while
/// Orbitty is frontmost, so the action looks dead. ⌘⌥+letter is fine.
/// ⌘⌥1/2/3 = dest tiles, ⌘⌥←/→ = ring yaw,
/// ⌘⌥↓ = dismiss the hosted tile, ⌘⌥↑ = raise the last lowered tile —
/// the tile chord wins, so a row carrying one shows `conflict`.
#[cfg(target_os = "macos")]
fn is_reserved(s: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    use tauri_plugin_global_shortcut::{Code, Modifiers};
    if s.mods == Modifiers::SUPER | Modifiers::SHIFT
        && matches!(s.key, Code::KeyO | Code::KeyY | Code::KeyN | Code::Slash)
    {
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
                | Code::ArrowUp
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
            let ran = super::dispatch_action(&app, &name, a.command.clone(), a.cwd.clone());
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

/// First-run seed + toy migration, the one
/// writer for the baked set. Runs on boot before the first `sync_combos`:
///   1. drop the dead toys (`calle-next`, `review-prs` — any command);
///      never restore `orbitty-calle-next`.
///   2. append the real rows (companion ⌥⌘O / kanban ⌥⌘K / stars ⌥⌘S)
///      only when no row already carries that name — an existing row's
///      id/combo/command is the user's and stays; every other row
///      survives untouched (empty, toy-only, and partial saves all heal).
/// Persists through the one save door only when the list changed.
/// Returns the resulting list for the boot combo sync.
pub fn bake_defaults(app: &tauri::AppHandle) -> Vec<ActionShortcut> {
    // Seeded chords stay human-readable bare `orbittyctl`.
    // Dispatch rewrites the bare token to the bundled ctl with `--sock`.
    // `thin` is dev-only. The chord does not shell its command string:
    // dispatch_action sends it through companion_thin (same path as the button).
    let mut seeds: Vec<(&str, &str, &str)> = vec![
        ("companion", "cmd+alt+o", "orbittyctl companion open"),
        ("kanban", "cmd+alt+k", "orbittyctl kanban open"),
        ("stars", "cmd+alt+s", "orbittyctl stars open"),
    ];
    // cfg(any(debug_assertions, feature = "dev-thin")), the same const as the gate.
    if crate::thin::seed_thin_row(crate::thin::THIN_ENABLED) {
        seeds.push(THIN_SEED);
    }
    let before = match crate::state::load_state(app) {
        Ok(st) => st,
        Err(e) => {
            eprintln!("actions-bake: load failed, combos unseeded: {e}");
            return Vec::new();
        }
    };
    let mut after = before.clone();
    let mut changed = false;
    let len_before = after.action_shortcuts.len();
    after
        .action_shortcuts
        .retain(|a| a.name != "calle-next" && a.name != "review-prs");
    if after.action_shortcuts.len() != len_before {
        changed = true;
    }
    for (name, combo, command) in seeds {
        if after.action_shortcuts.iter().any(|a| a.name == name) {
            continue;
        }
        after.action_shortcuts.push(ActionShortcut {
            id: crate::board::new_id("a"),
            name: name.to_string(),
            command: command.to_string(),
            cwd: None,
            combo: Some(combo.to_string()),
        });
        changed = true;
    }
    if changed {
        if let Err(e) = crate::board::save_with_events(app, &before, &after) {
            eprintln!("actions-bake: persist failed: {e}");
        } else {
            eprintln!("actions-bake: seeded/migrated action shortcuts");
        }
    }
    after.action_shortcuts
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

// control plane (`action.*` tools; unix socket only)

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
/// case-insensitive match). A thin row goes through `companion_thin`.
/// Every other row uses the same logged `/bin/zsh -lc` 120s door as a
/// Settings list click. Spark may run what it minted; Spark never gets
/// a raw shell.
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
    let name = found.name.clone();
    let command = found.command.clone();
    let cwd = found.cwd.clone();
    dispatch_action(app, &name, command, cwd)
}

/// If the trimmed command's first token is exactly `orbittyctl`, run the
/// bundled ctl with this app's socket. Any other command is returned as-is.
/// Stored rows stay bare; this is applied at dispatch, not at save.
fn bundled_ctl_command(command: &str, ctl: &Path, sock: &Path) -> String {
    let trimmed = command.trim();
    let token_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    if &trimmed[..token_end] != "orbittyctl" {
        return command.to_string();
    }
    let rest = trimmed[token_end..].trim_start();
    let ctl_q = crate::coordinator::sq(&ctl.to_string_lossy());
    if rest.contains("--sock") {
        format!("{ctl_q} {rest}")
    } else if rest.is_empty() {
        let sock_q = crate::coordinator::sq(&sock.to_string_lossy());
        format!("{ctl_q} --sock {sock_q}")
    } else {
        let sock_q = crate::coordinator::sq(&sock.to_string_lossy());
        format!("{ctl_q} --sock {sock_q} {rest}")
    }
}

/// Every action door shares this: the chord press, `action.run`, the chat
/// line intercept and the CheatSheet click (`run_action`). A thin row never
/// shells out (not the bare `orbittyctl` on PATH, which talks to Stable's
/// socket, and not an old `~/.buzz` script). It calls `companion_thin`.
pub(crate) fn dispatch_action(
    app: &tauri::AppHandle,
    name: &str,
    command: String,
    cwd: Option<String>,
) -> Result<crate::coordinator::ActionResult, String> {
    dispatch_with(
        name,
        command,
        cwd,
        || crate::coordinator::companion_thin(app.clone()),
        |command, cwd| {
            let ctl = crate::coordinator::bundled_orbittyctl();
            let command = if ctl.is_file() {
                bundled_ctl_command(&command, &ctl, &crate::coordinator::control_sock_path(app))
            } else {
                command
            };
            crate::coordinator::run_action_logged(app, command, cwd)
        },
    )
}

fn dispatch_with(
    name: &str,
    command: String,
    cwd: Option<String>,
    thin: impl FnOnce() -> Result<String, String>,
    shell: impl FnOnce(String, Option<String>) -> Result<crate::coordinator::ActionResult, String>,
) -> Result<crate::coordinator::ActionResult, String> {
    if crate::thin::routes_to_companion_thin(name, &command) {
        return match thin() {
            Ok(stdout) => Ok(crate::coordinator::ActionResult {
                ok: true,
                code: Some(0),
                stdout,
                stderr: String::new(),
            }),
            Err(e) => Ok(crate::coordinator::ActionResult {
                ok: false,
                code: Some(1),
                stdout: String::new(),
                stderr: e,
            }),
        };
    }
    shell(command, cwd)
}

/// Dev-only ⌘⌥T seed. Release builds do not append it.
const THIN_SEED: (&str, &str, &str) = ("thin", "cmd+alt+t", "orbittyctl companion thin");

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn thin_seed_uses_the_shared_path() {
        let (name, combo, command) = super::THIN_SEED;
        assert_eq!(name, "thin");
        assert_eq!(combo, "cmd+alt+t");
        assert!(crate::thin::routes_to_companion_thin(name, command));
        assert!(crate::thin::seed_thin_row(true));
        assert!(!crate::thin::seed_thin_row(false));
    }

    fn ok(stdout: &str) -> crate::coordinator::ActionResult {
        crate::coordinator::ActionResult {
            ok: true,
            code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    // B2: a thin row never reaches the shell, whatever door passes it in.
    // "" is the CheatSheet door (run_action gets the command only).
    #[test]
    fn thin_rows_go_to_companion_thin_never_the_shell() {
        let (_, _, seeded) = super::THIN_SEED;
        let legacy = "/Users/x/.buzz/RESEARCH/RGL_TESTBED/.venv/bin/python /Users/x/.buzz/RESEARCH/RGL_TESTBED/scripts/vivo_thin.py stamp";
        let rows: [(&str, &str); 6] = [
            ("thin", seeded),      // chord / action.run / chat by name
            ("", seeded),          // CheatSheet click of the seeded row
            ("thin", legacy),      // Matt's legacy row by name
            ("", legacy),          // legacy row from the CheatSheet
            ("stamp", "orbittyctl companion thin"),
            ("Thin", "echo anything"),
        ];
        for (name, command) in rows {
            let thin_calls = std::cell::Cell::new(0);
            let r = super::dispatch_with(
                name,
                command.to_string(),
                None,
                || {
                    thin_calls.set(thin_calls.get() + 1);
                    Ok("THIN stamped".into())
                },
                |c, _| panic!("{name:?} shelled {c:?}"),
            )
            .unwrap();
            assert_eq!(thin_calls.get(), 1, "{name:?} {command:?}");
            assert!(r.ok && r.stdout == "THIN stamped");
        }
        // A refusal comes back as a failed result, still without a shell.
        let r = super::dispatch_with(
            "thin",
            seeded.into(),
            None,
            || Err("THIN is dev-only and is not in this build.".into()),
            |c, _| panic!("shelled {c:?}"),
        )
        .unwrap();
        assert!(!r.ok && r.code == Some(1) && r.stderr.contains("dev-only"));
        // Other rows still shell through the logged door, unchanged.
        let r = super::dispatch_with(
            "kanban",
            "orbittyctl kanban open".into(),
            Some("/tmp".into()),
            || panic!("kanban went to thin"),
            |c, d| {
                assert_eq!(c, "orbittyctl kanban open");
                assert_eq!(d.as_deref(), Some("/tmp"));
                Ok(ok("opened"))
            },
        )
        .unwrap();
        assert_eq!(r.stdout, "opened");
    }

    // B2 + N2 (T4): all four doors (chat line, CheatSheet row, ⌘⌥T chord,
    // action.run) call dispatch_action; none reaches run_action_logged itself.
    #[test]
    fn every_action_door_calls_dispatch_action() {
        use crate::thin::tests::fn_body;
        let coord = concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
);
        let actions = include_str!("actions.rs");

        let cheat = fn_body(coord, "pub fn run_action(\n");
        assert!(cheat.contains("crate::actions::dispatch_action(&app, \"\", command, cwd)"), "{cheat}");
        assert!(!cheat.contains("run_action_logged("), "{cheat}");

        let at = coord.find("Some(ActionIntercept::Run(shortcut, keep_you)) => {").unwrap();
        let chat = &coord[at..at + coord[at..].find("persist_action_intercept(").unwrap()];
        assert!(chat.contains("crate::actions::dispatch_action(&action_app, &action_name, command, cwd)"), "{chat}");
        assert!(!chat.contains("run_action(") && !chat.contains("run_action_logged("), "{chat}");

        let at = actions.find("fn fire(app: tauri::AppHandle, id: String)").unwrap();
        let chord = &actions[at..at + actions[at..].find("let payload").unwrap()];
        assert!(chord.contains("super::dispatch_action(&app, &name, a.command.clone(), a.cwd.clone())"), "{chord}");
        assert!(!chord.contains("run_action_logged("), "{chord}");

        let run = fn_body(actions, "pub fn action_run(");
        assert!(run.trim_end().ends_with("dispatch_action(app, &name, command, cwd)"), "{run}");

        // dispatch_action's thin arm is companion_thin; only its shell arm
        // reaches run_action_logged.
        let d = fn_body(actions, "pub(crate) fn dispatch_action(");
        assert!(d.contains("|| crate::coordinator::companion_thin(app.clone())"), "{d}");
        let w = fn_body(actions, "fn dispatch_with(");
        assert!(w.contains("routes_to_companion_thin(name, &command)"), "{w}");
        assert!(!w.contains("Command::new") && !w.contains("run_action_logged"), "{w}");
    }

    // Guard: nothing on the THIN path spawns the bare `orbittyctl` from PATH
    // or a ~/.buzz script. THIN spawns only the configured python + script,
    // and check_paths refuses a .buzz script.
    #[test]
    fn thin_never_spawns_bare_orbittyctl_or_buzz() {
        let thin = include_str!("thin.rs");
        let spawns: Vec<&str> = thin.lines().filter(|l| l.contains("Command::new(")).collect();
        assert!(!spawns.is_empty());
        for l in &spawns {
            let l = l.trim();
            assert!(
                l == "let mut child = Command::new(&cfg.python)" || l.starts_with("let mut child = Command::new(\"python3\")"),
                "unexpected spawn in thin.rs: {l}"
            );
        }
        let flow = fn_body_of_companion_thin();
        assert!(!flow.contains("orbittyctl") && !flow.contains(".buzz") && !flow.contains("Command::new"));
        let script = include_str!("../../../scripts/thin/vivo_thin.py");
        assert!(!script.contains("orbittyctl") && !script.contains(".buzz"));
    }

    fn fn_body_of_companion_thin() -> &'static str {
        crate::thin::tests::fn_body(concat!(
include_str!("../coordinator/hermes.rs"),
include_str!("../coordinator/allowlist.rs"),
include_str!("../coordinator/plug.rs"),
include_str!("../coordinator/mouth_cli.rs"),
include_str!("../coordinator/commands.rs"),
include_str!("../coordinator/intercept.rs"),
include_str!("../coordinator/send.rs"),
include_str!("../coordinator/soul_wake.rs"),
include_str!("../coordinator/action.rs"),
include_str!("../coordinator/paths.rs"),
include_str!("../coordinator/cursor_perm.rs"),
include_str!("../coordinator/launch_agent.rs"),
include_str!("../coordinator/mac.rs"),
include_str!("../coordinator/launch_tests.rs")
), "pub fn companion_thin(")
    }

    #[test]
    fn bare_ctl_rewritten_with_sock() {
        let ctl = Path::new("/Applications/Orbitty.app/Contents/MacOS/orbittyctl");
        let sock = Path::new("/tmp/control.sock");
        let got = super::bundled_ctl_command("orbittyctl kanban open", ctl, sock);
        assert_eq!(
            got,
            format!(
                "{} --sock {} kanban open",
                crate::coordinator::sq(&ctl.to_string_lossy()),
                crate::coordinator::sq(&sock.to_string_lossy()),
            )
        );
    }

    #[test]
    fn existing_sock_not_doubled() {
        let ctl = Path::new("/Applications/Orbitty.app/Contents/MacOS/orbittyctl");
        let sock = Path::new("/tmp/control.sock");
        let got = super::bundled_ctl_command(
            "orbittyctl --sock /already.sock kanban open",
            ctl,
            sock,
        );
        assert_eq!(
            got,
            format!(
                "{} --sock /already.sock kanban open",
                crate::coordinator::sq(&ctl.to_string_lossy()),
            )
        );
        assert_eq!(got.matches("--sock").count(), 1);
        assert!(!got.contains("/tmp/control.sock"));
    }

    #[test]
    fn absolute_path_untouched() {
        let cmd = "/usr/local/bin/orbittyctl kanban open";
        let got = super::bundled_ctl_command(cmd, Path::new("/a/orbittyctl"), Path::new("/b/sock"));
        assert_eq!(got, cmd);
    }

    #[test]
    fn ctl_prefix_word_untouched() {
        let cmd = "orbittyctl2 kanban open";
        let got = super::bundled_ctl_command(cmd, Path::new("/a/orbittyctl"), Path::new("/b/sock"));
        assert_eq!(got, cmd);
    }

    #[test]
    fn mid_command_untouched() {
        let cmd = "echo orbittyctl";
        let got = super::bundled_ctl_command(cmd, Path::new("/a/orbittyctl"), Path::new("/b/sock"));
        assert_eq!(got, cmd);
    }

    #[test]
    fn paths_with_spaces_single_quoted() {
        let ctl = Path::new("/Applications/Orbitty Nightly.app/Contents/MacOS/orbittyctl");
        let sock = Path::new("/tmp/nightly sock/control.sock");
        let got = super::bundled_ctl_command("orbittyctl stars open", ctl, sock);
        assert_eq!(
            got,
            "'/Applications/Orbitty Nightly.app/Contents/MacOS/orbittyctl' --sock '/tmp/nightly sock/control.sock' stars open"
        );
    }

    #[test]
    fn thin_row_still_routes_to_companion_thin() {
        let (_, _, seeded) = super::THIN_SEED;
        let rewritten = super::bundled_ctl_command(
            seeded,
            Path::new("/Applications/Orbitty.app/Contents/MacOS/orbittyctl"),
            Path::new("/tmp/control.sock"),
        );
        assert_ne!(rewritten, seeded);
        assert!(crate::thin::routes_to_companion_thin("thin", seeded));
        let thin_calls = std::cell::Cell::new(0);
        let r = super::dispatch_with(
            "thin",
            seeded.to_string(),
            None,
            || {
                thin_calls.set(thin_calls.get() + 1);
                Ok("THIN stamped".into())
            },
            |c, _| panic!("thin row shelled {c:?}"),
        )
        .unwrap();
        assert_eq!(thin_calls.get(), 1);
        assert!(r.ok && r.stdout == "THIN stamped");
    }
}
