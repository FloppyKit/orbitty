# ACCEPTANCE — GO-MH-1-APP

status: stamped 2026-08-28
created: 2026-08-27
go: GO-MH-1-APP 2026-08-27
stamped: yes — Matt 2026-08-28. Picker that shipped (spark/grok/chaos-gremlin/fkt-homie). Recast is a later talk.

Requires MH-1 files already on disk (spark.md + allowlist.json).

- [ ] `orbittyctl companion agents` shows spark, grok, chaos-gremlin, fkt-homie — not the whole `~/.agents/agents` tree
- [ ] Default / selected is spark when persisted profile is empty or unknown
- [ ] set-agent grok works; set-agent a non-allowlisted stem errors honestly (`not on the Orbitty four-cast`)
- [ ] Missing allowlist.json: picker falls back to full stem list (fail open) — Pixel can verify; do not delete Matt's file
- [ ] Blessed install only; no hermes-path change
- [ ] Shared companion session id unchanged (`20260825_2`)
