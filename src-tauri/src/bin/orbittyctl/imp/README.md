# orbittyctl imp

The client body. `main` calls `run` here.

`mod.rs` holds `pub fn run` and declares `usage`, `parse`, `call`, `print`, and `tests`. `usage.rs` is exit consts and `USAGE`. `parse.rs` is `Opts` and flag parsing. `call.rs` is `tool_call` (one match). `print.rs` is the socket, token, `print_gate`, and `print_human`.

`#[path = "tests.rs"] mod tests` keeps bin test names under `imp::tests`.
