# CURSOR HANDOFF — GO-ORBITTY-DEST-STRIP

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_DEST_STRIP_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — dests in agentrow spacer + gear only

## Do

1. Today's `.agentrow` is `agentpick` · Prompts (spark) · empty `.spacer` · Speak · View · Expand. Put **Board · STARS · Planets** in that spacer, between spark and speaker. Replace the empty `<span class="spacer">` with `<span class="dests">` (`flex:1`) holding three text buttons class `dest`. Same tree collapsed and expanded.
2. Board = `invoke("open_kanban")`. STARS = `invoke("open_stars")`. Planets = `invoke("bring_all_front")`. These already exist. `open_kanban`/`open_stars` share the `kanban` overlay — do not spawn `"stars"`.
3. ⚙ top-right (`right: 10px`), mirror `.winx` top-left. `invoke("open_settings_window")`. `onmousedown` `stopPropagation`. Class `gear`. Glyph `⚙` is fine (rail uses it).
4. Do **not** add a `.destrow` between `.agentrow` and `.composer`. Do not restyle spark/speak/view/expand. No Actions dest. No Agents dest. No slide-under-field. Do not wrap dests; shrink dest type if 280px is tight — never drop a dest.
5. Rail chrome-stack and rail gear stay (INSPECTOR).
6. `./scripts/install-stable.sh`; stop.

## Do not

Park. Planets popup. Rail stack delete. `tauri dev`. Stamp. Fold PARK. Invent `.destrow`.
