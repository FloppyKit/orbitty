//! Open the real terminal for one seat.
//! A core seat gets a window whose only line is `hc attach`.
//! Watch-only unless the caller asks for full attach.
//! An attach already on screen comes forward; a second window opens
//! only when that seat has none.
//! A tui seat is focused, then a window runs the bare binary: a null-stdio
//! launch exits at once and shows nothing (S0).
//! This file never starts a core server and never types into the seat.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::state::{AgentRun, AgentRunHerdr};

use super::client;
use super::core;
use super::health;
use super::tui;

const NOTHING: &str = "Nothing to open for this card.";
const FINISHED: &str = "This seat is finished.";
const UNAVAILABLE: &str = "This seat's terminal isn't available right now.";
const OPEN_FAIL: &str = "Couldn't open the terminal.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Observe,
    Full,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mode::Observe => "observe",
            Mode::Full => "full",
        }
    }

    pub(crate) fn from_observe(observe: Option<bool>) -> Self {
        match observe {
            Some(false) => Mode::Full,
            _ => Mode::Observe,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Core { terminal: String },
    Tui { workspace: String, agent: String },
    Refuse { reason: String },
}

/// Seat choice follows the run that started it. `mode` only changes the
/// attach line, never which seat is chosen.
pub(crate) fn plan(run: Option<&AgentRun>, mode: Mode) -> Plan {
    let _mode = mode;
    let Some(run) = run else {
        return Plan::Refuse {
            reason: NOTHING.into(),
        };
    };
    if is_finished(run) {
        return Plan::Refuse {
            reason: FINISHED.into(),
        };
    }
    let Some(h) = run.herdr.as_ref() else {
        return Plan::Refuse {
            reason: NOTHING.into(),
        };
    };
    if h.backend == "core" {
        let terminal = terminal_id(h);
        if terminal.is_empty() {
            return Plan::Refuse {
                reason: NOTHING.into(),
            };
        }
        return Plan::Core { terminal };
    }
    if h.backend == "tui" || h.backend.trim().is_empty() {
        return Plan::Tui {
            workspace: h.workspace.clone(),
            agent: h.agent_name.clone(),
        };
    }
    Plan::Refuse {
        reason: NOTHING.into(),
    }
}

fn is_finished(run: &AgentRun) -> bool {
    matches!(
        run.state.trim().to_ascii_lowercase().as_str(),
        "done" | "finished"
    )
}

fn terminal_id(h: &AgentRunHerdr) -> String {
    let terminal = h.terminal.trim();
    if !terminal.is_empty() {
        return terminal.to_string();
    }
    h.agent_name.trim().to_string()
}

pub(crate) fn script_text(hc: &str, terminal: &str, mode: Mode) -> String {
    let bin = crate::coordinator::sq(hc);
    let id = crate::coordinator::sq(terminal);
    let line = match mode {
        Mode::Observe => format!("{bin} attach --observe {id}"),
        Mode::Full => format!("{bin} attach {id}"),
    };
    format!("#!/bin/zsh\n{line}\n")
}

pub(crate) fn tui_script_text(bin: &str) -> String {
    format!("#!/bin/zsh\n{}\n", crate::coordinator::sq(bin))
}

pub(crate) struct Prepared {
    pub ok: bool,
    pub backend: String,
    pub mode: String,
    pub terminal: Option<String>,
    pub reason: Option<String>,
    pub body: Option<String>,
    pub workspace: String,
    pub agent: String,
}

fn refuse(backend: &str, mode: &str, reason: impl Into<String>) -> Prepared {
    Prepared {
        ok: false,
        backend: backend.into(),
        mode: mode.into(),
        terminal: None,
        reason: Some(reason.into()),
        body: None,
        workspace: String::new(),
        agent: String::new(),
    }
}

/// Decide the jump. Does not write a file and does not open a window.
pub(crate) fn prepare(run: Option<&AgentRun>, mode: Mode) -> Prepared {
    match plan(run, mode) {
        Plan::Refuse { reason } => refuse("", mode.as_str(), reason),
        Plan::Core { terminal } => {
            let Some(bin) = core::locate() else {
                return refuse("core", mode.as_str(), client::missing_core_line());
            };
            if health::is_down() {
                health::poll();
                if health::is_down() {
                    return refuse("core", mode.as_str(), UNAVAILABLE);
                }
            }
            Prepared {
                ok: true,
                backend: "core".into(),
                mode: mode.as_str().into(),
                terminal: Some(terminal.clone()),
                reason: None,
                body: Some(script_text(&bin.display().to_string(), &terminal, mode)),
                workspace: String::new(),
                agent: String::new(),
            }
        }
        Plan::Tui { workspace, agent } => {
            let Some(bin) = tui::locate() else {
                return refuse("tui", "focus", UNAVAILABLE);
            };
            Prepared {
                ok: true,
                backend: "tui".into(),
                mode: "focus".into(),
                terminal: None,
                reason: None,
                body: Some(tui_script_text(&bin.display().to_string())),
                workspace,
                agent,
            }
        }
    }
}

pub(crate) fn find_run<'a>(runs: &'a [AgentRun], token: &str) -> Option<&'a AgentRun> {
    let id = token
        .trim()
        .strip_prefix("agent:")
        .unwrap_or(token.trim())
        .trim();
    if id.is_empty() {
        return None;
    }
    if let Some(run) = runs.iter().find(|r| r.card_id == id) {
        return Some(run);
    }
    runs.iter().find(|r| {
        r.herdr
            .as_ref()
            .is_some_and(|h| h.terminal.trim() == id || h.agent_name.trim() == id)
    })
}

pub(crate) fn command_path(dir: &Path, card: &str) -> PathBuf {
    let stem = crate::coordinator::safe_id(card);
    let stem = if stem.is_empty() {
        "seat".to_string()
    } else {
        stem
    };
    dir.join(format!("jump-{stem}.command"))
}

pub(crate) fn write_command(dir: &Path, card: &str, body: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = command_path(dir, card);
    fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .permissions();
        perm.set_mode(0o700);
        fs::set_permissions(&path, perm).map_err(|e| format!("mode {}: {e}", path.display()))?;
    }
    Ok(path)
}

pub(crate) fn terminal_argv(app_name: &str, script: &Path) -> Vec<String> {
    crate::coordinator::open_terminal_argv(app_name, script)
}

/// Args for `open`. Ghostty does not execute a `.command` file: `open -a
/// Ghostty <script>` returns and the script never starts. On macOS the
/// `ghostty` CLI cannot launch the app, so this is `open -na Ghostty.app
/// --args`. Ghostty 1.4 execs the single value after `-e` (a script path,
/// spaces included). `/bin/zsh` and the script as two words is the split
/// that opened an empty window. Other apps keep `-a <app> <script>`.
pub(crate) fn open_argv(app_name: &str, script: &Path) -> Vec<String> {
    launch_argv(app_name, script, None)
}

/// `open_argv`, plus a Ghostty `--title=` of the seat id when that id is plain.
/// Probed on Ghostty 1.4.0: `--title=<id> -e <script>` runs the script and
/// names the window with the id.
pub(crate) fn launch_argv(app_name: &str, script: &Path, title: Option<&str>) -> Vec<String> {
    if app_name == "Ghostty" {
        let mut argv = vec!["-na".into(), "Ghostty.app".into(), "--args".into()];
        if let Some(flag) = title_flag(title.unwrap_or("")) {
            argv.push(flag);
        }
        argv.push("-e".into());
        argv.push(script.display().to_string());
        argv
    } else {
        let _title = title;
        terminal_argv(app_name, script)
    }
}

fn title_flag(terminal: &str) -> Option<String> {
    let t = terminal.trim();
    if t.is_empty() {
        return None;
    }
    let plain = t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.');
    if !plain {
        return None;
    }
    Some(format!("--title={t}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LiveProc {
    pub pid: i32,
    pub ppid: i32,
    pub path: String,
    pub args: Vec<String>,
}

/// `KERN_PROCARGS2` body: argc, exec path, padding nuls, then argc strings.
pub(crate) fn parse_procargs2(buf: &[u8]) -> Vec<String> {
    if buf.len() < 4 {
        return Vec::new();
    }
    let argc = i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if argc <= 0 {
        return Vec::new();
    }
    let mut i = 4usize;
    while i < buf.len() && buf[i] != 0 {
        i += 1;
    }
    if i >= buf.len() {
        return Vec::new();
    }
    i += 1;
    while i < buf.len() && buf[i] == 0 {
        i += 1;
    }
    let mut out = Vec::new();
    for _ in 0..argc {
        if i >= buf.len() {
            break;
        }
        let start = i;
        while i < buf.len() && buf[i] != 0 {
            i += 1;
        }
        out.push(String::from_utf8_lossy(&buf[start..i]).into_owned());
        if i < buf.len() {
            i += 1;
        }
    }
    out
}

/// True when `args` is an attach of exactly `terminal` (watch-only or full).
pub(crate) fn argv_attaches(args: &[String], terminal: &str) -> bool {
    let term = terminal.trim();
    if term.is_empty() {
        return false;
    }
    args.iter().any(|a| a == "attach") && args.iter().any(|a| a == term)
}

fn path_is_terminal_app(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.contains("ghostty") || l.contains("terminal.app")
}

fn terminal_app_ancestor(procs: &[LiveProc], start: i32) -> Option<i32> {
    let mut pid = start;
    let mut guard = 0;
    while guard < 16 {
        guard += 1;
        let proc = procs.iter().find(|p| p.pid == pid)?;
        if pid != start && path_is_terminal_app(&proc.path) {
            return Some(pid);
        }
        if proc.ppid <= 0 || proc.ppid == pid {
            return None;
        }
        pid = proc.ppid;
    }
    None
}

/// Pid of the Ghostty or Terminal.app that already shows `terminal`.
pub(crate) fn attached_app_pid(procs: &[LiveProc], terminal: &str) -> Option<i32> {
    for proc in procs {
        if !argv_attaches(&proc.args, terminal) {
            continue;
        }
        if let Some(app) = terminal_app_ancestor(procs, proc.pid) {
            return Some(app);
        }
    }
    None
}

pub(crate) fn app_running_script(procs: &[LiveProc], script: &Path) -> Option<i32> {
    let needle = script.display().to_string();
    if needle.is_empty() {
        return None;
    }
    for proc in procs {
        if !proc.args.iter().any(|a| a == &needle) {
            continue;
        }
        if let Some(app) = terminal_app_ancestor(procs, proc.pid) {
            return Some(app);
        }
    }
    None
}

pub(crate) fn live_procs() -> Vec<LiveProc> {
    #[cfg(target_os = "macos")]
    {
        darwin::live_procs()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
mod darwin {
    use super::{parse_procargs2, LiveProc};

    #[link(name = "proc")]
    extern "C" {
        fn proc_listallpids(buffer: *mut i32, buffersize: i32) -> i32;
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut u8, buffersize: i32) -> i32;
        fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
    }

    extern "C" {
        fn sysctl(
            name: *mut i32,
            namelen: u32,
            oldp: *mut u8,
            oldlenp: *mut usize,
            newp: *mut u8,
            newlen: usize,
        ) -> i32;
    }

    const PROC_PIDTBSDINFO: i32 = 3;
    const PBI_SIZE: i32 = 136;
    const PPID_OFF: usize = 16;

    pub(super) fn live_procs() -> Vec<LiveProc> {
        let need = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
        if need <= 0 {
            return Vec::new();
        }
        let mut cap = need as usize + 64;
        let pids = loop {
            let mut buf = vec![0i32; cap];
            let nbytes = (buf.len() * std::mem::size_of::<i32>()) as i32;
            let n = unsafe { proc_listallpids(buf.as_mut_ptr(), nbytes) };
            if n <= 0 {
                return Vec::new();
            }
            let n = n as usize;
            if n < buf.len() {
                buf.truncate(n);
                break buf;
            }
            if cap > 100_000 {
                buf.truncate(n.min(buf.len()));
                break buf;
            }
            cap *= 2;
        };
        let mut out = Vec::new();
        for pid in pids {
            if pid <= 0 {
                continue;
            }
            let mut info = [0u8; 136];
            let got =
                unsafe { proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, info.as_mut_ptr(), PBI_SIZE) };
            if got < PPID_OFF as i32 + 4 {
                continue;
            }
            let ppid = i32::from_ne_bytes([
                info[PPID_OFF],
                info[PPID_OFF + 1],
                info[PPID_OFF + 2],
                info[PPID_OFF + 3],
            ]);
            let mut path_buf = [0u8; 1024];
            let plen = unsafe { proc_pidpath(pid, path_buf.as_mut_ptr(), path_buf.len() as u32) };
            let path = if plen > 0 {
                let n = (plen as usize).min(path_buf.len().saturating_sub(1));
                let n = path_buf[..n].iter().position(|&b| b == 0).unwrap_or(n);
                String::from_utf8_lossy(&path_buf[..n]).into_owned()
            } else {
                String::new()
            };
            let args = if wants_args(&path) {
                proc_args(pid)
            } else {
                Vec::new()
            };
            out.push(LiveProc {
                pid,
                ppid,
                path,
                args,
            });
        }
        out
    }

    fn wants_args(path: &str) -> bool {
        let l = path.to_ascii_lowercase();
        l.contains("herdr")
            || l.ends_with("/hc")
            || l.ends_with("/zsh")
            || l.ends_with("/bash")
            || l.ends_with("/login")
    }

    fn proc_args(pid: i32) -> Vec<String> {
        let mut mib = [1i32, 49i32, pid];
        let mut size: usize = 0;
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                3,
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || size == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; size.saturating_add(8)];
        let mut size2 = buf.len();
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr(),
                &mut size2,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || size2 < 4 {
            return Vec::new();
        }
        buf.truncate(size2);
        parse_procargs2(&buf)
    }
}

fn focus_attached(terminal: &str, script: &Path) -> bool {
    #[cfg(test)]
    {
        let _ = (terminal, script);
        false
    }
    #[cfg(not(test))]
    {
        let procs = live_procs();
        let Some(pid) =
            attached_app_pid(&procs, terminal).or_else(|| app_running_script(&procs, script))
        else {
            return false;
        };
        raise_attached(pid, terminal)
    }
}

#[cfg(not(test))]
fn raise_attached(pid: i32, title: &str) -> bool {
    let line = match title_flag(title) {
        Some(flag) => {
            let name = flag.trim_start_matches("--title=");
            format!(
                "tell application \"System Events\"\n\
                 tell (first process whose unix id is {pid})\n\
                 set frontmost to true\n\
                 repeat with w in windows\n\
                 if name of w is \"{name}\" then\n\
                 perform action \"AXRaise\" of w\n\
                 set value of attribute \"AXMain\" of w to true\n\
                 end if\n\
                 end repeat\n\
                 end tell\n\
                 end tell"
            )
        }
        None => format!(
            "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
        ),
    };
    crate::coordinator::spawn_detached("osascript", &["-e", &line])
}

fn reveal(app_name: &str, script: &Path, terminal: Option<&str>) -> bool {
    let term = terminal.map(str::trim).filter(|s| !s.is_empty());
    if let Some(term) = term {
        if focus_attached(term, script) {
            return true;
        }
    }
    let argv = match term {
        Some(term) => launch_argv(app_name, script, Some(term)),
        None => open_argv(app_name, script),
    };
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    #[cfg(test)]
    {
        let _kept = refs;
        !app_name.is_empty() && !script.as_os_str().is_empty()
    }
    #[cfg(not(test))]
    {
        crate::coordinator::spawn_detached("open", &refs)
    }
}

fn context_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = crate::coordinator::app_data(app)?.join("context");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn focus_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        let root = crate::state::instance::support_dir(crate::state::instance::STABLE_ID).ok()?;
        let dir = root.join("context");
        fs::create_dir_all(&dir).ok()?;
        Some(dir)
    }
}

fn to_value(p: &Prepared) -> Value {
    let mut v = json!({
        "ok": p.ok,
        "backend": p.backend,
        "mode": p.mode,
    });
    if let Some(t) = &p.terminal {
        v["terminal"] = json!(t);
    }
    if let Some(r) = &p.reason {
        v["reason"] = json!(r);
    }
    v
}

fn log_jump(app: &AppHandle, card: &str, planet: Option<&str>, p: &Prepared) {
    crate::event_log::append_best(
        app,
        "herdr-jump",
        None,
        planet,
        Some("agent"),
        true,
        &json!({
            "card": card,
            "backend": p.backend,
            "mode": p.mode,
            "ok": p.ok,
        }),
    );
}

fn open_prepared(
    app: &AppHandle,
    card: &str,
    setting: &str,
    prepared: &Prepared,
) -> Result<(), String> {
    let body = prepared
        .body
        .as_deref()
        .ok_or_else(|| OPEN_FAIL.to_string())?;
    if prepared.backend == "tui" {
        if let Some(bin) = tui::locate() {
            let workspace = prepared.workspace.trim();
            if !workspace.is_empty() {
                let _ = tui::focus_workspace(&bin, workspace);
            }
            let agent = prepared.agent.trim();
            if !agent.is_empty() {
                let _ = tui::focus_agent(&bin, agent);
            }
        }
    }
    let dir = context_dir(app).map_err(|_| OPEN_FAIL.to_string())?;
    let path = write_command(&dir, card, body).map_err(|_| OPEN_FAIL.to_string())?;
    let app_name = crate::coordinator::resolve_agent_terminal(
        setting,
        crate::coordinator::ghostty_installed(),
    );
    if reveal(app_name, &path, prepared.terminal.as_deref()) {
        Ok(())
    } else {
        Err(OPEN_FAIL.into())
    }
}

pub(crate) fn jump_for(app: &AppHandle, card: &str, observe: Option<bool>) -> Value {
    let mode = Mode::from_observe(observe);
    let requested = card
        .trim()
        .strip_prefix("agent:")
        .unwrap_or(card.trim())
        .trim();
    let loaded = crate::state::load_state(app).ok();
    let runs = loaded
        .as_ref()
        .map(|st| st.agent_runs.clone())
        .unwrap_or_default();
    let setting = loaded
        .as_ref()
        .map(|st| st.agent_terminal.clone())
        .unwrap_or_default();
    let found = find_run(&runs, requested);
    let mut prepared = prepare(found, mode);
    let file_card = found
        .map(|r| r.card_id.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(requested);
    if prepared.ok {
        if let Err(reason) = open_prepared(app, file_card, &setting, &prepared) {
            prepared.ok = false;
            prepared.reason = Some(reason);
            prepared.body = None;
        }
    }
    let event_card = if file_card.is_empty() {
        requested
    } else {
        file_card
    };
    let planet = found
        .map(|r| r.planet_id.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    log_jump(app, event_card, planet.as_deref(), &prepared);
    to_value(&prepared)
}

/// Core branch of `client::focus`. No app handle on that path, so the
/// terminal app is the default (empty setting) and no event is written.
pub(crate) fn focus_core(h: &AgentRunHerdr) {
    let card = terminal_id(h);
    let run = AgentRun {
        card_id: card.clone(),
        state: "working".into(),
        herdr: Some(h.clone()),
        ..AgentRun::default()
    };
    let prepared = prepare(Some(&run), Mode::Observe);
    if !prepared.ok {
        return;
    }
    let Some(dir) = focus_dir() else {
        return;
    };
    let Some(body) = prepared.body.as_deref() else {
        return;
    };
    let Ok(path) = write_command(&dir, &card, body) else {
        return;
    };
    let app_name =
        crate::coordinator::resolve_agent_terminal("", crate::coordinator::ghostty_installed());
    let _ = reveal(app_name, &path, Some(card.as_str()));
}
