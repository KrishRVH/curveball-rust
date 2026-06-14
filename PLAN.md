# PLAN — Curveball: faithful Rust 2024 rewrite

Target: a frame-accurate, systems-level Rust port of `curveball.swf` (Flash 5, GameLab-era original).
Every gameplay-relevant number and rule below was extracted from the decompiled ActionScript and the
SWF tag stream — not from memory of the game. Sections are tagged:

Status: implemented. Keep this file as the fidelity contract and provenance record for future
maintenance; intentional product/platform differences live in `DEVIATIONS.md`.

- **[VERIFIED]** — extracted directly from the SWF/decompiled source. Implement exactly.
- **[APPROX]** — visual detail not fully recoverable from tags (e.g. embedded font glyphs).
  Implemented to spec; final approximation notes live in `DEVIATIONS.md`.
- **[DEVIATION]** — deliberate, documented departure from the original. Do not add new ones without
  recording them in `DEVIATIONS.md`.

---

## 0. Historical build order

1. M0 — scaffold: configs below, fixed 30 Hz tick, 350×250 logical stage, native-scale presentation.
2. M1 — `sim/`: pure-`f64`, std-only simulation core. Must pass the GOLD-1 trajectory test (§14) exactly.
3. M2 — renderer: projection, draw order, entity visuals.
4. M3 — game flow: state machine with frame-accurate timings, scoring, lives, HUD.
5. M4 — animation tables (paddle flashes, bonus banner, pips) + audio.
6. M5 — menus, local high-score table, name entry.
7. M6 — side-by-side fidelity audit against the SWF/Ruffle reference and the desktop reference
   screenshot; close the deviations ledger.

---

## 1. Mission & fidelity contract

"Faithful" means, in priority order:

1. **Simulation**: identical per-tick arithmetic, in identical order, on `f64` (AS1 `Number` is an IEEE-754
   double). Identical update ordering between entities. Identical collision semantics (screen-space AABB
   of *previous-tick* ball rect vs *current-tick* paddle rect — see §4.6). Identical scoring economy.
2. **Timing**: the original is frame-locked at 30 fps. The port runs a fixed 30 Hz tick decoupled from
   render rate. All durations below are given in ticks, taken from the SWF timeline.
3. **Visuals**: exact stage geometry, colors, alphas, depths, and animation keyframe tables from the SWF.
4. **Quirks**: the original's oddities are features (§5.3). Preserve them unless listed in §16.

When the plan and the decompiled source ever appear to disagree, the source wins; record the discrepancy.

---

## 2. Ground truth & reference kit

Available to the implementing agent:

| Artifact | Path | Use |
|---|---|---|
| Decompiled AS1 source | `reference/decompiled/scripts/**` | Normative gameplay logic |
| Original SWF | `reference/decompiled/curveball.swf` | Visual/audio ground truth; run in Ruffle for the audit |
| Parsed tag dump | `reference/kit/tags.json` | Every placement matrix, cxform, shape, text field |
| SWF parser | `reference/kit/swf_parse.py` | Re-derive/extend `tags.json` if needed |
| Executable spec | `reference/kit/golden_sim.py` | Generates the GOLD-1 values in §14 |
| Sounds (extracted, converted) | `reference/kit/sounds/*.wav` | wallBounce1/2, pPaddleBounce, ePaddleBounce, missSound |

Key source files (paths under `reference/decompiled/scripts/`):

| File | Contents |
|---|---|
| `frame_44/DoAction.as` | World init, all constants, level tables, sound setup |
| `frame_90/DoAction.as` | Per-level setup, lives display, per-rally bonus reset |
| `frame_92/PlaceObject2_80_20/...enterFrame....as` | Ball: integration, walls, collisions, scoring, projection, bonus drain |
| `frame_92/PlaceObject2_80_20/...mouseDown....as` | Serve logic incl. minimum-curve injection and serve scoring |
| `frame_92/PlaceObject2_80_20/...load....as` | Ball init; local `curveDecay = 1.004` |
| `frame_91/PlaceObject2_75_13/...` | Enemy paddle AI + projection |
| `frame_45/PlaceObject2_59_43/...` | Player paddle (mouse easing, clamping, speed) |
| `DefineSprite_80/frame_20/DoAction.as` | End-of-rally routing (level up / game over / re-serve) |

---

## 3. Verified reference data

### 3.1 Runtime **[VERIFIED]**

| Item | Value |
|---|---|
| Stage | 350 × 250 px, background `#000000` |
| Frame rate | 30.0 fps (tick = 1/30 s) |
| Master volume | 80 % (`globalSound.setVolume(80)`) |
| RNG | Original/Faithful gameplay has none. D15 Zen aimbot uses a seeded deterministic xorshift for swipe-plan variety; no external `rand` crate. |

### 3.2 World geometry **[VERIFIED]**

The `bounds` clip (green outline, placed at (25, 25), content AABB 301 × 201 incl. 1 px stroke) defines:

```
world.left   = 25.0          world.right  = 25 + 301 = 326.0
world.top    = 25.0          world.bottom = 25 + 201 = 226.0
world.depth  = 75.0
center wx = 175.5, wy = 125.5      // note: NOT (175,125) — the stroke shifts it by 0.5
```

### 3.3 Projection **[VERIFIED]**

All depth rendering uses one scalar scale function (`varA = 31.066017 = 75 / tan(67.5°)`, chosen so
`s(75) = 0.25`):

```
s(z)      = (90 − atan(z / 31.066017) · 180/π) / 90
vis(p, z) = (wx − (wx − p.x)·s(z),  wy − (wy − p.y)·s(z))
size(z)   = natural_size · s(z)
```

Reference values (12 d.p.): `s(0) = 1.0`, `s(75) = 0.249999998710`, `s(76) = 0.247032735634`,
`s(−2) = 1.040928480087`. Note `z` may be slightly negative (ball overshoot) → scale slightly > 1.

### 3.4 Entities **[VERIFIED]**

| Entity | World size | Visual | Render rule |
|---|---|---|---|
| Ball | Ø 30 (radius = 15) | Radial gradient: `#ffffff` at ratio 33/255 → `#3fff11` at edge. No stroke. | Projected at its own z; width = height = 30·s(z) |
| Miss ball ("pop") | Ø 30 | Radial gradient `#ffffff` (center) → `#ff0000` (edge) | Replaces ball during Miss phase, frozen at last projected pos/scale |
| Player paddle | 60 × 40 | SWF base is `#0000ff` at alpha 128/256 with pips per §3.5; D7 supersedes the idle look with a rounded framed paddle | **Unprojected** (always z = 0 plane): drawn full-size at its world pos |
| Enemy paddle | 60 × 40 | SWF base is `#ff0000` at alpha 128/256 with pips per §3.5; D7 supersedes the idle look with a rounded framed paddle | Projected at fixed z = 75 → renders 15.0 × 10.0 at vis(pos, 75) |
| Depth ring | 301 × 201 rect outline | SWF stroke `#77fffc`; D7 desktop-reference presentation draws it green with the tunnel | Projected rect whose top-left is vis((25,25), z_ball_prev); alpha % = clamp(100 − z, 0, 100); ring z floors at 0 |
| Bounds border | 301 × 201 rect outline | SWF stroke `#00ff66`; D7 desktop-reference presentation draws the static outer frame cyan | Static at (25, 25), unprojected |
| Tunnel lattice | Projected bounds slices | D7 visual layer: repeated projected rectangles plus corner rails, green | Drawn before the moving depth ring during gameplay |

### 3.5 Paddle pips (both paddles) **[VERIFIED]**

The SWF uses five white overlays on the colored base rect, idle alpha 90/256 ≈ 35.2 %. The Rust
renderer keeps these dimensions and animation tables for hit flashes, but D7 replaces the always-idle
overlay look with a rounded framed paddle in normal play:

| Pip | Shape size | Center offset from paddle center |
|---|---|---|
| UR | 27.8 × 17.8 | (+14.1, −9.1) |
| UL | 27.8 × 17.8 | (−14.1, −9.1) |
| BL | 27.8 × 17.8 | (−14.1, +9.1) |
| BR | 27.8 × 17.8 | (+14.1, +9.1) |
| C  | 12 × 8     | (0, 0) |

### 3.6 Level tables **[VERIFIED]** (index = level − 1; exactly 10 entries)

```
levelSpeed       = [2, 2.33, 2.66, 3, 3.33, 3.66, 4, 4.33, 4.66, 6]
levelSkillFactor = [17, 14, 11, 9, 7, 5, 3.5, 2.75, 2, 1]
levelCurve       = [25, 22.5, 20, 17.5, 15, 12.5, 10, 10, 10, 10]   // curveAmount divisor
```

Constants: `curveDecay = 1.004` (ball-local; the `world.curveDecay = 0.01` in frame_44 is **dead code**
— never read by the ball). Wall curve damp = `(curveDecay − 1)·50 + 1 = 1.2`.

### 3.7 Scoring economy **[VERIFIED]**

| Variable | Init (per rally/level) | Award | Degrade per award | Floor |
|---|---|---|---|---|
| hitScore | 100 | every player-paddle return (not serve) | −10 | 0 |
| accuracyBonus | 100 | center-zone hit or center-zone serve | −10 | 0 |
| curveBonus | 50 | curve hit/serve (one axis) | −5 | 0 |
| superCurveBonus | 150 | curve hit/serve (both axes > 0.1) | −15 | 0 |
| bonusDisplay | 3000 (reset each level at Level setup) | added to score on level win | −25 per 11 in-flight ticks [CORRECTED — see DEVIATIONS.md C1] | 0 |

Bonus-drain mechanism: counter `bonus` starts at 10; every tick the ball is in flight (`vz != 0`) and
`bonusDisplay > 0`, decrement; when it reaches −1, reset to 10 and `bonusDisplay −= 25` (i.e. one −25
step every 11 flight ticks — 10 → −1 takes 11 decrements; 3000 drains in 1320 ticks = 44 s
[CORRECTED — see DEVIATIONS.md C1]). The reset-check sits *outside* the
flight gate (it runs every ball tick), but only flight decrements, so the counter never persists below
zero. The 11-tick counter itself is reset **only at level setup** — it carries across rallies within a
level (quirk Q11): a rally ending mid-cycle shortens the next rally's first drain step.

Resets: on **player miss** → hitScore/curveBonus/superCurveBonus/accuracyBonus back to 100/50/150/100
(bonusDisplay is *not* reset). On **enemy miss** → nothing resets. On **level setup** (Level splash) →
all five reset incl. bonusDisplay = 3000.

Lives: player 5, enemy 3 (enemy resets to 3 each level). Level up when enemyLives < 1
(then `score += bonusDisplay`); game over when playerLives < 1.

Curve-bonus classification (exact branch order, applied to the **just-assigned** curve values):

```
if |cx| > 0.1 { if |cy| > 0.1 → SUPER else → CURVE }
else if |cy| > 0.05 → CURVE
else if |cx| > 0.05 → CURVE
```

Center ("accuracy") zone: `pPos.x − 7 ≤ ball.x ≤ pPos.x + 7` AND `pPos.y − 5 ≤ ball.y ≤ pPos.y + 5`.

### 3.8 Sounds **[VERIFIED]**

11025 Hz mono, MP3-in-SWF, converted to WAV in the reference kit. Play at master volume 80 %.
The runtime embeds these extracted SWF clips directly.

| Linkage name | Trigger | Duration |
|---|---|---|
| `wallBounce1` | left/right wall bounce | 0.68 s |
| `wallBounce2` | top/bottom wall bounce | 0.73 s |
| `pPaddleBounce` | player return **and** serve | 0.84 s |
| `ePaddleBounce` | enemy return | 0.57 s |
| `missSound` | either side misses | 0.68 s |

---

## 4. Simulation spec (normative)

All quantities `f64`. One tick = one Flash frame. **Per-tick order is part of the spec** (it reproduces
Flash's clip-event order: userPaddle → enemyPaddle → ring → ball; mouse events before the tick):

```
tick(input):
  0. INPUT PHASE   — serve check (§4.5), using last tick's rects/snapshots
  1. PLAYER PADDLE — §4.2
  2. ENEMY PADDLE  — §4.3 (consumes ball state published at the END of the previous tick)
  3. RING          — cosmetic; z = max(ball.z_published_prev, 0)
  4. BALL          — §4.4 (consumes paddle state computed THIS tick)
  5. publish ball pos/dir; store ball rendered rect for next tick; bonus drain (§3.7) if vz != 0
```

### 4.1 State

```rust
struct Vec3 { x: f64, y: f64, z: f64 }
struct Paddle { pos: (f64, f64), speed: (f64, f64) }          // world coords
struct Ball   { pos: Vec3, vel: Vec3, curve: (f64, f64), stopped: bool }
struct Published { pos: Vec3, dir: Vec3 }                      // _parent.ballPos*/ballDir*
struct Rect { center: (f64, f64), w: f64, h: f64 }             // screen-space
```

`Published` persists across rallies (parent-timeline variables in Flash). A freshly spawned ball
publishes nothing until the end of its first tick; the enemy treats missing data as "ease to center"
(`dir.z = 0` after a miss guarantees this anyway — see §4.4 miss handling).

### 4.2 Player paddle **[VERIFIED]**

```
target = mouse (virtual coords; macroquad reports last known position when cursor leaves — matches Flash)
pos.x -= (pos.x - target.x) / 1.5
pos.y -= (pos.y - target.y) / 1.5
clamp pos.y to [top + 20, bottom - 20]      // half-height 20  → y ∈ [45, 206]
clamp pos.x to [left + 30, right - 30]      // half-width 30   → x ∈ [55, 296]
speed = pos - old_pos                        // post-clamp delta
```

Order quirk **[VERIFIED]**: y is clamped before x (irrelevant numerically, kept for symmetry with source).
On spawn (game start / new game) the paddle initializes to center (175.5, 125.5). The paddle instance
**persists** across rallies and across levels (it is never reinstantiated until Game Over removes it),
so its position carries over; it keeps tracking the mouse during the Level splash.

### 4.3 Enemy paddle **[VERIFIED]**

```
if published.dir.z > 0:   // ball moving away from player, toward enemy (previous tick's value)
    ease toward published.pos.(x,y) with divisor skillFactor (level table)
else:
    ease toward (wx, wy) with divisor 15
clamp identically to player (half-extents 30 / 20)
speed = post-clamp delta
```

Rendered projected at fixed z = 75. The enemy instance persists across rallies **within** a level
(keeps its drift position); it is reinstantiated (recentered, new skillFactor) only when passing
through the Level splash. After any miss the ball publishes `dir.z = 0`, so the enemy eases home.

### 4.4 Ball **[VERIFIED]** — exact per-tick algorithm

```
if stopped: return                          // entire body skipped during Miss phase, incl. bonus drain

read paddle snapshot  P  (player pos+speed, THIS tick)      // cache it: serve (§4.5) reads the cache
read enemy snapshot   E  (enemy pos+speed, THIS tick)

vel.x += curve.x
vel.y += curve.y
pos.z += vel.z
pos.x += vel.x
pos.y -= vel.y                              // NOTE: y SUBTRACTS vel.y

if curve.x != 0 { curve.x /= 1.004 }        // -0.0 != 0.0 is false in IEEE → skips, matching AS1
if curve.y != 0 { curve.y /= 1.004 }

r = 15.0
if pos.y - r < 25      { pos.y = 40;  curve.y /= 1.2; vel.y = -vel.y; play wallBounce2 }
else if 226 < pos.y + r{ pos.y = 211; curve.y /= 1.2; vel.y = -vel.y; play wallBounce2 }
if pos.x - r < 25      { pos.x = 40;  curve.x /= 1.2; vel.x = -vel.x; play wallBounce1 }
else if 326 < pos.x + r{ pos.x = 311; curve.x /= 1.2; vel.x = -vel.x; play wallBounce1 }

if 75 < pos.z {                             // enemy side
    if overlap(prev_ball_rect, enemy_rect_this_tick) {       // §4.6
        flash enemy pip per zone (§4.7) using (pos vs E.pos)
        pos.z = 75
        curve.x =  E.speed.x / curveAmount
        curve.y = -E.speed.y / curveAmount
        vel.z = -vel.z
        play ePaddleBounce                  // no score for enemy returns
    } else {
        vel = (0,0,0); curve = (0,0)
        enemyLives -= 1; play missSound; stopped = true; enter Miss phase (§9)
    }
} else if pos.z < 0 {                       // player side
    if overlap(prev_ball_rect, player_rect_this_tick) {
        zone = classify(pos vs P.pos)       // §4.7; flash player pip
        pos.z = 0
        curve.x = -P.speed.x / curveAmount
        curve.y =  P.speed.y / curveAmount
        vel.z = -vel.z
        score += hitScore; hitScore = max(0, hitScore - 10)
        if zone == C { score += accuracyBonus; accuracyBonus = max(0, accuracyBonus - 10); banner "Accuracy Bonus!" }
        apply curve-bonus classification (§3.7) → banner "Curve Bonus!" / "Super Curve Bonus!"
        play pPaddleBounce
    } else {
        vel = (0,0,0); curve = (0,0)
        playerLives -= 1
        hitScore=100; curveBonus=50; superCurveBonus=150; accuracyBonus=100
        play missSound; stopped = true; enter Miss phase
    }
}

project: vis_pos = vis(pos.xy, pos.z); rendered size = 30·s(pos.z)
store prev_ball_rect = Rect{vis_pos, size, size}            // used by NEXT tick's collisions & serve
publish pos/vel
bonus drain (§3.7)
```

Signs recap: x/y velocity is **never** reflected by paddles (only z); walls reflect x/y. Curve is fully
**replaced** (not added) on every paddle contact, and divided by 1.2 on every wall contact.

### 4.5 Serve **[VERIFIED]** (mouse click, any screen position)

Runs at tick start, only when `vel.z == 0` (covers Serve phase *and* the pop quirk, §5.3):

```
if click && overlap(prev_ball_rect, prev_player_rect):       // BOTH rects from previous tick
    cached = paddle snapshot from the last tick the ball ran     // previous tick normally; frozen at the miss tick during a pop (Q2)
    vel.z = levelSpeed[level-1]
    curve.x = -cached.speed.x / curveAmount
    curve.y =  cached.speed.y / curveAmount
    if |curve.x| < 0.01 { curve.x = if cached.pos.x < wx { 0.01 } else { -0.01 } }
    if |curve.y| < 0.01 { curve.y = if wy < cached.pos.y { 0.01 } else { -0.01 } }
    zone = classify(ball.pos vs cached.pos)
    if zone == C { score += accuracyBonus; degrade; banner }
    apply curve-bonus classification                          // NO hitScore on serve
    flash player pip per zone; play pPaddleBounce
```

A perfectly still paddle therefore always serves with curve = (±0.01, ±0.01) — the ball can never fly
truly straight off a serve, but **can** off a return (still paddle → curve = (−0.0, 0.0), decay skipped).

### 4.6 Collision = Flash `hitTest` **[VERIFIED]**

Screen-space AABB intersection. Crucial asymmetry: the ball's `_x/_width` are updated at the *end* of
its tick, but collisions are tested mid-tick — so the ball contributes its **previous tick's** rendered
rect, while paddles (updated earlier in the same tick) contribute their **current** rect.

```
ball rect   : center = vis(prev pos, prev z), w = h = 30·s(prev z)
player rect : 60 × 40 centered on paddle.pos (unprojected)
enemy rect  : 15.0... × 10.0... centered on vis(enemy.pos, 75)    // 60·s(75) × 40·s(75)
overlap     : |c1x−c2x|·2 ≤ w1+w2  &&  |c1y−c2y|·2 ≤ h1+h2
```

On the very first tick after a ball spawns, `prev_ball_rect` = its load-time rect: 30 × 30 at
(175.5, 125.5) — i.e. center, scale 1.

### 4.7 Hit-zone classification **[VERIFIED]** (drives pip flash + accuracy bonus)

Using paddle center `(px, py)` and ball world pos `(bx, by)` (post-clamp, this tick):

```
if px + 7 < bx            → UR if by < py else BR
else if bx < px − 7       → UL if by < py else BL
else /* |dx| ≤ 7 */:
    if py + 5 ≥ by:
        if by ≥ py − 5    → C            // accuracy zone
        else              → UR if bx ≥ px else UL
    else                  → BR if bx ≥ px else BL
```

---

## 5. Flash → Rust semantic mapping

### 5.1 Numerics
AS1 `Number` = IEEE-754 double → use `f64` for the entire sim. Convert to `f32` only at the macroquad
draw-call boundary. The sim contains only `+ − × ÷` and comparisons → bit-deterministic across
platforms. `atan` appears only in the projection (render + collision rects); assert goldens at 1e-9
relative tolerance, not bit-exact.

### 5.2 Timing model
Fixed-timestep accumulator at exactly 30 Hz; render every display frame. D10 interpolates
cosmetic render snapshots between fixed sim ticks. The live player paddle renders toward the
latest mouse sample only when no player-side hit can happen (no ball, or the ball is moving away
from the player); during serve, pop, and incoming-player-contact windows it stays on the sim
snapshot so visible hits and paddle sounds land on the same faithful 30 Hz tick:

```rust
const DT: f64 = 1.0 / 30.0;
let mut acc = 0.0_f64;
loop {
    acc += f64::from(get_frame_time()).min(0.25);
    while acc >= DT { game.tick(input.drain()); acc -= DT; }
    game.render();
    next_frame().await;
}
```

Input latching: record mouse position each display frame; latch `is_mouse_button_pressed` edges into a
queue so a click between ticks is never dropped and is consumed by exactly one tick.

Experimental timing override: `CURVEBALL_SIM_HZ=<hz>` overrides the selected mode's fixed-step
cadence for feel tests. This is intentionally non-faithful. Do not use it for parity captures or
tests.
The title-menu `VISUAL: FAITHFUL` / `VISUAL: SILKY` toggle (D14) keeps Faithful as the default. Silky
runs a 400 Hz app/world tick for input, motion, collision/event detection, sounds, and menu handling.
Flash-frame counters, score-bonus drain, caret blink, and SWF keyframe animations are scaled to keep
their original 30 Hz wall-clock speed, and cosmetic keyframes are sampled fractionally at draw time.
Silky late-samples the mouse immediately before rendering for paddle prediction, gates incoming-ball
prediction on near-contact hit/miss and hit-zone agreement, distributes mouse movement across
multiple catch-up ticks in one rendered frame, classifies Silky paddle hits at the swept
plane-crossing point, and adds swept plane contact checks inside the 400 Hz ball/paddle collision
slices. Faithful does not use those non-faithful contact or prediction paths.

### 5.3 Quirks ledger — preserve all of these **[VERIFIED]**

| # | Quirk | Behavior to reproduce |
|---|---|---|
| Q1 | Dead code | `world.curveDecay = 0.01` (ball uses local 1.004), `world.bounce = 1`, `world.lagFactor` (read into a local, never used), `m = 100` / `f = 0.8` in all four clips, `growshrink`, and the paddle's `_root._xmouse = wx` write (read-only property — a failed attempt to center the cursor at game start). Keep as doc comments only; implement none. |
| Q2 | Serve-during-pop | `mouseDown` is not gated on `stopped`. During the 19-tick Miss phase, `vel.z == 0`, so a click with the paddle overlapping the frozen pop awards serve scoring (accuracy/curve bonuses + pPaddleBounce + pip flash) once, sets `vel.z`, but the ball stays frozen and is discarded at routing. Free points, original behavior. Precision: the hit test uses the frozen pop rect vs the paddle's **live current** rect, while the zone/curve math uses the paddle snapshot **frozen at the miss tick** (the ball's cache stops updating when `stopped`). |
| Q3 | No straight serves | Minimum-curve injection (§4.5) applies only to serves, not returns. |
| Q4 | Lives pips show lives − 1 | See §7.6: 5 lives renders 4 pips. Applies to **both** displays [CORRECTED — see DEVIATIONS.md C2]. |
| Q5 | 1-tick lags | Enemy AI sees previous-tick ball; ring shows previous-tick z; collisions use previous-tick ball rect. These lags are gameplay-relevant — do not "fix". |
| Q6 | Paddle persistence | Player paddle never recenters between rallies/levels; enemy recenters only on level change. |
| Q7 | x/y momentum through paddles | Paddle contact reflects z only; lateral velocity continues, curve replaced. |
| Q8 | bonusDisplay survives player misses | Only level setup resets it. |
| Q9 | Enemy returns score nothing | No points, no degrade. |
| Q10 | −0.0 decay skip | `curve != 0` is false for −0.0 → a zero-curve return never decays (no-op anyway, but keep the guard shape). |
| Q11 | Drain counter persists | The 11-tick bonus-drain counter resets only at level setup, never between rallies (§3.7). |

### 5.4 Level 11 **[DEVIATION — default]**
Original arrays have 10 entries; AS1/SWF5 reads index 10 as `undefined`, which coerces to 0 in
numeric context. That makes level-11 speed/skill/curve parameters zero, so the serve sets
`vel.z = 0` and the ball never travels: a softlock after beating level 10. Default port behavior:
clamp the table index to 9 (level ≥ 10 reuses the last entry). Provide
`const STRICT_LEVEL_11_SOFTLOCK: bool = false`; when true, reproduce the zero-speed softlock.
Record in `DEVIATIONS.md`.

---

## 6. Architecture & dependency policy

### 6.1 Crate layout

```
curveball/
├── Cargo.toml  rustfmt.toml  clippy.toml          # §11, user-supplied baseline
├── assets/sounds/*.wav                            # 5 extracted SWF files from the reference kit
├── src/
│   ├── main.rs            # thin macroquad launcher
│   ├── app.rs             # top-level App: phase machine (§9) + transitions + persistence
│   ├── consts.rs          # every [VERIFIED] number in this plan, with source citations in doc comments
│   ├── runtime/
│   │   ├── mod.rs         # macroquad frame loop, fixed-step accumulator, capture/perf orchestration
│   │   ├── config.rs      # window config and virtual-stage letterboxing
│   │   ├── input.rs       # display-frame input latching into deterministic TickInput
│   │   ├── debug.rs       # CURVEBALL_WARP/MOUSE/SHOT debug helpers
│   │   ├── perf.rs        # CURVEBALL_PERF and CURVEBALL_SIM_HZ parsing/probes
│   │   └── audio.rs       # embedded-WAV rodio backend, silent no-op facade without audio
│   ├── sim/
│   │   ├── mod.rs         # Sim::tick(input) — orchestrates §4 order
│   │   ├── ball.rs  paddle.rs  enemy.rs
│   │   ├── score.rs       # economy of §3.7 (incl. bonus drain)
│   │   └── project.rs     # s(z), vis(), Rect, overlap()
│   ├── render/
│   │   ├── mod.rs         # depth-ordered draw (§7.1) in 350×250 stage coordinates
│   │   ├── entities.rs  hud.rs  menus.rs
│   │   └── anim.rs        # const keyframe tables (§7.4–7.6) + tick-indexed players
│   └── highscores.rs      # local table (§10), std-only line format
├── assets/fonts/Michroma-Regular.ttf              # D2 OFL Bank-Gothic substitute
└── tests/
    ├── gold1.rs           # §14 trajectory
    └── unit.rs            # projection, economy, drain cadence, zones, serve injection
```

`sim/` must not import macroquad — pure std, fully unit-testable. `render` and `runtime` own macroquad.

### 6.2 Dependencies **[POLICY]**
Runtime dependencies: `macroquad = "0.4"` for window/input/render and optional `rodio = "0.17"` for
audio output (patches pinned via lockfile). Macroquad stays graphics-only. Plain
`--no-default-features` is for headless library/test builds with no runtime dependencies; use
`--no-default-features --features runtime` for the playable silent macroquad runtime.
Rendering = `draw_rectangle{,_lines}` + radial-gradient dots/balls and framed paddle bases baked
into small `Texture2D`s at startup from `Image`s, then drawn directly into a native-scale
letterboxed viewport. Debug screenshot captures use the same 4× offscreen target as the audit
workflow. Audio = `runtime::audio` backed by rodio when enabled, with a no-op backend when
`runtime` is enabled without `audio`.
Text = `draw_text` / `TextParams` with bundled Michroma; D2 records the font deviation.
No serde, no external `rand`, no ECS — the entity count is 4. Original/Faithful gameplay has no
RNG; D15 Zen aimbot uses a seeded deterministic xorshift for swipe-plan variety.

### 6.3 Window & scaling
Logical stage 350 × 250 rendered directly to a native window viewport at
`scale = max(1, floor(min(win_w/350, win_h/250)))`, centered, black letterbox. Default window
1400 × 1000. `CURVEBALL_SHOT` captures render to a 4× native `render_target` with
`FilterMode::Linear` for deterministic visual audit PNGs; the live FPS overlay is suppressed in
capture renders.
Mouse mapping: `virtual = (screen − offset) / scale`, unclamped (the paddle clamps itself, §4.2).

---

## 7. Rendering spec

### 7.1 Draw order **[VERIFIED]** (Flash stage depths, back → front)

```
 1  bounds border (green)        22  bonus banner
11  tunnel lattice + depth ring  26  score value      27  enemy pips
13  enemy paddle                 33  player pips      38  "bonus:"  39  bonus value
20  ball / pop                   40  "score:"  41  "level:"  42  level value
                                 43  PLAYER PADDLE    50  level-splash text
```

The player paddle draws **over** the ball and all HUD except the splash text.

### 7.2 HUD layout **[VERIFIED anchors / APPROX ±2 px until audit]**

| Element | Anchor (x, y) | Style |
|---|---|---|
| `score:` label | (48.5, 26.7) | cyan `#00ffff`, 12 px |
| score value | tx (21.65, 26.7), text box local x ∈ [72.4, 172], left-aligned | cyan, 12 px |
| `level:` label | (240.05, 26.7) | cyan, 12 px |
| level value | (283.5, 26.7), local x ∈ [−2, 21], left | cyan, 12 px |
| `bonus:` label (var `bonusWord`) | (216, 210.7) | cyan, 12 px |
| bonus value (`bonusDisplay`) | (265.5, 210.7), local x ∈ [−2, 42.5], left | cyan, 12 px |
| enemy lives pips | anchor (70, 48) | §7.6 |
| player lives pips | anchor (280, 48) | §7.6 |
| Level splash | centered text box tx (59.8, 104.25), width 234.5 | white, 40 px, "Level N" |
| Bonus banner | (175, 206) | §7.5 |
| "Game Over" | (45.05, 96.25) | white, large **[APPROX size]** |

`bonusWord`/bonus value are blanked when entering Game Over **[VERIFIED]**.

### 7.3 Ball gradient **[VERIFIED]**
Radial: `#ffffff` from center out to ratio 33/255 of the radius, blending to `#3fff11` at the rim.
Pop ball: `#ffffff` center → `#ff0000` rim, full radius blend.

### 7.4 Paddle hit flash **[VERIFIED]** — 10-tick animation, per-tick alpha multipliers (/256, applied to
each pip's base white; base colored rect is untouched):

```
hit pip      : 256, 238, 219, 201, 182, 164, 145, 127, 108, 90
other 4 pips : 192, 181, 169, 158, 147, 135, 124, 113, 101, 90
```

Center ('C') hit additionally color-ramps the center pip red→white while the 4 corners do the
"other" ramp; center pip per tick `(mult, add_red, alpha)`:

```
mult : 0, 28, 57, 85, 114, 142, 171, 199, 228, 256
addR : 255, 227, 198, 170, 142, 113, 85, 57, 28, 0
alpha: 256, 238, 219, 201, 182, 164, 145, 127, 108, 90
→ rgb = (255·mult/256 + addR, 255·mult/256, 255·mult/256)
```

Re-triggering restarts the animation. After tick 10, the D7 framed paddle remains and flash overlays
hide instead of restoring the SWF idle overlay look. Because the D7 player paddle uses a gray framed
presentation, the Rust renderer adds a player-only blue outline around the active pip shape during
the same 10-tick flash; the original alpha/color tables above still drive the fill, and the enemy
paddle keeps the unoutlined SWF-derived flash.

### 7.5 Bonus banner **[VERIFIED]** — sprite at (175, 206): white, 14 px, centered text with content
"Accuracy Bonus!" / "Curve Bonus!" / "Super Curve Bonus!". The SWF sprite includes a static black
bar, but the D7 desktop-reference tunnel presentation renders the banner as text-only so the green
tunnel rails remain continuous behind it. 61-tick animation, `(rel_y, alpha/256)` — rel_y is the
text offset from the anchor:

```
in   (16): y 2.75 → −17.25 linear; alpha 0,17,34,51,68,85,102,119,137,154,171,188,205,222,239,256
dip  (15): y hold −17.25;          alpha 244,232,220,208,196,184,172,161,149,137,125,113,101,89,77
rise (16): y hold −17.25;          alpha 88,99,111,122,133,144,155,167,178,189,200,211,222,234,245,256
out  (14): y −15.8 → 2.75 linear;  alpha 238,219,201,183,165,146,128,110,91,73,55,37,18,0
```

Idle: text hidden. Re-trigger restarts at `in`; when one contact awards both an accuracy and a curve
bonus, the curve/super banner triggers last and wins (its text and restart overwrite).

### 7.6 Lives pips **[VERIFIED]** [CORRECTED — see DEVIATIONS.md C2]
5 × 5 px radial-gradient dots, 7 px horizontal spacing. Enemy dots extend **right** of the (70, 48)
anchor (centers at +0.25 + 7·i); player dots extend **left** of (280, 48). Survivors hug the anchor.
Enemy dots: white→`#ff0000`; player dots: white→`#0000ff`.

| lives | enemy pips shown | player pips shown |
|---|---|---|
| 5 | 4 | 4 |
| 4 | 3 | 3 |
| 3 | 2 | 2 |
| 2 | 1 | 1 |
| 1 | 0 | 0 |
| 0 | 0 | 0 |

(**Both** displays literally show lives − 1 — sprite 66 removes its fifth dot at frame 3, inside the
"L5" segment, so even the enemy's full-health "L3" settles at 2 dots. Quirk Q4 — keep it.)

### 7.7 Font **[APPROX / DEVIATION]** [CORRECTED — see DEVIATIONS.md D2]
The SWF embeds a subset of **BankGothicBT-Medium** (DefineFont2 id 8 — it *is* named; "nameless" was
a parser artifact). Bank Gothic is commercial, so the Rust port bundles **Michroma Regular** under
the SIL Open Font License (`assets/fonts/OFL-Michroma.txt`) and uses uppercase tracked styling for
the title, menus, HUD, banners, and buttons. Colors and anchors remain source-derived; glyph shapes
and some text metrics are the approximation.

---

## 8. Audio
Embed the 5 extracted SWF WAVs with `include_bytes!`. Default builds use `rodio` for sound output
while keeping macroquad graphics-only, avoiding `quad-snd` startup panics on Linux hosts without a
usable PCM route. The runtime decodes the embedded sound table once during audio startup. Triggers
exactly as in §3.8 / §4.4 / §4.5: one playback per event, overlapping instances allowed (Flash
`Sound.start(0, 1)` semantics), at master volume 0.8. If device setup or decode fails, log and
continue silently.
`cargo run --no-default-features --features runtime` compiles the playable silent no-op backend;
plain `--no-default-features` remains for headless library/tests.

---

## 9. Game flow — phase machine with frame-accurate timings **[VERIFIED]**

Timeline labels → phases (all durations in 30 Hz ticks):

| Phase | Origin frames | Duration / exit |
|---|---|---|
| `Title` | 4–5 (+ intro 1–4) | until button: Start Game / Zen / High Scores / Visual mode toggle |
| `HighScores` | 14–19 | until Main Menu button; returning from a post-game high-score submission clears stale game state before `Title` (D8) |
| `StartGameInit` | 36–44 | 9 ticks of transition, then init (§3 constants, score 0, lives 5/3, level 1, `bonus:` HUD strings restored after a prior Game Over blanked them) |
| `LevelSplash` | 45–90 | 46 ticks total: "Level N" text shows for the first 45; on the final tick the text is removed, the **entire HUD appears** (it is absent during the splash — score/level/bonus/lives/banner bar all gone), and per-level setup applies (speed/skill/curve from tables, rally bonuses reset, bonusDisplay = 3000, drain counter = 10, enemy lives pips refreshed). Player paddle live throughout; ball/ring/enemy absent. |
| `Serve` | 91–96 | enemy appears at tick 1, ball+ring at tick 2 (fresh ball: center, z 0, all velocities 0); waits for serve click |
| `Rally` | stopped at 96 | runs §4 until a miss |
| `Miss` | ball clip frames 2–20 | 19 ticks: ball replaced by frozen pop at last projected pos/scale; sim halted (Q2 serve quirk active). The ring stays live, pinned at the published miss z clamped ≥ 0 — full-size full-alpha on a player-side miss (z ≈ −1 → 0), small and dim (alpha ≈ 100 − z) on an enemy-side miss. Then route |
| route | sprite 80 frame 20 | enemyLives < 1 → level += 1, score += bonusDisplay, enemyLives = 3, → `LevelSplash`; else playerLives < 1 → blank bonus HUD → `GameOver`; else → `Serve` |
| `GameOver` | 97–103 | paddles/ball/ring removed; "Game Over" text appears; **the HUD stays on screen** (score/level/lives still visible; bonus strings blanked). After ~7 ticks the high-score check (local, §10) routes: qualified → `NameEntry`, else → `End`. The "Game Over" text + HUD persist underneath both. |
| `NameEntry` | 104–108 | box (183 × 76, black, white border) at (175, 181.3) **overlaid on the persisting Game Over screen**: "Name:" label, text input (default "enter here"), submit button. Submit → `HighScores` (original frame_110 jumps to the HighScores label) |
| `End` | 111–115 | Main Menu button at (174.85, 139.85) overlaid on the persisting Game Over screen + HUD → clean `Title` state (D8) |

Re-serve within a level resets only ball + ring (fresh instances); enemy and player paddles persist (Q6).

---

## 10. Menus & high scores (network replacement) **[DEVIATION — required]**

The original POSTs to `highscore.php` / `checkscore.php` / `enterscore.php` (with a hash salt
`"a83l9xj"` — historical curiosity, do not reimplement). Replace with a local table:

- File `highscores.txt` under the user's data directory by default (`%APPDATA%\curveball\`,
  `~/Library/Application Support/curveball/`, or `${XDG_DATA_HOME:-~/.local/share}/curveball/`),
  with `CURVEBALL_HIGHSCORES` as an explicit file-path override; 10 lines, tab-separated
  `name<TAB>level<TAB>score`; missing/corrupt file → defaults (name "none", level 0, score 0).
  std-only I/O; save creates the parent directory, writes via temp file + rename, and logs via
  `eprintln!` on failure (never crash the game loop).
- Display formatting **[VERIFIED from placeholders]**: score zero-padded to 9 digits, level to 2.
- Qualification: score strictly greater than the 10th entry's score → `NameEntry`; insert sorted desc.

Screen layouts **[VERIFIED anchors]**:
- Title: "curveball" (white) at (60.3, 32.25); buttons "start game" (174.5, 115.1), "high scores"
  (175.5, 132.3), and Rust-only extensions "zen" (D9) and `VISUAL: ...` (D14) below them — white
  pill buttons, black ~7 px labels, centered.
- Zen gameplay HUD extensions (D15): `SILKY: ON/OFF` and `AIMBOT: ON/OFF` pill buttons on the
  top row. These controls are not drawn or consumed in Classic games.
- High scores: white-bordered black panel spanning (66.5, 53)–(282.45, 197); headers `name` (154.15, 68.5),
  `level` (32.65, 68.5), `score` (82.15, 68.5); 10 rows at y = 85.5 + 10·i with level col x 29.2
  (centered in 15 px), score col x 81.55 (centered in 52.8 px), name col x 157.1 (centered in 92.4 px);
  "main menu" button at (176.45, 215.85). All text white 7 px. (The original briefly shows a 240 × 18
  black bar at (179, 190) as a loading indicator during the PHP wait, removed on response; with local
  storage the wait is zero and it never appears — covered by D3.)
- Name entry box contents (local to (175, 181.3)): "Name:" at (−83.3, −14.15); input field at (−71, −14.2),
  white 12 px, centered, default text "enter here" (a submit with the default text is ignored, as in the
  original `if(Name != "enter here")` guard — but the original still navigates on; replicate: navigate to
  Submit/HighScores regardless, only *record* when the name was edited). Congratulation line at
  (−72, −34.55): "You Got a High Score!" **[VERIFIED — decoded via the embedded DefineFontInfo code
  table; see DEVIATIONS.md C3]**.
  Submit button at (0.05, 15.6).
- Keyboard text entry: printable ASCII, max ~14 chars (field width), backspace; first keypress clears
  the placeholder. **[APPROX — Flash input-field niceties not fully specified.]**

---

## 11. Configs (user-supplied baseline — use verbatim except where noted)

`rustfmt.toml` and `clippy.toml`: copy the provided files **unchanged**.

`Cargo.toml`: the provided template with exactly these edits — `name = "curveball"`, add macroquad
and optional rodio dependencies, keep every lint and profile line as-is:

```toml
[package]
name = "curveball"
version = "0.1.0"
edition = "2024"
rust-version = "1.96.0"
publish = false

[dependencies]
macroquad = { version = "0.4", optional = true } # pin exact patch via Cargo.lock
rodio = { version = "0.17", optional = true, default-features = false, features = ["wav"] }

[features]
default = ["runtime", "audio"]
runtime = ["dep:macroquad"]
audio = ["runtime", "dep:rodio"]

[[bin]]
name = "curveball"
path = "src/main.rs"
required-features = ["runtime"]

# ... [lints.rust], [lints.clippy], [profile.*] sections copied verbatim from the template ...
```

Notes: `panic = "abort"` + `lto = "fat"` + `strip = true` in release are compatible with macroquad.
`unsafe_code = "forbid"` holds — no user-side unsafe is needed anywhere in this design. `publish =
false` prevents accidental crates.io publication of the bundled reference/provenance material and
assets.

## 12. Coding standards under this lint regime

- `unwrap_used` / `expect_used` / `panic` are warn-level and manual CI runs `-D warnings`: prefer
  real error handling and use Rust 2024's `#[expect(..., reason = "...")]` only for local, justified cases
  (tests, deterministic math checks, or an API boundary where failure is already handled). `main`
  performs setup and degrades gracefully (e.g. missing audio device → silent mode, logged).
- Every constant lives in `consts.rs` with a doc comment citing its source
  (`/// frame_44/DoAction.as: world.depth = 75`). No magic numbers at use sites.
- Sim core: no `f32`, no macroquad imports, exhaustive `match` on the phase enum, no interior
  mutability. `Sim::tick(&mut self, input: TickInput) -> Vec<SimEvent>` — sounds/banners/flashes are
  emitted as events so the sim stays headless-testable.
- Nursery/pedantic groups are warn: fix rather than allow; any `#[expect]` needs a reason string.
- Manual CI (`workflow_dispatch` only): format, all-features tests, headless tests,
  runtime-without-audio tests, all-features clippy, headless clippy, and
  `cargo deny check advisories`. `deny.toml` intentionally ignores
  `RUSTSEC-2025-0035` until macroquad has a patched release or the runtime is migrated/forked.

## 13. Milestones & acceptance criteria

| M | Deliverable | Acceptance |
|---|---|---|
| M0 | Scaffold, configs, 30 Hz fixed-step loop, display-rate native presentation + letterboxed scaling | Manual CI green; tick counter advances 30/s wall-clock ±0.1 %; window resizes with integer scaling; renderer is not capped to 30 FPS |
| M1 | `sim/` complete | `tests/gold1.rs` passes (§14); unit tests for projection table, zone classifier, serve injection, economy, drain cadence, wall damp |
| M2 | Entity renderer | Serve/rally screenshots match the SWF/Ruffle reference where applicable and the desktop reference for D7 tunnel/framed-paddle presentation |
| M3 | Phase machine + HUD + scoring/lives | Full game loop playable start→game over; splash/miss/serve durations are 46/19/n ticks exactly (assert via tick-stamped logs) |
| M4 | Anim tables + audio | Pip flash, banner, lives pips match §7.4–7.6 tables; 5 sounds fire per §3.8; Q2 quirk demonstrable |
| M5 | Menus + local high scores + name entry | Round-trips `highscores.txt`; layout anchors per §10 |
| M6 | Fidelity audit (§15) | Checklist complete; `DEVIATIONS.md` finalized with intentional D1-D12, D14, and D15 tradeoffs |

## 14. Test plan — GOLD-1 trajectory (normative values)

Scenario: level 1; mouse pinned at (175.5, 125.5) forever (paddle speed ≡ 0); serve applied at tick 0
per §4.5. Generated by `golden_sim.py` (reference kit), which implements §4 exactly — if the Rust sim
and these values disagree, the Rust sim is wrong (or you found a plan bug: prove it from the AS source
and update both).

Serve result: `curve = (−0.01, −0.01)`, `score = 100`, `accuracyBonus → 90`.

Ball state after tick N (assert each field, relative tolerance 1e-9):

| N | x | y | z | vx | vy | cx | cy |
|---|---|---|---|---|---|---|---|
| 1 | 175.49 | 125.51 | 2 | −0.01 | −0.01 | −0.00996015936255 | −0.00996015936255 |
| 2 | 175.470039841 | 125.529960159 | 4 | −0.0199601593625 | −0.0199601593625 | −0.00992047745274 | −0.00992047745274 |
| 10 | 174.956521616 | 126.043478384 | 20 | −0.0982260864642 | −0.0982260864642 | −0.0096086610101 | −0.0096086610101 |
| 37 | 168.794689725 | 132.205310275 | 74 | −0.344658758899 | −0.344658758899 | −0.00862685753427 | −0.00862685753427 |
| 38 | 168.441404108 | 132.558595892 | 75 | −0.353285616433 | −0.353285616433 | −0.00859261251464 | −0.00859261251464 |
| 76 | 148.951372744 | 152.048627256 | 0 | −0.656850257334 | −0.656850257334 | −0.0 | 0.0 |
| 77 | 148.294522487 | 152.705477513 | 2 | −0.656850257334 | −0.656850257334 | −0.0 | 0.0 |
| 114 | 123.991062966 | 177.008937034 | 75 | −0.656850257334 | −0.656850257334 | −0.0300558179039 | −0.0300558179039 |
| 117 | 121.840655786 | 179.159344214 | 69 | −0.746658955232 | −0.746658955232 | −0.0296980143266 | −0.0296980143266 |

Event log (assert order, ticks, and payloads):

```
tick 38  enemy hit    pip=C   curve=(−0.00859261251, −0.00859261251)  enemy speed=(−0.214815313, +0.214815313)
tick 76  player hit   pip=BL  (edge → no accuracy bonus)  score 100→200  hitScore→90  curve=(−0.0, 0.0)
tick 114 enemy hit    pip=BL  curve=(−0.0300558179, −0.0300558179)   enemy speed=(−0.751395448, +0.751395448)
tick 145 wallBounce2 (bottom)
tick 152 PLAYER MISS  → playerLives 4, rally bonuses reset, bonusDisplay stays 2675
```

bonusDisplay checkpoints: 3000 through tick 10 (the first −25 lands at tick 11); 2925 at tick 37;
2850 at tick 76; 2675 final (−25 every 11 in-flight ticks) [CORRECTED — see DEVIATIONS.md C1].
Projection unit values: §3.3 table. Additional unit cases: enemy
easing divisor 15 when `dir.z ≤ 0`; clamps [55, 296] × [45, 206]; wall clamp positions 40/211/40/311;
Q11 drain-counter carry-over across rallies (counter only resets at level setup).

## 15. Fidelity audit protocol (M6)

1. Run `reference/decompiled/curveball.swf` in Ruffle (desktop) alongside the port's 4× native capture output. Use the
   supplied desktop screenshot as the visual source for D7 tunnel/framed-paddle presentation.
2. Static states: Title, High Scores, Serve (frame 96), Rally, Level splash, Game Over, End — capture
   with `CURVEBALL_WARP=<state> CURVEBALL_MOUSE=x,y CURVEBALL_SHOT=path.png[:ticks]`, align to the
   green border, and keep every HUD anchor within ±2 px after scaling to the same reference size.
3. Dynamic: serve with a deliberately still cursor in both; the symmetric drift (GOLD-1) should be
   visually identical for the first rally. Verify pip flash decay, banner breathe cycle, ring fade,
   pop appearance, miss pause length (19 ticks ≈ 0.63 s), splash length (46 ticks ≈ 1.53 s).
4. Sound A/B per trigger; confirm 80 % loudness ratio against a 100 % reference tone.
5. Quirk demonstrations: Q2 (click pop), Q4 (pip counts), Q6 (paddle persistence across splash),
   D8 (post-game main-menu restart returns to a clean Title and can start a fresh game), D9
   (Zen player misses re-serve without decrementing lives or entering Game Over), D10
   (render interpolation smooths high-refresh displays without mutating gameplay), D11
   (experimental sim-rate override is clearly non-faithful), D12 (FPS counter visibility during
   normal window rendering, suppressed for `CURVEBALL_SHOT` captures),
   and D14 (Silky runs a non-faithful 400 Hz app/world tick with wall-clock-scaled counters,
   catch-up mouse distribution, zone-aware prediction, and swept crossing contacts), and D15
   (Zen-only in-game Silky/Aimbot controls, with level-11 pseudo-random angled swipe
   assistance).
6. Close `DEVIATIONS.md`: it must contain only approved fidelity or product-quality deviations
   (currently D1-D12, D14, and D15) plus implementation corrections C1-C4.
7. Residual-risk check: the normative per-tick entity order (§4: paddle → enemy → ring → ball) follows
   documented AVM1 instantiation-order semantics; the placement/replacement pattern preserves it across
   rallies and levels. If any dynamic discrepancy survives steps 1–5 with everything else exact, suspect
   this first and confirm against Ruffle (whose AVM1 implements the same ordering) before touching §4.

## 16. Deliberate deviations (initial ledger)

| ID | What | Why |
|---|---|---|
| D1 | Level ≥ 11 clamps to table index 9 (flag for strict zero-speed softlock) | Original softlocks after level 10; unplayable as shipped |
| D2 | Bundled Michroma replaces the embedded BankGothicBT-Medium subset | Commercial font, not licensable; OFL substitute with retuned tracked metrics |
| D3 | Local user-data `highscores.txt` replaces dead PHP endpoints | Servers gone since ~2005; user-data storage avoids installed-binary write failures |
| D4 | Logical 350×250 stage renders to a 4× native target (default 1400×1000) | Desktop reference screenshot is high-resolution; this avoids low-res scaled output |
| D5 | Name-entry editing niceties approximated | Flash TextField internals unspecified |
| D6 | Title intro (timeline ticks 1–4, ~0.13 s) skipped — instant title screen | Transition content not recoverable from tags |
| D7 | Gameplay adds the desktop-reference tunnel lattice and framed paddles | Supplied desktop reference is the visual source of truth |
| D8 | Post-game main-menu navigation clears stale gameplay state before returning to Title | Fixes restart behavior instead of preserving a SWF timeline wart |
| D9 | Title-menu Zen mode starts a normal game with unlimited player lives | Requested quality-of-life mode |
| D10 | Render interpolation between 30 Hz sim snapshots plus gated live player-paddle prediction | Smooth, responsive high-refresh presentation without changing gameplay math or desyncing player-hit sounds |
| D11 | `CURVEBALL_SIM_HZ=<hz>` experimental app/world cadence override | Feel-test alternate rates without changing the mode defaults |
| D12 | FPS counter visible during normal window rendering and suppressed for deterministic captures | Requested frame-pacing visibility |
| D14 | Title-menu Faithful/Silky visual toggle | Non-faithful 400 Hz app/world cadence, late mouse sampling, catch-up mouse distribution, zone-aware prediction, swept crossing contacts, and smoother render keyframes while keeping the faithful default baseline |
| D15 | Zen-only in-game Silky and Aimbot toggles | Requested assistance controls; aimbot uses level-11 AI plus pseudo-random angled swipe returns |

## 17. AS → Rust symbol map

| ActionScript | Rust |
|---|---|
| `world.{left,right,top,bottom,depth}` | `consts::{WORLD_LEFT, ... , WORLD_DEPTH}` |
| `world.speed / skillFactor / curveAmount` | `LevelParams { speed, skill, curve_amount }` from `consts::{LEVEL_SPEED, LEVEL_SKILL, LEVEL_CURVE}` |
| ball `myPos/mySpeed/myCurve`, `ballStop` | `sim::ball::Ball { pos, vel, curve, stopped }` |
| `_parent.ballPos*/ballDir*` | `sim::Published` (persists across rallies) |
| paddle `myPos/mySpeed` easing | `sim::paddle::step` / `sim::enemy::step` |
| `hitTest(clip)` | `project::overlap(prev_ball_rect, rect)` |
| `varA`, projection expression | `project::{VAR_A, scale, vis}` |
| `world.hitScore/...Bonus/...Degrade`, `bonus`, `bonusDisplay` | `sim::score::Economy` |
| sprite 59/75 labels `N/UR/UL/BL/BR/C` | `app::PipFlash { zone, tick }` + `render::anim` tables |
| sprite 62 "bonus" timeline | `app::Banner` + `render::anim::BANNER_FRAMES` (§7.5 tables) |
| sprites 66/68 `L0..L5` | `render::hud::draw_lives` (§7.6 table) |
| timeline labels Start/HighScores/StartGame/Level/Serve/GameOver/Winner/Submit/End | `app::Phase` enum |
| `Sound.start(0,1)` + `setVolume(80)` | `runtime::audio::Audio::play(SoundId)` at volume 0.8 |
| `loadVariables("*.php" ...)` | `highscores::{load, qualify, insert, save}` |

---

*Everything tagged [VERIFIED] traces to `reference/decompiled/scripts/**` or the tag dump in the reference
kit. When in doubt: read the source, run `golden_sim.py`, frame-step Ruffle — in that order.*
