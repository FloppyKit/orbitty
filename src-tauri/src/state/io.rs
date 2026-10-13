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

/// Disk write. Private: every caller goes through [`update`] so a stale
/// snapshot cannot land between another writer's load and save.
pub(crate) fn save_state(app: &tauri::AppHandle, state: &PersistedState) -> Result<(), String> {
    let path = state_path(app)?;
    save_path(&path, state)
}

pub(crate) fn save_path(path: &std::path::Path, state: &PersistedState) -> Result<(), String> {
    // Every writer hits this door. Canonicalize links here so a board
    // window's whole-state save keeps them and a hostile scheme never lands.
    let mut state = state.clone();
    crate::card_links::canonicalize_state_links(&mut state)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
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
}

pub(crate) struct UpdateFlag;
impl Drop for UpdateFlag {
    fn drop(&mut self) {
        IN_UPDATE.with(|c| c.set(false));
    }
}

/// The only write helper. Holds the process-wide lock, then the
/// `state.json.lock` file lock, across load, `f`, and save. `f` must not
/// call `update` again (the lock is not reentrant). An unchanged state
/// skips the disk write.
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
    let _file = lock_state_file(&state_path(app)?)?;
    let mut st = load_state(app)?;
    let before = serde_json::to_string(&st).ok();
    let out = f(&mut st)?;
    let after = serde_json::to_string(&st).ok();
    if before != after {
        save_state(app, &st)?;
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) fn update_at<T>(
    path: &std::path::Path,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    let _file = lock_state_file(path)?;
    let mut st = load_path(path)?;
    let before = serde_json::to_string(&st).ok();
    let out = f(&mut st)?;
    let after = serde_json::to_string(&st).ok();
    if before != after {
        save_path(path, &st)?;
    }
    Ok(out)
}
