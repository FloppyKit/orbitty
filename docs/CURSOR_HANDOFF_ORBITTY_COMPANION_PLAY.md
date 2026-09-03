# CURSOR HANDOFF — GO-ORBITTY-COMPANION-PLAY

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_COMPANION_PLAY_LOCK_v0.md` first.

Illegal until `GO-ORBITTY-COMPANION-3D` dogfood-green.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

The GLBs have **no animation tracks**. Play = bone envelopes.

## Do

1. Allowlist: `idle` · `think` · `nod` · `look-left` · `look-right`. `DEF-HEAD` + `DEF-NECK` at 30% of that yaw/pitch. One at a time; end → idle.

2. `companion.play` `{ clip }` on the existing control socket. `orbittyctl companion play nod`. MCP tool + `clip` schema. Companion-profile ACL. Unknown name = error. Spirit body = skipped ok.

3. Reduce Motion skips. Do not auto-play on send. Do not wiggle `DEF-JAW`. No `wave`.

4. `./scripts/install-stable.sh` then git commit `-m "GO-ORBITTY-COMPANION-PLAY"`. Do not push. STOP.

## Do not

`tauri dev`. Stamp. Clip import. Mixamo. Quaternion dump. VMC. Caption leftover.
