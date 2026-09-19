//! Décor hit kinds (plant / mushroom / building / boombox).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

/// Which decor item a frame belongs to (plants[i], mushrooms[i], boombox).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecorKind {
    Plant,
    Mushroom,
    Building,
    Boombox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecorRef {
    pub(crate) kind: DecorKind,
    pub(crate) index: usize,
}

