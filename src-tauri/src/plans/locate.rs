#![allow(unused_imports)]
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use super::dirs::*;
#[allow(unused_imports)]
use super::seal::*;
#[allow(unused_imports)]
use super::resolve::*;
#[allow(unused_imports)]
use super::parse::*;
#[allow(unused_imports)]
use super::rules::*;

pub fn legacy_prompt_file(go: &str) -> Option<PathBuf> {
    legacy_prompt_in(&Roots::real(), go)
}

pub(crate) fn legacy_prompt_in(roots: &Roots, go: &str) -> Option<PathBuf> {
    let name = format!("_{}_PROMPT.md", go.replace('-', "_"));
    for d in read_dirs_in(roots) {
        let p = d.join(&name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn gate_for_card(id: &str, title: &str, link_urls: &[String]) -> Gate {
    gate_in(&Roots::real(), id, title, link_urls)
}

pub(crate) fn gate_in(roots: &Roots, id: &str, title: &str, link_urls: &[String]) -> Gate {
    match locate(roots, id, title, link_urls) {
        Locate::Missing => Gate::Skip,
        Locate::Bad { note } => Gate::Refuse { note },
        Locate::Found(go) => match seal_note(roots, &go) {
            None => Gate::Launch(go),
            Some(note) => Gate::Refuse { note },
        },
    }
}

pub(crate) enum Locate {
    Found(ParsedGo),
    Bad { note: String },
    Missing,
}

/// Deepest folder level searched for GO files under a plans folder. A match
/// nested deeper is refused with a note, never silently skipped.
pub const MAX_GO_DEPTH: usize = 6;
/// How far the walk looks, so a too-deep match is seen and refused.
pub(crate) const WALK_LIMIT: usize = 16;

/// Fail closed: once a card names a GO token (or links a file inside the
/// plans folder), the answer is a GO file or a refusal note. `Missing`
/// (launch with the title only) is only for cards that are not GOs.
pub(crate) fn locate(roots: &Roots, id: &str, title: &str, link_urls: &[String]) -> Locate {
    let dirs = read_dirs_in(roots);
    let mut linked = false;
    for url in link_urls {
        let Some(path) = file_url_to_path(url) else {
            continue;
        };
        if !dirs.iter().any(|d| is_inside(&path, d)) {
            continue;
        }
        linked = true;
        if !path.is_file() {
            continue;
        }
        return match parse_file(&path) {
            Ok(g) => Locate::Found(g),
            Err(note) => Locate::Bad { note },
        };
    }
    let token = crate::needed_rules::extract_go(title);
    let tok = token.as_deref();
    let slug = tok.map(|t| t.replace('-', "_"));
    let names_token = |p: &Path| -> bool {
        let (Some(t), Some(s)) = (tok, slug.as_deref()) else {
            return false;
        };
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        name.contains(t) || name.contains(s)
    };
    let mut files = Vec::new();
    for d in &dirs {
        walk_md(d, &mut files, 0);
    }
    files.sort();
    let mut by_card: Option<(ParsedGo, usize)> = None;
    let mut by_id: Option<(ParsedGo, usize)> = None;
    let mut headerless: Option<PathBuf> = None;
    let mut malformed_named: Option<String> = None;
    let mut malformed_other: Option<String> = None;
    for (path, depth) in &files {
        match parse_file(path) {
            Ok(g) if !g.legacy => {
                if !id.is_empty() && g.header.card_id.as_deref() == Some(id.trim()) {
                    if by_card.is_none() {
                        by_card = Some((g, *depth));
                    }
                    continue;
                }
                // Header ids match case-insensitively: a lowercase `id`
                // is still this GO and still goes through the seal gate.
                if let Some(t) = tok {
                    if by_id.is_none()
                        && g.header.id.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(t))
                    {
                        by_id = Some((g, *depth));
                    }
                }
            }
            Ok(_) => {
                // Headerless file named for this GO (not a legacy
                // `_X_PROMPT.md`, which has its own rule below).
                if headerless.is_none()
                    && legacy_token_from_path(path).is_none()
                    && names_token(path)
                {
                    headerless = Some(path.clone());
                }
            }
            Err(note) => {
                if names_token(path) {
                    if malformed_named.is_none() {
                        malformed_named = Some(note);
                    }
                } else if tok.is_some() && malformed_other.is_none() {
                    malformed_other = Some(format!("{}: {note}", path.display()));
                }
            }
        }
    }
    if let Some((g, depth)) = by_card.or(by_id) {
        if depth > MAX_GO_DEPTH {
            return Locate::Bad {
                note: format!(
                    "{} is nested {depth} folders deep in the plans folder (max {MAX_GO_DEPTH}); not launching.",
                    g.path.display()
                ),
            };
        }
        return Locate::Found(g);
    }
    let Some(t) = tok else {
        if linked {
            return Locate::Bad {
                note: "The card links a GO file in the plans folder that is missing; not launching."
                    .into(),
            };
        }
        return Locate::Missing;
    };
    if let Some(path) = legacy_prompt_in(roots, t) {
        return match parse_file(&path) {
            Ok(g) => Locate::Found(g),
            Err(note) => Locate::Bad { note },
        };
    }
    if let Some(p) = headerless {
        return Locate::Bad {
            note: format!("{t}: {} has no GO header; not launching.", p.display()),
        };
    }
    if let Some(note) = malformed_named {
        return Locate::Bad { note };
    }
    if let Some(note) = malformed_other {
        return Locate::Bad {
            note: format!(
                "{t}: no GO file with a matching header; a malformed header may be it ({note}); not launching."
            ),
        };
    }
    Locate::Bad {
        note: format!("{t}: no GO file with a matching header in the plans folder; not launching."),
    }
}

