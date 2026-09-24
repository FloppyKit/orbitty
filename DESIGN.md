---
version: alpha
name: Orbitty Stars
description: Starter, Stars-derived. Scene tokens from the Stars page only. Not a full product system.
colors:
  sky: "#0B1020"
  ink: "#E8ECF8"
  dim: "#B7C0D8"
  gold: "#C8A45C"
  paper: "#E7E3F4"
  onPaper: "#1B1730"
  paperDim: "#5C5674"
  accent: "#3D8F8A"
  face: "#16182E"
  dustOchre: "#E59E3D"
  dustTeal: "#3D8F8A"
  dustViolet: "#C793E6"
typography:
  title:
    fontFamily: SF Pro Text
    fontSize: 0.9rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.02em
  label:
    fontFamily: SF Pro Text
    fontSize: 0.66rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.04em
  body:
    fontFamily: SF Pro Text
    fontSize: 0.75rem
    fontWeight: 400
    lineHeight: 1.35
  control:
    fontFamily: SF Pro Text
    fontSize: 0.68rem
    fontWeight: 650
    lineHeight: 1.2
  number:
    fontFamily: SF Pro Text
    fontSize: 1.35rem
    fontWeight: 700
    lineHeight: 1
rounded:
  control: 7px
  panel: 10px
  book: 14px
  face: 18px
  pill: 999px
spacing:
  base: 4px
  sm: 8px
  gutter: 10px
  page: 12px
components:
  sky:
    background: "{colors.sky}"
  depth-1:
    fill: "rgba(232, 236, 248, 0.10)"
    line: "rgba(232, 236, 248, 0.22)"
    shadow: "0 1px 0 rgba(232, 236, 248, 0.06), 0 8px 20px rgba(0, 0, 0, 0.28)"
  depth-2:
    shadow: "0 12px 28px rgba(0, 0, 0, 0.45)"
    shadowHover: "0 18px 36px rgba(0, 0, 0, 0.55)"
    hoverLift: "-2px"
---

# Orbitty Stars

Starter, Stars-derived. These tokens are what the Stars page uses after the scene pass. Other Orbitty screens are not covered.

## Overview

Stars is one night room. The page is the sky. Panels are glass on that sky. The radar, tracker faces, and the cloud book sit forward of the glass. The journal opens as a paper page in the same room.

The sky does not go translucent with the window opacity slider. Text on the sky is light. The open journal flips to paper and dark text. Motion is a short lift or a crossfade. Reduced motion drops the travel and keeps the crossfade.

## Colors

The night sky is the shared word-cloud field: `#0B1020` plus three dust radials (ochre `#E59E3D` at 9%, teal `#3D8F8A` at 11%, violet `#C793E6` at 9%). Do not invent a second sky.

- **Sky (`#0B1020`):** the page. Opaque. Depth 0.
- **Ink (`#E8ECF8`):** body text on the sky. With dim `#B7C0D8`, both clear 4.5:1 on `#0B1020`.
- **Gold (`#C8A45C`):** scored radar marks, the saved radar edge, and a running timer ring. Unscored marks are hollow, not gold, and not orange.
- **Accent (`#3D8F8A`):** Save today, and the selected planet pill. Label on the accent is sky `#0B1020`.
- **Paper (`#E7E3F4`):** the open journal. Text on paper is `#1B1730`. Secondary text is `#5C5674`.
- **Face (`#16182E`):** tracker watch faces. Light ink on this fill.

Dust ochre is sky atmosphere only. It is not a score color.

## Typography

One family: SF Pro Text, then SF Pro Display, system-ui, -apple-system, Helvetica Neue, sans-serif. No new face, no emoji as icons.

- **Title:** 0.9rem, weight 600. The STARS banner.
- **Label:** 0.66rem, weight 600, tracking 0.04em. Check-in and Trackers headings. Quiet.
- **Body:** 0.75rem, line-height 1.35. Journal lines and journal pages.
- **Control:** 0.68rem, weight 650. Tabs, pills, Save today.
- **Number:** 1.35rem, weight 700, tabular figures. Tracker faces. A running timer uses 1.05rem so hours fit.

Radar spoke names are 10 SVG user units and scale with the dial.

## Layout

Base unit is 4px, as stepped in the page (4px chip and tab gaps, 8px banner gap and tracker grid gap). The body gutter is 10px top and 12px sides and bottom. Do not respace the two columns to chase a new grid.

Left column: Check-in over Trackers, split by the existing drag handle (default check-in fraction 0.62). Right column: the book (default left fraction 0.52). The radar grows inside Check-in. Tracker faces grid-fill their card. All sits on its own shelf in the strip. Those are the sizing exceptions.

## Elevation & Depth

Three tiers, used as tokens on the Stars surface.

- **Depth 0 — sky.** The page itself. No panel fill behind the banner or the strip.
- **Depth 1 — glass.** Check-in and Trackers cards. Fill `rgba(232, 236, 248, 0.10)`, hairline `rgba(232, 236, 248, 0.22)`, shadow `0 1px 0 rgba(232, 236, 248, 0.06), 0 8px 20px rgba(0, 0, 0, 0.28)`.
- **Depth 2 — touchable.** Radar, each tracker face, and the book. Shadow `0 12px 28px rgba(0, 0, 0, 0.45)`. Hover and keyboard focus lift `translateY(-2px)` and grow the shadow to `0 18px 36px rgba(0, 0, 0, 0.55)`.

The book cover wears a left spine and a right-edge shade. The open page wears a left gutter shadow. Cover to page is one opacity-and-transform transition (about 280ms). Reduced motion is an opacity crossfade only.

## Shapes

- Controls and journal underlines: 7px where a control is a button. Journal lines are a bottom rule, not a rounded box.
- Glass cards: 10px.
- Book: 14px on the fore-edge, 4px on the spine edge.
- Tracker faces: 18px, a squircle.
- Pills and the selected planet: full pill (999px). Unselected planets have no outline.

## Components

**Radar.** Fills the Check-in card. Five rings, ghost knobs, gold edge on a saved shape. Beside each spoke name: a hollow dot when unscored, a gold dot when scored, with the typed score beside the gold dot. The dial lifts on hover.

**Journal lines.** Win 1, Win 2, and Grateful are underline fields with the prompt as placeholder text. Save today is the only primary button in Check-in. Edit stays a quiet text button.

**Book.** Cloud is the cover (night field, spine, depth-2 shadow). Journal is the paper page. An empty journal is that page, headed with today's date, and one Reflect button. A journal that already has pages keeps Reflect on the tab row instead, still one control.

**Tracker faces.** At rest: planet chip, label, number. Hover or focus reveals − and + over the face without moving the number. Click the number to type; Enter or blur commits a whole number, Escape cancels. The overflow action is `…`, and only on hover or focus.

**Timer faces.** Same face. Hover or focus shows Start or Stop. A running timer keeps a gold ring.

**Strip.** All sits on a raised shelf, separate from the planets. Unselected planets are a dot and a name. The selected one is a pill. Overflow uses a 2px moving cue. The native strip scrollbar stays hidden.

## Do's and Don'ts

**Do**

- Keep the two-column split and the 4px step.
- Use Orbitty type and the marks already on the page (planet dot, −/+, ×).
- Move with transform, opacity, and box-shadow only.
- Honor `prefers-reduced-motion`: no lift, opacity crossfade for the book.
- Keep body text at or above 4.5:1 on the sky and on the paper. Keep Reflect, inc, dec, and start/stop targets at least 24px, with names.

**Don't**

- Don't paint a second sky, a neon edge, or an XP costume.
- Don't bring back the orange dash, the boxed win fields, the mist strip under the journal tabs, or a second Reflect.
- Don't add a font or an icon family.
- Don't let window opacity punch a hole through the Stars sky.
- Don't show "No nudges yet." when nothing is there.
- Don't put this system on other screens from this file alone. It is a starter.
