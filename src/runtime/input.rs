//! Input latching from display frames into deterministic simulation ticks.

use curveball::app::TickInput;
use macroquad::prelude::*;

use super::config::letterbox;

/// Pending input latched per display frame, drained by exactly one tick.
#[derive(Default)]
pub struct InputLatch {
    mouse: (f64, f64),
    clicks: Vec<(f64, f64)>,
    chars: Vec<char>,
    backspaces: u32,
}

impl InputLatch {
    /// Record this display frame's events in virtual-canvas coordinates.
    pub fn latch(&mut self, fixed_mouse: Option<(f64, f64)>) {
        if let Some(mouse) = fixed_mouse {
            self.mouse = mouse;
        } else {
            let (scale, off_x, off_y) = letterbox();
            let (mx, my) = mouse_position();
            // Unclamped: the paddle clamps itself.
            self.mouse = (
                f64::from((mx - off_x) / scale),
                f64::from((my - off_y) / scale),
            );
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            self.clicks.push(self.mouse);
        }
        while let Some(c) = get_char_pressed() {
            self.chars.push(c);
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.backspaces += 1;
        }
    }

    pub fn drain(&mut self) -> TickInput {
        TickInput {
            mouse: self.mouse,
            clicks: std::mem::take(&mut self.clicks),
            chars: std::mem::take(&mut self.chars),
            backspaces: std::mem::take(&mut self.backspaces),
        }
    }
}
