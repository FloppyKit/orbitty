# plans

GO folder, header, seal, and rules.

Start at `mod.rs`. The crate docs (plans folder and the shared settings file) sit here. `seal.rs` hashes a GO and checks sealed/launchable (`seal_note`). `dirs.rs` is the folder layout. `parse.rs`, `locate.rs`, `resolve.rs`, and `rules.rs` sit beside it.

`tests.rs` is one `mod tests`. It stays one file: the helpers span the clusters.
