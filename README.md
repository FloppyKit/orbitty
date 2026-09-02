# Orbitty (Tauri)

**Living visual surface consolidating what's running — planets + claimed sprites. Seed of LifeRPG, not a harness.**

Orbitty (formerly **LiveSprite**) — Tauri 2 + Svelte 5 + TypeScript + Vite for the Mac runtime. Site: https://orbitty.space

## Swift tree = archived

The native Swift app at `~/goose artifacts/livesprite` is **deprecated** — do not launch it; open Orbitty only. See `~/goose artifacts/livesprite/SWIFT_ARCHIVE.md`. It remains the behavioral reference for window corral, stow journal, planet z-order, and theme tokens — **no dual-write feature parity**. See `docs/SWIFT_REFERENCE.md`.

## Thin-slice laws (bind all S1+ work)

- **Explicit claim only** — a window/tab becomes a sprite only after human claim.
- **Unmapped = 0 sprites** — rail may list windows; habitat stays empty of ghosts.
- **No harness in engine** — thin launchers into existing Hermes/Goose/Cursor only.
- **FKT separate** — no signer merge into this tree.
- **Not EHR** — titles/metadata only, no PHI.

Full text: `docs/THIN_SLICE_LAWS.md` → nest SoT `~/.buzz/PLANS/LIVESPRITE_THIN_SLICE_LAWS_PIXEL_v0.md`.

## Run

```bash
npm install
npm run tauri dev      # dev smoke
npm run tauri build    # release bundle (.app/.dmg in src-tauri/target/release/bundle)
```

**⌘⇧O** raises the companion; tray left-click raises the companion; right-click is the remote: **Show Companion · Launchpad · STARS · Settings · Inspector · Restore All Stowed Windows · Quit Orbitty**.

Blessed install: `scripts/install-stable.sh` → `~/Applications/Orbitty.app` (signed "LiveSprite Local Codesign"; re-grant Accessibility once for the new path). First launch migrates `~/Library/Application Support/app.livesprite.mac` → `space.orbitty.mac` when the new dir is empty.

Note: Quit does **not** force-restore minimized windows — stowed windows stay in the Dock per OS policy (same as the Swift app).

## Roadmap

`~/.buzz/PLANS/ROADMAP_LIVESPRITE_FULL_TAURI_v0.md` (S0 → S6 cutover). Identifier `space.orbitty.mac`. MIT license.

## Not in S0 (by design)

No Accessibility/window corral (S1), no sprites/planets (S3), no Android, no Swift feature work.
