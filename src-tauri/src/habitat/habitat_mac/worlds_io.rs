//! worlds.json load / save (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs.

use tauri::Manager;

use super::world::{migrate_world, WorldsFile};
use super::{app_handle, now_epoch, Habitat};

fn worlds_path() -> Option<std::path::PathBuf> {
    let app = app_handle()?;
    let dir = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("worlds.json"))
}

pub(crate) fn load_worlds(h: &mut Habitat) {
    if h.worlds_loaded {
        return;
    }
    h.worlds_loaded = true;
    if let Some(path) = worlds_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(file) = serde_json::from_str::<WorldsFile>(&text) {
                h.worlds = file.worlds;
            }
        }
    }
    // Pre-GROW saves: bool toggles → placements, first_seen stamped.
    let now = now_epoch();
    let mut dirty = false;
    for w in h.worlds.values_mut() {
        dirty |= migrate_world(w, now);
    }
    if dirty {
        save_worlds(h);
    }
}

pub(crate) fn save_worlds(h: &Habitat) {
    let Some(path) = worlds_path() else { return };
    let file = WorldsFile {
        worlds: h.worlds.clone(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}
