# yard_mac

The Mac yard panel.

Start at `mod.rs`: the view class, `motion_step`, `hop`, `spawn_worker`, and the menu callbacks. `cat_rect`, `hit_cat`, `sender_represented`, and `remove_cat` stay here too, with the menu popup and `menu_assign`.

`geom.rs` is the rect types, `visible_frame_for`, and `walk_rect`. `art.rs` loads cat images. `life.rs` is `esc_block` through `stop_on_main`. `mod.rs` re-exports `active`, `start`, and `stop`. `input.rs` is the hit test. `meow.rs` is sound.

`yard/mod.rs` pulls this in on macOS.
