//! Unit tests: projection reference values, zone classification, serve
//! injection, the scoring economy and drain cadence (incl. quirk Q11), wall
//! behavior, clamps, enemy easing, the serve-during-pop quirk (Q2), level
//! table clamping (D1), the phase machine's frame-accurate timings and
//! end-of-rally routing, name entry, and the local high-score table.

#![expect(
    clippy::expect_used,
    reason = "test assertions may unwrap scenario invariants"
)]
#![expect(clippy::float_cmp, reason = "the sim specifies exact IEEE comparisons")]

use curveball::app::{
    App, GameMode, PIP_FLASH_TICKS, Phase, PipFlash, SoundId, TickInput, VisualMode,
};
use curveball::consts::{
    BONUS_COUNTER_INIT, BTN_END_MENU, BTN_HS_MENU, BTN_TITLE_SCORES, BTN_TITLE_START,
    BTN_TITLE_VISUAL, BTN_TITLE_ZEN, FRAME_PLAY_HOLD, GAME_OVER_TICKS, MISS_TICKS, SILKY_DT_SCALE,
    SILKY_PHYSICS_HZ, SPLASH_TICKS, START_GAME_TICKS, TICK_HZ, WALL_CURVE_DAMP, WORLD_CX, WORLD_CY,
};
use curveball::highscores::ScoreTable;
use curveball::sim::{
    Ball, CurveClass, Economy, Enemy, PaddleSnapshot, Published, Rect, SimEvent, SimInput, Vec3,
    World, Zone, classify, level_params, scale, vis,
};

fn unique_temp_path(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn rect_center(rect: (f64, f64, f64, f64)) -> (f64, f64) {
    (f64::midpoint(rect.0, rect.2), f64::midpoint(rect.1, rect.3))
}

fn assert_rel(actual: f64, expected: f64, what: &str) {
    let rel = ((actual - expected) / expected).abs();
    assert!(rel < 1e-9, "{what}: {actual} vs {expected} (rel {rel:e})");
}

// ---------------------------------------------------------------------------
// Projection (§3.3)
// ---------------------------------------------------------------------------

#[test]
#[expect(
    clippy::suboptimal_flops,
    reason = "the expectation must use the AS1 operation order, not mul_add"
)]
fn projection_reference_values() {
    assert_eq!(scale(0.0), 1.0);
    assert_rel(scale(75.0), 0.249_999_998_710, "s(75)");
    assert_rel(scale(76.0), 0.247_032_735_634, "s(76)");
    assert_rel(scale(-2.0), 1.040_928_480_087, "s(-2)");
    // The wall damp must be the exact runtime arithmetic, not literal 1.2.
    assert_eq!(
        WALL_CURVE_DAMP.to_bits(),
        ((1.004_f64 - 1.0) * 50.0 + 1.0).to_bits()
    );
    assert_ne!(WALL_CURVE_DAMP.to_bits(), 1.2_f64.to_bits());
}

#[test]
#[expect(
    clippy::suboptimal_flops,
    reason = "the expectation must use the AS1 operation order, not mul_add"
)]
fn projection_center_fixed_point() {
    for z in [-2.0, 0.0, 37.5, 75.0, 76.0] {
        assert_eq!(vis(WORLD_CX, WORLD_CY, z), (WORLD_CX, WORLD_CY));
    }
    // vis pulls off-center points toward the center as z grows.
    let (x, _) = vis(25.0, 25.0, 75.0);
    assert_rel(
        x,
        WORLD_CX - (WORLD_CX - 25.0) * scale(75.0),
        "vis x at depth",
    );
}

// ---------------------------------------------------------------------------
// Zone classification (§4.7)
// ---------------------------------------------------------------------------

#[test]
fn zone_classifier_cascade() {
    let (px, py) = (175.5, 125.5);
    let case = |bx: f64, by: f64| classify(bx, by, px, py);
    // Outer columns: strictly beyond ±7.
    assert_eq!(case(px + 7.1, py - 1.0), Zone::UR);
    assert_eq!(case(px + 7.1, py), Zone::BR); // by == py lands in the >= branch
    assert_eq!(case(px - 7.1, py - 1.0), Zone::UL);
    assert_eq!(case(px - 7.1, py + 1.0), Zone::BL);
    // Exactly ±7 is not "beyond": falls through to the center column.
    assert_eq!(case(px + 7.0, py), Zone::C);
    assert_eq!(case(px - 7.0, py), Zone::C);
    // Center column, vertical bands.
    assert_eq!(case(px, py - 5.0), Zone::C);
    assert_eq!(case(px, py + 5.0), Zone::C);
    assert_eq!(case(px + 1.0, py - 5.1), Zone::UR);
    assert_eq!(case(px - 1.0, py - 5.1), Zone::UL);
    assert_eq!(case(px, py - 5.1), Zone::UR); // bx == px ties toward the right
    assert_eq!(case(px + 1.0, py + 5.1), Zone::BR);
    assert_eq!(case(px - 1.0, py + 5.1), Zone::BL);
    assert_eq!(case(px, py + 5.1), Zone::BR);
}

// ---------------------------------------------------------------------------
// Serve (§4.5)
// ---------------------------------------------------------------------------

/// A world in the Serve hold with the paddle settled at `mouse` and the ball
/// fresh at the center. Returns it after enough ticks for the paddle to
/// converge (speed below the injection threshold).
fn serve_world(mouse: (f64, f64)) -> World {
    let mut world = World::new(Published::default());
    world.level_setup();
    world.spawn_enemy();
    world.spawn_ball();
    for _ in 0..60 {
        world.tick(&SimInput {
            mouse,
            serve_clicks: 0,
        });
    }
    world
}

#[test]
fn serve_minimum_curve_injection_signs() {
    // (paddle position, expected injected (cx, cy) signs).
    // cx: +0.01 when paddle.x < wx, else −0.01; cy: +0.01 when paddle.y > wy.
    let cases: [((f64, f64), (f64, f64)); 3] = [
        ((140.0, 100.0), (0.01, -0.01)),
        ((200.0, 150.0), (-0.01, 0.01)),
        ((WORLD_CX, WORLD_CY), (-0.01, -0.01)), // exact center ties to −0.01
    ];
    for ((mx, my), (ecx, ecy)) in cases {
        let mut world = serve_world((mx, my));
        let events = world.tick(&SimInput {
            mouse: (mx, my),
            serve_clicks: 1,
        });
        assert!(
            matches!(events.as_slice(), [SimEvent::Serve { .. }]),
            "serve fires at paddle {mx},{my}"
        );
        let ball = world.ball.expect("ball");
        // Observed after this tick's single decay step.
        assert_eq!(
            ball.curve.0.to_bits(),
            (ecx / 1.004).to_bits(),
            "cx at {mx},{my}"
        );
        assert_eq!(
            ball.curve.1.to_bits(),
            (ecy / 1.004).to_bits(),
            "cy at {mx},{my}"
        );
        assert_eq!(ball.vel.z, world.params.speed);
    }
}

#[test]
fn serve_requires_paddle_overlap() {
    // Paddle parked far from the centered ball: clicks never serve.
    let mut world = serve_world((60.0, 60.0));
    let events = world.tick(&SimInput {
        mouse: (60.0, 60.0),
        serve_clicks: 3,
    });
    assert!(events.is_empty());
    assert_eq!(world.ball.expect("ball").vel.z, 0.0);
}

#[test]
fn serve_awards_no_hit_score() {
    // A centered serve awards accuracy only — hitScore is untouched (§4.5).
    let mut world = serve_world((WORLD_CX, WORLD_CY));
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 1,
    });
    assert_eq!(world.economy.score, 100);
    assert_eq!(world.economy.hit_score, 100);
    assert_eq!(world.economy.accuracy_bonus, 90);
}

#[test]
fn serve_curve_uses_cached_paddle_speed_and_level_curve() {
    let mut world = serve_world((WORLD_CX, WORLD_CY));
    world.ball.as_mut().expect("ball").snapshot = PaddleSnapshot {
        pos: (WORLD_CX, WORLD_CY),
        speed: (3.0, -3.0),
    };

    let events = world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 1,
    });

    assert!(
        matches!(
            events.as_slice(),
            [SimEvent::Serve {
                zone: Zone::C,
                accuracy: true,
                curve: CurveClass::SuperCurve
            }]
        ),
        "events: {events:?}"
    );
    let ball = world.ball.expect("ball");
    let assigned = -3.0 / world.params.curve_amount;
    assert_eq!(assigned, -0.12);
    // A serve runs before the frame, so the first ball enterFrame has already
    // integrated and decayed the just-assigned curve once by the time we read it.
    assert_eq!(ball.curve.0.to_bits(), (assigned / 1.004).to_bits());
    assert_eq!(ball.curve.1.to_bits(), (assigned / 1.004).to_bits());
    assert_eq!(ball.vel.x.to_bits(), assigned.to_bits());
    assert_eq!(ball.vel.y.to_bits(), assigned.to_bits());
    assert_eq!(ball.vel.z.to_bits(), world.params.speed.to_bits());
    assert_eq!(world.economy.score, 250, "accuracy + super curve");
    assert_eq!(world.economy.hit_score, 100, "serves do not award hitScore");
}

// ---------------------------------------------------------------------------
// Economy (§3.7)
// ---------------------------------------------------------------------------

#[test]
fn economy_degrade_floors() {
    let mut eco = Economy::new();
    let mut expected_score = 0_i64;
    for award in [100, 90, 80, 70, 60, 50, 40, 30, 20, 10, 0, 0] {
        expected_score += award;
        eco.award_hit();
        assert_eq!(eco.score, expected_score);
    }
    assert_eq!(eco.hit_score, 0);
}

#[test]
fn curve_classification_branch_order() {
    let class = |cx: f64, cy: f64| Economy::new().award_curve(cx, cy);
    assert_eq!(class(0.2, 0.2), CurveClass::SuperCurve);
    assert_eq!(class(-0.2, -0.2), CurveClass::SuperCurve);
    assert_eq!(class(0.2, 0.05), CurveClass::Curve); // one axis only
    assert_eq!(class(0.1, 0.2), CurveClass::Curve); // |cx| not strictly > 0.1
    assert_eq!(class(0.0, 0.06), CurveClass::Curve);
    assert_eq!(class(0.06, 0.0), CurveClass::Curve);
    assert_eq!(class(0.05, 0.05), CurveClass::None);
    assert_eq!(class(0.1, 0.05), CurveClass::Curve); // 0.1 fails >0.1 but passes the >0.05 fallback
}

#[test]
fn bonus_drain_cadence_is_11_flight_ticks() {
    let mut eco = Economy::new();
    for tick in 1..=33 {
        eco.drain_tick(true);
        let expected = match tick {
            t if t < 11 => 3000,
            t if t < 22 => 2975,
            t if t < 33 => 2950,
            _ => 2925,
        };
        assert_eq!(eco.bonus_display, expected, "tick {tick}");
    }
    // Grounded ticks never decrement, but the reset check still runs.
    let mut idle = Economy::new();
    for _ in 0..100 {
        idle.drain_tick(false);
    }
    assert_eq!(idle.bonus_display, 3000);
    assert_eq!(idle.bonus_counter, BONUS_COUNTER_INIT);
}

#[test]
fn drain_counter_persists_across_rallies_q11() {
    let mut world = World::new(Published::default());
    world.level_setup();
    world.spawn_enemy();
    world.spawn_ball();
    world.economy.bonus_counter = 3; // mid-cycle when the rally ends
    world.ball = None; // re-serve routing replaces only ball + ring
    world.spawn_ball();
    assert_eq!(world.economy.bonus_counter, 3, "carries across rallies");
    world.level_setup();
    assert_eq!(
        world.economy.bonus_counter, BONUS_COUNTER_INIT,
        "resets at level setup only"
    );
}

// ---------------------------------------------------------------------------
// Paddles (§4.2, §4.3)
// ---------------------------------------------------------------------------

#[test]
fn paddle_easing_and_clamps() {
    let mut world = World::new(Published::default());
    // Consume the placement tick, then ease 1.5× toward the mouse.
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 0,
    });
    world.tick(&SimInput {
        mouse: (WORLD_CX + 15.0, WORLD_CY),
        serve_clicks: 0,
    });
    assert_eq!(world.paddle.pos.0, WORLD_CX + 10.0);
    assert_eq!(world.paddle.speed.0, 10.0);
    // Clamps: x ∈ [55, 296], y ∈ [45, 206].
    for _ in 0..60 {
        world.tick(&SimInput {
            mouse: (-100.0, -100.0),
            serve_clicks: 0,
        });
    }
    assert_eq!(world.paddle.pos, (55.0, 45.0));
    for _ in 0..60 {
        world.tick(&SimInput {
            mouse: (1000.0, 1000.0),
            serve_clicks: 0,
        });
    }
    assert_eq!(world.paddle.pos, (296.0, 206.0));
}

#[test]
fn paddle_predicted_pos_matches_next_step_without_mutating() {
    let mut world = World::new(Published::default());
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 0,
    });
    let target = (WORLD_CX + 15.0, WORLD_CY + 9.0);
    let before = world.paddle.pos;
    let predicted = world.paddle.predicted_pos(target);

    assert_eq!(world.paddle.pos, before);
    world.tick(&SimInput {
        mouse: target,
        serve_clicks: 0,
    });
    assert_eq!(world.paddle.pos, predicted);
}

#[test]
fn silky_400hz_ball_motion_preserves_wall_clock_speed() {
    let mut world = World::new(Published::default());
    world.spawn_ball();
    let ball = world.ball.as_mut().expect("ball");
    ball.just_spawned = false;
    ball.vel.z = 2.0;

    for _ in 0..SILKY_PHYSICS_HZ {
        world.tick_silky_slice(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
    }

    let ball = world.ball.expect("ball");
    assert!((ball.pos.z - 60.0).abs() < 1e-9, "z={}", ball.pos.z);
}

#[test]
fn enemy_easing_divisors() {
    // dir.z ≤ 0 → ease home with divisor 15.
    let mut enemy = Enemy::new(17.0);
    enemy.step(&Published::default()); // placement tick
    enemy.pos = (100.0, 125.5);
    enemy.step(&Published::default());
    assert_eq!(enemy.pos.0, 100.0 - (100.0 - WORLD_CX) / 15.0);
    // dir.z > 0 → chase the published position with the skill divisor.
    let mut published = Published::default();
    published.dir.z = 2.0;
    published.pos = Vec3 {
        x: 100.0,
        y: 125.5,
        z: 10.0,
    };
    let mut chaser = Enemy::new(17.0);
    chaser.step(&Published::default());
    chaser.step(&published);
    assert_eq!(chaser.pos.0, WORLD_CX - (WORLD_CX - 100.0) / 17.0);
    assert_eq!(chaser.speed.0, chaser.pos.0 - WORLD_CX);
}

// ---------------------------------------------------------------------------
// Walls (§4.4)
// ---------------------------------------------------------------------------

/// A rally-ready world whose ball fields are then hand-set per case.
fn flying_ball_world() -> World {
    let mut world = World::new(Published::default());
    world.level_setup();
    world.spawn_enemy();
    world.spawn_ball();
    for _ in 0..2 {
        world.tick(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
    }
    world
}

#[test]
fn wall_clamps_reflect_and_damp() {
    // (start pos, vel, curve) → (clamped axis value, wall event horizontal?).
    struct Case {
        pos: Vec3,
        vel: Vec3,
        curve: (f64, f64),
        clamped: (Option<f64>, Option<f64>),
        horizontal: bool,
    }
    let cases = [
        Case {
            // Top: y − 15 < 25 → y = 40, vy reflects, cy damped.
            pos: Vec3 {
                x: 175.5,
                y: 42.0,
                z: 30.0,
            },
            vel: Vec3 {
                x: 0.0,
                y: 3.0,
                z: 2.0,
            }, // y -= vy → 39
            curve: (0.0, 0.12),
            clamped: (None, Some(40.0)),
            horizontal: false,
        },
        Case {
            // Bottom: 226 < y + 15 → y = 211.
            pos: Vec3 {
                x: 175.5,
                y: 209.0,
                z: 30.0,
            },
            vel: Vec3 {
                x: 0.0,
                y: -3.0,
                z: 2.0,
            },
            curve: (0.0, -0.12),
            clamped: (None, Some(211.0)),
            horizontal: false,
        },
        Case {
            // Left: x − 15 < 25 → x = 40.
            pos: Vec3 {
                x: 42.0,
                y: 125.5,
                z: 30.0,
            },
            vel: Vec3 {
                x: -3.0,
                y: 0.0,
                z: 2.0,
            },
            curve: (-0.12, 0.0),
            clamped: (Some(40.0), None),
            horizontal: true,
        },
        Case {
            // Right: 326 < x + 15 → x = 311.
            pos: Vec3 {
                x: 309.0,
                y: 125.5,
                z: 30.0,
            },
            vel: Vec3 {
                x: 3.0,
                y: 0.0,
                z: 2.0,
            },
            curve: (0.12, 0.0),
            clamped: (Some(311.0), None),
            horizontal: true,
        },
    ];
    for case in cases {
        let mut world = flying_ball_world();
        {
            let ball = world.ball.as_mut().expect("ball");
            ball.pos = case.pos;
            ball.vel = case.vel;
            ball.curve = case.curve;
        }
        let events = world.tick(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
        assert_eq!(
            events,
            vec![SimEvent::WallBounce {
                horizontal: case.horizontal
            }],
            "wall event"
        );
        let ball = world.ball.expect("ball");
        if let Some(x) = case.clamped.0 {
            assert_eq!(ball.pos.x, x);
            assert_eq!(ball.vel.x, -(case.vel.x + case.curve.0));
            // Decay applies before the wall damp, in source order.
            assert_eq!(
                ball.curve.0.to_bits(),
                (case.curve.0 / 1.004 / WALL_CURVE_DAMP).to_bits()
            );
        }
        if let Some(y) = case.clamped.1 {
            assert_eq!(ball.pos.y, y);
            assert_eq!(ball.vel.y, -(case.vel.y + case.curve.1));
            assert_eq!(
                ball.curve.1.to_bits(),
                (case.curve.1 / 1.004 / WALL_CURVE_DAMP).to_bits()
            );
        }
    }
}

#[test]
fn negative_zero_curve_skips_decay_q10() {
    let mut world = flying_ball_world();
    {
        let ball = world.ball.as_mut().expect("ball");
        ball.vel = Vec3 {
            x: 0.5,
            y: 0.5,
            z: 2.0,
        };
        ball.curve = (-0.0, 0.0);
        ball.pos = Vec3 {
            x: 175.5,
            y: 125.5,
            z: 10.0,
        };
    }
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 0,
    });
    let ball = world.ball.expect("ball");
    assert_eq!(
        ball.curve.0.to_bits(),
        (-0.0_f64).to_bits(),
        "-0.0 preserved un-decayed"
    );
    assert_eq!(ball.curve.1.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn player_return_curve_uses_current_paddle_speed_without_minimum_injection() {
    let mut world = flying_ball_world();
    {
        let ball = world.ball.as_mut().expect("ball");
        ball.pos = Vec3 {
            x: WORLD_CX,
            y: WORLD_CY,
            z: 1.0,
        };
        ball.vel = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -2.0,
        };
        ball.curve = (0.0, 0.0);
        ball.prev_rect = Rect::centered((WORLD_CX, WORLD_CY), 30.0, 30.0);
    }

    let events = world.tick(&SimInput {
        mouse: (WORLD_CX + 15.0, WORLD_CY - 9.0),
        serve_clicks: 0,
    });

    assert!(
        matches!(
            events.as_slice(),
            [SimEvent::PlayerHit {
                zone: Zone::BL,
                accuracy: false,
                curve: CurveClass::SuperCurve
            }]
        ),
        "events: {events:?}"
    );
    assert_eq!(world.paddle.speed, (10.0, -6.0));
    let ball = world.ball.expect("ball");
    assert_eq!(ball.pos.z, 0.0);
    assert_eq!(ball.vel.z, 2.0);
    assert_eq!(
        ball.curve.0.to_bits(),
        (-world.paddle.speed.0 / world.params.curve_amount).to_bits()
    );
    assert_eq!(
        ball.curve.1.to_bits(),
        (world.paddle.speed.1 / world.params.curve_amount).to_bits()
    );
    assert_eq!(ball.curve.0, -0.4);
    assert_eq!(ball.curve.1, -0.24);
    assert_eq!(world.economy.score, 250, "hitScore + super curve");
}

#[test]
fn silky_player_contact_sweeps_between_previous_rect_and_player_plane() {
    let mut world = flying_ball_world();
    {
        let ball = world.ball.as_mut().expect("ball");
        ball.pos = Vec3 {
            x: WORLD_CX + 50.0,
            y: WORLD_CY,
            z: SILKY_DT_SCALE,
        };
        ball.vel = Vec3 {
            x: -200.0,
            y: 0.0,
            z: -2.0,
        };
        ball.curve = (0.0, 0.0);
        // The original previous-rect test alone would miss at this offset.
        ball.prev_rect = Rect::centered((WORLD_CX + 50.0, WORLD_CY), 30.0, 30.0);
    }

    let events = world.tick_silky_slice(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 0,
    });

    assert!(
        matches!(events.as_slice(), [SimEvent::PlayerHit { .. }]),
        "events: {events:?}"
    );
    let ball = world.ball.expect("ball");
    assert_eq!(ball.pos.z, 0.0);
    assert_eq!(ball.vel.z, 2.0);
}

#[test]
fn player_hit_sound_and_flash_start_on_same_app_tick() {
    let mut world = flying_ball_world();
    {
        let ball = world.ball.as_mut().expect("ball");
        ball.pos = Vec3 {
            x: WORLD_CX,
            y: WORLD_CY,
            z: 1.0,
        };
        ball.vel = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -2.0,
        };
        ball.curve = (0.0, 0.0);
        ball.prev_rect = Rect::centered((WORLD_CX, WORLD_CY), 30.0, 30.0);
    }
    let mut app = App::new();
    app.world = Some(world);
    app.phase = Phase::Playing {
        frame: FRAME_PLAY_HOLD,
    };

    let sounds = app.tick(&TickInput {
        mouse: (WORLD_CX + 15.0, WORLD_CY - 9.0),
        ..TickInput::default()
    });

    assert_eq!(sounds, [SoundId::PPaddleBounce]);
    let flash = app.player_flash.expect("player flash");
    assert_eq!(flash.zone, Zone::BL);
    assert_eq!(flash.tick, 0);
}

// ---------------------------------------------------------------------------
// Serve-during-pop (quirk Q2)
// ---------------------------------------------------------------------------

#[test]
fn pop_serve_scores_once_and_stays_frozen() {
    // Replay GOLD-1 to the player miss at f152.
    let mut world = serve_world((WORLD_CX, WORLD_CY));
    let mut missed_at = None;
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 1,
    });
    for tick in 2..=200 {
        let events = world.tick(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
        if events.contains(&SimEvent::PlayerMiss) {
            missed_at = Some(tick);
            break;
        }
    }
    assert_eq!(missed_at, Some(152), "GOLD-1 miss tick");
    let frozen_pos = world.ball.expect("ball").pos;
    let score_before = world.economy.score;

    // Chase the pop with the paddle (it keeps tracking during the pop), then
    // click: the serve fires once against the frozen rect.
    let pop = world.ball.expect("ball").prev_rect;
    let target = (pop.cx, pop.cy);
    for _ in 0..10 {
        world.tick(&SimInput {
            mouse: target,
            serve_clicks: 0,
        });
    }
    let events = world.tick(&SimInput {
        mouse: target,
        serve_clicks: 1,
    });
    assert!(
        matches!(events.as_slice(), [SimEvent::Serve { .. }]),
        "pop serve fires: {events:?}"
    );
    let ball = world.ball.expect("ball");
    assert!(ball.stopped, "ball stays frozen");
    assert_eq!(ball.vel.z, world.params.speed, "vel.z set by the pop serve");
    assert_eq!(ball.pos, frozen_pos, "position frozen");
    // The frozen-center snapshot classifies BL with no curve bonus, so the
    // score is untouched — the quirk's value is the event itself here.
    assert_eq!(world.economy.score, score_before);

    // Further clicks are blocked by the vel.z == 0 gate.
    let events = world.tick(&SimInput {
        mouse: target,
        serve_clicks: 2,
    });
    assert!(events.is_empty(), "second pop serve blocked");
    // And the frozen ball never moves on subsequent ticks.
    for _ in 0..5 {
        world.tick(&SimInput {
            mouse: target,
            serve_clicks: 0,
        });
    }
    assert_eq!(world.ball.expect("ball").pos, frozen_pos);
}

#[test]
fn zen_world_keeps_player_lives_on_miss() {
    let mut world = serve_world((WORLD_CX, WORLD_CY));
    world.unlimited_player_lives = true;
    world.tick(&SimInput {
        mouse: (WORLD_CX, WORLD_CY),
        serve_clicks: 1,
    });
    let mut missed_at = None;
    for tick in 2..=200 {
        let events = world.tick(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
        if events.contains(&SimEvent::PlayerMiss) {
            missed_at = Some(tick);
            break;
        }
    }
    assert_eq!(missed_at, Some(152), "GOLD-1 miss tick");
    assert_eq!(world.player_lives, 5);
    assert_eq!(world.economy.hit_score, 100, "rally bonuses still reset");
}

// ---------------------------------------------------------------------------
// Level table (deviation D1)
// ---------------------------------------------------------------------------

#[test]
fn level_table_clamps_past_ten() {
    let ten = level_params(10);
    let eleven = level_params(11);
    assert_eq!(eleven.speed, ten.speed);
    assert_eq!(eleven.skill, ten.skill);
    assert_eq!(eleven.curve_amount, ten.curve_amount);
    assert_eq!(level_params(1).speed, 2.0);
    assert_eq!(level_params(10).speed, 6.0);
}

#[test]
fn ball_caches_level_params_at_spawn() {
    let mut world = World::new(Published::default());
    world.level = 3;
    world.level_setup();
    world.spawn_ball();
    let ball: &Ball = world.ball.as_ref().expect("ball");
    assert_eq!(ball.speed, 2.66);
    assert_eq!(ball.curve_amount, 20.0);
}

// ---------------------------------------------------------------------------
// Phase machine timings (§9)
// ---------------------------------------------------------------------------

fn pinned_input(clicks: Vec<(f64, f64)>) -> TickInput {
    TickInput {
        mouse: (WORLD_CX, WORLD_CY),
        clicks,
        ..TickInput::default()
    }
}

const fn silky_ticks_for_flash_frames(frames: u32) -> u32 {
    (frames * SILKY_PHYSICS_HZ).div_ceil(TICK_HZ)
}

#[test]
fn full_flow_frame_accurate_timings() {
    let mut app = App::new();
    assert_eq!(app.phase, Phase::Title);

    // Click "start game" (hit rect from the tag stream).
    app.tick(&pinned_input(vec![(175.0, 116.0)]));
    assert_eq!(app.phase, Phase::StartGameInit { tick: 1 });

    // 8 more ticks of transition; the 9th runs the frame-44 init.
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    assert_eq!(app.phase, Phase::LevelSplash { tick: 0 });
    assert!(app.world.is_some(), "world created at init");

    // 46 splash ticks (frames 45–90), then Playing at frame 90.
    for tick in 1..=SPLASH_TICKS {
        app.tick(&pinned_input(vec![]));
        if tick < SPLASH_TICKS {
            assert_eq!(app.phase, Phase::LevelSplash { tick });
        }
    }
    assert_eq!(app.phase, Phase::Playing { frame: 90 });

    // Frame 91 places the enemy, 92 the ball + ring.
    app.tick(&pinned_input(vec![]));
    assert_eq!(app.phase, Phase::Playing { frame: 91 });
    assert!(
        app.world
            .as_ref()
            .is_some_and(|w| w.enemy.is_some() && w.ball.is_none())
    );
    app.tick(&pinned_input(vec![]));
    assert_eq!(app.phase, Phase::Playing { frame: 92 });
    assert!(app.world.as_ref().is_some_and(|w| w.ball.is_some()));

    // Let the ball run its first enterFrame so the serve snapshot is cached,
    // then serve and replay GOLD-1: the miss lands 152 ticks after the serve.
    app.tick(&pinned_input(vec![]));
    app.tick(&pinned_input(vec![(10.0, 10.0)])); // serve click (any position)
    let mut ticks_to_miss = 0;
    for tick in 1..400 {
        if matches!(app.phase, Phase::Miss { .. }) {
            ticks_to_miss = tick;
            break;
        }
        app.tick(&pinned_input(vec![]));
    }
    assert_eq!(ticks_to_miss, 152, "GOLD-1 rally length through the app");
    assert_eq!(app.world.as_ref().map_or(0, |w| w.player_lives), 4);

    // 19 pop ticks, then the re-serve routing lands at frame 91 with the
    // enemy persisting and a fresh ball one tick later.
    let Phase::Miss { tick: miss_tick } = app.phase else {
        unreachable!("the loop above only exits in the Miss phase")
    };
    for _ in miss_tick..MISS_TICKS {
        app.tick(&pinned_input(vec![]));
    }
    assert_eq!(app.phase, Phase::Playing { frame: 91 });
    assert!(
        app.world
            .as_ref()
            .is_some_and(|w| w.enemy.is_some() && w.ball.is_none())
    );
    app.tick(&pinned_input(vec![]));
    assert!(
        app.world.as_ref().is_some_and(|w| w.ball.is_some()),
        "fresh ball at frame 92"
    );
}

#[test]
fn silky_app_ticks_at_400hz_without_speeding_wall_clock() {
    let mut app = App::new();
    app.visual_mode = VisualMode::Silky;

    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_START)]));
    assert_eq!(app.phase, Phase::StartGameInit { tick: 1 });

    let init_ticks = silky_ticks_for_flash_frames(START_GAME_TICKS - 1);
    for _ in 1..init_ticks {
        app.tick(&pinned_input(vec![]));
    }
    assert!(
        matches!(app.phase, Phase::StartGameInit { .. }),
        "init should still be in progress before the scaled wall-clock duration"
    );

    app.tick(&pinned_input(vec![]));
    assert_eq!(app.phase, Phase::LevelSplash { tick: 0 });
    assert!(
        app.world.is_some(),
        "world created at the scaled init boundary"
    );
}

#[test]
fn silky_app_tick_advances_one_400hz_world_slice() {
    let mut world = World::new(Published::default());
    world.spawn_ball();
    let ball = world.ball.as_mut().expect("ball");
    ball.just_spawned = false;
    ball.vel.z = 2.0;

    let mut app = App::new();
    app.visual_mode = VisualMode::Silky;
    app.world = Some(world);
    app.phase = Phase::Playing {
        frame: FRAME_PLAY_HOLD,
    };

    for _ in 0..SILKY_PHYSICS_HZ {
        app.tick(&pinned_input(vec![]));
    }

    let ball = app
        .world
        .as_ref()
        .and_then(|world| world.ball)
        .expect("ball");
    assert!((ball.pos.z - 60.0).abs() < 1e-9, "z={}", ball.pos.z);
}

#[test]
fn silky_bonus_drain_keeps_30hz_wall_clock_cadence() {
    let mut world = World::new(Published::default());
    world.level_setup();
    world.spawn_ball();
    let ball = world.ball.as_mut().expect("ball");
    ball.just_spawned = false;
    ball.vel.z = 2.0;

    let mut app = App::new();
    app.visual_mode = VisualMode::Silky;
    app.world = Some(world);
    app.phase = Phase::Playing {
        frame: FRAME_PLAY_HOLD,
    };

    let first_drain = silky_ticks_for_flash_frames(11);
    for _ in 1..first_drain {
        app.tick(&pinned_input(vec![]));
    }
    assert_eq!(
        app.world.as_ref().expect("world").economy.bonus_display,
        3000
    );

    app.tick(&pinned_input(vec![]));
    assert_eq!(
        app.world.as_ref().expect("world").economy.bonus_display,
        2975
    );
}

#[test]
fn silky_flash_animation_keeps_original_wall_clock_duration() {
    let mut app = App::new();
    app.visual_mode = VisualMode::Silky;
    app.player_flash = Some(PipFlash {
        zone: Zone::C,
        tick: 0,
    });

    let flash_ticks = silky_ticks_for_flash_frames(PIP_FLASH_TICKS);
    for _ in 1..flash_ticks {
        app.tick(&pinned_input(vec![]));
    }
    assert!(
        app.player_flash.is_some(),
        "flash should still be visible before the scaled wall-clock duration"
    );

    app.tick(&pinned_input(vec![]));
    assert!(app.player_flash.is_none());
}

#[test]
fn title_visual_button_toggles_runtime_visual_mode() {
    let mut app = App::new();
    assert_eq!(app.phase, Phase::Title);
    assert_eq!(app.visual_mode, VisualMode::Faithful);

    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_VISUAL)]));
    assert_eq!(app.phase, Phase::Title);
    assert_eq!(app.visual_mode, VisualMode::Silky);

    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_VISUAL)]));
    assert_eq!(app.phase, Phase::Title);
    assert_eq!(app.visual_mode, VisualMode::Faithful);
}

#[test]
fn returning_to_title_preserves_runtime_visual_settings() {
    let mut app = App::new();
    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_VISUAL)]));
    assert_eq!(app.visual_mode, VisualMode::Silky);

    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_SCORES)]));
    assert_eq!(app.phase, Phase::HighScores);
    app.tick(&pinned_input(vec![rect_center(BTN_HS_MENU)]));

    assert_eq!(app.phase, Phase::Title);
    assert_eq!(app.visual_mode, VisualMode::Silky);
}

#[test]
fn level_up_routing_banks_bonus_and_replays_splash() {
    let mut app = App::new();
    app.tick(&pinned_input(vec![(175.0, 116.0)]));
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    // Fake a won rally: enemy out of lives, pop about to route.
    if let Some(world) = &mut app.world {
        world.spawn_enemy();
        world.spawn_ball();
        world.enemy_lives = 0;
        world.economy.score = 500;
        world.economy.bonus_display = 2750;
    }
    app.phase = Phase::Miss {
        tick: MISS_TICKS - 1,
    };
    app.tick(&pinned_input(vec![]));
    // The routing tick already renders splash tick 1 (frame 45).
    assert_eq!(app.phase, Phase::LevelSplash { tick: 1 });
    let world = app.world.as_ref().expect("world");
    assert_eq!(world.level, 2);
    assert_eq!(world.economy.score, 500 + 2750);
    assert_eq!(world.enemy_lives, 3);
    assert!(
        world.ball.is_none() && world.enemy.is_none(),
        "level goto removes both"
    );
    // 45 more ticks finish the splash with the level-2 setup applied.
    for _ in 1..SPLASH_TICKS {
        app.tick(&pinned_input(vec![]));
    }
    assert_eq!(app.phase, Phase::Playing { frame: 90 });
    let world = app.world.as_ref().expect("world");
    assert_eq!(world.params.speed, 2.33);
    assert_eq!(world.economy.bonus_display, 3000);
}

#[test]
fn name_entry_records_only_edited_names() {
    let dir = unique_temp_path("curveball-ne");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut app = App::new();
    // Isolate the score table so repeated test runs stay hermetic.
    app.scores = ScoreTable::load_from(dir.join("highscores.txt"));
    // Drop straight into name entry with a known world.
    app.tick(&pinned_input(vec![(175.0, 116.0)]));
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    if let Some(world) = &mut app.world {
        world.economy.score = 777;
        world.level = 4;
    }
    app.phase = Phase::GameOver {
        tick: GAME_OVER_TICKS - 1,
    };
    app.tick(&pinned_input(vec![]));
    assert_eq!(
        app.phase,
        Phase::NameEntry,
        "777 qualifies against the default table"
    );

    // Type over the placeholder, then submit (click inside BTN_SUBMIT).
    let mut input = pinned_input(vec![]);
    input.chars = vec!['k', 'r'];
    app.tick(&input);
    assert_eq!(app.name_entry.text, "kr");
    let mut input = pinned_input(vec![(175.0, 198.0)]);
    input.backspaces = 1;
    app.tick(&input);
    assert_eq!(app.phase, Phase::HighScores);
    assert_eq!(
        app.scores.entries[0].name, "k",
        "backspace applied before submit"
    );
    assert_eq!(app.scores.entries[0].score, 777);
    assert_eq!(app.scores.entries[0].level, 4);

    app.tick(&pinned_input(vec![rect_center(BTN_HS_MENU)]));
    assert_eq!(app.phase, Phase::Title);
    assert!(app.world.is_none());
    assert!(!app.bonus_hud_blanked);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn game_over_routes_after_seven_ticks() {
    let mut app = App::new();
    // Isolate from any real score file beside the test executable.
    app.scores = ScoreTable::load_from(unique_temp_path("curveball-nonexistent"));
    app.tick(&pinned_input(vec![(175.0, 116.0)]));
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    // Exhaust player lives instantly and fake a finished pop.
    if let Some(world) = &mut app.world {
        world.player_lives = 0;
        world.spawn_enemy();
        world.spawn_ball();
    }
    app.phase = Phase::Miss {
        tick: MISS_TICKS - 1,
    };
    app.tick(&pinned_input(vec![]));
    assert_eq!(app.phase, Phase::GameOver { tick: 1 });
    assert!(app.bonus_hud_blanked);
    assert!(
        app.world
            .as_ref()
            .is_some_and(|w| w.ball.is_none() && w.enemy.is_none())
    );
    for tick in 2..=GAME_OVER_TICKS {
        assert_eq!(app.phase, Phase::GameOver { tick: tick - 1 });
        app.tick(&pinned_input(vec![]));
    }
    // Score 0 does not qualify against the all-zero default table.
    assert_eq!(app.phase, Phase::End);

    app.tick(&pinned_input(vec![rect_center(BTN_END_MENU)]));
    assert_eq!(app.phase, Phase::Title);
    assert!(app.world.is_none());
    assert!(!app.bonus_hud_blanked);

    app.tick(&pinned_input(vec![(175.0, 116.0)]));
    assert_eq!(app.phase, Phase::StartGameInit { tick: 1 });
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    let world = app.world.as_ref().expect("fresh world");
    assert_eq!(world.level, 1);
    assert_eq!(world.player_lives, 5);
    assert_eq!(world.enemy_lives, 3);
    assert_eq!(world.economy.score, 0);
}

#[test]
fn post_game_main_menu_starts_next_game_from_clean_world() {
    let mut app = App::new();
    let stale = Published {
        pos: Vec3 {
            x: 12.0,
            y: 34.0,
            z: 56.0,
        },
        dir: Vec3 {
            x: 1.0,
            y: -2.0,
            z: 3.0,
        },
    };
    app.world = Some(World::new(stale));
    app.mode = GameMode::Zen;
    app.phase = Phase::End;

    app.tick(&pinned_input(vec![rect_center(BTN_END_MENU)]));
    assert_eq!(app.phase, Phase::Title);
    assert_eq!(app.mode, GameMode::Classic);
    assert!(app.world.is_none());

    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_START)]));
    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    let world = app.world.as_ref().expect("fresh world");
    assert_eq!(world.published, Published::default());
    assert!(!world.unlimited_player_lives);
}

#[test]
fn zen_button_starts_game_with_unlimited_lives() {
    let mut app = App::new();
    app.tick(&pinned_input(vec![rect_center(BTN_TITLE_ZEN)]));
    assert_eq!(app.phase, Phase::StartGameInit { tick: 1 });
    assert_eq!(app.mode, GameMode::Zen);

    for _ in 0..START_GAME_TICKS - 1 {
        app.tick(&pinned_input(vec![]));
    }
    let world = app.world.as_ref().expect("zen world");
    assert!(world.unlimited_player_lives);
    assert_eq!(world.player_lives, 5);

    if let Some(world) = &mut app.world {
        world.player_lives = 0;
        world.spawn_enemy();
        world.spawn_ball();
    }
    app.phase = Phase::Miss {
        tick: MISS_TICKS - 1,
    };
    app.tick(&pinned_input(vec![]));
    assert_eq!(app.phase, Phase::Playing { frame: 91 });
    assert!(!app.bonus_hud_blanked);
}

// ---------------------------------------------------------------------------
// High scores (deviation D3)
// ---------------------------------------------------------------------------

#[test]
fn highscores_defaults_qualify_and_insert() {
    let dir = unique_temp_path("curveball-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("highscores.txt");

    let mut table = ScoreTable::load_from(path.clone());
    assert_eq!(table.entries.len(), 10);
    assert!(
        table
            .entries
            .iter()
            .all(|e| e.name == "none" && e.level == 0 && e.score == 0)
    );
    assert!(!table.qualifies(0), "strictly greater than the 10th entry");
    assert!(table.qualifies(1));

    table.insert("alpha".to_owned(), 3, 500);
    table.insert("beta".to_owned(), 2, 800);
    table.insert("gamma".to_owned(), 1, 500); // tie inserts after equals
    assert_eq!(table.entries[0].name, "beta");
    assert_eq!(table.entries[1].name, "alpha");
    assert_eq!(table.entries[2].name, "gamma");
    assert_eq!(table.entries.len(), 10);

    table.save();
    let reloaded = ScoreTable::load_from(path.clone());
    assert_eq!(reloaded.entries, table.entries);

    // Corrupt lines fall back to defaults individually.
    std::fs::write(
        &path,
        "good\t2\t300\nbad line without tabs\nworse\tnan\t12\n",
    )
    .expect("write corrupt file");
    let corrupt = ScoreTable::load_from(path);
    assert_eq!(corrupt.entries[0].name, "good");
    assert_eq!(corrupt.entries[1].name, "none");
    assert_eq!(corrupt.entries[2].name, "none");
    assert_eq!(corrupt.entries.len(), 10);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn highscores_load_sorts_truncates_and_pads() {
    let dir = unique_temp_path("curveball-sort");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("highscores.txt");

    std::fs::write(
        &path,
        (0..12)
            .map(|i| format!("p{i}\t{}\t{}", i % 10, 100 + i * 10))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("write unsorted table");
    let table = ScoreTable::load_from(path.clone());
    assert_eq!(table.entries.len(), 10);
    assert_eq!(table.entries[0].name, "p11");
    assert_eq!(table.entries[0].score, 210);
    assert_eq!(table.entries[9].name, "p2");
    assert_eq!(table.entries[9].score, 120);
    assert!(table.qualifies(121));
    assert!(!table.qualifies(120));

    std::fs::write(&path, "low\t1\t10\nhigh\t2\t50\n").expect("write short table");
    let padded = ScoreTable::load_from(path);
    assert_eq!(padded.entries.len(), 10);
    assert_eq!(padded.entries[0].name, "high");
    assert_eq!(padded.entries[1].name, "low");
    assert_eq!(padded.entries[2].name, "none");
    assert_eq!(padded.entries[2].score, 0);
    let _ = std::fs::remove_dir_all(dir);
}
