# CURSOR HANDOFF — GO-ORBITTY-LP-ACTOR

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_LP_ACTOR_LOCK_v0.md` first. **Illegal until LP-NEEDED dogfood.**

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/state.rs` — `actor` on `KanbanCard` + `PointsEntry`
- `src/lib/KanbanPage.svelte` — `completeCard` stamps `user`; Done chips
- Companion / Rail — **none**

## Do

1. Additive `actor?: "user" | "agent" | "action" | "vps"` on card and ledger. Old Done = missing = HUD treats as user.
2. `completeCard` (checkbox + drag-to-Done) stamps `actor: "user"` on the card and on the ledger row if written. Award math unchanged. Needed still never enters this door.
3. Done sidebar (and Board Done header): chips **All / I did / On my behalf**, default All. I did = `user` or missing. On my behalf = `agent|action|vps`.
4. Leave `runAction` alone (it does not complete cards today).
5. `./scripts/install-stable.sh`; stop.

## Do not

Extra columns. Agents dest. Talk. Change DAILY_CAP. Stamp. Fold #3–#4. Complete Needed.
