//! Enemy machines: Raider Biker, Junker Buggy, Armored Car.
//!
//! Fixed-capacity pool. Behaviour is classic pursuit AI, per kind:
//! * **Biker** — fast, weaves on a sine through traffic, fires an SMG on a cooldown
//!   when it gets a clean shot; rams at close range.
//! * **Buggy** — slower, jerks sideways, and periodically *jumps* (ballistic hop);
//!   rams and hurts itself doing it.
//! * **Armored Car** — keeps ~50 m stand-off distance ahead of the rig, sidesteps
//!   lazily, and shells the truck with rockets on a long cooldown (telegraphed
//!   by a glowing muzzle). Weak to tungsten AP, shrugs off incendiary.
//!
//! Allocation discipline: events are fixed-size structs with a counter; nothing
//! here allocates after construction.

use macroquad::prelude::*;

use crate::systems::meshkit::PartMesh;
use crate::systems::particles::ParticlePool;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Biker,
    Buggy,
    Armored,
}

impl EnemyKind {
    pub fn is_armored(self) -> bool {
        matches!(self, EnemyKind::Armored)
    }

    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Biker => 1.15,
            EnemyKind::Buggy => 1.55,
            EnemyKind::Armored => 2.35,
        }
    }

    fn max_hp(self) -> f32 {
        match self {
            EnemyKind::Biker => 30.0,
            EnemyKind::Buggy => 48.0,
            EnemyKind::Armored => 150.0,
        }
    }

    fn top_speed(self) -> f32 {
        match self {
            EnemyKind::Biker => 27.5,
            EnemyKind::Buggy => 23.5,
            EnemyKind::Armored => 21.0,
        }
    }

    fn turn_rate(self) -> f32 {
        match self {
            EnemyKind::Biker => 2.6,
            EnemyKind::Buggy => 1.9,
            EnemyKind::Armored => 1.5,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RocketSpawn {
    pub origin: Vec3,
    pub velocity: Vec3,
    pub damage: f32,
}

#[derive(Clone, Copy)]
pub struct SmgShot {
    pub from: Vec3,
    pub to: Vec3,
    pub hit_chance: f32,
    pub damage: f32,
}

const MAX_EVENTS: usize = 8;

/// Scratch buffer for one simulation frame. `clear` keeps the backing stores hot.
pub struct EnemyFrameEvents {
    rockets: [Option<RocketSpawn>; MAX_EVENTS],
    rocket_n: usize,
    smg: [Option<SmgShot>; MAX_EVENTS],
    smg_n: usize,
}

impl EnemyFrameEvents {
    pub fn new() -> EnemyFrameEvents {
        EnemyFrameEvents {
            rockets: [None; MAX_EVENTS],
            rocket_n: 0,
            smg: [None; MAX_EVENTS],
            smg_n: 0,
        }
    }

    fn push_rocket(&mut self, r: RocketSpawn) {
        if self.rocket_n < MAX_EVENTS {
            self.rockets[self.rocket_n] = Some(r);
            self.rocket_n += 1;
        }
    }

    fn push_smg(&mut self, s: SmgShot) {
        if self.smg_n < MAX_EVENTS {
            self.smg[self.smg_n] = Some(s);
            self.smg_n += 1;
        }
    }

    pub fn rockets(&self) -> impl Iterator<Item = &RocketSpawn> {
        self.rockets[..self.rocket_n].iter().filter_map(|r| r.as_ref())
    }

    pub fn smg_shots(&self) -> impl Iterator<Item = &SmgShot> {
        self.smg[..self.smg_n].iter().filter_map(|s| s.as_ref())
    }

    fn clear(&mut self) {
        self.rocket_n = 0;
        self.smg_n = 0;
    }
}

#[derive(Clone, Copy)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: Vec3,
    pub yaw: f32,
    pub speed: f32,
    pub hp: f32,
    pub burn_time: f32,
    pub burn_dps: f32,
    pub active: bool,
    phase: f32,
    fire_cooldown: f32,
    rocket_cooldown: f32,
    jump_cooldown: f32,
    vy: f32,
    grounded: bool,
}

impl Enemy {
    const INACTIVE: Enemy = Enemy {
        kind: EnemyKind::Biker,
        pos: Vec3::ZERO,
        yaw: 0.0,
        speed: 0.0,
        hp: 1.0,
        burn_time: 0.0,
        burn_dps: 0.0,
        active: false,
        phase: 0.0,
        fire_cooldown: 0.0,
        rocket_cooldown: 0.0,
        jump_cooldown: 0.0,
        vy: 0.0,
        grounded: true,
    };

    /// Velocity vector (used for rocket lead prediction and Android auto-aim).
    #[cfg(target_os = "android")]
    pub fn velocity(&self) -> Vec3 {
        vec3(self.yaw.sin(), 0.0, self.yaw.cos()) * self.speed
    }

    /// Shove the buggy away from what it just rammed.
    pub fn bounce(&mut self, away: Vec3) {
        self.pos += away * 1.7;
        self.vy = 3.2;
        self.grounded = false;
    }

    /// Fraction of muzzle glow telegraph remaining, 0 = idle/firing, 1 = about to fire.
    pub fn telegraph(&self) -> f32 {
        if self.kind != EnemyKind::Armored {
            return 0.0;
        }
        const TELEGRAPH: f32 = 0.65;
        if self.rocket_cooldown < TELEGRAPH && self.rocket_cooldown > 0.0 {
            1.0 - self.rocket_cooldown / TELEGRAPH
        } else {
            0.0
        }
    }
}

const POOL_CAPACITY: usize = 32;
const TRUCK_RADIUS: f32 = 1.95;

pub struct EnemyPool {
    items: Vec<Enemy>,
    biker: PartMesh,
    buggy: PartMesh,
    armored: PartMesh,
    rng: u64,
}

#[inline]
fn wrap_angle(mut a: f32) -> f32 {
    const TAU: f32 = std::f32::consts::TAU;
    while a > std::f32::consts::PI {
        a -= TAU;
    }
    while a < -std::f32::consts::PI {
        a += TAU;
    }
    a
}

#[inline]
fn turn_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    current + wrap_angle(target - current).clamp(-max_delta, max_delta)
}

impl EnemyPool {
    pub fn new() -> EnemyPool {
        EnemyPool {
            items: vec![Enemy::INACTIVE; POOL_CAPACITY],
            biker: Self::build_biker(),
            buggy: Self::build_buggy(),
            armored: Self::build_armored(),
            rng: 0x9E37_79B9_7F4A_7C15,
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

    pub fn spawn(&mut self, kind: EnemyKind, pos: Vec3) -> bool {
        let phase = self.rand_range(0.0, std::f32::consts::TAU);
        let fire_cooldown = self.rand_range(0.7, 1.6);
        let rocket_cooldown = self.rand_range(2.2, 3.4);
        let jump_cooldown = self.rand_range(0.6, 1.6);
        for e in self.items.iter_mut() {
            if !e.active {
                *e = Enemy {
                    kind,
                    pos,
                    yaw: 0.0,
                    speed: kind.top_speed() * 0.55,
                    hp: kind.max_hp(),
                    burn_time: 0.0,
                    burn_dps: 0.0,
                    active: true,
                    phase,
                    fire_cooldown,
                    rocket_cooldown,
                    jump_cooldown,
                    vy: 0.0,
                    grounded: true,
                };
                return true;
            }
        }
        false
    }

    pub fn active_count(&self) -> usize {
        self.items.iter().filter(|e| e.active).count()
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Raw access for the collision pass (driving iterates slots, tests, applies damage).
    pub fn get(&self, index: usize) -> Option<&Enemy> {
        self.items.get(index).filter(|e| e.active)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Enemy> {
        self.items.get_mut(index).filter(|e| e.active)
    }

    pub fn deactivate(&mut self, index: usize) {
        if let Some(e) = self.items.get_mut(index) {
            e.active = false;
        }
    }

    pub fn apply_burn(&mut self, index: usize, dps: f32, duration: f32) {
        if let Some(e) = self.items.get_mut(index) {
            e.burn_dps = e.burn_dps.max(dps);
            e.burn_time = e.burn_time.max(duration);
        }
    }

    /// Nearest active enemy to `pos` (for Android auto-aim). Returns index.
    #[cfg(target_os = "android")]
    pub fn nearest_to(&self, pos: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, e) in self.items.iter().enumerate() {
            if !e.active {
                continue;
            }
            let d = e.pos.distance_squared(pos);
            match best {
                Some((_, bd)) if bd <= d => {}
                _ => best = Some((i, d)),
            }
        }
        best.map(|(i, _)| i)
    }

    /// Advance AI. Writes rocket/SMG requests into `events`.
    pub fn update(
        &mut self,
        dt: f32,
        truck_pos: Vec3,
        truck_vel: Vec3,
        time: f32,
        events: &mut EnemyFrameEvents,
        particles: &mut ParticlePool,
    ) {
        events.clear();

        for e in self.items.iter_mut() {
            if !e.active {
                continue;
            }

            // burn DoT (incendiary rounds) — visual throttled to ~11 puffs/s per enemy
            if e.burn_time > 0.0 {
                e.hp -= e.burn_dps * dt;
                e.burn_time -= dt;
                if (time * 11.0 + e.phase).fract() < dt * 11.0 {
                    particles.burn_touch(e.pos);
                }
            }
            if e.hp <= 0.0 {
                continue; // death handled by the driving state (it needs kill rewards)
            }

            let to_truck = truck_pos - e.pos;
            let dist = Vec2::new(to_truck.x, to_truck.z).length().max(0.001);
            let perp = vec2(-to_truck.z, to_truck.x) / dist; // left of travel direction of truck-facing vector

            match e.kind {
                EnemyKind::Biker => {
                    let weave = (time * 1.9 + e.phase).sin() * 2.3;
                    let target = vec2(truck_pos.x, truck_pos.z) + perp * weave;
                    let desired_yaw = (target.x - e.pos.x).atan2(target.y - e.pos.z);
                    e.yaw = turn_toward(e.yaw, desired_yaw, e.kind.turn_rate() * dt);
                    e.speed += (e.kind.top_speed() - e.speed) * (1.0 - (-dt * 1.6).exp());
                    e.pos += vec3(e.yaw.sin(), 0.0, e.yaw.cos()) * e.speed * dt;

                    // SMG — only when roughly facing the rig and inside 55 m
                    e.fire_cooldown -= dt;
                    if e.fire_cooldown <= 0.0 && dist < 55.0 {
                        e.fire_cooldown = 0.95 + e.phase.frac() * 0.35;
                        let gun = e.pos + vec3(0.0, 1.15, 0.0) + vec3(e.yaw.sin(), 0.0, e.yaw.cos()) * 0.6;
                        let jitter = vec3(
                            (time * 13.7 + e.phase * 7.0).sin() * 1.1,
                            (time * 11.3 + e.phase * 5.0).sin() * 0.8,
                            (time * 15.1 + e.phase * 9.0).cos() * 1.1,
                        );
                        let falloff = (dist / 55.0).clamp(0.0, 1.0);
                        events.push_smg(SmgShot {
                            from: gun,
                            to: truck_pos + vec3(0.0, 1.6, 0.0) + jitter,
                            hit_chance: 0.78 - falloff * 0.45,
                            damage: 5.0 - falloff * 3.5,
                        });
                    }
                }
                EnemyKind::Buggy => {
                    let weave = (time * 2.6 + e.phase).sin() * 3.3;
                    let target = vec2(truck_pos.x, truck_pos.z) + perp * weave;
                    let desired_yaw = (target.x - e.pos.x).atan2(target.y - e.pos.z);
                    e.yaw = turn_toward(e.yaw, desired_yaw, e.kind.turn_rate() * dt);
                    e.speed += (e.kind.top_speed() - e.speed) * (1.0 - (-dt * 1.4).exp());
                    e.pos += vec3(e.yaw.sin(), 0.0, e.yaw.cos()) * e.speed * dt;

                    // ballistic hop
                    e.jump_cooldown -= dt;
                    if e.grounded && e.jump_cooldown <= 0.0 && dist < 48.0 {
                        e.vy = 6.4;
                        e.grounded = false;
                        e.jump_cooldown = 2.1 + e.phase.frac() * 1.1;
                    }
                    if !e.grounded {
                        e.vy -= 13.5 * dt;
                        e.pos.y += e.vy * dt;
                        if e.pos.y <= 0.0 {
                            e.pos.y = 0.0;
                            e.vy = 0.0;
                            e.grounded = true;
                            particles.dust(e.pos);
                        }
                    }
                }
                EnemyKind::Armored => {
                    // hold ~50 m stand-off, lazy sidestep
                    let drift = (time * 0.7 + e.phase).sin() * 9.0;
                    let target = vec2(truck_pos.x, truck_pos.z) + perp * drift;
                    let desired_yaw = (target.x - e.pos.x).atan2(target.y - e.pos.z);
                    e.yaw = turn_toward(e.yaw, desired_yaw, e.kind.turn_rate() * dt);
                    let desired_speed = ((dist - 46.0) * 0.55).clamp(6.0, e.kind.top_speed());
                    e.speed += (desired_speed - e.speed) * (1.0 - (-dt * 1.8).exp());
                    e.pos += vec3(e.yaw.sin(), 0.0, e.yaw.cos()) * e.speed * dt;

                    // rockets on a long cooldown, aimed with partial lead
                    e.rocket_cooldown -= dt;
                    if e.rocket_cooldown <= 0.0 && dist < 95.0 {
                        e.rocket_cooldown = 2.9 + e.phase.frac() * 1.0;
                        let muzzle = e.pos
                            + vec3(0.0, 1.92, 0.0)
                            + vec3(e.yaw.sin(), 0.0, e.yaw.cos()) * 1.35;
                        let lead_time = dist / 42.0;
                        let predicted = truck_pos + truck_vel * lead_time * 0.72;
                        let mut dir = predicted + vec3(0.0, 1.6, 0.0) - muzzle;
                        let len = dir.length().max(0.001);
                        dir /= len;
                        dir.y += 0.035;
                        let dir = dir.normalize();
                        events.push_rocket(RocketSpawn {
                            origin: muzzle,
                            velocity: dir * 42.0,
                            damage: 24.0,
                        });
                    }
                }
            }
        }

        // gentle separation so traffic doesn't stack up
        for i in 0..self.items.len() {
            if !self.items[i].active {
                continue;
            }
            for j in (i + 1)..self.items.len() {
                if !self.items[j].active {
                    continue;
                }
                let a = self.items[i].pos;
                let b = self.items[j].pos;
                let delta = vec2(b.x - a.x, b.z - a.z);
                let dist = delta.length();
                let min_dist = self.items[i].kind.radius() + self.items[j].kind.radius();
                if dist < min_dist && dist > 0.001 {
                    let push = (min_dist - dist) * 0.5;
                    let n = delta / dist;
                    self.items[i].pos.x -= n.x * push;
                    self.items[i].pos.z -= n.y * push;
                    self.items[j].pos.x += n.x * push;
                    self.items[j].pos.z += n.y * push;
                }
            }
        }

        // despawn anything hopelessly behind / too far ahead
        for e in self.items.iter_mut() {
            if !e.active {
                continue;
            }
            let dz = e.pos.z - truck_pos.z;
            let horizontal = Vec2::new(e.pos.x - truck_pos.x, dz).length();
            if dz < -95.0 || dz > 320.0 || horizontal > 320.0 {
                e.active = false;
            }
        }
    }

    /// Damage an enemy; returns true if it died from this hit.
    pub fn apply_damage(&mut self, index: usize, damage: f32) -> bool {
        if let Some(e) = self.items.get_mut(index) {
            e.hp -= damage;
            e.hp <= 0.0
        } else {
            false
        }
    }

    pub fn draw(&mut self, ambient: f32) {
        for i in 0..self.items.len() {
            if !self.items[i].active {
                continue;
            }
            let pos = self.items[i].pos;
            let yaw = self.items[i].yaw;
            let kind = self.items[i].kind;
            let model = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
            // shared model buffers are re-baked per enemy right before drawing —
            // one transform → one draw, no extra cost for N enemies.
            let mesh = match kind {
                EnemyKind::Biker => &mut self.biker,
                EnemyKind::Buggy => &mut self.buggy,
                EnemyKind::Armored => &mut self.armored,
            };
            mesh.write_transformed(&model, ambient);
            mesh.draw();

            if kind == EnemyKind::Armored {
                let telegraph = self.items[i].telegraph();
                if telegraph > 0.0 {
                    let gun = pos
                        + vec3(0.0, 1.92, 0.0)
                        + vec3(yaw.sin(), 0.0, yaw.cos()) * 1.45;
                    let s = 0.10 + 0.16 * telegraph;
                    draw_cube(
                        gun,
                        vec3(s, s, s),
                        None,
                        Color::new(1.0, 0.25 + 0.5 * telegraph, 0.05, 1.0),
                    );
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // models (offsets baked; draw-time rotation via PartMesh only)
    // ------------------------------------------------------------------
    fn build_biker() -> PartMesh {
        let frame = Color::new(0.10, 0.10, 0.11, 1.0);
        let leather = Color::new(0.17, 0.14, 0.13, 1.0);
        let red = Color::new(0.52, 0.12, 0.09, 1.0);
        let steel = Color::new(0.36, 0.35, 0.33, 1.0);
        let mut bike = PartMesh::cuboid_at(vec3(0.24, 0.16, 1.75), frame, vec3(0.0, 0.72, 0.0));
        bike.merge(&PartMesh::cuboid_at(vec3(0.32, 0.18, 0.62), red, vec3(0.0, 0.88, -0.1)));
        bike.merge(&PartMesh::cylinder_along_x_at(0.33, 0.33, 0.13, 10, frame, vec3(0.0, 0.33, 0.72)));
        bike.merge(&PartMesh::cylinder_along_x_at(0.33, 0.33, 0.13, 10, frame, vec3(0.0, 0.33, -0.72)));
        bike.merge(&PartMesh::cuboid_at(vec3(0.08, 0.62, 0.08), steel, vec3(0.0, 0.98, 0.66)));
        bike.merge(&PartMesh::cuboid_at(vec3(0.72, 0.05, 0.05), steel, vec3(0.0, 1.28, 0.62)));
        bike.merge(&PartMesh::cuboid_at(vec3(0.44, 0.5, 0.3), leather, vec3(0.0, 1.32, 0.05))); // torso
        bike.merge(&PartMesh::cuboid_at(vec3(0.14, 0.17, 0.32), leather, vec3(-0.30, 1.47, 0.02))); // shoulder pads
        bike.merge(&PartMesh::cuboid_at(vec3(0.14, 0.17, 0.32), leather, vec3(0.30, 1.47, 0.02)));
        bike.merge(&PartMesh::cuboid_at(vec3(0.25, 0.23, 0.26), red, vec3(0.0, 1.76, 0.08))); // helmet
        bike.merge(&PartMesh::cuboid_at(vec3(0.19, 0.07, 0.02), Color::new(0.04, 0.05, 0.06, 1.0), vec3(0.0, 1.74, 0.215))); // visor
        bike.merge(&PartMesh::cuboid_at(vec3(0.10, 0.34, 0.52), steel, vec3(0.16, 0.68, -0.35))); // exhaust
        bike
    }

    fn build_buggy() -> PartMesh {
        let frame = Color::new(0.23, 0.2, 0.12, 1.0);
        let dark = Color::new(0.12, 0.11, 0.10, 1.0);
        let cage = Color::new(0.32, 0.31, 0.28, 1.0);
        let amber = Color::new(0.95, 0.6, 0.12, 1.0);
        let mut buggy = PartMesh::cuboid_at(vec3(1.5, 0.3, 2.7), frame, vec3(0.0, 0.56, 0.0));
        buggy.merge(&PartMesh::cuboid_at(vec3(1.3, 0.24, 0.9), dark, vec3(0.0, 0.82, 0.78))); // hood
        // roll cage: four posts + roof
        for &(x, z) in &[(-0.62f32, 0.35f32), (0.62, 0.35), (-0.62, -0.75), (0.62, -0.75)] {
            buggy.merge(&PartMesh::cuboid_at(vec3(0.09, 0.78, 0.09), cage, vec3(x, 1.06, z)));
        }
        buggy.merge(&PartMesh::cuboid_at(vec3(1.34, 0.1, 1.35), cage, vec3(0.0, 1.47, -0.2)));
        buggy.merge(&PartMesh::cylinder_along_x_at(0.43, 0.43, 0.3, 12, dark, vec3(-0.8, 0.43, 0.88)));
        buggy.merge(&PartMesh::cylinder_along_x_at(0.43, 0.43, 0.3, 12, dark, vec3(0.8, 0.43, 0.88)));
        buggy.merge(&PartMesh::cylinder_along_x_at(0.43, 0.43, 0.3, 12, dark, vec3(-0.8, 0.43, -0.88)));
        buggy.merge(&PartMesh::cylinder_along_x_at(0.43, 0.43, 0.3, 12, dark, vec3(0.8, 0.43, -0.88)));
        buggy.merge(&PartMesh::cuboid_at(vec3(0.3, 0.26, 0.3), dark, vec3(0.0, 1.12, -0.25))); // driver helmet
        buggy.merge(&PartMesh::cuboid_glow_at(vec3(0.9, 0.09, 0.09), amber, vec3(0.0, 1.28, 1.16))); // light bar
        buggy
    }

    fn build_armored() -> PartMesh {
        let hull = Color::new(0.22, 0.23, 0.17, 1.0);
        let dark = Color::new(0.12, 0.12, 0.11, 1.0);
        let plate = Color::new(0.3, 0.19, 0.1, 1.0);
        let steel = Color::new(0.34, 0.33, 0.31, 1.0);
        let mut car = PartMesh::cuboid_at(vec3(2.3, 0.78, 4.7), hull, vec3(0.0, 1.14, 0.0));
        car.merge(&PartMesh::cuboid_at(vec3(2.0, 0.34, 1.0), hull, vec3(0.0, 0.86, 2.15))); // glacis hump
        car.merge(&PartMesh::cuboid_at(vec3(0.14, 0.4, 3.3), plate, vec3(-1.2, 1.22, 0.0))); // side plates
        car.merge(&PartMesh::cuboid_at(vec3(0.14, 0.4, 3.3), plate, vec3(1.2, 1.22, 0.0)));
        car.merge(&PartMesh::cylinder_at(0.56, 0.62, 0.42, 12, dark, vec3(0.0, 1.53, -0.3))); // turret ring
        car.merge(&PartMesh::cuboid_at(vec3(0.94, 0.32, 1.06), hull, vec3(0.0, 1.9, -0.3))); // turret head
        car.merge(&PartMesh::cuboid_at(vec3(0.13, 0.13, 1.65), steel, vec3(0.0, 1.92, 0.62))); // barrel
        car.merge(&PartMesh::cuboid_at(vec3(0.24, 0.22, 0.3), dark, vec3(0.0, 1.92, 1.38))); // muzzle brake
        car.merge(&PartMesh::cuboid_at(vec3(0.42, 0.2, 0.5), dark, vec3(0.0, 1.6, -2.15))); // rear deck box
        for &z in &[1.55f32, 0.0, -1.55] {
            car.merge(&PartMesh::cylinder_along_x_at(0.48, 0.48, 0.36, 12, dark, vec3(-1.06, 0.48, z)));
            car.merge(&PartMesh::cylinder_along_x_at(0.48, 0.48, 0.36, 12, dark, vec3(1.06, 0.48, z)));
        }
        car.merge(&PartMesh::cylinder_at(0.024, 0.024, 1.3, 5, dark, vec3(0.7, 2.06, -2.0))); // whip antenna
        car
    }
}

