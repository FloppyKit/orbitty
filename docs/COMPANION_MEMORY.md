# Companion memory RAM

Always-loaded core for one companion prompt. The microharness decides what is in it. Anything not in it is not in the prompt. Flag `memory_ram` defaults off. Off means the hop, handoff, and pocket prompts stay byte-identical to today. This GO does not flip the flag.

## RAM budget

The trimmable section is recent turns, open asks, in-window answers, aged stubs, and the active planet and card lines. Its budget is tokens, not characters:

`budget_tokens = clamp(floor(0.08 * ctx_window_tokens), 1_500, 12_000)`.

`ctx_window_tokens` comes from a prefix table in `ram.rs`, keyed by the active companion model (`reply_model`, else `direct_model`). Unknown, empty, or `?` is 32,000 tokens, so the budget is 2,560. Known rows: `grok-4` 256k, `claude` 200k, `gpt-5` 400k, `gpt-4.1` 1M, `gpt-4o` / `gpt-` 128k, local Gemma / Ollama 8k, Goose / Hermes 32k. Token count is `ceil(bytes / 4)` (`est_tokens`). No tokenizer.

Outside the budget, always, on every flag-on prompt: law files, user rules, soul, the recall line, the rule-change line, now, and pins. No truncation path may cut them. Law files stay read-only (`MODE_LAW.md`, `THIN_LAW.md`).

If the trimmable section is over the budget, drop aged stubs, then oldest turns, then in-window answers, then oldest asks, then the card line, then the planet line. A trimmed open ask stays as one line: `more asks: id1, id2`. An aged answer stays as `aged: <ask id> -> <answer id>` only while the budget allows.

Soul is `companion_soul_text` with its **4,096** cap unchanged. Pinned tags **≤ 12**. Planet notes contribute at most the first **1,200** characters, and only when that planet is the active one. Recall returns **≤ 8** hits. A tag proposal batch is **≤ 5** names. Last **6** turns are the window before the budget trim (`TURN_N`).

## Keep rules

Always selected, and never trimmed:

- law files and user rules
- pinned tags
- open asks (trimmed by budget only, oldest first; the id stub remains)
- an answered Needed for 6 companion turns after the answer, not 24 hours

## Forget rules

Leave RAM, and stay findable in the graph:

- answered Neededs after 6 companion turns (`ANSWERED_TTL_TURNS`)
- turns past the last 6, and turns the budget drops
- asks the budget drops (id stub, then the graph)

Hours do not age an answer. Two turns at 25 hours is still in RAM.

## User rules

`<app data>/companion/rules.json`, mode 0600, directory 0700. Rows are `{id, text, created_ts, updated_ts, source}` with `source` `user` or `accepted_proposal`. Not a `state.json` field. Writes are tmp + rename.

The user changes a rule only by:

- `companion.rule_set` `{text, id?}` adds, or replaces when the id matches
- `companion.rule_rm` `{id}`
- `companion.rule_list` reads

The same three verbs are on orbittyctl (`rule_set` / `rule-set`, `rule_rm`, `rule_list`) and orbitty-mcp. In chat the companion may only propose: `companion.rule_propose` opens one Approve needed and writes nothing until `companion.rule_accept`. Model output never edits or drops a rule on its own.

Pins use the existing accept path plus `companion.tags_unpin` `{name, planet?}`.

## Graph

`<app data>/companion/graph.jsonl`, append-only, mode 0600. One JSON node per new link: `{id, kind, ts, archive_id?, planet?, card?, links:[{rel, to}]}`. `kind` is `rule|ask|answer|card|planet|turn`. `rel` is `answers|asked_in|about_card|on_planet|next_turn|set_by_turn`. The same `id+rel+to` does not append again.

`companion.get` `{id}` returns the node, its links, and the archive text when the id is a turn or answer (`mouth_archive::by_id`). `companion.links` `{id, rel?}` returns neighbors. Both are reads, on orbittyctl and orbitty-mcp. `companion.recall` is unchanged and still the fuzzy fallback; archive hits carry the archive id so a follow-up `get` is exact.

## Tag index

`tag.v0`: `{id, name, color?, scope, source}`. `name` is microtags normal form. `scope` is `global` or `planet:<id>`. `source` is `user` or `accepted_proposal`.

`Tag` in `state/models_board.rs` stays `{id, name, color?}`. The extra fields live in `<app data>/tags/global.json` and `<app data>/planets/<id>/tags.json`. They are not written into `state.json`.

A model proposal sits in an in-memory queue and opens one Needed whose title starts with `Approve:`. Nothing is written to the tag files until that batch is accepted.

## Planet folder

`<app data>/planets/<project_id>/`, created lazily:

- `notes.md` — curated notes, file ≤ 8 KB, mode 0600
- `tabs/` — Tabbity, mode 0700
- `tags.json` — planet tag index, mode 0600

Directories are mode 0700.

## How F2b plugs in

The cold tree is not loaded. `recall` is the fuzzy seam. Exact lookup is `companion.get`. Neither enters the flag-off prompts.

`memory_ram` stays default off. The kill switch is `orbittyctl flags set memory_ram off`.
