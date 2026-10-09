# card_links

File and https link normalize, preview, and launch.

`norm.rs` is `normalize_link`, file URLs, percent-encoding, and open commands. `preview.rs` is `LinkPreview`, approvals, bplist, `launch`, and `reveal`. `mod.rs` re-exports both.

`tests.rs` is `#[path] mod tests`, so names stay `board::card_links::tests`. `OpenGuard` lives in that file and is re-exported for tests.
