use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::ram::{self, AnsweredAsk, CardIn, RamCtx};
use super::tags::{accept_into, propose_into, AcceptBatch, TagV0};
use super::{build, read_notes_head, recall, render, write_notes, NOTES_FILE_BYTES, NOTES_RAM_CHARS};
use crate::mouth_archive::{append, ArchiveLine};
use crate::mouth_hop::assemble_hop_messages;
use crate::mouth_roll::{handoff_from_turns, ResetReason};
use crate::state::{CompanionTurn, Tag};

fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
    let start = src.find(sig).unwrap_or_else(|| panic!("missing {sig}"));
    let rest = &src[start..];
    let end = rest[sig.len()..]
        .find("\npub(crate) fn ")
        .map(|i| sig.len() + i)
        .unwrap_or(rest.len());
    &rest[..end]
}

fn turn(role: &str, text: impl AsRef<str>) -> CompanionTurn {
    CompanionTurn {
        role: role.into(),
        text: text.as_ref().into(),
    }
}

fn ctx(rules: &str, soul: &str) -> RamCtx {
    RamCtx {
        rules: rules.into(),
        user_rules: String::new(),
        soul: soul.into(),
        now: "job: desk".into(),
        model: String::new(),
        planet_line: Some("planet-orbitty Orbitty".into()),
        notes: None,
        cards: vec![CardIn {
            id: "card-1".into(),
            title: "OPEN-CARD".into(),
            open: true,
            active: true,
            touched: 10.0,
        }],
        card_id: Some("card-1".into()),
        pinned_tags: vec!["pinned-tag".into()],
        open_asks: vec![("n1".into(), "ASK-KEEP".into())],
        answered: Vec::new(),
        turns: vec![turn("you", "TURN-VOL")],
        now_secs: 1_000_000.0,
    }
}

fn answered(id: &str, ask: &str, turns_since: usize, answer_id: &str, answer: &str, at: f64) -> AnsweredAsk {
    AnsweredAsk {
        id: id.into(),
        ask: ask.into(),
        at,
        turns_since,
        answer_id: answer_id.into(),
        answer: answer.into(),
    }
}

#[test]
fn ram_cap_6000_truncation_order() {
    let mut big = ctx("RULES-NEVER-CUT", "SOUL-NEVER-CUT");
    big.open_asks = vec![("a1".into(), "A".repeat(20_000))];
    big.pinned_tags = vec!["tag-keep".into()];
    big.turns = vec![
        turn("you", format!("TURN-OLD-{}", "x".repeat(20_000))),
        turn("them", "TURN-NEW"),
    ];
    let text = render(&build(&big));
    assert!(text.contains("RULES-NEVER-CUT"));
    assert!(text.contains("SOUL-NEVER-CUT"));
    assert!(!text.contains("TURN-OLD-"), "oldest turn drops first\n{text}");
    assert!(text.contains("tag-keep"), "pins are outside the budget\n{text}");
    assert!(
        !text.contains(&"A".repeat(100)),
        "asks drop after turns\n{text}"
    );
    assert!(text.contains("more asks: a1"), "trimmed ask id stays\n{text}");

    let mut rules = "RULE-START-".to_string();
    rules.push_str(&"R".repeat(7_000));
    rules.push_str("-RULE-END");
    let mut soul = "SOUL-START-".to_string();
    soul.push_str(&"S".repeat(5_000));
    soul.push_str("-SOUL-END");
    let mut over = ctx(&rules, &soul);
    over.turns = vec![turn("you", "Z".repeat(20_000))];
    over.open_asks = vec![("a1".into(), "A".repeat(20_000))];
    let text = render(&build(&over));
    assert!(text.contains("RULE-START-"));
    assert!(text.contains("-RULE-END"));
    assert!(text.contains("SOUL-START-"));
    assert!(text.contains("-SOUL-END"));
    assert!(!text.contains(&"Z".repeat(100)));
    assert!(!text.contains(&"A".repeat(100)));
    assert!(text.chars().count() > 6_000);
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
        id: "closed".into(),
        title: "CLOSED-SECRET".into(),
        open: false,
        active: true,
        touched: 99.0,
    });
    c.answered = vec![
        answered("old", "STALE-SECRET", 7, "ans-old", "STALE-BODY", 1_000_000.0),
        answered(
            "fresh",
            "FRESH-ASK",
            2,
            "ans-fresh",
            "",
            1_000_000.0 - (25.0 * 3600.0),
        ),
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

    let core = super::fit(&build(&c));
    let state_turns = c.turns.clone();
    let handoff = super::handoff_from_core(
        ResetReason::ManualTest,
        "spark",
        &core,
        &state_turns,
        "USER:\nping",
    );
    assert!(!handoff.contains("turn-0"), "{handoff}");
    assert!(!handoff.contains("turn-1"), "{handoff}");
    assert!(handoff.contains("turn-2"), "{handoff}");
    assert!(handoff.contains("turn-7"), "{handoff}");
    // handoff_from_turns keeps only the last 6, so this prefix is what makes
    // an 8-turn re-inject visible. If it disappears, the lock above goes blind.
    let raw = super::handoff_visible(
        ResetReason::ManualTest,
        "spark",
        &state_turns,
        "USER:\nping",
    );
    assert!(
        raw.contains("turn-0"),
        "8-turn slice must show turn-0 or the handoff lock cannot see a re-inject\n{raw}"
    );

    let src = include_str!("mod.rs");
    let line_fn = fn_body(src, "pub(crate) fn handoff_line(");
    assert!(
        line_fn.contains("handoff_from_core"),
        "handoff_line must go through handoff_from_core"
    );
    assert!(
        !line_fn.contains("handoff_from_turns"),
        "handoff_line must not pass turns straight to handoff_from_turns"
    );
    let core_fn = fn_body(src, "pub(crate) fn handoff_from_core(");
    assert!(
        core_fn.contains("let turns = &core.turns;"),
        "handoff must speak RAM turns, not the saved transcript"
    );
    assert!(
        core_fn.contains("handoff_visible(reason, mode, turns,"),
        "handoff_visible must receive the RAM slice"
    );
    assert!(
        !core_fn.contains("state_turns,"),
        "state_turns must not be passed into the handoff body"
    );
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
    use super::recall::{apply_checked, clear_cite, needs_checked_line};
    let claim = "We decided to ship the flag.";
    assert!(needs_checked_line(claim, false));
    assert!(!needs_checked_line(claim, true));
    assert!(!needs_checked_line("hello there", false));
    assert!(!needs_checked_line("We decided.\nChecked: terms | 2026-10-01 | 1", false));
    let stamped = apply_checked(claim, false);
    assert!(stamped.ends_with("\nChecked: none"));
    assert_eq!(apply_checked(claim, true), claim);
    assert_eq!(apply_checked("hello there", false), "hello there");

    clear_cite();
    let stamped = super::finish_reply(&true, claim);
    assert!(
        stamped.ends_with("\nChecked: none"),
        "finish_reply must stamp a claim with no recall\n{stamped}"
    );
    clear_cite();
    assert_eq!(super::finish_reply(&false, claim), claim);
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
    let draft = propose_into(&mut queue, &dir, &names, Some("p1".into())).unwrap();
    assert_eq!(draft.kind, "Approve");
    assert!(draft.title.starts_with("Approve:"));
    assert_eq!(draft.names.len(), 5);
    assert!(!draft.names.iter().any(|n| n.contains(' ')));
    assert!(queue.len() == 1);
    assert!(!dir.join("planets").join("p1").join("tags.json").exists());
    assert_eq!(fs::read(&state).unwrap(), before);
    let tags_src = include_str!("tags.rs");
    for sig in [
        "pub(crate) fn propose_live(",
        "pub(crate) fn propose_into(",
    ] {
        let body = fn_body(tags_src, sig);
        assert!(!body.contains("write_0600"), "{sig} must not write");
        assert!(!body.contains("std::fs::write"), "{sig} must not write");
        assert!(!body.contains("fs::write"), "{sig} must not write");
    }

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

fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "orbitty-ram-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

struct ArchiveReset;
impl Drop for ArchiveReset {
    fn drop(&mut self) {
        crate::mouth_archive::install_root(None);
        crate::mouth_archive::force_flag(None);
    }
}

#[test]
fn rules_never_trimmed_even_over_budget() {
    let mut c = ctx("law", "soul");
    c.user_rules = format!("USER-RULE-{}", "u".repeat(40_000));
    c.turns = vec![turn("you", "t".repeat(40_000))];
    let text = render(&build(&c));
    assert!(text.contains("USER-RULE-"));
    assert!(text.contains(&"u".repeat(100)));
    assert!(!text.contains(&"t".repeat(100)));
}

#[test]
fn pins_never_trimmed() {
    let mut c = ctx("rules", "soul");
    c.pinned_tags = vec![format!("PIN-ALWAYS-{}", "p".repeat(8_000))];
    c.turns = vec![turn("you", "t".repeat(40_000))];
    c.open_asks = vec![("a".into(), "q".repeat(40_000))];
    let text = render(&build(&c));
    assert!(text.contains("PIN-ALWAYS-"));
    assert!(text.contains(&"p".repeat(100)));
}

#[test]
fn rule_set_replace_rm_roundtrip() {
    let dir = temp_dir("rules");
    let state = dir.join("state.json");
    fs::write(&state, b"{\"tags\":[]}").unwrap();
    let before = fs::read(&state).unwrap();
    let row = super::rules::set(&dir, None, "Always answer in CT times", "user").unwrap();
    assert!(row.id.starts_with("ur-"));
    assert_eq!(row.source, "user");
    let path = super::rules::rules_path(&dir);
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let folder = fs::metadata(dir.join("companion")).unwrap().permissions().mode() & 0o777;
    assert_eq!(folder, 0o700);
    let replaced = super::rules::set(&dir, Some(&row.id), "Always answer in CT", "user").unwrap();
    assert_eq!(replaced.id, row.id);
    assert_eq!(replaced.text, "Always answer in CT");
    assert_eq!(super::rules::list(&dir).len(), 1);
    assert!(super::rules::rm(&dir, &row.id).unwrap());
    assert!(super::rules::list(&dir).is_empty());
    assert_eq!(fs::read(&state).unwrap(), before);
    assert!(!fs::read_to_string(dir.join("companion").join("rules.json"))
        .unwrap()
        .contains("state.json"));
    let mut queue = Vec::new();
    let draft = super::rules::propose_into(&mut queue, "Propose CT", None, false).unwrap();
    assert_eq!(draft.kind, "Approve");
    assert!(draft.title.starts_with("Approve:"));
    assert_eq!(super::rules::list(&dir).len(), 0);
    let wrote = super::rules::accept_into(&mut queue, &dir).unwrap();
    assert_eq!(wrote["source"], "accepted_proposal");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rule_survives_restart() {
    let dir = temp_dir("restart");
    let row = super::rules::set(&dir, None, "SURVIVE-RESTART", "user").unwrap();
    let again = super::rules::list(&dir);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, row.id);
    assert_eq!(again[0].text, "SURVIVE-RESTART");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rule_survives_3_real_rolls() {
    let app = temp_dir("rolls-app");
    let agents = temp_dir("rolls-agents");
    super::rules::set(&app, None, "RULE-ROLL-KEEP", "user").unwrap();
    crate::mouth_archive::install_root(Some(agents.clone()));
    crate::mouth_archive::force_flag(Some(true));
    let _reset = ArchiveReset;
    let plug_path = crate::coordinator::plug_path();
    let saved = fs::read(&plug_path).ok();
    let run = std::panic::catch_unwind(|| {
        for i in 0..3 {
            {
                let mut plug = crate::coordinator::load_plug();
                plug.reset_pending = None;
                crate::coordinator::save_plug(&plug);
            }
            crate::mouth_archive::arm_manual_roll().unwrap();
            let plug = crate::coordinator::load_plug();
            assert_eq!(
                plug.reset_pending.as_deref(),
                Some("manual_test"),
                "arm_manual_roll must set the pending reason the send path reads"
            );
            let armed = crate::mouth_roll::consume_armed_reason(
                plug.reset_pending.as_deref(),
                plug.grok_chars,
                crate::mouth_archive::session_budget(),
            )
            .expect("send path must consume the armed roll");
            assert_eq!(armed.reason(), ResetReason::ManualTest);
            // Do not assign user_rules. The seam reads rules.json the same
            // way core_for_app does, and it only accepts ArmedRoll.
            let handoff = super::handoff_after_armed_roll(&app, armed, "spark", "USER:\nnext");
            assert!(handoff.contains("RULE-ROLL-KEEP"), "{handoff}");
            assert!(
                handoff.contains("manual_test"),
                "handoff must carry the consumed roll reason\n{handoff}"
            );
            let send = include_str!("../../coordinator/send.rs");
            assert!(send.contains("consume_armed_reason"));
            assert!(send.contains("armed.reason()"));
            assert!(send.contains("handoff_prompt"));
            let ram = include_str!("ram.rs");
            let core_fn = fn_body(ram, "pub(crate) fn core_for_app(");
            assert!(
                core_fn.contains("user_rules_for_handoff"),
                "core_for_app must load rules through the handoff seam"
            );
            let seam = fn_body(include_str!("mod.rs"), "pub(crate) fn handoff_after_armed_roll(");
            assert!(seam.contains("user_rules_for_handoff"));
            assert!(seam.contains("handoff_from_core"));
            let mut st = crate::state::CompanionState::default();
            st.push_turn("you", &format!("roll-turn-{i}"));
        }
    });
    match saved {
        Some(bytes) => fs::write(&plug_path, bytes).unwrap(),
        None => {
            let _ = fs::remove_file(&plug_path);
        }
    }
    if let Err(payload) = run {
        std::panic::resume_unwind(payload);
    }
    let _ = fs::remove_dir_all(&app);
    let _ = fs::remove_dir_all(&agents);
}

#[test]
fn budget_respects_tokens() {
    assert_eq!(ram::budget_for_window(8_000), 1_500);
    assert_eq!(ram::budget_for_window(32_000), 2_560);
    assert_eq!(ram::budget_for_window(200_000), 12_000);
    assert_eq!(ram::budget_for_window(2_000_000), 12_000);
    assert_eq!(ram::budget_tokens(""), 2_560);
    assert_eq!(ram::budget_tokens("?"), 2_560);
    assert_eq!(ram::budget_tokens("grok-4.7-high"), 12_000);
    assert_eq!(ram::budget_tokens("claude-sonnet-4"), 12_000);
    assert_eq!(ram::budget_tokens("google/gemma-4-e4b"), 1_500);
    assert_eq!(ram::est_tokens("abcd"), 1);
    assert_eq!(ram::est_tokens("abcde"), 2);
    for (window, budget) in [
        (8_000usize, 1_500usize),
        (32_000, 2_560),
        (200_000, 12_000),
        (2_000_000, 12_000),
    ] {
        let mut c = ctx(&"R".repeat(20_000), "soul");
        c.user_rules = "USER-RULE-ALWAYS".into();
        c.pinned_tags = vec!["PIN-ALWAYS".into()];
        c.turns = vec![turn("you", "T".repeat(80_000))];
        c.open_asks = vec![("ax".into(), "Q".repeat(80_000))];
        let core = build(&c);
        let fitted = ram::fit_to(&core, budget);
        let text = ram::render_with_budget(&core, budget);
        assert!(
            ram::est_tokens(&ram::trimmable_text(&fitted)) <= budget,
            "window {window}"
        );
        assert!(text.contains("USER-RULE-ALWAYS"), "{window}");
        assert!(text.contains("PIN-ALWAYS"), "{window}");
        assert!(text.contains(&"R".repeat(100)), "{window}");
    }
    let mut c = ctx("rules", "soul");
    c.open_asks.clear();
    c.turns = vec![turn("you", format!("KEEP-TOKENS-{}", "k".repeat(5_000)))];
    let text = ram::render_with_budget(&build(&c), 1_500);
    assert!(
        text.contains("KEEP-TOKENS-"),
        "5000 chars is 1250 tokens and must stay under a 1500 token budget\n{text}"
    );
}

#[test]
fn answered_ask_ages_after_6_turns_not_hours() {
    let mut c = ctx("rules", "soul");
    c.answered = vec![answered("a", "KEEP-SIX", 6, "ans", "BODY-SIX", 1.0)];
    let text = render(&build(&c));
    assert!(text.contains("BODY-SIX"), "{text}");
    c.answered = vec![answered("a", "DROP-SEVEN", 7, "ans", "BODY-SEVEN", 1.0)];
    let text = render(&build(&c));
    assert!(!text.contains("BODY-SEVEN"), "{text}");
    assert!(text.contains("aged: a -> ans"), "{text}");
    c.answered = vec![answered(
        "a",
        "HOURS-STILL",
        2,
        "ans",
        "BODY-HOURS",
        1_000_000.0 - (25.0 * 3600.0),
    )];
    c.now_secs = 1_000_000.0;
    let text = render(&build(&c));
    assert!(text.contains("BODY-HOURS"), "{text}");
}

#[test]
fn aged_answer_exact_after_7_turns() {
    let app = temp_dir("exact-app");
    let agents = temp_dir("exact-agents");
    crate::mouth_archive::install_root(Some(agents.clone()));
    crate::mouth_archive::force_flag(Some(true));
    let _reset = ArchiveReset;
    let mut st = crate::state::CompanionState::default();
    st.push_turn("them", "EXACT-ANSWER-TEXT");
    let answer_id = crate::mouth_archive::last_archive_id(&agents);
    assert!(!answer_id.is_empty(), "push_archived must mint an id");
    let mut c = ctx("rules", "soul");
    c.answered = vec![answered("ask-1", "did we decide", 0, "", "", 1.0)];
    let first = super::settle(&app, &agents, &c);
    let text = render(&first);
    assert!(text.contains("EXACT-ANSWER-TEXT"), "{text}");
    for i in 0..7 {
        st.push_turn("you", &format!("later-{i}"));
    }
    let fitted = super::settle(&app, &agents, &c);
    let text = render(&fitted);
    assert!(!text.contains("EXACT-ANSWER-TEXT"), "{text}");
    let (node, arch) = super::graph::get(&app, &agents, &answer_id).expect("graph node");
    assert_eq!(arch.expect("archive text").text, "EXACT-ANSWER-TEXT");
    assert!(
        node.links.iter().any(|l| l.rel == "answers" && l.to == "ask-1"),
        "{node:?}"
    );
    let neighbors = super::graph::links(&app, "ask-1", Some("answers"));
    assert!(neighbors.iter().any(|l| l.to == answer_id), "{neighbors:?}");
    let lines = super::graph::line_count(&app);
    let _ = super::settle(&app, &agents, &c);
    assert_eq!(super::graph::line_count(&app), lines, "graph writes are idempotent");
    let _ = fs::remove_dir_all(&app);
    let _ = fs::remove_dir_all(&agents);
}

#[test]
fn graph_write_idempotent() {
    let app = temp_dir("graph");
    let node = super::graph::Node {
        id: "t-1".into(),
        kind: "turn".into(),
        ts: "2026-10-31T00:00:00Z".into(),
        archive_id: Some("t-1".into()),
        planet: None,
        card: None,
        links: vec![super::graph::Link {
            rel: "next_turn".into(),
            to: "t-2".into(),
        }],
    };
    super::graph::note_aged(&app, &node).unwrap();
    super::graph::note_aged(&app, &node).unwrap();
    assert_eq!(super::graph::line_count(&app), 1);
    let mode = fs::metadata(super::graph::graph_path(&app)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(&app);
}

#[test]
fn graph_links_answer_to_ask() {
    let app = temp_dir("links");
    super::graph::note_aged(
        &app,
        &super::graph::Node {
            id: "ans-9".into(),
            kind: "answer".into(),
            ts: "2026-10-31T00:00:00Z".into(),
            archive_id: Some("ans-9".into()),
            planet: None,
            card: None,
            links: vec![
                super::graph::Link {
                    rel: "answers".into(),
                    to: "ask-9".into(),
                },
                super::graph::Link {
                    rel: "asked_in".into(),
                    to: "ans-9".into(),
                },
            ],
        },
    )
    .unwrap();
    let links = super::graph::links(&app, "ask-9", Some("answers"));
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].to, "ans-9");
    let _ = fs::remove_dir_all(&app);
}

#[test]
fn open_ask_trimmed_leaves_id_stub() {
    let mut c = ctx("rules", "soul");
    c.open_asks = vec![
        ("ask-old".into(), "Q".repeat(40_000)),
        ("ask-new".into(), "short ask".into()),
    ];
    c.turns.clear();
    let text = render(&build(&c));
    assert!(!text.contains(&"Q".repeat(100)), "{text}");
    assert!(text.contains("more asks: ask-old"), "{text}");
    assert!(text.contains("ask-new"), "{text}");
}

#[test]
fn recall_archive_hit_carries_id() {
    let root = temp_dir("recall-id");
    let agents = root.join("agents");
    let app = root.join("app");
    fs::create_dir_all(&agents).unwrap();
    append(
        &agents,
        &ArchiveLine {
            v: 1,
            id: "arc-keep".into(),
            ts: "2026-10-31T12:00:00-05:00".into(),
            role: "them".into(),
            mode: "spark".into(),
            lane: "grok".into(),
            model: "grok-4.7".into(),
            session: None,
            text: "decided-widget exact".into(),
            src: "live".into(),
        },
    )
    .unwrap();
    let hits = recall(&agents, &app, "decided-widget", None);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "arc-keep");
    let _ = fs::remove_dir_all(&root);
}
