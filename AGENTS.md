# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust 2024 port of Curveball. Core game logic lives in `src/lib.rs` and
the headless simulation modules under `src/sim/`. The binary entry point is `src/main.rs`;
it delegates the `macroquad` window, fixed-step loop, input latching, audio, debug capture, and
performance probes to `src/runtime/`. UI and draw code is grouped in `src/render/`, while
high-score storage is in `src/highscores.rs`.

Integration tests are in `tests/`: `unit.rs` covers simulation and app behavior, and `gold1.rs`
checks the normative GOLD-1 trajectory. Runtime sound assets are in `assets/sounds/`, and the
bundled OFL display-font substitute is in `assets/fonts/`. Original/reference material lives under
`reference/`: `reference/decompiled/` contains the decompiled AS1 and SWF, while `reference/kit/`
contains parser/extraction helpers, tag dumps, and extracted source sounds. Avoid changing
`reference/` unless updating provenance or fixtures.

## Build, Test, and Development Commands

- `cargo run` launches the game with the rodio backend compiled; Linux/WSL hosts without a detected
  PulseAudio, PipeWire-backed ALSA, or direct ALSA route auto-run silent unless `CURVEBALL_AUDIO=1`.
- `cargo run --no-default-features --features runtime` launches the playable silent fallback for
  headless or broken-audio hosts.
- `cargo test --no-default-features` verifies the headless library without compiling the runtime.
- `cargo build` compiles the debug binary with overflow checks enabled.
- `cargo test` runs all unit and integration tests.
- `cargo test --test gold1` runs the frame-accuracy trajectory test only.
- `cargo fmt --check` verifies formatting from `rustfmt.toml`.
- `cargo clippy --all-targets --all-features -- -D warnings` runs the configured lint baseline.
- `CURVEBALL_WARP=rally CURVEBALL_MOUSE=222.75,114 CURVEBALL_SHOT=/tmp/curveball.png:45 cargo run`
  captures deterministic debug screenshots for parity checks.
- `CURVEBALL_WARP=rally CURVEBALL_MOUSE=222.75,114 CURVEBALL_PERF=300 cargo run` prints frame
  timing percentiles, per-frame tick pacing, and accumulator debt for render-performance checks.
- `CURVEBALL_SIM_HZ=144 cargo run` runs the experimental non-faithful sim/timeline-rate override
  (`240` and `400` are also useful probes); omit it for the default faithful 30 Hz sim.

Use Rust `1.96.0` or newer, matching `Cargo.toml`.

## Coding Style & Naming Conventions

Use four-space indentation, Unix newlines, and a 100-column width. Let `rustfmt` reorder imports
and modules. Keep simulation code deterministic and headless; rendering, input, and audio should
stay outside `src/sim/`. Prefer descriptive `snake_case` functions and modules, `PascalCase`
types, and `SCREAMING_SNAKE_CASE` constants. `unsafe` is forbidden. Avoid `unwrap`, `expect`,
`todo`, and `dbg!` outside tests or carefully justified debug paths.

## Testing Guidelines

Add tests for every gameplay or timing change. Put broad behavior checks in `tests/unit.rs` and
trajectory/parity checks in dedicated integration tests when useful. Preserve exact IEEE behavior
where tests assert bit patterns or frame timings. Test names should describe the invariant, for
example `serve_requires_paddle_overlap`.

## Commit & Pull Request Guidelines

Use short, direct commit messages such as `Fix serve collision timing`. Pull requests should describe
gameplay impact, list commands run, and call out any intentional deviations from `PLAN.md` or
`DEVIATIONS.md`. Include screenshots or short recordings for visible rendering, menu, or
audio-control changes.

## Architecture Notes

Treat `PLAN.md` as the implementation contract and `DEVIATIONS.md` as the record of intentional
differences. When matching Flash behavior, prefer adding focused tests before changing constants,
phase timing, scoring, or ball physics.
