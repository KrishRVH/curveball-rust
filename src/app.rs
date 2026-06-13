//! Top-level game: the timeline phase machine (PLAN.md §9), animation clocks,
//! event → sound/flash/banner mapping, and high-score persistence.
//!
//! Phases mirror the main-timeline labels and their frame-accurate timings
//! (all at 30 Hz): StartGame init spans frames 36–44, the Level splash 45–90,
//! the Serve/Rally hold sits at frame 96, the Miss pop runs 19 ticks inside
//! the ball sprite, and Game Over spans frames 97–103 before routing to name
//! entry or the end screen.

use crate::consts::{
    BTN_END_MENU, BTN_HS_MENU, BTN_SUBMIT, BTN_TITLE_SCORES, BTN_TITLE_SOUND, BTN_TITLE_START,
    BTN_TITLE_VISUAL, BTN_TITLE_ZEN, FRAME_BALL_SPAWN, FRAME_ENEMY_SPAWN, FRAME_PLAY_HOLD,
    FRAME_SPLASH_END, GAME_OVER_TICKS, MISS_TICKS, NAME_MAX_LEN, NAME_PLACEHOLDER,
    SILKY_PHYSICS_HZ, SPLASH_TICKS, START_GAME_TICKS, TICK_HZ,
};
use crate::highscores::ScoreTable;
use crate::sim::{CurveClass, Published, SimEvent, SimInput, World, Zone};

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

/// Runtime sound asset set. `Faithful` is the extracted SWF audio; `Modern`
/// keeps the same event mapping while using the recreated 48 kHz clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundSet {
    Faithful,
    Modern,
}

impl SoundSet {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Faithful => "FAITHFUL",
            Self::Modern => "MODERN",
        }
    }

    const fn toggled(self) -> Self {
        match self {
            Self::Faithful => Self::Modern,
            Self::Modern => Self::Faithful,
        }
    }
}

/// Runtime presentation mode. `Faithful` keeps the original 30 Hz world math;
/// `Silky` runs non-faithful 400 Hz world substeps and blends render-only
/// animation keyframes between fixed app ticks.
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
    pub const fn text(self) -> &'static str {
        match self {
            Self::Accuracy => "Accuracy Bonus!",
            Self::Curve => "Curve Bonus!",
            Self::SuperCurve => "Super Curve Bonus!",
        }
    }

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
    pub sound_set: SoundSet,
    pub visual_mode: VisualMode,
    silky_substep_accum: u32,
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
            sound_set: SoundSet::Faithful,
            visual_mode: VisualMode::Faithful,
            silky_substep_accum: 0,
            caret_tick: 0,
        }
    }

    /// Advance one 30 Hz tick. Returns the sounds to start this tick.
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
                    if in_rect(click, BTN_TITLE_SOUND) {
                        self.sound_set = self.sound_set.toggled();
                        continue;
                    }
                    if in_rect(click, BTN_TITLE_VISUAL) {
                        self.visual_mode = self.visual_mode.toggled();
                        continue;
                    }
                    if in_rect(click, BTN_TITLE_SCORES) {
                        self.phase = Phase::HighScores;
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
                    world.unlimited_player_lives = self.mode.unlimited_player_lives();
                    self.world = Some(world);
                    self.bonus_hud_blanked = false;
                    self.player_flash = None;
                    self.enemy_flash = None;
                    self.banner = None;
                    self.phase = Phase::LevelSplash { tick: 0 };
                } else {
                    self.phase = Phase::StartGameInit { tick: t };
                }
            },
            Phase::LevelSplash { tick } => {
                // The player paddle is live (and mouse-tracking) throughout
                // the splash; serve clicks are no-ops with no ball on stage.
                let events = self.sim_tick(input);
                self.apply_events(&events, &mut sounds);
                let t = tick + 1;
                if t == SPLASH_TICKS {
                    if let Some(world) = &mut self.world {
                        world.level_setup();
                    }
                    self.phase = Phase::Playing {
                        frame: FRAME_SPLASH_END,
                    };
                } else {
                    self.phase = Phase::LevelSplash { tick: t };
                }
            },
            Phase::Playing { frame } => {
                let events = self.sim_tick(input);
                if self.apply_events(&events, &mut sounds) {
                    self.phase = Phase::Miss { tick: 0 };
                } else {
                    let f = (frame + 1).min(FRAME_PLAY_HOLD);
                    if let Some(world) = &mut self.world {
                        if f == FRAME_ENEMY_SPAWN && world.enemy.is_none() {
                            world.spawn_enemy();
                        }
                        if f == FRAME_BALL_SPAWN && world.ball.is_none() {
                            world.spawn_ball();
                        }
                    }
                    self.phase = Phase::Playing { frame: f };
                }
            },
            Phase::Miss { tick } => {
                // Paddles and ring keep running; the stopped ball skips its
                // entire enterFrame. Pop serves (Q2) arrive via the input phase.
                let events = self.sim_tick(input);
                self.apply_events(&events, &mut sounds);
                let t = tick + 1;
                if t == MISS_TICKS {
                    self.route_after_miss();
                } else {
                    self.phase = Phase::Miss { tick: t };
                }
            },
            Phase::GameOver { tick } => {
                // No entity clips remain — the sim does not run. The banner
                // clip persists and keeps animating (handled above).
                let t = tick + 1;
                if t == GAME_OVER_TICKS {
                    let qualified = self
                        .world
                        .as_ref()
                        .is_some_and(|w| self.scores.qualifies(w.economy.score));
                    if qualified {
                        self.name_entry = NameEntry::new();
                        self.phase = Phase::NameEntry;
                    } else {
                        self.phase = Phase::End;
                    }
                } else {
                    self.phase = Phase::GameOver { tick: t };
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
                    self.phase = Phase::HighScores;
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

    fn start_game(&mut self, mode: GameMode) {
        self.mode = mode;
        self.silky_substep_accum = 0;
        self.phase = Phase::StartGameInit { tick: 1 };
    }

    fn return_to_title(&mut self) {
        self.phase = Phase::Title;
        self.mode = GameMode::Classic;
        self.world = None;
        self.player_flash = None;
        self.enemy_flash = None;
        self.banner = None;
        self.bonus_hud_blanked = false;
        self.name_entry = NameEntry::new();
        self.silky_substep_accum = 0;
        self.caret_tick = 0;
    }

    fn sim_tick(&mut self, input: &TickInput) -> Vec<SimEvent> {
        let input = SimInput {
            mouse: input.mouse,
            serve_clicks: input.clicks.len() as u32,
        };
        if self.visual_mode == VisualMode::Silky {
            let substeps = self.silky_substeps();
            return self
                .world
                .as_mut()
                .map_or_else(Vec::new, |world| world.tick_silky(&input, substeps));
        }
        self.world
            .as_mut()
            .map_or_else(Vec::new, |world| world.tick(&input))
    }

    fn silky_substeps(&mut self) -> u32 {
        self.silky_substep_accum += SILKY_PHYSICS_HZ;
        let substeps = self.silky_substep_accum / TICK_HZ;
        self.silky_substep_accum %= TICK_HZ;
        substeps
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
            self.phase = Phase::Title;
            return;
        };
        if world.enemy_lives < 1 {
            world.route_level_up();
            // The backwards goto to the Level label removes the enemy and the
            // HUD (banner included); the player paddle persists (quirk Q6).
            self.enemy_flash = None;
            self.banner = None;
            // This routing tick renders frame 45 — splash tick 1.
            self.phase = Phase::LevelSplash { tick: 1 };
        } else if world.player_lives < 1 && !self.mode.unlimited_player_lives() {
            // Frame 97 removes the paddles, ball, and ring; the HUD persists
            // with the bonus strings blanked. The banner clip persists too.
            world.ball = None;
            world.enemy = None;
            self.player_flash = None;
            self.enemy_flash = None;
            self.bonus_hud_blanked = true;
            self.phase = Phase::GameOver { tick: 1 };
        } else {
            // Re-serve: gotoAndStop("Serve") lands on frame 91 — the ball and
            // ring are removed and respawn fresh at 92; the enemy persists.
            world.ball = None;
            self.phase = Phase::Playing {
                frame: FRAME_ENEMY_SPAWN,
            };
        }
    }

    /// Restart the banner animation (Flash's gotoAndStop("bonus") + play()).
    fn trigger_banner(&mut self, kind: BannerKind) {
        self.banner = Some(Banner { kind, tick: 0 });
    }

    fn advance_anims(&mut self) {
        advance_flash(&mut self.player_flash);
        advance_flash(&mut self.enemy_flash);
        if let Some(banner) = &mut self.banner {
            banner.tick += 1;
            if banner.tick >= BANNER_TICKS {
                self.banner = None;
            }
        }
    }
}

fn advance_flash(slot: &mut Option<PipFlash>) {
    if let Some(flash) = slot {
        flash.tick += 1;
        if flash.tick >= PIP_FLASH_TICKS {
            *slot = None;
        }
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
