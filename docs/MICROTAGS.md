# Microtags

Orbitty's tag scheme for Nostr `t` tags. This is our proposal, not a NIP.
Tabbity and the later ISSUES / PATCHES / NIP-34 work should call this module
instead of inventing a second spelling.

The only standard in force is [NIP-24](https://github.com/nostr-protocol/nips/blob/master/24.md):
a `t` tag is a hashtag and its value must be lowercase. NIP-01 indexes
single-letter tags, so `#t` filters keep working. The `namespace:value`
spelling (the colon) is Orbitty's. How other clients render a colon inside
a hashtag is unverified.

## Module

- TypeScript: `src/lib/microtags.ts`
- Rust: `src-tauri/src/microtags.rs`
- Shared rows: `qa/fixtures/microtags.json`

`normalize(raw) -> tag | null` is the whole decision. Both languages read the
same fixture.

## Normal form

ASCII only. The `unicode-normalization` crate is not in `Cargo.lock`, so
there is no NFKC step: any non-ASCII input is refused (`null`).

On ASCII input, in order:

1. Lowercase.
2. Trim.
3. Drop one leading `#`.
4. Turn spaces and underscores into `-`.
5. Collapse runs of `-` into one.
6. Keep the result only when all of these hold:
   - length 1 through 64
   - characters are `a-z`, `0-9`, `-`, and at most one `:`
   - no leading or trailing `-` (on the whole tag, and on each side of `:`)
   - a `:` has a non-empty namespace and a non-empty value

Anything else is `null`. The original card text is not rewritten.

## Namespaces

Known namespaces: `planet`, `kind`, `area`. A tag with no colon is a plain
tag (`nostr`, `bug-fix`).

| Form | Rule |
| --- | --- |
| `kind:` | value must be `bug`, `feature`, `chore`, or `question` |
| `area:` | any value that passes the normal form |
| `planet:` | any value that passes the normal form |
| other `namespace:` | still a lowercase `t` if the normal form passes; not part of the known set |
| plain | no colon |

`status:*` is reserved and refused. Status comes from the board kind
(420 todo, 421 doing, 422 done), not from a tag.

## Reserved

These exact tags are refused, after normalization. A longer tag such as
`root-cause` or `orbitty-extra` is not the reserved word and is kept.

- `orbitty`
- `local-only`
- `root`
- `root-revision`

`t=orbitty` on a board event is the relay marker. It is not a microtag.

## Board events (420, 421, 422)

Flag `microtags`, default off. Off means the board line is byte-identical
to the line written before this scheme. On, in Nightly, after Verify PASS
(`auto_on_verify`). Kill switch: `orbittyctl flags set microtags off`.

With the flag on, `append_board_proof` resolves the card's `tag_ids` to tag
names and inserts the normalized `t` tags immediately after `t=orbitty`:

1. `t=orbitty`
2. one `t` per accepted card tag, in card order, duplicates dropped
3. exactly one `t=planet:<slug>`
4. the existing `d`, `e`, `project`, `actor`, and `trace` tags

The publishable rule is unchanged: a signed 420/421/422 with `t=orbitty`
and without `t=nch`, `t=publishable`, or `t=local-only` is still publishable.

A card tag in the `planet:` namespace is not copied onto the event. The
single planet tag is the frozen slug.

### Planet slug

The slug is `normalize(project id)`, not the project display name. Renaming
the project does not change `planet:<slug>`. A project id that itself fails
`normalize` produces no planet tag.

This is the stand-in until a later ISSUES GO has a repo id to freeze.
ISSUES (1621) and PATCHES (1617/1618) are not wired here. They should call
`to_t_tags` / `toTTags` when they land.

### Refused card tags

A name that normalizes to `null` is not emitted. The process logs one line:

```
orbitty: microtag refused: <original text>
```

That note is not an event line. The card still shows the original text.

## Inbound

`inbound(raw)` returns `{ microtag, display }`.

- `display` is always the original string.
- `microtag` is `normalize(raw)`, or `null` when that refuses.

A value that fails normalization stays raw display text. It is never stored
or matched as a microtag. A value that succeeds keeps the original display
too (`#Bug Fix` stays `#Bug Fix` on screen; the `t` value is `bug-fix`).
