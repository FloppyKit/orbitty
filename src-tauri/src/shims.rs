//! Named workarounds and one counter each. Flag `shim_ledger` off: nothing is written.

use std::cell::RefCell;

pub struct ShimDef {
    pub id: &'static str,
    pub site: &'static str,
    pub since: &'static str,
    pub note: &'static str,
    pub patches: &'static str,
}

pub const SHIMS: &[ShimDef] = &[
    ShimDef {
        id: "ack_fallback",
        site: "companion/mouth_ack.rs",
        since: "2026-09-17",
        note: "keyword classifier when the sidecar does not ack",
        patches: "any",
    },
    ShimDef {
        id: "handoff_strip",
        site: "companion/mouth_hop",
        since: "2026-10-08",
        note: "strip a forged handoff mark before the reply is saved",
        patches: "any",
    },
    ShimDef {
        id: "goose_context_dead",
        site: "coordinator/send.rs",
        since: "2026-10-08",
        note: "goose context dead rolls the mouth onto a fresh session",
        patches: "goose",
    },
    ShimDef {
        id: "goose_empty_resume",
        site: "coordinator/send.rs",
        since: "2026-10-08",
        note: "empty goose resume rolls the mouth onto a fresh session",
        patches: "goose",
    },
    ShimDef {
        id: "grok_session_gone",
        site: "coordinator/send.rs",
        since: "2026-10-08",
        note: "grok session gone starts a fresh session",
        patches: "grok",
    },
    ShimDef {
        id: "grok_context_dead",
        site: "coordinator/send.rs",
        since: "2026-10-08",
        note: "grok context dead rolls onto a new session",
        patches: "grok",
    },
    ShimDef {
        id: "grok_empty_resume",
        site: "coordinator/send.rs",
        since: "2026-10-08",
        note: "empty grok resume rolls onto a new session",
        patches: "grok",
    },
    ShimDef {
        id: "model_argv_refuse",
        site: "coordinator/launch_agent.rs",
        since: "2026-10-08",
        note: "model text in herdr argv is refused, never stripped",
        patches: "cursor",
    },
];

pub struct ShimCtx<'a> {
    pub seat: Option<&'a str>,
}

pub(crate) fn rows() -> Vec<crate::ledger::ShimRow> {
    SHIMS
        .iter()
        .map(|s| crate::ledger::ShimRow {
            id: s.id.to_string(),
            site: s.site.to_string(),
            since: s.since.to_string(),
            note: s.note.to_string(),
            patches: s.patches.to_string(),
        })
        .collect()
}

/// Count one firing. Flag off, or no app-data dir: return without creating files.
/// A ledger write error does not fail the caller.
/// Inside `cargo test` this is a no-op so the suite cannot touch the real ledger;
/// writer tests call `run_fire` on a temp directory.
pub fn fired(app: &tauri::AppHandle, id: &str, ctx: ShimCtx<'_>) {
    #[cfg(test)]
    {
        let _ = (app, id, ctx.seat);
    }
    #[cfg(not(test))]
    fire_live(app, id, ctx);
}

#[cfg(not(test))]
fn fire_live(app: &tauri::AppHandle, id: &str, ctx: ShimCtx<'_>) {
    use tauri::Manager;
    if !crate::flags::on(app, "shim_ledger") {
        return;
    }
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    run_fire(&dir, true, id, ctx.seat, cfg!(debug_assertions));
}

/// `debug` panics on an id that is not in [`SHIMS`]. Release counts it as `unknown`.
pub(crate) fn run_fire(
    app_data: &std::path::Path,
    flag_on: bool,
    id: &str,
    seat: Option<&str>,
    debug: bool,
) {
    if !flag_on {
        return;
    }
    let id: &str = if SHIMS.iter().any(|s| s.id == id) {
        id
    } else if debug {
        panic!("unknown shim id {id}");
    } else {
        "unknown"
    };
    let _ = crate::ledger::record(app_data, id, seat);
}

thread_local! {
    static BOUND: RefCell<Option<tauri::AppHandle>> = RefCell::new(None);
}

/// `assemble_hop_messages` has no app handle. The hop door arms this for the call.
pub struct Bind {
    _priv: (),
}

impl Bind {
    pub fn arm(app: &tauri::AppHandle) -> Self {
        BOUND.with(|slot| *slot.borrow_mut() = Some(app.clone()));
        Self { _priv: () }
    }
}

impl Drop for Bind {
    fn drop(&mut self) {
        BOUND.with(|slot| *slot.borrow_mut() = None);
    }
}

pub fn fired_bound(id: &str, ctx: ShimCtx<'_>) {
    let app = BOUND.with(|slot| slot.borrow().clone());
    if let Some(app) = app {
        fired(&app, id, ctx);
    }
}

#[cfg(test)]
#[path = "shims_tests.rs"]
mod tests;
