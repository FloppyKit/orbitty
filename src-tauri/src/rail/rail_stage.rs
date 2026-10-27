//! Pure rail visibility. AppKit apply lives in `drawers`; this module only
//! decides Raise vs Park.

pub const RAIL_AWAY_MS: u64 = 700;
/// `NSFloatingWindowLevel` / `kCGFloatingWindowLevel`.
pub const RAIL_LEVEL_FLOATING: i64 = 3;
pub const RAIL_LEVEL_NORMAL: i64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailVis {
    Parked,
    Revealed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailEvent {
    EdgeReveal,
    Hotkey,
    Expand,
    ParkRequest,
    RaisedOther,
    Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailAct {
    Raise,
    Park,
    Emit(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailFacts {
    pub app_active: bool,
    pub key: bool,
    pub occluded: bool,
    pub cursor_in_frame: bool,
    pub away_ms: u64,
    pub occluded_ms: u64,
    pub button_down: bool,
    pub auto_hide: bool,
    /// Fresh coach is still due. Tick, hotkey, and park requests must not hide it.
    pub coach_hold: bool,
}

impl RailFacts {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn inactive_clear() -> Self {
        Self {
            app_active: false,
            key: false,
            occluded: false,
            cursor_in_frame: false,
            away_ms: 0,
            occluded_ms: 0,
            button_down: false,
            auto_hide: true,
            coach_hold: false,
        }
    }
}

fn hold_open(state: RailVis) -> (RailVis, Vec<RailAct>) {
    if state == RailVis::Parked {
        return (RailVis::Revealed, vec![RailAct::Raise, RailAct::Emit("coach")]);
    }
    (RailVis::Revealed, Vec::new())
}

/// One step of the reveal / raise machine. `Tick` parks only when auto-hide
/// is on, Orbitty is inactive, no button is down, and either the cursor has
/// been outside the frame or the rail has been occluded for `RAIL_AWAY_MS`.
pub fn rail_vis_step(state: RailVis, event: RailEvent, facts: &RailFacts) -> (RailVis, Vec<RailAct>) {
    if facts.coach_hold {
        return hold_open(state);
    }
    match event {
        RailEvent::EdgeReveal => (RailVis::Revealed, vec![RailAct::Raise, RailAct::Emit("edge")]),
        RailEvent::Expand => (RailVis::Revealed, vec![RailAct::Raise, RailAct::Emit("expand")]),
        RailEvent::Hotkey => match state {
            RailVis::Parked => (RailVis::Revealed, vec![RailAct::Raise, RailAct::Emit("hotkey")]),
            RailVis::Revealed if facts.occluded || !facts.app_active => {
                (RailVis::Revealed, vec![RailAct::Raise, RailAct::Emit("hotkey")])
            }
            RailVis::Revealed => (RailVis::Parked, vec![RailAct::Park, RailAct::Emit("hotkey")]),
        },
        RailEvent::RaisedOther => (RailVis::Parked, vec![RailAct::Park, RailAct::Emit("raised")]),
        RailEvent::ParkRequest => (RailVis::Parked, vec![RailAct::Park, RailAct::Emit("request")]),
        RailEvent::Tick => {
            if state != RailVis::Revealed {
                return (state, Vec::new());
            }
            if !facts.auto_hide || facts.button_down || facts.app_active {
                return (state, Vec::new());
            }
            let away = !facts.cursor_in_frame && facts.away_ms >= RAIL_AWAY_MS;
            let buried = facts.occluded && facts.occluded_ms >= RAIL_AWAY_MS;
            if away || buried {
                (RailVis::Parked, vec![RailAct::Park, RailAct::Emit("native")])
            } else {
                (state, Vec::new())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromePlan {
    pub alpha: f64,
    pub ignore_cursor: bool,
    pub level: i64,
    pub steps: &'static [&'static str],
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn reveal_plan() -> ChromePlan {
    ChromePlan {
        alpha: 1.0,
        ignore_cursor: false,
        level: RAIL_LEVEL_FLOATING,
        steps: &[
            "ignore_off",
            "alpha_1",
            "level_floating",
            "place",
            "order_front",
            "activate",
            "make_key",
        ],
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn park_plan() -> ChromePlan {
    ChromePlan {
        alpha: 0.0,
        ignore_cursor: true,
        level: RAIL_LEVEL_NORMAL,
        steps: &["alpha_0", "ignore_on", "level_normal", "place"],
    }
}

pub fn plan_has_raise(acts: &[RailAct]) -> bool {
    acts.iter().any(|a| matches!(a, RailAct::Raise))
}

pub fn plan_has_park(acts: &[RailAct]) -> bool {
    acts.iter().any(|a| matches!(a, RailAct::Park))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> RailFacts {
        RailFacts {
            app_active: true,
            key: true,
            occluded: false,
            cursor_in_frame: true,
            away_ms: 0,
            occluded_ms: 0,
            button_down: false,
            auto_hide: true,
            coach_hold: false,
        }
    }

    #[test]
    fn coach_hold_refuses_park() {
        let mut f = RailFacts::inactive_clear();
        f.coach_hold = true;
        f.away_ms = 5_000;
        f.occluded = true;
        f.occluded_ms = 5_000;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(acts.is_empty());
        let (hot, hot_acts) = rail_vis_step(RailVis::Revealed, RailEvent::Hotkey, &f);
        assert_eq!(hot, RailVis::Revealed);
        assert!(hot_acts.is_empty());
        let (parked, raise) = rail_vis_step(RailVis::Parked, RailEvent::Tick, &f);
        assert_eq!(parked, RailVis::Revealed);
        assert!(plan_has_raise(&raise));
    }

    #[test]
    fn hotkey_parks_visible_active() {
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Hotkey, &live());
        assert_eq!(vis, RailVis::Parked);
        assert!(plan_has_park(&acts));
        assert!(!plan_has_raise(&acts));
    }

    #[test]
    fn hotkey_raises_buried() {
        let mut f = live();
        f.occluded = true;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Hotkey, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(plan_has_raise(&acts));
        assert!(!plan_has_park(&acts));
    }

    #[test]
    fn hotkey_raises_inactive() {
        let mut f = live();
        f.app_active = false;
        f.key = false;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Hotkey, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(plan_has_raise(&acts));
        assert!(!plan_has_park(&acts));
    }

    #[test]
    fn edge_reveal_while_revealed_reraises() {
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::EdgeReveal, &live());
        assert_eq!(vis, RailVis::Revealed);
        assert!(plan_has_raise(&acts));
        assert!(!plan_has_park(&acts));
        let (from_park, acts2) = rail_vis_step(RailVis::Parked, RailEvent::EdgeReveal, &live());
        assert_eq!(from_park, RailVis::Revealed);
        assert!(plan_has_raise(&acts2));
    }

    #[test]
    fn raised_other_parks() {
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::RaisedOther, &live());
        assert_eq!(vis, RailVis::Parked);
        assert!(plan_has_park(&acts));
    }

    #[test]
    fn tick_parks_inactive_away() {
        let mut f = RailFacts::inactive_clear();
        f.away_ms = 700;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Parked);
        assert!(plan_has_park(&acts));
        f.away_ms = 699;
        let (stay, none) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(stay, RailVis::Revealed);
        assert!(none.is_empty());
    }

    #[test]
    fn tick_parks_inactive_occluded() {
        let mut f = RailFacts::inactive_clear();
        f.cursor_in_frame = true;
        f.occluded = true;
        f.occluded_ms = 700;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Parked);
        assert!(plan_has_park(&acts));
        f.occluded_ms = 699;
        let (stay, none) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(stay, RailVis::Revealed);
        assert!(none.is_empty());
    }

    #[test]
    fn tick_never_parks_active() {
        let mut f = live();
        f.cursor_in_frame = false;
        f.away_ms = 5_000;
        f.occluded = true;
        f.occluded_ms = 5_000;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(acts.is_empty());
    }

    #[test]
    fn tick_never_parks_button_down() {
        let mut f = RailFacts::inactive_clear();
        f.button_down = true;
        f.away_ms = 5_000;
        f.occluded = true;
        f.occluded_ms = 5_000;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(acts.is_empty());
    }

    #[test]
    fn autohide_off_disables_tick_park() {
        let mut f = RailFacts::inactive_clear();
        f.auto_hide = false;
        f.away_ms = 5_000;
        f.occluded = true;
        f.occluded_ms = 5_000;
        let (vis, acts) = rail_vis_step(RailVis::Revealed, RailEvent::Tick, &f);
        assert_eq!(vis, RailVis::Revealed);
        assert!(acts.is_empty());
        let (hot, hot_acts) = rail_vis_step(RailVis::Revealed, RailEvent::Hotkey, &f);
        assert_eq!(hot, RailVis::Revealed);
        assert!(plan_has_raise(&hot_acts));
    }

    #[test]
    fn park_state_sets_alpha0_ignore_level0() {
        let p = park_plan();
        assert_eq!(p.alpha, 0.0);
        assert!(p.ignore_cursor);
        assert_eq!(p.level, RAIL_LEVEL_NORMAL);
        assert_eq!(p.steps[0], "alpha_0");
    }

    #[test]
    fn reveal_plan_sets_level_floating_alpha1_before_order_front() {
        let p = reveal_plan();
        assert_eq!(p.alpha, 1.0);
        assert!(!p.ignore_cursor);
        assert_eq!(p.level, RAIL_LEVEL_FLOATING);
        let alpha = p.steps.iter().position(|s| *s == "alpha_1").unwrap();
        let level = p.steps.iter().position(|s| *s == "level_floating").unwrap();
        let order = p.steps.iter().position(|s| *s == "order_front").unwrap();
        assert!(alpha < order, "alpha before orderFront");
        assert!(level < order, "floating level before orderFront");
    }
}
