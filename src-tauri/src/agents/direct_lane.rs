//! Direct lane. One pure `send_route` puts each line in LOCAL, DIRECT,
//! MOUTH, NoHarness, or Unknown. `direct` is a picker id: local, then an
//! OpenAI-compatible base URL + key, then OpenRouter. No CLI spawn.
//! The compat key is a 0600 file, never state.json, never ctl/MCP.

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteInput {
    pub local_hit: bool,
    pub attach: bool,
    pub resolve: crate::companion_backend::BackendResolve,
    pub reply_lane: String,
}

/// Ordered lanes for a Direct reply lane, before availability filters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectPlan {
    pub lanes: Vec<&'static str>,
}

impl DirectPlan {
    pub fn for_lane(reply_lane: &str) -> Self {
        Self {
            lanes: wanted_lanes(reply_lane),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendRoute {
    Local,
    NoHarness,
    Unknown(String),
    Direct(DirectPlan),
    Mouth(String),
}

impl SendRoute {
    pub fn log_name(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::NoHarness => "no_harness",
            Self::Unknown(_) => "unknown",
            Self::Direct(_) => "direct",
            Self::Mouth(_) => "mouth",
        }
    }
}

fn wanted_lanes(reply_lane: &str) -> Vec<&'static str> {
    match crate::mouth_hop::normalize_reply_lane(reply_lane).as_str() {
        "local" => vec!["local"],
        "openrouter" => vec!["openrouter"],
        "compat" => vec!["compat"],
        // free, grok, and empty (normalize turns empty into grok).
        _ => vec!["local", "compat", "openrouter"],
    }
}

/// Reply lane × what is actually up. An empty vec is the honest down line.
pub fn direct_plan(
    reply_lane: &str,
    local_up: bool,
    compat_configured: bool,
    openrouter_key: bool,
) -> Vec<&'static str> {
    wanted_lanes(reply_lane)
        .into_iter()
        .filter(|lane| match *lane {
            "local" => local_up,
            "compat" => compat_configured,
            "openrouter" => openrouter_key,
            _ => false,
        })
        .collect()
}

pub fn direct_down_notes(
    reply_lane: &str,
    local_up: bool,
    compat_configured: bool,
    openrouter_key: bool,
) -> Vec<String> {
    let ready = direct_plan(reply_lane, local_up, compat_configured, openrouter_key);
    let mut notes = Vec::new();
    for lane in wanted_lanes(reply_lane) {
        if ready.iter().any(|r| *r == lane) {
            continue;
        }
        notes.push(match lane {
            "local" => "local: none found — open Ollama or LM Studio.".into(),
            "compat" => "compat: not configured".into(),
            "openrouter" => "openrouter: no key".into(),
            other => format!("{other}: down"),
        });
    }
    if notes.is_empty() {
        notes.push("no lane configured".into());
    }
    notes
}

/// Pure. A local hit with no attachment wins over NoHarness and Unknown.
/// An attachment never routes Local. `direct` is Direct; every other
/// sendable id is Mouth. The resolver is not copied here.
pub fn send_route(input: RouteInput) -> SendRoute {
    if input.local_hit && !input.attach {
        return SendRoute::Local;
    }
    let r = &input.resolve;
    if r.unknown {
        return SendRoute::Unknown(crate::companion_backend::unknown_backend_message(
            &r.pick.id,
            r.pick.source,
        ));
    }
    if r.pick.id.is_empty() {
        return SendRoute::NoHarness;
    }
    if r.pick.id == "direct" {
        return SendRoute::Direct(DirectPlan::for_lane(&input.reply_lane));
    }
    SendRoute::Mouth(r.pick.id.clone())
}

/// http(s) only. Plain http is allowed only for a loopback host.
/// Empty clears the saved base. The stored form has a lowercased scheme
/// and no trailing slash.
pub fn validate_direct_base(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    if s.chars().any(char::is_whitespace) {
        return Err("Base URL must be http or https.".into());
    }
    let Some((scheme_raw, rest)) = s.split_once("://") else {
        return Err("Base URL must be http or https.".into());
    };
    let scheme = scheme_raw.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("Base URL must be http or https.".into());
    }
    if rest.is_empty() {
        return Err("Base URL must be http or https.".into());
    }
    let host = host_of(rest)?;
    if scheme == "http" && !is_loopback(&host) {
        return Err("http is only allowed for localhost.".into());
    }
    let rest = rest.trim_end_matches('/');
    Ok(format!("{scheme}://{rest}"))
}

/// The host of an http(s) authority, strictly. Userinfo (`@`, raw or
/// `%40`), backslashes, any `%` escape, whitespace / control chars and an
/// empty host are refused outright, so `http://127.0.0.1:1@evil.com` can
/// never pass as loopback. IPv6 only as a bracketed literal followed by
/// nothing but `:port`. The port is digits only. Errors are fixed text and
/// never echo the URL (or anything typed into it).
fn host_of(rest: &str) -> Result<String, String> {
    let bad = || "Base URL must be http or https.".to_string();
    if rest.contains('\\') || rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad());
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(bad());
    }
    if authority.contains('@') {
        return Err("Base URL must not carry a user or key before the host.".into());
    }
    if authority.contains('%') {
        return Err(bad());
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (h, after) = inner.split_once(']').ok_or_else(bad)?;
        if h.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(bad());
        }
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':').ok_or_else(bad)?),
        };
        (h, port)
    } else {
        let (h, port) = match authority.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        };
        if h.contains(['[', ']']) {
            return Err(bad());
        }
        (h, port)
    };
    if host.is_empty() {
        return Err(bad());
    }
    if let Some(p) = port {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) || p.parse::<u16>().is_err() {
            return Err(bad());
        }
    }
    Ok(host.to_string())
}

fn is_loopback(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "localhost"
        || h.parse::<std::net::Ipv6Addr>() == Ok(std::net::Ipv6Addr::LOCALHOST)
        || is_ipv4_loopback(&h)
}

fn is_ipv4_loopback(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 || parts[0] != "127" {
        return false;
    }
    parts[1..].iter().all(|p| {
        !p.is_empty()
            && !p.starts_with('+')
            && p.chars().all(|c| c.is_ascii_digit())
            && p.parse::<u8>().is_ok()
    })
}

pub fn compat_configured(base: &str, model: &str, key_present: bool) -> bool {
    key_present
        && !model.trim().is_empty()
        && validate_direct_base(base)
            .ok()
            .is_some_and(|b| !b.is_empty())
}

pub fn direct_key_path() -> std::path::PathBuf {
    crate::coordinator::companions_root().join("direct.key")
}

/// Write or clear a secret file. Tests pass a temp path. Mode 0600.
pub fn write_secret_file(path: &Path, key: &str) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        }
        return Ok(());
    }
    if key.chars().any(char::is_whitespace) {
        return Err("That doesn't look like a key — no spaces or newlines.".into());
    }
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
    }
    std::fs::write(path, format!("{key}\n")).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("mode {}: {e}", path.display()))?;
    }
    Ok(())
}

pub fn read_secret_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectKeyStatus {
    pub set: bool,
    pub tail: String,
}

pub fn key_status_at(path: &Path) -> DirectKeyStatus {
    match read_secret_file(path) {
        Some(k) => DirectKeyStatus {
            set: true,
            tail: k
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect(),
        },
        None => DirectKeyStatus {
            set: false,
            tail: String::new(),
        },
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectEndpoint {
    pub base: String,
    pub model: String,
}

#[tauri::command]
pub fn direct_key_status() -> DirectKeyStatus {
    key_status_at(&direct_key_path())
}

#[tauri::command]
pub fn set_direct_key(key: String) -> Result<DirectKeyStatus, String> {
    write_secret_file(&direct_key_path(), &key)?;
    Ok(key_status_at(&direct_key_path()))
}

#[tauri::command]
pub fn set_direct_endpoint(app: AppHandle, base: String, model: String) -> Result<DirectEndpoint, String> {
    let base = validate_direct_base(&base)?;
    let model = model.trim().chars().take(160).collect::<String>();
    if model.chars().any(char::is_control) {
        return Err("Model id can't include a newline.".into());
    }
    crate::state::update(&app, |st| {
        st.companion.direct_base_url = base.clone();
        st.companion.direct_model = model.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(DirectEndpoint { base, model })
}

/// Text-only compat lane (no model-list probe). Other lanes keep the
/// hop vision bit `image_lane_vision` already computed.
pub fn direct_vision(reply_lane: &str, hop_lane: &str, hop_vision: bool) -> (String, bool) {
    if crate::mouth_hop::normalize_reply_lane(reply_lane) == "compat" {
        ("compat".into(), false)
    } else {
        (hop_lane.to_string(), hop_vision)
    }
}

pub fn would_answer_locally(line: &str, shortcuts: &[crate::state::ActionShortcut]) -> bool {
    crate::companion_todo::parse_todo_line(line).is_some()
        || crate::companion_go::parse_go_line(line).is_some()
        || crate::mouth_route::classify(line).is_some()
        || crate::coordinator::action_line_hits(shortcuts, line)
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoutePreview {
    pub route: String,
    pub backend: String,
    pub source: String,
    pub lanes: Vec<String>,
}

/// Dry run. Does not call a model, does not write, does not probe a server.
pub fn preview_route(
    resolve: &crate::companion_backend::BackendResolve,
    line: &str,
    reply_lane: &str,
    shortcuts: &[crate::state::ActionShortcut],
    attach: bool,
) -> RoutePreview {
    let local_hit = !attach && would_answer_locally(line, shortcuts);
    let route = send_route(RouteInput {
        local_hit,
        attach,
        resolve: resolve.clone(),
        reply_lane: reply_lane.to_string(),
    });
    let lanes = match &route {
        SendRoute::Direct(plan) => plan.lanes.iter().map(|s| (*s).to_string()).collect(),
        SendRoute::Mouth(id) => vec![id.clone()],
        _ => Vec::new(),
    };
    RoutePreview {
        route: route.log_name().to_string(),
        backend: resolve.pick.id.clone(),
        source: resolve.pick.source.as_str().to_string(),
        lanes,
    }
}

pub fn companion_route(app: AppHandle, line: String) -> Result<RoutePreview, String> {
    let resolve = crate::companion_backend::resolve_companion_backend(&app);
    let reply_lane = crate::mouth_hop::effective_reply_lane(&app);
    let shortcuts = crate::state::load_state(&app)
        .map(|s| s.action_shortcuts)
        .unwrap_or_default();
    Ok(preview_route(&resolve, &line, &reply_lane, &shortcuts, false))
}

/// Gist pack from the grok router, then the free lanes plus compat.
/// Never the Grok CLI.
pub async fn send(
    app: &tauri::AppHandle,
    raw_line: &str,
    prose: &str,
    hop_ack: &str,
    vision_path: Option<&str>,
) -> Result<String, String> {
    let settings = crate::mouth_hop::effective_reply_lane(app);
    let has_attach = vision_path.is_some();
    let want = crate::state::load_state(app)
        .map(|s| s.companion.reply_model)
        .unwrap_or_default();
    let local_up = crate::mouth_scan::pick_local(&want).is_some();
    let (base, model) = crate::state::load_state(app)
        .map(|s| (s.companion.direct_base_url, s.companion.direct_model))
        .unwrap_or_default();
    let key_present = read_secret_file(&direct_key_path()).is_some();
    let compat_ok = compat_configured(&base, &model, key_present);
    let or_key = crate::mouth_hop::openrouter_configured();
    let lanes = direct_plan(&settings, local_up, compat_ok, or_key);
    if lanes.is_empty() {
        let notes = direct_down_notes(&settings, local_up, compat_ok, or_key);
        return Ok(crate::mouth_hop::honest_down_line(false, &notes));
    }
    let short = crate::mouth_gist::is_short_chat(raw_line, has_attach);
    let guessed = crate::mouth_hop::hop_lane_for(&settings, raw_line, has_attach, local_up);
    let hop_lane = if guessed == "grok" || settings == "compat" {
        lanes[0]
    } else {
        guessed
    };
    let (oneb, hp) = crate::mouth_sidecar::last_harness()
        .map(|(h, p)| (Some(h), p))
        .unwrap_or((None, 0.0));
    let file = crate::mouth_gist::file_note(prose);
    let harness = if file.is_some() {
        crate::mouth_gist::Harness::Pocket
    } else {
        crate::mouth_gist::pick_harness(app, raw_line, "chat", oneb, hp, short)
    };
    let image_on = vision_path.is_some();
    let gist = match harness {
        crate::mouth_gist::Harness::Skip => None,
        crate::mouth_gist::Harness::Job => {
            crate::mouth_gist::build(app, raw_line, "chat", hop_ack, hop_lane, false, file)
        }
        crate::mouth_gist::Harness::Tight => {
            crate::mouth_gist::build(app, raw_line, "chat", hop_ack, hop_lane, !image_on, file)
        }
        crate::mouth_gist::Harness::Pocket => None,
    };
    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode)
        .unwrap_or_else(|| "spark".into());
    let packed = crate::mouth_gist::pack_prompt(&mode, harness, raw_line, prose, gist.as_deref());
    crate::event_log::append_best(
        app,
        "companion-gist",
        None,
        None,
        Some("agent"),
        true,
        &serde_json::json!({
            "lane": hop_lane,
            "short": short,
            "gist": crate::mouth_gist::is_gist(&packed),
            "harness": harness.as_str(),
            "n": packed.chars().count(),
            "direct": true,
        }),
    );
    let try_local = lanes.iter().any(|l| *l == "local");
    let try_compat = lanes.iter().any(|l| *l == "compat");
    let try_or = lanes.iter().any(|l| *l == "openrouter");
    let vision = vision_path.map(str::to_string);
    let (a, r, p) = (app.clone(), raw_line.to_string(), packed);
    tauri::async_runtime::spawn_blocking(move || {
        crate::mouth_hop::run_free_lanes(
            a, r, p, true, true, try_local, try_compat, try_or, Vec::new(), vision,
        )
    })
    .await
    .map_err(|e| format!("hop task: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion_backend::{
        resolve_backend, unknown_backend_message, BackendPick, BackendResolve, BackendSource,
        Detected,
    };

    fn resolved(id: &str, source: BackendSource, unknown: bool) -> BackendResolve {
        BackendResolve {
            pick: BackendPick {
                id: id.into(),
                source,
            },
            unknown,
        }
    }

    fn expect_route(local_hit: bool, attach: bool, label: &str, lane: &str) -> SendRoute {
        if local_hit && !attach {
            return SendRoute::Local;
        }
        match label {
            "env grok" => SendRoute::Mouth("grok".into()),
            "state direct" => SendRoute::Direct(DirectPlan::for_lane(lane)),
            "state goose" => SendRoute::Mouth("goose".into()),
            "state hermes" => SendRoute::Mouth("hermes".into()),
            "env berd" => SendRoute::Mouth("berd".into()),
            "unset" => SendRoute::NoHarness,
            "unknown env" => SendRoute::Unknown(unknown_backend_message("nope", BackendSource::Env)),
            "unknown state" => {
                SendRoute::Unknown(unknown_backend_message("nope", BackendSource::State))
            }
            other => panic!("bad case {other}"),
        }
    }

    #[test]
    fn send_route_table() {
        let cases = [
            ("env grok", resolved("grok", BackendSource::Env, false)),
            ("state direct", resolved("direct", BackendSource::State, false)),
            ("state goose", resolved("goose", BackendSource::State, false)),
            ("state hermes", resolved("hermes", BackendSource::State, false)),
            ("env berd", resolved("berd", BackendSource::Env, false)),
            ("unset", resolved("", BackendSource::Unset, false)),
            ("unknown env", resolved("nope", BackendSource::Env, true)),
            ("unknown state", resolved("nope", BackendSource::State, true)),
        ];
        let lanes = ["local", "openrouter", "compat", "free", "grok", ""];
        for local_hit in [false, true] {
            for attach in [false, true] {
                for (label, resolve) in &cases {
                    for lane in lanes {
                        let got = send_route(RouteInput {
                            local_hit,
                            attach,
                            resolve: resolve.clone(),
                            reply_lane: (*lane).to_string(),
                        });
                        let expect = expect_route(local_hit, attach, label, lane);
                        assert_eq!(got, expect, "{label} hit={local_hit} attach={attach} lane={lane}");
                        if attach {
                            assert!(!matches!(got, SendRoute::Local));
                        }
                    }
                }
            }
        }
        let direct = send_route(RouteInput {
            local_hit: false,
            attach: false,
            resolve: resolved("direct", BackendSource::State, false),
            reply_lane: "free".into(),
        });
        match direct {
            SendRoute::Direct(plan) => {
                assert_eq!(plan.lanes, ["local", "compat", "openrouter"]);
            }
            other => panic!("{other:?}"),
        }
        let grok = send_route(RouteInput {
            local_hit: false,
            attach: true,
            resolve: resolved("grok", BackendSource::Env, false),
            reply_lane: "compat".into(),
        });
        assert_eq!(grok, SendRoute::Mouth("grok".into()));
    }

    #[test]
    fn direct_plan_table() {
        let rows = [
            ("local", true, true, true, vec!["local"]),
            ("local", false, true, true, vec![]),
            ("openrouter", true, true, true, vec!["openrouter"]),
            ("openrouter", false, false, false, vec![]),
            ("compat", false, true, false, vec!["compat"]),
            ("compat", true, false, true, vec![]),
            ("free", true, true, true, vec!["local", "compat", "openrouter"]),
            ("free", true, false, true, vec!["local", "openrouter"]),
            ("free", false, true, false, vec!["compat"]),
            ("free", false, false, true, vec!["openrouter"]),
            ("free", false, false, false, vec![]),
            ("grok", true, true, true, vec!["local", "compat", "openrouter"]),
            ("grok", false, false, false, vec![]),
            ("", true, true, false, vec!["local", "compat"]),
            ("paid", false, true, true, vec!["compat", "openrouter"]),
            ("lmstudio", true, true, true, vec!["local"]),
            ("Free Mode", false, true, true, vec!["compat", "openrouter"]),
        ];
        for (lane, up, compat, key, expect) in rows {
            assert_eq!(
                direct_plan(lane, up, compat, key),
                expect,
                "{lane} up={up} compat={compat} key={key}"
            );
        }
        let notes = direct_down_notes("local", false, false, false);
        let line = crate::mouth_hop::honest_down_line(false, &notes);
        assert!(line.contains("local"));
        assert!(!line.contains("Free Mode"));
        assert!(!line.contains("Paid Grok"));
        assert!(direct_plan("free", false, false, false).is_empty());
    }

    #[test]
    fn env_grok_arm_matches_de36149() {
        let states = [
            "", " ", "\t", "\n", "goose", "grok", "hermes", "berd", "direct", "openrouter",
            "local", "nope", " GOOSE ", "Hermes",
        ];
        let lanes = ["", "grok", "local", "openrouter", "free", "compat", "paid"];
        for goose in [false, true] {
            for grok_on in [false, true] {
                for hermes in [false, true] {
                    for state in states {
                        let resolved = resolve_backend(
                            "grok",
                            state,
                            Detected {
                                goose,
                                grok: grok_on,
                                hermes,
                            },
                        );
                        assert_eq!(resolved.pick.id, "grok");
                        assert_eq!(resolved.pick.source, BackendSource::Env);
                        assert!(!resolved.unknown);
                        for lane in lanes {
                            let route = send_route(RouteInput {
                                local_hit: false,
                                attach: false,
                                resolve: resolved.clone(),
                                reply_lane: (*lane).to_string(),
                            });
                            assert_eq!(
                                route,
                                SendRoute::Mouth("grok".into()),
                                "state={state:?} lane={lane}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_copy_names_direct() {
        assert_eq!(
            unknown_backend_message("nope", BackendSource::Env),
            "Unknown ORBITTY_COMPANION_BACKEND \"nope\" — goose, grok, hermes, berd, or direct."
        );
        assert_eq!(
            unknown_backend_message("nope", BackendSource::State),
            "Unknown harness \"nope\" — goose, grok, hermes, or direct."
        );
    }

    fn prod_direct() -> &'static str {
        include_str!("direct_lane.rs").split("mod tests").next().unwrap()
    }

    fn body_of<'a>(src: &'a str, sig: &str) -> &'a str {
        let at = src.find(sig).unwrap_or_else(|| panic!("missing {sig}"));
        let rest = &src[at..];
        &rest[..rest.find("\n}\n").unwrap_or(rest.len())]
    }

    #[test]
    fn direct_arm_and_send_never_spawn_the_grok_cli() {
        let prod = prod_direct();
        let send = body_of(prod, "pub async fn send(");
        for banned in ["companion_send_grok", "grok_cli_path", "Command::new"] {
            assert!(!send.contains(banned), "{banned} in send");
            assert!(!prod.contains(banned), "{banned} in direct_lane");
        }
        // The send arm, inside companion_send_inner (the agent-roster arms
        // also name "direct" now, alongside grok).
        let coord = body_of(concat!(
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
), "async fn companion_send_inner(");
        let at = coord.find("\"direct\" =>").expect("direct arm");
        let arm = &coord[at..];
        let arm = &arm[..arm.find("\"hermes\" =>").expect("hermes arm follows")];
        assert!(arm.contains("direct_lane::send"));
        assert!(!arm.contains("companion_send_grok"));
        assert!(!arm.contains("grok_cli_path"));
        assert!(!arm.contains("Command::new"));
        assert!(!arm.contains("send_with_hops"));
    }

    #[test]
    fn mouth_route_runs_before_no_harness_and_before_pin() {
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
        let at = coord.find("async fn companion_send_inner(").unwrap();
        let rest = &coord[at..];
        let send = &rest[..rest.find("\n}\n").unwrap()];
        let todo = send.find("companion_todo::intercept").unwrap();
        let route = send.find("mouth_route::run(").unwrap();
        let unknown = send.find("SendRoute::Unknown").unwrap();
        let no_h = send.find("SendRoute::NoHarness").unwrap();
        let pin = send.find("pin_reply_if_empty(").unwrap();
        let conduct = send.find("mouth_ack::conduct(").unwrap();
        assert!(todo < route, "intercepts stay before local verbs");
        assert!(route < unknown, "local verbs beat Unknown");
        assert!(unknown < no_h, "Unknown before NoHarness");
        assert!(no_h < pin, "NoHarness returns before the LM Studio pin");
        assert!(pin < conduct, "pin stays before the conductor");
    }

    #[test]
    fn direct_key_file_is_0600_under_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!(
            "orbitty-direct-key-{}-{}",
            std::process::id(),
            "lane"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("direct.key");
        write_secret_file(&path, "sk-test-abcd").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "sk-test-abcd\n");
        let st = key_status_at(&path);
        assert!(st.set);
        assert_eq!(st.tail, "abcd");
        assert!(write_secret_file(&path, "has space").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "sk-test-abcd\n");
        write_secret_file(&path, "").unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base_url_rejects_non_http_and_non_loopback_http() {
        assert_eq!(validate_direct_base("").unwrap(), "");
        assert_eq!(
            validate_direct_base(" HTTPS://api.example.com/v1/ ").unwrap(),
            "https://api.example.com/v1"
        );
        assert_eq!(
            validate_direct_base("http://127.0.0.1:1234/v1").unwrap(),
            "http://127.0.0.1:1234/v1"
        );
        assert_eq!(
            validate_direct_base("http://localhost:11434").unwrap(),
            "http://localhost:11434"
        );
        assert_eq!(
            validate_direct_base("http://[::1]:8080/v1").unwrap(),
            "http://[::1]:8080/v1"
        );
        assert!(validate_direct_base("http://192.168.1.2:1234").is_err());
        assert!(validate_direct_base("http://example.com").is_err());
        assert!(validate_direct_base("file:///tmp/x").is_err());
        assert!(validate_direct_base("javascript:alert(1)").is_err());
        assert!(validate_direct_base("ftp://localhost").is_err());
        assert!(validate_direct_base("http://localhost.evil.com").is_err());
        // Userinfo / encoding / IPv6 tricks: never plain http (or a key in the
        // URL) to anything but real loopback. Errors are fixed text.
        for bad in [
            "http://127.0.0.1:1@evil.com/v1",
            "http://localhost:x@evil.com/v1",
            "http://user@127.0.0.1/v1",
            "http://127.0.0.1%40evil.com/v1",
            "http://127.0.0.1\\@evil.com/v1",
            "http://evil.com\\@127.0.0.1/v1",
            "http://[::1]@evil.com/v1",
            "http://[::1]evil.com/v1",
            "http://[::ffff:127.0.0.1]/v1",
            "http://[::1%25en0]/v1",
            "http://127.0.0.1:12a/v1",
            "https://u:p@api.example.com/v1",
            "http://:8080/v1",
            "http://[]/v1",
            "http://localhost:/v1",
            "http://localhost:1:2/v1",
            "http://127.0.0.1:99999/v1",
            "http://local%68ost/v1",
            "https://sk-secret-key@api.example.com/v1",
        ] {
            let err = validate_direct_base(bad).expect_err(bad);
            assert!(!err.contains("evil"), "{bad} -> {err}");
            assert!(!err.contains("sk-secret"), "{bad} -> {err}");
            assert!(!err.contains("u:p"), "{bad} -> {err}");
            assert!(!compat_configured(bad, "m", true), "{bad}");
        }
        for good in [
            "http://localhost:1234/v1",
            "http://127.0.0.1:11434/v1",
            "http://[::1]:1234/v1",
            "https://api.example.com/v1",
        ] {
            assert_eq!(validate_direct_base(good).unwrap(), good);
        }
        assert!(!compat_configured("http://example.com", "m", true));
        assert!(compat_configured("https://api.example.com/v1", "m", true));
        assert!(!compat_configured("https://api.example.com/v1", "", true));
        assert!(!compat_configured("https://api.example.com/v1", "m", false));
    }

    #[test]
    fn compat_vision_is_text_only() {
        assert_eq!(direct_vision("compat", "local", true), ("compat".into(), false));
        assert_eq!(direct_vision("local", "local", true), ("local".into(), true));
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
        let at = coord.find("async fn companion_send_inner(").unwrap();
        let rest = &coord[at..];
        let send = &rest[..rest.find("\n}\n").unwrap()];
        assert!(send.contains("b == \"grok\" || b == \"direct\""));
        assert!(crate::attach_vision::vision_gate_applies("compat", false));
        assert!(!crate::attach_vision::vision_gate_applies("compat", true));
    }

    #[test]
    fn route_preview_lists_lanes_and_does_not_echo_the_line() {
        let unset = resolved("", BackendSource::Unset, false);
        let board = preview_route(&unset, "gate status", "grok", &[], false);
        assert_eq!(board.route, "local");
        assert!(board.lanes.is_empty());
        let blob = serde_json::to_string(&board).unwrap();
        assert!(!blob.contains("gate status"));
        let hello = preview_route(&unset, "hello", "grok", &[], false);
        assert_eq!(hello.route, "no_harness");
        let direct = resolved("direct", BackendSource::State, false);
        let planned = preview_route(&direct, "hello", "local", &[], false);
        assert_eq!(planned.route, "direct");
        assert_eq!(planned.lanes, ["local"]);
        assert_eq!(planned.backend, "direct");
        assert_eq!(planned.source, "state");
        let attached = preview_route(&unset, "gate status", "grok", &[], true);
        assert_eq!(attached.route, "no_harness");
    }

    #[test]
    fn ctl_mcp_and_control_list_route_and_not_the_key() {
        let control = include_str!("control.rs");
        let ctl = include_str!("../bin/orbittyctl.rs");
        let mcp = include_str!("../bin/orbitty_mcp.rs");
        assert!(control.contains("\"companion.route\" =>"));
        assert!(ctl.contains("[\"companion\", \"route\"]"));
        assert!(ctl.contains("\"companion.route\", json!({ \"line\": payload(o)? })"));
        assert!(mcp.contains("name: \"companion.route\""));
        assert!(!control.contains("set_direct_key"));
        assert!(!ctl.contains("set_direct_key"));
        assert!(!mcp.contains("set_direct_key"));
        assert!(!control.contains("companion.set_direct_key"));
        let hop = include_str!("../companion/mouth_hop.rs");
        let run = body_of(hop, "pub(crate) fn run_free_lanes(");
        assert!(run.contains("honest_down_line(!direct && skip_grok, &notes)"));
        assert!(run.contains("\"direct\": true"));
        let ui = include_str!("../../../src/lib/MouthSettings.svelte");
        assert!(ui.contains("A compat key is not part of Free Mode."));
        assert!(!ui.contains("compat key is free"));
        let ts = include_str!("../../../src/lib/persist.ts");
        assert!(ts.contains("directBaseUrl"));
        assert!(ts.contains("directModel"));
        assert!(!ts.contains("directKey"));
    }

    /// Lines of `body` from the `"goose" | "grok" | "berd" ...` arm on.
    fn agent_arms(coord: &str, sig: &str) -> (String, String) {
        let body = body_of(coord, sig);
        let arm = body
            .lines()
            .find(|l| l.contains("\"goose\" | \"grok\" | \"berd\""))
            .unwrap_or_else(|| panic!("four-cast arm in {sig}"))
            .trim()
            .to_string();
        let other = &body[body.find("other =>").unwrap_or_else(|| panic!("other arm in {sig}"))..];
        (arm, other.to_string())
    }

    #[test]
    fn direct_takes_the_grok_agent_arm_and_unknown_still_errors() {
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
        for sig in [
            "pub fn companion_agents(app: tauri::AppHandle)",
            "pub fn set_companion_agent(app: tauri::AppHandle, profile: String)",
        ] {
            let (arm, other) = agent_arms(coord, sig);
            // Direct shares grok's companion soul / four-cast stems.
            assert_eq!(arm, "\"goose\" | \"grok\" | \"berd\" | \"direct\" => {", "{sig}");
            let body = body_of(coord, sig);
            assert_eq!(body.matches("\"direct\"").count(), 1, "{sig}: direct only in the grok arm");
            // Anything else is still the honest unknown-harness error.
            let other = &other[..other.find("\n        }\n").expect("other arm end")];
            assert!(other.contains("return Err(crate::companion_backend::unknown_backend_message("), "{sig}");
        }
        assert!(crate::companion_backend::unknown_backend_message("nope", BackendSource::State)
            .starts_with("Unknown harness \"nope\""));
    }

    /// Pins M3. The local-verb door is gated only on "no attachment",
    /// never on a picked harness: with nothing picked,
    /// "what's on the board" still answers (bare `board` is not a local verb).
    #[test]
    fn local_verbs_are_not_gated_on_a_picked_harness() {
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
        let send = body_of(coord, "async fn companion_send_inner(");
        let route = send.find("crate::mouth_route::run(").expect("mouth_route::run");
        let before = &send[..route];
        let guard_at = before.rfind("\n").unwrap();
        let line_start = before[..guard_at].rfind('\n').unwrap() + 1;
        let guard = before[line_start..guard_at].trim();
        assert_eq!(guard, "if attach.is_none() {", "the local-verb guard is attach only");
        let call_line_end = route + send[route..].find('\n').unwrap();
        let call_line = send[guard_at + 1..call_line_end].trim();
        assert_eq!(
            call_line,
            "if let Some(spoken) = crate::mouth_route::run(&app, &line, origin) {"
        );
        // Inside the door, `picked` only decides whether the you-row is saved.
        let door = &send[line_start..];
        let door = &door[..door.find("return Ok(spoken);").expect("door returns")];
        assert_eq!(door.matches("picked").count(), 1, "{door}");
        assert!(door.contains("if !picked {\n                persist_companion_you("));
    }

    /// Split a call's argument list on top-level commas.
    fn call_args(src: &str, call: &str) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = src[from..].find(call) {
            let open = from + at + call.len();
            let (mut depth, mut cur, mut args) = (0i32, String::new(), Vec::new());
            for c in src[open..].chars() {
                match c {
                    '(' | '[' | '{' => {
                        depth += 1;
                        cur.push(c);
                    }
                    ')' | ']' | '}' if depth == 0 => break,
                    ')' | ']' | '}' => {
                        depth -= 1;
                        cur.push(c);
                    }
                    ',' if depth == 0 => args.push(std::mem::take(&mut cur).trim().to_string()),
                    _ => cur.push(c),
                }
            }
            if !cur.trim().is_empty() {
                args.push(cur.trim().to_string());
            }
            out.push(args);
            from = open;
        }
        out
    }

    /// Pins M9. The grok harness (send_with_hops) never opens the compat
    /// HTTP lane: every run_free_lanes call passes direct=false and
    /// try_compat=false. Only direct_lane::send may try compat.
    #[test]
    fn grok_send_with_hops_never_tries_compat() {
        let hop = include_str!("../companion/mouth_hop.rs");
        let sig = body_of(hop, "pub(crate) fn run_free_lanes(");
        let params: Vec<&str> = sig
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with(')'))
            .map(|l| l.trim().split(':').next().unwrap())
            .collect();
        assert_eq!(params[4], "direct");
        assert_eq!(params[6], "try_compat");
        let hops = body_of(hop, "pub async fn send_with_hops(");
        let calls = call_args(hops, "run_free_lanes(");
        assert_eq!(calls.len(), 2, "both grok free-lane calls are pinned");
        for args in &calls {
            assert_eq!(args.len(), 10, "{args:?}");
            assert_eq!(args[4], "false", "grok path is never direct: {args:?}");
            assert_eq!(args[6], "false", "grok path never tries compat: {args:?}");
        }
        assert!(!hops.contains("try_compat"));
    }

    #[test]
    fn route_log_names_the_route_only() {
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
        let body = body_of(coord, "fn log_companion_route(");
        assert!(body.contains("\"companion-route\""));
        assert!(body.contains("\"route\": route"));
        assert!(!body.contains("you_line"));
        assert!(!body.contains("\"line\""));
    }
}
