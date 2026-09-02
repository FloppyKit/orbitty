# Cursor Ultra — GO-MH-1-APP

**Model:** `kimi-k3-max` if listed, else `composer-2.5` · **one writer**
**Gate:** Matt **GO-MH-1-APP** via Pixel Artist **2026-08-27** (applied)
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/CASCADE_COMPANION_MICROHARNESS_v0.md`

**Files-only MH-1 is already applied (2026-08-27):**
- `~/.agents/agents/spark.md`
- `~/.agents/companions/orbitty-primary/allowlist.json` (spark, grok, chaos-gremlin, fkt-homie; default spark)
- `soul.json` agent_stem → spark
- Shared session stays `20260825_2`

This GO is **Orbitty filter + default resolve + stable install**. No new persona files.

## Why
`companion_agents` listed every stem under `~/.agents/agents`. Companion should show the four-cast when `allowlist.json` exists. Missing/bad JSON = fail open + log once.

## Applied
`coordinator.rs`: `berd_companion_stems()` filters to allowlist ∩ existing `.md`, preserve allowlist order. `resolve_agent_prefer` defaults to spark. `set_companion_agent` rejects non-members with `not on the Orbitty four-cast`. Hermes path unchanged. `companion_send` unchanged.

Acceptance: `docs/ACCEPTANCE-ORBITTY-MH-1-APP.md`
