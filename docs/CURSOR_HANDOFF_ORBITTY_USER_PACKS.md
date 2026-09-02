# Cursor Ultra — GO-LS-TAURI-USER-PACKS + reboot rematch

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

READ:
- `docs/ACCEPTANCE-USER-PACKS.md`
- `~/.buzz/PLANS/ORBITTY_USER_PACKS_PERSIST_LOCK_v0.md`
- `~/.buzz/PLANS/LIVESPRITE_SPRITE_PACKS_LOCK_v0.md`
- existing `src-tauri/src/packs.rs`

**Do not scrape itch. Do not download zips from the web. Do not touch Swift.**

## A) User packs (Gear → Packs)

Extend `packs.rs`:

```
packs/<slug>/
  manifest.json   optional { name, license, frames?: ["a.png"], sheet?: {file, frameW, frameH, count} }
  *.png
```

Commands:
- `list_packs() -> [{id, name, license, warnLicense, frameCount, path}]`
- `import_pack_folder()` — **NSOpenPanel** (directories). Copy into packs/<slug>.
- `import_pack_zip()` — NSOpenPanel for `.zip`. Unzip into packs/<slug> (`zip` crate OK).
- `remove_pack(id)` — delete that subfolder only.
- `packs_dir() -> string`

License: if no manifest.license (or empty), `warnLicense=true` still import. Never block.

Habitat: load pack PNG frames (or sheet slices) into `NSImage`s. Walkers use a pack frame (cycle 2–8 frames if present) **instead of** ASCII TEMPLATES when any pack has frames. Hash `entry_id` to pick which pack/template. Fallback blob/cat/robot if zero frames.

Update `packs/README.txt` (space.orbitty.mac, import UI exists, itch is browse-only).

## B) Itch directory (not a store)

Gear → Packs:
- **Browse itch.io** → `open` `src/lib/itch-directory.json` `directoryUrl` (fallback URL if you want a second button).
- List the JSON `packs[]` as links (`open` the url). Caption: “Directory only — download on itch, then Import zip. Confirm CC0 on the page.”

`shell:allow-open` already exists.

## C) Reboot rematch (the actual bug)

Live ids are `pid:cgWindowID` — they die on reboot.

1. `WindowInfo` add `stableKey: String` = `{bundleId or appName}|{title}|{n}` where `n` is 1-based among same bundle+title in this enumerate.
2. `Claim` add optional `bundleId`, `appName`, `titleAtClaim`, `titleIndex` (serde default).
3. On **claim**, stamp those from the live window (AX title, not override).
4. After every `list_windows` (rail), **rematch**:
   - Build unused live windows by stableKey / bundle+title.
   - If `claim.windowId` not in live ids, assign the best unused live window: exact stableKey, else same bundle+title first unused.
   - Rewrite `windowId` on that claim, matching `kanbanCards.windowId`, `windowTags`, `titleOverrides`, `nudgeSuppressed`.
   - Persist once if any rewrite.
5. Do not merge two claims onto one window. Unmatched claims stay listed (orphan) until the window returns — do not delete.

Same rematch in Lists/Kanban if it uses window ids.

## D) Nudge: already-minimized at open = silent

S3 lock stays “once then quiet” for **fresh** minimizes.

- On **first successful poll after `stateLoaded`**, record every currently minimized window id (and their stableKeys) into `nudgeSuppressed` and **do not** set `nudgeFor`.
- After that, only nudge when a window **transitions** visible → minimized (track previous `isMinimized` set). Still once per id.
- Never auto-claim.

## Keep
Home, Lists hide, opacity, circle-closes-window, planet zoom/rclick, Agents/Actions, tray.rgba.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# codesign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```

Summarize. No nest STAMP.
