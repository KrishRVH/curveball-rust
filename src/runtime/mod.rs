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
use curveball::sim::{Rect as SimRect, Vec3, Zone, classify, overlap, scale, vis};
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
    let mut last_tick_mouse: Option<(f64, f64)> = None;
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
        let frame_mouse = latch.mouse();
        let interpolation_start_mouse = *last_tick_mouse.get_or_insert(frame_mouse);
        let latch_elapsed = perf_elapsed(latch_start);

        let tick_dt = effective_tick_dt(&app, sim_dt_override);
        let frame_time = if canvas.is_some() {
            tick_dt
        } else {
            f64::from(get_frame_time()).min(0.25)
        };
        accumulator += frame_time;
        let pending_tick_debt = accumulator;
        let tick_start = perf_now(perf.as_ref());
        let mut ticks_this_frame = 0_u32;
        let ticks_due = pending_tick_count(accumulator, tick_dt);
        #[expect(
            clippy::while_float,
            reason = "fixed-timestep accumulator per PLAN.md §5.2"
        )]
        while accumulator >= effective_tick_dt(&app, sim_dt_override) {
            let tick_dt = effective_tick_dt(&app, sim_dt_override);
            let mut input = latch.drain();
            if let Some(mouse) = silky_catch_up_mouse(
                app.visual_mode,
                interpolation_start_mouse,
                frame_mouse,
                ticks_this_frame + 1,
                ticks_due,
            ) {
                input.mouse = mouse;
            }
            previous_visuals = render::Visuals::capture(&app);
            for sound in app.tick(&input) {
                audio.play(sound);
            }
            current_visuals = render::Visuals::capture(&app);
            last_tick_mouse = Some(input.mouse);
            accumulator -= tick_dt;
            ticks_this_frame += 1;
            #[cfg(debug_assertions)]
            {
                sim_tick_count += 1;
            }
        }
        let tick_elapsed = perf_elapsed(tick_start);

        let tick_dt = effective_tick_dt(&app, sim_dt_override);
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
                mode: app.visual_mode.label(),
                tick_dt,
                pending_tick_debt,
                residual_tick_debt: accumulator,
            })
        {
            perf.report();
            break;
        }
    }
}

fn effective_tick_dt(app: &App, sim_dt_override: Option<f64>) -> f64 {
    sim_dt_override.unwrap_or_else(|| app.tick_dt())
}

fn pending_tick_count(accumulator: f64, tick_dt: f64) -> u32 {
    if !(accumulator.is_finite() && tick_dt.is_finite()) || tick_dt <= 0.0 {
        return 0;
    }
    (accumulator / tick_dt).floor().max(0.0) as u32
}

fn silky_catch_up_mouse(
    mode: VisualMode,
    previous_mouse: (f64, f64),
    current_mouse: (f64, f64),
    tick_index: u32,
    ticks_due: u32,
) -> Option<(f64, f64)> {
    if mode != VisualMode::Silky || ticks_due <= 1 {
        return None;
    }
    let t = f64::from(tick_index.min(ticks_due)) / f64::from(ticks_due);
    Some((
        (current_mouse.0 - previous_mouse.0).mul_add(t, previous_mouse.0),
        (current_mouse.1 - previous_mouse.1).mul_add(t, previous_mouse.1),
    ))
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
    let crossing = silky_player_plane_crossing(ball);
    let current_contact = paddle_contact_zone(ball, crossing, world.paddle.pos, current_rect);
    let predicted_contact = paddle_contact_zone(ball, crossing, predicted_pos, predicted_rect);
    current_contact == predicted_contact
}

fn silky_player_contact_is_imminent(ball: &curveball::sim::Ball) -> bool {
    const CONTACT_GUARD_SLICES: f64 = 4.0;

    let dz_per_slice = -ball.vel.z * SILKY_DT_SCALE;
    dz_per_slice > 0.0 && ball.pos.z <= dz_per_slice * CONTACT_GUARD_SLICES
}

fn paddle_contact_zone(
    ball: &curveball::sim::Ball,
    crossing: Option<BallPlaneCrossing>,
    paddle_pos: (f64, f64),
    paddle_rect: SimRect,
) -> Option<Zone> {
    let hits = overlap(&ball.prev_rect, &paddle_rect)
        || crossing.is_some_and(|crossing| overlap(&crossing.rect, &paddle_rect));
    if !hits {
        return None;
    }
    let hit_pos = crossing.map_or(ball.pos, |crossing| crossing.pos);
    Some(classify(hit_pos.x, hit_pos.y, paddle_pos.0, paddle_pos.1))
}

#[derive(Debug, Clone, Copy)]
struct BallPlaneCrossing {
    pos: Vec3,
    rect: SimRect,
}

fn silky_player_plane_crossing(ball: &curveball::sim::Ball) -> Option<BallPlaneCrossing> {
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

fn player_plane_crossing_rect(start: Vec3, end: Vec3) -> Option<BallPlaneCrossing> {
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
    Some(BallPlaneCrossing {
        pos,
        rect: ball_rect_at(pos),
    })
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

    #[test]
    fn pending_tick_count_reports_due_fixed_steps() {
        assert_eq!(pending_tick_count(0.0, 0.1), 0);
        assert_eq!(pending_tick_count(0.099, 0.1), 0);
        assert_eq!(pending_tick_count(0.1, 0.1), 1);
        assert_eq!(pending_tick_count(0.35, 0.1), 3);
    }

    #[test]
    fn silky_catch_up_mouse_distributes_frame_delta_across_due_ticks() {
        assert_eq!(
            silky_catch_up_mouse(VisualMode::Silky, (0.0, 0.0), (30.0, 60.0), 1, 3),
            Some((10.0, 20.0))
        );
        assert_eq!(
            silky_catch_up_mouse(VisualMode::Silky, (0.0, 0.0), (30.0, 60.0), 2, 3),
            Some((20.0, 40.0))
        );
        assert_eq!(
            silky_catch_up_mouse(VisualMode::Silky, (0.0, 0.0), (30.0, 60.0), 3, 3),
            Some((30.0, 60.0))
        );
    }

    #[test]
    fn catch_up_mouse_keeps_faithful_latch_behavior() {
        assert_eq!(
            silky_catch_up_mouse(VisualMode::Faithful, (0.0, 0.0), (30.0, 60.0), 1, 3),
            None
        );
        assert_eq!(
            silky_catch_up_mouse(VisualMode::Silky, (0.0, 0.0), (30.0, 60.0), 1, 1),
            None
        );
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

    #[test]
    fn silky_live_visuals_hold_paddle_when_imminent_prediction_would_change_zone() {
        let (mut app, current) = app_with_ball(|ball| {
            ball.just_spawned = false;
            ball.pos.x = WORLD_CX + 7.0;
            ball.pos.z = 0.1;
            ball.vel.z = -2.0;
            ball.prev_rect = Rect::centered((WORLD_CX + 7.0, WORLD_CY), 30.0, 30.0);
        });
        app.visual_mode = VisualMode::Silky;

        let visuals = render::Visuals::capture(&app);
        let pos = live_visuals(&app, visuals, (current.0 + 250.0, current.1), 1.0).player_pos;

        assert_eq!(pos, Some(current));
    }
}
