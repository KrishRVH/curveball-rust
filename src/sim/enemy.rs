//! Enemy paddle — frame_91/PlaceObject2_75_13 clip scripts.
//!
//! Consumes the ball state *published at the end of the previous tick* (quirk
//! Q5): while the ball travels away from the player (`dir.z > 0`) it chases
//! the published position with the level's skill divisor, otherwise it eases
//! home to the world center with divisor 15. The instance persists across
//! rallies within a level and is reinstantiated (recentered, fresh skill
//! factor) only via the Level splash (quirk Q6).

use super::Published;
use super::paddle::clamp_to_world;
use crate::consts::{ENEMY_EASE_HOME, WORLD_CX, WORLD_CY};

#[derive(Debug, Clone, Copy)]
pub struct Enemy {
    /// World-plane position (rendered projected at fixed z = 75).
    pub pos: (f64, f64),
    /// Post-clamp per-tick delta, published for the ball's curve math.
    pub speed: (f64, f64),
    /// `skillFactor` cached from the world at load time.
    skill: f64,
    old: (f64, f64),
    just_spawned: bool,
}

impl Enemy {
    /// Load script: centered, zero speed, skill factor cached.
    #[must_use]
    pub const fn new(skill: f64) -> Self {
        Self {
            pos: (WORLD_CX, WORLD_CY),
            speed: (0.0, 0.0),
            skill,
            old: (WORLD_CX, WORLD_CY),
            just_spawned: true,
        }
    }

    /// One enterFrame: chase or ease home, clamp, derive speed.
    pub fn step(&mut self, published: &Published) {
        if self.just_spawned {
            self.just_spawned = false;
            return;
        }
        let (tx, ty, divisor) = if published.dir.z > 0.0 {
            (published.pos.x, published.pos.y, self.skill)
        } else {
            (WORLD_CX, WORLD_CY, ENEMY_EASE_HOME)
        };
        self.pos.0 -= (self.pos.0 - tx) / divisor;
        self.pos.1 -= (self.pos.1 - ty) / divisor;
        clamp_to_world(&mut self.pos);
        self.speed = (self.pos.0 - self.old.0, self.pos.1 - self.old.1);
        self.old = self.pos;
    }
}
