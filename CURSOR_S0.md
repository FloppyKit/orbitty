# Cursor Ultra — GO-LS-TAURI-S0

**Model:** kimi-k3-max  
**One writer.** Pixel Artist org. Path: `/Users/dude/orbitty`

## Mission
Scaffold **Tauri 2 + Svelte 5 + TypeScript + Vite** Orbitty hello shell. Exit: `tauri build` or `tauri dev` works; tray stub; README + docs laws.

## Constraints
- NO Accessibility/window module (S1)
- NO sprites/planets (S3)
- NO Android
- Do NOT add features to the Swift tree (path in git history before GO-ORBITTY-RENAME-MOVE) (freeze)
- MIT license
- identifier: the pre-Orbitty app id (see git history); now `space.orbitty.mac`
- product name Orbitty (S0 used the pre-Orbitty name)

## Steps
1. If dir partial, complete or recreate clean scaffold under `/Users/dude/orbitty`
2. Prefer: `npm create tauri-app` / create-tauri-app with template svelte-ts, tauri 2, npm
   OR hand-equivalent if needed
3. Install deps; ensure `npm run tauri build` succeeds (or `tauri dev` smoke)
4. Tray/menu stub in Rust (show window / quit)
5. Hello UI: wordmark Orbitty + one-liner “Runtime consolidator (Tauri S0)” + note Swift is reference
6. README.md as specified below
7. docs/THIN_SLICE_LAWS.md + docs/SWIFT_REFERENCE.md pointers
8. LICENSE MIT
9. .gitignore for node_modules, target, dist

## README must include
- Product one-liner: living visual surface consolidating what’s running (planets + claimed sprites) — seed of LifeRPG, not a harness
- the Swift tree (path in git history) = **reference only**; no dual-write feature parity
- Thin-slice: explicit claim only; unmapped=0 sprites; no harness-in-engine; FKT separate; not EHR
- How to run: `npm install` · `npm run tauri dev` · `npm run tauri build`
- Roadmap: the full-Tauri roadmap in `~/.buzz/PLANS` (2026-08)

## docs/THIN_SLICE_LAWS.md
Copy bullets from nest thin-slice laws (claim, unmapped=0, zombie, points explicit complete, no harness in engine, FKT separate, not EHR). Point to nest for full text.

## When done
Print: tree top-level, how to run, build result, any residual limits. Do not write nest STAMP (Pixel stamps).
