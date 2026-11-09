//! Salvage pickups: scrap crates, repair kits and fuel cans.
//!
//! Fixed-capacity pool. Enemy wrecks pop crates with a toss; the level design
//! (wave director) seeds crate clusters on the asphalt. The salvage magnet
//! beam reels anything inside its radius straight into the rig — radius
//! grows with the workbench MAGNET track.

use macroquad::prelude::*;

use crate::systems::meshkit::PartMesh;
use crate::systems::particles::ParticlePool;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LootKind {
    /// Scrap chunks — workbench currency. `scrap` field carries the amount.
    Scrap,
    /// Medkit — patches 18 hull on pickup.
    Repair,
    /// Fuel can — tops up 22 fuel on pickup.
    Fuel,
}

#[derive(Clone, Copy)]
struct Loot {
    kind: LootKind,
    pos: Vec3,
    vel: Vec3,
    scrap: u32,
    phase: f32,
    life: f32,
    attracted: bool,
    active: bool,
}

impl Loot {
    const INACTIVE: Loot = Loot {
        kind: LootKind::Scrap,
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        scrap: 0,
        phase: 0.0,
        life: 0.0,
        attracted: false,
        active: false,
    };
}

const POOL_CAPACITY: usize = 128;
const CAPTURE_DIST: f32 = 1.7;
const MAGNET_PULL: f32 = 30.0;
const MAX_PULL_SPEED: f32 = 24.0;
const LIFETIME_S: f32 = 75.0;

/// What the rig swallowed this frame. Counts, not events — zero allocation.
#[derive(Default, Clone, Copy)]
pub struct LootTally {
    pub scrap: u32,
    pub repair: u32,
    pub fuel: u32,
}

impl LootTally {
    pub fn any(&self) -> bool {
        self.scrap > 0 || self.repair > 0 || self.fuel > 0
    }
}

pub struct LootPool {
    items: Vec<Loot>,
    scrap_mesh: PartMesh,
    repair_mesh: PartMesh,
    fuel_mesh: PartMesh,
    rng: u64,
}

impl LootPool {
    pub fn new() -> LootPool {
        LootPool {
            items: vec![Loot::INACTIVE; POOL_CAPACITY],
            scrap_mesh: Self::build_scrap(),
            repair_mesh: Self::build_repair(),
            fuel_mesh: Self::build_fuel(),
            rng: 0xD1B5_4A32_D192_ED03,
        }
    }

    fn rand01(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f32 / (1u64 << 53) as f32
    }

    fn rand_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.rand01() * (hi - lo)
    }

    /// Seed one pickup (gentle pop). Returns false when the pool is full.
    pub fn spawn(&mut self, kind: LootKind, pos: Vec3, scrap: u32) -> bool {
        self.spawn_vel(kind, pos, vec3(0.0, 2.1, 0.0), scrap)
    }

    /// Seed one pickup with a random toss (wreck loot).
    pub fn spawn_scatter(&mut self, kind: LootKind, pos: Vec3, scrap: u32) -> bool {
        let angle = self.rand_range(0.0, std::f32::consts::TAU);
        let speed = self.rand_range(1.6, 4.2);
        let vy = self.rand_range(2.4, 4.6);
        let vel = vec3(angle.sin() * speed, vy, angle.cos() * speed);
        self.spawn_vel(kind, pos, vel, scrap)
    }

    fn spawn_vel(&mut self, kind: LootKind, pos: Vec3, vel: Vec3, scrap: u32) -> bool {
        let phase = self.rand_range(0.0, std::f32::consts::TAU);
        for l in self.items.iter_mut() {
            if !l.active {
                *l = Loot {
                    kind,
                    pos,
                    vel,
                    scrap,
                    phase,
                    life: LIFETIME_S,
                    attracted: false,
                    active: true,
                };
                return true;
            }
        }
        false
    }

    /// Magnet update. Anything inside `magnet_radius` of the rig is reeled in;
    /// the returned tally tells the driving state what to apply.
    pub fn update(
        &mut self,
        dt: f32,
        truck_pos: Vec3,
        magnet_radius: f32,
        particles: &mut ParticlePool,
    ) -> LootTally {
        let mut tally = LootTally::default();
        let capture_at = truck_pos + vec3(0.0, 1.2, 0.0);

        for l in self.items.iter_mut() {
            if !l.active {
                continue;
            }
            l.life -= dt;
            if l.life <= 0.0 {
                l.active = false;
                continue;
            }

            let to = capture_at - l.pos;
            let dist = to.length();
            if dist < magnet_radius && dist > 0.001 {
                let dir = to / dist;
                l.vel += dir * MAGNET_PULL * dt;
                // reel slightly upwards so crates fly over the hood
                l.vel.y += MAGNET_PULL * 0.35 * dt;
                let speed = l.vel.length();
                if speed > MAX_PULL_SPEED {
                    l.vel *= MAX_PULL_SPEED / speed;
                }
                l.attracted = true;
            } else {
                l.attracted = false;
                l.vel.y -= 14.0 * dt;
                if l.pos.y <= 0.36 {
                    l.vel.x *= 0.86;
                    l.vel.z *= 0.86;
                    if l.vel.y < 0.0 {
                        l.vel.y = -l.vel.y * 0.38;
                    }
                }
            }
            l.pos += l.vel * dt;
            if l.pos.y < 0.32 {
                l.pos.y = 0.32;
            }

            if dist < CAPTURE_DIST {
                match l.kind {
                    LootKind::Scrap => tally.scrap += l.scrap.max(1),
                    LootKind::Repair => tally.repair += 1,
                    LootKind::Fuel => tally.fuel += 1,
                }
                particles.pickup_spark(l.pos);
                l.active = false;
            }
        }

        // cull hopelessly behind / too far ahead
        for l in self.items.iter_mut() {
            if !l.active {
                continue;
            }
            let dz = l.pos.z - truck_pos.z;
            if dz < -70.0 || dz > 300.0 {
                l.active = false;
            }
        }

        tally
    }

    /// Beam-render hook: invokes `f` for every currently magnetised pickup.
    pub fn for_each_attracted(&self, mut f: impl FnMut(Vec3)) {
        for l in self.items.iter() {
            if l.active && l.attracted {
                f(l.pos);
            }
        }
    }

    pub fn draw(&mut self, ambient: f32, time: f32) {
        for i in 0..self.items.len() {
            if !self.items[i].active {
                continue;
            }
            let bob = (time * 2.1 + self.items[i].phase).sin() * 0.16;
            let spin = time * 1.6 + self.items[i].phase;
            let model = Mat4::from_translation(self.items[i].pos + vec3(0.0, bob, 0.0))
                * Mat4::from_rotation_y(spin);
            let mesh = match self.items[i].kind {
                LootKind::Scrap => &mut self.scrap_mesh,
                LootKind::Repair => &mut self.repair_mesh,
                LootKind::Fuel => &mut self.fuel_mesh,
            };
            mesh.write_transformed(&model, ambient);
            mesh.draw();
        }
    }

    // ------------------------------------------------------------------
    // models (glow cores punch through ambient light)
    // ------------------------------------------------------------------
    fn build_scrap() -> PartMesh {
        let rust = Color::new(0.44, 0.26, 0.12, 1.0);
        let dark = Color::new(0.15, 0.13, 0.11, 1.0);
        let amber = Color::new(0.98, 0.66, 0.18, 1.0);
        let mut m = PartMesh::cuboid_at(vec3(0.5, 0.32, 0.42), rust, vec3(0.0, 0.36, 0.0));
        m.merge(&PartMesh::cuboid_at(vec3(0.56, 0.08, 0.48), dark, vec3(0.0, 0.53, 0.0)));
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.3, 0.06, 0.05), amber, vec3(0.0, 0.37, 0.215)));
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.3, 0.06, 0.05), amber, vec3(0.0, 0.37, -0.215)));
        m
    }

    fn build_repair() -> PartMesh {
        let white = Color::new(0.88, 0.88, 0.85, 1.0);
        let red = Color::new(0.92, 0.18, 0.14, 1.0);
        let dark = Color::new(0.2, 0.2, 0.2, 1.0);
        let mut m = PartMesh::cuboid_at(vec3(0.46, 0.3, 0.36), white, vec3(0.0, 0.36, 0.0));
        m.merge(&PartMesh::cuboid_at(vec3(0.5, 0.06, 0.4), dark, vec3(0.0, 0.53, 0.0)));
        // red cross on both faces
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.24, 0.08, 0.03), red, vec3(0.0, 0.37, 0.185)));
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.08, 0.2, 0.03), red, vec3(0.0, 0.37, 0.185)));
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.24, 0.08, 0.03), red, vec3(0.0, 0.37, -0.185)));
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.08, 0.2, 0.03), red, vec3(0.0, 0.37, -0.185)));
        m
    }

    fn build_fuel() -> PartMesh {
        let red = Color::new(0.62, 0.13, 0.09, 1.0);
        let dark = Color::new(0.16, 0.13, 0.11, 1.0);
        let amber = Color::new(0.95, 0.58, 0.12, 1.0);
        let mut m = PartMesh::cuboid_at(vec3(0.34, 0.46, 0.26), red, vec3(0.0, 0.4, 0.0));
        m.merge(&PartMesh::cuboid_at(vec3(0.12, 0.12, 0.12), dark, vec3(0.09, 0.68, 0.0))); // cap
        m.merge(&PartMesh::cuboid_at(vec3(0.1, 0.05, 0.3), dark, vec3(0.0, 0.16, 0.0))); // seam
        m.merge(&PartMesh::cuboid_glow_at(vec3(0.26, 0.07, 0.03), amber, vec3(0.0, 0.42, 0.135))); // label
        m
    }
}
