---
title: "Acceptance — LiveSprite Tauri S1 (window list + focus)"
tags: [livesprite, tauri, acceptance, s1]
status: active
created: 2026-08-21
clinical_claim: false
go: GO-LS-TAURI-S1
---

# Acceptance — GO-LS-TAURI-S1

Tree: `~/goose artifacts/livesprite-tauri`

- [ ] Enumerate open macOS windows: title + app name + stable-ish id
- [ ] Click/select row focuses that window (bring front; deminiaturize if needed)
- [ ] Frontmost window highlighted (or clearly marked) and updates on refresh
- [ ] AX denied → empty-state with grant path + Retry (no silent empty list)
- [ ] Shows bundle path and signing honesty when relevant (adhoc vs identity)
- [ ] List is **unmapped only** — no claim UI, no sprites, no planets
- [ ] Titles only — no window content scrape / not EHR
- [ ] Own LiveSprite windows excluded from list
- [ ] `npm run tauri dev` works
- [ ] Poll/refresh ~0.5–1s or on focus change (good enough dogfood)
