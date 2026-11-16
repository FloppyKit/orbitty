# orbittyctl imp

The client body. `main` calls `run` here.

`mod.rs` holds `pub fn run` and declares `usage`, `parse`, `call`, `print`, `ledger`, `net`, and `tests`. `usage.rs` is exit consts and `USAGE`. `parse.rs` is `Opts` and flag parsing. `call.rs` is `tool_call` (one match). `print.rs` is the socket, token, `print_gate`, and `print_human`. `ledger.rs` and `net.rs` are local verbs: no control socket, and `net.rs` includes `../../../net/mod.rs` by `#[path]` so the app's permit door and the CLI share one body.

`#[path = "tests.rs"] mod tests` keeps bin test names under `imp::tests`.
