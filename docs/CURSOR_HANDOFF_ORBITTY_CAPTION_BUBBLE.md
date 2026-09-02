# CURSOR HANDOFF — GO-ORBITTY-CAPTION-BUBBLE

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_CAPTION_BUBBLE_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Do

1. Collapsed `.cc` idle max-height ~6 lines (`calc(1.3em * 6 + 10px)`), overflow-y auto. Hosted stays 3-line cap. Not 40vh.

2. `arriveReply`: if `!speak`, no pager (full text in the small box). If `speak`, `fitPages` to the box; `say` flips; last chunk stays.

3. Click `.cc.bot` toggles a full-reply overlay **inside** the companion window. Sprite-off hug: overlay `top: 100%` over the dock. Escape / second click closes. Drop `.cc-hover` hover-expand and `.cc-exp`.

4. Pane click must **not** both page and expand. Paging = Enter / Space / `say` auto only.

5. `./scripts/install-stable.sh` then git commit `-m "GO-ORBITTY-CAPTION-BUBBLE"`. Do not push. STOP.

## Do not

`tauri dev`. Stamp. Think-blips. INSPECTOR-HOST. Grow the hug.
