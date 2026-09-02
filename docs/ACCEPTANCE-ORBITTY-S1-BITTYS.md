---
title: "Acceptance — GO-ORBITTY-S1-BITTYS"
tags: [orbitty, acceptance, bitty]
status: active
created: 2026-08-23
clinical_claim: false
go: GO-ORBITTY-S1-BITTYS
---

# Acceptance — Season 1 Bittys

Pixel Artist verifies on `~/Applications/Orbitty.app` (LiveSprite Local Codesign). Child self-report is not proof.

- [ ] Blessed PNGs from `brand/bitty-crew/New Crew/planet-sprites/out` are the default planet walkers (not glyph `chars.rs` people, not old Imagine guy/cats as the default)
- [ ] Eleven kinds: boy girl man woman old_man old_woman cat dog cow horse chicken
- [ ] Each kind has default + 3 named palettes (the blessed recolor PNGs). Orbitty plays those bitmaps — it does not invent pixels
- [ ] Walk: 4 frames, 32×32, ~170 ms, hard alpha. Engine slices the 128×32 sheet or loads `walk_0..3.png`
- [ ] All shipped frames face **right**. Habitat already flips when roam.dx < 0. Humans in `out/` face left — must be flipped per 32×32 cell at pack time
- [ ] `man_walk_v1.pxo`, NOTES.md, GIFs, 4× GIFs are **not** in the .app
- [ ] New claim on a planet = a Season 1 Bitty walking
- [ ] Persist: same claim, same Kind+Palette after relaunch
- [ ] Old `charLook` (species/clothes/eyes/mouth/hat) maps to nearest Kind+Palette; no crash
- [ ] Being editor: **Kind · Palette · Size · Use my image…** only. Hair/Clothes/Eyes/Mouth/Body cyclers gone from the default path
- [ ] Single-click walker still opens Being editor. Double-click still restores
- [ ] User local PNG/folder still wins. No itch/Kenney/Habitica fetch
- [ ] No companion. No STARS. No EHR. No new kinds
- [ ] `./scripts/install-stable.sh` → `~/Applications/Orbitty.app` signed LiveSprite Local Codesign
- [ ] Pixel Artist opened that app and **saw** these walkers on a planet (not a Cursor summary)
