//! Player paddle — frame_45/PlaceObject2_59_43 clip scripts.
//!
//! The paddle eases toward the mouse with divisor 1.5, clamps to the world
//! rect (y before x, as in the source), and publishes its post-clamp delta as
//! its speed. The instance persists across rallies and levels (quirk Q6); it
//! is only removed at Game Over.

use crate::consts::{
    PADDLE_H, PADDLE_W, PLAYER_EASE, WORLD_BOTTOM, WORLD_CX, WORLD_CY, WORLD_LEFT, WORLD_RIGHT,
    WORLD_TOP,
};

#[derive(Debug, Clone, Copy)]
pub struct Paddle {
    /// World-plane position (the paddle lives on the z = 0 plane).
    pub pos: (f64, f64),
    /// Post-clamp per-tick delta (`mySpeed`), published for the ball's curve math.
    pub speed: (f64, f64),
    /// `oldPos` from the load script.
    old: (f64, f64),
    /// A clip placed at frame N receives its first enterFrame at frame N + 1.
    just_spawned: bool,
}

impl Paddle {
    /// Load script: `myPos = (wx, wy)`, `oldPos = myPos`, `mySpeed = 0`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pos: (WORLD_CX, WORLD_CY),
            speed: (0.0, 0.0),
            old: (WORLD_CX, WORLD_CY),
            just_spawned: true,
        }
    }

    /// One enterFrame: ease toward the mouse target, clamp, derive speed.
    pub fn step(&mut self, mouse: (f64, f64)) {
        if self.just_spawned {
            self.just_spawned = false;
            return;
        }
        self.pos = self.predicted_pos(mouse);
        self.speed = (self.pos.0 - self.old.0, self.pos.1 - self.old.1);
        self.old = self.pos;
    }

    /// Position after one easing step toward `mouse`, without mutating the
    /// paddle. Rendering uses this to reduce live input latency without
    /// changing the fixed-step simulation state.
    #[must_use]
    pub fn predicted_pos(&self, mouse: (f64, f64)) -> (f64, f64) {
        if self.just_spawned {
            return self.pos;
        }
        let mut pos = self.pos;
        pos.0 -= (pos.0 - mouse.0) / PLAYER_EASE;
        pos.1 -= (pos.1 - mouse.1) / PLAYER_EASE;
        clamp_to_world(&mut pos);
        pos
    }
}

impl Default for Paddle {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared paddle clamp — y before x, mirroring the source order. Half-extents
/// are 30/20 (60×40 shape), so x ∈ [55, 296] and y ∈ [45, 206].
pub fn clamp_to_world(pos: &mut (f64, f64)) {
    let half_h = PADDLE_H / 2.0;
    let half_w = PADDLE_W / 2.0;
    if pos.1 - half_h < WORLD_TOP {
        pos.1 = WORLD_TOP + half_h;
    } else if WORLD_BOTTOM < pos.1 + half_h {
        pos.1 = WORLD_BOTTOM - half_h;
    }
    if pos.0 - half_w < WORLD_LEFT {
        pos.0 = WORLD_LEFT + half_w;
    } else if WORLD_RIGHT < pos.0 + half_w {
        pos.0 = WORLD_RIGHT - half_w;
    }
}
