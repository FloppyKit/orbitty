    use super::*;

    fn screen() -> ScreenRect {
        ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        }
    }

    fn arm(s: &mut EdgeWatch, scr: ScreenRect, edge: &str) {
        let outside = match edge {
            "right" => (scr.x + scr.w - 80.0, scr.y + scr.h / 2.0),
            "top" => (scr.x + scr.w / 2.0, scr.y + 80.0),
            "bottom" => (scr.x + scr.w / 2.0, scr.y + scr.h - 80.0),
            _ => (scr.x + 80.0, scr.y + scr.h / 2.0),
        };
        assert!(!edge_watch_step(s, outside, scr, edge, false, 0));
        assert!(s.armed());
    }

    fn band_point(scr: ScreenRect, edge: &str) -> (f64, f64) {
        let mid_x = scr.x + scr.w / 2.0;
        let mid_y = scr.y + scr.h / 2.0;
        match edge {
            "right" => (scr.x + scr.w - 3.0, mid_y),
            "top" => (mid_x, scr.y + 3.0),
            "bottom" => (mid_x, scr.y + scr.h - 3.0),
            _ => (scr.x + 3.0, mid_y),
        }
    }

    #[test]
    fn edge_band_dwell_reveals() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (3.0, 800.0);
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 1_000));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 1_119));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 1_120));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 2_000));
    }

    #[test]
    fn edge_corners_excluded() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        for y in [scr.y + 10.0, scr.y + 47.0, scr.y + scr.h - 47.0, scr.y + scr.h - 1.0] {
            let mut w = s;
            for t in 0..20 {
                assert!(
                    !edge_watch_step(&mut w, (2.0, y), scr, "left", false, 1_000 + t * 50),
                    "y {y}"
                );
            }
        }
    }

    #[test]
    fn edge_button_down_blocks() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (2.0, 800.0);
        for t in 0..40 {
            assert!(!edge_watch_step(&mut s, p, scr, "left", true, t * 50));
        }
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 3_000));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 3_120));
    }

    #[test]
    fn edge_rearm_hysteresis() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (4.0, 800.0);
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 0));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 120));
        let mut t = 200u64;
        let end = t + 2_000;
        let mut near = true;
        while t < end {
            let x = if near { 3.0 } else { 7.0 };
            near = !near;
            assert!(!edge_watch_step(&mut s, (x, 800.0), scr, "left", false, t));
            t += 33;
        }
        // Stay inside the 6px band longer than a dwell. Rearm must hold.
        assert!(!edge_watch_step(&mut s, (3.0, 800.0), scr, "left", false, t));
        assert!(!edge_watch_step(&mut s, (4.0, 800.0), scr, "left", false, t + 200));
        t += 200;
        assert!(!edge_watch_step(&mut s, (80.0, 800.0), scr, "left", false, t));
        t += 30;
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, t));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, t + 120));
    }

    #[test]
    fn edge_after_park_needs_leave() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        edge_watch_parked(&mut s);
        let p = (2.0, 800.0);
        for t in 0..30 {
            assert!(!edge_watch_step(&mut s, p, scr, "left", false, t * 40));
        }
        assert!(!edge_watch_step(&mut s, (30.0, 800.0), scr, "left", false, 2_000));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 2_100));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 2_220));
    }

    #[test]
    fn edge_all_four_edges() {
        let scr = ScreenRect {
            x: 2820.0,
            y: 40.0,
            w: 1920.0,
            h: 1080.0,
        };
        for edge in ["left", "right", "top", "bottom"] {
            let mut s = EdgeWatch::new();
            arm(&mut s, scr, edge);
            let p = band_point(scr, edge);
            assert!(
                !edge_watch_step(&mut s, p, scr, edge, false, 10),
                "{edge} enter"
            );
            assert!(
                edge_watch_step(&mut s, p, scr, edge, false, 130),
                "{edge} dwell"
            );
        }
    }

    #[test]
    fn edge_other_display_ignored() {
        let rail = ScreenRect {
            x: 2820.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let mut s = EdgeWatch::new();
        arm(&mut s, rail, "left");
        let other_left = (3.0, 800.0);
        for t in 0..40 {
            assert!(!edge_watch_step(
                &mut s,
                other_left,
                rail,
                "left",
                false,
                t * 50
            ));
        }
    }

    #[test]
    fn edge_flap_sim() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let mut reveals = 0;
        let mut t = 100u64;
        for _ in 0..20 {
            if edge_watch_step(&mut s, (2.0, 800.0), scr, "left", false, t) {
                reveals += 1;
            }
            t += 50;
            if edge_watch_step(&mut s, (40.0, 800.0), scr, "left", false, t) {
                reveals += 1;
            }
            t += 50;
        }
        assert_eq!(reveals, 0);
    }

    #[test]
    fn appkit_y_flips_to_top_left() {
        assert_eq!(flip_appkit_y(1600.0, 1600.0), 0.0);
        assert_eq!(flip_appkit_y(0.0, 1600.0), 1600.0);
        let r = flip_appkit_rect(0.0, 79.0, 900.0, 1521.0, 1600.0);
        assert_eq!(r.y, 1600.0 - (79.0 + 1521.0));
        assert_eq!(r.x, 0.0);
        assert_eq!(r.w, 900.0);
        assert_eq!(r.h, 1521.0);
    }

    #[test]
    fn next_poll_far_is_slow() {
        assert_eq!(next_poll_ms(Some(900.0), false, false), 250);
        assert_eq!(next_poll_ms(None, false, false), 250);
        assert_eq!(next_poll_ms(Some(200.0), false, false), 120);
    }

    #[test]
    fn next_poll_band_is_fast() {
        assert_eq!(next_poll_ms(Some(2.0), true, false), 33);
        assert_eq!(next_poll_ms(Some(40.0), false, true), 33);
        assert_eq!(next_poll_ms(Some(80.0), false, false), 50);
    }

    #[test]
    fn fast_flick_latency_bounded() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        assert!(!edge_watch_step(&mut s, (400.0, 800.0), scr, "left", false, 0));
        assert!(s.armed());
        let mut x = 450.0_f64;
        let mut t = 0u64;
        let mut arrived: Option<u64> = None;
        let mut revealed: Option<u64> = None;
        while t < 8_000 && revealed.is_none() {
            let sample = if x <= EDGE_REVEAL_PX { 3.0 } else { x };
            if sample <= EDGE_REVEAL_PX && arrived.is_none() {
                arrived = Some(t);
            }
            let dist = edge_distance((sample, 800.0), scr, "left");
            let band = in_band((sample, 800.0), scr, "left");
            if edge_watch_step(&mut s, (sample, 800.0), scr, "left", false, t) {
                revealed = Some(t);
                break;
            }
            let ms = next_poll_ms(dist, band, s.dwelling());
            if x > EDGE_REVEAL_PX {
                let next = x - 3000.0 * (ms as f64) / 1000.0;
                if x > EDGE_REVEAL_PX && next <= EDGE_REVEAL_PX {
                    let frac = (x - EDGE_REVEAL_PX) / (x - next);
                    arrived = Some(t + ((ms as f64) * frac) as u64);
                }
                x = next;
            }
            t += ms;
        }
        let arrived = arrived.expect("cursor reached the band");
        let revealed = revealed.expect("dwell revealed");
        assert!(
            revealed.saturating_sub(arrived) <= EDGE_DWELL_MS + 150,
            "latency {} arrived {arrived} revealed {revealed}",
            revealed.saturating_sub(arrived)
        );
    }

    #[test]
    fn parked_offscreen_frame_outer_edges() {
        let primary = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let alone = [primary];
        for edge in ["left", "right", "top", "bottom"] {
            let p = parked_place(edge, primary, &alone, 360.0, 420.0);
            assert_eq!(p.alpha, 0.0, "{edge}");
            assert!(p.offscreen, "{edge}");
            let frame = ScreenRect {
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
            };
            assert_eq!(on_screen_px(frame, &alone), 0.0, "{edge}");
        }
        let neighbor = ScreenRect {
            x: 900.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        let both = [primary, neighbor];
        let right = parked_place("right", primary, &both, 360.0, 420.0);
        assert_eq!(right.alpha, 0.0);
        assert!(!right.offscreen);
        let (sx, sy, sw, sh) =
            crate::state::rail_stage_frame("right", false, 0.0, 0.0, 900.0, 1600.0, 360.0, 420.0);
        assert_eq!(right.x, sx);
        assert_eq!(right.y, sy);
        assert_eq!(right.w, sw);
        assert_eq!(right.h, sh);
        let left = parked_place("left", primary, &both, 360.0, 420.0);
        assert!(left.offscreen);
        assert_eq!(
            on_screen_px(
                ScreenRect {
                    x: left.x,
                    y: left.y,
                    w: left.w,
                    h: left.h,
                },
                &both
            ),
            0.0
        );
    }
