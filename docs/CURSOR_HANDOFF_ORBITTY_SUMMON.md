# CURSOR HANDOFF — GO-ORBITTY-SUMMON

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_SUMMON_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/lib.rs` — global shortcut handler ~448–466
- `src/lib/CompanionPage.svelte` — **only if** `.field` does not take caret from webview focus

## Do

1. Keep registration `Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyO)`. `Pressed` only.
2. On Pressed: `panes::show_companion(&handle)` then focus the `"companion"` webview. **Not a toggle.** Do not hide companion if already visible. Do not touch `"main"`. Delete the `get_webview_window("main")` hide/show branch.
3. `show_companion` already `set_focus()`s via `show_drawer`. If `.field` (`fieldEl`) still does not take the caret, `emit_to("companion", "orbitty:companion-summon", ())` and CompanionPage `listen` → `fieldEl?.focus()`. Do not add the event if webview focus is enough.
4. Tray, dock Reopen, left-click tray, `show_rail`, rail ×: **unchanged**.
5. `./scripts/install-stable.sh`; stop. Do not stamp.

## Do not

DEST-STRIP. Gear. Park. Tray click-split. `tauri dev`. Second writer. Rebind the chord. Fold #2.
