#[path = "windows.rs"]
mod root;
pub(crate) use root::*;
pub(crate) mod claim_front;
/// Planet scenes (GO 42b): the AX frame convention, and nothing else.
pub(crate) mod scene;
pub(crate) mod drawers;
pub(crate) mod glass;
pub(crate) mod panes;
pub(crate) mod tilechords;
pub(crate) mod window_chrome;
pub(crate) mod window_thumb;
