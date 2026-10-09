# windows

Window types, the drawer leash, and the AX corral.

`windows.rs` is the types (`mod.rs` is `#[path = "windows.rs"]`). It `#[path]`s `windows_mac.rs` as `platform` and `pub use`s `list_windows`, `focus_window`, and the other public fns. Those `pub fn`s stay in `windows_mac.rs`. The AX catalog walk is `windows_mac_ax.rs`, private to `windows_mac.rs`.

`drawers` places panes. `claim_front.rs` keeps `mod tests` via `#[path = "claim_front_tests.rs"]`.
