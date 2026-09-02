# Cursor Ultra — rail **Hide titles**

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
No Swift. No companion. No art.

Matt: the Settings → Rows button **Show full titles** is wrong. It should be **Hide titles** (same voice as **Hide icons**).

## Behavior
- Default: titles **shown**, **one line**, `ellipsis` — as much as the row width allows. **No wrap.** Kill `.rail.full-titles` wrap CSS.
- Button label always **Hide titles**. `active` / `aria-pressed` when titles are hidden.
- Hidden: do not draw the title string (icon + chrome only). Keep row height / hit target honest.
- Persist `hideTitles` (default **false**). Stop writing `showFullTitles`. Old saves: ignore `showFullTitles` (both old values → titles visible, one line).

## Verify
`./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
No STAMP_*.
