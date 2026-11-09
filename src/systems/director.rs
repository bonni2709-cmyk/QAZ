//! Wave director: difficulty curve, spawn pacing, ambush patterns.
//!
//! Difficulty scales with distance travelled. Spawns come in ahead of the rig
//! (usually on or near the asphalt) with an occasional biker rear-ambush.
//! Combat is intentionally NOT deterministic; the world layout is.

use macroquad::prelude::*;

use crate::entities::enemies::{EnemyKind, EnemyPool};
use crate::systems::loot::{LootKind, LootPool};
use crate::world::road::ROAD_HALF;

const GRACE_PERIOD_S: f32 = 6.0;

pub struct Director {
    rng: u64,
    spawn_timer: f32,
    loot_timer: f32,
    grace: f32,
}

impl Director {
    pub fn new() -> Director {
        // seed from a clock-ish value so runs differ but pools stay warm
        let seed = (macroquad::time::get_time() * 1_000_000.0) as u64 ^ 0xDEAD_BEEF_CAFE_1234;
        Director {
            rng: seed | 1,
            spawn_timer: 1.5,
            loot_timer: 5.0,
            grace: GRACE_PERIOD_S,
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

    /// Difficulty scalar: 0.9 at spawn-out, ~2.8 at 500+ m deep into a run.
    fn difficulty(distance_m: f32) -> f32 {
        (0.9 + distance_m / 700.0).clamp(0.9, 2.8)
    }

    fn concurrency_cap(distance_m: f32) -> usize {
        (2.0 + distance_m / 320.0).min(7.0) as usize
    }

    pub fn update(
        &mut self,
        dt: f32,
        distance_m: f32,
        truck_z: f32,
        enemies: &mut EnemyPool,
        loot: &mut LootPool,
    ) {
        if self.grace > 0.0 {
            self.grace -= dt;
            // still seed loot during the grace period so the opener feels generous
            self.update_loot(dt, truck_z, loot);
            return;
        }
        self.update_loot(dt, truck_z, loot);

        let difficulty = Self::difficulty(distance_m);
        let cap = Self::concurrency_cap(distance_m);
        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 || enemies.active_count() >= cap {
            return;
        }
        self.spawn_timer = self.rand_range(2.0, 4.3) / difficulty;

        let roll = self.rand01();
        let kind = if distance_m < 250.0 {
            if roll < 0.68 {
                EnemyKind::Biker
            } else {
                EnemyKind::Buggy
            }
        } else if roll < 0.45 {
            EnemyKind::Biker
        } else if roll < 0.74 {
            EnemyKind::Buggy
        } else {
            EnemyKind::Armored
        };

        // bikers occasionally ambush from behind instead of chasing from ahead
        let rear_ambush = kind == EnemyKind::Biker && self.rand01() < 0.22;
        let z = if rear_ambush {
            truck_z - self.rand_range(45.0, 60.0)
        } else {
            truck_z + self.rand_range(95.0, 150.0)
        };
        let on_road = self.rand01() < 0.62;
        let x = if on_road {
            self.rand_range(-ROAD_HALF * 1.55, ROAD_HALF * 1.55)
        } else {
            let side = if self.rand01() < 0.5 { -1.0 } else { 1.0 };
            side * self.rand_range(ROAD_HALF + 1.5, ROAD_HALF + 9.0)
        };

        enemies.spawn(kind, vec3(x, 0.0, z));
    }

    /// Seed crate clusters on the asphalt/shoulders every 8–13 s.
    fn update_loot(&mut self, dt: f32, truck_z: f32, loot: &mut LootPool) {
        self.loot_timer -= dt;
        if self.loot_timer > 0.0 {
            return;
        }
        self.loot_timer = self.rand_range(8.0, 13.0);
        let z = truck_z + self.rand_range(85.0, 135.0);
        let on_road = self.rand01() < 0.78;
        let x = if on_road {
            self.rand_range(-ROAD_HALF * 1.35, ROAD_HALF * 1.35)
        } else {
            let side = if self.rand01() < 0.5 { -1.0 } else { 1.0 };
            side * self.rand_range(ROAD_HALF + 1.0, ROAD_HALF + 7.0)
        };
        let n = 2 + (self.rand01() * 3.0) as usize; // 2–4 items
        for i in 0..n {
            let roll = self.rand01();
            let kind = if roll < 0.62 {
                LootKind::Scrap
            } else if roll < 0.82 {
                LootKind::Fuel
            } else {
                LootKind::Repair
            };
            let scrap = if kind == LootKind::Scrap {
                6 + (self.rand01() * 8.0) as u32
            } else {
                0
            };
            let ox = if n > 1 { (i as f32 - (n as f32 - 1.0) * 0.5) * 1.4 } else { 0.0 };
            loot.spawn(kind, vec3(x + ox, 0.4, z + self.rand_range(-2.0, 2.0)), scrap);
        }
    }
}
