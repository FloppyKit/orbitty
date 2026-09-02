# CURSOR HANDOFF — GO-ORBITTY-HOST-VIEW

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_HOST_VIEW_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — only.

## Do

1. Change `headShowing` to:

   `viewPref === "head" && bustOk && !missing`

   Delete the `|| (hosted !== "off" && !expanded)` term. Update the HOST-SPLIT comment so it no longer says the hosted column is always Head.

2. Do not rewrite `toggleView`. Do not touch `.wellhead` `{#if !expanded}` (OVERLAY-HEAD).

3. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. Rust. Touch `set_companion_host` / clamp / drawers. Fold HOST-FIT / action-edit / CLEAR.
