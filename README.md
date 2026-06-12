# Curveball Rust

An idiomatic Rust 2024 + macroquad rewrite of the original Flash Curveball.

The goal is not to preserve Flash implementation habits. The game keeps the original 30 Hz gameplay
math, timeline timings, scoring, AI, hit quirks, sounds, and visual feel, while the Rust version uses a
headless deterministic simulation, a macroquad runtime shell, native-scale rendering, high-refresh
visual interpolation, local high scores, and a small Zen mode extension.

## What Is Here

- `src/sim/`: pure `f64`, std-only gameplay simulation. This is the parity core.
- `src/app.rs`: timeline phase machine, sounds/events, banners, name entry, high-score routing.
- `src/runtime/`: macroquad window config, fixed-step loop, input latching, audio, debug capture, perf probes.
- `src/render/`: macroquad drawing, text, baked textures, HUD, menus, animation tables.
- `assets/`: runtime sounds and the bundled OFL Michroma display font.
- `reference/`: original SWF/decompiled source and extraction/reference tooling.
- `tests/`: unit coverage and the GOLD-1 trajectory test.

More detail: [docs/architecture.md](docs/architecture.md), [PLAN.md](PLAN.md), and
[DEVIATIONS.md](DEVIATIONS.md).

## Run

```bash
cargo run
```

Controls:

- Move the mouse to move the near paddle.
- Click to serve when the ball is waiting.
- Type and backspace on the high-score name screen.
- Main menu buttons: `START GAME`, `ZEN`, and `HIGH SCORES`.

High scores are stored as `highscores.txt` beside the executable, for example
`target/debug/highscores.txt` in debug builds.

## Audio

Default builds include sound effects through `rodio`. Macroquad stays graphics-only, which avoids
`quad-snd` audio-thread panics on hosts without a usable ALSA/PipeWire route. If no output device is
detected, the game auto-detects that and runs silent instead of crashing. WSL is conservative by
default: it starts silent unless you explicitly force an audio probe.

Useful options:

```bash
CURVEBALL_AUDIO=0 cargo run          # force silent runtime mode
CURVEBALL_AUDIO=1 cargo run          # force audio attempt, including on WSL
cargo run --no-default-features --features runtime  # run the no-audio backend
cargo test --no-default-features                    # test the headless library only
```

On WSL2 with WSLg, these packages and a Pulse-backed ALSA default are usually enough:

```bash
sudo apt-get update
sudo apt-get install -y alsa-utils libasound2-plugins pipewire-alsa pulseaudio-utils
cat > ~/.asoundrc <<'EOF'
pcm.!default {
    type pulse
    server "unix:/mnt/wslg/PulseServer"
}
ctl.!default {
    type pulse
    server "unix:/mnt/wslg/PulseServer"
}
EOF
```

## Development

Use Rust `1.96.0` or newer.

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets --no-default-features -- -D warnings
```

The release profile is optimized for the desktop game:

```bash
cargo build --release
```

## Debug And Perf Tools

These are intended for parity work and frame-pacing checks.

| Env var | Meaning |
|---|---|
| `CURVEBALL_WARP=<state>` | Debug-only warp to `highscores`, `splash`, `serve`, `rally`, `miss`, or game-over routing. |
| `CURVEBALL_MOUSE=x,y` | Debug-only fixed virtual-stage mouse coordinate for deterministic captures. |
| `CURVEBALL_SHOT=path.png[:ticks]` | Debug-only deterministic 4x PNG capture after a simulation tick count. |
| `CURVEBALL_PERF=<frames>` | Print average frame timing over N rendered frames, then exit. |
| `CURVEBALL_SIM_HZ=<hz>` | Experimental non-faithful sim/timeline cadence override, useful for feel-testing 144/240/400 Hz. |

Example:

```bash
CURVEBALL_WARP=rally \
CURVEBALL_MOUSE=222.75,114 \
CURVEBALL_SHOT=/tmp/curveball-rally.png:45 \
cargo run
```

By default, gameplay state advances at the original 30 Hz. Rendering is not capped to 30 FPS;
macroquad renders each display frame, interpolates autonomous visuals between fixed simulation
snapshots, and renders the live player paddle toward the latest mouse sample without changing
collision math. A small FPS counter is always visible at the top left.
