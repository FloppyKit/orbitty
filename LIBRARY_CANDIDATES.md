# Library candidates

List only. Nothing here is extracted. No new crate. No `Cargo.toml` dependency change.

Checked against the tree after the code-map splits. A row is a module that already has a boundary. Files this GO only split are not extra rows.

## Relay sidecar

`src-tauri/src/bin/orbitty-relay/` (`main.rs`, `nip.rs`, `store.rs`, `ws.rs`). Loopback NIP-01 for kinds 420/421/422. Binds `127.0.0.1:13120`. Already a bin with no Tauri. Coupled to the Orbitty data dir (`~/Library/Application Support/space.orbitty.mac`) and the rule that a non-loopback serve refuses a copied device nsec. Outside Orbitty: a one-user loopback relay.

## Event log

`src-tauri/src/relay/event_log.rs`. Append-only `events.jsonl`. A log failure does not fail the click. Kind 31200, plus board kinds 420/421/422. Coupled to the Tauri app-data path. There is no replay reducer in this tree. `state/merge.rs` merges window saves, deltas, and tombstones. HISTORY-REWIND is a later GO. Outside: an append-only local log.

## Device key

`src-tauri/src/relay/event_keys.rs`. Schnorr signatures, `device.nsec`, mode 0600, never logged. Coupled to app data. Outside: a local Nostr identity file.

## Event publish

`src-tauri/src/relay/event_relay.rs`. Best-effort EVENT to `ws://127.0.0.1:13120` after a signed board proof. Coupled to `event_keys` and `event_log`. Outside: a thin publisher in front of that relay.

## Control socket

Server `src-tauri/src/agents/control.rs` (Unix sock 0600, newline JSON, errors 1/3/4/5). Client `src-tauri/src/bin/orbittyctl` (exit 0/2/3/4/5/1). Coupled to every tool the app implements. The protocol could leave. The tool list stays Orbitty. Outside: a local app remote.

## Window corral

`windows.rs` `WindowInfo`, and `windows_mac.rs` for the AX list and focus. Coupled to AppKit and Tauri. Outside: a macOS window picker. The drawer leash in `windows/drawers` stays Orbitty.

## Stars model

Serde slice plus `stars_ctl.rs` `handle` (check-in, journal, trackers, timer, cloud). Coupled to `PersistedState` and `board::save_with_events`. Outside: a day log. The page stays Orbitty.

## GO seal

`plans/seal.rs`. SHA-256 and the sealed/launchable check (`seal_note` returns `None` when the GO is sealed and launchable) over the GO header. Coupled to the plans-dir layout and the header schema. Outside: another kitchen that seals markdown specs.

## Card-link normalizer

`board/card_links` `normalize_link` in `norm.rs`. File and https only, percent-encoded paths. Coupled to the approval file beside state. Outside: a strict file/https opener.

## Mouth sidecar

`sidecar/mouth`. Already its own process. `serve.py` binds `127.0.0.1:13121`. Recorded as already split out. Not moved by this GO.
