//! Thin macroquad launcher; runtime details live under `runtime/`.

mod render;
mod runtime;

fn window_conf() -> macroquad::prelude::Conf {
    runtime::window_conf()
}

#[macroquad::main(window_conf)]
async fn main() {
    runtime::run().await;
}
