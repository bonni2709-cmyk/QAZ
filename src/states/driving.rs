//! Driving state — the endless highway run.
//!
//! Inputs (desktop): W/↑ gas · S/↓ brake/reverse · A,D/←,→ steer · Space (nothing yet, reserved for Module 2 weapons) · R reset · Esc pause.
//! Inputs (Android): AUTO-GAS at 72 % throttle, hold left/right screen third to steer.
//! The 3D scene is rendered with a Y-up chase camera; HUD is a 2D overlay.

use macroquad::prelude::*;

use crate::entities::truck::Truck;
use crate::systems::chase::ChaseCamera;
use crate::ui::{self, HudData};
use crate::world::road::{biome_at, draw_world, haze_color, Biome, ROAD_HALF};

/// Length of one full day/night cycle in seconds (game time runs fast).
const DAY_LENGTH_S: f32 = 240.0;

pub struct DrivingState {
    truck: Truck,
    camera: ChaseCamera,
    time_of_day: f32,
    distance_m: f32,
    best_m: f32,
    biome_band: u32,
    banner_timer: f32,
    paused: bool,
    fade_in: f32,
}

impl DrivingState {
    pub fn new() -> DrivingState {
        DrivingState {
            truck: Truck::new(),
            camera: ChaseCamera::new(),
            time_of_day: 0.34, // mid-morning wasteland light
            distance_m: 0.0,
            best_m: 0.0,
            biome_band: biome_at(0.0).band_index(),
            banner_timer: 3.2, // greet the player with the starting biome
            paused: false,
            fade_in: 1.0,
        }
    }

    #[inline]
    fn sun_elevation(&self) -> f32 {
        ((self.time_of_day - 0.25) * std::f32::consts::TAU).sin()
    }

    /// Global ambient multiplier for every lit surface, [0.2, 1.0].
    #[inline]
    fn ambient(&self) -> f32 {
        (0.22 + self.sun_elevation().max(0.0) * 0.82).clamp(0.2, 1.0)
    }

    fn sky_color(&self, biome: Biome) -> Color {
        let elev = self.sun_elevation();
        let day = [0.34, 0.60, 0.82];
        let dusk = [0.88, 0.47, 0.26];
        let night = [0.020, 0.025, 0.055];

        let smoothstep = |a: f32, b: f32, x: f32| -> f32 {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let base = if elev >= 0.0 {
            let k = smoothstep(0.0, 0.6, elev);
            [
                dusk[0] + (day[0] - dusk[0]) * k,
                dusk[1] + (day[1] - dusk[1]) * k,
                dusk[2] + (day[2] - dusk[2]) * k,
            ]
        } else {
            let k = smoothstep(0.0, 0.6, -elev);
            [
                dusk[0] + (night[0] - dusk[0]) * k,
                dusk[1] + (night[1] - dusk[1]) * k,
                dusk[2] + (night[2] - dusk[2]) * k,
            ]
        };

        // 12 % biome haze so swamp/city air reads different
        let haze = haze_color(biome, 1.0);
        Color::new(
            base[0] * 0.88 + haze.r * 0.12,
            base[1] * 0.88 + haze.g * 0.12,
            base[2] * 0.88 + haze.b * 0.12,
            1.0,
        )
    }

    pub fn update(&mut self, dt: f32) {
        // ---------- global keys ----------
        let mut pause_toggle = is_key_pressed(KeyCode::Escape);
        #[cfg(target_os = "android")]
        {
            if is_key_pressed(KeyCode::Back) {
                pause_toggle = true;
            }
        }
        if pause_toggle {
            self.paused = !self.paused;
        }

        if is_key_pressed(KeyCode::R) {
            self.truck.reset();
            self.camera = ChaseCamera::new();
            self.distance_m = 0.0;
            self.time_of_day = 0.34;
            self.fade_in = 1.0;
        }

        if self.paused {
            return;
        }

        self.fade_in = (self.fade_in - dt * 1.6).max(0.0);
        self.banner_timer = (self.banner_timer - dt).max(0.0);
        self.time_of_day = (self.time_of_day + dt / DAY_LENGTH_S).rem_euclid(1.0);

        // ---------- driving input ----------
        let mut throttle = 0.0f32;
        let mut steer = 0.0f32;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            throttle += 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            throttle -= 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            steer -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            steer += 1.0;
        }
        // touch: left/right screen thirds steer; AUTO-GAS on phones
        for touch in touches() {
            if matches!(touch.phase, TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary) {
                let frac = touch.position.x / screen_width();
                if frac < 0.38 {
                    steer -= 1.0;
                } else if frac > 0.62 {
                    steer += 1.0;
                }
            }
        }
        #[cfg(target_os = "android")]
        {
            if throttle == 0.0 {
                throttle = 0.72; // AUTO-GAS — thumbs stay on the steer zones
            }
        }
        let steer = steer.clamp(-1.0, 1.0);

        let offroad = self.truck.position.x.abs() > ROAD_HALF + 0.5;
        self.truck.update(dt, throttle, steer, offroad, get_time() as f32);

        self.distance_m = self.distance_m.max(self.truck.position.z.max(0.0));
        self.best_m = self.best_m.max(self.distance_m);

        // ---------- biome transition banner ----------
        let band = biome_at(self.truck.position.z).band_index();
        if band != self.biome_band {
            self.biome_band = band;
            self.banner_timer = 3.2;
        }

        // ---------- chase camera ----------
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);
        self.camera
            .update(self.truck.position, self.truck.forward(), self.truck.steer, speed_norm, dt);
    }

    pub fn draw(&mut self) {
        let z = self.truck.position.z;
        let biome = biome_at(z);
        let ambient = self.ambient();
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);

        // ---------- 3D pass ----------
        clear_background(self.sky_color(biome));
        set_camera(&self.camera.to_camera3d(speed_norm));
        draw_world(z, ambient);
        self.truck.draw(ambient);
        set_default_camera();

        // ---------- 2D pass ----------
        ui::draw_hud(&HudData {
            health: self.truck.health,
            max_health: self.truck.max_health,
            fuel: self.truck.fuel,
            max_fuel: self.truck.max_fuel,
            turret_heat: self.truck.turret_heat,
            speed_kmh: self.truck.speed_kmh(),
            distance_m: self.distance_m,
            best_m: self.best_m,
            biome: biome.name(),
            offroad: self.truck.position.x.abs() > ROAD_HALF + 0.5,
            low_fuel: self.truck.fuel / self.truck.max_fuel < 0.15,
        });
        ui::draw_letterbox_and_vignette();

        if self.banner_timer > 0.0 {
            // fade in over first 0.4 s, hold, fade out over last 0.8 s
            let t = self.banner_timer;
            let alpha = (t / 0.8).min(1.0).min((3.2 - t) / 0.4).clamp(0.0, 1.0);
            ui::draw_biome_banner(biome.name(), alpha);
        }

        if self.fade_in > 0.0 {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, self.fade_in * 0.9));
        }

        if self.paused {
            ui::draw_pause_overlay();
        }
    }
}
