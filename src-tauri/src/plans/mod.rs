//! Plans folder, GO header, seal gate, and ruleset.
//!
//! One shared settings file (`~/.config/orbitty/settings.json`) for Nightly
//! and stable. `plansDir` defaults to the visible `$HOME/Orbitty GOs`.
//! `~/.buzz/PLANS` is read-only back-compat. No absolute home path in code.

mod dirs;
mod locate;
mod seal;
mod resolve;
mod parse;
mod rules;
#[cfg(test)]
mod tests;

pub(crate) use dirs::*;
pub(crate) use locate::*;
pub(crate) use seal::*;
pub(crate) use resolve::*;
pub(crate) use parse::*;
#[allow(unused_imports)]
pub(crate) use rules::*;
