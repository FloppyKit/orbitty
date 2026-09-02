# Cursor Ultra — GO-ORBITTY-SLEEPERS

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
Do **not** edit Swift `~/goose artifacts/livesprite`.
Do **not** touch companion, marble-drift, nest, Dock.

**Lock:** `~/.buzz/PLANS/ORBITTY_SLEEPERS_LOCK_v0.md`  
**Acceptance:** `docs/ACCEPTANCE-ORBITTY-SLEEPERS.md`

Today rematch **keeps** unmatched claims but `rowsFor` only lists **live** `windows[]`, and `syncHabitat` only those rows — so close = vanish. That is the bug.

## Rail
- `rowsFor` includes **orphan claims** for that project (window id not in `windows`). Synthesize a row from the claim fingerprint: `id = claim.windowId`, title = override or `titleAtClaim` or appName, icon if you still have one (ok if null), `isMinimized: true`, mark sleeper in the row (dim / “asleep” — obvious, not a tiny glyph-only).
- Click row → `wake_sleeper` (below), not a silent no-op.
- **Done** circle still completes (unclaim + card done). Do not Complete on close.
- Persist new claim fields additively. Old saves load.

## Habitat
- Snapshot includes orphans. Add `asleep: bool` on `HabitatEntry` (default false).
- Sleeping walker: idle frame 0, dim, no bounce. Same Bitty / pack as the claim. Do **not** omit for age `gone`.
- Double-click sleeper → `wake_sleeper(id)` (dead `pid:cgid` will fail `restore_window` — don’t only call that).
- Single-click still caption only.

## Stamp (claim + rematch when live)
Additive on `Claim`:
- `filePath?: string` — AX document if `file://` and path exists
- `hermesProfile?: string`, `hermesSession?: string` — only for Hermes-class apps; session = AX title if non-empty; do not invent companion thread names
- `actionId?: string` optional
- `command?`, `cwd?` optional (Terminal-class if AX gives a cwd)

Implement AX document read in `windows_mac.rs` if missing (titles only, no file *contents*).

## `wake_sleeper(id)`
Look up claim by `windowId == id` (or rematched live id):
1. If live window exists → `restore_window` / focus
2. Else if `filePath` exists on disk → `open` that path
3. Else if Hermes session stamped → `hermes -p <profile>` if set, `--continue <session>` (no `--create-if-missing` unless this claim was created from a named Agent shortcut)
4. Else if `actionId` or command+cwd → run that Action (existing Actions runner)
5. Else if `bundleId` → `open -b`
6. Else `Err` one honest sentence

Find `hermes` via PATH + `~/.local/bin` (same as companion_send).

## Forbidden
`cargo build` into Orbitty.app · Dock aliases folder · Chrome tab dump · deleting orphans · companion · STAMP_*

## Verify
`./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
Summarize files. Pixel stamps after eyes-on.
