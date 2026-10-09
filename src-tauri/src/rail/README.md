# rail

Rail edge, stage, and toggle on the Rust side.

Start at `rail_edge.rs`, `rail_stage.rs`, and `rail_toggle.rs`. `mod.rs` declares them. `rail_edge.rs` keeps `mod tests` via `#[path = "rail_edge_tests.rs"]`, so names stay `rail::rail_edge::tests`.

The page is `src/lib/RailPage.svelte`. Stage geometry in TypeScript is `src/lib/rail/stageArch.ts`.
