//! Depth projection and screen-space collision rectangles.
//!
//! All depth rendering in the original uses one scalar scale function (every
//! clip script repeats the same expression):
//!
//! ```text
//! s(z)      = (90 − atan(z / varA)·180/π) / 90          // varA = 31.066017
//! vis(p, z) = (wx − (wx − p.x)·s(z),  wy − (wy − p.y)·s(z))
//! ```
//!
//! Collisions are Flash `hitTest`: screen-space AABB intersection with
//! inclusive edges. The ball contributes its *previous tick's* rendered rect
//! (its `_x`/`_width` are only updated at the end of its own enterFrame),
//! while paddles contribute their *current* rect (quirk Q5).

use crate::consts::{VAR_A, WORLD_CX, WORLD_CY};

/// Projection scale at depth `z`. `z` may be slightly negative (ball overshoot
/// past the player plane), giving a scale slightly above 1.
#[must_use]
#[expect(
    clippy::suboptimal_flops,
    reason = "must reproduce the AS1 expression's operation order exactly"
)]
pub fn scale(z: f64) -> f64 {
    (90.0 - (z / VAR_A).atan() * 180.0 / std::f64::consts::PI) / 90.0
}

/// Project a world-plane point at depth `z` to screen coordinates.
#[must_use]
#[expect(
    clippy::suboptimal_flops,
    reason = "must reproduce the AS1 expression's operation order exactly"
)]
pub fn vis(x: f64, y: f64, z: f64) -> (f64, f64) {
    let s = scale(z);
    (WORLD_CX - (WORLD_CX - x) * s, WORLD_CY - (WORLD_CY - y) * s)
}

/// A screen-space rectangle identified by its center and full extents,
/// mirroring how Flash exposes `_x`/`_width` for the centered shapes here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    #[must_use]
    pub const fn centered(center: (f64, f64), w: f64, h: f64) -> Self {
        Self {
            cx: center.0,
            cy: center.1,
            w,
            h,
        }
    }
}

/// Flash `hitTest(clip)` — bounding-box intersection, edges inclusive.
#[must_use]
pub fn overlap(a: &Rect, b: &Rect) -> bool {
    (a.cx - b.cx).abs() * 2.0 <= a.w + b.w && (a.cy - b.cy).abs() * 2.0 <= a.h + b.h
}
