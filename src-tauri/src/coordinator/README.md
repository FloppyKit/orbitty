# coordinator

Companion launch and send. Titles only. No LLM in this module (`mod.rs` crate docs).

Start at `mod.rs`. `send.rs` is the send path (hermes, berd, goose, grok). `hermes.rs` is the hermes profile. `plug.rs`, `mouth_cli.rs`, `commands.rs`, `intercept.rs`, `soul_wake.rs`, `action.rs`, `paths.rs`, `cursor_perm.rs`, and `launch_agent.rs` sit beside it. `mac.rs` is macOS-only.

`lib.rs` mounts this module. The page calls `companion_send`.
