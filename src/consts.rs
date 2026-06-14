//! Gameplay- and presentation-relevant constants for the port.
//!
//! Faithful values cite their source: a decompiled script under
//! `reference/decompiled/scripts/` or the SWF tag stream. Port extensions are
//! marked as deviations in `DEVIATIONS.md`. Simulation constants are `f64`
//! because AS1 `Number` is an IEEE-754 double; the sim must reproduce its
//! arithmetic bit for bit.

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// SWF header: stage is 350×250 px.
pub const STAGE_W: f64 = 350.0;
/// SWF header: stage is 350×250 px.
pub const STAGE_H: f64 = 250.0;
/// SWF header frame rate; one app/sim tick is one original Flash frame.
pub const TICK_HZ: u32 = 30;
/// Opt-in non-faithful Silky app/world cadence.
pub const SILKY_PHYSICS_HZ: u32 = 400;
/// Fraction of one original Flash frame covered by one Silky tick.
pub const SILKY_DT_SCALE: f64 = TICK_HZ as f64 / SILKY_PHYSICS_HZ as f64;
/// Native render scale for vector/raster presentation. Simulation and input
/// stay in the original 350×250 stage coordinates; the render target is 4×.
pub const RENDER_SCALE: u32 = 4;

// ---------------------------------------------------------------------------
// World geometry — frame_44/DoAction.as + bounds clip placement
// ---------------------------------------------------------------------------

/// `world.left = bounds._x` — bounds clip placed at (25, 25) (tag stream, frame 1, depth 1).
pub const WORLD_LEFT: f64 = 25.0;
/// `world.top = bounds._y`.
pub const WORLD_TOP: f64 = 25.0;
/// `world.right = bounds._x + bounds._width` = 25 + 301 (content AABB incl. 1 px stroke).
pub const WORLD_RIGHT: f64 = 326.0;
/// `world.bottom = bounds._y + bounds._height` = 25 + 201.
pub const WORLD_BOTTOM: f64 = 226.0;
/// `world.depth = 75`.
pub const WORLD_DEPTH: f64 = 75.0;
/// Ball/paddle load scripts: `wx = (wright - wleft) / 2 + wleft` = 175.5.
pub const WORLD_CX: f64 = 175.5;
/// `wy = (wbottom - wtop) / 2 + wtop` = 125.5.
pub const WORLD_CY: f64 = 125.5;

// ---------------------------------------------------------------------------
// Projection — ball/enemy/ring scripts
// ---------------------------------------------------------------------------

/// `varA = 31.066017` (= 75 / tan 67.5°, chosen so the scale at z = 75 is 0.25).
pub const VAR_A: f64 = 31.066_017;

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// Ball shape (DefineShape 78): Ø 30 px; `radius = swidth / 2` in the ball script.
pub const BALL_DIAMETER: f64 = 30.0;
/// Paddle shapes (DefineShape 53/74): 60 × 40 px. Clamp half-extents derive from these.
pub const PADDLE_W: f64 = 60.0;
/// See [`PADDLE_W`].
pub const PADDLE_H: f64 = 40.0;
/// Player paddle easing divisor — frame_45 paddle enterFrame: `myPos.x -= (myPos.x - myTarX) / 1.5`.
pub const PLAYER_EASE: f64 = 1.5;
/// Enemy easing divisor toward world center when the ball is not approaching —
/// frame_91 enemy enterFrame: `myPos.x -= (myPos.x - myTarX) / 15`.
pub const ENEMY_EASE_HOME: f64 = 15.0;

// ---------------------------------------------------------------------------
// Ball physics — frame_92 ball scripts
// ---------------------------------------------------------------------------

/// Ball-local `curveDecay = 1.004` (load script). The `world.curveDecay = 0.01` set in
/// frame_44 is dead code — the ball never reads it (quirk Q1).
pub const CURVE_DECAY: f64 = 1.004;
/// Wall curve damp, computed inline at every wall bounce.
///
/// `myCurve /= (curveDecay - 1) * 50 + 1`. Evaluates to 1.2000000000000002 in
/// IEEE f64; the const expression reproduces the exact runtime arithmetic.
pub const WALL_CURVE_DAMP: f64 = (CURVE_DECAY - 1.0) * 50.0 + 1.0;
/// Serve minimum-curve injection threshold and magnitude — frame_92 ball mouseDown:
/// `if(Math.abs(myCurve.x) < 0.01) myCurve.x = ±0.01` (quirk Q3: serves only).
pub const SERVE_MIN_CURVE: f64 = 0.01;

// ---------------------------------------------------------------------------
// Hit-zone classification — frame_92 ball enterFrame/mouseDown cascades
// ---------------------------------------------------------------------------

/// Horizontal center-zone half-width: `pPosX + 7 < myPos.x` etc.
pub const ZONE_DX: f64 = 7.0;
/// Vertical center-zone half-height: `pPosY + 5 >= myPos.y` etc.
pub const ZONE_DY: f64 = 5.0;
/// Curve-bonus outer threshold: `0.1 < Math.abs(myCurve.x)`.
pub const CURVE_CLASS_HI: f64 = 0.1;
/// Curve-bonus inner threshold: `0.05 < Math.abs(myCurve.y)`.
pub const CURVE_CLASS_LO: f64 = 0.05;

// ---------------------------------------------------------------------------
// Level tables — frame_44/DoAction.as (index = level − 1; exactly 10 entries)
// ---------------------------------------------------------------------------

/// `levelSpeed = new Array(2,2.33,2.66,3,3.33,3.66,4,4.33,4.66,6)`.
pub const LEVEL_SPEED: [f64; 10] = [2.0, 2.33, 2.66, 3.0, 3.33, 3.66, 4.0, 4.33, 4.66, 6.0];
/// `levelSkillFactor = new Array(17,14,11,9,7,5,3.5,2.75,2,1)`.
pub const LEVEL_SKILL: [f64; 10] = [17.0, 14.0, 11.0, 9.0, 7.0, 5.0, 3.5, 2.75, 2.0, 1.0];
/// `levelCurve = new Array(25,22.5,20,17.5,15,12.5,10,10,10,10)` — the `curveAmount` divisor.
pub const LEVEL_CURVE: [f64; 10] = [25.0, 22.5, 20.0, 17.5, 15.0, 12.5, 10.0, 10.0, 10.0, 10.0];

/// Deviation D1 (see `DEVIATIONS.md`).
///
/// `false` clamps level ≥ 11 to the last table entry; `true` reproduces the
/// original softlock (AS1/SWF5 `undefined` coerces to 0, so level-11 params
/// become 0 and the serve never travels).
pub const STRICT_LEVEL_11_SOFTLOCK: bool = false;

// ---------------------------------------------------------------------------
// Scoring economy — frame_44 init, frame_90 per-level reset, frame_92 awards
// ---------------------------------------------------------------------------

/// `world.hitScore = 100`.
pub const HIT_SCORE_INIT: i32 = 100;
/// `world.hitDegrade = 10`.
pub const HIT_DEGRADE: i32 = 10;
/// `world.curveBonus = 50`.
pub const CURVE_BONUS_INIT: i32 = 50;
/// `world.curveDegrade = 5`.
pub const CURVE_DEGRADE: i32 = 5;
/// `world.superCurveBonus = 150`.
pub const SUPER_CURVE_BONUS_INIT: i32 = 150;
/// `world.superCurveDegrade = 15`.
pub const SUPER_CURVE_DEGRADE: i32 = 15;
/// `world.accuracyBonus = 100`.
pub const ACCURACY_BONUS_INIT: i32 = 100;
/// `world.accuracyDegrade = 10`.
pub const ACCURACY_DEGRADE: i32 = 10;
/// `world.bonusDisplay = 3000` (reset each level at frame_90).
pub const BONUS_DISPLAY_INIT: i32 = 3000;
/// `world.bonus = 10` — the in-flight drain counter (resets only at level setup; quirk Q11).
pub const BONUS_COUNTER_INIT: i32 = 10;
/// frame_92 ball enterFrame drain: `world.bonusDisplay -= 25`.
pub const BONUS_DRAIN_STEP: i32 = 25;

/// `world.playerLives = 5`.
pub const PLAYER_LIVES_INIT: i32 = 5;
/// `world.enemyLives = 3` (frame_44; restored to 3 at each level-up routing).
pub const ENEMY_LIVES_INIT: i32 = 3;

// ---------------------------------------------------------------------------
// Phase timings (30 Hz ticks) — main timeline frames
// ---------------------------------------------------------------------------

/// Frames 36–44 ("StartGame" label through the frame_44 init action): 9 ticks.
pub const START_GAME_TICKS: u32 = 9;
/// Frames 45–90 ("Level" label through the frame_90 setup action): 46 ticks.
pub const SPLASH_TICKS: u32 = 46;
/// The "Level N" text is visible for the first 45 splash ticks (removed at frame 90).
pub const SPLASH_TEXT_TICKS: u32 = 45;
/// Ball sprite (DefineSprite 80) frames 2–20: the frozen "pop" shows for 19 ticks,
/// then the frame-20 routing action runs.
pub const MISS_TICKS: u32 = 19;
/// Frames 97–103 ("GameOver" label through the frame-103 winner check): 7 ticks.
pub const GAME_OVER_TICKS: u32 = 7;
/// The splash's final frame (the frame_90 setup action); `Playing` advances
/// from here so its first tick lands on frame 91.
pub const FRAME_SPLASH_END: u32 = 90;
/// Main-timeline frame placing the enemy paddle (the "Serve" label).
pub const FRAME_ENEMY_SPAWN: u32 = 91;
/// Main-timeline frame placing the ball and depth ring.
pub const FRAME_BALL_SPAWN: u32 = 92;
/// Main timeline stops here (frame_96 `stop()`) awaiting the serve.
pub const FRAME_PLAY_HOLD: u32 = 96;

// ---------------------------------------------------------------------------
// Audio — frame_44 sound setup
// ---------------------------------------------------------------------------

/// `globalSound.setVolume(80)` — all effects play at 80 %.
pub const MASTER_VOLUME: f32 = 0.8;

// ---------------------------------------------------------------------------
// High scores — replacement for the dead PHP endpoints (deviation D3)
// ---------------------------------------------------------------------------

/// The original table holds 10 entries (`hsName0`..`hsName9` text fields).
pub const HIGH_SCORE_ROWS: usize = 10;
/// Name-entry placeholder — DefineEditText 83 initial text; an unedited submit is
/// not recorded (`if(Name != "enter here")` in the submit button action).
pub const NAME_PLACEHOLDER: &str = "enter here";
/// Approximate field-width cap on typed names (deviation D5).
pub const NAME_MAX_LEN: usize = 14;

// ---------------------------------------------------------------------------
// Button hit rectangles — (x_min, y_min, x_max, y_max), virtual-canvas px.
// Derived from each DefineButton2's hit shape transformed by its placement
// matrix in the tag stream. Shared by input handling and rendering.
// ---------------------------------------------------------------------------

/// Title "start game" — button 12 at (174.5, 115.1); pill shape 10 at
/// (0.8, 0.7) scaled (1.084, 0.8982).
pub const BTN_TITLE_START: (f64, f64, f64, f64) = (140.61, 110.73, 209.99, 120.87);
/// Title "high scores" — button 14 at (175.5, 132.3); pill at (−0.25, 0.65)
/// scaled (1.148, 0.9956).
pub const BTN_TITLE_SCORES: (f64, f64, f64, f64) = (138.51, 127.33, 211.99, 138.58);
/// Title "zen" — local extension matching the title-menu pill style.
pub const BTN_TITLE_ZEN: (f64, f64, f64, f64) = (143.50, 144.53, 207.50, 155.83);
/// Title visual-mode toggle — local extension matching the title-menu pill style.
pub const BTN_TITLE_VISUAL: (f64, f64, f64, f64) = (116.50, 161.73, 234.50, 173.03);
/// In-game Silky toggle — local extension, top HUD row.
pub const BTN_GAME_SILKY: (f64, f64, f64, f64) = (92.0, 6.0, 172.0, 17.0);
/// In-game aimbot toggle — local extension, top HUD row.
pub const BTN_GAME_AIMBOT: (f64, f64, f64, f64) = (178.0, 6.0, 258.0, 17.0);
/// HighScores "main menu" — button 21 at (176.45, 215.85); pill unscaled (64 × 11.3).
pub const BTN_HS_MENU: (f64, f64, f64, f64) = (144.45, 210.20, 208.45, 221.50);
/// End-screen "main menu" — button 21 reused at (174.85, 139.85).
pub const BTN_END_MENU: (f64, f64, f64, f64) = (142.85, 134.20, 206.85, 145.50);
/// "submit" — button 87 at (0.05, 15.6) inside sprite 90 at (175, 181.3);
/// hit shape 85 spans (−28.25..29) × (−6.75..8.5).
pub const BTN_SUBMIT: (f64, f64, f64, f64) = (146.80, 190.15, 204.05, 205.40);

// ---------------------------------------------------------------------------
// Colors — DefineShape fill/line styles and DefineText/EditText colors.
// ---------------------------------------------------------------------------

/// Original SWF bounds-border stroke; D7 also uses it for the tunnel lattice
/// and moving depth-ring stroke in the desktop-reference presentation.
pub const COLOR_BORDER: (u8, u8, u8) = (0x00, 0xff, 0x66);
/// Original SWF depth-ring stroke; D7 uses it for the static outer frame in
/// the desktop-reference presentation.
pub const COLOR_RING: (u8, u8, u8) = (0x77, 0xff, 0xfc);
/// D7 tunnel/depth-ring stroke alias.
pub const COLOR_TUNNEL: (u8, u8, u8) = COLOR_BORDER;
/// D7 static outer-frame stroke alias.
pub const COLOR_OUTER_FRAME: (u8, u8, u8) = COLOR_RING;
/// Ball radial-gradient rim (shape 78); center is white.
pub const COLOR_BALL_RIM: (u8, u8, u8) = (0x3f, 0xff, 0x11);
/// Pop rim, enemy paddle fill, enemy lives dots (shapes 79/74/64/65).
pub const COLOR_RED: (u8, u8, u8) = (0xff, 0x00, 0x00);
/// Player paddle fill and player lives dots (shapes 53/67).
pub const COLOR_BLUE: (u8, u8, u8) = (0x00, 0x00, 0xff);
/// HUD text (EditTexts 63/69/70/73 and DefineTexts 71/72).
pub const COLOR_HUD: (u8, u8, u8) = (0x00, 0xff, 0xff);

// ---------------------------------------------------------------------------
// Stage layout — placements from the tag stream. Stroked shapes extend half
// a pixel past their registration point (Flash centers strokes on the path),
// so the painted border AABB starts at 24.5, not 25.
// ---------------------------------------------------------------------------

/// Painted bounds-border rect (x, y, w, h).
///
/// Flash paints shape 6's 1 px stroke centered on the path: AABB
/// (24.5, 24.5)–(325.5, 225.5). Snapped outward to the pixel grid so the
/// stroke's inner edge sits exactly on the world walls (25/326/25/226) that
/// the ball clamps against.
pub const BORDER_RECT: (f32, f32, f32, f32) = (24.0, 24.0, 302.0, 202.0);
/// Ring shape 76 local offset from the projected (25, 25) anchor, and size.
pub const RING_OFFSET: (f32, f32) = (-0.5, -0.45);
/// See [`RING_OFFSET`].
pub const RING_SIZE: (f32, f32) = (301.0, 201.0);

/// Corner pip center offset from the paddle center (shapes 54–57 at (±14.1, ±9.1)).
pub const PIP_CORNER_OFFSET: (f32, f32) = (14.1, 9.1);
/// Corner pip size (27.8 × 17.8).
pub const PIP_CORNER_SIZE: (f32, f32) = (27.8, 17.8);
/// Center pip size (shape 58: 12 × 8).
pub const PIP_CENTER_SIZE: (f32, f32) = (12.0, 8.0);

/// Ball gradient: highlight center offset from the shape center and the
/// gradient radius — fill matrix tx/ty = (3.75, −4.5), scale 417/16384
/// (= 20.85 px over the 819.2 px gradient half-extent).
pub const BALL_GRAD_CENTER: (f32, f32) = (3.75, -4.5);
/// See [`BALL_GRAD_CENTER`].
pub const BALL_GRAD_RADIUS: f32 = 20.85;
/// White-core stop at ratio 33/255 (the pop uses ratio 0).
pub const BALL_GRAD_INNER_STOP: f32 = 33.0 / 255.0;

/// Lives dot: 5×5 px, gradient highlight offset (+0.5, −0.45), radius 3.175.
pub const DOT_SIZE: f32 = 5.0;
/// See [`DOT_SIZE`].
pub const DOT_GRAD_CENTER: (f32, f32) = (0.5, -0.45);
/// See [`DOT_SIZE`].
pub const DOT_GRAD_RADIUS: f32 = 3.175;
/// Enemy lives dots run *right* from (70.25, 48); player dots run *left*
/// from (280, 48); spacing 7 px. Both displays show lives − 1 dots (quirk
/// Q4 — see DEVIATIONS.md C2).
pub const LIVES_ENEMY_ANCHOR: (f32, f32) = (70.25, 48.0);
/// See [`LIVES_ENEMY_ANCHOR`].
pub const LIVES_PLAYER_ANCHOR: (f32, f32) = (280.0, 48.0);
/// See [`LIVES_ENEMY_ANCHOR`].
pub const LIVES_DOT_SPACING: f32 = 7.0;

/// Banner text: centered at x 175; top edge = 206 + rel_y from the §7.5
/// table; baseline = top + 0.8 × 14 px.
pub const BANNER_TEXT_CX: f32 = 175.0;
/// See [`BANNER_TEXT_CX`].
pub const BANNER_TEXT_ANCHOR_Y: f32 = 206.0;
/// See [`BANNER_TEXT_CX`].
pub const BANNER_BASELINE_OFFSET: f32 = 11.2;

// ---------------------------------------------------------------------------
// Text anchors — placement + DefineText run origin (baseline) or EditText
// bounds + 2 px gutter. Centered texts anchor on the original ink-span
// center so a substitute font keeps the optical placement (deviation D2).
// All Flash text baselines sit at 0.8 em below the box top (BankGothic
// ascent 819/1024).
// ---------------------------------------------------------------------------

/// HUD row baseline: EditTexts at y 26.7, 12 px → 26.7 + 9.6 × 0.997.
pub const HUD_TOP_BASELINE: f32 = 36.27;
/// Bottom HUD row baseline: y 210.7 + 9.57.
pub const HUD_BOTTOM_BASELINE: f32 = 220.27;
/// `score:` label ink-left (DefineText 71 at (48.5, 26.7), run x 1.0).
pub const SCORE_LABEL_X: f32 = 49.5;
/// `level:` label ink-left (DefineText 72 at (240.05, 26.7), run x 1.15).
pub const LEVEL_LABEL_X: f32 = 241.2;
/// `bonus:` label text-left (EditText 69 at (216, 210.7)).
pub const BONUS_LABEL_X: f32 = 216.0;
/// HUD font size (12 px cyan).
pub const HUD_FONT_PX: u16 = 12;

/// Splash "Level N": EditText 60 at (59.8, 104.25), box width 234.5,
/// centered, 40 px white; baseline = 104.25 + 32.04.
pub const SPLASH_CX: f32 = 175.05;
/// See [`SPLASH_CX`].
pub const SPLASH_BASELINE: f32 = 136.29;
/// See [`SPLASH_CX`].
pub const SPLASH_FONT_PX: u16 = 40;

/// "Game Over" (DefineText 81 at (45.05, 96.25), 40 px): ink center 186.2,
/// baseline 96.25 + 32.04.
pub const GAME_OVER_CX: f32 = 186.2;
/// See [`GAME_OVER_CX`].
pub const GAME_OVER_BASELINE: f32 = 128.29;

/// Title "curveball" (DefineText 9 at (60.3, 32.25), 40 px white).
pub const TITLE_CX: f32 = 175.5;
/// See [`TITLE_CX`].
pub const TITLE_BASELINE: f32 = 64.29;

/// Button label anchors (ink center x, baseline y); 10 px black except the
/// 12 px "submit". From each label DefineText inside its button records.
pub const TITLE_START_LABEL: (f32, f32) = (177.04, 117.91);
/// See [`TITLE_START_LABEL`].
pub const TITLE_SCORES_LABEL: (f32, f32) = (178.15, 135.11);
/// See [`TITLE_START_LABEL`].
pub const TITLE_ZEN_LABEL: (f32, f32) = (175.5, 152.31);
/// See [`TITLE_START_LABEL`].
pub const TITLE_VISUAL_LABEL: (f32, f32) = (175.5, 169.51);
/// In-game Silky button label anchor.
pub const GAME_SILKY_LABEL: (f32, f32) = (132.0, 14.45);
/// In-game aimbot button label anchor.
pub const GAME_AIMBOT_LABEL: (f32, f32) = (218.0, 14.45);
/// See [`TITLE_START_LABEL`].
pub const HS_MENU_LABEL: (f32, f32) = (179.13, 218.66);
/// See [`TITLE_START_LABEL`].
pub const END_MENU_LABEL: (f32, f32) = (177.53, 142.66);
/// See [`TITLE_START_LABEL`].
pub const SUBMIT_LABEL: (f32, f32) = (178.2, 200.3);

/// HighScores panel (shape 15): (66.5, 53)–(282.45, 197), black, 1 px white
/// border.
pub const HS_PANEL: (f32, f32, f32, f32) = (66.5, 53.0, 215.95, 144.0);
/// "high scores" heading (DefineText 16 at (103, 54.05) + matrix 31.4, 12 px).
pub const HS_HEADING: (f32, f32) = (178.2, 63.62);
/// Column header ink centers (DefineTexts 17/18/19, 10 px), baseline 76.53.
pub const HS_HEADER_NAME_CX: f32 = 228.8;
/// See [`HS_HEADER_NAME_CX`].
pub const HS_HEADER_LEVEL_CX: f32 = 107.3;
/// See [`HS_HEADER_NAME_CX`].
pub const HS_HEADER_SCORE_CX: f32 = 156.75;
/// See [`HS_HEADER_NAME_CX`].
pub const HS_HEADER_BASELINE: f32 = 76.53;
/// Row column centers: EditText boxes at x 29.2/81.55/157.1 + local bounds.
pub const HS_COL_LEVEL_CX: f32 = 101.2;
/// See [`HS_COL_LEVEL_CX`].
pub const HS_COL_SCORE_CX: f32 = 153.55;
/// See [`HS_COL_LEVEL_CX`].
pub const HS_COL_NAME_CX: f32 = 229.1;
/// First row (rank 1) at y 85.5; baseline = top + 0.8 × 7 px; rows step 10.
pub const HS_ROW_BASELINE: f32 = 91.1;
/// See [`HS_ROW_BASELINE`].
pub const HS_ROW_STEP: f32 = 10.0;

/// Name-entry box (shape 82 in sprite 90 at (175, 181.3)): local
/// (−90..93) × (−42..34) → abs (85, 139.3), 183 × 76, black, white border.
pub const NAME_BOX: (f32, f32, f32, f32) = (85.0, 139.3, 183.0, 76.0);
/// "You Got a High Score!" (DefineText 89, 12 px white, ink-centered).
pub const CONGRATS_CX: f32 = 177.65;
/// See [`CONGRATS_CX`].
pub const CONGRATS_BASELINE: f32 = 156.35;
/// See [`CONGRATS_CX`].
pub const CONGRATS_TEXT: &str = "You Got a High Score!";
/// "Name:" label ink-left (DefineText 84 at sprite-local (−83.3, −14.15)).
pub const NAME_LABEL_X: f32 = 94.4;
/// Input/label baseline (box top 167.1 + 9.6).
pub const NAME_BASELINE: f32 = 176.7;
/// Input field box (EditText 83 at (−71, −14.2)): abs x 137.05..254.5, centered.
pub const NAME_INPUT_CX: f32 = 195.78;
/// "submit" button face (shape 85): abs (146.8, 190.15), 57.25 × 15.25, white.
pub const SUBMIT_RECT: (f32, f32, f32, f32) = (146.8, 190.15, 57.25, 15.25);
