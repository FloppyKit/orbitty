# Face-map pass 2 — 🎨 Bot · pixel-artist

## Identity
Session title: **🎨 Bot · pixel-artist**

## Primary: manual UV island
- Blend: `femhuman_pass2_uv_island.blend`
- Front faces selected by normal·front + Y bias → mat slot face plate
- Back/side → solid skin
- UV island: orthographic XZ of front verts → 0..1 plate
- Renders: `turns/uv_*.png`

## Backup: face-card
- Blend: `femhuman_pass2_face_card.blend`
- Textured plane parented in front of head
- Renders: `face_card/card_*.png`

## Eyes-on
Compare strip: `pass2_compare_strip.png`
Pick UV if readable; else iterate card scale/placement or UV thresholds.

## Memory locks
- brand/walkers/PIXEL_ARTIST_IDENTITY.md
- bodies_dx/FACE_MAP_PASS2_LOCK.md
- ~/.buzz/PLANS/PIXEL_ARTIST_TITLE_LOCK.md
- pixel-artist.md agent note
