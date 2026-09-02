# Walker motion — polygon glitch

**Intent (Matt 2026-08-26):** when the faceted doll animates, let **polygons glitch a little** with movement — not seizure, not dissolve; a slight low-poly shimmer so facets feel alive.

## Spec
- On walk/idle frames: tiny per-vertex or per-face offset (noise), phase-locked to walk cycle
- Prefer **silhouette-stable** glitch (interior facets jitter more than outer ink)
- Amplitude: subtle at planet scale (32–64px); a bit more readable at portrait
- Optional: 1-frame “crack” every N frames for personality — keep rare

## Implementation
- Blender factory: `--glitch 0.0..1.0` on `sheet_walker_v0.py` (default ~0.35)
- Future PNG packs: pre-render glitch into sheet frames; engine can just flip frames
