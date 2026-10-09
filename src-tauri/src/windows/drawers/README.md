# drawers

Where panes sit. Callers still use `crate::windows::drawers` (and `crate::drawers` where `lib.rs` re-exports it).

`mod.rs` re-exports `rt`, `place`, `stage`, `show`, and `drag`. `rt.rs` is runtime, raise, the mount check, and the AX web-area walk. `place.rs` is the rect law, `place_chrome`, clamp, and blossom. `stage.rs` is rail stage apply, alpha, and order. `show.rs` is heal, persist, `show_drawer`, and wire events. `drag.rs` is the companion drag grab. `pane_mounted` stays `pub`.

`tests.rs` is `#[path] mod host_dock_tests` on this parent. That module name was already `host_dock_tests` before the split, so the test names did not change.
