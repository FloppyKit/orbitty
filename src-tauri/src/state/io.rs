use std::path::PathBuf;
use super::{ PersistedState };

pub(crate) fn state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("state.json"))
}

pub fn load_state(app: &tauri::AppHandle) -> Result<PersistedState, String> {
    let path = state_path(app)?;
    load_path(&path)
}

pub(crate) fn load_path(path: &std::path::Path) -> Result<PersistedState, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut st: PersistedState = serde_json::from_str(&text)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            // Old saves heal here — mode defaults
            // spark, a pre-split transcript parks under spark.
            st.companion.heal_modes();
            st.companion.heal_retired_body();
            Ok(st)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Disk write. Every caller goes through [`update`] so a stale snapshot
/// cannot land between another writer's load and save.
pub(crate) fn save_path(path: &std::path::Path, state: &PersistedState) -> Result<(), String> {
    write_canonical(path, state.clone()).map(|_| ())
}

/// Returns the state exactly as written, which is what history logs.
fn write_canonical(
    path: &std::path::Path,
    mut state: PersistedState,
) -> Result<PersistedState, String> {
    // Every writer hits this door. Canonicalize links here so a board
    // window's whole-state save keeps them and a hostile scheme never lands.
    crate::card_links::canonicalize_state_links(&mut state)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(state)
}

pub(crate) fn write_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A panicked writer must not freeze every later save.
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// `<state file>.lock` — `state.json.lock` beside `state.json`.
pub(crate) fn lock_path(path: &std::path::Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// OS file lock on `<path>.lock`, held until drop. The in-process mutex
/// cannot stop another process; this does. Always taken after `write_lock`.
pub(crate) struct StateFileLock(std::fs::File);

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(crate) fn lock_state_file(path: &std::path::Path) -> Result<StateFileLock, String> {
    let lock = lock_path(path);
    if let Some(dir) = lock.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let f = opts
        .open(&lock)
        .map_err(|e| format!("open {}: {e}", lock.display()))?;
    f.lock().map_err(|e| format!("lock {}: {e}", lock.display()))?;
    Ok(StateFileLock(f))
}

std::thread_local! {
    static IN_UPDATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ACTOR: std::cell::Cell<&'static str> = const { std::cell::Cell::new("app") };
}

pub(crate) struct UpdateFlag;
impl Drop for UpdateFlag {
    fn drop(&mut self) {
        IN_UPDATE.with(|c| c.set(false));
    }
}

/// Saves made by `f` on this thread log `actor` in history. Nests: the
/// outer actor is back when `f` returns or panics. Default `app`.
pub fn with_actor<T>(actor: &'static str, f: impl FnOnce() -> T) -> T {
    let _scope = actor_scope(actor);
    f()
}

/// `with_actor` for a whole function body: the actor holds until drop.
#[must_use]
pub(crate) struct ActorScope(&'static str);
impl Drop for ActorScope {
    fn drop(&mut self) {
        ACTOR.with(|a| a.set(self.0));
    }
}

pub(crate) fn actor_scope(actor: &'static str) -> ActorScope {
    ActorScope(ACTOR.with(|a| a.replace(actor)))
}

pub(crate) fn current_actor() -> &'static str {
    ACTOR.with(|a| a.get())
}

/// The only write helper. Holds the process-wide lock, then the
/// `state.json.lock` file lock, across load, `f`, save and the history
/// lines. `f` must not call `update` again (the lock is not reentrant).
/// An unchanged state skips the disk write. A history failure never fails
/// the save.
pub fn update<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    if IN_UPDATE.with(|c| c.get()) {
        return Err("state update re-entered".into());
    }
    let _guard = write_lock();
    IN_UPDATE.with(|c| c.set(true));
    let _flag = UpdateFlag;
    let path = state_path(app)?;
    let _file = lock_state_file(&path)?;
    let (out, saved) = change_and_save(&path, f)?;
    if let Some((before, after)) = saved {
        crate::relay::history::record_best(app, &before, &after);
    }
    Ok(out)
}

/// Load, change, save. `Some` carries the board before `f` and the state
/// as written, only when the save happened.
fn change_and_save<T>(
    path: &std::path::Path,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<(T, Option<(crate::relay::history::Snapshot, PersistedState)>), String> {
    let mut st = load_path(path)?;
    let before = serde_json::to_string(&st).ok();
    let board = crate::relay::history::Snapshot::of(&st);
    let out = f(&mut st)?;
    let after = serde_json::to_string(&st).ok();
    if before == after {
        return Ok((out, None));
    }
    let written = write_canonical(path, st)?;
    Ok((out, Some((board, written))))
}

/// Reads `state.json` under the same two locks as [`update`], so `f` sees
/// a whole save and no save lands while it runs.
pub(crate) fn read_locked<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&std::path::Path, &PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    if IN_UPDATE.with(|c| c.get()) {
        return Err("state read re-entered an update".into());
    }
    let _guard = write_lock();
    IN_UPDATE.with(|c| c.set(true));
    let _flag = UpdateFlag;
    let path = state_path(app)?;
    let _file = lock_state_file(&path)?;
    let st = load_path(&path)?;
    f(&path, &st)
}

#[cfg(test)]
pub(crate) fn update_at<T>(
    path: &std::path::Path,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    let _file = lock_state_file(path)?;
    change_and_save(path, f).map(|(out, _)| out)
}

/// [`update_at`] plus the history lines `update` writes, unsigned, into
/// `events`.
#[cfg(test)]
pub(crate) fn update_at_logged<T>(
    path: &std::path::Path,
    events: &std::path::Path,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    update_at_signed(path, events, &crate::relay::history::unsigned, f)
}

#[cfg(test)]
pub(crate) fn update_at_signed<T>(
    path: &std::path::Path,
    events: &std::path::Path,
    ident: crate::relay::history::Ident,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    let _file = lock_state_file(path)?;
    let (out, saved) = change_and_save(path, f)?;
    if let Some((before, after)) = saved {
        crate::relay::history::record_at(events, &before, &after, ident);
    }
    Ok(out)
}
