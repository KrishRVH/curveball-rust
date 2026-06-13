//! Entity rendering: bounds border, depth ring, paddles with pips, ball/pop.
//!
//! The radial-gradient shapes (ball, pop, lives dots) are baked into small
//! textures at startup from the exact gradient definitions in the tag stream
//! (off-center highlight included), then linearly filtered from the native-scale
//! render target.

use curveball::app::{App, PipFlash};
use curveball::consts::{
    BALL_DIAMETER, BALL_GRAD_CENTER, BALL_GRAD_INNER_STOP, BALL_GRAD_RADIUS, BORDER_RECT,
    COLOR_BALL_RIM, COLOR_BLUE, COLOR_OUTER_FRAME, COLOR_RED, COLOR_TUNNEL, DOT_GRAD_CENTER,
    DOT_GRAD_RADIUS, DOT_SIZE, PADDLE_H, PADDLE_W, PIP_CENTER_SIZE, PIP_CORNER_OFFSET,
    PIP_CORNER_SIZE, RENDER_SCALE, RING_OFFSET, RING_SIZE, WORLD_BOTTOM, WORLD_DEPTH, WORLD_LEFT,
    WORLD_RIGHT, WORLD_TOP,
};
use curveball::sim::{Zone, scale, vis};
use macroquad::prelude::*;
use std::sync::LazyLock;

use super::Visuals;
use super::anim::{C_PIP_ADD_RED, C_PIP_MULT, HIT_PIP_ALPHA, OTHER_PIP_ALPHA};

const TUNNEL_DEPTHS: [f64; 9] = [0.0, 15.0, 25.0, 35.0, 45.0, 55.0, 65.0, 72.0, 75.0];
type LineSegment = ((f32, f32), (f32, f32));

static TUNNEL_CORNERS: LazyLock<[[(f32, f32); 4]; TUNNEL_DEPTHS.len()]> =
    LazyLock::new(|| TUNNEL_DEPTHS.map(projected_corners));
static TUNNEL_SEGMENTS: LazyLock<Vec<LineSegment>> = LazyLock::new(|| {
    let mut segments = Vec::with_capacity((TUNNEL_DEPTHS.len() - 1) * 8);
    let mut previous = TUNNEL_CORNERS[0];
    for corners in &TUNNEL_CORNERS[1..] {
        for i in 0..4 {
            segments.push((corners[i], corners[(i + 1) % 4]));
            segments.push((previous[i], corners[i]));
        }
        previous = *corners;
    }
    segments
});

pub fn rgb(c: (u8, u8, u8)) -> Color {
    Color::from_rgba(c.0, c.1, c.2, 255)
}

pub fn rgba(c: (u8, u8, u8), alpha: f32) -> Color {
    Color::new(
        f32::from(c.0) / 255.0,
        f32::from(c.1) / 255.0,
        f32::from(c.2) / 255.0,
        alpha,
    )
}

fn half_rgb(c: (u8, u8, u8)) -> Color {
    Color::from_rgba(c.0 / 2, c.1 / 2, c.2 / 2, 255)
}

/// Bake a Flash radial gradient into a circle texture of diameter `d` px.
/// `center`/`radius` are the gradient matrix translation and scaled radius;
/// `inner_stop` is the white-core ratio. 4×4 supersampled, transparent
/// outside the shape circle.
fn bake_radial(
    diameter: f32,
    center: (f32, f32),
    radius: f32,
    inner_stop: f32,
    rim: Color,
) -> Texture2D {
    let raster_scale = RENDER_SCALE as f32;
    let raster_diameter = (diameter * raster_scale).round() as u32;
    let mut image = Image::gen_image_color(
        raster_diameter as u16,
        raster_diameter as u16,
        Color::new(rim.r, rim.g, rim.b, 0.0),
    );
    let shape_radius = diameter / 2.0;
    for py in 0..raster_diameter {
        for px in 0..raster_diameter {
            let mut sum = [0.0_f32; 3];
            let mut covered = 0.0_f32;
            for sy in 0..4 {
                for sx in 0..4 {
                    // Sample position in shape-local coords (origin at center).
                    let sample_x =
                        (px as f32 + (sx as f32 + 0.5) / 4.0) / raster_scale - shape_radius;
                    let sample_y =
                        (py as f32 + (sy as f32 + 0.5) / 4.0) / raster_scale - shape_radius;
                    if sample_x.hypot(sample_y) > shape_radius {
                        continue;
                    }
                    let dist = (sample_x - center.0).hypot(sample_y - center.1);
                    let ratio = ((dist / radius - inner_stop) / (1.0 - inner_stop)).clamp(0.0, 1.0);
                    sum[0] += (rim.r - 1.0).mul_add(ratio, 1.0);
                    sum[1] += (rim.g - 1.0).mul_add(ratio, 1.0);
                    sum[2] += (rim.b - 1.0).mul_add(ratio, 1.0);
                    covered += 1.0;
                }
            }
            let alpha = covered / 16.0;
            if alpha > 0.0 {
                // Average the color over covered samples only; alpha softens
                // the rim like Flash's shape anti-aliasing.
                image.set_pixel(
                    px,
                    py,
                    Color::new(sum[0] / covered, sum[1] / covered, sum[2] / covered, alpha),
                );
            }
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

pub struct Textures {
    pub ball: Texture2D,
    pub pop: Texture2D,
    pub enemy_dot: Texture2D,
    pub player_dot: Texture2D,
    player_paddle: Texture2D,
    enemy_paddle: Texture2D,
}

impl Textures {
    pub fn bake() -> Self {
        let d = BALL_DIAMETER as f32;
        Self {
            ball: bake_radial(
                d,
                BALL_GRAD_CENTER,
                BALL_GRAD_RADIUS,
                BALL_GRAD_INNER_STOP,
                rgb(COLOR_BALL_RIM),
            ),
            pop: bake_radial(d, BALL_GRAD_CENTER, BALL_GRAD_RADIUS, 0.0, rgb(COLOR_RED)),
            enemy_dot: bake_radial(
                DOT_SIZE,
                DOT_GRAD_CENTER,
                DOT_GRAD_RADIUS,
                0.0,
                rgb(COLOR_RED),
            ),
            player_dot: bake_radial(
                DOT_SIZE,
                DOT_GRAD_CENTER,
                DOT_GRAD_RADIUS,
                0.0,
                rgb(COLOR_BLUE),
            ),
            player_paddle: bake_paddle_base(COLOR_BLUE),
            enemy_paddle: bake_paddle_base(COLOR_RED),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Premul {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Premul {
    fn over(&mut self, color: Color) {
        let keep = 1.0 - color.a;
        self.r = color.r.mul_add(color.a, self.r * keep);
        self.g = color.g.mul_add(color.a, self.g * keep);
        self.b = color.b.mul_add(color.a, self.b * keep);
        self.a = self.a.mul_add(keep, color.a);
    }

    fn add_sample(&mut self, sample: Self) {
        self.r += sample.r;
        self.g += sample.g;
        self.b += sample.b;
        self.a += sample.a;
    }

    fn averaged(self, samples: f32, transparent_rgb: Color) -> Color {
        let r = self.r / samples;
        let g = self.g / samples;
        let b = self.b / samples;
        let a = self.a / samples;
        if a <= f32::EPSILON {
            Color::new(transparent_rgb.r, transparent_rgb.g, transparent_rgb.b, 0.0)
        } else {
            Color::new(r / a, g / a, b / a, a)
        }
    }
}

fn bake_paddle_base(fill: (u8, u8, u8)) -> Texture2D {
    const SUPERSAMPLE: u32 = 3;

    let raster_scale = RENDER_SCALE as f32;
    let raster_w = (PADDLE_W as f32 * raster_scale).round() as u32;
    let raster_h = (PADDLE_H as f32 * raster_scale).round() as u32;
    let mut image = Image::gen_image_color(
        raster_w as u16,
        raster_h as u16,
        Color::new(0.0, 0.0, 0.0, 0.0),
    );
    let stroke = half_rgb(fill);
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;

    for py in 0..raster_h {
        for px in 0..raster_w {
            let mut pixel = Premul::default();
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let local_x =
                        (px as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32) / raster_scale;
                    let local_y =
                        (py as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32) / raster_scale;
                    let mut sample = Premul::default();
                    paint_paddle_sample(&mut sample, local_x, local_y, stroke);
                    pixel.add_sample(sample);
                }
            }
            image.set_pixel(px, py, pixel.averaged(samples, stroke));
        }
    }

    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

fn paint_paddle_sample(sample: &mut Premul, x: f32, y: f32, stroke: Color) {
    let w = PADDLE_W as f32;
    let h = PADDLE_H as f32;
    let border = 2.0;
    let fill = Color::new(0.50, 0.50, 0.50, 0.70);
    paint_rounded_fill(sample, x, y, 0.0, 0.0, w, h, 8.0, fill);
    paint_rounded_outline(sample, x, y, 0.0, 0.0, w, h, 8.0, border, stroke);

    let cx = w / 2.0;
    let cy = h / 2.0;
    let guide = 0.5;
    paint_rect(
        sample,
        x,
        y,
        cx - guide / 2.0,
        border,
        guide,
        h - border * 2.0,
        stroke,
    );
    paint_rect(
        sample,
        x,
        y,
        border,
        cy - guide / 2.0,
        w - border * 2.0,
        guide,
        stroke,
    );

    let (cw, ch) = PIP_CENTER_SIZE;
    paint_rounded_fill(
        sample,
        x,
        y,
        cx - cw / 2.0,
        cy - ch / 2.0,
        cw,
        ch,
        3.0,
        fill,
    );
    paint_rounded_outline(
        sample,
        x,
        y,
        cx - cw / 2.0,
        cy - ch / 2.0,
        cw,
        ch,
        3.0,
        1.0,
        stroke,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "small raster painter mirrors x/y/w/h drawing APIs"
)]
fn paint_rounded_fill(
    sample: &mut Premul,
    sample_x: f32,
    sample_y: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: Color,
) {
    if contains_rounded_rect(sample_x, sample_y, x, y, w, h, radius) {
        sample.over(color);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "small raster painter mirrors x/y/w/h drawing APIs"
)]
fn paint_rounded_outline(
    sample: &mut Premul,
    sample_x: f32,
    sample_y: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    thickness: f32,
    color: Color,
) {
    if !contains_rounded_rect(sample_x, sample_y, x, y, w, h, radius) {
        return;
    }
    let inset = thickness.min(radius).max(0.0);
    if !contains_rounded_rect(
        sample_x,
        sample_y,
        x + inset,
        y + inset,
        w - inset * 2.0,
        h - inset * 2.0,
        (radius - inset).max(0.0),
    ) {
        sample.over(color);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "small raster painter mirrors x/y/w/h drawing APIs"
)]
fn paint_rect(
    sample: &mut Premul,
    sample_x: f32,
    sample_y: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Color,
) {
    if (x..=x + w).contains(&sample_x) && (y..=y + h).contains(&sample_y) {
        sample.over(color);
    }
}

fn contains_rounded_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, radius: f32) -> bool {
    if w <= 0.0 || h <= 0.0 {
        return false;
    }
    let right = x + w;
    let bottom = y + h;
    if px < x || right < px || py < y || bottom < py {
        return false;
    }
    let r = radius.min(w / 2.0).min(h / 2.0);
    let cx = if px < x + r {
        x + r
    } else if right - r < px {
        right - r
    } else {
        px
    };
    let cy = if py < y + r {
        y + r
    } else if bottom - r < py {
        bottom - r
    } else {
        py
    };
    let dx = px - cx;
    let dy = py - cy;
    dx.mul_add(dx, dy * dy) <= r * r
}

/// A 1 px rectangle outline drawn inside the given rect as four full-pixel
/// quads. (macroquad's `draw_rectangle_lines` uses half-thickness quads,
/// which vanish at half-pixel coordinates under the render-target y-flip.)
pub fn outline(x: f32, y: f32, w: f32, h: f32, color: Color) {
    draw_rectangle(x, y, w, 1.0, color);
    draw_rectangle(x, y + h - 1.0, w, 1.0, color);
    draw_rectangle(x, y + 1.0, 1.0, h - 2.0, color);
    draw_rectangle(x + w - 1.0, y + 1.0, 1.0, h - 2.0, color);
}

/// Depth 1: the static cyan outer frame.
pub fn draw_border() {
    let (x, y, w, h) = BORDER_RECT;
    outline(x, y, w, h, rgb(COLOR_OUTER_FRAME));
}

fn projected_corners(z: f64) -> [(f32, f32); 4] {
    let (left, top) = vis(WORLD_LEFT, WORLD_TOP, z);
    let (right, bottom) = vis(WORLD_RIGHT, WORLD_BOTTOM, z);
    [
        (left as f32, top as f32),
        (right as f32, top as f32),
        (right as f32, bottom as f32),
        (left as f32, bottom as f32),
    ]
}

/// Static tunnel lattice used by the richer desktop original: projected world
/// slices plus corner rails, all generated from the same perspective function
/// as the ball and collision ring.
pub fn draw_tunnel_grid() {
    let grid = rgb(COLOR_TUNNEL);
    for &(a, b) in TUNNEL_SEGMENTS.iter() {
        draw_line(a.0, a.1, b.0, b.1, 1.0, grid);
    }
}

/// Depth 11: the projected depth ring at the previous tick's published z.
/// `_alpha = 100 − z`, clamped to [0, 100] like Flash display alpha.
pub fn draw_ring(ring_z: f64) {
    let s = scale(ring_z) as f32;
    let (vx, vy) = vis(WORLD_LEFT, WORLD_TOP, ring_z);
    let alpha = ((100.0 - ring_z) / 100.0).clamp(0.0, 1.0) as f32;
    outline(
        RING_OFFSET.0.mul_add(s, vx as f32),
        RING_OFFSET.1.mul_add(s, vy as f32),
        RING_SIZE.0 * s,
        RING_SIZE.1 * s,
        rgba(COLOR_TUNNEL, alpha),
    );
}

fn sample_keyframe<const N: usize>(table: &[u16; N], phase: f32, tail: u16) -> f32 {
    let last = N - 1;
    let phase = phase.clamp(0.0, N as f32);
    if phase >= last as f32 {
        let t = (phase - last as f32).clamp(0.0, 1.0);
        return lerp(f32::from(table[last]), f32::from(tail), t);
    }
    let idx = phase.floor() as usize;
    let t = phase - idx as f32;
    lerp(f32::from(table[idx]), f32::from(table[idx + 1]), t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}

/// One flash overlay (or the C pip's red→white ramp on a center hit).
fn pip_color(zone: Zone, flash: PipFlash, phase: f32) -> Color {
    if flash.zone == zone {
        let alpha = sample_keyframe(&HIT_PIP_ALPHA, phase, 0) / 256.0;
        if zone == Zone::C {
            // rgb = (255·mult/256 + add_red, 255·mult/256, 255·mult/256)
            let mult = sample_keyframe(&C_PIP_MULT, phase, 256) / 256.0;
            let red = (mult + sample_keyframe(&C_PIP_ADD_RED, phase, 0) / 255.0).min(1.0);
            return Color::new(red, mult, mult, alpha);
        }
        return Color::new(1.0, 1.0, 1.0, alpha);
    }
    let alpha = sample_keyframe(&OTHER_PIP_ALPHA, phase, 0) / 256.0;
    Color::new(1.0, 1.0, 1.0, alpha)
}

/// A framed paddle with hit-flash overlays, scaled by `s` about the screen-space center
/// (s = 1 unprojected player, s = scale(75) for the enemy).
fn draw_paddle(
    center: (f32, f32),
    s: f32,
    texture: &Texture2D,
    flash: Option<&PipFlash>,
    flash_phase: f32,
    hit_outline: Option<Color>,
) {
    let (cx, cy) = center;
    let (w, h) = (PADDLE_W as f32 * s, PADDLE_H as f32 * s);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            ..Default::default()
        },
    );

    let (cw, ch) = (PIP_CENTER_SIZE.0 * s, PIP_CENTER_SIZE.1 * s);
    let corners = [
        (Zone::UR, PIP_CORNER_OFFSET.0, -PIP_CORNER_OFFSET.1),
        (Zone::UL, -PIP_CORNER_OFFSET.0, -PIP_CORNER_OFFSET.1),
        (Zone::BL, -PIP_CORNER_OFFSET.0, PIP_CORNER_OFFSET.1),
        (Zone::BR, PIP_CORNER_OFFSET.0, PIP_CORNER_OFFSET.1),
    ];
    if let Some(flash) = flash {
        for (zone, dx, dy) in corners {
            let (pw, ph) = (PIP_CORNER_SIZE.0 * s, PIP_CORNER_SIZE.1 * s);
            draw_rectangle(
                dx.mul_add(s, cx) - pw / 2.0,
                dy.mul_add(s, cy) - ph / 2.0,
                pw,
                ph,
                pip_color(zone, *flash, flash_phase),
            );
        }
        draw_rectangle(
            cx - cw / 2.0,
            cy - ch / 2.0,
            cw,
            ch,
            pip_color(Zone::C, *flash, flash_phase),
        );
        if let Some(color) = hit_outline {
            draw_hit_pip_outline(center, s, flash.zone, color);
        }
    }
}

fn draw_hit_pip_outline(center: (f32, f32), s: f32, zone: Zone, color: Color) {
    let (x, y, w, h) = pip_rect(center, s, zone);
    outline(x, y, w, h, color);
}

fn pip_rect(center: (f32, f32), s: f32, zone: Zone) -> (f32, f32, f32, f32) {
    let (cx, cy) = center;
    let (dx, dy, w, h) = match zone {
        Zone::UR => (
            PIP_CORNER_OFFSET.0,
            -PIP_CORNER_OFFSET.1,
            PIP_CORNER_SIZE.0,
            PIP_CORNER_SIZE.1,
        ),
        Zone::UL => (
            -PIP_CORNER_OFFSET.0,
            -PIP_CORNER_OFFSET.1,
            PIP_CORNER_SIZE.0,
            PIP_CORNER_SIZE.1,
        ),
        Zone::BL => (
            -PIP_CORNER_OFFSET.0,
            PIP_CORNER_OFFSET.1,
            PIP_CORNER_SIZE.0,
            PIP_CORNER_SIZE.1,
        ),
        Zone::BR => (
            PIP_CORNER_OFFSET.0,
            PIP_CORNER_OFFSET.1,
            PIP_CORNER_SIZE.0,
            PIP_CORNER_SIZE.1,
        ),
        Zone::C => (0.0, 0.0, PIP_CENTER_SIZE.0, PIP_CENTER_SIZE.1),
    };
    let (w, h) = (w * s, h * s);
    (
        dx.mul_add(s, cx) - w / 2.0,
        dy.mul_add(s, cy) - h / 2.0,
        w,
        h,
    )
}

/// Depth 13: the enemy paddle, projected at fixed z = 75.
pub fn draw_enemy(app: &App, textures: &Textures, visuals: &Visuals) {
    let Some(world) = &app.world else { return };
    let Some(enemy) = &world.enemy else { return };
    let pos = visuals.enemy_pos.unwrap_or(enemy.pos);
    let (vx, vy) = vis(pos.0, pos.1, WORLD_DEPTH);
    let s = scale(WORLD_DEPTH) as f32;
    let flash_phase = app
        .enemy_flash
        .map_or(0.0, |flash| flash_phase(app, visuals, flash));
    draw_paddle(
        (vx as f32, vy as f32),
        s,
        &textures.enemy_paddle,
        app.enemy_flash.as_ref(),
        flash_phase,
        None,
    );
}

/// Depth 43: the player paddle, unprojected on the z = 0 plane.
pub fn draw_player(app: &App, textures: &Textures, visuals: &Visuals) {
    let Some(world) = &app.world else { return };
    let pos = visuals.player_pos.unwrap_or(world.paddle.pos);
    draw_paddle(
        (pos.0 as f32, pos.1 as f32),
        1.0,
        &textures.player_paddle,
        app.player_flash.as_ref(),
        app.player_flash
            .map_or(0.0, |flash| flash_phase(app, visuals, flash)),
        Some(rgb(COLOR_BLUE)),
    );
}

fn flash_phase(app: &App, visuals: &Visuals, flash: PipFlash) -> f32 {
    let alpha = if app.visual_mode.smooths_cosmetics() {
        visuals.cosmetic_alpha
    } else {
        0.0
    };
    flash.tick as f32 + alpha
}

/// Depth 20: the ball (or the frozen pop during the Miss phase), drawn at its
/// last rendered rect — the same rect the collision system uses.
pub fn draw_ball(app: &App, textures: &Textures, visuals: &Visuals, show_pop: bool) {
    let Some(world) = &app.world else { return };
    let Some(ball) = &world.ball else { return };
    let rect = visuals.ball_rect.unwrap_or(ball.prev_rect);
    let texture = if show_pop {
        &textures.pop
    } else {
        &textures.ball
    };
    draw_texture_ex(
        texture,
        (rect.cx - rect.w / 2.0) as f32,
        (rect.cy - rect.h / 2.0) as f32,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(rect.w as f32, rect.h as f32)),
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use curveball::consts::{
        PIP_CENTER_SIZE, PIP_CORNER_OFFSET, PIP_CORNER_SIZE, WORLD_CX, WORLD_CY,
    };

    use super::*;

    #[test]
    fn pip_rects_follow_source_offsets() {
        let center = (WORLD_CX as f32, WORLD_CY as f32);

        assert_eq!(
            pip_rect(center, 1.0, Zone::C),
            (
                center.0 - PIP_CENTER_SIZE.0 / 2.0,
                center.1 - PIP_CENTER_SIZE.1 / 2.0,
                PIP_CENTER_SIZE.0,
                PIP_CENTER_SIZE.1,
            )
        );
        assert_eq!(
            pip_rect(center, 1.0, Zone::UR),
            (
                center.0 + PIP_CORNER_OFFSET.0 - PIP_CORNER_SIZE.0 / 2.0,
                center.1 - PIP_CORNER_OFFSET.1 - PIP_CORNER_SIZE.1 / 2.0,
                PIP_CORNER_SIZE.0,
                PIP_CORNER_SIZE.1,
            )
        );
        let s = 0.25;
        let pw = PIP_CORNER_SIZE.0 * s;
        let ph = PIP_CORNER_SIZE.1 * s;
        assert_eq!(
            pip_rect(center, s, Zone::BL),
            (
                PIP_CORNER_OFFSET.0.mul_add(-s, center.0) - pw / 2.0,
                PIP_CORNER_OFFSET.1.mul_add(s, center.1) - ph / 2.0,
                pw,
                ph,
            )
        );
    }
}
