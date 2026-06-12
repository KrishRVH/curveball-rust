//! Debug-only deterministic render and state-warp helpers.

#[cfg(debug_assertions)]
use curveball::app::{App, TickInput};
#[cfg(debug_assertions)]
use curveball::consts::{
    BTN_TITLE_SCORES, BTN_TITLE_START, FRAME_PLAY_HOLD, START_GAME_TICKS, WORLD_CX, WORLD_CY,
};

#[cfg(debug_assertions)]
pub struct DebugShot {
    pub path: String,
    pub tick: u64,
}

/// Fast-forward the app to a named state with synthetic centered-mouse ticks.
#[cfg(debug_assertions)]
pub fn debug_warp(app: &mut App, state: &str, mouse: (f64, f64)) {
    use curveball::app::Phase;

    let center = (WORLD_CX, WORLD_CY);
    let rect_center = |r: (f64, f64, f64, f64)| (f64::midpoint(r.0, r.2), f64::midpoint(r.1, r.3));
    let tick = |app: &mut App, mouse: (f64, f64), clicks: Vec<(f64, f64)>| {
        app.tick(&TickInput {
            mouse,
            clicks,
            ..TickInput::default()
        });
    };
    if state == "highscores" {
        tick(app, mouse, vec![rect_center(BTN_TITLE_SCORES)]);
        return;
    }

    tick(app, mouse, vec![rect_center(BTN_TITLE_START)]);
    for _ in 0..START_GAME_TICKS {
        tick(app, if state == "serve" { mouse } else { center }, vec![]);
    }
    if state == "splash" {
        return;
    }

    while !matches!(
        app.phase,
        Phase::Playing {
            frame: FRAME_PLAY_HOLD
        }
    ) {
        tick(app, if state == "serve" { mouse } else { center }, vec![]);
    }
    if state == "serve" {
        return;
    }

    tick(app, center, vec![center]);
    if state == "rally" {
        for _ in 0..40 {
            tick(app, mouse, vec![]);
        }
        return;
    }
    if state == "miss" {
        while !matches!(app.phase, Phase::Miss { .. }) {
            tick(app, mouse, vec![]);
        }
        for _ in 0..5 {
            tick(app, mouse, vec![]);
        }
        return;
    }

    while !matches!(
        app.phase,
        Phase::GameOver { .. } | Phase::NameEntry | Phase::End
    ) {
        let serve = matches!(
            app.phase,
            Phase::Playing {
                frame: FRAME_PLAY_HOLD
            }
        ) && app.world.as_ref().is_some_and(|world| {
            world
                .ball
                .as_ref()
                .is_some_and(|ball| ball.vel.z == 0.0 && !ball.stopped)
        });
        tick(app, mouse, if serve { vec![mouse] } else { vec![] });
    }
}

#[cfg(debug_assertions)]
pub fn debug_shot() -> Option<DebugShot> {
    let spec = std::env::var("CURVEBALL_SHOT").ok()?;
    let (path, tick) = parse_shot_spec(&spec);
    if path.is_empty() {
        eprintln!("curveball: CURVEBALL_SHOT path is empty; capture disabled");
        return None;
    }
    Some(DebugShot {
        path: path.to_owned(),
        tick,
    })
}

#[cfg(debug_assertions)]
fn parse_shot_spec(spec: &str) -> (&str, u64) {
    if let Some((path, raw_tick)) = spec.rsplit_once(':')
        && !path.is_empty()
        && let Ok(tick) = raw_tick.parse::<u64>()
    {
        return (path, tick.max(1));
    }
    (spec, 30)
}

#[cfg(debug_assertions)]
pub fn fixed_mouse_from_env() -> Option<(f64, f64)> {
    let spec = std::env::var("CURVEBALL_MOUSE").ok()?;
    parse_mouse(&spec).or_else(|| {
        eprintln!(
            "curveball: invalid CURVEBALL_MOUSE '{spec}', expected finite x,y; using live mouse"
        );
        None
    })
}

#[cfg(debug_assertions)]
fn parse_mouse(spec: &str) -> Option<(f64, f64)> {
    let (x, y) = spec.split_once(',')?;
    let x = x.parse::<f64>().ok()?;
    let y = y.parse::<f64>().ok()?;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::{parse_mouse, parse_shot_spec};

    #[test]
    fn mouse_parser_accepts_finite_coordinates() {
        assert_eq!(parse_mouse("222.75,114"), Some((222.75, 114.0)));
    }

    #[test]
    fn mouse_parser_rejects_invalid_coordinates() {
        assert_eq!(parse_mouse("222.75"), None);
        assert_eq!(parse_mouse("x,114"), None);
        assert_eq!(parse_mouse("NaN,114"), None);
        assert_eq!(parse_mouse("1,inf"), None);
    }

    #[test]
    fn shot_parser_uses_numeric_suffix_as_tick_count() {
        assert_eq!(parse_shot_spec("/tmp/shot.png:45"), ("/tmp/shot.png", 45));
        assert_eq!(
            parse_shot_spec("C:/tmp/shot.png:45"),
            ("C:/tmp/shot.png", 45)
        );
    }

    #[test]
    fn shot_parser_keeps_colon_paths_without_numeric_suffix() {
        assert_eq!(parse_shot_spec("C:/tmp/shot.png"), ("C:/tmp/shot.png", 30));
        assert_eq!(
            parse_shot_spec("/tmp/shot.png:soon"),
            ("/tmp/shot.png:soon", 30)
        );
    }
}
