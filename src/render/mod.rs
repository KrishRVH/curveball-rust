//! Depth-ordered scene rendering in 350×250 logical stage coordinates.
//!
//! Flash stage depths (back → front): 1 bounds border, 11 depth ring,
//! 13 enemy paddle, 20 ball/pop, 22 bonus banner, 26–42 HUD text + lives,
//! 43 player paddle (over the ball and HUD!), 50 splash text, then the
//! Game Over text (43, after the paddle is gone) and overlays at 44.

pub mod anim;
pub mod entities;
pub mod hud;
pub mod menus;

use curveball::app::{App, Phase};
use curveball::consts::{SPLASH_BASELINE, SPLASH_CX, SPLASH_FONT_PX, SPLASH_TEXT_TICKS};
use curveball::sim::Rect as SimRect;
use macroquad::prelude::*;

pub use entities::Textures;

const FPS_COUNTER_X: f32 = 5.0;
const FPS_COUNTER_BASELINE: f32 = 12.0;
const FPS_COUNTER_FONT_PX: u16 = 7;
const FPS_COUNTER_TRACKING: f32 = 0.4;
const FPS_COUNTER_ASPECT: f32 = 0.72;

/// Cosmetic render snapshots interpolated between fixed simulation ticks. In
/// Faithful mode the sim state remains frame-accurate at 30 Hz; Silky mode
/// feeds this with non-faithful 400 Hz app/world ticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct Visuals {
    pub(crate) player_pos: Option<(f64, f64)>,
    pub(crate) enemy_pos: Option<(f64, f64)>,
    pub(crate) ball_rect: Option<SimRect>,
    pub(crate) ring_z: Option<f64>,
    pub(crate) cosmetic_alpha: f32,
}

impl Visuals {
    #[must_use]
    pub fn capture(app: &App) -> Self {
        let Some(world) = &app.world else {
            return Self::default();
        };
        Self {
            player_pos: Some(world.paddle.pos),
            enemy_pos: world.enemy.as_ref().map(|enemy| enemy.pos),
            ball_rect: world.ball.as_ref().map(|ball| ball.prev_rect),
            ring_z: world.ball.as_ref().map(|_| world.ring_z),
            cosmetic_alpha: 0.0,
        }
    }

    #[must_use]
    pub fn between(previous: Self, current: Self, alpha: f32) -> Self {
        let alpha = f64::from(alpha.clamp(0.0, 1.0));
        Self {
            player_pos: blend_point(previous.player_pos, current.player_pos, alpha),
            enemy_pos: blend_point(previous.enemy_pos, current.enemy_pos, alpha),
            ball_rect: blend_rect(previous.ball_rect, current.ball_rect, alpha),
            ring_z: blend_scalar(previous.ring_z, current.ring_z, alpha),
            cosmetic_alpha: alpha as f32,
        }
    }

    #[must_use]
    pub fn with_player_pos(mut self, pos: Option<(f64, f64)>) -> Self {
        self.player_pos = pos;
        self
    }
}

fn blend_point(
    previous: Option<(f64, f64)>,
    current: Option<(f64, f64)>,
    alpha: f64,
) -> Option<(f64, f64)> {
    match (previous, current) {
        (Some(a), Some(b)) => Some((lerp(a.0, b.0, alpha), lerp(a.1, b.1, alpha))),
        (_, Some(b)) => Some(b),
        _ => None,
    }
}

fn blend_rect(previous: Option<SimRect>, current: Option<SimRect>, alpha: f64) -> Option<SimRect> {
    match (previous, current) {
        (Some(a), Some(b)) => Some(SimRect {
            cx: lerp(a.cx, b.cx, alpha),
            cy: lerp(a.cy, b.cy, alpha),
            w: lerp(a.w, b.w, alpha),
            h: lerp(a.h, b.h, alpha),
        }),
        (_, Some(b)) => Some(b),
        _ => None,
    }
}

fn blend_scalar(previous: Option<f64>, current: Option<f64>, alpha: f64) -> Option<f64> {
    match (previous, current) {
        (Some(a), Some(b)) => Some(lerp(a, b, alpha)),
        (_, Some(b)) => Some(b),
        _ => None,
    }
}

fn lerp(a: f64, b: f64, alpha: f64) -> f64 {
    (b - a).mul_add(alpha, a)
}

/// Install the bundled Bank-Gothic-style substitute as macroquad's default
/// text face. If loading fails, macroquad's built-in font remains active.
pub fn install_display_font() {
    match load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/Michroma-Regular.ttf")) {
        Ok(font) => set_default_font(font),
        Err(err) => eprintln!("curveball: failed to load bundled display font: {err}"),
    }
}

/// Text helpers shared by the render modules. Flash anchors static text at
/// the glyph-run baseline; centered blocks anchor on the ink-span center.
pub mod text {
    use curveball::consts::RENDER_SCALE;
    use macroquad::prelude::*;
    use std::fmt::{self, Write};

    const DEFAULT_TEXT_ASPECT: f32 = 1.08;

    pub struct TextBuf<const N: usize> {
        bytes: [u8; N],
        len: usize,
    }

    impl<const N: usize> TextBuf<N> {
        #[must_use]
        pub const fn new() -> Self {
            Self {
                bytes: [0; N],
                len: 0,
            }
        }

        #[must_use]
        pub fn as_str(&self) -> &str {
            std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
        }
    }

    impl<const N: usize> Write for TextBuf<N> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let available = N.saturating_sub(self.len);
            let copied = available.min(s.len());
            let end = self.len + copied;
            self.bytes[self.len..end].copy_from_slice(&s.as_bytes()[..copied]);
            self.len = end;
            if copied < s.len() {
                return Err(fmt::Error);
            }
            Ok(())
        }
    }

    #[must_use]
    pub fn text_buf<const N: usize>(args: fmt::Arguments<'_>) -> TextBuf<N> {
        let mut buf = TextBuf::new();
        let _ = buf.write_fmt(args);
        buf
    }

    fn raster_font_size(font_px: u16) -> u16 {
        font_px.saturating_mul(RENDER_SCALE as u16)
    }

    fn params_with_aspect(font_px: u16, color: Color, aspect: f32) -> TextParams<'static> {
        TextParams {
            font_size: raster_font_size(font_px),
            font_scale: 1.0 / RENDER_SCALE as f32,
            font_scale_aspect: aspect,
            color,
            ..Default::default()
        }
    }

    pub fn centered(s: &str, cx: f32, baseline: f32, font_px: u16, color: Color) {
        centered_tracked_aspect(s, cx, baseline, font_px, color, 0.0, DEFAULT_TEXT_ASPECT);
    }

    pub fn left_tracked_aspect(
        s: &str,
        x: f32,
        baseline: f32,
        font_px: u16,
        color: Color,
        tracking: f32,
        aspect: f32,
    ) {
        let font_size = raster_font_size(font_px);
        let font_scale = 1.0 / RENDER_SCALE as f32;
        let mut cursor = x;
        for ch in s.chars() {
            let mut bytes = [0; 4];
            let glyph = ch.encode_utf8(&mut bytes);
            draw_text_ex(
                &*glyph,
                cursor,
                baseline,
                params_with_aspect(font_px, color, aspect),
            );
            let dims = measure_text(glyph, None, font_size, font_scale);
            cursor += dims.width.mul_add(aspect, tracking);
        }
    }

    pub fn centered_tracked_aspect(
        s: &str,
        cx: f32,
        baseline: f32,
        font_px: u16,
        color: Color,
        tracking: f32,
        aspect: f32,
    ) {
        let width = tracked_text_width(s, font_px, tracking, aspect);
        left_tracked_aspect(
            s,
            cx - width / 2.0,
            baseline,
            font_px,
            color,
            tracking,
            aspect,
        );
    }

    fn tracked_text_width(s: &str, font_px: u16, tracking: f32, aspect: f32) -> f32 {
        let font_size = raster_font_size(font_px);
        let font_scale = 1.0 / RENDER_SCALE as f32;
        let mut width = 0.0;
        let mut chars = s.chars().peekable();
        while let Some(ch) = chars.next() {
            let mut bytes = [0; 4];
            let glyph = ch.encode_utf8(&mut bytes);
            let dims = measure_text(glyph, None, font_size, font_scale);
            width = dims.width.mul_add(aspect, width);
            if chars.peek().is_some() {
                width += tracking;
            }
        }
        width
    }
}

/// Draw the full scene for the current phase, in stage-depth order.
pub fn draw_scene(app: &App, textures: &Textures, visuals: &Visuals, show_fps: bool) {
    clear_background(BLACK);
    // Depth 1: the bounds border is placed at frame 1 and never removed —
    // it is visible on every screen, menus included.
    entities::draw_border();

    match app.phase {
        Phase::Title => {
            entities::draw_tunnel_grid();
            menus::draw_title(app);
        },
        Phase::HighScores => {
            entities::draw_tunnel_grid();
            menus::draw_high_scores(app);
        },
        Phase::StartGameInit { .. } => {}, // frames 36–44: border only
        Phase::LevelSplash { tick } => {
            // Splash: no HUD, no entities except the live player paddle; the
            // "Level N" text shows for the first 45 ticks. Tick 0 is the
            // StartGame init tick (frame 44) — the paddle and text are only
            // placed at frame 45 (tick 1), so it still renders bare.
            if tick >= 1 {
                entities::draw_player(app, textures, visuals);
                if tick <= SPLASH_TEXT_TICKS
                    && let Some(world) = &app.world
                {
                    let label = text::text_buf::<16>(format_args!("Level {}", world.level));
                    text::centered(
                        label.as_str(),
                        SPLASH_CX,
                        SPLASH_BASELINE,
                        SPLASH_FONT_PX,
                        WHITE,
                    );
                }
            }
        },
        Phase::Playing { .. } | Phase::Miss { .. } => {
            let show_pop = matches!(app.phase, Phase::Miss { tick } if tick >= 1);
            draw_gameplay(app, textures, visuals, show_pop);
        },
        Phase::GameOver { .. } | Phase::NameEntry | Phase::End => {
            // Paddles/ball/ring are gone; banner bar and HUD persist;
            // "Game Over" sits at the paddle's old depth 43.
            hud::draw_banner(app, visuals);
            hud::draw_text_hud(app);
            hud::draw_lives(app, textures);
            menus::draw_game_over_text();
            match app.phase {
                Phase::NameEntry => menus::draw_name_entry(app),
                Phase::End => menus::draw_end(),
                _ => {},
            }
        },
    }

    if show_fps {
        draw_fps_counter();
    }
}

fn draw_gameplay(app: &App, textures: &Textures, visuals: &Visuals, show_pop: bool) {
    if let Some(world) = &app.world {
        // D7 tunnel lattice sits with the depth-11 visual layer; the ring
        // still exists only alongside the ball (placed together).
        if world.ball.is_some() {
            entities::draw_tunnel_grid();
            entities::draw_ring(visuals.ring_z.unwrap_or(world.ring_z));
        }
        // Depth 13.
        entities::draw_enemy(app, textures, visuals);
        // Depth 20.
        entities::draw_ball(app, textures, visuals, show_pop);
    }
    // Depth 22.
    hud::draw_banner(app, visuals);
    // Original HUD depths 26–42 plus D15 Zen tool pills.
    hud::draw_text_hud(app);
    hud::draw_lives(app, textures);
    hud::draw_zen_tools(app);
    // Depth 43: the player paddle draws over the ball and the HUD.
    entities::draw_player(app, textures, visuals);
}

fn draw_fps_counter() {
    let label = text::text_buf::<16>(format_args!("FPS: {}", get_fps()));
    text::left_tracked_aspect(
        label.as_str(),
        FPS_COUNTER_X,
        FPS_COUNTER_BASELINE,
        FPS_COUNTER_FONT_PX,
        WHITE,
        FPS_COUNTER_TRACKING,
        FPS_COUNTER_ASPECT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visuals_interpolate_existing_entities_and_snap_spawns() {
        let previous = Visuals {
            player_pos: Some((10.0, 20.0)),
            enemy_pos: None,
            ball_rect: Some(SimRect {
                cx: 0.0,
                cy: 10.0,
                w: 20.0,
                h: 30.0,
            }),
            ring_z: Some(0.0),
            cosmetic_alpha: 0.0,
        };
        let current = Visuals {
            player_pos: Some((30.0, 60.0)),
            enemy_pos: Some((100.0, 120.0)),
            ball_rect: Some(SimRect {
                cx: 20.0,
                cy: 30.0,
                w: 30.0,
                h: 40.0,
            }),
            ring_z: Some(10.0),
            cosmetic_alpha: 0.0,
        };

        let blended = Visuals::between(previous, current, 0.25);

        assert_eq!(blended.player_pos, Some((15.0, 30.0)));
        assert_eq!(blended.enemy_pos, Some((100.0, 120.0)));
        assert_eq!(blended.ring_z, Some(2.5));
        assert!((blended.cosmetic_alpha - 0.25).abs() < f32::EPSILON);
        assert_eq!(
            blended.ball_rect,
            Some(SimRect {
                cx: 5.0,
                cy: 15.0,
                w: 22.5,
                h: 32.5,
            })
        );
    }
}
