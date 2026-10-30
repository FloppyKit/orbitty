# Companion memory RAM

Always-loaded core for one companion prompt. The microharness decides what is in it. Anything not in it is not in the prompt. Flag `memory_ram` defaults off. Off means the hop, handoff, and pocket prompts stay byte-identical to today.

## RAM budget

Hard cap **6,000 characters**. Last **6 turns** (same N as `HOP_TURNS`). Soul is `companion_soul_text` with its **4,096** cap unchanged. Pinned tags **≤ 12**. Planet notes contribute at most the first **1,200** characters, and only when that planet is the active one. Recall returns **≤ 8** hits. A tag proposal batch is **≤ 5** names.

Render order, stable prefix first: rules, soul, the one recall instruction, now, planet, card, pinned tags, asks, turns. Rules and soul are never cut. If the render is over the cap, drop oldest turns, then asks, then tags, then the card line, then the planet line. The same inputs render byte-identical text.

Rules text is `MODE_LAW.md` then `THIN_LAW.md` from `~/.agents/companions/orbitty-primary/` (read only). Now is `now.json`. The active planet and the active card are one line each.

## Keep rules

Always selected into RAM:

- user rules (the two law files)
- open asks (open Needed cards for the active planet; if no planet is active, every open Needed)
- pinned tags
- the active card one-liner
- an answered Needed younger than 24 hours

## Forget rules

Not selected, and never copied back into the prompt:

- closed cards (`status` done)
- answered Neededs 24 hours old or older
- turns beyond the last 6

The F1 archive keeps the turns that fell out of the window. A forgotten fact comes back only through `companion.recall`. No silent re-inject on hop or handoff.

A reply that asserts a past decision (`we decided`, `the decision`, `we agreed`, `we chose`, and the same shapes) must end with a `Checked:` line: terms, dates, hit count. If that reply had no recall in the turn, a post-check appends `Checked: none`.

## Tag index

`tag.v0`: `{id, name, color?, scope, source}`. `name` is microtags normal form. `scope` is `global` or `planet:<id>`. `source` is `user` or `accepted_proposal`.

`Tag` in `state/models_board.rs` stays `{id, name, color?}`. `tag.v0` converts to that struct and back. The extra fields live in `<app data>/tags/global.json` and `<app data>/planets/<id>/tags.json`. They are not written into `state.json`.

A model proposal sits in an in-memory queue and opens one Needed whose title starts with `Approve:`. The card's existing kind stays `needed`, so the inbox still lists it. Nothing is written to the tag files until that batch is accepted. A restart before accept drops the queue and still writes no tag file.

## Planet folder

`<app data>/planets/<project_id>/`, created lazily, same root Tabbity uses for `tabs/`:

- `notes.md` — curated notes, file ≤ 8 KB, mode 0600
- `tabs/` — Tabbity, mode 0700
- `tags.json` — planet tag index, mode 0600

Directories are mode 0700. Notes are read into RAM only while that planet is active.

## How F2b plugs in

The cold tree (F2b) is not loaded, not walked, and not part of this prompt. `recall` is the only seam: a later cold-tree store can answer the same call. It does not enter RAM and it does not change the flag-off prompts.

`memory_ram` is auto-on in Nightly after Verify PASS. The kill switch is `orbittyctl flags set memory_ram off`.
