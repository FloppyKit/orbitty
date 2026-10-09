# relay

Event log, device key, and publish. Inside the app, not the sidecar bin.

`event_log.rs` appends `events.jsonl` (kind 31200, and board kinds 420/421/422). A log failure does not fail the click. `event_keys.rs` is the Schnorr device key (`device.nsec`, mode 0600, never logged). `event_relay.rs` publishes a best-effort EVENT to `ws://127.0.0.1:13120` after a signed board proof.

The process that binds that port is `src/bin/orbitty-relay/`.
