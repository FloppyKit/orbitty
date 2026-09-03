# Orbitty

Mac habitat for **what is actually running**. A window becomes a sprite only after you claim it onto a project planet. Unclaimed work is nothing — the rail may list it; the desk stays empty.

Identifier `space.orbitty.mac`. MIT. Not an EHR: titles and metadata only, never PHI.

## The loop

1. **Claim** a window onto a named planet.
2. A **walker** lives there (open, minimized, or sleeping if the window closed).
3. Click to locate or restore. Right-click the being to edit it.
4. **Done** walks off the far rim. Goal-aligned completes can earn capped points.

That is the product. Everything else is a seat or an inspector.

## Companion is a seat

The default body is the Spirit flipbook. Orbitty does not host a model and is not a harness — launchers go into Hermes / Goose / Cursor that already exist. `three.js` loads only if you pick a 3D body; it never loads on the rail.

## Run

Dogfood is the blessed app, not `tauri dev`:

```bash
./scripts/install-stable.sh
open ~/Applications/Orbitty.app
```

First launch migrates `~/Library/Application Support/app.livesprite.mac` → `space.orbitty.mac` when the new directory is empty. Re-grant Accessibility once for `~/Applications/Orbitty.app` (sign identity stays “LiveSprite Local Codesign” so later rebuilds keep the grant).

**⌘⇧O** raises the companion. Tray left-click does the same; right-click is the remote (Companion, Launchpad, STARS, Settings, Inspector, restore stowed windows, Quit). Quit does not force-restore minimized windows — they stay in the Dock.

Drive a running app without opening a WebView:

```bash
orbittyctl status
orbittyctl windows list
```

`orbitty-mcp` is the same socket over stdio.

## Laws

`docs/THIN_SLICE_LAWS.md` — explicit claim, unmapped = zero sprites, no harness in the engine, FKT stays out, not EHR.

Accessibility, window corral, and planets are in the blessed app. Ignore any leftover “S0 scaffold” note in older papers.

How the pieces sit: `docs/ARCHITECTURE.md`.  
What is next (ops, not product): `~/.buzz/PLANS/ORBIT_ROADMAP.md`.

The Swift tree at `~/goose artifacts/livesprite` is archived. Do not launch it. Behavioral reference only: `docs/SWIFT_REFERENCE.md`.
