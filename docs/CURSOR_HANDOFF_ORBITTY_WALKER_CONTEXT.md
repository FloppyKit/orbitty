# Cursor Ultra — GO-ORBITTY-WALKER-CONTEXT

**Model:** `kimi-k3-max` · **one writer**
**Gate:** waiting Matt phrase `GO-ORBITTY-WALKER-CONTEXT`. **Do not code until issued.**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_WALKER_CONTEXT_LOCK_v0.md` + this handoff

Matt 21:25: walker icon out of the rail main menu; right-click the walker.

## Ship

### 1) Hit test — `src-tauri/src/habitat_mac.rs` `right_mouse_down_inner`

Today: `over_disc` then planet NSMenu. Walkers are ignored.

**Before** the planet menu:

```
hit_being(planet, local)
  walker true  → walker NSMenu; return
  walker false → fall through (dots do not get this menu)
over_disc      → planet NSMenu (unchanged items)
else           → return (click-through)
```

`hit_being` already prefers walkers (`frame.inset(-3)`). Reuse it.

### 2) Walker NSMenu

Same `popUpContextMenu` pattern as `OrbittyPlanetMenuTarget`. New target class **or** extra selectors + `representedObject` = entry id (not project id).

Items:

1. Use my image…
2. Folder…
3. separator
4. Generated — checkmark if this walker’s `spritePackId` is empty
5. One item per loaded **walker** pack (`role` not `companion` / `habitat`) — checkmark current
6. separator
7. Open being… → existing `habitat:being` emit (Being editor)

Pack names: habitat already has pack frames in memory after `pack_art`. If the native side lacks display names, emit and let the rail own the list — **prefer native list** from the same snapshot the walker draws.

### 3) Rail listens — `src/routes/+page.svelte`

Do **not** write `claims` from Rust. Emit to `main`, e.g. `habitat:walker-pack`:

| Payload | Rail |
|---------|------|
| `{ entryId, kind: "image" }` | `import_pack_image` then `setSpritePack(entryId, p.id)` |
| `{ entryId, kind: "folder" }` | `import_pack_folder` then `setSpritePack` |
| `{ entryId, packId: null }` | `setSpritePack(entryId, null)` |
| `{ entryId, packId: "slug" }` | `setSpritePack(entryId, packId)` |
| Open being | existing `habitat:being` |

`beingImportImage` / `beingImportFolder` today require `beingEditorFor`. Take an **id argument** so import works with the editor **closed**.

No zip on this menu. Magenta is not chroma-key. Reuse existing import commands (file dialogs are AppKit — OK from a menu action posted to main; if a dialog from the NSMenu action is awkward, emit `kind: "image"` immediately and let the webview invoke — **prefer that** so the dialog is the same as today).

### 4) Strip Image from the rail

- Being editor (`being-editor`): **remove** the Image label + Use my image / Folder / clear. Keep Kind, Palette, Size, Tags, Home, Reroll, Unclaim.
- Claim-row Sprite chips (`generated` + pack name buttons): **remove**. Keep **Edit being…**.

Settings → Packs **untouched**. Gear sprite size **untouched**. Left-click walker / double-click restore **untouched**. Planet menu items **untouched**.

## Forbidden

HTML overlay menu on the NSPanel. Writing claims from Rust. Zip on the walker menu. Chroma-key. reload-off-main / pack-import freeze polish. Rail-close (other GO). Rail-marks. Sky→Cloud. Clay / brand/. Marble clocks. `tauri dev`. `STAMP_*`.

## Dogfood

Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant.

1. Right-click **empty disc** → plant / boombox menu (unchanged).
2. Right-click **walker** → Use my image / Folder / Generated / packs / Open being. No plants.
3. Pick a pack → that walker changes; others don’t. Persist across quit.
4. Generated → built-in being back.
5. Use my image… → same file dialog as before; walker wears it. Magenta PNG still a rectangle.
6. Open being… → Kind/Palette editor **without** Image block.
7. Claim-row Sprite chips gone. Settings Packs still imports.

When issued: menu + strip, install-stable, stop. Do not stamp.
