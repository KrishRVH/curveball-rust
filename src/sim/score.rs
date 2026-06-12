//! Scoring economy — frame_44 init, frame_90 per-level reset, frame_92 awards.
//!
//! Award/degrade pairs all follow the same shape in the source: add the
//! current bonus to the score, subtract the degrade, floor at zero. The
//! bonus-drain counter (`world.bonus`) decrements every tick the ball is in
//! flight while `bonusDisplay > 0`; when it goes below zero it resets to 10
//! and `bonusDisplay` drops by 25 — one step every 11 flight ticks. The
//! reset-check sits outside the flight gate exactly as in the source. The
//! counter is reset only at level setup, never between rallies (quirk Q11).

use crate::consts::{
    ACCURACY_BONUS_INIT, ACCURACY_DEGRADE, BONUS_COUNTER_INIT, BONUS_DISPLAY_INIT,
    BONUS_DRAIN_STEP, CURVE_BONUS_INIT, CURVE_CLASS_HI, CURVE_CLASS_LO, CURVE_DEGRADE, HIT_DEGRADE,
    HIT_SCORE_INIT, SUPER_CURVE_BONUS_INIT, SUPER_CURVE_DEGRADE,
};

/// Outcome of the curve-bonus classification (§3.7 branch order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveClass {
    None,
    Curve,
    SuperCurve,
}

#[derive(Debug, Clone, Copy)]
pub struct Economy {
    pub score: i64,
    pub hit_score: i32,
    pub curve_bonus: i32,
    pub super_curve_bonus: i32,
    pub accuracy_bonus: i32,
    pub bonus_display: i32,
    /// `world.bonus` — the 11-tick drain counter.
    pub bonus_counter: i32,
}

impl Economy {
    /// frame_44 initial values (also matches the frame_90 per-level reset).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            score: 0,
            hit_score: HIT_SCORE_INIT,
            curve_bonus: CURVE_BONUS_INIT,
            super_curve_bonus: SUPER_CURVE_BONUS_INIT,
            accuracy_bonus: ACCURACY_BONUS_INIT,
            bonus_display: BONUS_DISPLAY_INIT,
            bonus_counter: BONUS_COUNTER_INIT,
        }
    }

    /// frame_90 per-level setup: rally bonuses, `bonusDisplay`, and the drain
    /// counter all reset (score and lives are untouched there).
    pub const fn level_setup(&mut self) {
        self.hit_score = HIT_SCORE_INIT;
        self.curve_bonus = CURVE_BONUS_INIT;
        self.super_curve_bonus = SUPER_CURVE_BONUS_INIT;
        self.accuracy_bonus = ACCURACY_BONUS_INIT;
        self.bonus_display = BONUS_DISPLAY_INIT;
        self.bonus_counter = BONUS_COUNTER_INIT;
    }

    /// Player-miss reset: rally bonuses return to initial values;
    /// `bonusDisplay` is deliberately *not* reset (quirk Q8).
    pub const fn reset_rally_bonuses(&mut self) {
        self.hit_score = HIT_SCORE_INIT;
        self.curve_bonus = CURVE_BONUS_INIT;
        self.super_curve_bonus = SUPER_CURVE_BONUS_INIT;
        self.accuracy_bonus = ACCURACY_BONUS_INIT;
    }

    /// Every player-paddle return (not serves): award `hitScore`, degrade by 10.
    pub const fn award_hit(&mut self) {
        self.score += self.hit_score as i64;
        self.hit_score -= HIT_DEGRADE;
        if self.hit_score < 0 {
            self.hit_score = 0;
        }
    }

    /// Center-zone hit or serve: award `accuracyBonus`, degrade by 10.
    pub const fn award_accuracy(&mut self) {
        self.score += self.accuracy_bonus as i64;
        self.accuracy_bonus -= ACCURACY_DEGRADE;
        if self.accuracy_bonus < 0 {
            self.accuracy_bonus = 0;
        }
    }

    /// Curve-bonus classification and award, applied to the just-assigned
    /// curve values. Exact branch order from the source:
    ///
    /// ```text
    /// if |cx| > 0.1 { if |cy| > 0.1 → SUPER else → CURVE }
    /// else if |cy| > 0.05 → CURVE
    /// else if |cx| > 0.05 → CURVE
    /// ```
    pub const fn award_curve(&mut self, cx: f64, cy: f64) -> CurveClass {
        if CURVE_CLASS_HI < cx.abs() {
            if CURVE_CLASS_HI < cy.abs() {
                self.score += self.super_curve_bonus as i64;
                self.super_curve_bonus -= SUPER_CURVE_DEGRADE;
                if self.super_curve_bonus < 0 {
                    self.super_curve_bonus = 0;
                }
                CurveClass::SuperCurve
            } else {
                self.award_curve_bonus();
                CurveClass::Curve
            }
        } else if CURVE_CLASS_LO < cy.abs() || CURVE_CLASS_LO < cx.abs() {
            self.award_curve_bonus();
            CurveClass::Curve
        } else {
            CurveClass::None
        }
    }

    const fn award_curve_bonus(&mut self) {
        self.score += self.curve_bonus as i64;
        self.curve_bonus -= CURVE_DEGRADE;
        if self.curve_bonus < 0 {
            self.curve_bonus = 0;
        }
    }

    /// End-of-ball-tick drain. Runs on every tick the ball's enterFrame body
    /// runs (i.e. whenever the ball is not stopped), with `in_flight` =
    /// `vel.z != 0`. Shape mirrors the source: only flight decrements, but the
    /// reset-check runs unconditionally.
    pub const fn drain_tick(&mut self, in_flight: bool) {
        if in_flight && self.bonus_display > 0 {
            self.bonus_counter -= 1;
        }
        if self.bonus_counter < 0 {
            self.bonus_counter = BONUS_COUNTER_INIT;
            self.bonus_display -= BONUS_DRAIN_STEP;
        }
    }
}

impl Default for Economy {
    fn default() -> Self {
        Self::new()
    }
}
