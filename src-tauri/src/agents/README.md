# agents

Agent control, the dev stamp, and card close.

Start at `mod.rs`. It `#[path]`s `agents.rs` and declares the children. `control.rs` is the Unix control socket server (mode 0600, newline JSON, errors 1/3/4/5). `thin.rs` is the dev stamp; its `mod tests` is `#[path = "thin_tests.rs"]`. `verify_close.rs` closes a card.

`orbittyctl` is the client of `control.rs`.
