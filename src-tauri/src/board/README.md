# board

The kanban board on the Rust side.

Start at `board.rs` (`mod.rs` is `#[path = "board.rs"]`). `save_with_events` is the board save that emits `state:changed`.

`card_links` normalizes file and https links. `needed_rules.rs`, `nextcard.rs`, `task.rs`, and `todo_trees.rs` sit beside it. The Svelte board is `src/lib/KanbanPage.svelte`.
