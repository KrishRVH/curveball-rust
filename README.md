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
- Main menu buttons: `START GAME`, `ZEN`, `HIGH SCORES`, and `VISUAL: FAITHFUL` / `VISUAL: SILKY`.
- Zen games show in-game `SILKY` and `AIMBOT` toggles. Aimbot mirrors the level-11 CPU on the player
  paddle and aims far-corner edge swipes on incoming returns. When enabled, aimbot auto-serves on
  eligible waiting-ball gameplay ticks; the toggle click itself is consumed so it does not also serve.

High scores are stored as `highscores.txt` under the user's data directory by default:
`%APPDATA%\curveball\` on Windows, `~/Library/Application Support/curveball/` on macOS, and
`${XDG_DATA_HOME:-~/.local/share}/curveball/` on Linux. Set `CURVEBALL_HIGHSCORES=/path/to/file`
to override the location.

## Audio

Default builds include sound effects through `rodio`. Macroquad stays graphics-only, which avoids
`quad-snd` audio-thread panics on hosts without a usable ALSA/PipeWire route. On Linux and WSL, the
runtime probes for PulseAudio, PipeWire-backed ALSA, or a direct ALSA card; if no route is detected,
the game runs silent instead of crashing. When audio is enabled, the runtime decodes the five
extracted SWF clips once at startup and starts a fresh overlapping source per trigger, so hit sounds
do not pay decode work at contact time.

Useful options:

```bash
CURVEBALL_AUDIO=0 cargo run          # force silent runtime mode
CURVEBALL_AUDIO=1 cargo run          # force an audio attempt even if probing finds no route
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
cargo test --no-default-features
cargo test --no-default-features --features runtime
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets --no-default-features -- -D warnings
cargo clippy --all-targets --no-default-features --features runtime -- -D warnings
cargo deny check advisories
```

`deny.toml` intentionally ignores `RUSTSEC-2025-0035` for `macroquad` because RustSec lists no
patched version. Treat that as a tracked release risk until the runtime is migrated, forked, or
upstream ships a fix.

The GitHub Actions workflow is manual-only (`workflow_dispatch`) to avoid automatic CI spend.

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
| `CURVEBALL_SHOT=path.png[:ticks]` | Debug-only deterministic 4x PNG capture after a simulation tick count; the live FPS overlay is suppressed. |
| `CURVEBALL_PERF=<frames>` | Print frame-time averages, p95/p99/max timing, mode, per-frame tick pacing, and accumulator debt over N rendered frames, then exit. |
| `CURVEBALL_SIM_HZ=<hz>` | Experimental non-faithful app/world cadence override, useful for feel-testing alternate rates. |

Example:

```bash
CURVEBALL_WARP=rally \
CURVEBALL_MOUSE=222.75,114 \
CURVEBALL_SHOT=/tmp/curveball-rally.png:45 \
cargo run
```

By default, Faithful gameplay state advances at the original 30 Hz. Rendering is not capped to
30 FPS; macroquad renders each display frame, interpolates autonomous visuals between fixed
simulation snapshots, and renders the live player paddle toward the latest mouse sample only when no
player-side hit can occur. During serve, pop, and incoming-player-contact windows, the paddle stays
synced to the fixed-step simulation so visible hits and paddle sounds land together. `VISUAL: SILKY`
runs a non-faithful 400 Hz app/world tick for input consumption, ball and paddle motion, enemy
tracking, collisions, sounds, and menu handling while scaling Flash-frame counters, score-bonus
drain, caret blink, and keyframe animations to preserve their original wall-clock speed. Silky also
late-samples the mouse for render-only paddle prediction, distributes mouse movement across multiple
catch-up ticks in one rendered frame, suppresses near-plane prediction when it would change the
visible hit/miss result or awarded hit zone, classifies Silky paddle hits at the swept plane-crossing
point, and performs swept ball/paddle checks inside 400 Hz slices. A small FPS counter is visible at
the top left during normal window rendering; `CURVEBALL_SHOT` suppresses it for deterministic
capture PNGs. In Zen mode, the in-game Silky toggle changes this mode without returning to the
title, and the aimbot toggle hands the player paddle to a level-11 CPU controller that
pseudo-randomly winds up on diagonal angles before swiping through incoming balls for mixed x/y spin.
`CURVEBALL_PERF=<frames>` additionally reports frame-time percentiles, per-frame tick pacing, and
accumulator debt.
