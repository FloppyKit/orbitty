# src-tauri/src

Pointer for the crate.

`lib.rs` is the invoke list (`generate_handler!`) and `run`. `main.rs` calls `orbitty_lib::run`. `objc_try.m` catches `NSException`.

Folders: `agents`, `attach`, `bin`, `board`, `companion`, `coordinator`, `habitat`, `plans`, `rail`, `relay`, `stars`, `state`, `windows`, `yard`.

## bin

There is no `README.md` inside `bin/`. Tauri's bundler treats a non-`.rs` file in `src/bin` as an extra bundle binary (the file stem). `README.md` made it look for `target/release/README` and the app bundle failed. The map for that folder is here.

Three binaries. `lib.rs` does not list them. Tauri bundles them beside the app.

- `orbittyctl/` — control-socket client. Cargo `name` is `orbittyctl`. See `bin/orbittyctl/README.md`.
- `orbitty_mcp.rs` — MCP over stdio. Its `include_str!` is `../agents/control.rs`.
- `orbitty-relay/` — loopback relay. Cargo `name` is `orbitty-relay`. The directory stem matches the file Cargo writes. See `bin/orbitty-relay/README.md`.
