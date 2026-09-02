# CURSOR HANDOFF — GO-ORBITTY-RAIL-REVIEW

Model: kimi-k3-max. One writer.

Read `~/.buzz/PLANS/ORBITTY_RAIL_REVIEW_LOCK_v0.md`.

## Do

1. New `src/lib/orbit-field.css` — exactly Cloud’s four-layer stack as `.orbit-field`. Import once.

2. `src/lib/SkyPage.svelte` — `.field` uses `.orbit-field` (no duplicated hex). Split an embeddable field (words + mute + empty) so Stars Review can mount it without the Cloud banner/×. Bag law, `state.sky.muted`, click/right-click stay. Do not restyle the field.

3. `src/lib/StarsPage.svelte` — banner segmented **Check-in | Review**, default Check-in.
   - Check-in: today’s pentagon, 0–4, Save, Prompts. Body `--bg-fill`. No field.
   - Review: mean pentagon for `averageWindow`; week/14/month chips here only; embed Sky field under the pentagon with `.orbit-field`. No 0–4.

4. `src/routes/+page.svelte` — **no accordion.** Delete Cloud `stack-btn` / `openSky` from the nav. Home is not a stack-btn: wordmark (or a tiny control next to ⚙) calls `bringAllFront`. Window stack only **Companion · Board · STARS**. Move **Projects · Agents · Actions** onto `list-head` as tabs with the count / + Project (they filter the list). Default lane Projects. Gear stays on the banner.

Leave `sky` webview in the app (hide-not-destroy). Do not call `open_sky_window` from the rail.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.

Forbidden: tauri dev, mist on check-in/companion/board/rail, destroying sky, I-DOT, RPG, caption, MH-2, drawers, second writer, STAMP_*.
