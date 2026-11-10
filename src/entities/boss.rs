//! Boss #1 — JUGGERNAUT, the raffinate tanker.
//!
//! A slow, absurdly armored fuel tanker that hauls its load down the asphalt
//! with the rig. Appears from ~1300 m and then roughly every ~2600 m.
//!
//! Behaviour:
//! * **Rubber-band chase** — it holds ~24 m ahead; outrun it and it eases off,
//!   fall behind and it waits (11 m/s floor) so the duel stays on.
//! * **Weave** — slow sine through the lanes, like any tanker traffic.
//! * **Ram maneuver** — every 5–9 s it squares up and lunges at the rig's lane
//!   (telegraphed by a volatile glow under the cab). Contact: 26 dmg + shake.
//! * **Mine laying** — every ~4.2 s it drops a proximity mine off the tail;
//!   mines blink amber, then red, then kill anything inside 2.35 m.
//!
//! Armor: AP deals 80 %, incendiary 35 % (it shrugs off fire). Killing it
//! showers salvage and guarantees a medkit.
//!
//! Structure: a single `BossTruck` (not pooled — only one exists at a time) and
//! a fixed-capacity `MinePool` (24). No allocation after construction.

use macroquad::prelude::*;

use crate::systems::meshkit::{world_at, PartMesh};
use crate::systems::particles::ParticlePool;

const BOSS_WHEELS: [(f32, f32); 6] = [
    (1.12, 3.9),
    (-1.12, 3.9),
    (1.12, 0.6),
    (-1.12, 0.6),
    (1.12, -3.4),
    (-1.12, -3.4),
];

#[derive(Default, Clone, Copy)]
pub struct BossEvents {
    /// Set while the ram maneuver is telegraphed / lunging.
    pub ramming: bool,
    /// Set the frame a mine is dropped (for the soft thunk shake).
    pub mine_dropped: bool,
}

pub struct BossTruck {
    pub pos: Vec3,
    pub yaw: f32,
    pub speed: f32,
    pub hp: f32,
    pub max_hp: f32,
    wheel_spin: f32,
    mine_timer: f32,
    swerve_timer: f32,
    maneuver: f32,
    swerve_dir: f32,
    weave_phase: f32,
    flash: f32,
    body: PartMesh,
    wheel: PartMesh,
}

impl BossTruck {
    pub const NAME: &'static str = "JUGGERNAUT";
    pub const RADIUS: f32 = 2.75;
    pub const MAX_HP: f32 = 900.0;

    pub fn spawn(z: f32) -> BossTruck {
        BossTruck {
            pos: vec3(0.0, 0.0, z),
            yaw: 0.0,
            speed: 17.0,
            hp: Self::MAX_HP,
            max_hp: Self::MAX_HP,
            wheel_spin: 0.0,
            mine_timer: 2.0,
            swerve_timer: 4.0,
            maneuver: 0.0,
            swerve_dir: 1.0,
            weave_phase: 0.0,
            flash: 0.0,
            body: Self::build_body(),
            wheel: Self::build_wheel(),
        }
    }

    pub fn hp_ratio(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    /// AP 80 % / incendiary 35 % — the armor is the point of the fight.
    /// Returns true when this hit destroys it.
    pub fn apply_damage(&mut self, damage: f32, incendiary: bool) -> bool {
        let dealt = if incendiary { damage * 0.35 } else { damage * 0.8 };
        self.hp -= dealt;
        self.hp <= 0.0
    }

    pub fn update(&mut self, dt: f32, truck_pos: Vec3, truck_speed: f32, mines: &mut MinePool) -> BossEvents {
        let mut events = BossEvents::default();

        // --- rubber-band: hang ~24 m ahead, ease off if the rig falls behind ---
        let gap = self.pos.z - truck_pos.z;
        let desired_speed = (truck_speed + (gap - 24.0) * 0.22).clamp(10.5, 20.5);
        self.speed += (desired_speed - self.speed) * (1.0 - (-dt * 1.6).exp());

        // --- lane weave ---
        self.weave_phase += dt * 0.5;
        let weave = self.weave_phase.sin() * 3.1;

        // --- ram maneuver: countdown → 1.55 s lunge at the rig ---
        self.swerve_timer -= dt;
        self.maneuver = (self.maneuver - dt).max(0.0);
        self.flash = (self.flash - dt * 1.3).max(0.0);
        if self.swerve_timer <= 0.0 && self.maneuver <= 0.0 {
            self.maneuver = 1.55;
            self.swerve_timer = 5.0 + self.weave_phase.sin().abs() * 4.0;
            self.swerve_dir = if truck_pos.x >= self.pos.x { 1.0 } else { -1.0 };
            self.flash = 1.0;
        }
        events.ramming = self.maneuver > 0.0;

        let desired_yaw = if self.maneuver > 0.0 {
            let dx = (truck_pos.x - self.pos.x).clamp(-7.0, 7.0);
            (dx * 0.095).clamp(-0.46, 0.46)
        } else {
            ((weave - self.pos.x) * 0.06).clamp(-0.34, 0.34)
        };
        self.yaw += (desired_yaw - self.yaw) * (1.0 - (-dt * 2.3).exp());
        self.pos += vec3(self.yaw.sin(), 0.0, self.yaw.cos()) * self.speed * dt;
        self.wheel_spin += self.speed * dt / 0.62;

        // --- mine laying, tail of the trailer ---
        self.mine_timer -= dt;
        if self.mine_timer <= 0.0 && gap.abs() < 70.0 {
            self.mine_timer = 4.2;
            let (sy, cy) = (self.yaw.sin(), self.yaw.cos());
            // local offset (0, 0, -5.3) rotated into world
            let rear = vec3(self.pos.x - sy * 5.3, 0.0, self.pos.z - cy * 5.3);
            if mines.drop(rear) {
                events.mine_dropped = true;
            }
        }

        events
    }

    /// Horizontal range to a point (body length ignored; the hit sphere covers it).
    pub fn horizontal_distance_to(&self, p: Vec3) -> f32 {
        let dx = self.pos.x - p.x;
        let dz = self.pos.z - p.z;
        (dx * dx + dz * dz).sqrt()
    }

    pub fn draw(&mut self, ambient: f32, time: f32) {
        let model = Mat4::from_translation(self.pos) * Mat4::from_rotation_y(self.yaw);

        self.body.write_transformed(&model, ambient);
        self.body.draw();

        for &(x, z) in BOSS_WHEELS.iter() {
            let w = world_at(&model, vec3(x, 0.62, z))
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_y(self.wheel_spin);
            self.wheel.write_transformed(&w, ambient);
            self.wheel.draw();
        }

        // ram-maneuver telegraph: volatile orange pulse slung under the cab
        if self.flash > 0.02 {
            let pulse = 0.6 + 0.4 * (time * 14.0).sin();
            let s = 1.0 + 0.6 * self.flash * pulse;
            let (sy, cy) = (self.yaw.sin(), self.yaw.cos());
            let gp = self.pos + vec3(0.0 * cy + 3.2 * sy, 1.05, -0.0 * sy + 3.2 * cy);
            draw_cube(gp, vec3(s * 1.5, s * 0.62, s * 0.9), None, Color::new(1.0, 0.22 + 0.2 * pulse, 0.05, 1.0));
        }
    }

    // ------------------------------------------------------------------
    // models
    // ------------------------------------------------------------------
    fn build_wheel() -> PartMesh {
        let tire = Color::new(0.075, 0.075, 0.08, 1.0);
        let hub = Color::new(0.52, 0.5, 0.46, 1.0);
        let rust = Color::new(0.46, 0.22, 0.10, 1.0);
        let mut wheel = PartMesh::cylinder_at(0.62, 0.62, 0.48, 14, tire, vec3(0.0, -0.24, 0.0));
        wheel.merge(&PartMesh::cylinder_at(0.32, 0.32, 0.52, 10, hub, vec3(0.0, -0.26, 0.0)));
        wheel.merge(&PartMesh::cylinder_at(0.13, 0.13, 0.55, 8, rust, vec3(0.0, -0.28, 0.0)));
        wheel
    }

    fn build_body() -> PartMesh {
        let olive = Color::new(0.26, 0.28, 0.17, 1.0);
        let dark = Color::new(0.12, 0.12, 0.12, 1.0);
        let tank = Color::new(0.34, 0.2, 0.09, 1.0);
        let metal = Color::new(0.37, 0.36, 0.33, 1.0);
        let glass = Color::new(0.06, 0.1, 0.13, 1.0);
        let rust = Color::new(0.44, 0.22, 0.1, 1.0);
        let hazard = Color::new(0.95, 0.6, 0.12, 1.0);

        // --- frame ---
        let mut b = PartMesh::cuboid_at(vec3(2.3, 0.5, 9.8), dark, vec3(0.0, 1.15, 0.0));

        // --- tractor: hood, cab, grille, bullbar ---
        b.merge(&PartMesh::cuboid_at(vec3(2.3, 1.0, 1.5), olive, vec3(0.0, 1.75, 4.15))); // hood
        b.merge(&PartMesh::cuboid_at(vec3(2.45, 0.14, 1.6), dark, vec3(0.0, 2.32, 4.15))); // hood plate
        b.merge(&PartMesh::cuboid_at(vec3(2.5, 1.7, 2.1), olive, vec3(0.0, 2.35, 2.15))); // cab
        b.merge(&PartMesh::cuboid_at(vec3(2.6, 0.12, 2.2), rust, vec3(0.0, 3.26, 2.15))); // roof rim
        b.merge(&PartMesh::cuboid_at(vec3(2.15, 0.62, 0.08), glass, vec3(0.0, 2.5, 3.22))); // windshield
        b.merge(&PartMesh::cuboid_at(vec3(0.07, 0.42, 1.1), glass, vec3(-1.27, 2.5, 2.1))); // side glass
        b.merge(&PartMesh::cuboid_at(vec3(0.07, 0.42, 1.1), glass, vec3(1.27, 2.5, 2.1)));
        b.merge(&PartMesh::cuboid_at(vec3(2.1, 0.75, 0.14), dark, vec3(0.0, 1.6, 4.93))); // grille
        b.merge(&PartMesh::cuboid_at(vec3(2.2, 0.18, 0.18), metal, vec3(0.0, 1.13, 5.02))); // bullbar low
        b.merge(&PartMesh::cuboid_at(vec3(0.18, 0.75, 0.16), metal, vec3(-0.8, 1.42, 5.0))); // bullbar posts
        b.merge(&PartMesh::cuboid_at(vec3(0.18, 0.75, 0.16), metal, vec3(0.8, 1.42, 5.0)));
        b.merge(&PartMesh::cuboid_at(vec3(2.2, 0.34, 0.1), dark, vec3(0.0, 1.05, 5.08))); // bumper
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.3, 0.2, 0.08), Color::new(1.0, 0.96, 0.78, 1.0), vec3(-0.75, 1.85, 4.94))); // headlights
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.3, 0.2, 0.08), Color::new(1.0, 0.96, 0.78, 1.0), vec3(0.75, 1.85, 4.94)));
        // exhaust stacks
        b.merge(&PartMesh::cylinder_at(0.11, 0.11, 1.7, 8, metal, vec3(-1.08, 3.1, 1.35)));
        b.merge(&PartMesh::cylinder_at(0.11, 0.11, 1.7, 8, metal, vec3(1.08, 3.1, 1.35)));
        // roof beacon (amber, always on)
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.5, 0.15, 0.34), hazard, vec3(0.0, 3.4, 2.15)));

        // --- saddle & tank trailer ---
        b.merge(&PartMesh::cuboid_at(vec3(1.6, 0.5, 1.2), dark, vec3(0.0, 1.95, 0.55))); // saddle
        b.merge(&PartMesh::cylinder_along_z_at(1.45, 1.45, 5.9, 16, tank, vec3(0.0, 2.35, -1.95))); // THE TANK
        b.merge(&PartMesh::cuboid_at(vec3(0.24, 0.3, 5.6), rust, vec3(-1.42, 2.35, -1.95))); // side walkway L
        b.merge(&PartMesh::cuboid_at(vec3(0.24, 0.3, 5.6), rust, vec3(1.42, 2.35, -1.95))); // side walkway R
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.1, 0.55, 1.1), hazard, vec3(-1.5, 2.2, -1.3))); // hazard placard L
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.1, 0.55, 1.1), hazard, vec3(1.5, 2.2, -1.3))); // hazard placard R
        b.merge(&PartMesh::cuboid_at(vec3(1.1, 0.42, 0.9), dark, vec3(0.0, 1.62, -5.15))); // rear bumper box
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.32, 0.16, 0.08), Color::new(0.95, 0.12, 0.08, 1.0), vec3(-0.72, 1.62, -5.62))); // taillights
        b.merge(&PartMesh::cuboid_glow_at(vec3(0.32, 0.16, 0.08), Color::new(0.95, 0.12, 0.08, 1.0), vec3(0.72, 1.62, -5.62)));
        // top hatch + whip antenna
        b.merge(&PartMesh::cylinder_at(0.34, 0.4, 0.22, 10, metal, vec3(0.0, 3.82, -2.4)));
        b.merge(&PartMesh::cylinder_at(0.02, 0.02, 1.1, 5, dark, vec3(0.8, 4.35, -4.6)));
        b
    }
}

// ---------------------------------------------------------------------------
// Proximity mines (pooled)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Mine {
    pos: Vec3,
    arm: f32,
    life: f32,
    blink: f32,
    active: bool,
}

impl Mine {
    const INACTIVE: Mine = Mine {
        pos: Vec3::ZERO,
        arm: 0.0,
        life: 0.0,
        blink: 0.0,
        active: false,
    };
}

const MINE_CAPACITY: usize = 24;

pub struct MinePool {
    items: Vec<Mine>,
}

impl MinePool {
    pub fn new() -> MinePool {
        MinePool {
            items: vec![Mine::INACTIVE; MINE_CAPACITY],
        }
    }

    /// Lay a mine (0.8 s arming, 45 s fuse). Returns false when the pool is full.
    pub fn drop(&mut self, pos: Vec3) -> bool {
        for m in self.items.iter_mut() {
            if !m.active {
                *m = Mine {
                    pos: vec3(pos.x, 0.0, pos.z),
                    arm: 0.8,
                    life: 45.0,
                    blink: pos.x * 0.37 + pos.z * 0.11, // stable per-mine phase
                    active: true,
                };
                return true;
            }
        }
        false
    }

    /// Advance mines; returns total damage inflicted on the rig this frame.
    pub fn update(&mut self, dt: f32, truck_pos: Vec3, particles: &mut ParticlePool) -> f32 {
        let mut damage = 0.0f32;
        for m in self.items.iter_mut() {
            if !m.active {
                continue;
            }
            m.arm -= dt;
            m.life -= dt;
            if m.life <= 0.0 {
                m.active = false;
                continue;
            }
            if m.arm > 0.0 {
                continue;
            }
            let dx = truck_pos.x - m.pos.x;
            let dz = truck_pos.z - m.pos.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d < 2.35 {
                m.active = false;
                let splash = (1.0 - (d / 11.0).clamp(0.0, 1.0)) * 24.0;
                damage += 8.0 + splash;
                particles.explosion(m.pos + vec3(0.0, 0.45, 0.0), 1.25);
            }
        }
        // cull far behind
        for m in self.items.iter_mut() {
            if m.active && m.pos.z - truck_pos.z < -80.0 {
                m.active = false;
            }
        }
        damage
    }

    pub fn draw(&self, time: f32) {
        for m in self.items.iter() {
            if !m.active {
                continue;
            }
            draw_cube(m.pos + vec3(0.0, 0.09, 0.0), vec3(0.56, 0.18, 0.56), None, Color::new(0.11, 0.11, 0.11, 1.0));
            let armed = m.arm <= 0.0;
            let on = (time * if armed { 3.4 } else { 1.6 } + m.blink).sin() > 0.15;
            if on {
                let c = if armed {
                    Color::new(1.0, 0.14, 0.09, 1.0)
                } else {
                    Color::new(0.95, 0.62, 0.14, 1.0)
                };
                draw_cube(m.pos + vec3(0.0, 0.21, 0.0), vec3(0.2, 0.07, 0.2), None, c);
            }
        }
    }
}
