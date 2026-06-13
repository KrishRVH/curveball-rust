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
    frame_samples: Vec<Duration>,
    total_ticks: u64,
    max_ticks_per_frame: u32,
    frames_without_ticks: u64,
    mode: Option<&'static str>,
    mode_switches: u32,
    tick_dt: f64,
    pending_tick_debt: f64,
    residual_tick_debt: f64,
    max_pending_tick_debt: f64,
    max_residual_tick_debt: f64,
}

pub struct FrameSample {
    pub frame: Duration,
    pub latch: Duration,
    pub tick: Duration,
    pub scene: Duration,
    pub blit: Duration,
    pub wait: Duration,
    pub ticks_this_frame: u32,
    pub mode: &'static str,
    pub tick_dt: f64,
    pub pending_tick_debt: f64,
    pub residual_tick_debt: f64,
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
            frame_samples: Vec::new(),
            total_ticks: 0,
            max_ticks_per_frame: 0,
            frames_without_ticks: 0,
            mode: None,
            mode_switches: 0,
            tick_dt: 0.0,
            pending_tick_debt: 0.0,
            residual_tick_debt: 0.0,
            max_pending_tick_debt: 0.0,
            max_residual_tick_debt: 0.0,
        })
    }

    pub fn record(&mut self, sample: FrameSample) -> bool {
        self.frames += 1;
        self.total += sample.frame;
        self.latch += sample.latch;
        self.tick += sample.tick;
        self.scene += sample.scene;
        self.blit += sample.blit;
        self.wait += sample.wait;
        self.max_frame = self.max_frame.max(sample.frame);
        self.frame_samples.push(sample.frame);
        self.total_ticks += u64::from(sample.ticks_this_frame);
        self.max_ticks_per_frame = self.max_ticks_per_frame.max(sample.ticks_this_frame);
        if sample.ticks_this_frame == 0 {
            self.frames_without_ticks += 1;
        }
        if self.mode.is_some_and(|mode| mode != sample.mode) {
            self.mode_switches += 1;
        }
        self.mode = Some(sample.mode);
        self.tick_dt = sample.tick_dt;
        self.pending_tick_debt += sample.pending_tick_debt;
        self.residual_tick_debt += sample.residual_tick_debt;
        self.max_pending_tick_debt = self.max_pending_tick_debt.max(sample.pending_tick_debt);
        self.max_residual_tick_debt = self.max_residual_tick_debt.max(sample.residual_tick_debt);
        self.frames >= self.limit
    }

    pub fn report(&self) {
        let frames = self.frames.max(1) as f64;
        let total = self.total.as_secs_f64();
        let mut frame_samples = self.frame_samples.clone();
        frame_samples.sort_unstable();
        eprintln!(
            "curveball perf: frames={} elapsed={:.3}s fps={:.1} avg_frame={:.3}ms p95_frame={:.3}ms p99_frame={:.3}ms max_frame={:.3}ms",
            self.frames,
            total,
            frames / total.max(f64::EPSILON),
            millis(self.total) / frames,
            millis(percentile(&frame_samples, 0.95)),
            millis(percentile(&frame_samples, 0.99)),
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
        eprintln!(
            "curveball perf: avg_ticks_per_frame={:.2} max_ticks_per_frame={} frames_without_ticks={}",
            self.total_ticks as f64 / frames,
            self.max_ticks_per_frame,
            self.frames_without_ticks,
        );
        eprintln!(
            "curveball perf: mode={} effective_tick_hz={:.1} mode_switches={} avg_pending_tick_debt={:.3}ms max_pending_tick_debt={:.3}ms avg_residual_tick_debt={:.3}ms max_residual_tick_debt={:.3}ms",
            self.mode.unwrap_or("UNKNOWN"),
            1.0 / self.tick_dt.max(f64::EPSILON),
            self.mode_switches,
            self.pending_tick_debt * 1000.0 / frames,
            self.max_pending_tick_debt * 1000.0,
            self.residual_tick_debt * 1000.0 / frames,
            self.max_residual_tick_debt * 1000.0,
        );
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn percentile(sorted_samples: &[Duration], percentile: f64) -> Duration {
    if sorted_samples.is_empty() {
        return Duration::ZERO;
    }
    let rank = ((sorted_samples.len() - 1) as f64 * percentile).round() as usize;
    sorted_samples[rank.min(sorted_samples.len() - 1)]
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
