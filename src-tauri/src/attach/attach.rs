//! One file chip for the companion composer.
//! Original path only (never a staging copy): the chip carries the
//! path, `companion_send` re-validates and hands it to the mouth on the
//! prompt, and the agent's own tools read the file there. Cap 25 MB
//! with an honest error over. Orbitty does not index, embed, convert,
//! or keep a library — Spark is the front door. Office convert and
//! image vision live in attach_md / attach_vision (sidecars).

use serde::Serialize;

/// Locked cap: 25 MB. Over = honest error, chip stays.
pub(crate) const ATTACH_CAP_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AttachInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
}

pub(crate) fn human_size(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Validate a chip path: absolute, an existing plain file, within the
/// cap. Errors are the honest strings the composer note shows.
pub(crate) fn stat_path(path: &str) -> Result<AttachInfo, String> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("file path must be absolute".into());
    }
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let md = std::fs::metadata(p).map_err(|e| format!("can't reach {name}: {e}"))?;
    if !md.is_file() {
        return Err(format!("{name} is a folder — one file only"));
    }
    if md.len() > ATTACH_CAP_BYTES {
        return Err(format!(
            "{name} is {} — over the 25 MB cap",
            human_size(md.len())
        ));
    }
    Ok(AttachInfo {
        path: path.to_string(),
        name,
        size: md.len(),
    })
}

/// Drag-drop validation: the drop event hands a path, this stats it
/// into a chip (or the honest error the note shows).
#[tauri::command]
pub(crate) async fn companion_attach_stat(path: String) -> Result<AttachInfo, String> {
    stat_path(path.trim())
}

/// Paperclip: native one-file panel on the main thread, then the same
/// validation a drop gets. Cancel = Ok(None), never an error.
#[tauri::command]
pub(crate) async fn companion_attach_pick() -> Result<Option<AttachInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        let picked = tauri::async_runtime::spawn_blocking(|| {
            let mut out: Option<String> = None;
            let slot = &mut out;
            crate::main_thread_sync(move || {
                *slot = mac::pick_file();
            });
            out
        })
        .await
        .map_err(|e| format!("pick task: {e}"))?;
        match picked {
            Some(p) => stat_path(&p).map(Some),
            None => Ok(None),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("paperclip picker is macOS-only in v0 — drag a file onto the composer instead".into())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    extern "C" {
        fn orbitty_pick_file() -> *mut c_char;
        fn orbitty_pick_file_free(s: *mut c_char);
    }

    pub(super) fn pick_file() -> Option<String> {
        unsafe {
            let p = orbitty_pick_file();
            if p.is_null() {
                return None;
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            orbitty_pick_file_free(p);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}
