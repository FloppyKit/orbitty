//! Companion harness pick.
//! Order: non-empty `ORBITTY_COMPANION_BACKEND` (override) → non-empty
//! `companion.backend` → legacy `goose` only when goose is on PATH → unset.
//! `berd` is valid from the env var only. `direct` is a built-in picker
//! id (no CLI). OpenRouter and local stay on the Reply lane.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Shown as the reply when nothing is picked. No model is called.
pub const NO_HARNESS_ROW: &str = "No harness picked. Pick one in Mouth settings.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendSource {
    Env,
    State,
    LegacyDefault,
    Unset,
}

impl BackendSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::State => "state",
            Self::LegacyDefault => "legacy_default",
            Self::Unset => "unset",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendPick {
    pub id: String,
    pub source: BackendSource,
}

/// Presence flags only. Paths stay on the choice list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detected {
    pub goose: bool,
    pub grok: bool,
    pub hermes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendResolve {
    pub pick: BackendPick,
    /// The winning non-empty value is not a sendable id for that source.
    pub unknown: bool,
}

fn norm(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn env_sendable(id: &str) -> bool {
    matches!(id, "goose" | "grok" | "hermes" | "berd" | "direct")
}

fn state_sendable(id: &str) -> bool {
    matches!(id, "goose" | "grok" | "hermes" | "direct")
}

/// Pure pick. Tests pass explicit strings and flags; this does not read
/// the process env, PATH, or state.json.
pub fn resolve_backend(env: &str, state: &str, detected: Detected) -> BackendResolve {
    let env = norm(env);
    if !env.is_empty() {
        return BackendResolve {
            pick: BackendPick {
                id: env.clone(),
                source: BackendSource::Env,
            },
            unknown: !env_sendable(&env),
        };
    }
    let state = norm(state);
    if !state.is_empty() {
        return BackendResolve {
            pick: BackendPick {
                id: state.clone(),
                source: BackendSource::State,
            },
            unknown: !state_sendable(&state),
        };
    }
    if detected.goose {
        return BackendResolve {
            pick: BackendPick {
                id: "goose".into(),
                source: BackendSource::LegacyDefault,
            },
            unknown: false,
        };
    }
    BackendResolve {
        pick: BackendPick {
            id: String::new(),
            source: BackendSource::Unset,
        },
        unknown: false,
    }
}

/// One sentence for a bad id. Names the env var only when that value
/// came from the env override.
pub fn unknown_backend_message(id: &str, source: BackendSource) -> String {
    if source == BackendSource::Env {
        format!("Unknown ORBITTY_COMPANION_BACKEND {id:?} — goose, grok, hermes, berd, or direct.")
    } else {
        format!("Unknown harness {id:?} — goose, grok, hermes, or direct.")
    }
}

/// Known Mouth ids, plus `""` to clear. `berd` is not a saved pick.
pub fn validate_companion_backend(raw: &str) -> Result<String, String> {
    let s = norm(raw);
    match s.as_str() {
        "" | "goose" | "grok" | "hermes" | "direct" => Ok(s),
        _ => Err(unknown_backend_message(&s, BackendSource::State)),
    }
}

fn read_env() -> String {
    std::env::var("ORBITTY_COMPANION_BACKEND").unwrap_or_default()
}

pub fn resolve_companion_backend(app: &AppHandle) -> BackendResolve {
    let env = read_env();
    let state = crate::state::load_state(app)
        .map(|s| s.companion.backend)
        .unwrap_or_default();
    let probe = crate::mouth_scan::harness_probe();
    resolve_backend(
        &env,
        &state,
        Detected {
            goose: probe.goose,
            grok: probe.grok,
            hermes: probe.hermes,
        },
    )
}

/// Sendable id, or the plain error (unknown, or unset).
pub fn require_send_backend(app: &AppHandle) -> Result<(String, BackendSource), String> {
    let r = resolve_companion_backend(app);
    if r.unknown {
        return Err(unknown_backend_message(&r.pick.id, r.pick.source));
    }
    if r.pick.id.is_empty() {
        return Err(NO_HARNESS_ROW.into());
    }
    Ok((r.pick.id, r.pick.source))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionHarnesses {
    pub choices: Vec<crate::mouth_scan::HarnessChoice>,
    pub id: String,
    pub source: String,
    pub env_override: bool,
}

#[tauri::command]
pub fn companion_harnesses(app: AppHandle) -> Result<CompanionHarnesses, String> {
    let r = resolve_companion_backend(&app);
    Ok(CompanionHarnesses {
        choices: crate::mouth_scan::harness_choices(),
        id: r.pick.id,
        source: r.pick.source.as_str().into(),
        env_override: r.pick.source == BackendSource::Env,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionBackendSet {
    pub backend: String,
    pub env_override: bool,
    pub message: String,
}

/// Saves the pick even when the env override is set. The reply says so
/// when that override is active; the env value still wins at send.
#[tauri::command]
pub fn set_companion_backend(app: AppHandle, id: String) -> Result<CompanionBackendSet, String> {
    let id = validate_companion_backend(&id)?;
    let before = crate::state::load_state(&app)
        .map(|s| s.companion.backend)
        .unwrap_or_default();
    crate::state::update(&app, |st| {
        st.companion.backend = id.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    if before != id {
        crate::event_log::append_best(
            &app,
            "companion-backend",
            None,
            None,
            Some("agent"),
            true,
            &serde_json::json!({ "from": before, "to": id, "source": "state" }),
        );
    }
    let env_override = !norm(&read_env()).is_empty();
    let message = if env_override {
        "Saved. ORBITTY_COMPANION_BACKEND is set, so that override still wins.".into()
    } else if id.is_empty() {
        "Harness cleared.".into()
    } else {
        format!("Harness set to {id}.")
    };
    Ok(CompanionBackendSet {
        backend: id,
        env_override,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Detected {
        Detected {
            goose: false,
            grok: false,
            hermes: false,
        }
    }

    fn goose_on() -> Detected {
        Detected {
            goose: true,
            grok: false,
            hermes: false,
        }
    }

    #[test]
    fn resolve_backend_table() {
        let r = resolve_backend("grok", "hermes", goose_on());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "grok");
        assert_eq!(r.pick.source, BackendSource::Env);

        let r = resolve_backend("  Grok  ", "goose", none());
        assert_eq!(r.pick.id, "grok");
        assert_eq!(r.pick.source, BackendSource::Env);

        let r = resolve_backend("", "grok", goose_on());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "grok");
        assert_eq!(r.pick.source, BackendSource::State);

        let r = resolve_backend("   ", "hermes", goose_on());
        assert_eq!(r.pick.id, "hermes");
        assert_eq!(r.pick.source, BackendSource::State);

        let r = resolve_backend("", "", goose_on());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "goose");
        assert_eq!(r.pick.source, BackendSource::LegacyDefault);

        let r = resolve_backend("", "", Detected {
            goose: false,
            grok: true,
            hermes: true,
        });
        assert!(!r.unknown);
        assert!(r.pick.id.is_empty());
        assert_eq!(r.pick.source, BackendSource::Unset);

        let r = resolve_backend("", "", none());
        assert_eq!(r.pick.source, BackendSource::Unset);

        let r = resolve_backend("nope", "goose", goose_on());
        assert!(r.unknown);
        assert_eq!(r.pick.id, "nope");
        assert_eq!(r.pick.source, BackendSource::Env);
        let msg = unknown_backend_message(&r.pick.id, r.pick.source);
        assert!(msg.contains("ORBITTY_COMPANION_BACKEND"));
        assert!(msg.contains("nope"));

        let r = resolve_backend("berd", "goose", none());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "berd");
        assert_eq!(r.pick.source, BackendSource::Env);

        let r = resolve_backend("", "berd", goose_on());
        assert!(r.unknown);
        assert_eq!(r.pick.id, "berd");
        assert_eq!(r.pick.source, BackendSource::State);
        let msg = unknown_backend_message(&r.pick.id, r.pick.source);
        assert!(!msg.contains("ORBITTY_COMPANION_BACKEND"));
    }

    #[test]
    fn picks_hermes() {
        let r = resolve_backend("", "Hermes", none());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "hermes");
        assert_eq!(r.pick.source, BackendSource::State);
        assert_eq!(validate_companion_backend("hermes").unwrap(), "hermes");

        let r = resolve_backend("hermes", "goose", goose_on());
        assert!(!r.unknown);
        assert_eq!(r.pick.id, "hermes");
        assert_eq!(r.pick.source, BackendSource::Env);
    }

    #[test]
    fn set_companion_backend_validation() {
        assert_eq!(validate_companion_backend("").unwrap(), "");
        assert_eq!(validate_companion_backend(" GOOSE ").unwrap(), "goose");
        assert_eq!(validate_companion_backend("grok").unwrap(), "grok");
        assert_eq!(validate_companion_backend(" Direct ").unwrap(), "direct");
        assert!(validate_companion_backend("berd").is_err());
        assert!(validate_companion_backend("openrouter").is_err());
        assert!(validate_companion_backend("local").is_err());
        assert!(validate_companion_backend("nope").is_err());
    }

    #[test]
    fn control_ctl_mcp_list_the_two_verbs() {
        let control = include_str!("../agents/control.rs");
        assert!(control.contains("\"companion.harnesses\" =>"));
        assert!(control.contains("\"companion.set_backend\" =>"));
        let ctl = include_str!("../bin/orbittyctl.rs");
        assert!(ctl.contains("[\"companion\", \"harnesses\"]"));
        assert!(ctl.contains("[\"companion\", \"set-backend\"]"));
        let mcp = include_str!("../bin/orbitty_mcp.rs");
        assert!(mcp.contains("name: \"companion.harnesses\""));
        assert!(mcp.contains("name: \"companion.set_backend\""));
    }

    #[test]
    fn env_var_read_and_unknown_copy_are_not_duplicated() {
        let here = include_str!("companion_backend.rs");
        let prod = here.split("mod tests").next().unwrap();
        assert_eq!(
            prod.matches("std::env::var(\"ORBITTY_COMPANION_BACKEND\")")
                .count(),
            1
        );
        assert_eq!(prod.matches("Unknown ORBITTY_COMPANION_BACKEND").count(), 1);
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
        assert_eq!(
            coord
                .matches("std::env::var(\"ORBITTY_COMPANION_BACKEND\")")
                .count(),
            0
        );
        assert_eq!(coord.matches("Unknown ORBITTY_COMPANION_BACKEND").count(), 0);
    }
}
