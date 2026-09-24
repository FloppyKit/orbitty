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
  terminal-surface-space: "#000000"
  terminal-surface-panel: "#0C0E14"
  terminal-surface-inset: "#000000"
  terminal-surface-scroll: "#000000"
  terminal-line-panel: "#2A3148"
  terminal-line-instrument: "#7AA2F7"
  terminal-voice-ship: "#7AA2F7"
  terminal-voice-captain: "#C0CAF5"
  terminal-type-text: "#C0CAF5"
  terminal-type-dim: "#565F89"
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
    letterSpacing: 0.01em
  label:
    fontFamily: SF Pro Text
    fontSize: 0.66rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.01em
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
    size: 8px
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

Space is near-black navy `#070B12`, with a faint nebula haze, navy into purple, drifting slowly. It is never bright. There is no star-dot wallpaper and no repeating speck tile. Stars appear only as things you can mean: a constellation handle, a word in the cloud. A sparse starfield behind the panels was drawn beside this haze and lost. The specks stayed in the margins. The haze is the space.

Panel is real glass over that haze. Backdrop blur, a top-lit vertical gradient, a 1px lit top edge, the 10px radius, and a gentle shadow falling down and to the right. Light comes from the upper left, so the tile sits in the space. No corner brackets. No filled flat card. The opaque resolve of the glass is still `#101820`. Inset is `#0A1018`. Scroll is the same glass. Ship voice is cool cyan `#8FD0DC`. Captain voice is warm amber `#E4B56A`.

Touchable is a faint cyan presence. Touching is brighter. Set is the same family, steady and dimmer. Active is the bright edge on the one action that is underway. Disabled is `#5C6772`.

Light only comes from things doing something: an unset star, a hovered control, a running gauge, a selected orbit, a planet with recent activity, the Save today edge, the Reflect edge. No spotlights. No decorative glow. No fake floor shadow. No ellipse under the radar.

Two literal ship touches, and only these: monospace readouts such as `Log · 2026-09-24`, and one faint tick ring on the radar.

### Professional Light

The same states, drawn with ink weight and a soft shadow. Space `#F3F1EB`. Panel `#FBFBF8`. Inset `#EFECE4`. Scroll is that same light panel. Ship ink `#0E5C62`. Captain ink `#7A4A08`. Text `#1C1B17`.

Touchable is a broken ink ring and a hollow core. Touching is heavier type and a filled core. Set is a steady filled core. There is no glow and no pulse. There is no dark card sitting in the light page. Save today is a light button with ship-colored ink and a small soft shadow.

### Terminal

Herdr is the reference, and the running theme on this machine is Tokyo Night. Stars' two columns match herdr's sidebar and main pane. The planet strip matches herdr's tab strip. One blue accent `#7AA2F7`. Black background. Monospace everywhere (`SF Mono`, then ui-monospace, Menlo).

Space `#000000`. The left column sits on a slightly lifted shade `#0C0E14`. The right column stays black. Ship accent `#7AA2F7`. Captain and body text `#C0CAF5`. Dim `#565F89`. The input rule is `#2A3148`.

Herdr's manners, as the screen is drawn:

- No corner brackets. No box-drawing panel frames. Regions separate by background shade and spacing.
- Section labels are lowercase and dim (`check-in`, `trackers`, `log`).
- A selected planet tab is a solid inverted block: accent background, dark text, and the planet's colored dot still beside the name.
- An unselected planet is plain text plus that dot. A hovered row is a slightly lighter band, not a second inverted block.
- Item headers use `◆`.
- Timestamps and readouts are dim and right-aligned.
- The bottom hint is `←/→:day │ tab:planet │ s:save │ r:reflect`.
- The only rounded border is on a text input. Save today and the selected tab are inverted blocks, not framed boxes.
- Orange and green status marks still mean planet activity. They are not a second chrome accent.

An unset spoke is a dim `*`. A set spoke is `✦`, larger as the value rises. Touching inverts the value. Set labels are bold. The planet chip stays a plain colored dot inside the inverted block.

### RPG

Dragon Quest VIII windows are the base. Space is a textured dark blue `#101C38`. Panel is a blue window `#1E3C9A`, white text `#F7F8FF`, a white rounded border. Captain lines on that window are gold `#FFE7A3`.

Touchable shows a `▶` cursor. Touching is a highlighted menu row. Set is steady white, with no cursor.

Journal and other long text use a parchment scroll `#F0E2C0` with ink `#3B2A16`. Paper is allowed because RPG owns it. The check-in radar and the three win lines stay on the blue window. They are not paper.

## Typography

Two families already on the machine. SF Pro Text for the ship and for the captain's prose. SF Mono for readouts. No downloaded face. No emoji as icons.

Terminal uses SF Mono for everything, including prose.

- **Title:** 0.9rem, weight 600, tracking 0.01em. The banner reads `Stars` in sentence case. Terminal lowercases it.
- **Label:** 0.66rem, weight 600, tracking 0.01em. Section titles are sentence case in Dark, Light, and RPG (`Check-in`, `Trackers`). Terminal lowercases them. They are sans, not uppercase mono.
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
5. A hint line along the bottom. Terminal draws `←/→:day │ tab:planet │ s:save │ r:reflect`. The other official themes keep the same slot and draw it in their own voice.

Check-in holds the radar, then `LOG ·` the date, then Win 1, Win 2, and Grateful, then Save today and Edit. Trackers are one row of small round gauges. The tracker card is as tall as that row. Check-in takes the space above it. Empty copy such as "No nudges yet." is not shown.

The left/right split defaults near 0.52. The check-in/tracker split is content-sized on the tracker side. Double-click still resets the live app's splits. This file does not change those fractions in code.

**Planet chip.** In every official theme the chip sits immediately beside the planet name in the top strip. Dark and Light: an 8px matte disc in `planet-<name>`, no gloss. Activity is a soft halo. Selection is a thin orbit ring. RPG keeps the gem. Terminal keeps a plain colored dot, and the selected name is an inverted block with that dot still inside it. The name is `type-text`, not the planet color. Orange working and green ok may mark activity on the chip. They do not replace the planet color.

**Structure parity** is official-only. A custom theme may rearrange, and should say which part it dropped.

## Elevation & Depth

Depth is not a stack of drop shadows.

In Professional Dark, space is the nebula behind the glass. The glass has a lit top edge, a top-to-bottom shade, and a soft shadow down-right. That shadow is depth, not a floor ellipse and not a spotlight. Hover is a change of light on the thing being touched. `motion-lift` may move a control 1 or 2px, with no shadow growing under it.

`motion-hum` is a slow opacity pulse, about 3.5s, on an unset constellation star, a running gauge, and a planet chip with recent activity. Reduced motion turns the pulse off. Light does not pulse. It uses ink weight. Terminal does not blink and does not pulse. RPG keeps the `▶` still.

In Professional Light, a soft shadow under a panel is allowed. It is ink and paper, not a glow.

In Terminal, shade and spacing separate regions. A lighter row band marks selection.

In RPG, the blue window sits on the textured field. The parchment scroll is the forward surface for journal text.

## Shapes

Panels are one radius on every corner: 10px in Dark and Light, 14px on the RPG window, square in Terminal. The book edge is gone. One straight side and a rounded other side is not a style. It reads as a broken corner. Dark panels do not wear corner brackets.

Controls are 7px, or square in Terminal. Pills are full round where Dark, Light, and RPG use them. Gauges are circles, about 32px. Dark and Light planet chips are flat matte discs, 8px, in `planet-<name>`. At most a two-stop shade. No specular highlight, no glossy marble. Recent activity is a soft thin halo in that same color. Selected is a thin orbit ring in ship linework. RPG keeps a larger gem chip, about 14px, with a highlight, because that chip works in the blue window. Terminal's chip is a plain 8px dot of the planet color, including inside the inverted tab.

Journal lines are a bottom rule, not a rounded box.

## Components

**Radar.** The hero of Check-in. It fills the tile width. Spoke names sit against the chart. Five rings, five spokes: Sleep, Terroir, Athleticism, Relationships, Self. No shadow under it. No radial wash behind it. One faint instrument tick ring on the outer cage is the ship literal.

Each value is a star on its spoke. Unset is a dim small star near the center, a faint dotted track, and five tick marks along the spoke, so it reads as draggable before a touch. Touching brightens the star, lights the track, and shows the value. Set stars grow and brighten from 1 to 5, and only a set star gets a four-point glint. The shape is thin constellation lines between stars that have a value. A missing spoke stays a gap. Fill, if any, is a whisper. Values 1–5 stay typeable. Empty plus Enter clears a spoke. Escape leaves the score. First grab on an unset spoke still sets 3.

Dark draws glowing stars on the glass. Light draws ink stars, weight instead of glow. Terminal draws `*` and `✦`, and inverts the value while touching. RPG draws the star and puts `▶` on the touching spoke.

**Log lines.** Win 1, Win 2, and Grateful sit under the date readout. The date is monospace. The keys are sentence case, and lowercase in Terminal. The words are captain voice. Save today is the one primary action in Check-in. Edit stays quiet. A dirty draft brightens Save today. It does not add a second primary.

**Cloud.** Words float in a nebula, not on a flat black card. The haze drifts, navy into purple, faintly tinted by the planet colors. How often a word appears is how bright and how large it is. The field is dense. Ochre, teal, and violet stay available as word colors in Dark. They are not planet chips and not a page surface. Terminal has no nebula: dim and bright text only, one blue accent. Light is a pale haze and stays light. RPG is the blue window. Drift is a few pixels. Reduced motion holds the words still.

**Journal.** Past entries on `surface-scroll`. One Reflect, on the tab row when pages exist, on the empty page when they do not. Never both. No book edge. In Dark the entries are amber on glass. In Light they are captain ink on the light panel. In Terminal they are captain text on black, with a `◆` header. In RPG they are ink on the parchment scroll.

**Trackers.** Small round lamps in a row, about a third of the old watch-face card. The card shrinks to the row. At rest: lamp, figure, short label. Hover or focus shows decrease and increase, or Start and Stop on a timer. The figure can be typed. A running timer hums and is visually distinct from a set lamp. Planet identity color is not painted on the lamp. The planet is named in dim readout type, or by the chip up in the strip.

**Destination strip.** All stays on its own shelf, outside the scrolling row. Each planet is chip plus name. Recent activity is the soft halo on a Dark or Light chip. The selected planet takes an orbit ring, except in Terminal, where the tab inverts and the dot remains. Overflow fades at the edge and offers a chevron. The scrollbar stays hidden.

**Hint line.** Last, full width. Terminal: `←/→:day │ tab:planet │ s:save │ r:reflect`, dim, monospace, with the draft readout on the right. Other official themes: the same slot, one quiet readout, their own type.

## Do's and Don'ts

**Do**

- Keep the two-column Stars frame and the 4px step in every official theme.
- Declare touchable, touching, active, set, and disabled, and let the theme draw them.
- Put the planet chip beside the planet name in every official theme, including inside a Terminal inverted block.
- Use herdr as the Terminal reference: black, one blue accent, lowercase labels, an inverted selected tab that still shows the dot, no panel frames, `◆` headers, dim right-aligned readouts, the bottom hint line, orange and green activity marks.
- Keep captain words and ship chrome in their own voices.
- Move with transform, opacity, and a shadow only where that theme owns shadow. Reduced motion: no pulse, no blink.
- Keep body text at or above 4.5:1 on its surface. Keep Reflect, Save today, inc, dec, and start/stop targets at least 24px.

**Don't**

- Don't put a light surface in Professional Dark. Don't put a dark island in Professional Light.
- Don't borrow parchment outside RPG, or box-drawing as general chrome outside an input, or herdr reverse-video outside Terminal.
- Don't put corner brackets on Dark panels. Don't paint a repeating star-dot tile. Don't put a glossy highlight on a Dark or Light planet chip.
- Don't add a spotlight, a radial glare, a floor shadow, or an ellipse under the radar.
- Don't bring back the book edge, the orange score dash, or a second Reflect.
- Don't paint planet colors on gauges, banners, buttons, or journal text.
- Don't show "No nudges yet." when nothing is there.
- Don't treat a custom theme as a broken official theme. Custom themes get the soft guidance above, not the structure contract.
- Don't restyle other Orbitty screens from this file alone.
