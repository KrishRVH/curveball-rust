//! Window configuration and virtual-stage coordinate mapping.

use curveball::consts::{RENDER_SCALE, STAGE_H, STAGE_W};
use macroquad::prelude::*;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "curveball".to_owned(),
        window_width: (STAGE_W as i32) * RENDER_SCALE as i32,
        window_height: (STAGE_H as i32) * RENDER_SCALE as i32,
        high_dpi: false,
        platform: macroquad::miniquad::conf::Platform {
            // Request display refresh pacing when the platform honors it; the
            // app never caps rendering to the original 30 Hz SWF tick.
            swap_interval: Some(1),
            apple_gfx_api: macroquad::miniquad::conf::AppleGfxApi::OpenGl,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Letterbox transform for the current window: integer scale and offset.
pub fn letterbox() -> (f32, f32, f32) {
    let scale = (screen_width() / STAGE_W as f32)
        .min(screen_height() / STAGE_H as f32)
        .floor()
        .max(1.0);
    let off_x = (scale.mul_add(-(STAGE_W as f32), screen_width())) / 2.0;
    let off_y = (scale.mul_add(-(STAGE_H as f32), screen_height())) / 2.0;
    (scale, off_x, off_y)
}

pub fn letterbox_viewport(scale: f32, off_x: f32, off_y: f32) -> (i32, i32, i32, i32) {
    (
        off_x.round() as i32,
        off_y.round() as i32,
        (STAGE_W as f32 * scale).round() as i32,
        (STAGE_H as f32 * scale).round() as i32,
    )
}
