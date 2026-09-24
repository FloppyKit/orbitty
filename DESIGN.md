---
version: alpha
name: Orbitty
description: >
  Stars is the captain's log on a ship moving through space.
  Components declare state. Official themes decide look.
  Flagship values below are Professional Dark.
colors:
  surface-space: "#070B12"
  surface-panel: "#101820"
  surface-inset: "#0A1018"
  surface-scroll: "#101820"
  line-panel: "#24383E"
  line-instrument: "#7EC8D6"
  state-touchable: "#6AA8B4"
  state-touching: "#D7F6FB"
  state-active: "#9BE0EA"
  state-set: "#6AA8B4"
  state-disabled: "#5C6772"
  voice-ship: "#8FD0DC"
  voice-captain: "#E4B56A"
  type-text: "#D5DCE6"
  type-readout: "#9BB0C0"
  type-dim: "#8A97A8"
  status-working: "#E08A2E"
  status-ok: "#3CB87A"
  planet-clinic: "#5CCC94"
  planet-orbitty: "#5C81CC"
  planet-house: "#CC5C94"
  planet-book: "#A75CCC"
  planet-grove: "#CC8B5C"
  planet-sail: "#6E5CCC"
  light-surface-space: "#F3F1EB"
  light-surface-panel: "#FBFBF8"
  light-surface-inset: "#EFECE4"
  light-surface-scroll: "#FBFBF8"
  light-line-panel: "#D5D0C6"
  light-line-instrument: "#0E5C62"
  light-voice-ship: "#0E5C62"
  light-voice-captain: "#7A4A08"
  light-type-text: "#1C1B17"
  light-type-dim: "#5E5A52"
  terminal-surface-space: "#0A0A0A"
  terminal-surface-panel: "#000000"
  terminal-surface-inset: "#000000"
  terminal-surface-scroll: "#000000"
  terminal-line-panel: "#6FBF84"
  terminal-line-instrument: "#8FDC9A"
  terminal-voice-ship: "#8FDC9A"
  terminal-voice-captain: "#E4D7A2"
  terminal-type-text: "#C8E8CE"
  terminal-type-dim: "#5F8A68"
  rpg-surface-space: "#101C38"
  rpg-surface-panel: "#1E3C9A"
  rpg-surface-inset: "#162F78"
  rpg-surface-scroll: "#F0E2C0"
  rpg-line-panel: "#F7F8FF"
  rpg-line-instrument: "#F7F8FF"
  rpg-voice-ship: "#F7F8FF"
  rpg-voice-captain: "#FFE7A3"
  rpg-type-text: "#F7F8FF"
  rpg-paper-ink: "#3B2A16"
typography:
  title:
    fontFamily: SF Pro Text
    fontSize: 0.9rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.14em
  label:
    fontFamily: SF Pro Text
    fontSize: 0.66rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.08em
  body:
    fontFamily: SF Pro Text
    fontSize: 0.75rem
    fontWeight: 400
    lineHeight: 1.45
  captain:
    fontFamily: SF Pro Text
    fontSize: 0.78rem
    fontWeight: 400
    lineHeight: 1.45
  control:
    fontFamily: SF Pro Text
    fontSize: 0.68rem
    fontWeight: 700
    lineHeight: 1.2
  readout:
    fontFamily: SF Mono
    fontSize: 0.62rem
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: 0.12em
rounded:
  control: 7px
  panel: 10px
  full: 999px
spacing:
  base: 4px
  sm: 8px
  gutter: 10px
  page: 12px
components:
  panel:
    backgroundColor: "{colors.surface-panel}"
    textColor: "{colors.type-text}"
    rounded: "{rounded.panel}"
    padding: 10px
  button-primary:
    backgroundColor: "{colors.surface-inset}"
    textColor: "{colors.voice-ship}"
    rounded: "{rounded.control}"
    padding: 8px
  log-entry:
    backgroundColor: "{colors.surface-scroll}"
    textColor: "{colors.voice-captain}"
    typography: "{typography.captain}"
    padding: 8px
  readout:
    backgroundColor: "{colors.surface-panel}"
    textColor: "{colors.type-readout}"
    typography: "{typography.readout}"
  planet-chip:
    backgroundColor: "{colors.planet-clinic}"
    textColor: "{colors.type-text}"
    rounded: "{rounded.full}"
    size: 14px
---

# Orbitty

Stars is the captain's log on a ship moving through space. Quiet. Instruments hover on dark glass. Light comes from things that are doing something.

Professional Dark is the flagship and the default. The same Stars structure has to survive three more official themes: Professional Light, Terminal, and RPG. Components declare state. Themes decide look.

## Overview

The room is a logbook, not a book, a watch, or a stage. The page is the view out the window. Panels are instrument glass. The captain's own words (wins, grateful, journal) are a warm voice. Ship chrome (controls, radar, readouts) is a cool voice. Planet color appears only on the planet chip.

Two rules sit above any single screen.

**Official theme contract (hard).** The four official themes are Professional Dark, Professional Light, Terminal, and RPG. They share one structure: the same parts, the same places, the same order. Only the skin changes. A theme never paints a surface it does not own. Professional Dark never contains a light page. Professional Light never contains a dark island. Terminal is monospace and borrows its manners from herdr. RPG owns the blue window, and it owns parchment for long text. These four mappings are a contract. A Stars screen that drops a part, moves it, or swaps in another theme's surface is wrong.

**Custom theme guidance (soft).** An end-user theme is not held to that structure. It should still speak the semantic tokens below, so a control can say "touchable" without naming a color. It may invent a surface it owns, the way RPG owns parchment. It may omit a part if it says so. It should not import an official surface it does not own: no Dark glass inside a light custom theme, no RPG parchment inside a custom dark theme, no Terminal reverse-video as decoration on a themed window that is not Terminal.

Stars is the reference screen. Other Orbitty screens are not restyled from this file alone.

## Colors

Token names use hyphens. The design.md reference syntax already uses dots as paths, so `surface.space` is written `surface-space`.

State tokens, shared by every theme:

- **surface-space:** the view out the window.
- **surface-panel:** instrument glass, or that theme's window.
- **surface-inset:** fields and wells.
- **surface-scroll:** long text. Journal lives here. In Dark, Light, and Terminal this resolves to that theme's own panel. In RPG it resolves to parchment, because RPG owns paper.
- **line-panel:** the edge of a window.
- **line-instrument:** radar cage, gauge ring, orbit ring around a selected planet.
- **state-touchable:** can be used, and must look it before anyone clicks.
- **state-touching:** hover or focus.
- **state-active:** pressed, or running.
- **state-set:** saved or done. Steady. The hum stops.
- **state-disabled:** present, not available.
- **voice-ship:** controls, instruments, linework.
- **voice-captain:** Matt's words. Wins, grateful, journal prose.
- **type-text:** ordinary interface text.
- **type-readout:** numbers, dates, `LOG · 2026-09-24`.
- **type-dim:** quiet labels. Still the ship voice, at lower contrast. Body text stays on `type-text` or `voice-captain`.
- **status-working / status-ok:** herdr's orange and green, used only as activity marks on a planet chip. Orange is working. Green is ok. They are not score colors and not panel fills.
- **planet-\<name\>:** identity color of one planet. Painted only on that planet's chip.

### Professional Dark

Space is near-black navy `#070B12`, with a faint nebula in the corners and a slow drift of small stars. It is never bright. Panel is smoked glass resolving to `#101820`, a thin lit edge, no filled flat card. Inset is `#0A1018`. Scroll is the same glass. Ship voice is cool cyan `#8FD0DC`. Captain voice is warm amber `#E4B56A`.

Touchable is a faint cyan presence. Touching is brighter. Set is the same family, steady and dimmer. Active is the bright edge on the one action that is underway. Disabled is `#5C6772`.

Light only comes from things doing something: an unset beacon, a hovered control, a running gauge, a selected orbit, a planet with recent activity, the Save today edge, the Reflect edge. No spotlights. No decorative glow. No fake floor shadow. No ellipse under the radar.

A few literal touches, and only these: corner brackets on panels, and monospace readouts such as `LOG · 2026-09-24`.

### Professional Light

The same states, drawn with ink weight and a soft shadow. Space `#F3F1EB`. Panel `#FBFBF8`. Inset `#EFECE4`. Scroll is that same light panel. Ship ink `#0E5C62`. Captain ink `#7A4A08`. Text `#1C1B17`.

Touchable is a broken ink ring and a hollow core. Touching is heavier type and a filled core. Set is a steady filled core. There is no glow and no pulse. There is no dark card sitting in the light page. Save today is a light button with ship-colored ink and a small soft shadow.

### Terminal

Herdr is the reference. Stars' two columns match herdr's sidebar and main pane. The planet strip matches herdr's tab strip.

Everything is monospace (`SF Mono`, then ui-monospace, Menlo). Space `#0A0A0A`. Panel and scroll `#000000`. Ship phosphor `#8FDC9A`. Captain phosphor `#E4D7A2`. Text `#C8E8CE`. Dim `#5F8A68`.

Borrow herdr's manners:

- Section labels are lowercase and dim, the way herdr labels spaces and agents.
- Selection is a colored status dot plus a slightly lighter row band.
- A selected tab is an inverted solid block.
- Separation is mostly background shade and spacing. Borders are rare.
- Box-drawing is used only where it earns a frame, such as a text input. It is not the frame of every panel.
- Item headers use the `◆` glyph.
- Timestamps and readouts are dim and right-aligned, in the manner of `74K / 500K`.
- A hint line sits on the bottom: `Shift+Tab: mode  |  Ctrl+c: cancel`.
- Orange and green status dots mark planet activity.

Touchable commands read `[ Sleep ]`, and the brackets blink slowly until the spoke is set. Touching and the selected tab are reverse video (the inverted block). Set is bold. The planet chip stays visible inside that inverted block, beside the name.

### RPG

Dragon Quest VIII windows are the base. Space is a textured dark blue `#101C38`. Panel is a blue window `#1E3C9A`, white text `#F7F8FF`, a white rounded border. Captain lines on that window are gold `#FFE7A3`.

Touchable shows a `▶` cursor. Touching is a highlighted menu row. Set is steady white, with no cursor.

Journal and other long text use a parchment scroll `#F0E2C0` with ink `#3B2A16`. Paper is allowed because RPG owns it. The check-in radar and the three win lines stay on the blue window. They are not paper.

## Typography

Two families already on the machine. SF Pro Text for the ship and for the captain's prose. SF Mono for readouts. No downloaded face. No emoji as icons.

Terminal uses SF Mono for everything, including prose.

- **Title:** 0.9rem, weight 600, tracking 0.14em. The STARS banner.
- **Label:** 0.66rem, weight 600, tracking 0.08em. Section labels. In Terminal these are lowercase.
- **Body / captain:** about 0.78rem, line-height 1.45. Wins and journal prose.
- **Control:** 0.68rem, weight 700. Save today, Reflect, tabs.
- **Readout:** 0.62rem SF Mono, tracking 0.12em. Dates, spoke numbers, `LOG ·` headers, gauge figures.

Radar spoke names scale with the dial. Gauge figures stay small enough to sit in a 32px lamp.

## Layout

The base unit is 4px. Banner gap and gauge gap are 8px. The body gutter is 10px top and 12px on the sides and bottom. Do not respace the two columns to chase a new grid.

Official themes keep this tree, in this order:

1. Banner. STARS, previous day, `TODAY ·` date, next day, Day, Prompts, close.
2. Destination strip. All on its own shelf. Then each planet as a chip beside its name. Edges fade. A small chevron. No scrollbar.
3. Left column, about half the body. Check-in, then a thin split, then Trackers.
4. Right column. Cloud and Journal as two tabs. One Reflect.
5. A hint line along the bottom. Terminal draws the herdr key hint. The other official themes keep the same slot and draw it in their own voice.

Check-in holds the radar, then `LOG ·` the date, then Win 1, Win 2, and Grateful, then Save today and Edit. Trackers are one row of small round gauges. The tracker card is as tall as that row. Check-in takes the space above it. Empty copy such as "No nudges yet." is not shown.

The left/right split defaults near 0.52. The check-in/tracker split is content-sized on the tracker side. Double-click still resets the live app's splits. This file does not change those fractions in code.

**Planet chip.** In every official theme the chip sits immediately beside the planet name in the top strip. The chip is a small lit body in `planet-<name>`. Terminal does not replace it with a different element. A selected planet is an inverted block, and the chip is still visible inside that block, beside the name. Recent activity adds the orange or green status mark on the chip. Selection itself is an orbit ring in ship linework (Dark, Light, RPG) or the inverted block (Terminal). The name is `type-text`, not the planet color.

**Structure parity** is official-only. A custom theme may rearrange, and should say which part it dropped.

## Elevation & Depth

Depth is not a stack of drop shadows.

In Professional Dark, space is behind smoked glass. The glass has a hairline lit edge and corner brackets. Nothing casts a floor shadow. Hover is a change of light on the thing being touched, not a lifted card with a bigger shadow. `motion-lift` may move a control 1 or 2px, with no shadow growing under it.

`motion-hum` is a slow opacity pulse, about 3.5s, on unset beacons, running gauges, and planet chips. Reduced motion turns the pulse off. Light does not pulse. It uses ink weight. Terminal blinks the brackets of an untouched command, and stops blinking when reduced motion is on. RPG keeps the `▶` still.

In Professional Light, a soft shadow under a panel is allowed. It is ink and paper, not a glow.

In Terminal, shade and spacing separate regions. A lighter row band marks selection.

In RPG, the blue window sits on the textured field. The parchment scroll is the forward surface for journal text.

## Shapes

Panels are one radius on every corner: 10px in Dark and Light, 14px on the RPG window, square in Terminal. The book edge is gone. One straight side and a rounded other side is not a style. It reads as a broken corner.

Controls are 7px, or square in Terminal. Pills are full round where Dark, Light, and RPG use them. Gauges are circles, about 32px. Planet chips are 14px circles. In Terminal the chip is still a colored mark beside the name, the same size order, not a letter in place of the chip.

Journal lines are a bottom rule, not a rounded box.

## Components

**Radar.** A navigation readout in thin instrument lines, floating on the panel. Five rings, five spokes: Sleep, Terroir, Athleticism, Relationships, Self. No shadow under it. No radial wash behind it. The filled shape is a thin stroke plus a very quiet tint of the ship line, only over spokes that have a score.

Each handle is a small beacon with an orbit ring. Until the spoke is set, the ring hums, so the handle looks touchable before a click. Set locks the beacon at a steady dim. Touching brightens it and stops the hum. Values 1–5 stay typeable. Empty plus Enter clears a spoke. Escape leaves the score. First grab on an unset spoke still sets 3.

Terminal draws the same cage in phosphor. An unset label is `[ Sleep ]`. A touched label is reverse video. A set label is bold. RPG puts `▶` on the touchable spoke and a highlighted row on the touched one. Light uses a dashed ink ring, then a heavier filled core.

**Log lines.** Win 1, Win 2, and Grateful sit under the monospace header. The words are captain voice. The keys are dim readouts. Save today is the one primary action in Check-in. Edit stays quiet. A dirty draft brightens Save today. It does not add a second primary.

**Cloud.** A star field on space. Each word is a star. How often it appears is how bright and how large it is. The field keeps the three existing star hues, ochre, teal, and violet. They are stars in the window, not planet chips and not a page surface. Drift is a few pixels. Reduced motion holds them still.

**Journal.** Past entries on `surface-scroll`. One Reflect, on the tab row when pages exist, on the empty page when they do not. Never both. No book edge. In Dark the entries are amber on glass. In Light they are captain ink on the light panel. In Terminal they are captain phosphor on black, with a `◆` header. In RPG they are ink on the parchment scroll.

**Trackers.** Small round lamps in a row, about a third of the old watch-face card. The card shrinks to the row. At rest: lamp, figure, short label. Hover or focus shows decrease and increase, or Start and Stop on a timer. The figure can be typed. A running timer hums and is visually distinct from a set lamp. Planet identity color is not painted on the lamp. The planet is named in dim readout type, or by the chip up in the strip.

**Destination strip.** All stays on its own shelf, outside the scrolling row. Each planet is chip plus name. Planets with recent activity carry the orange or green status mark and read a little brighter. The selected planet takes an orbit ring, except in Terminal, where the whole row inverts and the chip remains. Overflow fades at the edge and offers a chevron. The scrollbar stays hidden.

**Hint line.** Last, full width. Terminal: the herdr key hint, dim, monospace. Other official themes: the same slot, one quiet readout, their own type.

## Do's and Don'ts

**Do**

- Keep the two-column Stars frame and the 4px step in every official theme.
- Declare touchable, touching, active, set, and disabled, and let the theme draw them.
- Put the planet chip beside the planet name in every official theme, including inside a Terminal inverted block.
- Use herdr as the Terminal reference: lowercase labels, inverted selected tab, rare borders, `◆` headers, dim right-aligned readouts, a bottom hint line, orange and green activity marks.
- Keep captain words and ship chrome in their own voices.
- Move with transform, opacity, and a shadow only where that theme owns shadow. Reduced motion: no pulse, no blink.
- Keep body text at or above 4.5:1 on its surface. Keep Reflect, Save today, inc, dec, and start/stop targets at least 24px.

**Don't**

- Don't put a light surface in Professional Dark. Don't put a dark island in Professional Light.
- Don't borrow parchment outside RPG, or box-drawing as general chrome outside an input, or herdr reverse-video outside Terminal.
- Don't add a spotlight, a radial glare, a floor shadow, or an ellipse under the radar.
- Don't bring back the book edge, the orange score dash, or a second Reflect.
- Don't paint planet colors on gauges, banners, buttons, or journal text.
- Don't show "No nudges yet." when nothing is there.
- Don't treat a custom theme as a broken official theme. Custom themes get the soft guidance above, not the structure contract.
- Don't restyle other Orbitty screens from this file alone.
