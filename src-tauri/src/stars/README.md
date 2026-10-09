# stars

Stars tools on the Rust side.

Start at `stars_ctl.rs`. `handle` (the match over the stars tools) is the entry. Check-in, journal, trackers, timer, and cloud live in that one file. It stays whole: `go/stars-keys` edits the pre-move path, and a split here would fight that rebase.

`mod.rs` declares it. The page is `src/lib/StarsPage.svelte`, also left whole.
