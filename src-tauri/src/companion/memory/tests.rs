use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::ram::{AnsweredAsk, CardIn, RamCtx, RAM_CAP};
use super::tags::{accept_into, propose_into, AcceptBatch, TagV0};
use super::{build, read_notes_head, recall, render, write_notes, NOTES_FILE_BYTES, NOTES_RAM_CHARS};
use crate::mouth_archive::{append, ArchiveLine};
use crate::mouth_hop::assemble_hop_messages;
use crate::mouth_roll::{handoff_from_turns, ResetReason};
use crate::state::{CompanionTurn, Tag};

fn turn(role: &str, text: impl AsRef<str>) -> CompanionTurn {
    CompanionTurn {
        role: role.into(),
        text: text.as_ref().into(),
    }
}

fn ctx(rules: &str, soul: &str) -> RamCtx {
    RamCtx {
        rules: rules.into(),
        soul: soul.into(),
        now: "job: desk".into(),
        planet_line: Some("planet-orbitty Orbitty".into()),
        notes: None,
        cards: vec![CardIn {
            title: "OPEN-CARD".into(),
            open: true,
            active: true,
            touched: 10.0,
        }],
        pinned_tags: vec!["pinned-tag".into()],
        open_asks: vec![("n1".into(), "ASK-KEEP".into())],
        answered: Vec::new(),
        turns: vec![turn("you", "TURN-VOL")],
        now_secs: 1_000_000.0,
    }
}

#[test]
fn ram_cap_6000_truncation_order() {
    let mut big = ctx("RULES-NEVER-CUT", "SOUL-NEVER-CUT");
    big.open_asks = vec![("a1".into(), "A".repeat(7_000))];
    big.pinned_tags = vec!["tag-keep".into()];
    big.turns = vec![turn("you", format!("TURN-OLD-{}", "x".repeat(3_000))), turn("them", "TURN-NEW")];
    let text = render(&build(&big));
    assert!(text.chars().count() <= RAM_CAP, "{}", text.chars().count());
    assert!(text.contains("RULES-NEVER-CUT"));
    assert!(text.contains("SOUL-NEVER-CUT"));
    assert!(!text.contains("TURN-OLD-"));
    assert!(text.contains("tag-keep"), "tags survive until asks are gone\n{text}");
    assert!(
        !text.contains(&"A".repeat(100)),
        "asks drop after turns\n{text}"
    );

    let mut rules = "RULE-START-".to_string();
    rules.push_str(&"R".repeat(7_000));
    rules.push_str("-RULE-END");
    let mut soul = "SOUL-START-".to_string();
    soul.push_str(&"S".repeat(5_000));
    soul.push_str("-SOUL-END");
    let mut over = ctx(&rules, &soul);
    over.turns = vec![turn("you", "Z".repeat(2_000))];
    over.open_asks = vec![("a1".into(), "ASK-DROP".into())];
    let text = render(&build(&over));
    assert!(text.contains("RULE-START-"));
    assert!(text.contains("-RULE-END"));
    assert!(text.contains("SOUL-START-"));
    assert!(text.contains("-SOUL-END"));
    assert!(!text.contains("ASK-DROP"));
    assert!(text.chars().count() > RAM_CAP);
}

#[test]
fn ram_stable_prefix() {
    let core = build(&ctx("RULE-KEEP", "SOUL-KEEP"));
    let a = render(&core);
    let b = render(&core);
    assert_eq!(a, b);
    assert!(a.starts_with("[rules]\nRULE-KEEP\n[soul]\nSOUL-KEEP\n"));
    let rules_at = a.find("RULE-KEEP").unwrap();
    let soul_at = a.find("SOUL-KEEP").unwrap();
    let turn_at = a.find("TURN-VOL").unwrap();
    assert!(rules_at < soul_at && soul_at < turn_at);
}

#[test]
fn keep_rules_always_in() {
    let mut c = ctx("MODE-LAW-RULE", "soul");
    c.pinned_tags = (1..=15).map(|n| format!("tag-{n:02}")).collect();
    c.turns = vec![turn("you", "x".repeat(4_000)), turn("them", "kept-turn")];
    let text = render(&build(&c));
    assert!(text.contains("MODE-LAW-RULE"));
    assert!(text.contains("n1 ASK-KEEP"));
    assert!(text.contains("tag-01"));
    assert!(text.contains("tag-12"));
    assert!(!text.contains("tag-13"));
    assert!(text.contains("OPEN-CARD"));
}

#[test]
fn forget_closed_and_old() {
    let mut c = ctx("rules", "soul");
    c.cards.push(CardIn {
        title: "CLOSED-SECRET".into(),
        open: false,
        active: true,
        touched: 99.0,
    });
    c.answered = vec![
        AnsweredAsk {
            id: "old".into(),
            ask: "STALE-SECRET".into(),
            at: 1_000_000.0 - (24.0 * 3600.0),
        },
        AnsweredAsk {
            id: "fresh".into(),
            ask: "FRESH-ASK".into(),
            at: 1_000_000.0 - (24.0 * 3600.0) + 1.0,
        },
    ];
    c.turns = (0..8).map(|i| turn("you", format!("turn-{i}"))).collect();
    let text = render(&build(&c));
    assert!(text.contains("OPEN-CARD"));
    assert!(!text.contains("CLOSED-SECRET"));
    assert!(!text.contains("STALE-SECRET"));
    assert!(text.contains("FRESH-ASK"));
    assert!(text.contains("n1 ASK-KEEP"));
    assert!(!text.contains("turn-0"));
    assert!(!text.contains("turn-1"));
    assert!(text.contains("turn-2"));
    assert!(text.contains("turn-7"));
}

#[test]
fn planet_notes_only_when_active_1200() {
    let dir = std::env::temp_dir().join(format!("orbitty-ram-notes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut notes = String::from("HEAD-MARKER\n");
    while notes.chars().count() < 1_300 {
        notes.push('n');
    }
    notes.push_str("TAIL-MARKER");
    write_notes(&dir, "planet-a", &notes).unwrap();
    let file = dir.join("planets").join("planet-a").join("notes.md");
    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let folder = fs::metadata(dir.join("planets").join("planet-a"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(folder, 0o700);
    let tabs = fs::metadata(dir.join("planets").join("planet-a").join("tabs"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(tabs, 0o700);
    assert!(fs::metadata(&file).unwrap().len() <= NOTES_FILE_BYTES as u64);

    let head = read_notes_head(&dir, "planet-a");
    assert!(head.contains("HEAD-MARKER"));
    assert!(!head.contains("TAIL-MARKER"));
    assert!(head.chars().count() <= NOTES_RAM_CHARS);

    let mut active = ctx("rules", "soul");
    active.planet_line = Some("planet-a Alpha".into());
    active.notes = Some(head);
    let on = render(&build(&active));
    assert!(on.contains("HEAD-MARKER"));
    assert!(!on.contains("TAIL-MARKER"));

    let mut other = ctx("rules", "soul");
    other.planet_line = Some("planet-b Beta".into());
    other.notes = None;
    let off = render(&build(&other));
    assert!(!off.contains("HEAD-MARKER"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn recall_hits_dated_max8() {
    let root = std::env::temp_dir().join(format!("orbitty-ram-recall-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let agents = root.join("agents");
    let app = root.join("app");
    fs::create_dir_all(&agents).unwrap();
    for i in 0..10 {
        append(
            &agents,
            &ArchiveLine {
                v: 1,
                id: format!("t-{i}"),
                ts: format!("2026-10-0{}T12:00:00-05:00", (i % 9) + 1),
                role: "them".into(),
                mode: "spark".into(),
                lane: "grok".into(),
                model: "m".into(),
                session: None,
                text: format!("decided-widget number {i}"),
                src: "live".into(),
            },
        )
        .unwrap();
    }
    append(
        &agents,
        &ArchiveLine {
            v: 1,
            id: "t-miss".into(),
            ts: "2026-10-01T12:00:00-05:00".into(),
            role: "you".into(),
            mode: "spark".into(),
            lane: "grok".into(),
            model: "m".into(),
            session: None,
            text: "unrelated picnic".into(),
            src: "live".into(),
        },
    )
    .unwrap();
    write_notes(&app, "p1", "decided-widget in notes").unwrap();
    let tabs = app.join("planets").join("p1").join("tabs");
    fs::write(tabs.join("page.md"), "decided-widget in tabs\n").unwrap();
    let hits = recall(&agents, &app, "decided-widget", Some("p1"));
    assert_eq!(hits.len(), 8);
    assert!(hits.iter().all(|h| !h.date.is_empty()));
    assert!(hits.iter().all(|h| h.text.contains("decided-widget")));
    assert!(hits.iter().all(|h| !h.text.contains("picnic")));
    assert!(hits.iter().any(|h| h.source == "archive"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn checked_line_added_when_claim_without_recall() {
    use super::recall::{apply_checked, needs_checked_line};
    let claim = "We decided to ship the flag.";
    assert!(needs_checked_line(claim, false));
    assert!(!needs_checked_line(claim, true));
    assert!(!needs_checked_line("hello there", false));
    assert!(!needs_checked_line("We decided.\nChecked: terms | 2026-10-01 | 1", false));
    let stamped = apply_checked(claim, false);
    assert!(stamped.ends_with("\nChecked: none"));
    assert_eq!(apply_checked(claim, true), claim);
    assert_eq!(apply_checked("hello there", false), "hello there");
}

#[test]
fn tag_v0_roundtrips_tag_struct() {
    let tag = Tag {
        id: "t1".into(),
        name: "bug-fix".into(),
        color: Some("blue".into()),
    };
    let v0 = TagV0::from_tag(&tag, "planet:p1", "user").unwrap();
    assert_eq!(v0.name, "bug-fix");
    assert_eq!(v0.scope, "planet:p1");
    assert_eq!(v0.source, "user");
    let back = v0.to_tag();
    assert_eq!(back.id, tag.id);
    assert_eq!(back.name, tag.name);
    assert_eq!(back.color, tag.color);
    let json = serde_json::to_value(&back).unwrap();
    assert!(json.get("scope").is_none());
    assert!(json.get("source").is_none());
    assert_eq!(json["name"], "bug-fix");
    assert!(TagV0::from_tag(
        &Tag {
            id: "t2".into(),
            name: "???".into(),
            color: None,
        },
        "global",
        "user"
    )
    .is_none());
}

#[test]
fn proposals_write_nothing_until_accept() {
    let dir = std::env::temp_dir().join(format!("orbitty-ram-tags-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state.json");
    fs::write(&state, b"{\"tags\":[{\"id\":\"t1\",\"name\":\"old\"}]}").unwrap();
    let before = fs::read(&state).unwrap();
    let mut queue = Vec::new();
    let names: Vec<String> = (0..7).map(|i| format!("Hello Tag {i}")).collect();
    let draft = propose_into(&mut queue, &names, Some("p1".into())).unwrap();
    assert_eq!(draft.kind, "Approve");
    assert!(draft.title.starts_with("Approve:"));
    assert_eq!(draft.names.len(), 5);
    assert!(!draft.names.iter().any(|n| n.contains(' ')));
    assert!(queue.len() == 1);
    assert!(!dir.join("planets").join("p1").join("tags.json").exists());
    assert_eq!(fs::read(&state).unwrap(), before);

    let wrote = accept_into(&mut queue, &dir, Some("p1")).unwrap();
    assert_eq!(wrote.len(), 5);
    assert!(wrote.iter().all(|t| t.source == "accepted_proposal"));
    assert!(wrote.iter().all(|t| t.scope == "planet:p1"));
    let tags_path = dir.join("planets").join("p1").join("tags.json");
    assert!(tags_path.is_file());
    let mode = fs::metadata(&tags_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(fs::read(&state).unwrap(), before);
    assert!(!fs::read_to_string(&state).unwrap().contains("accepted_proposal"));
    assert!(queue.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn state_json_untouched() {
    let dir = std::env::temp_dir().join(format!("orbitty-ram-state-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state.json");
    let bytes = br#"{"tags":[{"id":"t9","name":"kept","color":"red"}]}"#;
    fs::write(&state, bytes).unwrap();
    let mut queue = vec![AcceptBatch {
        names: vec!["kept-name".into()],
        planet: None,
    }];
    accept_into(&mut queue, &dir, None).unwrap();
    assert_eq!(fs::read(&state).unwrap(), bytes);
    let global = fs::read_to_string(dir.join("tags").join("global.json")).unwrap();
    assert!(global.contains("accepted_proposal"));
    assert!(!global.contains("state.json"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rule_and_ask_survive_3_resets() {
    let mut turns = vec![turn("you", "session-turn")];
    for _ in 0..3 {
        turns.clear();
        let mut c = ctx("RULE-KEEP", "soul");
        c.open_asks = vec![("n1".into(), "ASK-KEEP".into())];
        c.turns = turns.clone();
        let text = render(&build(&c));
        assert!(text.contains("RULE-KEEP"), "{text}");
        assert!(text.contains("ASK-KEEP"), "{text}");
        assert!(!text.contains("session-turn"), "{text}");
    }
}

#[test]
fn flag_off_prompt_goldens_identical() {
    let gist = include_str!("../mouth_gist.rs");
    assert!(gist.contains("const GIST_CAP: usize = 800;"));
    let plug = include_str!("../../coordinator/plug.rs");
    assert!(plug.contains("const SOUL_CAP: usize = 4096;"));
    let hop = include_str!("../mouth_hop/local.rs");
    assert!(hop.contains("const HOP_TURNS: usize = 6;"));
    let wrap = include_str!("../../coordinator/intercept.rs");
    assert!(wrap.contains("pub(crate) fn wrap_with_pocket"));
    assert!(wrap.contains("USER:\\n{line}"));
    let roll = include_str!("../mouth_roll.rs");
    assert!(roll.contains("pub fn handoff_prompt("));
    let arm = roll.find("memory::enabled").expect("flag-on handoff arm");
    let flag_arm = &roll[arm..];
    let end = flag_arm.find("let (mode, turns)").expect("flag-off load");
    let flag_arm = &flag_arm[..end];
    assert!(flag_arm.contains("handoff_line"));
    assert!(!flag_arm.contains("companion.turns"));
    let hop_arm = hop.find("memory::enabled").expect("flag-on hop arm");
    let hop_body = &hop[hop_arm..];
    let end = hop_body.find("let turns =").expect("flag-off turns");
    let hop_body = &hop_body[..end];
    assert!(hop_body.contains("prompt_block"));
    assert!(!hop_body.contains("load_state"));

    let turns = vec![turn("you", "hello"), turn("them", "hi there")];
    let got = handoff_from_turns(ResetReason::ManualTest, "spark", &turns, "USER:\nping");
    let expect = "\
[handoff — fresh backend session (manual_test); this is the same desk thread (mode: spark) on a fresh session. The transcript so far is below. Answer the USER line at the end; do not greet, do not summarize.]
you: hello
them: hi there

USER:
ping";
    assert_eq!(got, expect);
    let msgs = assemble_hop_messages("SOUL", "prompt-line", "prompt-line", false, &turns, true);
    let text = serde_json::to_string(&msgs).unwrap();
    assert_eq!(
        text,
        r#"[{"content":"SOUL","role":"system"},{"content":"hello","role":"user"},{"content":"hi there","role":"assistant"},{"content":"prompt-line","role":"user"}]"#
    );
}
