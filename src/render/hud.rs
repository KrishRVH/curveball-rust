//! HUD: score/level/bonus text rows, lives dots, and the bonus banner.
//!
//! The HUD is placed at frame 90 (the end of the Level splash) and persists
//! through Game Over and the screens over it; only the bonus strings blank
//! at Game Over. The bonus banner renders text over the stage background.

use curveball::app::App;
use curveball::consts::{
    BANNER_BASELINE_OFFSET, BANNER_TEXT_ANCHOR_Y, BANNER_TEXT_CX, BONUS_LABEL_X, COLOR_HUD,
    HUD_BOTTOM_BASELINE, HUD_FONT_PX, HUD_TOP_BASELINE, LEVEL_LABEL_X, LIVES_DOT_SPACING,
    LIVES_ENEMY_ANCHOR, LIVES_PLAYER_ANCHOR, SCORE_LABEL_X,
};
use macroquad::prelude::*;

use super::Visuals;
use super::anim::BANNER_FRAMES;
use super::entities::{Textures, rgb, rgba};
use super::text;

const HUD_TEXT_ASPECT: f32 = 0.72;
const HUD_TRACKING: f32 = 1.0;
const BANNER_TEXT_ASPECT: f32 = 0.72;
const BANNER_TRACKING: f32 = 0.8;
const BANNER_DISPLAY_FONT_PX: u16 = 10;

/// Depths 26–42: the text HUD.
pub fn draw_text_hud(app: &App) {
    let Some(world) = &app.world else { return };
    let cyan = rgb(COLOR_HUD);
    let score = text::text_buf::<32>(format_args!("SCORE: {}", world.economy.score));
    text::left_tracked_aspect(
        score.as_str(),
        SCORE_LABEL_X,
        HUD_TOP_BASELINE,
        HUD_FONT_PX,
        cyan,
        HUD_TRACKING,
        HUD_TEXT_ASPECT,
    );
    let level = text::text_buf::<24>(format_args!("LEVEL: {}", world.level));
    text::left_tracked_aspect(
        level.as_str(),
        LEVEL_LABEL_X,
        HUD_TOP_BASELINE,
        HUD_FONT_PX,
        cyan,
        HUD_TRACKING,
        HUD_TEXT_ASPECT,
    );
    if !app.bonus_hud_blanked {
        let bonus = text::text_buf::<32>(format_args!("BONUS: {}", world.economy.bonus_display));
        text::left_tracked_aspect(
            bonus.as_str(),
            BONUS_LABEL_X,
            HUD_BOTTOM_BASELINE,
            HUD_FONT_PX,
            cyan,
            HUD_TRACKING,
            HUD_TEXT_ASPECT,
        );
    }
}

/// Depths 27/33: lives dots. Both displays show lives − 1 (quirk Q4 /
/// correction C2): enemy dots extend right of (70.25, 48), player dots left
/// of (280, 48), survivors hugging the anchor.
pub fn draw_lives(app: &App, textures: &Textures) {
    let Some(world) = &app.world else { return };
    let enemy_dots = (world.enemy_lives - 1).clamp(0, 4);
    for i in 0..enemy_dots {
        draw_dot(
            &textures.enemy_dot,
            LIVES_DOT_SPACING.mul_add(i as f32, LIVES_ENEMY_ANCHOR.0),
            LIVES_ENEMY_ANCHOR.1,
        );
    }
    let player_dots = (world.player_lives - 1).clamp(0, 4);
    for i in 0..player_dots {
        draw_dot(
            &textures.player_dot,
            LIVES_DOT_SPACING.mul_add(-(i as f32), LIVES_PLAYER_ANCHOR.0),
            LIVES_PLAYER_ANCHOR.1,
        );
    }
}

fn draw_dot(texture: &Texture2D, cx: f32, cy: f32) {
    use curveball::consts::DOT_SIZE;

    draw_texture_ex(
        texture,
        cx - DOT_SIZE / 2.0,
        cy - DOT_SIZE / 2.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(DOT_SIZE, DOT_SIZE)),
            ..Default::default()
        },
    );
}

/// Depth 22: animated white bonus text per the §7.5 table.
pub fn draw_banner(app: &App, visuals: &Visuals) {
    let Some(banner) = &app.banner else { return };
    let phase = if app.visual_mode.smooths_cosmetics() {
        banner.tick as f32 + visuals.cosmetic_alpha
    } else {
        banner.tick as f32
    };
    let (rel_y, alpha) = banner_frame(phase);
    let color = rgba((0xff, 0xff, 0xff), f32::from(alpha) / 256.0);
    text::centered_tracked_aspect(
        banner.kind.text_upper(),
        BANNER_TEXT_CX,
        BANNER_TEXT_ANCHOR_Y + rel_y + BANNER_BASELINE_OFFSET,
        BANNER_DISPLAY_FONT_PX,
        color,
        BANNER_TRACKING,
        BANNER_TEXT_ASPECT,
    );
}

fn banner_frame(phase: f32) -> (f32, u16) {
    let phase = phase.clamp(0.0, BANNER_FRAMES.len().saturating_sub(1) as f32);
    let idx = phase.floor() as usize;
    let &(y, alpha) = &BANNER_FRAMES[idx];
    let Some(&(next_y, next_alpha)) = BANNER_FRAMES.get(idx + 1) else {
        return (y, alpha);
    };
    let t = phase - idx as f32;
    let y = (next_y - y).mul_add(t, y);
    let alpha = (f32::from(next_alpha) - f32::from(alpha)).mul_add(t, f32::from(alpha));
    (y, alpha.round() as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_frame_samples_original_keyframes_at_integer_phases() {
        for (tick, &(y, alpha)) in BANNER_FRAMES.iter().enumerate() {
            assert_eq!(banner_frame(tick as f32), (y, alpha));
        }
    }

    #[test]
    fn banner_frame_interpolates_between_keyframes() {
        let (y, alpha) = banner_frame(0.5);
        assert!((y - 2.075).abs() < f32::EPSILON);
        assert_eq!(alpha, 9);
    }
}
