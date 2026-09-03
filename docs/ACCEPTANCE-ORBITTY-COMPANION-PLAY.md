# ACCEPTANCE — GO-ORBITTY-COMPANION-PLAY

Dogfood: 3D body selected (Tang or Female). Terminal:

```
orbittyctl companion play nod
orbittyctl companion play look-left
orbittyctl companion play idle
orbittyctl companion play no-such
```

## Must

- `nod` / `look-left` / `look-right` move the head, then return to idle orbit.
- `idle` cancels a pose. `think` holds.
- Unknown clip prints an error (non-zero or honest JSON error). Does not nod anyway.
- Spirit body: play is skipped, no crash, no extra PNG frames.
- Reduce Motion: no head swing.
- Send a chat line does **not** auto-nod.

## Must not

- Jaw chatter
- Wave
- Clip import UI
- New overlay
- `tauri dev` / stamp / push
