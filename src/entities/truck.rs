//! The war rig — player truck.
//!
//! Construction mirrors the real vehicle: chassis → armor → cab → cargo bed →
//! bullbar → exhaust stacks → turret pedestal (body, one merged mesh) and a
//! separately articulated turret + six spinning wheels. Everything is rendered
//! through [`crate::systems::meshkit::PartMesh`] so the whole truck costs
//! ~3 draw calls and zero per-frame allocations.
//!
//! World convention: Y-up, truck forward is local +Z, `yaw` rotates around Y.

use macroquad::prelude::*;

use crate::systems::meshkit::{world_at, PartMesh};

/// Wheel radius in meters (also the axle height).
pub const WHEEL_RADIUS: f32 = 0.52;
/// Local origin of the turret pivot (top of the bed pedestal).
const TURRET_PIVOT: Vec3 = Vec3::new(0.0, 2.64, -1.075);
/// Wheel x/z offsets (front axle + dual rear axles).
const WHEEL_OFFSETS: [(f32, f32); 6] = [
    (1.06, 2.35),
    (-1.06, 2.35),
    (1.06, -1.55),
    (-1.06, -1.55),
    (1.06, -2.45),
    (-1.06, -2.45),
];

// ---------------------------------------------------------------------------
// Mad-Max / Rage palette
// ---------------------------------------------------------------------------
const OLIVE: Color = Color::new(0.30, 0.33, 0.19, 1.0);
const RUST: Color = Color::new(0.46, 0.22, 0.10, 1.0);
const DARK: Color = Color::new(0.13, 0.13, 0.14, 1.0);
const METAL: Color = Color::new(0.38, 0.37, 0.34, 1.0);
const HUB: Color = Color::new(0.58, 0.58, 0.55, 1.0);
const GLASS: Color = Color::new(0.06, 0.10, 0.13, 1.0);
const TIRE: Color = Color::new(0.075, 0.075, 0.08, 1.0);
const HEADLIGHT: Color = Color::new(1.0, 0.96, 0.78, 1.0);
const TAILLIGHT: Color = Color::new(0.95, 0.12, 0.08, 1.0);
const ARMOR: Color = Color::new(0.24, 0.27, 0.16, 1.0);

const MAX_SPEED: f32 = 33.0; // ≈ 119 km/h
const MAX_REVERSE: f32 = 7.0;
const ENGINE_POWER: f32 = 14.0;
const WHEELBASE_YAW: f32 = 0.115;

pub struct Truck {
    pub position: Vec3,
    pub yaw: f32,
    pub speed: f32,
    /// Smoothed steering, -1 (left) .. 1 (right).
    pub steer: f32,
    pub health: f32,
    pub max_health: f32,
    pub fuel: f32,
    pub max_fuel: f32,
    /// Flat damage resist from armor ranks, 0.0..0.6 (set by the driving state).
    pub damage_reduction: f32,
    /// Top-speed multiplier from engine ranks (1.0 + rank * 0.10).
    pub top_speed_mult: f32,
    /// Acceleration multiplier from engine ranks (1.0 + rank * 0.12).
    pub acceleration_mult: f32,
    wheel_spin: f32,
    turret_yaw: f32,
    hull_bonus: f32,

    body: PartMesh,
    turret: PartMesh,
    wheel: PartMesh,
}

impl Truck {
    pub fn new() -> Truck {
        let body = Self::build_body();
        let turret = Self::build_turret();
        let wheel = Self::build_wheel();
        Truck {
            position: Vec3::ZERO,
            yaw: 0.0,
            speed: 0.0,
            steer: 0.0,
            health: 100.0,
            max_health: 100.0,
            fuel: 100.0,
            max_fuel: 100.0,
            damage_reduction: 0.0,
            top_speed_mult: 1.0,
            acceleration_mult: 1.0,
            wheel_spin: 0.0,
            turret_yaw: 0.0,
            hull_bonus: 0.0,
            body,
            turret,
            wheel,
        }
    }

    /// Forward unit vector in world space.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    /// Speed in km/h for the HUD.
    pub fn speed_kmh(&self) -> f32 {
        self.speed.abs() * 3.6
    }

    /// Set the turret's relative yaw (driven by the turret controller).
    pub fn set_turret_yaw(&mut self, yaw: f32) {
        self.turret_yaw = yaw;
    }

    /// Absolute world yaw of the turret (truck yaw + relative turret yaw).
    pub fn turret_world_yaw(&self) -> f32 {
        self.yaw + self.turret_yaw
    }

    /// World-space muzzle position and flat firing direction.
    pub fn turret_muzzle(&self) -> (Vec3, Vec3) {
        let world_yaw = self.turret_world_yaw();
        let dir = vec3(world_yaw.sin(), 0.0, world_yaw.cos());
        // rotate the pivot offset from local to world (Y-up, around the truck yaw)
        let (sy, cy) = (self.yaw.sin(), self.yaw.cos());
        let pivot_world = vec3(
            self.position.x + TURRET_PIVOT.x * cy + TURRET_PIVOT.z * sy,
            self.position.y + TURRET_PIVOT.y,
            self.position.z - TURRET_PIVOT.x * sy + TURRET_PIVOT.z * cy,
        );
        let muzzle = pivot_world + vec3(dir.x * 2.05, 0.21, dir.z * 2.05);
        (muzzle, dir)
    }

    /// Re-derive max hull from the armor track. Buying ranks mid-run
    /// patches the rig by the delta (a fresh plate on the bed, fresh blood in it).
    pub fn set_hull_bonus(&mut self, bonus: f32) {
        let delta = bonus - self.hull_bonus;
        self.hull_bonus = bonus;
        self.max_health = 100.0 + bonus;
        if delta != 0.0 {
            self.health = (self.health + delta).clamp(0.0, self.max_health);
        }
    }

    /// Apply damage to the hull (fires the health bar; 0 = wrecked).
    pub fn apply_damage(&mut self, damage: f32) {
        let resisted = damage * (1.0 - self.damage_reduction.clamp(0.0, 0.6));
        self.health = (self.health - resisted).max(0.0);
    }

    /// Truck velocity vector (used for rocket lead prediction).
    pub fn velocity(&self) -> Vec3 {
        self.forward() * self.speed
    }

    /// Sandstorm gust: nudges the nose sideways. Drift grows with speed.
    pub fn apply_wind_gust(&mut self, gust: f32, dt: f32) {
        self.yaw += gust * 0.055 * dt * (0.4 + self.speed.abs() * 0.02);
    }

    fn build_body() -> PartMesh {
        // -- chassis & underbody --
        let mut body = PartMesh::cuboid_at(vec3(2.0, 0.35, 6.3), DARK, vec3(0.0, 1.02, 0.0));
        body.merge(&PartMesh::cuboid_at(
            vec3(1.7, 0.18, 3.2),
            DARK,
            vec3(0.0, 0.80, -0.2),
        ));
        // -- cargo bed --
        body.merge(&PartMesh::cuboid_at(
            vec3(2.3, 0.15, 3.85),
            DARK,
            vec3(0.0, 1.26, -1.075),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.14, 0.8, 3.85),
            OLIVE,
            vec3(-1.09, 1.74, -1.075),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.14, 0.8, 3.85),
            OLIVE,
            vec3(1.09, 1.74, -1.075),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(2.3, 0.8, 0.14),
            OLIVE,
            vec3(0.0, 1.74, 0.85),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(2.3, 0.8, 0.14),
            RUST,
            vec3(0.0, 1.74, -3.0),
        ));
        // bolt-on side armor plates (Module 3 lets the player upgrade these)
        body.merge(&PartMesh::cuboid_at(
            vec3(0.1, 0.45, 2.6),
            ARMOR,
            vec3(-1.20, 1.55, -1.1),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.1, 0.45, 2.6),
            ARMOR,
            vec3(1.20, 1.55, -1.1),
        ));
        // -- cab --
        body.merge(&PartMesh::cuboid_at(
            vec3(2.15, 0.95, 1.45),
            OLIVE,
            vec3(0.0, 1.85, 1.65),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(2.25, 0.10, 1.55),
            DARK,
            vec3(0.0, 2.38, 1.65),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.8, 0.08, 0.7),
            RUST,
            vec3(0.0, 2.46, 1.65),
        ));
        // windshield & side glass (unrotated low-poly slabs)
        body.merge(&PartMesh::cuboid_at(
            vec3(1.9, 0.5, 0.05),
            GLASS,
            vec3(0.0, 2.02, 2.385),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.05, 0.38, 0.8),
            GLASS,
            vec3(-1.08, 2.02, 1.65),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.05, 0.38, 0.8),
            GLASS,
            vec3(1.08, 2.02, 1.65),
        ));
        // -- hood & nose --
        body.merge(&PartMesh::cuboid_at(
            vec3(1.9, 0.5, 0.85),
            OLIVE,
            vec3(0.0, 1.55, 2.75),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.7, 0.16, 0.5),
            DARK,
            vec3(0.0, 1.86, 2.7),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(1.6, 0.4, 0.1),
            DARK,
            vec3(0.0, 1.35, 3.19),
        ));
        // -- lights (emissive: punch through night ambient) --
        body.merge(&PartMesh::cuboid_glow_at(
            vec3(0.28, 0.18, 0.06),
            HEADLIGHT,
            vec3(-0.68, 1.52, 3.20),
        ));
        body.merge(&PartMesh::cuboid_glow_at(
            vec3(0.28, 0.18, 0.06),
            HEADLIGHT,
            vec3(0.68, 1.52, 3.20),
        ));
        body.merge(&PartMesh::cuboid_glow_at(
            vec3(0.22, 0.14, 0.06),
            TAILLIGHT,
            vec3(-0.75, 1.32, -3.13),
        ));
        body.merge(&PartMesh::cuboid_glow_at(
            vec3(0.22, 0.14, 0.06),
            TAILLIGHT,
            vec3(0.75, 1.32, -3.13),
        ));
        // -- bullbar --
        body.merge(&PartMesh::cuboid_at(
            vec3(2.05, 0.16, 0.16),
            METAL,
            vec3(0.0, 1.28, 3.38),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.16, 0.55, 0.14),
            METAL,
            vec3(-0.72, 1.32, 3.36),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.16, 0.55, 0.14),
            METAL,
            vec3(0.72, 1.32, 3.36),
        ));
        // -- dual vertical exhaust stacks behind the cab --
        body.merge(&PartMesh::cylinder_at(
            0.09,
            0.09,
            1.15,
            8,
            METAL,
            vec3(-0.92, 2.32, 0.78),
        ));
        body.merge(&PartMesh::cylinder_at(
            0.09,
            0.09,
            1.15,
            8,
            METAL,
            vec3(0.92, 2.32, 0.78),
        ));
        // -- antenna --
        body.merge(&PartMesh::cylinder_at(
            0.022,
            0.022,
            1.35,
            5,
            DARK,
            vec3(0.85, 2.35, -2.85),
        ));
        // -- mirrors --
        body.merge(&PartMesh::cuboid_at(
            vec3(0.08, 0.16, 0.22),
            DARK,
            vec3(-1.22, 2.1, 2.3),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.08, 0.16, 0.22),
            DARK,
            vec3(1.22, 2.1, 2.3),
        ));
        // -- rear mud flaps --
        body.merge(&PartMesh::cuboid_at(
            vec3(0.5, 0.3, 0.05),
            DARK,
            vec3(-1.0, 0.72, -3.16),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.5, 0.3, 0.05),
            DARK,
            vec3(1.0, 0.72, -3.16),
        ));
        // -- side fuel tanks --
        body.merge(&PartMesh::cuboid_at(
            vec3(0.32, 0.32, 1.1),
            RUST,
            vec3(-1.17, 0.95, 0.35),
        ));
        body.merge(&PartMesh::cuboid_at(
            vec3(0.32, 0.32, 1.1),
            RUST,
            vec3(1.17, 0.95, 0.35),
        ));
        // -- turret pedestal (fixed to bed) --
        body.merge(&PartMesh::cylinder_at(
            0.30,
            0.37,
            0.5,
            10,
            DARK,
            vec3(TURRET_PIVOT.x, 2.14, TURRET_PIVOT.z),
        ));
        body
    }

    fn build_turret() -> PartMesh {
        // origin = turret pivot (top of pedestal)
        let mut turret = PartMesh::cuboid_at(vec3(0.95, 0.38, 1.15), OLIVE, vec3(0.0, 0.19, 0.10));
        turret.merge(&PartMesh::cuboid_at(
            vec3(1.02, 0.09, 1.22),
            RUST,
            vec3(0.0, 0.42, 0.10),
        ));
        turret.merge(&PartMesh::cuboid_at(
            vec3(0.52, 0.28, 0.22),
            DARK,
            vec3(0.0, 0.17, 0.66),
        ));
        turret.merge(&PartMesh::cuboid_at(
            vec3(0.13, 0.13, 1.45),
            DARK,
            vec3(0.0, 0.21, 1.32),
        ));
        turret.merge(&PartMesh::cuboid_at(
            vec3(0.20, 0.20, 0.28),
            RUST,
            vec3(0.0, 0.21, 1.95),
        ));
        turret
    }

    fn build_wheel() -> PartMesh {
        // axis = local Y; rotated onto X at draw time via Rz(90°)
        let mut wheel =
            PartMesh::cylinder_at(WHEEL_RADIUS, WHEEL_RADIUS, 0.40, 14, TIRE, vec3(0.0, -0.20, 0.0));
        wheel.merge(&PartMesh::cylinder_at(
            0.27,
            0.27,
            0.44,
            10,
            HUB,
            vec3(0.0, -0.22, 0.0),
        ));
        wheel.merge(&PartMesh::cylinder_at(
            0.10,
            0.10,
            0.47,
            8,
            RUST,
            vec3(0.0, -0.235, 0.0),
        ));
        wheel
    }

    /// Advance the simulation. `throttle` ∈ [-1, 1] (negative = brake/reverse),
    /// `steer_input` ∈ [-1, 1] (negative = left).
    pub fn update(&mut self, dt: f32, throttle: f32, steer_input: f32, offroad: bool) {
        // fuel burn: gentle idle draw + throttle-proportional load
        let burn = 0.010 + throttle.abs() * 0.055;
        self.fuel = (self.fuel - burn * dt).max(0.0);

        let engine = if self.fuel > 0.0 {
            throttle * ENGINE_POWER * self.acceleration_mult
        } else {
            0.0
        };
        // quadratic-ish drag keeps top speed stable without a lookup table
        let drag = self.speed * (0.35 + self.speed.abs() * 0.006);
        self.speed += (engine - drag) * dt;
        // soft-sand shoulder punishes leaving the asphalt
        if offroad {
            self.speed -= self.speed * 1.35 * dt;
        }
        self.speed = self.speed.clamp(-MAX_REVERSE, MAX_SPEED * self.top_speed_mult);
        if self.speed.abs() < 0.02 && throttle == 0.0 {
            self.speed = 0.0;
        }

        // steering loses authority at speed (stability for the Infinix's 60 fps)
        let steer_max = 0.55 / (1.0 + self.speed.abs() * 0.055);
        let target = steer_input.clamp(-1.0, 1.0) * steer_max;
        let blend = 1.0 - (-dt * 6.0).exp();
        self.steer += (target - self.steer) * blend;
        self.yaw += self.steer * self.speed * WHEELBASE_YAW * dt;

        let forward = self.forward();
        self.position += forward * self.speed * dt;

        self.wheel_spin += self.speed * dt / WHEEL_RADIUS;
        // turret yaw is driven externally by the turret controller (set_turret_yaw)
    }

    /// Render. `ambient` ∈ [0, 1.05] from the day/night cycle.
    pub fn draw(&mut self, ambient: f32) {
        let model = Mat4::from_translation(self.position) * Mat4::from_rotation_y(self.yaw);

        self.body.write_transformed(&model, ambient);
        self.body.draw();

        for &(x, z) in WHEEL_OFFSETS.iter() {
            let wheel_world = world_at(&model, vec3(x, WHEEL_RADIUS, z))
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_y(self.wheel_spin);
            self.wheel.write_transformed(&wheel_world, ambient);
            self.wheel.draw();
        }

        let turret_world = world_at(&model, TURRET_PIVOT) * Mat4::from_rotation_y(self.turret_yaw);
        self.turret.write_transformed(&turret_world, ambient);
        self.turret.draw();
    }
}
