# orbitty-relay

Loopback NIP-01 sidecar. Not a Tauri command.

Start at `main.rs`. It declares `nip.rs`, `store.rs`, and `ws.rs`. Default bind is `127.0.0.1:13120` (`ws://127.0.0.1:13120`). Kinds 420/421/422.

`relay/event_relay.rs` publishes here. The bin reads the device key and does not log it.
