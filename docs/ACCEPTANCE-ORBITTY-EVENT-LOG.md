---
title: "Acceptance — GO-ORBITTY-EVENT-LOG"
status: wait_go
created: 2026-09-01
go: GO-ORBITTY-EVENT-LOG
---

- [ ] File exists at `~/Library/Application Support/space.orbitty.mac/events.jsonl` after first event
- [ ] One line = one JSON object with `id`, `pubkey=""`, `created_at`, `kind=31200`, `tags`, `content`, `sig=""`
- [ ] Board Todo→Doing writes `e-type=board-move` with card id in content
- [ ] Needed mint / answer / skip / later each write their e-type
- [ ] Wake sleeper writes `sleeper-wake`
- [ ] Companion send does **not** put the message text in the log
- [ ] Forced IO failure (chmod 000 on the file, then chmod back) does not break sending or saving
- [ ] No rusqlite, no relay, no TCP
- [ ] `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
