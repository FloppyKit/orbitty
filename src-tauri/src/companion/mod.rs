#[path = "companion.rs"]
mod root;
pub(crate) use root::*;
pub(crate) mod chars;
pub(crate) mod companion_backend;
pub(crate) mod companion_clear;
pub(crate) mod companion_desk;
#[cfg(unix)]
pub(crate) mod companion_go;
#[cfg(unix)]
pub(crate) mod companion_policy;
#[cfg(unix)]
pub(crate) mod companion_todo;
pub(crate) mod mouth_ack;
pub(crate) mod mouth_archive;
pub(crate) mod mouth_blip;
pub(crate) mod mouth_board;
pub(crate) mod mouth_gist;
pub(crate) mod key_redact;
pub(crate) mod keystore;
pub(crate) mod model_setup;
pub(crate) mod mouth_hop;
pub(crate) mod mouth_roll;
pub(crate) mod mouth_route;
pub(crate) mod mouth_scan;
pub(crate) mod mouth_sidecar;
pub(crate) mod mouth_tools;
pub(crate) mod policy_guard;
pub(crate) mod talk;
