---
title: "Acceptance — AVATAR-LARGE S2.4"
tags: [orbitty, acceptance, avatar]
status: active
created: 2026-08-23
clinical_claim: false
---

# Acceptance — send + agent + CC + expand

- [ ] Send does **not** only focus Hermes / does **not** show a `…/companion.md` path
- [ ] Send actually talks to the chosen Hermes **profile** (`hermes -p … chat -q` + `--continue orbitty-companion --create-if-missing -Q`)
- [ ] Agent picker = profiles under `~/.hermes/profiles/` (+ default). Persist. Default `grok` if present
- [ ] Collapsed: 2-line CC (last you / last them). No fake replies
- [ ] Expand: transcript + small ghost **head** (top crop of idle frame) top-right; × stays left; Collapse returns
- [ ] Ghost still drags when collapsed. Composer never a drag region
- [ ] `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
