//! Coordinator + soul.md. Titles-only context. No LLM in this module.

mod hermes;
mod allowlist;
mod plug;
mod mouth_cli;
mod commands;
mod intercept;
mod send;
mod soul_wake;
mod action;
mod paths;
mod cursor_perm;
mod launch_agent;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(test)]
mod launch_tests;

pub(crate) use hermes::*;
pub(crate) use allowlist::*;
pub(crate) use plug::*;
pub(crate) use mouth_cli::*;
pub(crate) use commands::*;
pub(crate) use intercept::*;
pub(crate) use send::*;
pub(crate) use soul_wake::*;
pub(crate) use action::*;
pub(crate) use paths::*;
pub(crate) use cursor_perm::*;
pub(crate) use launch_agent::*;
