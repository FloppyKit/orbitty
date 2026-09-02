# Cursor Ultra — S3.1 crash fix (AX then quit)

**ONE writer** · Tree: `/Users/dude/goose artifacts/livesprite-tauri`

## Crash (Matt dogfood, 2026-08-21 23:47)
`~/Applications/LiveSprite.app` launched, AX granted, then **SIGABRT / rust abort** on main thread.

Stack (demangled):
`habitat::sync` → `world_view_class` OnceLock init → `ClassDecl` / `class!(LiveSpriteWorldView)` → `panic_fmt` → abort.

App lived ~7s. Crash: `~/Library/Logs/DiagnosticReports/livesprite-2026-08-21-234735.ips`

## Likely causes in habitat_mac.rs
1. `ClassDecl::new` returns `None` then `class!(LiveSpriteWorldView)` **panics** if class isn’t actually registered
2. `add_method(sel!(drawRect:), draw_rect as fn(&Object, Sel))` — **wrong IMP**: `drawRect:` takes `NSRect`. `register()` / add_method can panic
3. Any other unwrap/expect in first `do_sync` after list_windows

## Required fix
- `world_view_class`: `Class::get("LiveSpriteWorldView")` first; never use panicking `class!` fallback
- `drawRect:` IMP must accept dirty rect (`NSRect` or ignored third arg matching objc encoding)
- `do_sync` / `sync`: **catch_unwind** + log; return Err to JS — **never abort the app**
- Habitat failure must leave **rail usable** (list/focus still works)
- Re-run `./scripts/install-stable.sh` so `~/Applications/LiveSprite.app` is the fixed binary
- Confirm Local Codesign

Keep S3 behavior (planets, stow, marble). No S4.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```
Summarize root cause + files. No nest STAMP (Pixel stamps after Matt relaunch).
