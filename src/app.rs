//! Top-level game: the timeline phase machine (PLAN.md §9), animation clocks,
//! event → sound/flash/banner mapping, and high-score persistence.
//!
//! Phases mirror the main-timeline labels and their frame-accurate timings.
//! Faithful mode advances them at 30 Hz. Silky mode runs the app/world at
//! 400 Hz and advances those Flash-frame counters through a scaled accumulator
//! so the wall-clock duration stays the same.

use crate::consts::{
    BALL_DIAMETER, BTN_END_MENU, BTN_GAME_AIMBOT, BTN_GAME_SILKY, BTN_HS_MENU, BTN_SUBMIT,
    BTN_TITLE_SCORES, BTN_TITLE_START, BTN_TITLE_VISUAL, BTN_TITLE_ZEN, ENEMY_EASE_HOME,
    FRAME_BALL_SPAWN, FRAME_ENEMY_SPAWN, FRAME_PLAY_HOLD, FRAME_SPLASH_END, GAME_OVER_TICKS,
    MISS_TICKS, NAME_MAX_LEN, NAME_PLACEHOLDER, PLAYER_EASE, SILKY_DT_SCALE, SILKY_PHYSICS_HZ,
    SPLASH_TICKS, START_GAME_TICKS, TICK_HZ, WORLD_BOTTOM, WORLD_CX, WORLD_CY, WORLD_LEFT,
    WORLD_RIGHT, WORLD_TOP,
};
use crate::highscores::ScoreTable;
use crate::sim::paddle::clamp_to_world;
use crate::sim::{CurveClass, Published, SimEvent, SimInput, World, Zone, level_params};

/// Length of the paddle pip-flash animation (sprites 59/75: 10 frames per label).
pub const PIP_FLASH_TICKS: u32 = 10;
/// Length of the bonus-banner animation (sprite 62 frames 10–70: 61 frames).
pub const BANNER_TICKS: u32 = 61;

/// One latched tick's worth of input, in virtual-canvas coordinates.
#[derive(Debug, Clone, Default)]
pub struct TickInput {
    /// Last known mouse position (macroquad reports the last position when
    /// the cursor leaves the window — matching Flash).
    pub mouse: (f64, f64),
    /// Press edges since the previous tick.
    pub clicks: Vec<(f64, f64)>,
    /// Printable characters typed since the previous tick.
    pub chars: Vec<char>,
    /// Backspace press edges since the previous tick.
    pub backspaces: u32,
}

struct GameplayInput {
    input: TickInput,
    control_clicked: bool,
    toggle_visual_after_tick: bool,
}

#[derive(Debug, Clone, Copy)]
struct AimbotSwipe {
    windup_offset: (f64, f64),
    strike_offset: (f64, f64),
}

const AIMBOT_RNG_SEED: u64 = 0x4d59_5df4_d0f3_3173;
const AIMBOT_SWIPE_START_FRAMES: f64 = 8.0;
const AIMBOT_SWIPE_LUNGE_TICKS: f64 = 1.12;
const AIMBOT_SWIPE_CONTACT_TICKS: f64 = 1.0;
const AIMBOT_WINDUP_OVERSHOOT: f64 = 400.0;
const AIMBOT_STRIKE_X_MIN: f64 = 38.0;
const AIMBOT_STRIKE_X_SPAN: f64 = 4.0;
const AIMBOT_STRIKE_Y_MIN: f64 = 28.0;
const AIMBOT_STRIKE_Y_SPAN: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
struct AimbotController {
    enabled: bool,
    swipe: Option<AimbotSwipe>,
    rng: u64,
}

impl AimbotController {
    const fn new() -> Self {
        Self {
            enabled: false,
            swipe: None,
            rng: AIMBOT_RNG_SEED,
        }
    }

    fn reset_for_new_game(&mut self) {
        self.enabled = false;
        self.swipe = None;
        self.rng = AIMBOT_RNG_SEED;
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.swipe = None;
        }
    }

    fn toggle(&mut self) {
        self.set_enabled(!self.enabled);
    }

    fn clear_swipe(&mut self) {
        self.swipe = None;
    }

    fn update_swipe(&mut self, incoming: bool, world: &World) {
        if incoming {
            if self.swipe.is_none() {
                self.swipe = Some(self.next_swipe(world));
            }
        } else {
            self.swipe = None;
        }
    }

    fn next_swipe(&mut self, world: &World) -> AimbotSwipe {
        let bits = self.next_random();
        select_aimbot_swipe(world, bits)
    }

    fn next_random(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }
}

/// Sounds the bin layer plays — exported linkage names from frame_44.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundId {
    /// Left/right wall bounce.
    WallBounce1,
    /// Top/bottom wall bounce.
    WallBounce2,
    /// Player return *and* serve.
    PPaddleBounce,
    /// Enemy return.
    EPaddleBounce,
    /// Either side misses.
    Miss,
}

/// Runtime presentation mode.
///
/// `Faithful` keeps the original 30 Hz app/world tick. `Silky` runs the
/// app/world at a non-faithful 400 Hz while preserving the original wall-clock
/// speed of Flash-frame counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Faithful,
    Silky,
}

impl VisualMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Faithful => "FAITHFUL",
            Self::Silky => "SILKY",
        }
    }

    #[must_use]
    pub const fn smooths_cosmetics(self) -> bool {
        matches!(self, Self::Silky)
    }

    #[must_use]
    pub const fn tick_hz(self) -> u32 {
        match self {
            Self::Faithful => TICK_HZ,
            Self::Silky => SILKY_PHYSICS_HZ,
        }
    }

    #[must_use]
    pub const fn flash_frame_scale(self) -> f32 {
        match self {
            Self::Faithful => 1.0,
            Self::Silky => TICK_HZ as f32 / SILKY_PHYSICS_HZ as f32,
        }
    }

    const fn toggled(self) -> Self {
        match self {
            Self::Faithful => Self::Silky,
            Self::Silky => Self::Faithful,
        }
    }
}

/// Main-timeline position. Counters inside variants are explained in
/// `App::tick`; transitions reproduce the original `gotoAndStop` routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Title,
    HighScores,
    /// Frames 36–44; `tick` counts 1..=9, the init action runs at 9 (frame 44).
    StartGameInit {
        tick: u32,
    },
    /// Frames 45–90; `tick` counts 1..=46. The "Level N" text shows for ticks
    /// 1..=45; tick 46 (frame 90) removes it, reveals the HUD, and applies
    /// the per-level setup.
    LevelSplash {
        tick: u32,
    },
    /// Frames 91..=96, holding at 96. Frame 91 places the enemy, 92 the ball
    /// and ring; the serve and rally both happen while stopped at 96.
    Playing {
        frame: u32,
    },
    /// The ball sprite's pop: 19 ticks, then the frame-20 routing action.
    /// The sim keeps running (pop serves still score — quirk Q2).
    Miss {
        tick: u32,
    },
    /// Frames 97–103; `tick` counts 1..=7, then the winner check routes.
    GameOver {
        tick: u32,
    },
    NameEntry,
    End,
}

/// A running pip-flash animation on one paddle (`tick` indexes §7.4 tables).
#[derive(Debug, Clone, Copy)]
pub struct PipFlash {
    pub zone: Zone,
    pub tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    Accuracy,
    Curve,
    SuperCurve,
}

impl BannerKind {
    #[must_use]
    pub const fn text_upper(self) -> &'static str {
        match self {
            Self::Accuracy => "ACCURACY BONUS!",
            Self::Curve => "CURVE BONUS!",
            Self::SuperCurve => "SUPER CURVE BONUS!",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Classic,
    Zen,
}

impl GameMode {
    const fn unlimited_player_lives(self) -> bool {
        matches!(self, Self::Zen)
    }
}

/// A running bonus-banner animation (`tick` indexes the §7.5 table).
#[derive(Debug, Clone, Copy)]
pub struct Banner {
    pub kind: BannerKind,
    pub tick: u32,
}

#[derive(Debug, Clone)]
pub struct NameEntry {
    pub text: String,
    pub edited: bool,
}

impl NameEntry {
    fn new() -> Self {
        Self {
            text: NAME_PLACEHOLDER.to_owned(),
            edited: false,
        }
    }

    fn type_char(&mut self, c: char) {
        if !self.edited {
            self.text.clear();
            self.edited = true;
        }
        if self.text.len() < NAME_MAX_LEN && (' '..='~').contains(&c) {
            self.text.push(c);
        }
    }

    fn backspace(&mut self) {
        if self.edited {
            self.text.pop();
        } else {
            self.text.clear();
            self.edited = true;
        }
    }
}

pub struct App {
    pub phase: Phase,
    /// Created at the StartGame init (frame 44) and kept until the next
    /// game's init; Game Over and the menus over it still read score/level/lives.
    pub world: Option<World>,
    pub player_flash: Option<PipFlash>,
    pub enemy_flash: Option<PipFlash>,
    /// `None` while the banner clip is absent (before the first level setup)
    /// or idle.
    pub banner: Option<Banner>,
    /// Set when Game Over blanks `bonusWord`/`bonusScore`; cleared by init.
    pub bonus_hud_blanked: bool,
    pub scores: ScoreTable,
    pub name_entry: NameEntry,
    pub mode: GameMode,
    pub visual_mode: VisualMode,
    aimbot: AimbotController,
    silky_frame_accum: u32,
    /// Free-running tick counter for the name-entry caret blink (deviation D5).
    pub caret_tick: u32,
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Phase::Title,
            world: None,
            player_flash: None,
            enemy_flash: None,
            banner: None,
            bonus_hud_blanked: false,
            scores: ScoreTable::load(),
            name_entry: NameEntry::new(),
            mode: GameMode::Classic,
            visual_mode: VisualMode::Faithful,
            aimbot: AimbotController::new(),
            silky_frame_accum: 0,
            caret_tick: 0,
        }
    }

    /// Advance one runtime tick. Faithful ticks at 30 Hz; Silky ticks at
    /// 400 Hz while scaling Flash-frame counters to preserve wall-clock speed.
    pub fn tick(&mut self, input: &TickInput) -> Vec<SoundId> {
        let mut sounds = Vec::new();
        self.caret_tick = self.caret_tick.wrapping_add(1);
        // Animation clocks advance before any of this tick's triggers so a
        // fresh trigger renders its first keyframe this tick (Flash's
        // gotoAndStop-then-play lands on the label frame immediately).
        self.advance_anims();

        match self.phase {
            Phase::Title => {
                for &click in &input.clicks {
                    if in_rect(click, BTN_TITLE_START) {
                        self.start_game(GameMode::Classic);
                        break;
                    }
                    if in_rect(click, BTN_TITLE_ZEN) {
                        self.start_game(GameMode::Zen);
                        break;
                    }
                    if in_rect(click, BTN_TITLE_VISUAL) {
                        self.toggle_visual_mode();
                        continue;
                    }
                    if in_rect(click, BTN_TITLE_SCORES) {
                        self.enter_phase(Phase::HighScores);
                        break;
                    }
                }
            },
            Phase::HighScores => {
                if input.clicks.iter().any(|&c| in_rect(c, BTN_HS_MENU)) {
                    self.return_to_title();
                }
            },
            Phase::StartGameInit { tick } => {
                if self.flash_frame_due() {
                    let t = tick + 1;
                    if t == START_GAME_TICKS {
                        // frame_44 init: fresh world. D8 resets the title path by
                        // clearing `world`, but intra-game reroutes preserve the
                        // published parent-timeline variables.
                        let published = self
                            .world
                            .take()
                            .map_or_else(Published::default, |w| w.published);
                        let mut world = World::new(published);
                        world.set_unlimited_player_lives(self.mode.unlimited_player_lives());
                        self.world = Some(world);
                        self.bonus_hud_blanked = false;
                        self.player_flash = None;
                        self.enemy_flash = None;
                        self.banner = None;
                        self.enter_phase(Phase::LevelSplash { tick: 0 });
                    } else {
                        self.phase = Phase::StartGameInit { tick: t };
                    }
                }
            },
            Phase::LevelSplash { tick } => {
                // The player paddle is live (and mouse-tracking) throughout
                // the splash; serve clicks are no-ops with no ball on stage.
                let events = self.sim_tick(input, false);
                self.apply_events(&events, &mut sounds);
                if self.flash_frame_due() {
                    let t = tick + 1;
                    if t == SPLASH_TICKS {
                        if let Some(world) = &mut self.world {
                            world.level_setup();
                        }
                        self.enter_phase(Phase::Playing {
                            frame: FRAME_SPLASH_END,
                        });
                    } else {
                        self.phase = Phase::LevelSplash { tick: t };
                    }
                }
            },
            Phase::Playing { frame } => {
                let gameplay = self.consume_zen_tool_clicks(input);
                let events = self.sim_tick(&gameplay.input, gameplay.control_clicked);
                if self.apply_events(&events, &mut sounds) {
                    self.enter_phase(Phase::Miss { tick: 0 });
                } else if self.flash_frame_due() {
                    let f = (frame + 1).min(FRAME_PLAY_HOLD);
                    if let Some(world) = &mut self.world {
                        if f == FRAME_ENEMY_SPAWN && !world.has_enemy() {
                            world.spawn_enemy();
                        }
                        if f == FRAME_BALL_SPAWN && !world.has_ball() {
                            world.spawn_ball();
                        }
                    }
                    self.phase = Phase::Playing { frame: f };
                }
                if gameplay.toggle_visual_after_tick {
                    self.toggle_visual_mode();
                }
            },
            Phase::Miss { tick } => {
                // Paddles and ring keep running; the stopped ball skips its
                // entire enterFrame. Pop serves (Q2) arrive via the input phase.
                let gameplay = self.consume_zen_tool_clicks(input);
                let events = self.sim_tick(&gameplay.input, gameplay.control_clicked);
                self.apply_events(&events, &mut sounds);
                if self.flash_frame_due() {
                    let t = tick + 1;
                    if t == MISS_TICKS {
                        self.route_after_miss();
                    } else {
                        self.phase = Phase::Miss { tick: t };
                    }
                }
                if gameplay.toggle_visual_after_tick {
                    self.toggle_visual_mode();
                }
            },
            Phase::GameOver { tick } => {
                // No entity clips remain — the sim does not run. The banner
                // clip persists and keeps animating (handled above).
                if self.flash_frame_due() {
                    let t = tick + 1;
                    if t == GAME_OVER_TICKS {
                        let qualified = self
                            .world
                            .as_ref()
                            .is_some_and(|w| self.scores.qualifies(w.economy.score));
                        if qualified {
                            self.name_entry = NameEntry::new();
                            self.enter_phase(Phase::NameEntry);
                        } else {
                            self.enter_phase(Phase::End);
                        }
                    } else {
                        self.phase = Phase::GameOver { tick: t };
                    }
                }
            },
            Phase::NameEntry => {
                for &c in &input.chars {
                    self.name_entry.type_char(c);
                }
                for _ in 0..input.backspaces {
                    self.name_entry.backspace();
                }
                if input.clicks.iter().any(|&c| in_rect(c, BTN_SUBMIT)) {
                    // Navigate on regardless; only record when edited away
                    // from the placeholder (the original's guard).
                    if self.name_entry.text != NAME_PLACEHOLDER
                        && let Some(world) = &self.world
                    {
                        self.scores.insert(
                            self.name_entry.text.clone(),
                            world.level,
                            world.economy.score,
                        );
                        self.scores.save();
                    }
                    self.enter_phase(Phase::HighScores);
                }
            },
            Phase::End => {
                if input.clicks.iter().any(|&c| in_rect(c, BTN_END_MENU)) {
                    self.return_to_title();
                }
            },
        }
        sounds
    }

    #[must_use]
    pub const fn tick_hz(&self) -> u32 {
        self.visual_mode.tick_hz()
    }

    #[must_use]
    pub fn tick_dt(&self) -> f64 {
        1.0 / f64::from(self.tick_hz())
    }

    #[must_use]
    pub fn flash_phase(&self, elapsed_ticks: u32, alpha: f32) -> f32 {
        (elapsed_ticks as f32 + alpha) * self.visual_mode.flash_frame_scale()
    }

    #[must_use]
    pub fn caret_visible(&self) -> bool {
        let hz = self.tick_hz();
        self.caret_tick % hz < hz / 2
    }

    #[must_use]
    pub fn zen_tools_available(&self) -> bool {
        self.mode == GameMode::Zen
            && matches!(self.phase, Phase::Playing { .. } | Phase::Miss { .. })
    }

    #[must_use]
    pub const fn aimbot_enabled(&self) -> bool {
        self.aimbot.enabled
    }

    pub fn set_aimbot_enabled(&mut self, enabled: bool) {
        self.aimbot.set_enabled(enabled);
    }

    #[must_use]
    pub fn player_control_mouse(&self, mouse: (f64, f64)) -> (f64, f64) {
        if self.zen_tools_available()
            && self.aimbot.enabled
            && let Some(world) = &self.world
        {
            return aimbot_mouse(world, self.visual_mode, self.aimbot.swipe);
        }
        mouse
    }

    fn start_game(&mut self, mode: GameMode) {
        self.mode = mode;
        self.aimbot.reset_for_new_game();
        self.enter_phase(Phase::StartGameInit { tick: 1 });
    }

    fn return_to_title(&mut self) {
        self.enter_phase(Phase::Title);
        self.mode = GameMode::Classic;
        self.aimbot.reset_for_new_game();
        self.world = None;
        self.player_flash = None;
        self.enemy_flash = None;
        self.banner = None;
        self.bonus_hud_blanked = false;
        self.name_entry = NameEntry::new();
        self.caret_tick = 0;
    }

    fn consume_zen_tool_clicks(&mut self, input: &TickInput) -> GameplayInput {
        if !self.zen_tools_available() {
            return GameplayInput {
                input: input.clone(),
                control_clicked: false,
                toggle_visual_after_tick: false,
            };
        }

        let mut gameplay_input = TickInput {
            mouse: input.mouse,
            clicks: Vec::new(),
            chars: input.chars.clone(),
            backspaces: input.backspaces,
        };
        let mut control_clicked = false;
        let mut toggle_visual_after_tick = false;
        for &click in &input.clicks {
            if in_rect(click, BTN_GAME_SILKY) {
                toggle_visual_after_tick = true;
                control_clicked = true;
            } else if in_rect(click, BTN_GAME_AIMBOT) {
                self.aimbot.toggle();
                control_clicked = true;
            } else {
                gameplay_input.clicks.push(click);
            }
        }

        GameplayInput {
            input: gameplay_input,
            control_clicked,
            toggle_visual_after_tick,
        }
    }

    fn sim_tick(&mut self, input: &TickInput, suppress_auto_serve: bool) -> Vec<SimEvent> {
        let mut serve_clicks = input.clicks.len() as u32;
        if !suppress_auto_serve && self.aimbot_should_auto_serve() {
            serve_clicks = serve_clicks.saturating_add(1);
        }
        let mouse = self.player_control_mouse_for_tick(input.mouse);
        let input = SimInput {
            mouse,
            serve_clicks,
        };
        if self.visual_mode == VisualMode::Silky {
            self.world
                .as_mut()
                .map_or_else(Vec::new, |world| world.tick_silky_slice(&input))
        } else {
            self.world
                .as_mut()
                .map_or_else(Vec::new, |world| world.tick(&input))
        }
    }

    fn aimbot_should_auto_serve(&self) -> bool {
        self.zen_tools_available()
            && self.aimbot.enabled
            && matches!(self.phase, Phase::Playing { .. })
            && self.world.as_ref().is_some_and(|world| {
                world
                    .ball
                    .as_ref()
                    .is_some_and(|ball| !ball.just_spawned && !ball.stopped && ball.vel.z == 0.0)
            })
    }

    fn player_control_mouse_for_tick(&mut self, mouse: (f64, f64)) -> (f64, f64) {
        if !self.zen_tools_available() || !self.aimbot.enabled {
            self.aimbot.clear_swipe();
            return mouse;
        }
        self.update_aimbot_swipe();
        self.player_control_mouse(mouse)
    }

    fn update_aimbot_swipe(&mut self) {
        let Some(world) = self.world.as_ref() else {
            self.aimbot.clear_swipe();
            return;
        };
        self.aimbot.update_swipe(aimbot_ball_incoming(world), world);
    }

    fn flash_frame_due(&mut self) -> bool {
        if self.visual_mode == VisualMode::Faithful {
            return true;
        }
        self.silky_frame_accum += TICK_HZ;
        if self.silky_frame_accum < SILKY_PHYSICS_HZ {
            return false;
        }
        self.silky_frame_accum -= SILKY_PHYSICS_HZ;
        true
    }

    fn enter_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.reset_silky_frame_clock();
    }

    fn reset_silky_frame_clock(&mut self) {
        self.silky_frame_accum = 0;
    }

    fn toggle_visual_mode(&mut self) {
        self.visual_mode = self.visual_mode.toggled();
        self.reset_silky_frame_clock();
    }

    /// Map sim events to sounds and animation triggers. Returns whether a
    /// miss occurred (the caller enters the Miss phase).
    fn apply_events(&mut self, events: &[SimEvent], sounds: &mut Vec<SoundId>) -> bool {
        let mut missed = false;
        for event in events {
            match *event {
                SimEvent::WallBounce { horizontal } => {
                    sounds.push(if horizontal {
                        SoundId::WallBounce1
                    } else {
                        SoundId::WallBounce2
                    });
                },
                SimEvent::EnemyHit { zone } => {
                    sounds.push(SoundId::EPaddleBounce);
                    self.enemy_flash = Some(PipFlash { zone, tick: 0 });
                },
                SimEvent::PlayerHit {
                    zone,
                    accuracy,
                    curve,
                }
                | SimEvent::Serve {
                    zone,
                    accuracy,
                    curve,
                } => {
                    sounds.push(SoundId::PPaddleBounce);
                    self.player_flash = Some(PipFlash { zone, tick: 0 });
                    // When one contact awards both, the curve banner triggers
                    // last and wins (its restart overwrites the accuracy one).
                    if accuracy {
                        self.trigger_banner(BannerKind::Accuracy);
                    }
                    match curve {
                        CurveClass::Curve => self.trigger_banner(BannerKind::Curve),
                        CurveClass::SuperCurve => self.trigger_banner(BannerKind::SuperCurve),
                        CurveClass::None => {},
                    }
                },
                SimEvent::PlayerMiss | SimEvent::EnemyMiss => {
                    sounds.push(SoundId::Miss);
                    missed = true;
                },
            }
        }
        missed
    }

    /// Sprite-80 frame-20 routing at the end of the pop.
    fn route_after_miss(&mut self) {
        let Some(world) = &mut self.world else {
            self.enter_phase(Phase::Title);
            return;
        };
        if world.enemy_lives < 1 {
            world.route_level_up();
            // The backwards goto to the Level label removes the enemy and the
            // HUD (banner included); the player paddle persists (quirk Q6).
            self.enemy_flash = None;
            self.banner = None;
            // This routing tick renders frame 45 — splash tick 1.
            self.enter_phase(Phase::LevelSplash { tick: 1 });
        } else if world.player_lives < 1 && !self.mode.unlimited_player_lives() {
            // Frame 97 removes the paddles, ball, and ring; the HUD persists
            // with the bonus strings blanked. The banner clip persists too.
            world.clear_entities_for_game_over();
            self.player_flash = None;
            self.enemy_flash = None;
            self.bonus_hud_blanked = true;
            self.enter_phase(Phase::GameOver { tick: 1 });
        } else {
            // Re-serve: gotoAndStop("Serve") lands on frame 91 — the ball and
            // ring are removed and respawn fresh at 92; the enemy persists.
            world.clear_ball_for_reserve();
            self.enter_phase(Phase::Playing {
                frame: FRAME_ENEMY_SPAWN,
            });
        }
    }

    /// Restart the banner animation (Flash's gotoAndStop("bonus") + play()).
    fn trigger_banner(&mut self, kind: BannerKind) {
        self.banner = Some(Banner { kind, tick: 0 });
    }

    fn advance_anims(&mut self) {
        advance_flash(&mut self.player_flash, self.visual_mode);
        advance_flash(&mut self.enemy_flash, self.visual_mode);
        if let Some(banner) = &mut self.banner {
            banner.tick += 1;
            if animation_complete(banner.tick, BANNER_TICKS, self.visual_mode) {
                self.banner = None;
            }
        }
    }
}

fn advance_flash(slot: &mut Option<PipFlash>, visual_mode: VisualMode) {
    if let Some(flash) = slot {
        flash.tick += 1;
        if animation_complete(flash.tick, PIP_FLASH_TICKS, visual_mode) {
            *slot = None;
        }
    }
}

fn animation_complete(elapsed_ticks: u32, flash_frames: u32, visual_mode: VisualMode) -> bool {
    match visual_mode {
        VisualMode::Faithful => elapsed_ticks >= flash_frames,
        VisualMode::Silky => {
            elapsed_ticks.saturating_mul(TICK_HZ) >= flash_frames.saturating_mul(SILKY_PHYSICS_HZ)
        },
    }
}

fn select_aimbot_swipe(world: &World, bits: u64) -> AimbotSwipe {
    let ball_pos = world
        .ball
        .as_ref()
        .map_or((WORLD_CX, WORLD_CY), |ball| (ball.pos.x, ball.pos.y));
    let enemy_pos = world
        .enemy
        .as_ref()
        .map_or((WORLD_CX, WORLD_CY), |enemy| enemy.pos);
    let radius = BALL_DIAMETER / 2.0;
    let targets = [
        (WORLD_LEFT + radius, WORLD_TOP + radius),
        (WORLD_RIGHT - radius, WORLD_TOP + radius),
        (WORLD_LEFT + radius, WORLD_BOTTOM - radius),
        (WORLD_RIGHT - radius, WORLD_BOTTOM - radius),
    ];

    let mut best_target = targets[0];
    let mut best_score = f64::NEG_INFINITY;
    for (index, target) in targets.into_iter().enumerate() {
        let dx = target.0 - enemy_pos.0;
        let dy = target.1 - enemy_pos.1;
        let tie_break = random_unit(bits, 8 + index as u32 * 8);
        let score = dx.mul_add(dx, dy * dy) + tie_break;
        if score > best_score {
            best_score = score;
            best_target = target;
        }
    }

    aimbot_swipe_for_target(ball_pos, best_target, bits)
}

fn aimbot_swipe_for_target(ball_pos: (f64, f64), target: (f64, f64), bits: u64) -> AimbotSwipe {
    let target_x = nonzero_sign(target.0 - ball_pos.0, bits & (1_u64 << 32) == 0);
    let target_y = nonzero_sign(target.1 - ball_pos.1, bits & (1_u64 << 33) == 0);
    let speed_sign = (-target_x, -target_y);
    let strike_x = AIMBOT_STRIKE_X_MIN + random_unit(bits, 40) * AIMBOT_STRIKE_X_SPAN;
    let strike_y = AIMBOT_STRIKE_Y_MIN + random_unit(bits, 48) * AIMBOT_STRIKE_Y_SPAN;

    AimbotSwipe {
        windup_offset: (
            -speed_sign.0 * AIMBOT_WINDUP_OVERSHOOT,
            -speed_sign.1 * AIMBOT_WINDUP_OVERSHOOT,
        ),
        strike_offset: (speed_sign.0 * strike_x, speed_sign.1 * strike_y),
    }
}

fn nonzero_sign(value: f64, positive_tie: bool) -> f64 {
    if value < 0.0 {
        -1.0
    } else if value > 0.0 || positive_tie {
        1.0
    } else {
        -1.0
    }
}

fn aimbot_mouse(world: &World, visual_mode: VisualMode, swipe: Option<AimbotSwipe>) -> (f64, f64) {
    let desired = level_11_cpu_desired_pos(world, visual_mode, swipe);
    let alpha = player_ease_alpha(visual_mode);
    if alpha <= 0.0 {
        return desired;
    }
    let current = world.paddle.pos;
    (
        (desired.0 - current.0) / alpha + current.0,
        (desired.1 - current.1) / alpha + current.1,
    )
}

fn level_11_cpu_desired_pos(
    world: &World,
    visual_mode: VisualMode,
    swipe: Option<AimbotSwipe>,
) -> (f64, f64) {
    let (target, divisor) = aimbot_target_and_divisor(world, visual_mode, swipe);
    let current = world.paddle.pos;
    let mut desired = if divisor <= 0.0 {
        target
    } else {
        let alpha = match visual_mode {
            VisualMode::Faithful => 1.0 / divisor,
            VisualMode::Silky => 1.0 - (1.0 - 1.0 / divisor).powf(SILKY_DT_SCALE),
        };
        (
            (target.0 - current.0).mul_add(alpha, current.0),
            (target.1 - current.1).mul_add(alpha, current.1),
        )
    };
    clamp_to_world(&mut desired);
    desired
}

fn aimbot_target_and_divisor(
    world: &World,
    visual_mode: VisualMode,
    swipe: Option<AimbotSwipe>,
) -> ((f64, f64), f64) {
    let cpu_skill = level_params(11).skill;
    let home = (WORLD_CX, WORLD_CY);
    let Some(ball) = &world.ball else {
        return (home, ENEMY_EASE_HOME);
    };
    if !ball.stopped && ball.vel.z == 0.0 {
        return ((ball.pos.x, ball.pos.y), cpu_skill);
    }
    if !ball.stopped && world.published.dir.z < 0.0 {
        let mut target = (world.published.pos.x, world.published.pos.y);
        if let Some(swipe) = swipe {
            let (dx, dy) = aimbot_swipe_offset(world, visual_mode, swipe);
            target.0 += dx;
            target.1 += dy;
        }
        return (target, cpu_skill);
    }
    (home, ENEMY_EASE_HOME)
}

fn aimbot_ball_incoming(world: &World) -> bool {
    world
        .ball
        .as_ref()
        .is_some_and(|ball| !ball.stopped && ball.vel.z < 0.0 && world.published.dir.z < 0.0)
}

fn aimbot_swipe_offset(world: &World, visual_mode: VisualMode, swipe: AimbotSwipe) -> (f64, f64) {
    let frames_to_contact = world
        .ball
        .as_ref()
        .and_then(|ball| {
            if ball.vel.z < 0.0 {
                Some((ball.pos.z / -ball.vel.z).max(0.0))
            } else {
                None
            }
        })
        .unwrap_or(AIMBOT_SWIPE_START_FRAMES);

    let start = swipe.windup_offset;
    let end = swipe.strike_offset;
    let tick_frames = aimbot_tick_frame_width(visual_mode);
    let contact_frames = tick_frames * AIMBOT_SWIPE_CONTACT_TICKS;
    let lunge_frames = tick_frames * AIMBOT_SWIPE_LUNGE_TICKS;
    if frames_to_contact >= AIMBOT_SWIPE_START_FRAMES {
        start
    } else if frames_to_contact <= 0.0 || frames_to_contact < contact_frames {
        end
    } else if frames_to_contact >= lunge_frames {
        start
    } else {
        let t = (lunge_frames - frames_to_contact) / (lunge_frames - contact_frames);
        let smooth = t * t * 2.0_f64.mul_add(-t, 3.0);
        (
            (end.0 - start.0).mul_add(smooth, start.0),
            (end.1 - start.1).mul_add(smooth, start.1),
        )
    }
}

fn aimbot_tick_frame_width(visual_mode: VisualMode) -> f64 {
    match visual_mode {
        VisualMode::Faithful => 1.0,
        VisualMode::Silky => SILKY_DT_SCALE,
    }
}

fn player_ease_alpha(visual_mode: VisualMode) -> f64 {
    match visual_mode {
        VisualMode::Faithful => 1.0 / PLAYER_EASE,
        VisualMode::Silky => 1.0 - (1.0 - 1.0 / PLAYER_EASE).powf(SILKY_DT_SCALE),
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn in_rect(p: (f64, f64), r: (f64, f64, f64, f64)) -> bool {
    (r.0..=r.2).contains(&p.0) && (r.1..=r.3).contains(&p.1)
}

fn random_unit(bits: u64, shift: u32) -> f64 {
    let byte = ((bits >> shift) & 0xff) as u8;
    f64::from(byte) / 255.0
}
