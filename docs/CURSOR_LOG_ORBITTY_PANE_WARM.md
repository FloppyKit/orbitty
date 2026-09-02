# LOG — GO-ORBITTY-PANE-WARM (writer run 2026-08-31)

Sole writer run against `docs/CURSOR_HANDOFF_ORBITTY_PANE_WARM.md` +
`docs/ACCEPTANCE-ORBITTY-PANE-WARM.md`. SoT: Pixel lock
(`~/.buzz/PLANS/ORBITTY_PANE_WARM_LOCK_v0.md`) — first rail click of
Companion / Board / STARS / Settings beach-balled: lazy `ensure_window`
built the WKWebView on the click path, then the SPA-leftover mount check
`reload()`ed a still-hydrating view (both main-thread hops). Second click
was already instant (hide-not-destroy). Not a stamp.

## What changed

### Native (`src-tauri/src`) — two files, no re-merge

- `lib.rs` — `run().setup`: after the rail wiring loops (pane events +
  follow-docked Moved/Resized hook) and BEFORE the `companion.visible`
  auto-show, warm-build the three chrome drawers:
  `ensure_window` for `companion`, `kanban`, `settings`. Failures log to
  stderr and never abort setup. The builder's own law holds: `visible:
  false`, born click-through, no `show()`, and `ensure_window` still
  skips the stay-visible pin for `companion` (pin-while-shown untouched —
  the pin is earned only by a real `show_drawer`). Not `stars` (STARS
  shares the kanban pane), not `sky` (stays lazy). Placement after the
  rail-only loops matters: those loops `get_webview_window` (cache-only),
  so the warmed panes are wired + glassed exactly once, by
  `ensure_window` itself.
- `lib.rs` — `ensure_window`: `drawers::stamp_window_created(label)`
  right after `.build()` succeeds. The cache-hit early return is above
  it, so only a real build stamps.
- `drawers.rs` — new created-at stamp store next to the mount check:
  `OnceLock<Mutex<HashMap<String, Instant>>>`, `pub(crate)
  stamp_window_created`, private `window_created_at`.
- `drawers.rs` — `schedule_drawer_mount_check`: after the first (900ms)
  AX read comes back not-mounted, a pane whose stamp is < 3s old returns
  — first AX read only, no `reload()`, no navigate. A `reload()` landing
  mid-hydrate WAS the beach ball. A warm pane (alive > 3s, or never
  stamped) with an empty WebArea falls through to the existing one
  `reload()` + one bare-`index.html` navigate (SPA law unchanged). Never
  hide, never loop — both still hold.

## Kept (C-list, verified in place)

MATCH Overlay+transparent+`ensure_transparent`, SPA query-only nav guard
(never hides), HOLD never-hide, GLASS one `--bg-fill`, companion
pin-while-shown, COVER, STARS-shares-kanban, `?mode=` ban,
hide-not-destroy, slot-if-fits, list-row locate-only,
`hide_traffic_lights`, in-SPA banners.

## Untouched on purpose

No pre-`show()` of drawers (no flash / focus steal at launch; the
`companion.visible` auto-show path is the pre-existing one and now runs
against an already-built webview). No visible titlebar, no
`ensure_opaque`, no hide-after-show, no Svelte edits, no `state.json`
edits, no `slide_into_slot`, no screen-center, drawers kept, no Habitat /
Boombox, no `tauri dev`, no stamp, sole writer.

## Verify

- `cargo check`: clean (pre-existing warnings only — objc macro `cfg`
  noise, `unused import` in `main_thread_sync`, `mut config`; none from
  this GO's lines).
- `./scripts/install-stable.sh`: **exit 0** (~238s). Signed with the
  stable "LiveSprite Local Codesign" identity (no ad-hoc fallback),
  `--verify --deep --strict` clean, installed to
  `~/Applications/Orbitty.app`; `orbittyctl` + `orbitty-mcp` rebuilt and
  installed to `~/.local/bin`; Goose seat merge ok. No `tauri dev` at any
  point. Orbitty not killed — tray Quit is Matt's.

Dogfood path left to the human (acceptance doc): tray Quit → open only
`~/Applications/Orbitty.app` → AX re-grant → cold launch → click
Companion (spirit+thread, no beach ball or only a blink), Board (Lists
UI), STARS in that same window, Settings — glass still matching the rail,
drawers hidden until clicked, second click still instant.
