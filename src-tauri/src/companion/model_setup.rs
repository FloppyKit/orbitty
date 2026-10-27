//! Set up a model. Probe is loopback only: Ollama :11434 and LM Studio :1234.

use serde::Serialize;
use std::path::Path;

pub const OLLAMA_DOWNLOAD: &str = "https://ollama.com/download";

pub const LMSTUDIO_OFF_LINE: &str =
    "LM Studio is installed, but its server is off. In LM Studio choose Developer > Start Server, port 1234, then Look again.";

pub fn probe_urls() -> &'static [&'static str] {
    &[
        "http://127.0.0.1:11434/api/tags",
        "http://127.0.0.1:1234/v1/models",
    ]
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeHit {
    pub source: String,
    pub id: String,
    pub base: String,
}

pub fn probe_with(fetch: &mut dyn FnMut(&str) -> Option<String>) -> Vec<ProbeHit> {
    let mut hits = Vec::new();
    for url in probe_urls() {
        let Some(body) = fetch(url) else {
            continue;
        };
        hits.extend(parse_probe(url, &body));
    }
    hits
}

fn parse_probe(url: &str, body: &str) -> Vec<ProbeHit> {
    let (source, base) = if url.contains(":11434") {
        ("ollama", "http://127.0.0.1:11434")
    } else {
        ("lmstudio", "http://127.0.0.1:1234")
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return vec![ProbeHit {
            source: source.into(),
            id: source.into(),
            base: base.into(),
        }];
    };
    let names: Vec<String> = if source == "ollama" {
        v.get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        v.get("data")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    if names.is_empty() {
        return vec![ProbeHit {
            source: source.into(),
            id: source.into(),
            base: base.into(),
        }];
    }
    names
        .into_iter()
        .map(|id| ProbeHit {
            source: source.into(),
            id,
            base: base.into(),
        })
        .collect()
}

fn http_get_loopback(url: &str) -> Option<String> {
    if !probe_urls().contains(&url) {
        return None;
    }
    let rest = url.strip_prefix("http://127.0.0.1:")?;
    let (port, path) = rest.split_once('/')?;
    let port: u16 = port.parse().ok()?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut sock = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400)).ok()?;
    sock.set_read_timeout(Some(std::time::Duration::from_millis(800))).ok()?;
    sock.set_write_timeout(Some(std::time::Duration::from_millis(400))).ok()?;
    use std::io::{Read, Write};
    let req = format!("GET /{path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    sock.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    let _ = sock.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).to_string();
    if !text.contains("200") {
        return None;
    }
    Some(text.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

#[tauri::command]
pub fn alpha_build() -> bool {
    cfg!(feature = "alpha")
}

#[tauri::command]
pub fn model_setup_probe() -> Vec<ProbeHit> {
    probe_with(&mut http_get_loopback)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LmStudioStatus {
    pub installed: bool,
    pub serving: bool,
    pub line: String,
}

pub fn lmstudio_app_installed(home: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    exists(Path::new("/Applications/LM Studio.app"))
        || exists(&home.join("Applications/LM Studio.app"))
}

pub fn lmstudio_status(installed: bool, serving: bool) -> LmStudioStatus {
    let line = if installed && !serving {
        LMSTUDIO_OFF_LINE.to_string()
    } else {
        String::new()
    };
    LmStudioStatus {
        installed,
        serving,
        line,
    }
}

fn port_open(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok()
}

#[tauri::command]
pub fn model_setup_lmstudio() -> LmStudioStatus {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let installed = lmstudio_app_installed(&home, |p| p.is_dir());
    let serving = port_open(1234);
    lmstudio_status(installed, serving)
}

#[tauri::command]
pub fn model_setup_clis() -> Vec<String> {
    clis_present(|bin| crate::coordinator::find_on_path(bin).is_some())
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

pub fn clis_present(present: impl Fn(&str) -> bool) -> Vec<&'static str> {
    [("grok", "grok"), ("goose", "goose"), ("cursor", "cursor-agent"), ("hermes", "hermes")]
        .into_iter()
        .filter(|(_, bin)| present(bin))
        .map(|(lane, _)| lane)
        .collect()
}

/// Alpha ignores home files and env. Nightly empty lane stays grok.
pub fn status_ignoring_home(home: &Path, env_key: Option<&str>, lane: &str, free_mode: bool) -> String {
    let _ = (home, env_key);
    let resolved = crate::mouth_hop::lane_from_fields(lane, free_mode);
    if resolved == "none" {
        "No model".into()
    } else {
        resolved
    }
}

pub fn home_key_visible(home: &Path, env_key: Option<&str>) -> bool {
    #[cfg(feature = "alpha")]
    {
        let _ = (home, env_key);
        false
    }
    #[cfg(not(feature = "alpha"))]
    {
        env_key.is_some_and(|k| !k.trim().is_empty())
            || home
                .join(".agents/companions/orbitty-primary/openrouter.key")
                .is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lane_is_none_under_alpha() {
        let lane = crate::mouth_hop::lane_from_fields("", false);
        #[cfg(feature = "alpha")]
        assert_eq!(lane, "none");
        #[cfg(not(feature = "alpha"))]
        assert_eq!(lane, "grok");
        assert_eq!(crate::mouth_hop::lane_from_fields("", true), "free");
    }

    #[test]
    fn no_model_send_spawns_nothing() {
        let lane = crate::mouth_hop::lane_from_fields("", false);
        let mut spawns = 0;
        let mut http = 0;
        let out = crate::mouth_hop::dispatch_model_send(lane.as_str(), || spawns += 1, || http += 1);
        #[cfg(feature = "alpha")]
        {
            assert_eq!(out.as_deref(), Ok(crate::mouth_hop::NO_MODEL_LINE));
            assert_eq!((spawns, http), (0, 0));
        }
        #[cfg(not(feature = "alpha"))]
        {
            assert!(out.is_err());
            assert_eq!((spawns, http), (1, 1));
        }
    }

    #[test]
    fn alpha_fake_home_is_no_model() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!("orbitty-home-{n}"));
        let key_path = home.join(".agents/companions/orbitty-primary/openrouter.key");
        let settings = home.join(".config/orbitty/settings.json");
        std::fs::create_dir_all(key_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&key_path, "sk-or-v1-TESTfixturevalue\n").unwrap();
        std::fs::write(&settings, "{\"model\":\"grok\"}\n").unwrap();
        let status = status_ignoring_home(&home, Some("sk-or-v1-TESTfixturevalue"), "", false);
        #[cfg(feature = "alpha")]
        {
            assert_eq!(status, "No model");
            assert!(!home_key_visible(&home, Some("sk-or-v1-TESTfixturevalue")));
        }
        #[cfg(not(feature = "alpha"))]
        {
            assert_eq!(status, "grok");
            assert!(home_key_visible(&home, Some("sk-or-v1-TESTfixturevalue")));
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_targets_are_loopback_pair() {
        let urls = probe_urls();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "http://127.0.0.1:11434/api/tags");
        assert_eq!(urls[1], "http://127.0.0.1:1234/v1/models");
        assert!(urls.iter().all(|u| !u.contains("8080")));
        let mut seen = Vec::new();
        let hits = probe_with(&mut |url| {
            seen.push(url.to_string());
            if url.contains("11434") {
                Some(r#"{"models":[{"name":"llama3"}]}"#.into())
            } else {
                None
            }
        });
        assert_eq!(seen, urls.iter().map(|s| (*s).to_string()).collect::<Vec<_>>());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "llama3");
    }

    #[test]
    fn ollama_download_link() {
        assert_eq!(OLLAMA_DOWNLOAD, "https://ollama.com/download");
    }

    #[test]
    fn lmstudio_installed_server_off_says_how_to_start() {
        let home = Path::new("/tmp/orbitty-lm-home");
        assert!(lmstudio_app_installed(home, |p| p == Path::new("/Applications/LM Studio.app")));
        assert!(!lmstudio_app_installed(home, |_| false));
        let off = lmstudio_status(true, false);
        assert!(off.installed && !off.serving);
        assert_eq!(off.line, LMSTUDIO_OFF_LINE);
        assert!(off.line.contains("Developer > Start Server"));
        assert!(off.line.contains("1234"));
        assert!(off.line.contains("Look again"));
        assert!(lmstudio_status(true, true).line.is_empty());
        assert!(lmstudio_status(false, false).line.is_empty());
    }

    #[test]
    fn plans_dir_creates_nothing_under_alpha() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!("orbitty-plans-{n}"));
        std::fs::create_dir_all(&home).unwrap();
        let roots = crate::plans::Roots { home: home.clone() };
        let p = crate::plans::plans_dir(&roots);
        #[cfg(feature = "alpha")]
        assert!(!p.exists());
        #[cfg(not(feature = "alpha"))]
        assert!(p.is_dir());
        let _ = std::fs::remove_dir_all(&home);
    }
}
