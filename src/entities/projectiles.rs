//! Pooled projectiles: player tungsten AP rounds, incendiary slugs, and
//! enemy rockets — one fixed-capacity pool, discriminated by [`ProjKind`].
//!
//! Bullets fly flat-ish with a whisper of drop; rockets get real drop and ask
//! the particle system for smoke breadcrumbs (driving wires that up).

use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    /// Tungsten AP — flat damage, ignores armor, ricochets off armored cars.
    Ap,
    /// Incendiary — light damage + heavy bonus vs unarmored + burn DoT.
    Incendiary,
    /// Unguided rocket from an armored car — AoE on impact.
    Rocket,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjOwner {
    Player,
    Enemy,
}

impl ProjKind {
    fn tracer_color(self) -> Color {
        match self {
            ProjKind::Ap => Color::new(1.0, 0.92, 0.55, 1.0),
            ProjKind::Incendiary => Color::new(1.0, 0.48, 0.12, 1.0),
            ProjKind::Rocket => Color::new(0.95, 0.75, 0.35, 1.0),
        }
    }

    fn drop_accel(self) -> f32 {
        match self {
            ProjKind::Ap => 1.1,
            ProjKind::Incendiary => 1.6,
            ProjKind::Rocket => 1.4,
        }
    }
}

#[derive(Clone, Copy)]
struct Projectile {
    pos: Vec3,
    prev: Vec3,
    vel: Vec3,
    life: f32,
    damage: f32,
    kind: ProjKind,
    owner: ProjOwner,
    active: bool,
}

impl Projectile {
    const INACTIVE: Projectile = Projectile {
        pos: Vec3::ZERO,
        prev: Vec3::ZERO,
        vel: Vec3::ZERO,
        life: 0.0,
        damage: 0.0,
        kind: ProjKind::Ap,
        owner: ProjOwner::Player,
        active: false,
    };
}

const POOL_CAPACITY: usize = 384;

pub struct ProjectilePool {
    items: Vec<Projectile>,
}

impl ProjectilePool {
    pub fn new() -> ProjectilePool {
        ProjectilePool {
            items: vec![Projectile::INACTIVE; POOL_CAPACITY],
        }
    }

    pub fn spawn(&mut self, pos: Vec3, vel: Vec3, kind: ProjKind, owner: ProjOwner, damage: f32, life: f32) {
        for p in self.items.iter_mut() {
            if !p.active {
                *p = Projectile {
                    pos,
                    prev: pos,
                    vel,
                    life,
                    damage,
                    kind,
                    owner,
                    active: true,
                };
                return;
            }
        }
    }

    pub fn capacity(&self) -> usize {
        self.items.len()
    }

    /// Read-only access by slot (for the collision pass in the driving state).
    pub fn get(&self, index: usize) -> Option<(Vec3, Vec3, ProjKind, ProjOwner, f32)> {
        let p = self.items.get(index)?;
        if !p.active {
            return None;
        }
        Some((p.pos, p.prev, p.kind, p.owner, p.damage))
    }

    pub fn deactivate(&mut self, index: usize) {
        if let Some(p) = self.items.get_mut(index) {
            p.active = false;
        }
    }

    pub fn rocket_smoke_positions(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.items
            .iter()
            .filter(|p| p.active && p.kind == ProjKind::Rocket)
            .map(|p| p.pos)
    }

    /// Integrate the pool. Returns the number of *enemy* rockets still in flight
    /// after the update (used by the HUD INCOMING warning).
    pub fn update(&mut self, dt: f32) -> usize {
        let mut rockets_flying = 0usize;
        for p in self.items.iter_mut() {
            if !p.active {
                continue;
            }
            p.prev = p.pos;
            p.vel.y -= p.kind.drop_accel() * dt;
            p.pos += p.vel * dt;
            p.life -= dt;
            if p.life <= 0.0 {
                p.active = false;
                continue;
            }
            if p.kind == ProjKind::Rocket && p.owner == ProjOwner::Enemy {
                rockets_flying += 1;
            }
        }
        rockets_flying
    }

    pub fn any_enemy_rocket_near(&self, truck_pos: Vec3, radius: f32) -> bool {
        self.items.iter().any(|p| {
            p.active
                && p.kind == ProjKind::Rocket
                && p.owner == ProjOwner::Enemy
                && p.pos.distance_squared(truck_pos) < radius * radius
        })
    }

    pub fn draw(&self) {
        for p in self.items.iter() {
            if !p.active {
                continue;
            }
            let speed = p.vel.length();
            let dir = if speed > 0.001 { p.vel / speed } else { Vec3::Z };
            let streak = match p.kind {
                ProjKind::Ap => 2.0,
                ProjKind::Incendiary => 1.5,
                ProjKind::Rocket => 0.9,
            };
            draw_line_3d(p.pos - dir * streak, p.pos, p.kind.tracer_color());
            if p.kind == ProjKind::Rocket {
                // rocket body: a stubby dark dart
                draw_cube(p.pos, vec3(0.14, 0.14, 0.55), None, Color::new(0.16, 0.16, 0.15, 1.0));
                draw_cube(p.pos - dir * 0.3, vec3(0.09, 0.09, 0.12), None, Color::new(1.0, 0.62, 0.18, 1.0));
            }
        }
    }
}
