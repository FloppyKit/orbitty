# Cursor Ultra — GO-ORBITTY-RAIL-ROW-MENU

**Model:** `kimi-k3-max` · **one writer**
**Gate:** waiting Matt phrase `GO-ORBITTY-RAIL-ROW-MENU`. **Do not code until issued.**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_RAIL_ROW_MENU_LOCK_v0.md` + this handoff

Matt 22:09. Folds walker-context. Do **not** also run `GO-ORBITTY-WALKER-CONTEXT`.

## Ship — rail (`src/routes/+page.svelte`)

### Click matrix on `winRow`

- **Left-click** (mouseup, no drag): `locate(w)` — glow. **Not** `focus`. **Not** `wakeSleeper`.
- **Double-click**: open → `stow` / `minimize_window`. Minimized → `restore`. Sleeper → `wakeSleeper`.
- **Right-click** title/icon: row menu (replace today’s View-on-board / Rename-only ctx).
- **Drag**: unchanged. If the pointer moved, left-click does not locate.

Detect double-click with `clickCount` / a short timer matching `NSEvent.doubleClickInterval` if you can read it; else ~300ms. **Glow on the first down/up immediately** — do not wait to see if a second click comes.

### Kill row chrome

Delete: pencil `rename-btn`, stow `stow-btn`, folder/+ `claim-btn`, the whole `claim-menu` popover, ctx **View on board**.

Keep: `rowwrap` drag, title+icon button, **circle Complete** (claimed only).

### Right-click menu items

1. **Assign to…** submenu: each project (`claimTo`); new-project field + Claim (`createAndClaim`).
2. **Rename** → `startRename`.
3. **Minimize** — shown if `!w.isMinimized && !sleeping`. Claimed: `stow`. Unmapped: `minimize_window` (Dock, no planet).
4. **Restore** — shown if `w.isMinimized && !sleeping` → `restore`. Never both Minimize and Restore.
5. **Close window** — shown if `!sleeping` → invoke existing `close_window`. **Not** `completeWindow`. Claimed becomes sleeper. Swallow AX refusal.
6. separator + **Unclaim** — claimed only → `unclaim`. Label may be `× Unclaim`. No Unclaim on Unmapped.

Sleepers: Assign, Rename, Unclaim (if claimed — they are). No Minimize / Restore / Close.

### Locate

New command `locate_window(id)` in `windows_mac.rs` (+ stub). Read `AXPosition` + existing `AXSize`. Draw a borderless, ignore-mouse, non-activating NSPanel ring (2–3px, accent or white ~0.85) around that rect for ~500ms, then orderOut/close. Add `AXPosition` to `AxNames` if missing (`ax_point` parallel to `ax_size`).

Walker: emit `habitat:locate` `{ entryId }` to habitat. Habitat strokes `being.frame` ~500ms (same ignore-mouse idea — just a draw flag, no extra panel required on the planet).

Sleeper: if a walker still exists, habitat locate; else CSS flash the **row** (~500ms outline).

`locate` never AX-raises, never `focus_window`.

## Ship — walker (`habitat_mac.rs`) — folded walker-context

### Hit test `right_mouse_down_inner`

Today: `over_disc` then planet NSMenu.

**Before** the planet menu:

```
hit_being(planet, local)
  walker true  → walker NSMenu; return
  walker false → fall through (dots do not get this menu)
over_disc      → planet menu (unchanged)
else           → return
```

### Walker NSMenu (`popUpContextMenu`, same pattern as planet menu)

1. Use my image…
2. Folder…
3. separator
4. Generated — check if `spritePackId` empty
5. one item per walker pack (`role` not companion/habitat) — check current
6. separator
7. Open being… → existing `habitat:being`

Emit to `main` e.g. `habitat:walker-pack`:

| Payload | Rail |
|---------|------|
| `{ entryId, kind: "image" }` | `import_pack_image` then `setSpritePack` |
| `{ entryId, kind: "folder" }` | `import_pack_folder` then `setSpritePack` |
| `{ entryId, packId: null }` | `setSpritePack(entryId, null)` |
| `{ entryId, packId }` | `setSpritePack` |

Prefer emit-then-webview-dialog over AppKit dialog from the menu action. No zip. Magenta is a color. Do **not** write claims from Rust.

`beingImportImage` / `beingImportFolder` take an **id** so import works with the editor closed.

### Walker left-click

**Change:** walker single-click is **locate**, not deferred Being editor.

- First click: emit `habitat:locate` (or stroke immediately in habitat) + tell rail to locate the matching row if you want; **do not** arm `deferred_edit` → `habitat:being`.
- Double-click: still restore (already cancels deferred_edit). Keep that.
- Open being… on the NSMenu is how you get the editor.

Dots (open-window beings): left-click still whatever they do today unless it fights locate; do not give them the walker menu.

### Strip Image

- Being editor: **remove** Image label + Use my image / Folder / clear. Keep Kind, Palette, Size, Tags, Home, Reroll, Unclaim.
- Claim-row Sprite chips: gone with the claim menu.
- Settings → Packs **untouched**. Gear sprite size **untouched**.

## Forbidden

Quit from the rail. Destroy webviews. `sky.open` / `settings.open`. Stack toggle (RAIL-CLOSE PARK). Sky→Cloud. Rail-marks. HTML menu on the NSPanel. Writing claims from Rust. Zip on walker menu. Chroma-key. Complete from Close. Unclaim from Close. `tauri dev`. `STAMP_*`. Clay / brand/. Marble clocks / skip `drift_pass`. Companion process. `GO-ORBITTY-WALKER-CONTEXT` as a second writer.

## Dogfood

Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant.

1. Left-click an open claimed row → OS window outline flashes; window does **not** steal focus if it wasn’t front.
2. Double-click that row → minimizes; walker appears; double-click walker or row → window back.
3. Left-click walker → sprite outline flashes; Being editor does **not** open.
4. Right-click walker → image/folder/generated/packs/Open being. Disc miss → planet menu.
5. Right-click Unmapped row → Assign / Rename / Minimize / Close. **No** Unclaim. Minimize → Dock, no planet.
6. Right-click claimed → Unclaim present. Close → sleeper, not Done. Circle still Completes (Done + AX-close).
7. Pencil / stow / folder / Sprite chips gone. Circle remains.
8. Being editor (Open being…) has no Image block. Settings Packs still imports.
9. Home / STARS / Sky still only show (no toggle). Tray still quits.

When issued: row menu + locate + walker menu + strip, install-stable, stop. Do not stamp.
