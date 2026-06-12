//! Menu screens: title, high scores, the Game Over overlay stack (text, name
//! entry, end screen). Layout anchors trace to the tag stream (PLAN.md §10).

use curveball::app::App;
use curveball::consts::{
    BTN_END_MENU, BTN_HS_MENU, BTN_TITLE_SCORES, BTN_TITLE_START, BTN_TITLE_ZEN, CONGRATS_BASELINE,
    CONGRATS_CX, CONGRATS_TEXT, END_MENU_LABEL, GAME_OVER_BASELINE, GAME_OVER_CX, HS_COL_LEVEL_CX,
    HS_COL_NAME_CX, HS_COL_SCORE_CX, HS_HEADER_BASELINE, HS_HEADER_LEVEL_CX, HS_HEADER_NAME_CX,
    HS_HEADER_SCORE_CX, HS_HEADING, HS_MENU_LABEL, HS_PANEL, HS_ROW_BASELINE, HS_ROW_STEP,
    HUD_FONT_PX, NAME_BASELINE, NAME_BOX, NAME_INPUT_CX, NAME_LABEL_X, SPLASH_FONT_PX,
    SUBMIT_LABEL, SUBMIT_RECT, TITLE_BASELINE, TITLE_CX, TITLE_SCORES_LABEL, TITLE_START_LABEL,
    TITLE_ZEN_LABEL,
};
use macroquad::prelude::*;

use super::entities::outline;
use super::text;

const MENU_TITLE_ASPECT: f32 = 0.9;
const MENU_HEADER_ASPECT: f32 = 0.95;
const MENU_LABEL_ASPECT: f32 = 0.88;
const MENU_ROW_ASPECT: f32 = 0.9;
const MENU_TITLE_FONT_PX: u16 = 28;
const MENU_LABEL_FONT_PX: u16 = 7;
const MENU_HEADING_FONT_PX: u16 = 8;
const MENU_HEADER_FONT_PX: u16 = 7;
const MENU_ROW_FONT_PX: u16 = 5;
const MENU_TITLE_TRACKING: f32 = 1.4;
const MENU_LABEL_TRACKING: f32 = 0.35;
const MENU_HEADER_TRACKING: f32 = 1.0;
const MENU_ROW_TRACKING: f32 = 0.4;
const PANEL_RADIUS: f32 = 5.0;

/// A white pill button (rounded rect) with a black label, from its hit rect.
fn draw_pill(rect: (f64, f64, f64, f64), label: &str, anchor: (f32, f32)) {
    let (x0, y0, x1, y1) = (rect.0 as f32, rect.1 as f32, rect.2 as f32, rect.3 as f32);
    let h = y1 - y0;
    let radius = h / 2.0;
    let cy = y0 + radius;
    draw_circle(x0 + radius, cy, radius, WHITE);
    draw_circle(x1 - radius, cy, radius, WHITE);
    draw_rectangle(x0 + radius, y0, (x1 - x0) - h, h, WHITE);
    text::centered_tracked_aspect(
        label,
        anchor.0,
        anchor.1,
        MENU_LABEL_FONT_PX,
        BLACK,
        MENU_LABEL_TRACKING,
        MENU_LABEL_ASPECT,
    );
}

fn rounded_panel(x: f32, y: f32, w: f32, h: f32, radius: f32) {
    draw_rectangle(x + radius, y, radius.mul_add(-2.0, w), h, BLACK);
    draw_rectangle(x, y + radius, radius, radius.mul_add(-2.0, h), BLACK);
    draw_rectangle(
        x + w - radius,
        y + radius,
        radius,
        radius.mul_add(-2.0, h),
        BLACK,
    );
    draw_circle(x + radius, y + radius, radius, BLACK);
    draw_circle(x + w - radius, y + radius, radius, BLACK);
    draw_circle(x + w - radius, y + h - radius, radius, BLACK);
    draw_circle(x + radius, y + h - radius, radius, BLACK);

    draw_line(x + radius, y, x + w - radius, y, 1.0, WHITE);
    draw_line(x + radius, y + h, x + w - radius, y + h, 1.0, WHITE);
    draw_line(x, y + radius, x, y + h - radius, 1.0, WHITE);
    draw_line(x + w, y + radius, x + w, y + h - radius, 1.0, WHITE);
    quarter_arc(
        x + radius,
        y + radius,
        radius,
        std::f32::consts::PI,
        1.5 * std::f32::consts::PI,
    );
    quarter_arc(
        x + w - radius,
        y + radius,
        radius,
        1.5 * std::f32::consts::PI,
        2.0 * std::f32::consts::PI,
    );
    quarter_arc(
        x + w - radius,
        y + h - radius,
        radius,
        0.0,
        0.5 * std::f32::consts::PI,
    );
    quarter_arc(
        x + radius,
        y + h - radius,
        radius,
        0.5 * std::f32::consts::PI,
        std::f32::consts::PI,
    );
}

fn quarter_arc(cx: f32, cy: f32, radius: f32, start: f32, end: f32) {
    let steps = 8;
    let mut prev = (
        radius.mul_add(start.cos(), cx),
        radius.mul_add(start.sin(), cy),
    );
    for i in 1..=steps {
        let t = (end - start).mul_add(i as f32 / steps as f32, start);
        let next = (radius.mul_add(t.cos(), cx), radius.mul_add(t.sin(), cy));
        draw_line(prev.0, prev.1, next.0, next.1, 1.0, WHITE);
        prev = next;
    }
}

fn display_centered(
    s: &str,
    cx: f32,
    baseline: f32,
    font_px: u16,
    color: Color,
    tracking: f32,
    aspect: f32,
) {
    text::centered_tracked_aspect(s, cx, baseline, font_px, color, tracking, aspect);
}

pub fn draw_title() {
    display_centered(
        "CURVEBALL",
        TITLE_CX,
        TITLE_BASELINE,
        MENU_TITLE_FONT_PX,
        WHITE,
        MENU_TITLE_TRACKING,
        MENU_TITLE_ASPECT,
    );
    draw_pill(BTN_TITLE_START, "START GAME", TITLE_START_LABEL);
    draw_pill(BTN_TITLE_SCORES, "HIGH SCORES", TITLE_SCORES_LABEL);
    draw_pill(BTN_TITLE_ZEN, "ZEN", TITLE_ZEN_LABEL);
}

pub fn draw_high_scores(app: &App) {
    let (x, y, w, h) = HS_PANEL;
    rounded_panel(x, y, w, h, PANEL_RADIUS);
    display_centered(
        "HIGH SCORES",
        HS_HEADING.0,
        HS_HEADING.1,
        MENU_HEADING_FONT_PX,
        WHITE,
        MENU_HEADER_TRACKING,
        MENU_HEADER_ASPECT,
    );
    display_centered(
        "LEVEL",
        HS_HEADER_LEVEL_CX,
        HS_HEADER_BASELINE,
        MENU_HEADER_FONT_PX,
        WHITE,
        MENU_HEADER_TRACKING,
        MENU_HEADER_ASPECT,
    );
    display_centered(
        "SCORE",
        HS_HEADER_SCORE_CX,
        HS_HEADER_BASELINE,
        MENU_HEADER_FONT_PX,
        WHITE,
        MENU_HEADER_TRACKING,
        MENU_HEADER_ASPECT,
    );
    display_centered(
        "NAME",
        HS_HEADER_NAME_CX,
        HS_HEADER_BASELINE,
        MENU_HEADER_FONT_PX,
        WHITE,
        MENU_HEADER_TRACKING,
        MENU_HEADER_ASPECT,
    );
    for (i, entry) in app.scores.entries.iter().enumerate() {
        if entry.name == "none" && entry.level == 0 && entry.score == 0 {
            continue;
        }
        let baseline = HS_ROW_STEP.mul_add(i as f32, HS_ROW_BASELINE);
        // Display formatting from the original placeholders: level
        // zero-padded to 2 digits, score to 9.
        let level = text::text_buf::<16>(format_args!("{:02}", entry.level));
        display_centered(
            level.as_str(),
            HS_COL_LEVEL_CX,
            baseline,
            MENU_ROW_FONT_PX,
            WHITE,
            MENU_ROW_TRACKING,
            MENU_ROW_ASPECT,
        );
        let score = text::text_buf::<32>(format_args!("{:09}", entry.score));
        display_centered(
            score.as_str(),
            HS_COL_SCORE_CX,
            baseline,
            MENU_ROW_FONT_PX,
            WHITE,
            MENU_ROW_TRACKING,
            MENU_ROW_ASPECT,
        );
        display_centered(
            &entry.name,
            HS_COL_NAME_CX,
            baseline,
            MENU_ROW_FONT_PX,
            WHITE,
            MENU_ROW_TRACKING,
            MENU_ROW_ASPECT,
        );
    }
    draw_pill(BTN_HS_MENU, "MAIN MENU", HS_MENU_LABEL);
}

/// Depth 43 at frame 97: the "Game Over" text (the HUD around it persists).
pub fn draw_game_over_text() {
    text::centered(
        "Game Over",
        GAME_OVER_CX,
        GAME_OVER_BASELINE,
        SPLASH_FONT_PX,
        WHITE,
    );
}

/// Depth 44 at frame 104: the name-entry box over the Game Over screen.
pub fn draw_name_entry(app: &App) {
    let (x, y, w, h) = NAME_BOX;
    draw_rectangle(x, y, w, h, BLACK);
    outline(x, y, w, h, WHITE);
    text::centered(
        CONGRATS_TEXT,
        CONGRATS_CX,
        CONGRATS_BASELINE,
        HUD_FONT_PX,
        WHITE,
    );
    text::left("Name:", NAME_LABEL_X, NAME_BASELINE, HUD_FONT_PX, WHITE);
    // Blinking caret while editing (deviation D5): 15 ticks on, 15 off.
    let caret = if app.caret_tick % 30 < 15 { "|" } else { "" };
    let shown = text::text_buf::<32>(format_args!("{}{caret}", app.name_entry.text));
    text::centered(
        shown.as_str(),
        NAME_INPUT_CX,
        NAME_BASELINE,
        HUD_FONT_PX,
        WHITE,
    );
    let (sx, sy, sw, sh) = SUBMIT_RECT;
    draw_rectangle(sx, sy, sw, sh, WHITE);
    text::centered("SUBMIT", SUBMIT_LABEL.0, SUBMIT_LABEL.1, HUD_FONT_PX, BLACK);
}

/// Depth 44 at frame 111: the end screen's main-menu button.
pub fn draw_end() {
    draw_pill(BTN_END_MENU, "MAIN MENU", END_MENU_LABEL);
}
