# habitat_mac

The Mac habitat panel.

Start at `mod.rs`. `tick.rs` is the frame. `draw.rs` paints (`draw_world.rs`, `draw_fx.rs`, `draw_board.rs` hang off it). `input.rs` is the pointer.

`habitat/mod.rs` pulls this in with `#[path]` on macOS.
