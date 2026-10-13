# relay

Event log, device key, and publish. Inside the app, not the sidecar bin.

`event_log.rs` appends `events.jsonl` (kind 31200, and board kinds 420/421/422). A log failure does not fail the click. `event_keys.rs` is the Schnorr device key (`device.nsec`, mode 0600, never logged). `event_relay.rs` publishes a best-effort EVENT to `ws://127.0.0.1:13120` after a signed board proof.

`history.rs` writes the board's local-only history into the same `events.jsonl`, from inside `state::update` after each save: 423 card add/set, 424 card tombstone, 425 planet, 426 checkpoint (full board in `history/checkpoint-*.json`). Every line is tagged `t=local-only` and is never published. `fold` rebuilds a board from a checkpoint and the lines after it.

`rewind.rs` is the read-back side. Pure parts: `board_at` (the board just after a line), `full_card`, `plan_card` / `plan_board`, `drift`, and `rows` (history in plain-word rows). App parts: `apply` writes a checkpoint and then one save under actor `rewind` or `repair`, `launch` runs the drift check on its own thread after setup, and `repair` / `keep` close the drift Needed card without a `needed-answer` line. Tests are in `rewind_tests.rs`.

The process that binds that port is `src/bin/orbitty-relay/`.
