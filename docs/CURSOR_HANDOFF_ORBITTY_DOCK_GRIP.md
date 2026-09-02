# CURSOR HANDOFF — GO-ORBITTY-DOCK-GRIP

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_DOCK_GRIP_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Why resize arrows click through

`.ghost .ghostdock` is inset (`margin: 4px 8px 8px`). Hits = that box. Native resize is the **window frame**. Those pixels are empty → poll mouse-off.

## Do

1. Ghost only. Paint stays inset. Hits: from dock **top** to window **bottom**, **full width** (`x: 0, w: innerWidth, h: innerHeight - dockTop`). Do not report the painted inset as the dock rect.

2. Ghost dock `onmousedown`: if the target is not a control (`input` / `textarea` / `select` / `button` / `.field`), `startDragging()`. Do not steal dests / agent / send / talk.

3. No rust rewrite. `companion_clear.rs` poll stays.

4. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. PNG sampling. Walker menu. Action edit. Click-through on well/expanded. Shrink/grow the spirit box.
