//! Microtags on standard NIP-24 `t` tags.
//!
//! The only standard is NIP-24: a `t` value must be lowercase. `namespace:value`
//! (planet, kind, area) is Orbitty's proposal, not a NIP. See docs/MICROTAGS.md.
//!
//! ASCII only. `unicode-normalization` is not in Cargo.lock, so any non-ASCII
//! input is refused. That matches the sealed default (no NFKC).

pub const RESERVED: &[&str] = &["orbitty", "local-only", "root", "root-revision"];
pub const NAMESPACES: &[&str] = &["planet", "kind", "area"];
pub const KIND_VALUES: &[&str] = &["bug", "feature", "chore", "question"];

pub fn normalize(raw: &str) -> Option<String> {
    if !raw.is_ascii() {
        return None;
    }
    let mut s = raw.to_ascii_lowercase();
    s = s.trim().to_string();
    if let Some(rest) = s.strip_prefix('#') {
        s = rest.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(if c == ' ' || c == '_' { '-' } else { c });
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    if out.is_empty() || out.len() > 64 {
        return None;
    }
    if out.starts_with('-') || out.ends_with('-') {
        return None;
    }
    let mut colons = 0usize;
    for c in out.chars() {
        if c == ':' {
            colons += 1;
            if colons > 1 {
                return None;
            }
            continue;
        }
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if !ok {
            return None;
        }
    }
    if let Some((ns, val)) = out.split_once(':') {
        if ns.is_empty() || val.is_empty() || edged(ns) || edged(val) {
            return None;
        }
        if ns == "status" {
            return None;
        }
        if ns == "kind" && !KIND_VALUES.contains(&val) {
            return None;
        }
    }
    if RESERVED.contains(&out.as_str()) {
        return None;
    }
    Some(out)
}

/// Project id, not the display name. A rename does not move the slug.
pub fn planet_slug(project_id: &str, display_name: &str) -> Option<String> {
    let _frozen_across_rename = display_name;
    normalize(project_id)
}

pub fn to_t_tags(tags: &[String], planet_slug: Option<&str>) -> Vec<[String; 2]> {
    let mut out: Vec<[String; 2]> = Vec::new();
    for raw in tags {
        let Some(n) = normalize(raw) else {
            continue;
        };
        if n.starts_with("planet:") {
            continue;
        }
        if out.iter().any(|t| t[1] == n) {
            continue;
        }
        out.push(["t".into(), n]);
    }
    if let Some(slug) = planet_slug {
        if let Some(tag) = planet_tag(slug) {
            out.push(["t".into(), tag]);
        }
    }
    out
}

/// Flag off: tags are unchanged and nothing is refused (the event stays
/// byte-identical). Flag on: microtags are inserted immediately after
/// `t=orbitty`. Returns the raw card tags that normalize refused.
pub fn apply_board_microtags(
    tags: &mut Vec<[String; 2]>,
    flag_on: bool,
    names: &[String],
    project_id: &str,
    display_name: &str,
) -> Vec<String> {
    if !flag_on {
        return Vec::new();
    }
    let slug = planet_slug(project_id, display_name);
    let extra = to_t_tags(names, slug.as_deref());
    let refused: Vec<String> = names
        .iter()
        .filter(|n| normalize(n).is_none())
        .cloned()
        .collect();
    let at = tags
        .iter()
        .position(|t| t[0] == "t" && t[1] == "orbitty")
        .map(|i| i + 1)
        .unwrap_or(0);
    for (i, tag) in extra.into_iter().enumerate() {
        tags.insert(at + i, tag);
    }
    refused
}

/// Failed values stay the original display string and are never microtags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inbound {
    pub microtag: Option<String>,
    pub display: String,
}

pub fn inbound(raw: &str) -> Inbound {
    Inbound {
        microtag: normalize(raw),
        display: raw.to_string(),
    }
}

fn edged(s: &str) -> bool {
    s.is_empty() || s.starts_with('-') || s.ends_with('-')
}

fn planet_tag(slug: &str) -> Option<String> {
    let n = normalize(slug)?;
    if n.starts_with("planet:") {
        return Some(n);
    }
    normalize(&format!("planet:{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        reserved: Vec<String>,
        namespaces: Vec<String>,
        #[serde(rename = "kindValues")]
        kind_values: Vec<String>,
        rows: Vec<Row>,
    }

    #[derive(Deserialize)]
    struct Row {
        id: String,
        raw: String,
        tag: Option<String>,
    }

    fn fixture() -> Fixture {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../qa/fixtures/microtags.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse fixture: {e}"))
    }

    fn names(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    fn base_tags() -> Vec<[String; 2]> {
        vec![
            ["t".into(), "orbitty".into()],
            ["d".into(), "c-1".into()],
            ["project".into(), "p-keep".into()],
            ["actor".into(), "user".into()],
            ["trace".into(), "tr".into()],
        ]
    }

    fn t_vals(tags: &[[String; 2]]) -> Vec<&str> {
        tags.iter()
            .filter(|t| t[0] == "t")
            .map(|t| t[1].as_str())
            .collect()
    }

    #[test]
    fn m_t1_normal_forms() {
        assert_eq!(normalize("#Bug Fix").as_deref(), Some("bug-fix"));
        assert_eq!(normalize("Kind:Bug").as_deref(), Some("kind:bug"));
        assert_eq!(normalize("  a__b  ").as_deref(), Some("a-b"));
        assert_eq!(normalize("area:Relay").as_deref(), Some("area:relay"));
        assert_eq!(normalize("Nostr").as_deref(), Some("nostr"));
    }

    #[test]
    fn m_t2_refusals() {
        assert_eq!(normalize("a:b:c"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize(&"a".repeat(65)), None);
        let sixty_four = "a".repeat(64);
        assert_eq!(normalize(&sixty_four).as_deref(), Some(sixty_four.as_str()));
        assert_eq!(normalize("é"), None);
        assert_eq!(normalize("kind:nope"), None);
        assert_eq!(normalize("-bad"), None);
        assert_eq!(normalize("bad-"), None);
    }

    #[test]
    fn m_t3_reserved() {
        for raw in [
            "orbitty",
            "local-only",
            "root",
            "root-revision",
            "#Orbitty",
            "ROOT",
            "Local-Only",
            "status:done",
            "Status:Open",
        ] {
            assert_eq!(normalize(raw), None, "{raw}");
        }
        assert_eq!(normalize("root-cause").as_deref(), Some("root-cause"));
        assert_eq!(normalize("orbitty-extra").as_deref(), Some("orbitty-extra"));
        assert_eq!(normalize("status").as_deref(), Some("status"));
    }

    #[test]
    fn m_t4_fixture_matches_normalize() {
        let fix = fixture();
        assert!(fix.rows.len() >= 30, "rows {}", fix.rows.len());
        assert_eq!(fix.reserved, RESERVED);
        assert_eq!(fix.namespaces, NAMESPACES);
        assert_eq!(fix.kind_values, KIND_VALUES);
        for row in &fix.rows {
            assert_eq!(normalize(&row.raw), row.tag, "{}", row.id);
        }
    }

    #[test]
    fn m_t5_orbitty_first_then_microtags_and_publishable() {
        let mut tags = base_tags();
        let card = names(&[
            "#Bug Fix",
            "area:Relay",
            "root",
            "#Bug Fix",
            "planet:nope",
            "kind:bug",
            "local-only",
        ]);
        let refused = apply_board_microtags(&mut tags, true, &card, "p-keep", "Relay");
        assert_eq!(tags[0], ["t".to_string(), "orbitty".to_string()]);
        assert_eq!(
            t_vals(&tags),
            vec![
                "orbitty",
                "bug-fix",
                "area:relay",
                "kind:bug",
                "planet:p-keep"
            ]
        );
        assert_eq!(refused, vec!["root".to_string(), "local-only".to_string()]);
        assert!(
            t_vals(&tags).iter().position(|t| *t == "orbitty")
                < t_vals(&tags).iter().position(|t| *t == "bug-fix")
        );
        let line = serde_json::json!({
            "id": "ab".repeat(32),
            "pubkey": "pk",
            "sig": "sig",
            "kind": 420,
            "tags": tags,
            "content": "{}",
        });
        assert!(crate::event_relay::publishable(&line));
    }

    #[test]
    fn m_t6_inbound_keeps_raw_display() {
        let bad = inbound("é");
        assert_eq!(bad.microtag, None);
        assert_eq!(bad.display, "é");
        let reserved = inbound("root");
        assert_eq!(reserved.microtag, None);
        assert_eq!(reserved.display, "root");
        let two = inbound("a:b:c");
        assert_eq!(two.microtag, None);
        assert_eq!(two.display, "a:b:c");
        let ok = inbound("#Bug Fix");
        assert_eq!(ok.microtag.as_deref(), Some("bug-fix"));
        assert_eq!(ok.display, "#Bug Fix");
    }

    #[test]
    fn m_t7_one_planet_tag_frozen_across_rename() {
        assert_eq!(planet_slug("p-keep", "Relay").as_deref(), Some("p-keep"));
        assert_eq!(
            planet_slug("p-keep", "Renamed Relay").as_deref(),
            Some("p-keep")
        );
        let card = names(&["#Bug Fix", "planet:other"]);
        let before = to_t_tags(&card, planet_slug("p-keep", "Relay").as_deref());
        let after = to_t_tags(&card, planet_slug("p-keep", "Renamed Relay").as_deref());
        assert_eq!(before, after);
        let planets: Vec<_> = before
            .iter()
            .filter(|t| t[1].starts_with("planet:"))
            .map(|t| t[1].as_str())
            .collect();
        assert_eq!(planets, vec!["planet:p-keep"]);
    }

    #[test]
    fn flag_off_board_event_byte_equal() {
        let mut tags = base_tags();
        let before = serde_json::json!({
            "kind": 420,
            "tags": tags,
            "content": "{\"cardId\":\"c-1\"}",
        })
        .to_string();
        let refused = apply_board_microtags(
            &mut tags,
            false,
            &names(&["#Bug Fix", "root", "area:Relay"]),
            "p-keep",
            "Renamed",
        );
        assert!(refused.is_empty());
        let after = serde_json::json!({
            "kind": 420,
            "tags": tags,
            "content": "{\"cardId\":\"c-1\"}",
        })
        .to_string();
        assert_eq!(after, before);
    }
}
