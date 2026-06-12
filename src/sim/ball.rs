//! Ball — frame_92/PlaceObject2_80_20 clip scripts (load / enterFrame / mouseDown).
//!
//! The per-tick algorithm reproduces the source statement for statement:
//! curve accelerates velocity, position integrates (y *subtracts* vel.y),
//! curve decays (skipped for exact zero — IEEE `-0.0 != 0.0` is false, quirk
//! Q10), walls clamp/reflect/damp, then the depth planes are tested with the
//! previous tick's rendered ball rect against this tick's paddle rects.
//! Paddle contact reflects only z; lateral velocity passes through and curve
//! is fully replaced (quirk Q7).
//!
//! Dead code in the originals, preserved here as documentation only (quirk
//! Q1): `world.curveDecay = 0.01` (the ball uses its local 1.004),
//! `world.bounce = 1`, `world.lagFactor` (read, never used), `m = 100` /
//! `f = 0.8` in all four clips, `growshrink`, and the paddle's
//! `_root._xmouse = wx` write (a failed attempt to center the cursor).

use super::project::{Rect, scale, vis};
use crate::consts::{
    BALL_DIAMETER, CURVE_DECAY, WALL_CURVE_DAMP, WORLD_BOTTOM, WORLD_CX, WORLD_CY, WORLD_LEFT,
    WORLD_RIGHT, WORLD_TOP,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Paddle state as cached by the ball at the top of its enterFrame.
///
/// (`pPosX`/`pSpeedX`...). The serve handler reads this cache, so it sees the
/// paddle as of the last tick the ball ran — frozen at the miss tick during a
/// pop (quirk Q2). A freshly spawned ball has not cached anything; AS1/SWF5
/// coerces the undefined reads to 0.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaddleSnapshot {
    pub pos: (f64, f64),
    pub speed: (f64, f64),
}

#[derive(Debug, Clone, Copy)]
pub struct Ball {
    pub pos: Vec3,
    pub vel: Vec3,
    pub curve: (f64, f64),
    /// `ballStop` — set on a miss; the entire enterFrame body is gated on it.
    pub stopped: bool,
    /// `speed = world.speed`, cached at load.
    pub speed: f64,
    /// `curveAmount = world.curveAmount`, cached at load.
    pub curve_amount: f64,
    /// The ball's rendered rect as of the end of its last completed tick;
    /// next tick's collisions and the serve hit-test read this (quirk Q5).
    pub prev_rect: Rect,
    /// See [`PaddleSnapshot`].
    pub snapshot: PaddleSnapshot,
    pub just_spawned: bool,
}

impl Ball {
    /// Load script: centered at (wx, wy, 0), all velocities zero, rendered
    /// full-size at the center — the load-time rect stands in for
    /// `prev_rect` until the first enterFrame completes.
    #[must_use]
    pub fn new(speed: f64, curve_amount: f64) -> Self {
        Self {
            pos: Vec3 {
                x: WORLD_CX,
                y: WORLD_CY,
                z: 0.0,
            },
            vel: Vec3::default(),
            curve: (0.0, 0.0),
            stopped: false,
            speed,
            curve_amount,
            prev_rect: Rect::centered((WORLD_CX, WORLD_CY), BALL_DIAMETER, BALL_DIAMETER),
            snapshot: PaddleSnapshot::default(),
            just_spawned: true,
        }
    }

    /// Integration, curve decay, and wall handling — the first half of the
    /// enterFrame. Returns (bounced horizontal wall, bounced vertical wall),
    /// i.e. (`wallBounce1`, `wallBounce2`) triggers. The `curve != 0.0`
    /// guards are exact IEEE comparisons; `-0.0` skips decay (quirk Q10).
    pub fn integrate_and_walls(&mut self) -> (bool, bool) {
        self.vel.x += self.curve.0;
        self.vel.y += self.curve.1;
        self.pos.z += self.vel.z;
        self.pos.x += self.vel.x;
        self.pos.y -= self.vel.y;

        if self.curve.0 != 0.0 {
            self.curve.0 /= CURVE_DECAY;
        }
        if self.curve.1 != 0.0 {
            self.curve.1 /= CURVE_DECAY;
        }

        let radius = BALL_DIAMETER / 2.0;
        let mut bounce_y = false;
        let mut bounce_x = false;
        if self.pos.y - radius < WORLD_TOP {
            self.pos.y = WORLD_TOP + radius;
            self.curve.1 /= WALL_CURVE_DAMP;
            self.vel.y = -self.vel.y;
            bounce_y = true;
        } else if WORLD_BOTTOM < self.pos.y + radius {
            self.pos.y = WORLD_BOTTOM - radius;
            self.curve.1 /= WALL_CURVE_DAMP;
            self.vel.y = -self.vel.y;
            bounce_y = true;
        }
        if self.pos.x - radius < WORLD_LEFT {
            self.pos.x = WORLD_LEFT + radius;
            self.curve.0 /= WALL_CURVE_DAMP;
            self.vel.x = -self.vel.x;
            bounce_x = true;
        } else if WORLD_RIGHT < self.pos.x + radius {
            self.pos.x = WORLD_RIGHT - radius;
            self.curve.0 /= WALL_CURVE_DAMP;
            self.vel.x = -self.vel.x;
            bounce_x = true;
        }
        (bounce_x, bounce_y)
    }

    /// Miss handling shared by both planes: zero everything and stop. Lives
    /// and (for the player side) bonus resets are the caller's business.
    pub const fn stop_for_miss(&mut self) {
        self.vel = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        self.curve = (0.0, 0.0);
        self.stopped = true;
    }

    /// End of enterFrame: project, store the rendered rect for the next
    /// tick's collisions, hand back what the parent publishes.
    pub fn project_and_rect(&mut self) -> Vec3 {
        let s = scale(self.pos.z);
        let (vx, vy) = vis(self.pos.x, self.pos.y, self.pos.z);
        self.prev_rect = Rect::centered((vx, vy), BALL_DIAMETER * s, BALL_DIAMETER * s);
        self.pos
    }
}
