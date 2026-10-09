# companion

The mouth and the companion backend.

Start at `companion.rs` (`mod.rs` is `#[path = "companion.rs"]`). `mouth_route` is the keyword door. `mouth_hop` is the lane hop (local, OpenRouter, handoff).

`talk_mac.m` is compiled from `build.rs`. The page is `src/lib/CompanionPage.svelte`. The separate process is `sidecar/mouth`.
