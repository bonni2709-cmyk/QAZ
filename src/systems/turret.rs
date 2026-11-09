//! Turret controller: 360° yaw authority, heat model, ammo selection.
//!
//! The player owns the turret. Desktop: mouse X axis rotates it (cursor grabbed
//! while driving), LMB / Space fires, 1/2/Q pick ammo. Android: auto-aim tracks
//! the nearest hostile with partial target lead; FIRE is a big on-screen button.
//!
//! Heat model: sustained fire walks the barrel into an overheat lockout —
//! you must feather the trigger. Incendiary runs cooler than AP.

use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmmoKind {
    /// Tungsten AP — flat 34 damage, ignores armor, ricochets off armored cars.
    Ap,
    /// Incendiary — 16 damage +8 vs unarmored, sets the target burning (3 s).
    Incendiary,
}

impl AmmoKind {
    pub fn label(self) -> &'static str {
        match self {
            AmmoKind::Ap => "AP  ·  TUNGSTEN",
            AmmoKind::Incendiary => "INC · INCENDIARY",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            AmmoKind::Ap => "AP",
            AmmoKind::Incendiary => "INC",
        }
    }

    /// Damage against `armored` targets (only the armored car for now).
    pub fn damage(self, armored: bool) -> f32 {
        match self {
            AmmoKind::Ap => 34.0,
            AmmoKind::Incendiary => {
                if armored {
                    14.0
                } else {
                    24.0
                }
            }
        }
    }

    fn heat_per_shot(self) -> f32 {
        match self {
            AmmoKind::Ap => 0.078,
            AmmoKind::Incendiary => 0.066,
        }
    }

    fn bullet_speed(self) -> f32 {
        match self {
            AmmoKind::Ap => 155.0,
            AmmoKind::Incendiary => 132.0,
        }
    }

    pub fn life(self) -> f32 {
        match self {
            AmmoKind::Ap => 1.55,
            AmmoKind::Incendiary => 1.45,
        }
    }

    pub fn velocity(self, dir: Vec3) -> Vec3 {
        dir * self.bullet_speed()
    }
}

const COOL_PER_SEC: f32 = 0.30;
const OVERHEAT_UNLOCK: f32 = 0.35;
const SHOT_INTERVAL: f32 = 0.115;

pub struct TurretState {
    /// Turret yaw relative to the truck hull, radians (unbounded; full 360°).
    pub yaw: f32,
    pub heat: f32,
    pub overheated: bool,
    pub ammo: AmmoKind,
    cooldown: f32,
}

impl TurretState {
    pub fn new() -> TurretState {
        TurretState {
            yaw: 0.0,
            heat: 0.0,
            overheated: false,
            ammo: AmmoKind::Ap,
            cooldown: 0.0,
        }
    }

    /// Mouse-look delta applied to the turret (desktop).
    pub fn add_aim(&mut self, dx_pixels: f32) {
        self.yaw += dx_pixels * 0.0031;
    }

    /// Rotate the turret directly toward an absolute world yaw (Android auto-aim).
    /// Turns at `rate` rad/s. `world_yaw` = truck yaw + desired relative.
    pub fn track(&mut self, target_world_yaw: f32, truck_yaw: f32, dt: f32) {
        const RATE: f32 = 3.4;
        let desired = target_world_yaw - truck_yaw;
        let mut delta = desired - self.yaw;
        const TAU: f32 = std::f32::consts::TAU;
        while delta > std::f32::consts::PI {
            delta -= TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += TAU;
        }
        let step = delta.clamp(-RATE * dt, RATE * dt);
        self.yaw += step;
    }

    pub fn cycle_ammo(&mut self) {
        self.ammo = match self.ammo {
            AmmoKind::Ap => AmmoKind::Incendiary,
            AmmoKind::Incendiary => AmmoKind::Ap,
        };
    }

    pub fn set_ammo(&mut self, ammo: AmmoKind) {
        self.ammo = ammo;
    }

    pub fn update(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        // heat always bleeds; the lockout needs a real cooldown to clear
        let cool_rate = if self.overheated { COOL_PER_SEC * 1.25 } else { COOL_PER_SEC };
        self.heat = (self.heat - cool_rate * dt).max(0.0);
        if self.overheated && self.heat <= OVERHEAT_UNLOCK {
            self.overheated = false;
        }
    }

    /// Try to fire one round. Returns false if the trigger is held but the
    /// barrel is cycling or overheated.
    pub fn try_fire(&mut self) -> bool {
        if self.overheated || self.cooldown > 0.0 {
            return false;
        }
        self.heat = (self.heat + self.ammo.heat_per_shot()).min(1.05);
        self.cooldown = SHOT_INTERVAL;
        if self.heat >= 1.0 {
            self.overheated = true;
        }
        true
    }
}
