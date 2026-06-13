//! Runtime timing probes and experimental simulation-rate parsing.

use std::time::{Duration, Instant};

pub struct PerfProbe {
    limit: u64,
    frames: u64,
    total: Duration,
    latch: Duration,
    tick: Duration,
    scene: Duration,
    blit: Duration,
    wait: Duration,
    max_frame: Duration,
}

impl PerfProbe {
    pub fn from_env() -> Option<Self> {
        let limit = std::env::var("CURVEBALL_PERF")
            .ok()?
            .parse::<u64>()
            .unwrap_or_else(|err| {
                eprintln!("curveball: invalid CURVEBALL_PERF value: {err}; using 300");
                300
            })
            .max(1);
        Some(Self {
            limit,
            frames: 0,
            total: Duration::ZERO,
            latch: Duration::ZERO,
            tick: Duration::ZERO,
            scene: Duration::ZERO,
            blit: Duration::ZERO,
            wait: Duration::ZERO,
            max_frame: Duration::ZERO,
        })
    }

    pub fn record(
        &mut self,
        frame: Duration,
        latch: Duration,
        tick: Duration,
        scene: Duration,
        blit: Duration,
        wait: Duration,
    ) -> bool {
        self.frames += 1;
        self.total += frame;
        self.latch += latch;
        self.tick += tick;
        self.scene += scene;
        self.blit += blit;
        self.wait += wait;
        self.max_frame = self.max_frame.max(frame);
        self.frames >= self.limit
    }

    pub fn report(&self) {
        let frames = self.frames.max(1) as f64;
        let total = self.total.as_secs_f64();
        eprintln!(
            "curveball perf: frames={} elapsed={:.3}s fps={:.1} avg_frame={:.3}ms max_frame={:.3}ms",
            self.frames,
            total,
            frames / total.max(f64::EPSILON),
            millis(self.total) / frames,
            millis(self.max_frame),
        );
        eprintln!(
            "curveball perf: avg latch={:.3}ms tick={:.3}ms scene={:.3}ms blit={:.3}ms wait={:.3}ms",
            millis(self.latch) / frames,
            millis(self.tick) / frames,
            millis(self.scene) / frames,
            millis(self.blit) / frames,
            millis(self.wait) / frames,
        );
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub fn sim_dt_override_from_env() -> Option<f64> {
    let raw = std::env::var("CURVEBALL_SIM_HZ").ok()?;
    let hz = parse_sim_hz(&raw).unwrap_or_else(|err| {
        eprintln!("curveball: invalid CURVEBALL_SIM_HZ value '{raw}': {err}; using mode default");
        0.0
    });
    if hz <= 0.0 {
        return None;
    }
    eprintln!(
        "curveball: CURVEBALL_SIM_HZ={hz} is an experimental non-faithful app/world cadence override"
    );
    Some(1.0 / hz)
}

fn parse_sim_hz(raw: &str) -> Result<f64, &'static str> {
    let hz = raw.parse::<f64>().map_err(|_| "expected a number")?;
    if hz.is_finite() && hz > 0.0 && (1.0 / hz).is_finite() {
        Ok(hz)
    } else {
        Err("expected a positive finite number")
    }
}

pub fn perf_now(perf: Option<&PerfProbe>) -> Option<Instant> {
    perf.map(|_| Instant::now())
}

pub fn perf_elapsed(start: Option<Instant>) -> Duration {
    start.map_or(Duration::ZERO, |start| start.elapsed())
}

#[cfg(test)]
mod tests {
    use super::parse_sim_hz;

    #[test]
    fn sim_hz_parser_accepts_positive_finite_values() {
        assert_eq!(parse_sim_hz("30"), Ok(30.0));
        assert_eq!(parse_sim_hz("144"), Ok(144.0));
        assert_eq!(parse_sim_hz("240.0"), Ok(240.0));
        assert_eq!(parse_sim_hz("400"), Ok(400.0));
    }

    #[test]
    fn sim_hz_parser_rejects_invalid_values() {
        assert!(parse_sim_hz("0").is_err());
        assert!(parse_sim_hz("-30").is_err());
        assert!(parse_sim_hz("NaN").is_err());
        assert!(parse_sim_hz("inf").is_err());
        assert!(parse_sim_hz("1e-309").is_err());
        assert!(parse_sim_hz("fast").is_err());
    }
}
