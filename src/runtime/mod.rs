//! Macroquad runtime shell: input, timing, windowing, audio, and the frame loop.

mod audio;
mod config;
mod debug;
mod input;
mod perf;

use curveball::app::{App, VisualMode};
use curveball::consts::{BALL_DIAMETER, PADDLE_H, PADDLE_W, SILKY_DT_SCALE, STAGE_H, STAGE_W};
#[cfg(debug_assertions)]
use curveball::consts::{RENDER_SCALE, WORLD_CX, WORLD_CY};
use curveball::sim::{Rect as SimRect, Vec3, overlap, scale, vis};
use macroquad::prelude::*;

use self::config::{letterbox, letterbox_viewport};
#[cfg(debug_assertions)]
use self::debug::{debug_shot, debug_warp, fixed_mouse_from_env};
use self::input::InputLatch;
use self::perf::{FrameSample, PerfProbe, perf_elapsed, perf_now, sim_dt_override_from_env};
use crate::render;

pub use self::config::window_conf;

pub async fn run() {
    let audio = audio::Audio::load();
    render::install_display_font();
    let textures = render::Textures::bake();

    let mut app = App::new();
    let mut previous_visuals = render::Visuals::capture(&app);
    let mut current_visuals = previous_visuals;
    let mut latch = InputLatch::default();
    let mut accumulator = 0.0_f64;
    let sim_dt_override = sim_dt_override_from_env();
    let mut perf = PerfProbe::from_env();

    #[cfg(debug_assertions)]
    let (shot, fixed_mouse) = {
        let shot = debug_shot();
        let fixed_mouse = fixed_mouse_from_env();
        if let Ok(state) = std::env::var("CURVEBALL_WARP") {
            debug_warp(
                &mut app,
                &state,
                fixed_mouse.unwrap_or((WORLD_CX, WORLD_CY)),
            );
        }
        (shot, fixed_mouse)
    };
    #[cfg(debug_assertions)]
    let canvas = shot.as_ref().map(|_| {
        let canvas = render_target(STAGE_W as u32 * RENDER_SCALE, STAGE_H as u32 * RENDER_SCALE);
        canvas.texture.set_filter(FilterMode::Linear);
        canvas
    });
    #[cfg(not(debug_assertions))]
    let fixed_mouse: Option<(f64, f64)> = None;
    #[cfg(not(debug_assertions))]
    let canvas: Option<RenderTarget> = None;
    #[cfg(debug_assertions)]
    let mut sim_tick_count = 0_u64;

    loop {
        let frame_start = perf_now(perf.as_ref());
        let latch_start = perf_now(perf.as_ref());
        latch.latch(fixed_mouse);
        let latch_elapsed = perf_elapsed(latch_start);

        let tick_dt = sim_dt_override.unwrap_or_else(|| app.tick_dt());
        let frame_time = if canvas.is_some() {
            tick_dt
        } else {
            f64::from(get_frame_time()).min(0.25)
        };
        accumulator += frame_time;
        let tick_start = perf_now(perf.as_ref());
        let mut ticks_this_frame = 0_u32;
        #[expect(
            clippy::while_float,
            reason = "fixed-timestep accumulator per PLAN.md §5.2"
        )]
        while accumulator >= sim_dt_override.unwrap_or_else(|| app.tick_dt()) {
            let tick_dt = sim_dt_override.unwrap_or_else(|| app.tick_dt());
            let input = latch.drain();
            previous_visuals = render::Visuals::capture(&app);
            for sound in app.tick(&input) {
                audio.play(sound);
            }
            current_visuals = render::Visuals::capture(&app);
            accumulator -= tick_dt;
            ticks_this_frame += 1;
            #[cfg(debug_assertions)]
            {
                sim_tick_count += 1;
            }
        }
        let tick_elapsed = perf_elapsed(tick_start);

        let tick_dt = sim_dt_override.unwrap_or_else(|| app.tick_dt());
        let alpha = if canvas.is_some() {
            1.0
        } else {
            (accumulator / tick_dt) as f32
        };
        let visuals = render::Visuals::between(previous_visuals, current_visuals, alpha);
        let (scale, off_x, off_y) = letterbox();
        let scene_start = perf_now(perf.as_ref());
        let blit_elapsed = if let Some(canvas) = &canvas {
            draw_to_capture_target(canvas, &app, &textures, &visuals);
            let blit_start = perf_now(perf.as_ref());
            draw_capture_to_window(canvas, scale, off_x, off_y);
            perf_elapsed(blit_start)
        } else {
            let render_mouse = if app.visual_mode == VisualMode::Silky {
                InputLatch::sample_mouse(fixed_mouse)
            } else {
                latch.mouse()
            };
            let visuals = live_visuals(&app, visuals, render_mouse, alpha);
            draw_to_window(scale, off_x, off_y, &app, &textures, &visuals);
            std::time::Duration::ZERO
        };
        let scene_elapsed = perf_elapsed(scene_start).saturating_sub(blit_elapsed);

        #[cfg(debug_assertions)]
        {
            if let Some(shot) = &shot
                && sim_tick_count >= shot.tick
            {
                if let Some(canvas) = &canvas {
                    canvas.texture.get_texture_data().export_png(&shot.path);
                }
                break;
            }
        }

        let wait_start = perf_now(perf.as_ref());
        next_frame().await;
        let wait_elapsed = perf_elapsed(wait_start);
        if let Some(perf) = &mut perf
            && perf.record(FrameSample {
                frame: perf_elapsed(frame_start),
                latch: latch_elapsed,
                tick: tick_elapsed,
                scene: scene_elapsed,
                blit: blit_elapsed,
                wait: wait_elapsed,
                ticks_this_frame,
            })
        {
            perf.report();
            break;
        }
    }
}

fn draw_to_capture_target(
    canvas: &RenderTarget,
    app: &App,
    textures: &render::Textures,
    visuals: &render::Visuals,
) {
    let mut camera = capture_stage_camera();
    camera.render_target = Some(canvas.clone());
    camera.viewport = None;
    set_camera(&camera);
    render::draw_scene(app, textures, visuals);
}

fn draw_capture_to_window(canvas: &RenderTarget, scale: f32, off_x: f32, off_y: f32) {
    set_default_camera();
    clear_background(BLACK);
    draw_texture_ex(
        &canvas.texture,
        off_x,
        off_y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(STAGE_W as f32 * scale, STAGE_H as f32 * scale)),
            // Render-target textures store the scene bottom-up.
            flip_y: true,
            ..Default::default()
        },
    );
}

fn draw_to_window(
    scale: f32,
    off_x: f32,
    off_y: f32,
    app: &App,
    textures: &render::Textures,
    visuals: &render::Visuals,
) {
    let mut camera = window_stage_camera();
    camera.render_target = None;
    camera.viewport = Some(letterbox_viewport(scale, off_x, off_y));
    set_default_camera();
    clear_background(BLACK);
    set_camera(&camera);
    render::draw_scene(app, textures, visuals);
    set_default_camera();
}

fn live_visuals(
    app: &App,
    visuals: render::Visuals,
    mouse: (f64, f64),
    alpha: f32,
) -> render::Visuals {
    let Some(world) = &app.world else {
        return visuals;
    };
    let alpha = f64::from(alpha.clamp(0.0, 1.0));
    let pos = match app.visual_mode {
        VisualMode::Faithful => faithful_live_player_pos(world, mouse, alpha),
        VisualMode::Silky => silky_live_player_pos(world, mouse, alpha),
    };
    pos.map_or(visuals, |pos| visuals.with_player_pos(Some(pos)))
}

fn faithful_live_player_pos(
    world: &curveball::sim::World,
    mouse: (f64, f64),
    alpha: f64,
) -> Option<(f64, f64)> {
    if !faithful_player_prediction_allowed(world) {
        return None;
    }
    let current = world.paddle.pos;
    let next = world.paddle.predicted_pos(mouse);
    Some((
        (next.0 - current.0).mul_add(alpha, current.0),
        (next.1 - current.1).mul_add(alpha, current.1),
    ))
}

fn silky_live_player_pos(
    world: &curveball::sim::World,
    mouse: (f64, f64),
    alpha: f64,
) -> Option<(f64, f64)> {
    let pos = world.paddle.predicted_pos_scaled(
        mouse,
        alpha * f64::from(VisualMode::Silky.flash_frame_scale()),
    );
    if silky_player_prediction_allowed(world, pos) {
        Some(pos)
    } else {
        None
    }
}

fn faithful_player_prediction_allowed(world: &curveball::sim::World) -> bool {
    world
        .ball
        .as_ref()
        .is_none_or(|ball| !ball.stopped && ball.vel.z > 0.0)
}

fn silky_player_prediction_allowed(
    world: &curveball::sim::World,
    predicted_pos: (f64, f64),
) -> bool {
    let Some(ball) = &world.ball else {
        return true;
    };
    if ball.stopped || ball.vel.z == 0.0 {
        return false;
    }
    if ball.vel.z > 0.0 || !silky_player_contact_is_imminent(ball) {
        return true;
    }

    let current_rect = SimRect::centered(world.paddle.pos, PADDLE_W, PADDLE_H);
    let predicted_rect = SimRect::centered(predicted_pos, PADDLE_W, PADDLE_H);
    let swept_rect = silky_player_plane_rect(ball);
    let current_hit = paddle_contact_matches(ball.prev_rect, swept_rect, current_rect);
    let predicted_hit = paddle_contact_matches(ball.prev_rect, swept_rect, predicted_rect);
    current_hit == predicted_hit
}

fn silky_player_contact_is_imminent(ball: &curveball::sim::Ball) -> bool {
    const CONTACT_GUARD_SLICES: f64 = 4.0;

    let dz_per_slice = -ball.vel.z * SILKY_DT_SCALE;
    dz_per_slice > 0.0 && ball.pos.z <= dz_per_slice * CONTACT_GUARD_SLICES
}

fn paddle_contact_matches(
    previous_ball_rect: SimRect,
    swept_ball_rect: Option<SimRect>,
    paddle_rect: SimRect,
) -> bool {
    overlap(&previous_ball_rect, &paddle_rect)
        || swept_ball_rect.is_some_and(|swept| overlap(&swept, &paddle_rect))
}

fn silky_player_plane_rect(ball: &curveball::sim::Ball) -> Option<SimRect> {
    let start = ball.pos;
    let end = Vec3 {
        x: ball
            .curve
            .0
            .mul_add(SILKY_DT_SCALE, ball.vel.x)
            .mul_add(SILKY_DT_SCALE, ball.pos.x),
        y: ball
            .curve
            .1
            .mul_add(SILKY_DT_SCALE, ball.vel.y)
            .mul_add(-SILKY_DT_SCALE, ball.pos.y),
        z: ball.vel.z.mul_add(SILKY_DT_SCALE, ball.pos.z),
    };
    player_plane_crossing_rect(start, end)
}

fn player_plane_crossing_rect(start: Vec3, end: Vec3) -> Option<SimRect> {
    let dz = end.z - start.z;
    if dz == 0.0 || !(end.z < 0.0 && 0.0 <= start.z) {
        return None;
    }
    let t = -start.z / dz;
    let pos = Vec3 {
        x: (end.x - start.x).mul_add(t, start.x),
        y: (end.y - start.y).mul_add(t, start.y),
        z: 0.0,
    };
    Some(ball_rect_at(pos))
}

fn ball_rect_at(pos: Vec3) -> SimRect {
    let s = scale(pos.z);
    let (vx, vy) = vis(pos.x, pos.y, pos.z);
    SimRect::centered((vx, vy), BALL_DIAMETER * s, BALL_DIAMETER * s)
}

fn window_stage_camera() -> Camera2D {
    Camera2D::from_display_rect(Rect::new(
        0.0,
        STAGE_H as f32,
        STAGE_W as f32,
        -(STAGE_H as f32),
    ))
}

fn capture_stage_camera() -> Camera2D {
    Camera2D::from_display_rect(Rect::new(0.0, 0.0, STAGE_W as f32, STAGE_H as f32))
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "test setup unwraps scenario invariants")]

    use super::*;
    use curveball::consts::{WORLD_CX, WORLD_CY};
    use curveball::sim::{Ball, Published, Rect, SimInput, World};

    #[test]
    fn window_stage_camera_uses_y_down_stage_coordinates() {
        let camera = window_stage_camera();
        let top = camera.matrix().transform_point3(vec3(0.0, 0.0, 0.0)).y;
        let bottom = camera
            .matrix()
            .transform_point3(vec3(0.0, STAGE_H as f32, 0.0))
            .y;

        assert!(camera.zoom.y.is_sign_positive());
        assert!(top > bottom);
    }

    #[test]
    fn capture_stage_camera_preserves_render_target_orientation() {
        let camera = capture_stage_camera();

        assert!(camera.zoom.y.is_sign_negative());
    }

    fn settled_world() -> World {
        let mut world = World::new(Published::default());
        world.tick(&SimInput {
            mouse: (WORLD_CX, WORLD_CY),
            serve_clicks: 0,
        });
        world
    }

    fn app_with_world(world: World) -> App {
        let mut app = App::new();
        app.world = Some(world);
        app
    }

    fn app_with_ball(configure: impl FnOnce(&mut Ball)) -> (App, (f64, f64)) {
        let mut world = settled_world();
        world.spawn_ball();
        configure(world.ball.as_mut().expect("ball"));
        let current = world.paddle.pos;
        (app_with_world(world), current)
    }

    fn live_player_pos(app: &App, current: (f64, f64)) -> Option<(f64, f64)> {
        let visuals = render::Visuals::capture(app);
        live_visuals(app, visuals, (current.0 + 15.0, current.1), 1.0).player_pos
    }

    #[test]
    fn live_visuals_keep_player_paddle_synced_when_ball_can_hit_player() {
        let (app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.vel.z = -1.0;
        });

        assert_eq!(live_player_pos(&app, current), Some(current));
    }

    #[test]
    fn live_visuals_keep_player_paddle_synced_when_ball_is_stopped() {
        let (app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.stopped = true;
            ball.vel.z = 0.0;
        });

        assert_eq!(live_player_pos(&app, current), Some(current));
    }

    #[test]
    fn live_visuals_predict_player_paddle_when_no_ball_exists() {
        let world = settled_world();
        let current = world.paddle.pos;
        let app = app_with_world(world);

        assert_eq!(
            live_player_pos(&app, current),
            Some((current.0 + 10.0, current.1))
        );
    }

    #[test]
    fn live_visuals_predict_player_paddle_when_ball_moves_away() {
        let (app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.vel.z = 1.0;
        });

        assert_eq!(
            live_player_pos(&app, current),
            Some((current.0 + 10.0, current.1))
        );
    }

    #[test]
    fn silky_live_visuals_predict_incoming_paddle_when_contact_is_not_imminent() {
        let (mut app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.pos.z = 10.0;
            ball.vel.z = -2.0;
        });
        app.visual_mode = VisualMode::Silky;
        let mouse = (current.0 + 15.0, current.1);
        let expected = app
            .world
            .as_ref()
            .expect("world")
            .paddle
            .predicted_pos_scaled(mouse, f64::from(VisualMode::Silky.flash_frame_scale()));

        let visuals = render::Visuals::capture(&app);
        let pos = live_visuals(&app, visuals, mouse, 1.0).player_pos;

        assert_eq!(pos, Some(expected));
    }

    #[test]
    fn silky_live_visuals_hold_paddle_when_imminent_prediction_would_change_contact() {
        let (mut app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.pos.x = WORLD_CX + 44.0;
            ball.pos.z = 0.1;
            ball.vel.z = -2.0;
            ball.prev_rect = Rect::centered((WORLD_CX + 44.0, WORLD_CY), 30.0, 30.0);
        });
        app.visual_mode = VisualMode::Silky;

        let visuals = render::Visuals::capture(&app);
        let pos = live_visuals(&app, visuals, (-100.0, current.1), 1.0).player_pos;

        assert_eq!(pos, Some(current));
    }

    #[test]
    fn silky_live_visuals_hold_paddle_when_swept_contact_would_change_result() {
        let (mut app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.pos.x = WORLD_CX + 50.0;
            ball.pos.z = 0.1;
            ball.vel.x = -200.0;
            ball.vel.z = -2.0;
            ball.prev_rect = Rect::centered((WORLD_CX + 50.0, WORLD_CY), 30.0, 30.0);
        });
        app.visual_mode = VisualMode::Silky;

        let visuals = render::Visuals::capture(&app);
        let pos = live_visuals(&app, visuals, (-100.0, current.1), 1.0).player_pos;

        assert_eq!(pos, Some(current));
    }
}
