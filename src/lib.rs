//! Curveball — a frame-accurate Rust port of `curveball.swf` (Flash 5).
//!
//! This library half is pure std (no macroquad): the simulation core, the
//! timeline phase machine, constants, and the local high-score table. The
//! binary half owns rendering, audio, and the fixed-step loop. Keeping the
//! sim headless makes the GOLD-1 trajectory and every quirk unit-testable.

pub mod app;
pub mod consts;
pub mod highscores;
pub mod sim;
