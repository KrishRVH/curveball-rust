//! Pure-`f64`, std-only simulation core.
//!
//! One [`World::tick`] reproduces one Flash frame in the original's clip-event
//! order (mouse events before the frame, then userPaddle → enemyPaddle → ring
//! → ball). The per-tick order is part of the spec: the enemy consumes the
//! ball state *published at the end of the previous tick*, the ring shows the
//! previous tick's depth, and collisions test the previous tick's rendered
//! ball rect against this tick's paddle rects (quirk Q5).

pub mod ball;
pub mod enemy;
pub mod paddle;
pub mod project;
pub mod score;

pub use ball::{Ball, PaddleSnapshot, Vec3};
pub use enemy::Enemy;
pub use paddle::Paddle;
pub use project::{Rect, overlap, scale, vis};
pub use score::{CurveClass, Economy};

use crate::consts::{
    ENEMY_LIVES_INIT, LEVEL_CURVE, LEVEL_SKILL, LEVEL_SPEED, PADDLE_H, PADDLE_W, PLAYER_LIVES_INIT,
    SERVE_MIN_CURVE, STRICT_LEVEL_11_SOFTLOCK, WORLD_CX, WORLD_CY, WORLD_DEPTH, ZONE_DX, ZONE_DY,
};

/// `_parent.ballPosX/Y/Z` and `ballDirX/Y/Z` parent-timeline variables.
///
/// Written at the end of each ball enterFrame. They persist across rallies
/// within a live game. The original also let them bleed across new games, but
/// D8 clears stale post-game state on the title path; before any ball has ever
/// run, AS1 reads them as `undefined`, which SWF5 coerces to 0, so the zero
/// default reproduces the startup case.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Published {
    pub pos: Vec3,
    pub dir: Vec3,
}

/// Paddle-contact zone — drives the pip flash and the accuracy bonus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    UR,
    UL,
    BL,
    BR,
    C,
}

/// Gameplay events emitted by the sim; sounds, pip flashes, banner triggers
/// and phase transitions all derive from these, keeping the core headless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimEvent {
    /// `horizontal` = a left/right wall (`wallBounce1`); vertical walls play `wallBounce2`.
    WallBounce {
        horizontal: bool,
    },
    EnemyHit {
        zone: Zone,
    },
    PlayerHit {
        zone: Zone,
        accuracy: bool,
        curve: CurveClass,
    },
    Serve {
        zone: Zone,
        accuracy: bool,
        curve: CurveClass,
    },
    PlayerMiss,
    EnemyMiss,
}

/// Per-tick input to the sim. Clicks anywhere on the screen attempt a serve.
#[derive(Debug, Clone, Copy, Default)]
pub struct SimInput {
    pub mouse: (f64, f64),
    pub serve_clicks: u32,
}

/// Per-level parameters cached from the tables at level setup (frame_90).
#[derive(Debug, Clone, Copy)]
pub struct LevelParams {
    pub speed: f64,
    pub skill: f64,
    pub curve_amount: f64,
}

/// Level parameters for a 1-based level number.
///
/// The tables are indexed by level − 1 and hold exactly 10 entries. Past
/// level 10 the index clamps to the last entry (deviation D1) unless
/// [`STRICT_LEVEL_11_SOFTLOCK`] reproduces the original: AS1/SWF5 reads the
/// missing entry as `undefined` and coerces it to 0 wherever it is used.
#[must_use]
pub fn level_params(level: u32) -> LevelParams {
    let index = (level as usize).saturating_sub(1);
    if let (Some(&speed), Some(&skill), Some(&curve_amount)) = (
        LEVEL_SPEED.get(index),
        LEVEL_SKILL.get(index),
        LEVEL_CURVE.get(index),
    ) {
        LevelParams {
            speed,
            skill,
            curve_amount,
        }
    } else if STRICT_LEVEL_11_SOFTLOCK {
        LevelParams {
            speed: 0.0,
            skill: 0.0,
            curve_amount: 0.0,
        }
    } else {
        LevelParams {
            speed: LEVEL_SPEED[9],
            skill: LEVEL_SKILL[9],
            curve_amount: LEVEL_CURVE[9],
        }
    }
}

/// Hit-zone classification — the exact branch cascade from the ball scripts.
///
/// `(bx, by)` is the ball's world position (post-wall-clamp on returns),
/// `(px, py)` the paddle center the script compares against.
#[must_use]
pub fn classify(bx: f64, by: f64, px: f64, py: f64) -> Zone {
    if px + ZONE_DX < bx {
        if by < py { Zone::UR } else { Zone::BR }
    } else if bx < px - ZONE_DX {
        if by < py { Zone::UL } else { Zone::BL }
    } else if py + ZONE_DY >= by {
        if by >= py - ZONE_DY {
            Zone::C
        } else if bx >= px {
            Zone::UR
        } else {
            Zone::UL
        }
    } else if bx >= px {
        Zone::BR
    } else {
        Zone::BL
    }
}

/// The full gameplay state for one game, minus the phase machine.
///
/// (The phase machine lives in [`crate::app`].) Entities are `Option` because
/// the timeline places and removes them: the paddle persists from the first
/// Level splash to Game Over, the enemy persists across rallies within a
/// level, and the ball/ring are fresh every rally (quirk Q6).
#[derive(Debug)]
pub struct World {
    pub level: u32,
    pub params: LevelParams,
    pub player_lives: i32,
    pub enemy_lives: i32,
    pub economy: Economy,
    pub paddle: Paddle,
    pub enemy: Option<Enemy>,
    pub ball: Option<Ball>,
    pub published: Published,
    pub unlimited_player_lives: bool,
    /// Depth ring state: previous tick's published z, floored at 0. Cosmetic.
    pub ring_z: f64,
    ring_just_spawned: bool,
}

impl World {
    /// frame_44 init. The caller chooses whether `published` carries over:
    /// intra-game reroutes preserve it, while the D8 title path starts fresh.
    #[must_use]
    pub fn new(published: Published) -> Self {
        Self {
            level: 1,
            params: level_params(1),
            player_lives: PLAYER_LIVES_INIT,
            enemy_lives: ENEMY_LIVES_INIT,
            economy: Economy::new(),
            paddle: Paddle::new(),
            enemy: None,
            ball: None,
            published,
            unlimited_player_lives: false,
            ring_z: 0.0,
            ring_just_spawned: false,
        }
    }

    /// frame_90 per-level setup: table parameters and the per-level economy
    /// reset. Lives and score are untouched.
    pub fn level_setup(&mut self) {
        self.params = level_params(self.level);
        self.economy.level_setup();
    }

    /// Sprite-80 frame-20 routing, level-up branch: bank the remaining bonus,
    /// restore enemy lives, and tear down the clips the backwards goto to the
    /// Level label removes (ball, ring, enemy — the player paddle survives).
    pub fn route_level_up(&mut self) {
        self.level += 1;
        self.economy.score += i64::from(self.economy.bonus_display);
        self.enemy_lives = ENEMY_LIVES_INIT;
        self.ball = None;
        self.enemy = None;
    }

    /// Timeline frame 91: place the enemy paddle (fresh: centered, current
    /// skill factor).
    pub fn spawn_enemy(&mut self) {
        self.enemy = Some(Enemy::new(self.params.skill));
    }

    /// Timeline frame 92: place a fresh ball and ring. The ring's load state
    /// is z = 0 (full size, full alpha) regardless of stale published data.
    pub fn spawn_ball(&mut self) {
        self.ball = Some(Ball::new(self.params.speed, self.params.curve_amount));
        self.ring_z = 0.0;
        self.ring_just_spawned = true;
    }

    /// One Flash frame in clip-event order.
    pub fn tick(&mut self, input: &SimInput) -> Vec<SimEvent> {
        let mut events = Vec::new();
        // 0. Input phase — mouse events dispatch before the frame.
        for _ in 0..input.serve_clicks {
            self.try_serve(&mut events);
        }
        // 1. Player paddle.
        self.paddle.step(input.mouse);
        // 2. Enemy paddle — consumes the previous tick's published ball state.
        if let Some(enemy) = &mut self.enemy {
            enemy.step(&self.published);
        }
        // 3. Ring — cosmetic; also reads the previous tick's publish.
        self.ring_step();
        // 4. Ball — consumes paddle state computed this tick.
        self.ball_step(&mut events);
        events
    }

    fn ring_step(&mut self) {
        if self.ball.is_none() {
            return;
        }
        if self.ring_just_spawned {
            self.ring_just_spawned = false;
            return;
        }
        self.ring_z = self.published.pos.z.max(0.0);
    }

    /// frame_92 ball mouseDown. Not gated on `stopped` — during the Miss pop
    /// the frozen ball still has `vel.z == 0` (an exact IEEE comparison), so
    /// a click over the pop awards serve scoring once without un-freezing
    /// anything (quirk Q2). The hit test pairs the ball's last rendered rect
    /// with the paddle's live rect, while the zone/curve math uses the
    /// snapshot cached the last time the ball's enterFrame ran.
    fn try_serve(&mut self, events: &mut Vec<SimEvent>) {
        let Some(ball) = &mut self.ball else { return };
        if ball.vel.z != 0.0 {
            return;
        }
        let paddle_rect = Rect::centered(self.paddle.pos, PADDLE_W, PADDLE_H);
        if !overlap(&ball.prev_rect, &paddle_rect) {
            return;
        }
        let snap = ball.snapshot;
        let zone = classify(ball.pos.x, ball.pos.y, snap.pos.0, snap.pos.1);
        let accuracy = zone == Zone::C;
        if accuracy {
            self.economy.award_accuracy();
        }
        ball.vel.z = ball.speed;
        ball.curve.0 = -snap.speed.0 / ball.curve_amount;
        ball.curve.1 = snap.speed.1 / ball.curve_amount;
        if ball.curve.0.abs() < SERVE_MIN_CURVE {
            ball.curve.0 = if snap.pos.0 < WORLD_CX {
                SERVE_MIN_CURVE
            } else {
                -SERVE_MIN_CURVE
            };
        }
        if ball.curve.1.abs() < SERVE_MIN_CURVE {
            ball.curve.1 = if WORLD_CY < snap.pos.1 {
                SERVE_MIN_CURVE
            } else {
                -SERVE_MIN_CURVE
            };
        }
        let curve = self.economy.award_curve(ball.curve.0, ball.curve.1);
        events.push(SimEvent::Serve {
            zone,
            accuracy,
            curve,
        });
    }

    /// frame_92 ball enterFrame.
    fn ball_step(&mut self, events: &mut Vec<SimEvent>) {
        let Some(ball) = &mut self.ball else { return };
        if ball.just_spawned {
            // A clip placed at frame N runs its first enterFrame at N + 1.
            ball.just_spawned = false;
            return;
        }
        if ball.stopped {
            return;
        }
        // Snapshot reads at the top of the enterFrame — this tick's paddle
        // values (both paddles already ran), cached for the serve handler.
        ball.snapshot = PaddleSnapshot {
            pos: self.paddle.pos,
            speed: self.paddle.speed,
        };
        let enemy_snap = self.enemy.as_ref().map(|e| (e.pos, e.speed));

        let (bounce_x, bounce_y) = ball.integrate_and_walls();
        // The source checks (and sounds) the vertical walls first.
        if bounce_y {
            events.push(SimEvent::WallBounce { horizontal: false });
        }
        if bounce_x {
            events.push(SimEvent::WallBounce { horizontal: true });
        }

        if WORLD_DEPTH < ball.pos.z {
            // Enemy side. A missing enemy clip makes hitTest false in Flash.
            let hit = enemy_snap.filter(|(pos, _)| {
                let s = scale(WORLD_DEPTH);
                let rect =
                    Rect::centered(vis(pos.0, pos.1, WORLD_DEPTH), PADDLE_W * s, PADDLE_H * s);
                overlap(&ball.prev_rect, &rect)
            });
            if let Some((e_pos, e_speed)) = hit {
                let zone = classify(ball.pos.x, ball.pos.y, e_pos.0, e_pos.1);
                ball.pos.z = WORLD_DEPTH;
                ball.curve.0 = e_speed.0 / ball.curve_amount;
                ball.curve.1 = -e_speed.1 / ball.curve_amount;
                ball.vel.z = -ball.vel.z;
                events.push(SimEvent::EnemyHit { zone });
            } else {
                ball.stop_for_miss();
                self.enemy_lives -= 1;
                events.push(SimEvent::EnemyMiss);
            }
        } else if ball.pos.z < 0.0 {
            // Player side.
            let paddle_rect = Rect::centered(self.paddle.pos, PADDLE_W, PADDLE_H);
            if overlap(&ball.prev_rect, &paddle_rect) {
                let snap = ball.snapshot;
                let zone = classify(ball.pos.x, ball.pos.y, snap.pos.0, snap.pos.1);
                let accuracy = zone == Zone::C;
                if accuracy {
                    self.economy.award_accuracy();
                }
                ball.pos.z = 0.0;
                ball.curve.0 = -snap.speed.0 / ball.curve_amount;
                ball.curve.1 = snap.speed.1 / ball.curve_amount;
                ball.vel.z = -ball.vel.z;
                self.economy.award_hit();
                let curve = self.economy.award_curve(ball.curve.0, ball.curve.1);
                events.push(SimEvent::PlayerHit {
                    zone,
                    accuracy,
                    curve,
                });
            } else {
                ball.stop_for_miss();
                if !self.unlimited_player_lives {
                    self.player_lives -= 1;
                }
                self.economy.reset_rally_bonuses();
                events.push(SimEvent::PlayerMiss);
            }
        }

        // End of the enterFrame — runs even on the miss tick (the gate was
        // checked at the top): project, publish, drain.
        let pos = ball.project_and_rect();
        self.published = Published { pos, dir: ball.vel };
        self.economy.drain_tick(ball.vel.z != 0.0);
    }
}
