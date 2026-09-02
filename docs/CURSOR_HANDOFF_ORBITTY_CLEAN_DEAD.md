# CURSOR HANDOFF — GO-ORBITTY-CLEAN-DEAD

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_CLEAN_DEAD_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. Confirm no live import of `kenney-directory` / `itch-directory` under `src/` and `src-tauri/` (ignore `docs/`).

2. Delete `src/lib/kenney-directory.json` and `src/lib/itch-directory.json`.

3. `./scripts/install-stable.sh`. Then `git add` those deletions + `.gitignore` if needed, `git commit -m "GO-ORBITTY-CLEAN-DEAD"`. Do not push. STOP.

## Do not

`tauri dev`. Stamp. Rust. CompanionPage. Replacement catalog.
