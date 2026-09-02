# ACCEPTANCE — GO-ORBITTY-PTT-MIC

status: built (dogfood)
created: 2026-08-27
go: GO-ORBITTY-PTT-MIC 2026-08-27
stamped: no — shipping Talk is `GO-ORBITTY-PTT-DICTATION` (system speech, no Handy required)

Blessed dogfood: Talk click holds Handy’s PTT (`ctrl_left+space`) until the second click; Handy pastes into the composer. Matt 2026-08-27: works for desk use; **must not ship as the default** (cannot make people download Handy).

- [x] `Info.plist` has `NSMicrophoneUsageDescription`
- [x] `Info.plist` has `NSSpeechRecognitionUsageDescription`
- [x] Click Talk: Listening / Handy overlay stays until second click (Matt eyes-on)
- [x] Second click: transcript in composer, **not** auto-sent
- [x] No always-on / no wake word
- [ ] Shipping default = system dictation — **not this GO** (`GO-ORBITTY-PTT-DICTATION`)
