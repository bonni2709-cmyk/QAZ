//! Boss #2 — THE CRAWLER, a burrowing horror from under the asphalt.
//!
//! Alternates with the JUGGERNAUT on the distance odometer. Where the
//! tanker is armor and artillery, the Crawler is flesh and patience: it
//! swims through the shoulder sand, surfaces ahead of the rig and cuts
//! across lanes to intercept. No ranged attack — being *touched* by it
//! is the attack.
//!
//! * **Movement** — head steers toward the rig at 1.9 rad/s; speed bands
//!   rub out rubber (faster when far ahead, slower right on top of you).
//! * **Body** — six segments chain-follow the head on a fixed 2.35 m
//!   spacing, so the whole spine weaves through corners behind it.
//! * **Contact** — any segment within 2.4 m is 16 hull damage (1.1 s
//!   cooldown shared across the body).
//! * **Vulnerability** — incendiary deals ×1.5 (it *hates* fire), AP ×0.8.
//!
//! Structure: a single `CrawlerWorm` (only one exists at a time). Meshes
//! are built once at spawn; per-frame cost is transformed cuboids, no
//! allocation.

use macroquad::prelude::*;

use crate::systems::meshkit::PartMesh;

const SPACING: f32 = 2.35;
const CONTACT_RADIUS: f32 = 2.4;
const CONTACT_DAMAGE: f32 = 16.0;
const CONTACT_CD: f32 = 1.1;

pub struct CrawlerWorm {
    pub hp: f32,
    pub max_hp: f32,
    head: Vec3,
    yaw: f32,
    segments: [Vec3; 6],
    contact_cd: f32,
    bob: f32,
    head_mesh: PartMesh,
    seg_big: PartMesh,
    seg_mid: PartMesh,
    seg_small: PartMesh,
}

impl CrawlerWorm {
    pub const NAME: &'static str = "CRAWLER";
    pub const MAX_HP: f32 = 640.0;

    pub fn spawn(z: f32) -> CrawlerWorm {
        let mut segments = [Vec3::ZERO; 6];
        for (i, seg) in segments.iter_mut().enumerate() {
            *seg = vec3(0.0, 0.0, z - (i as f32 + 1.0) * SPACING);
        }
        CrawlerWorm {
            hp: Self::MAX_HP,
            max_hp: Self::MAX_HP,
            head: vec3(0.0, 0.0, z),
            yaw: 0.0,
            segments,
            contact_cd: 0.0,
            bob: 0.0,
            head_mesh: Self::build_head(),
            seg_big: PartMesh::cuboid_at(vec3(2.35, 1.55, 2.1), Color::new(0.24, 0.22, 0.14, 1.0), Vec3::ZERO),
            seg_mid: PartMesh::cuboid_at(vec3(1.85, 1.25, 1.85), Color::new(0.21, 0.19, 0.12, 1.0), Vec3::ZERO),
            seg_small: PartMesh::cuboid_at(vec3(1.35, 0.95, 1.6), Color::new(0.17, 0.16, 0.10, 1.0), Vec3::ZERO),
        }
    }

    /// World position of the head (for explosions / scrap showers).
    pub fn pos(&self) -> Vec3 {
        self.head
    }

    /// Head's odometer coordinate (for the escape valve).
    pub fn z(&self) -> f32 {
        self.head.z
    }

    pub fn hp_ratio(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    /// Incendiary ×1.5 — flesh burns; AP ×0.8 — psionic chitin shrugs it off.
    pub fn apply_damage(&mut self, damage: f32, incendiary: bool) -> bool {
        let dealt = if incendiary { damage * 1.5 } else { damage * 0.8 };
        self.hp -= dealt;
        self.hp <= 0.0
    }

    /// Nearest horizontal range from `p` to any part of the body (for prompts).
    pub fn horizontal_distance_to(&self, p: Vec3) -> f32 {
        let mut best = self.flat_dist(self.head, p);
        for seg in &self.segments {
            best = best.min(self.flat_dist(*seg, p));
        }
        best
    }

    /// Sphere tests for the player's projectiles: (center, radius) per part.
    pub fn hit_spheres(&self) -> [(Vec3, f32); 7] {
        [
            (self.head + vec3(0.0, 1.1, 0.0), 1.75),
            (self.segments[0] + vec3(0.0, 0.9, 0.0), 1.55),
            (self.segments[1] + vec3(0.0, 0.85, 0.0), 1.4),
            (self.segments[2] + vec3(0.0, 0.8, 0.0), 1.2),
            (self.segments[3] + vec3(0.0, 0.75, 0.0), 1.05),
            (self.segments[4] + vec3(0.0, 0.7, 0.0), 0.9),
            (self.segments[5] + vec3(0.0, 0.65, 0.0), 0.75),
        ]
    }

    /// Advance the head and the chain; returns damage dealt to the rig this frame.
    pub fn update(&mut self, dt: f32, truck_pos: Vec3) -> f32 {
        // ---- rubber-band: hang on the back of the rig, faster from far ----
        let gap = self.head.z - truck_pos.z;
        let speed = if gap > 60.0 {
            25.0
        } else if gap > 8.0 {
            21.5
        } else if gap > -4.0 {
            17.0 // right on top of you — but you can feel it coming
        } else {
            11.0 // ahead of the rig: it waits for you, it is patient
        };

        // ---- steer toward the rig ----
        let to = truck_pos - self.head;
        let target_yaw = to.x.atan2(to.z);
        let mut delta = target_yaw - self.yaw;
        // wrap to [-PI, PI]
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        let max_turn = 1.9 * dt;
        self.yaw += delta.clamp(-max_turn, max_turn);
        self.head += vec3(self.yaw.sin(), 0.0, self.yaw.cos()) * speed * dt;

        // ---- body chain-follows with fixed spacing ----
        let mut prev = self.head;
        for seg in self.segments.iter_mut() {
            let delta = prev - *seg;
            let d = delta.length();
            if d > SPACING {
                *seg = prev - delta / d * SPACING;
            }
            prev = *seg;
        }

        self.contact_cd = (self.contact_cd - dt).max(0.0);
        self.bob += dt * 2.6;

        // ---- contact damage ----
        if self.contact_cd <= 0.0 && self.horizontal_distance_to(truck_pos) < CONTACT_RADIUS {
            self.contact_cd = CONTACT_CD;
            return CONTACT_DAMAGE;
        }
        0.0
    }

    #[inline]
    fn flat_dist(&self, a: Vec3, b: Vec3) -> f32 {
        let dx = a.x - b.x;
        let dz = a.z - b.z;
        (dx * dx + dz * dz).sqrt()
    }

    pub fn draw(&mut self, ambient: f32, time: f32) {
        let bob = (self.bob).sin() * 0.22;

        // ---- head ----
        let pulse = 1.0 + 0.05 * (time * 7.0).sin();
        let head_world = Mat4::from_translation(self.head + vec3(0.0, 1.05 + bob, 0.0))
            * Mat4::from_rotation_y(self.yaw)
            * Mat4::from_scale(Vec3::splat(pulse));
        self.head_mesh.write_transformed(&head_world, ambient);
        self.head_mesh.draw();

        // ---- segments, tapering into the tail ----
        for (i, seg) in self.segments.iter().enumerate() {
            let seg_bob = bob * (1.0 - i as f32 * 0.16);
            let yaw = if i == 0 {
                (self.head.x - seg.x).atan2(self.head.z - seg.z)
            } else {
                (self.segments[i - 1].x - seg.x).atan2(self.segments[i - 1].z - seg.z)
            };
            let center = *seg + vec3(0.0, 0.85 - i as f32 * 0.09 + seg_bob, 0.0);
            let world = Mat4::from_translation(center) * Mat4::from_rotation_y(yaw);
            let mesh = match i {
                0..=1 => &mut self.seg_big,
                2..=3 => &mut self.seg_mid,
                _ => &mut self.seg_small,
            };
            mesh.write_transformed(&world, ambient);
            mesh.draw();
        }

    }

    fn build_head() -> PartMesh {
        let chitin = Color::new(0.28, 0.25, 0.13, 1.0);
        let dark = Color::new(0.12, 0.11, 0.07, 1.0);
        let rust = Color::new(0.45, 0.22, 0.10, 1.0);
        let glow = Color::new(1.0, 0.55, 0.08, 1.0);

        let mut h = PartMesh::cuboid_at(vec3(2.75, 1.65, 2.35), chitin, vec3(0.0, 0.0, 0.0));
        // brow plate
        h.merge(&PartMesh::cuboid_at(vec3(2.9, 0.34, 0.7), dark, vec3(0.0, 0.72, 0.72)));
        // mandibles
        h.merge(&PartMesh::cylinder_along_z_at(0.09, 0.28, 1.15, 6, rust, vec3(-0.78, -0.32, 1.35)));
        h.merge(&PartMesh::cylinder_along_z_at(0.09, 0.28, 1.15, 6, rust, vec3(0.78, -0.32, 1.35)));
        // eyes — two amber coals set deep
        h.merge(&PartMesh::cuboid_glow_at(vec3(0.34, 0.17, 0.07), glow, vec3(-0.58, 0.26, 1.19)));
        h.merge(&PartMesh::cuboid_glow_at(vec3(0.34, 0.17, 0.07), glow, vec3(0.58, 0.26, 1.19)));
        // dorsal ridge
        h.merge(&PartMesh::cuboid_at(vec3(0.55, 0.3, 1.7), dark, vec3(0.0, 0.95, -0.15)));
        h
    }
}

