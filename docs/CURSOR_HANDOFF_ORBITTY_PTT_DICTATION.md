# Cursor Ultra — GO-ORBITTY-PTT-DICTATION

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-PTT-DICTATION** via Pixel Artist **2026-08-27** (applied)
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_NEXT_PATH_v0.md` + this handoff

**This GO is shipping Talk.** Default is macOS Speech.framework. Do not require Handy.app. Handy stays optional via Settings → Talk → Hotkey.

Pixel started the slice in-session then stopped so you are the sole remaining writer. Finish, link, install. Do not rewrite the product.

## Why

`webkitSpeechRecognition` dies in WKWebView. Desk dogfood (`GO-ORBITTY-PTT-MIC`) holds Handy’s Ctrl+Space. Matt: works, cannot ship (people must not download Handy). Default = system speech.

## Shape (do not change)

Press Talk → Listening. Press Talk again → stop. Transcript in the composer. User may edit. **Send is still their tap.** Never auto-send. Never always-on. Never a wake word.

- System path = Speech.framework (`SFSpeechRecognizer` + `AVAudioEngine`)
- Hotkey path = today’s Ctrl+Space hold while Listening
- Persist `state.companion.talkSource` (`system` | `hotkey`)
- `ORBITTY_TALK=handy|system` may override for Pixel, not shipping UI
- Settings picker already exists (`GO-ORBITTY-SETTINGS-WIN`)

## What Pixel already landed (keep / finish, do not throw away)

| File | State |
|------|--------|
| `src-tauri/src/talk_mac.m` | NEW — SFSpeech + AVAudioEngine, segment-final restart, C API `orbitty_talk_engine_start/stop` |
| `src-tauri/src/talk.rs` | rewritten — system = engine, hotkey = Handy chord; emits `talk:partial` / `talk:ended` / `talk:error` |
| `src-tauri/build.rs` | `cc` compiles `talk_mac.m` with `-fobjc-arc`; links Speech + AVFoundation + Foundation |
| `src-tauri/Cargo.toml` | `cc = "1"` under `[build-dependencies]` |
| `src-tauri/src/state.rs` | `talk_source_default()` = `"system"` |
| `src-tauri/src/coordinator.rs` | `set_companion_talk_source` calls `talk::stop_if_any()` |
| `src/lib/CompanionPage.svelte` | click toggle; listens `talk:partial/ended/error`; keeps typed prefix |
| `src/lib/SettingsPage.svelte` | help text: System is default |
| `src-tauri/Info.plist` | usage strings (Talk is Listening / click again) |
| `src-tauri/tauri.conf.json` | `bundle.macOS.infoPlist = "Info.plist"` |
| `~/Library/Application Support/space.orbitty.mac/state.json` | `talkSource` set to `system` (backup `.bak-ptt-dictation`) |

## Blocker you MUST fix before install

A `cargo check` passed, but the **release** `~/Applications/Orbitty.app` binary did **not** contain `orbitty_talk_engine_*` and did **not** link Speech.framework. LTO dropped the static lib. `strings` on the last blessed binary still said `System speech isn’t wired yet`.

`cargo clean -p livesprite` already ran (so the next build is a full crate rebuild).

**Required link proof after rebuild, before calling it done:**

```
nm "$HOME/Applications/Orbitty.app/Contents/MacOS/livesprite" | rg orbitty_talk_engine
otool -L "$HOME/Applications/Orbitty.app/Contents/MacOS/livesprite" | rg -i 'Speech|AVFoundation'
plutil -p "$HOME/Applications/Orbitty.app/Contents/Info.plist" | rg 'Microphone|SpeechRecognition'
```

`nm` must show `orbitty_talk_engine_start` and `_stop`. `otool` must show Speech.framework. Plist must have the Listening strings from `src-tauri/Info.plist`.

If LTO still drops the `.a`: keep `#[link(name = "orbitty_talk_mac", kind = "static")]` on the extern block in `talk.rs` (already added) and/or add `cargo:rustc-link-lib=static=orbitty_talk_mac` from `build.rs`. Do not leave Handy as the only live path.

Return codes from `orbitty_talk_engine_start`: `0` ok, `1` denied, `2` unavailable, `4` “Allow Mic and Speech, then click Talk again.” Honest strings already in `talk.rs`.

## Forbidden

- Requiring Handy, Superwhisper, MacWhisper, or any download
- webkitSpeechRecognition as the shipping engine
- Always-listen / wake word
- Auto-send
- In-app model
- New MCP / ctl tools (`settings.open` still forbidden)
- Changing Talk placement beyond label/tooltip honesty
- Jacob, Spark four-cast, STARS stamp, gravity poles
- `tauri dev` / `target/` dogfood — only `~/Applications/Orbitty.app`

## Verify (blessed app after tray Quit)

1. Handy **quit**. Settings → Talk = System speech. Click Talk → OS may prompt Mic/Speech once → Listening stays until second click.
2. Speak, click Talk → text in the field, **not** sent. Edit, Send.
3. Deny/revoke → type instead. Typing still works.
4. `orbittyctl status` still `Orbitty up · socket ok · AX trusted`.
5. Handy running + Settings System: Talk does **not** hold Ctrl+Space.
6. Settings Hotkey + Handy running = today’s dogfood chord still works.

Acceptance: `docs/ACCEPTANCE-ORBITTY-PTT-DICTATION.md`

When done: `./scripts/install-stable.sh`. Summarize files + dogfood steps. Do **not** write STAMP_* (Pixel stamps after Matt eyes-on). AX re-grant after install is expected.
