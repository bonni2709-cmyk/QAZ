//! Weather: the sandstorm cycle.
//!
//! Day/night lives in the driving state (it tints sky + ambient); this module
//! owns the storm roll — Clear → Incoming (warning, dust ramps up) → Storm
//! (gusts, heavy drag, near-whiteout streaks) → Clearing → Clear.
//!
//! The first storm lands ~40 s into a run; afterwards every 55–95 s.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeatherEvent {
    None,
    StormInbound,
    StormCleared,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WeatherPhase {
    Clear,
    Incoming,
    Storm,
    Clearing,
}

pub struct Weather {
    phase: WeatherPhase,
    /// Smoothed storm strength 0..1 (drives tinting, drag, HUD, overlay).
    intensity: f32,
    /// Signed gust noise, refreshed continuously. Already scaled by intensity.
    gust: f32,
    timer: f32,
    gust_seed: f32,
    rng: u64,
}

const GRACE_S: f32 = 40.0;
const INCOMING_S: f32 = 6.0;
const CLEARING_S: f32 = 5.0;
const CLEAR_MIN_S: f32 = 55.0;
const CLEAR_MAX_S: f32 = 95.0;

fn approach(cur: f32, target: f32, max_delta: f32) -> f32 {
    if cur < target {
        (cur + max_delta).min(target)
    } else {
        (cur - max_delta).max(target)
    }
}

impl Weather {
    pub fn new() -> Weather {
        Weather {
            phase: WeatherPhase::Clear,
            intensity: 0.0,
            gust: 0.0,
            timer: GRACE_S,
            gust_seed: 0.0,
            rng: 0xD1CE_5EED_1234_5678,
        }
    }

    fn rand_range(&mut self, lo: f32, hi: f32) -> f32 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let r = (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f32 / (1u64 << 53) as f32;
        lo + r * (hi - lo)
    }

    pub fn update(&mut self, dt: f32) -> WeatherEvent {
        let mut event = WeatherEvent::None;
        self.timer -= dt;
        self.gust_seed += dt;

        match self.phase {
            WeatherPhase::Clear => {
                self.intensity = approach(self.intensity, 0.0, 0.5 * dt);
                if self.timer <= 0.0 {
                    self.phase = WeatherPhase::Incoming;
                    self.timer = INCOMING_S;
                    event = WeatherEvent::StormInbound;
                }
            }
            WeatherPhase::Incoming => {
                self.intensity = approach(self.intensity, 0.42, 0.075 * dt);
                if self.timer <= 0.0 {
                    self.phase = WeatherPhase::Storm;
                    self.timer = self.rand_range(13.0, 21.0);
                }
            }
            WeatherPhase::Storm => {
                self.intensity = approach(self.intensity, 0.95, 0.35 * dt);
                if self.timer <= 0.0 {
                    self.phase = WeatherPhase::Clearing;
                    self.timer = CLEARING_S;
                    event = WeatherEvent::StormCleared;
                }
            }
            WeatherPhase::Clearing => {
                self.intensity = approach(self.intensity, 0.0, 0.22 * dt);
                if self.timer <= 0.0 {
                    self.phase = WeatherPhase::Clear;
                    self.timer = self.rand_range(CLEAR_MIN_S, CLEAR_MAX_S);
                }
            }
        }

        // layered sine gust — cheap, smooth, feels like buffeting wind
        let t = self.gust_seed;
        let raw = (t * 1.7).sin() * 0.6 + (t * 3.7 + 1.7).sin() * 0.4;
        self.gust = raw * self.intensity;

        event
    }

    /// Smoothed storm strength, 0..1.
    pub fn intensity(&self) -> f32 {
        self.intensity
    }

    /// Signed gust, already scaled by intensity. Use for shake / yaw drift.
    pub fn gust(&self) -> f32 {
        self.gust
    }
}
