//! Yard. Unclaimed windows rain onto the
//! desktop as bitty cats (one fullscreen click-through overlay per
//! display) until each is claimed (Assign), closed, or scooped. Session
//! only: cats never persist, and planets never paint unmapped sprites
//! (thin-slice law 2 stands — yard cats are not habitat bodies).
//! Platform dispatch mirrors habitat.rs.

#[cfg(target_os = "macos")]
#[path = "yard_mac/mod.rs"]
mod yard_mac;

#[cfg(not(target_os = "macos"))]
#[path = "yard_stub.rs"]
mod yard_mac;

pub use yard_mac::{active, start, stop};
