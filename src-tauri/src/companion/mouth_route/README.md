# mouth_route

Keyword door. A miss stays chat.

Start at `mod.rs`: `classify`, `run`, `run_outcome`, and the keyword doors. `write.rs` parses, resolves, decides, and commits board-add and agent-spawn. `mod.rs` re-exports `write`.

`tests.rs` is `#[path] mod tests`, so names stay `companion::mouth_route::tests`. The `#[cfg(test)]` helpers live in `write.rs` and are `pub` so the tests call them.
