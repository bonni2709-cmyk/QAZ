//! BOMBER DRONE — a one-pass strafing set-piece on the highway.
//!
//! The sky goes quiet, a klaxon sounds, and a maintenance-turned-murder
//! drone sweeps across the road at ~30 m up, dropping three dumb bombs
//! wherever it overflies. It cannot be shot down — there is no fair fight
//! here, only evasive driving. Each bomb is telegraphed by a painted-amber
//! blast decal so the dodging is readable at speed.
//!
//! Structure: a single `BomberDrone` (never two in the air) with a fixed
//! pool of three falling bombs. No allocation after construction.

use macroquad::prelude::*;

use crate::systems::meshkit::PartMesh;
use crate::systems::particles::ParticlePool;

const BOMB_CAPACITY: usize = 3;
const BOMB_DAMAGE: f32 = 26.0;
const BOMB_RADIUS: f32 = 5.5;

#[derive(Clone, Copy)]
struct DroneBomb {
    pos: Vec3,
    vy: f32,
}

impl DroneBomb {
    const INACTIVE: DroneBomb = DroneBomb {
        pos: Vec3::ZERO,
        vy: 0.0,
    };
}

pub struct BomberDrone {
    active: bool,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    drop_timer: f32,
    bombs_dropped: u32,
    bombs: [Option<DroneBomb>; BOMB_CAPACITY],
    klaxon_side: bool,
    body: PartMesh,
}

impl BomberDrone {
    pub const ALTITUDE: f32 = 30.0;

    pub fn new() -> BomberDrone {
        BomberDrone {
            active: false,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            life: 0.0,
            drop_timer: 0.0,
            bombs_dropped: 0,
            bombs: [None; BOMB_CAPACITY],
            klaxon_side: false,
            body: Self::build_body(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Launch one pass over the road. `side`: -1 enters from the left, +1 from the right.
    pub fn spawn(&mut self, z: f32, side: f32) {
        self.active = true;
        self.pos = vec3(side * 58.0, Self::ALTITUDE, z);
        self.vel = vec3(-side * 36.0, 0.0, -1.5);
        self.life = 0.0;
        self.drop_timer = 0.9;
        self.bombs_dropped = 0;
        self.bombs = [None; BOMB_CAPACITY];
        self.klaxon_side = side > 0.0;
    }

    /// Tick the pass and the falling bombs; returns damage dealt to the rig.
    /// `incoming_scale` is the NG+ lethality multiplier.
    pub fn update(
        &mut self,
        dt: f32,
        truck_pos: Vec3,
        incoming_scale: f32,
        particles: &mut ParticlePool,
    ) -> f32 {
        if !self.active {
            return 0.0;
        }
        self.life += dt;
        self.pos += self.vel * dt;
        self.drop_timer -= dt;

        // ---- release, one bomb every 1.05 s while over the highway ----
        if self.bombs_dropped < BOMB_CAPACITY as u32 && self.drop_timer <= 0.0 && self.pos.x.abs() < 26.0 {
            self.drop_timer = 1.05;
            self.bombs_dropped += 1;
            for slot in self.bombs.iter_mut() {
                if slot.is_none() {
                    *slot = Some(DroneBomb {
                        pos: self.pos - vec3(0.0, 1.1, 0.0),
                        vy: 2.0,
                    });
                    break;
                }
            }
        }

        // ---- bombs fall and burst ----
        let mut damage = 0.0f32;
        for slot in self.bombs.iter_mut() {
            let Some(bomb) = slot else {
                continue;
            };
            bomb.vy += 16.0 * dt;
            bomb.pos.y -= bomb.vy * dt;
            if bomb.pos.y <= 0.15 {
                particles.explosion(bomb.pos, 1.35);
                let dx = truck_pos.x - bomb.pos.x;
                let dz = truck_pos.z - bomb.pos.z;
                let d = (dx * dx + dz * dz).sqrt();
                if d < BOMB_RADIUS {
                    let falloff = 1.0 - (d / BOMB_RADIUS).clamp(0.0, 1.0);
                    damage += (BOMB_DAMAGE * (0.45 + 0.55 * falloff)) * incoming_scale;
                }
                *slot = None;
            }
        }

        // ---- the pass ends when it clears the far shoulder ----
        if self.pos.x.abs() > 62.0 || self.life > 11.0 {
            self.active = false;
        }
        damage
    }

    pub fn draw(&mut self, ambient: f32, time: f32) {
        if self.active {
            let yaw = self.vel.x.atan2(self.vel.z);
            let world = Mat4::from_translation(self.pos) * Mat4::from_rotation_y(yaw);
            self.body.write_transformed(&world, ambient);
            self.body.draw();

            // belly beacon — the blink that tells you it made this pass on purpose
            let blink = (time * 9.0).sin() > 0.2;
            if blink {
                draw_sphere(
                    self.pos + vec3(0.0, -0.5, 0.0),
                    0.22,
                    None,
                    Color::new(1.0, 0.15, 0.1, 1.0),
                );
            }
        }

        // ---- bombs in the air + painted blast decals on the asphalt ----
        for slot in &self.bombs {
            let Some(bomb) = slot else {
                continue;
            };
            // falling bomb: dark teardrop + whistle-lit tail
            draw_cube(bomb.pos, vec3(0.36, 0.66, 0.36), None, Color::new(0.09, 0.09, 0.10, 1.0));
            draw_cube(bomb.pos + vec3(0.0, 0.42, 0.0), vec3(0.14, 0.2, 0.14), None, Color::new(0.85, 0.42, 0.1, 1.0));
            // blast decal: pulsing amber scorch square where it will land
            let pulse = 0.35 + 0.3 * (time * 11.0).sin().abs();
            draw_plane(
                vec3(bomb.pos.x, 0.03, bomb.pos.z),
                vec2(BOMB_RADIUS * 2.0, BOMB_RADIUS * 2.0),
                None,
                Color::new(1.0, 0.42, 0.08, pulse),
            );
        }
    }

    fn build_body() -> PartMesh {
        let hull = Color::new(0.16, 0.16, 0.18, 1.0);
        let dark = Color::new(0.09, 0.09, 0.10, 1.0);
        let glass = Color::new(0.05, 0.09, 0.12, 1.0);
        let rust = Color::new(0.42, 0.21, 0.09, 1.0);
        let glow = Color::new(1.0, 0.62, 0.1, 1.0);

        let mut b = PartMesh::cuboid_at(vec3(2.6, 0.55, 1.7), hull, vec3(0.0, 0.0, 0.0));
        // beak nose
        b.merge(&PartMesh::cuboid_at(vec3(0.9, 0.32, 0.5), dark, vec3(0.0, -0.02, 1.08)));
        // cockpit slit
        b.merge(&PartMesh::cuboid_at(vec3(1.1, 0.14, 0.06), glass, vec3(0.0, 0.08, 0.86)));
        // shoulder slabs
        b.merge(&PartMesh::cuboid_at(vec3(0.72, 0.3, 1.1), rust, vec3(-1.45, 0.05, -0.05)));
        b.merge(&PartMesh::cuboid_at(vec3(0.72, 0.3, 1.1), rust, vec3(1.45, 0.05, -0.05)));
        // rotor stubs
        b.merge(&PartMesh::cylinder_at(0.05, 0.05, 0.32, 6, dark, vec3(-1.45, 0.34, -0.05)));
        b.merge(&PartMesh::cylinder_at(0.05, 0.05, 0.32, 6, dark, vec3(1.45, 0.34, -0.05)));
        // bomb bay + hazard strip
        b.merge(&PartMesh::cuboid_at(vec3(0.9, 0.09, 0.75), dark, vec3(0.0, -0.32, 0.0)));
        b.merge(&PartMesh::cuboid_glow_at(vec3(2.0, 0.07, 0.09), glow, vec3(0.0, -0.29, 0.62)));
        b
    }
}
