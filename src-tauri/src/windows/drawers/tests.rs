    use super::*;
    use crate::state;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> state::CompanionBounds {
        state::CompanionBounds { x, y, w, h }
    }

    #[test]
    fn pin_keeps_bottom_center() {
        let cur = rect(100.0, 200.0, 520.0, 780.0);
        let p = pin_bottom_center(cur, 1080.0, 620.0);
        assert!((p.y + p.h - (cur.y + cur.h)).abs() < 1e-9);
        assert!((p.x + p.w / 2.0 - (cur.x + cur.w / 2.0)).abs() < 1e-9);
        assert_eq!(p.w, 1080.0);
        assert_eq!(p.h, 620.0);
    }

    #[test]
    fn clamp_keep_bottom_does_not_walk_dock() {
        let vf = VisibleFrame {
            x: 0.0,
            y: 38.0,
            w: 1512.0,
            h: 874.0,
        };
        let pinned = rect(200.0, 100.0, 1080.0, 620.0);
        let bottom = pinned.y + pinned.h;
        let c = clamp_to_frame_keep_bottom(pinned, &vf);
        assert!((c.y + c.h - bottom).abs() < 1e-9);
        assert_eq!(c.w, 1080.0);
        assert_eq!(c.h, 620.0);
    }

    #[test]
    fn beside_rail_ignores_foreign_display() {
        let primary = VisibleFrame {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        let portrait = VisibleFrame {
            x: 1920.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let frames = [primary, portrait];
        let rail = rect(8.0, 80.0, 360.0, 700.0);
        let stale = rect(2400.0, 200.0, 780.0, 640.0);
        let placed = place_beside_rail_display(&rail, (640.0, 480.0), &frames, Some(&stale))
            .expect("place");
        assert!(
            rect_mostly_on(&placed, &primary),
            "placed on the rail display"
        );
        assert!(
            !rect_mostly_on(&placed, &portrait),
            "stale portrait display did not win"
        );
        let gap = placed.x - (rail.x + rail.w);
        assert!(gap <= 48.0 && gap >= -placed.w, "adjacent, gap {gap}");

        let rail_p = rect(1928.0, 40.0, 280.0, 700.0);
        let stale_primary = rect(400.0, 120.0, 780.0, 640.0);
        let on_portrait =
            place_beside_rail_display(&rail_p, (500.0, 400.0), &frames, Some(&stale_primary))
                .expect("portrait");
        assert!(rect_mostly_on(&on_portrait, &portrait));
        assert!(!rect_mostly_on(&on_portrait, &primary));
        let gap_p = on_portrait.x - (rail_p.x + rail_p.w);
        assert!(gap_p <= 48.0 && gap_p >= -on_portrait.w, "portrait gap {gap_p}");

        let tight = VisibleFrame {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let rail_left = rect(8.0, 40.0, 360.0, 700.0);
        let crowded = place_beside_rail_display(&rail_left, (780.0, 640.0), &[tight], Some(&stale))
            .expect("crowded");
        assert!(rect_mostly_on(&crowded, &tight));
        assert!(crowded.x + crowded.w <= tight.x + tight.w + 0.5);
    }

    #[test]
    fn stage_parked_strip_spans_edge() {
        assert_eq!(stage_parked_size("left", 1440.0, 875.0), (360.0, 779.0));
        assert_eq!(stage_parked_size("right", 1440.0, 875.0), (360.0, 779.0));
        assert_eq!(stage_parked_size("top", 1440.0, 875.0), (1344.0, 420.0));
        assert_eq!(stage_parked_size("bottom", 1440.0, 875.0), (1344.0, 420.0));
        assert_eq!(stage_parked_size("left", 200.0, 100.0), (360.0, 420.0));
        assert_eq!(stage_parked_size("top", 100.0, 200.0), (360.0, 420.0));
        assert_eq!(stage_parked_size("nope", 1440.0, 875.0), (360.0, 779.0));
    }
