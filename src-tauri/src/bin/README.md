# bin

Three binaries.

- `orbittyctl/` — control-socket client. `name` in `Cargo.toml` is `orbittyctl`.
- `orbitty_mcp.rs` — MCP over stdio. Its `include_str!` is `../agents/control.rs`.
- `orbitty-relay/` — loopback relay. `name` is `orbitty-relay`. The directory stem matches the file Cargo writes.

`lib.rs` does not list these. Tauri bundles them beside the app.
