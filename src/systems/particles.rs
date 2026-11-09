//! Pooled particle system: fire, smoke, sparks, bouncing 3D debris.
//!
//! Fixed-capacity pools created at load time. No heap traffic after warm-up:
//! `spawn_*` reuses the first inactive slot and silently drops the spawn if the
//! pool is full (better a missed spark than a frame hitch).

use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParticleKind {
    Debris,
    Fire,
    Smoke,
    Spark,
}

#[derive(Clone, Copy)]
struct Particle {
    kind: ParticleKind,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max_life: f32,
    size: f32,
    active: bool,
    /// Acceleration along Y (positive = down, negative = buoyant rise).
    gravity: f32,
    bounce: bool,
}

impl Particle {
    const INACTIVE: Particle = Particle {
        kind: ParticleKind::Smoke,
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        life: 0.0,
        max_life: 1.0,
        size: 0.2,
        active: false,
        gravity: 0.0,
        bounce: false,
    };
}

const POOL_CAPACITY: usize = 720;

pub struct ParticlePool {
    items: Vec<Particle>,
    rng: u64,
}

impl ParticlePool {
    pub fn new() -> ParticlePool {
        ParticlePool {
            items: vec![Particle::INACTIVE; POOL_CAPACITY],
            rng: 0x243F_6A88_85A3_08D3,
        }
    }

    fn rand01(&mut self) -> f32 {
        // xorshift64* — cheap, good enough for effects
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f32 / (1u64 << 53) as f32
    }

    #[inline]
    fn rand_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.rand01() * (hi - lo)
    }

    fn spawn(&mut self, kind: ParticleKind, pos: Vec3, vel: Vec3, life: f32, size: f32, gravity: f32, bounce: bool) {
        for p in self.items.iter_mut() {
            if !p.active {
                *p = Particle {
                    kind,
                    pos,
                    vel,
                    life,
                    max_life: life.max(0.001),
                    size,
                    active: true,
                    gravity,
                    bounce,
                };
                return;
            }
        }
    }

    /// Random unit vector on a sphere (rejection-free-ish, good enough).
    fn random_dir(&mut self) -> Vec3 {
        let z = self.rand_range(-1.0, 1.0);
        let t = self.rand_range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        vec3(r * t.cos(), r * t.sin(), z)
    }

    /// Big explosion: flash + fireball + smoke ring + tumbling debris + sparks.
    /// `scale` 0.6 = fender-bender, 1.0 = rocket, 1.8 = armored car, 2.6 = wrecked rig.
    pub fn explosion(&mut self, pos: Vec3, scale: f32) {
        // central flash
        self.spawn(ParticleKind::Fire, pos + vec3(0.0, 0.4, 0.0), Vec3::ZERO, 0.32, 2.1 * scale, 0.0, false);

        let fire_n = (14.0 * scale) as u32;
        for _ in 0..fire_n {
            let dir = self.random_dir();
            let speed = self.rand_range(2.5, 8.5) * scale;
            let life = self.rand_range(0.5, 1.15);
            let size = self.rand_range(0.35, 0.8) * scale;
            self.spawn(
                ParticleKind::Fire,
                pos + dir * 0.4,
                dir * speed + vec3(0.0, 2.0, 0.0),
                life,
                size,
                -2.4,
                false,
            );
        }

        let smoke_n = (9.0 * scale) as u32;
        for _ in 0..smoke_n {
            let dir = self.random_dir() * 0.6;
            let vy = self.rand_range(1.6, 3.4);
            let life = self.rand_range(0.9, 1.9);
            let size = self.rand_range(0.5, 0.95) * scale;
            self.spawn(
                ParticleKind::Smoke,
                pos + vec3(0.0, 0.8, 0.0) + dir,
                dir * 3.0 + vec3(0.0, vy, 0.0),
                life,
                size,
                -1.1,
                false,
            );
        }

        let debris_n = (9.0 * scale) as u32 + 2;
        for _ in 0..debris_n {
            let dir = self.random_dir();
            let mut vel = dir * self.rand_range(5.0, 11.5) * scale;
            vel.y = vel.y.abs() * 0.75 + 2.5; // always kick some upward
            let life = self.rand_range(1.4, 3.0);
            let size = self.rand_range(0.09, 0.38) * scale;
            self.spawn(
                ParticleKind::Debris,
                pos + vec3(0.0, 0.6, 0.0),
                vel,
                life,
                size,
                17.5,
                true,
            );
        }

        for _ in 0..6 {
            let dir = self.random_dir();
            let speed = self.rand_range(9.0, 17.0);
            let life = self.rand_range(0.3, 0.7);
            self.spawn(
                ParticleKind::Spark,
                pos + vec3(0.0, 0.7, 0.0),
                dir * speed,
                life,
                0.06,
                9.0,
                false,
            );
        }
    }

    /// Muzzle flash sparks for the turret.
    pub fn muzzle(&mut self, pos: Vec3, dir: Vec3) {
        for _ in 0..3 {
            let jitter = vec3(
                self.rand_range(-2.2, 2.2),
                self.rand_range(-1.0, 2.4),
                self.rand_range(-2.2, 2.2),
            );
            let speed = self.rand_range(16.0, 24.0);
            let life = self.rand_range(0.08, 0.16);
            self.spawn(
                ParticleKind::Spark,
                pos,
                dir * speed + jitter,
                life,
                0.05,
                2.0,
                false,
            );
        }
    }

    /// Bullet impact on flesh/armor: short hot sparks.
    pub fn impact(&mut self, pos: Vec3, incendiary: bool) {
        let n = if incendiary { 5 } else { 4 };
        for _ in 0..n {
            let dir = self.random_dir();
            let speed = self.rand_range(4.0, 9.0);
            let life = self.rand_range(0.16, 0.38);
            let size = self.rand_range(0.06, 0.14);
            self.spawn(
                if incendiary { ParticleKind::Fire } else { ParticleKind::Spark },
                pos,
                dir * speed,
                life,
                size,
                if incendiary { -0.8 } else { 8.0 },
                false,
            );
        }
    }

    /// AP round striking the tank-ish plate of an armored car: bright deflection.
    pub fn ricochet(&mut self, pos: Vec3) {
        for _ in 0..5 {
            let dir = self.random_dir();
            let speed = self.rand_range(8.0, 14.0);
            let life = self.rand_range(0.2, 0.4);
            self.spawn(
                ParticleKind::Spark,
                pos,
                dir * speed + vec3(0.0, 2.0, 0.0),
                life,
                0.05,
                8.0,
                false,
            );
        }
    }

    /// Dust kicked up when a buggy slams back onto the road.
    pub fn dust(&mut self, pos: Vec3) {
        for _ in 0..5 {
            let dir = self.random_dir();
            let spread = self.rand_range(1.5, 3.5);
            let life = self.rand_range(0.4, 0.8);
            let size = self.rand_range(0.28, 0.5);
            self.spawn(
                ParticleKind::Smoke,
                pos,
                vec3(dir.x, 0.35, dir.z) * spread,
                life,
                size,
                -0.4,
                false,
            );
        }
    }

    /// One puff of rocket exhaust. Called every frame while a rocket is alive.
    pub fn rocket_smoke(&mut self, pos: Vec3) {
        let vx = self.rand_range(-1.0, 1.0);
        let vy = self.rand_range(0.4, 1.6);
        let vz = self.rand_range(-1.0, 1.0);
        let life = self.rand_range(0.35, 0.65);
        let size = self.rand_range(0.14, 0.26);
        self.spawn(
            ParticleKind::Smoke,
            pos,
            vec3(vx, vy, vz),
            life,
            size,
            -0.6,
            false,
        );
    }

    /// Bright twinkle when a salvage pickup reaches the rig.
    pub fn pickup_spark(&mut self, pos: Vec3) {
        for _ in 0..4 {
            let dir = self.random_dir();
            let speed = self.rand_range(2.5, 5.5);
            let life = self.rand_range(0.18, 0.34);
            self.spawn(ParticleKind::Spark, pos, dir * speed + vec3(0.0, 2.0, 0.0), life, 0.05, 6.0, false);
        }
    }

    /// Sustained fire effect for burning enemies (call once per frame per burning enemy).
    pub fn burn_touch(&mut self, pos: Vec3) {
        let ox = self.rand_range(-0.4, 0.4);
        let oy = self.rand_range(0.6, 1.6);
        let oz = self.rand_range(-0.4, 0.4);
        let vx = self.rand_range(-0.8, 0.8);
        let vy = self.rand_range(1.5, 3.0);
        let vz = self.rand_range(-0.8, 0.8);
        let life = self.rand_range(0.3, 0.6);
        let size = self.rand_range(0.22, 0.4);
        self.spawn(
            ParticleKind::Fire,
            pos + vec3(ox, oy, oz),
            vec3(vx, vy, vz),
            life,
            size,
            -1.6,
            false,
        );
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.items.iter_mut() {
            if !p.active {
                continue;
            }
            p.life -= dt;
            if p.life <= 0.0 {
                p.active = false;
                continue;
            }
            p.vel.y -= p.gravity * dt; // gravity positive = downward pull
            p.pos += p.vel * dt;

            // ground interaction
            let half = p.size * 0.5;
            if p.pos.y < half {
                p.pos.y = half;
                if p.bounce {
                    p.vel.y = p.vel.y.abs() * 0.45;
                    p.vel.x *= 0.62;
                    p.vel.z *= 0.62;
                } else if p.kind == ParticleKind::Spark {
                    p.life = 0.0;
                    p.active = false;
                } else {
                    p.vel.x *= 0.9;
                    p.vel.z *= 0.9;
                    p.vel.y = 0.0;
                }
            }
        }
    }

    pub fn draw(&self) {
        for p in self.items.iter() {
            if !p.active {
                continue;
            }
            let lf = (p.life / p.max_life).clamp(0.0, 1.0);
            match p.kind {
                ParticleKind::Debris => {
                    let s = p.size * (0.35 + 0.65 * lf);
                    let shade = 0.22 + 0.55 * lf;
                    draw_cube(
                        p.pos,
                        vec3(s, s * 0.8, s),
                        None,
                        Color::new(0.32 * shade + 0.1, 0.28 * shade + 0.08, 0.26 * shade + 0.07, 1.0),
                    );
                }
                ParticleKind::Fire => {
                    let s = p.size * (0.35 + 0.75 * lf);
                    let color = if lf > 0.62 {
                        Color::new(1.0, 0.93, 0.55, 1.0)
                    } else if lf > 0.30 {
                        Color::new(0.96, 0.45 + 0.2 * lf, 0.10, 1.0)
                    } else {
                        Color::new(0.42, 0.06, 0.03, 1.0)
                    };
                    draw_cube(p.pos, vec3(s, s, s), None, color);
                }
                ParticleKind::Smoke => {
                    let s = p.size * (0.7 + (1.0 - lf) * 1.6);
                    let g = 0.055 + 0.075 * lf;
                    draw_cube(p.pos, vec3(s, s * 0.9, s), None, Color::new(g, g, g * 1.05, 1.0));
                }
                ParticleKind::Spark => {
                    let tail = p.pos - p.vel * 0.045;
                    draw_line_3d(tail, p.pos, Color::new(1.0, 0.82 + 0.18 * lf, 0.35 + 0.3 * lf, 1.0));
                }
            }
        }
    }
}

/// Short-lived hitscan tracer lines (biker SMG fire, railgun-like beams later).
#[derive(Clone, Copy)]
struct Tracer {
    a: Vec3,
    b: Vec3,
    ttl: f32,
    max_ttl: f32,
    color: Color,
    active: bool,
}

impl Tracer {
    const INACTIVE: Tracer = Tracer {
        a: Vec3::ZERO,
        b: Vec3::ZERO,
        ttl: 0.0,
        max_ttl: 1.0,
        color: WHITE,
        active: false,
    };
}

const TRACER_CAPACITY: usize = 48;

pub struct TracerPool {
    items: Vec<Tracer>,
}

impl TracerPool {
    pub fn new() -> TracerPool {
        TracerPool {
            items: vec![Tracer::INACTIVE; TRACER_CAPACITY],
        }
    }

    pub fn shoot(&mut self, from: Vec3, to: Vec3, color: Color, ttl: f32) {
        for t in self.items.iter_mut() {
            if !t.active {
                *t = Tracer {
                    a: from,
                    b: to,
                    ttl,
                    max_ttl: ttl.max(0.001),
                    color,
                    active: true,
                };
                return;
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        for t in self.items.iter_mut() {
            if !t.active {
                continue;
            }
            t.ttl -= dt;
            if t.ttl <= 0.0 {
                t.active = false;
            }
        }
    }

    pub fn draw(&self) {
        for t in self.items.iter() {
            if !t.active {
                continue;
            }
            let k = (t.ttl / t.max_ttl).clamp(0.0, 1.0);
            let dim = k * k;
            draw_line_3d(
                t.a,
                t.b,
                Color::new(t.color.r * dim, t.color.g * dim, t.color.b * dim, 1.0),
            );
        }
    }
}
