# Cursor Ultra — GO-ORBITTY-HEADROOM-SPIRIT-ROLLBACK

**Model:** `kimi-k3-max` · **one writer**
**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-HEADROOM-SPIRIT-ROLLBACK`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_HEADROOM_SPIRIT_ROLLBACK_LOCK_v0.md`

## Why

Headroom 18:39 killed the 128px crop. Jacob bust is a keep. Spirit `bust_*` is a timid reframe of almost the whole wisp — worse than the old closeup. Default body is spirit, so expanded Head is the regression.

Matt 18:45: roll back spirit. Stamp Full + closeup as good for the wisp. No 3D window for spirit. Jacob leaves as-is.

## Law

1. Keep Full / Head chrome, two prefs, float + react-rotate, Reduce Motion stills.
2. Spirit Head = **old tight closeup** (top ~128px of idle/think, face of the wisp filling the stage). Not the timid bake. Not Head disabled.
3. Preferred: rebake `brand/companion/spirit/bust_0…7` and `thinking/bust_0…7` as a **tight** camera — crop the upper ~128–160px of each idle/think frame, nearest-scale onto 256×384, hard alpha, same 140ms tick, one rect from frame 0. Then keep serving them as Head.
4. Literal rollback also legal: restore CSS `.head { height: 128px }` **for spirit only** when Head is showing, and unbundle timid spirit busts so they are not used. Jacob still uses `bust_*`.
5. After this GO, spirit Head must look like 18:38, not 18:39.
6. Do not touch Jacob bust files, `bake_jacob_bust.py`, or Jacob pack manifest.
7. Spirit stays default. No picker. No CoS. No habitat. No Talk. No `tauri dev`. No `STAMP_*`.

## Ship

- Spirit Head closeup restored (tight bake **or** spirit-only crop).
- `tauri.conf.json` resources match whatever Head actually loads.
- `./scripts/install-stable.sh` when product code/art is in the bundle.

## Forbidden

Jacob restyle. CoS clay / idle. `allowlist.json`. habitat_mac.rs. Marbles. Sky. STARS. Rail marks. Opening vendor `.blend`. Live 3D.

When done: install, stop, say spirit Head is the old closeup again.
