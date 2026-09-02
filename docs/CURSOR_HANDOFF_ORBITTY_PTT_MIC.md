# Cursor Ultra — GO-ORBITTY-PTT-MIC

**Model:** `kimi-k3-max` if listed, else `composer-2.5` · **one writer**
**Gate:** Matt **GO-ORBITTY-PTT-MIC** via Pixel Artist **2026-08-27** (applied)
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_NEXT_PATH_v0.md` + this handoff

**This GO is the mic usage string + any WKWebView grant needed so hold-to-talk is honest.** Talk UI already exists. Do not redesign Talk. Do not Always-listen. Do not stamp STARS.

## Why
Matt 2026-08-27: Talk flashes **Listening** then back to **Talk**; error **“Mic permission denied — type instead.”** Blessed `Orbitty.app` Info.plist has **no** `NSMicrophoneUsageDescription` (and no speech-recognition usage string). `webkitSpeechRecognition.start()` then fires `not-allowed` and `onend` clears `talking`.

Wry already grants `requestMediaCapturePermission` (`WKPermissionDecision::Grant`). The OS still denies without the plist keys.

## Law
1. Hold-to-talk only. Transcript lands in the composer. **Never auto-send.**
2. Fail honest: if still denied after the plist, keep “type instead.” No fake mic.
3. No always-on, no wake word, no extra STARS arcade.
4. No in-app model. No MCP/ctl changes. No Jacob.
5. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Do

### 1. Merge Info.plist
Add `src-tauri/Info.plist` (Tauri 2 merges a file next to `tauri.conf.json`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>NSMicrophoneUsageDescription</key>
  <string>Orbitty uses the microphone only while you hold Talk, so speech can land in the companion field. It is never always-on.</string>
  <key>NSSpeechRecognitionUsageDescription</key>
  <string>Orbitty turns what you say while holding Talk into text in the companion field. Release to stop. Send is still your tap.</string>
</dict>
</plist>
```

Do **not** replace the whole generated plist. Merge-only keys.

Optional equivalent: `bundle.macOS.infoPlist` in `tauri.conf.json` pointing at that file. Prefer the sibling `Info.plist` so `tauri.conf.json` stays small.

### 2. Entitlements only if needed
Current blessed signature has **no** entitlements blob (`codesign -d --entitlements` empty, flags `0x0`). Do **not** invent hardened-runtime audio entitlements unless `install-stable` / Tauri starts signing with hardened runtime and mic still fails after the plist.

If you must add them: `src-tauri/entitlements.plist` with `com.apple.security.device.audio-input` = true, wire `bundle.macOS.entitlements`, and keep signing identity **LiveSprite Local Codesign**.

### 3. Do not rewrite Talk JS unless broken after plist
`CompanionPage.svelte` hold-to-talk stays:
- hold → `talkStart` / Listening
- release → `talkStop` / transcript in `draft`
- `not-allowed` → “Mic permission denied — type instead.”

Allowed JS polish (only if still required after plist):
- If `r.start()` throws immediately, keep the honest line (already does).
- Do not auto-send. Do not change STARS.

### 4. Install
`./scripts/install-stable.sh`. Tray Quit first. Open **only** `~/Applications/Orbitty.app`.

After install, Pixel/Matt must see both keys in:

```bash
plutil -p "$HOME/Applications/Orbitty.app/Contents/Info.plist" | rg 'Microphone|SpeechRecognition'
```

First hold of Talk should prompt macOS for Mic (and Speech Recognition if the OS asks). Grant once. Path+sign identity is `space.orbitty.mac` / LiveSprite Local Codesign — same as Accessibility: one re-grant after this install is expected.

## Out of scope
- Always-listen
- STARS arcade / pullers
- MCP, orbittyctl, Spark files, Jacob, viewport
- Changing Talk placement or making it a toggle
- Auto-writing Goose config

## Verify
1. Plist keys present on the blessed app.
2. Hold Talk: macOS prompt **or** (if already granted) Listening stays while held.
3. Speak, release: text in the field. Send still a separate tap.
4. Deny (or later revoke in System Settings → Privacy → Microphone): one honest “type instead.” Composer still works.
5. Accessibility still trusted (`orbittyctl status`).

Acceptance: `docs/ACCEPTANCE-ORBITTY-PTT-MIC.md`
