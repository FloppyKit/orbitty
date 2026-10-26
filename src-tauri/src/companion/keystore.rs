//! Alpha API keys. macOS Keychain via `security -i` (secret on stdin, never argv).
//! Tests use [`MemoryStore`]. Nothing here writes `openrouter.key` or state.json.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

pub const ACCOUNT: &str = "orbitty";

pub fn service_name(bundle_id: &str, lane: &str) -> String {
    format!("{bundle_id}.{lane}")
}

pub fn security_args() -> &'static [&'static str] {
    &["-i"]
}

fn valid_token(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

pub fn validate_key(key: &str) -> Result<&str, String> {
    let key = key.trim();
    if key.is_empty() {
        return Ok(key);
    }
    if !valid_token(key) {
        return Err("That doesn't look like a key — no spaces or newlines.".into());
    }
    Ok(key)
}

pub trait SecretStore {
    fn put(&mut self, service: &str, key: &str) -> Result<(), String>;
    fn get(&self, service: &str) -> Option<String>;
    fn clear(&mut self, service: &str) -> Result<(), String>;
}

#[derive(Default)]
pub struct MemoryStore {
    items: HashMap<String, String>,
}

impl SecretStore for MemoryStore {
    fn put(&mut self, service: &str, key: &str) -> Result<(), String> {
        self.items.insert(service.to_string(), key.to_string());
        Ok(())
    }
    fn get(&self, service: &str) -> Option<String> {
        self.items.get(service).cloned()
    }
    fn clear(&mut self, service: &str) -> Result<(), String> {
        self.items.remove(service);
        Ok(())
    }
}

/// Writes the secret only through `store`. `root` is never created or written.
pub fn save_through(
    store: &mut dyn SecretStore,
    root: &Path,
    bundle: &str,
    lane: &str,
    key: &str,
) -> Result<(), String> {
    let _ = root;
    if !valid_token(bundle) || !valid_token(lane) {
        return Err("bad service name".into());
    }
    let service = service_name(bundle, lane);
    let key = validate_key(key)?;
    if key.is_empty() {
        return store.clear(&service);
    }
    store.put(&service, key)
}

pub fn running_bundle_id() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe.to_string_lossy();
    let target = match path.find(".app/") {
        Some(i) => path[..i + 4].to_string(),
        None => path.to_string(),
    };
    let out = Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=2", &target])
        .output()
        .map_err(|e| format!("codesign: {e}"))?;
    let blob = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    for line in blob.lines() {
        if let Some(id) = line.trim().strip_prefix("Identifier=") {
            let id = id.trim();
            if valid_token(id) {
                return Ok(id.to_string());
            }
        }
    }
    Err("bundle id unavailable".into())
}

fn security_stdin(script: &str) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/security")
        .args(security_args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("security: {e}"))?;
    if let Some(mut sin) = child.stdin.take() {
        use std::io::Write;
        sin.write_all(script.as_bytes())
            .map_err(|e| format!("security stdin: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("security: {e}"))?;
    if !out.status.success() {
        return Err("security failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

struct SecurityStore;

impl SecretStore for SecurityStore {
    fn put(&mut self, service: &str, key: &str) -> Result<(), String> {
        let script = format!(
            "add-generic-password -U -a {ACCOUNT} -s {service} -w {key}\n"
        );
        security_stdin(&script).map(|_| ())
    }
    fn get(&self, service: &str) -> Option<String> {
        let script = format!("find-generic-password -a {ACCOUNT} -s {service} -w\n");
        security_stdin(&script)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
    fn clear(&mut self, service: &str) -> Result<(), String> {
        let script = format!("delete-generic-password -a {ACCOUNT} -s {service}\n");
        match security_stdin(&script) {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

fn with_security<T>(f: impl FnOnce(&mut SecurityStore) -> T) -> T {
    f(&mut SecurityStore)
}

pub fn put(lane: &str, key: &str) -> Result<(), String> {
    let bundle = running_bundle_id()?;
    let mut store = SecurityStore;
    save_through(&mut store, std::path::Path::new(""), &bundle, lane, key)
}

pub fn get(lane: &str) -> Option<String> {
    let bundle = running_bundle_id().ok()?;
    with_security(|s| s.get(&service_name(&bundle, lane)))
}

pub fn status(lane: &str) -> (bool, String) {
    match get(lane) {
        Some(k) => {
            let tail: String = k.chars().rev().take(4).collect::<String>().chars().rev().collect();
            (true, tail)
        }
        None => (false, String::new()),
    }
}

pub fn stored_secrets() -> Vec<String> {
    ["openrouter", "compat"]
        .into_iter()
        .filter_map(get)
        .collect()
}

pub fn dir_has_secret(root: &Path, secret: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if std::fs::read_to_string(&path)
                .ok()
                .is_some_and(|t| t.contains(secret))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_service_uses_bundle_id() {
        assert_eq!(
            service_name("space.orbitty.alpha", "openrouter"),
            "space.orbitty.alpha.openrouter"
        );
        assert_eq!(security_args(), &["-i"]);
        let key = "sk-or-v1-TESTfixturevalue";
        assert!(security_args().iter().all(|a| !a.contains(key)));
    }

    #[test]
    fn save_key_writes_no_file() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("orbitty-ks-{n}"));
        let app_data = root.join("app_data");
        let companions = root.join(".agents/companions/orbitty-primary");
        std::fs::create_dir_all(&app_data).unwrap();
        std::fs::create_dir_all(&companions).unwrap();
        let key = "sk-or-v1-TESTfixturevalue";
        let mut mem = MemoryStore::default();
        save_through(&mut mem, &root, "space.orbitty.alpha", "openrouter", key).unwrap();
        assert_eq!(
            mem.get("space.orbitty.alpha.openrouter").as_deref(),
            Some(key)
        );
        assert!(!dir_has_secret(&root, key));
        assert!(!companions.join("openrouter.key").exists());
        save_through(&mut mem, &root, "space.orbitty.alpha", "openrouter", "").unwrap();
        assert!(mem.get("space.orbitty.alpha.openrouter").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
