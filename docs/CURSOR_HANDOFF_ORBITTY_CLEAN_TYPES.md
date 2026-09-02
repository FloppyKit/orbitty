# CURSOR HANDOFF — GO-ORBITTY-CLEAN-TYPES

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_CLEAN_TYPES_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Native shape SoT: `src-tauri/src/state.rs`.

## Do

1. Add `src/lib/persist.ts` with the **full** persist unions (every key rail + companion + stars + sky + drawers + Needed/actor + sprite). Mirror `state.rs`. Do not drop optionals.

2. Point the eight persist pages at it. Delete their duplicate `PersistedState` / shared claim-card types. Keep page-local UI types.

3. `mutate` / save still writes the whole object.

4. `./scripts/install-stable.sh`. `git add` the named files. `git commit -m "GO-ORBITTY-CLEAN-TYPES"`. No push. STOP.

## Do not

`tauri dev`. Stamp. Rust. Rename keys. Caption leftover. Generate-from-bindgen.
