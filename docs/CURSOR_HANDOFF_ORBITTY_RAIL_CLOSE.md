# Cursor Ultra — GO-ORBITTY-RAIL-CLOSE

**Model:** `kimi-k3-max` · **one writer**
**Gate:** waiting Matt phrase `GO-ORBITTY-RAIL-CLOSE`. **Do not code until issued.**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_RAIL_CLOSE_LOCK_v0.md` + this handoff

Matt 21:25: close Orbitty windows from the rail. Stack buttons today only show.

## Ship

Toggle hide on **already-visible** Orbitty webviews. First click still shows+focus.

| Rail control | Label | Show (already) | Hide if visible |
|--------------|--------|----------------|-----------------|
| STARS | `stars` | `open_stars` → `show_labeled` | `hide_window("stars")` |
| Sky | `sky` | `open_sky_window` | `hide_window("sky")` |
| Board | `kanban` | `open_kanban` | `hide_window("kanban")` |
| Companion | `companion` | `open_companion` | `hide_window("companion")` (keeps talk-stop + persist hidden) |
| ⚙ | `settings` | `open_settings_window` | `hide_window("settings")` |

**Preferred:** one helper, e.g. `toggle_labeled(app, label)` / `toggle_companion`:

```
if window.is_visible()? { hide_window(label) } else { existing show path }
```

Wire rail clicks (`openStars` / `openSky` / `openKanban` / `openCompanion` / `openSettingsWindow` in `src/routes/+page.svelte`) to that helper. Gear ⚙ is in the banner, same rule.

Do **not** change Home (`bring_all_front`). Do not hide Agents / Actions / Projects lanes. Do not AX-close claimed OS windows from these buttons.

× on each window still `hide_window`. Tray Quit still the only quit.

## Forbidden

Quit from the rail. Destroy webviews. `sky.open` / `settings.open` ctl/MCP. Sky-HOLD recreate. Sky→Cloud rename. Rail-marks icons. Walker context. Being editor. habitat_mac.rs. Marble clocks. Talk source. Packs. brand/. `tauri dev`. `STAMP_*`. Active-class polish (optional, skip).

## Dogfood

Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant.

1. Rail STARS → window. STARS again → gone. STARS again → back.
2. Same for Sky, Board, Companion, ⚙.
3. Home still only planet panels.
4. Companion hide still stops PTT. × still hides. Tray still quits.

When issued: toggle, install-stable, stop. Do not stamp.
