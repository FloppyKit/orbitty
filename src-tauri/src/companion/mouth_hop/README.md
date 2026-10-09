# mouth_hop

Lane hop for a companion line.

`local.rs` is curl, the tmp file, Ollama, LM Studio, and `local_chat`. `openrouter.rs` is the key, quota, lanes, and `decide_vision_gate`. `handoff.rs` is `PendingHandoff`, dispatch, and the settings setters. `mod.rs` re-exports them.

`tests.rs` is `#[path] mod tests`, so names stay `companion::mouth_hop::tests`.
