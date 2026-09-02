# CURSOR HANDOFF — GO-ORBITTY-LP-MINE

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_LP_MINE_LOCK_v0.md` first. **Illegal until LP-ACTOR dogfood.**

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/KanbanPage.svelte` only
- No `state.rs` unless a comment. No Companion. No Rail restyle.

## Do

1. **List view:** if the filtered set has both `kind: "window"` and `kind: "todo"`, two quiet sections — **On this planet** (window; heading **Inbox** when `selected === "inbox"`) then **My list** (todos). Skip empty heading.
2. **Board view:** still three columns. Inside a column, window cards above todos. No extra column. No extra banner tab.
3. Add-a-todo still `kind: "todo"`. Do not touch rail claim.
4. Today / Done honor the same split if mixed; Done still honors actor chips.
5. `./scripts/install-stable.sh`; stop.

## Do not

Fourth column. Dest chrome. Agents well. Talk. Stamp. Fold #4. Shame counts. `tauri dev`.
