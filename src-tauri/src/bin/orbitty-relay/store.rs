//! Append-only jsonl in the relay datadir. Not events.jsonl (app SoT).

use serde_json::Value;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::nip;

pub struct Store {
    path: PathBuf,
    by_id: HashMap<String, Value>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Mutex<Store>, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        let path = dir.join("events.jsonl");
        let mut by_id = HashMap::new();
        if path.exists() {
            let f = File::open(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            for line in std::io::BufReader::new(f).lines() {
                let Ok(line) = line else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = v.get("id").and_then(Value::as_str) {
                    by_id.insert(id.to_string(), v);
                }
            }
        }
        Ok(Mutex::new(Store { path, by_id }))
    }

    pub fn insert(&mut self, ev: Value) -> Result<bool, String> {
        let id = ev
            .get("id")
            .and_then(Value::as_str)
            .ok_or("id")?
            .to_string();
        if self.by_id.contains_key(&id) {
            return Ok(false);
        }
        let mut opts = OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let f = opts
            .open(&self.path)
            .map_err(|e| format!("append: {e}"))?;
        let mut w = BufWriter::new(f);
        serde_json::to_writer(&mut w, &ev).map_err(|e| format!("serialize: {e}"))?;
        w.write_all(b"\n").map_err(|e| format!("append: {e}"))?;
        w.flush().map_err(|e| format!("append: {e}"))?;
        self.by_id.insert(id, ev);
        Ok(true)
    }

    pub fn query(&self, filters: &[Value]) -> Vec<Value> {
        let mut out = Vec::new();
        for ev in self.by_id.values() {
            if filters.iter().any(|f| nip::event_matches(ev, f)) {
                out.push(ev.clone());
            }
        }
        out.sort_by_key(|e| e.get("created_at").and_then(Value::as_u64).unwrap_or(0));
        if let Some(limit) = filters
            .iter()
            .filter_map(|f| f.get("limit").and_then(Value::as_u64))
            .min()
        {
            let n = limit as usize;
            if out.len() > n {
                let skip = out.len() - n;
                out = out.split_off(skip);
            }
        }
        out
    }
}
