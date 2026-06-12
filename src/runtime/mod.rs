//! Macroquad runtime shell: input, timing, windowing, audio, and the frame loop.

mod audio;
mod config;
mod debug;
mod input;
mod perf;

use curveball::app::App;
use curveball::consts::{RENDER_SCALE, STAGE_H, STAGE_W, WORLD_CX, WORLD_CY};
use macroquad::prelude::*;

use self::config::{letterbox, letterbox_viewport};
#[cfg(debug_assertions)]
use self::debug::{debug_shot, debug_warp, fixed_mouse_from_env};
use self::input::InputLatch;
use self::perf::{PerfProbe, perf_elapsed, perf_now, sim_dt_from_env};
use crate::render;

pub use self::config::window_conf;

pub async fn run() {
    let audio = audio::Audio::load();
    render::install_display_font();
    let textures = render::Textures::bake();
    let mut camera =
        Camera2D::from_display_rect(Rect::new(0.0, 0.0, STAGE_W as f32, STAGE_H as f32));

    let mut app = App::new();
    let mut previous_visuals = render::Visuals::capture(&app);
    let mut current_visuals = previous_visuals;
    let mut latch = InputLatch::default();
    let mut accumulator = 0.0_f64;
    let sim_dt = sim_dt_from_env();
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

        let frame_time = if canvas.is_some() {
            sim_dt
        } else {
            f64::from(get_frame_time()).min(0.25)
        };
        accumulator += frame_time;
        let tick_start = perf_now(perf.as_ref());
        #[expect(
            clippy::while_float,
            reason = "fixed-timestep accumulator per PLAN.md §5.2"
        )]
        while accumulator >= sim_dt {
            let input = latch.drain();
            previous_visuals = render::Visuals::capture(&app);
            for sound in app.tick(&input) {
                audio.play(sound);
            }
            current_visuals = render::Visuals::capture(&app);
            accumulator -= sim_dt;
            #[cfg(debug_assertions)]
            {
                sim_tick_count += 1;
            }
        }
        let tick_elapsed = perf_elapsed(tick_start);

        let alpha = if canvas.is_some() {
            1.0
        } else {
            (accumulator / sim_dt) as f32
        };
        let visuals = render::Visuals::between(previous_visuals, current_visuals, alpha);
        let (scale, off_x, off_y) = letterbox();
        let scene_start = perf_now(perf.as_ref());
        let blit_elapsed = if let Some(canvas) = &canvas {
            draw_to_capture_target(&mut camera, canvas, &app, &textures, &visuals);
            let blit_start = perf_now(perf.as_ref());
            draw_capture_to_window(canvas, scale, off_x, off_y);
            perf_elapsed(blit_start)
        } else {
            draw_to_window(&mut camera, scale, off_x, off_y, &app, &textures, &visuals);
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
            && perf.record(
                perf_elapsed(frame_start),
                latch_elapsed,
                tick_elapsed,
                scene_elapsed,
                blit_elapsed,
                wait_elapsed,
            )
        {
            perf.report();
            break;
        }
    }
}

fn draw_to_capture_target(
    camera: &mut Camera2D,
    canvas: &RenderTarget,
    app: &App,
    textures: &render::Textures,
    visuals: &render::Visuals,
) {
    camera.render_target = Some(canvas.clone());
    camera.viewport = None;
    set_camera(camera);
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
    camera: &mut Camera2D,
    scale: f32,
    off_x: f32,
    off_y: f32,
    app: &App,
    textures: &render::Textures,
    visuals: &render::Visuals,
) {
    camera.render_target = None;
    camera.viewport = Some(letterbox_viewport(scale, off_x, off_y));
    set_default_camera();
    clear_background(BLACK);
    set_camera(camera);
    render::draw_scene(app, textures, visuals);
    set_default_camera();
}
