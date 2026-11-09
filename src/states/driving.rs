//! Driving state — the endless highway run, now with combat.
//!
//! Inputs (desktop): W/↑ gas · S/↓ brake/reverse · A,D/←,→ steer · mouse aims
//! the turret (cursor grabbed) · LMB / Space fire · 1/2/Q ammo · R reset · Esc pause.
//! Inputs (Android): AUTO-GAS, hold screen thirds to steer, auto-aim tracks the
//! nearest hostile, FIRE button bottom-right.
//!
//! The rig wrecks at 0 HULL: the run ends, stats show, R rolls out again.

use macroquad::prelude::*;

use crate::entities::enemies::{EnemyFrameEvents, EnemyKind, EnemyPool};
use crate::entities::projectiles::{ProjKind, ProjOwner, ProjectilePool};
use crate::entities::truck::Truck;
use crate::systems::chase::ChaseCamera;
use crate::systems::director::Director;
use crate::systems::loot::{LootKind, LootPool};
use crate::systems::particles::{ParticlePool, TracerPool};
use crate::systems::turret::{AmmoKind, TurretState};
use crate::systems::upgrades::UpgradeState;
use crate::ui::{self, HudData};
use crate::world::road::{biome_at, draw_world, haze_color, Biome, ROAD_HALF};

/// Length of one full day/night cycle in seconds.
const DAY_LENGTH_S: f32 = 240.0;

pub struct DrivingState {
    truck: Truck,
    camera: ChaseCamera,
    projectiles: ProjectilePool,
    enemies: EnemyPool,
    particles: ParticlePool,
    tracers: TracerPool,
    turret: TurretState,
    director: Director,
    loot: LootPool,
    upgrades: UpgradeState,
    events: EnemyFrameEvents,
    hit_rng: u64,
    time_of_day: f32,
    distance_m: f32,
    best_m: f32,
    kills: u32,
    biome_band: u32,
    banner_timer: f32,
    paused: bool,
    workbench_open: bool,
    wb_flash: f32,
    wrecked: bool,
    wreck_boom_done: bool,
    fade_in: f32,
    hit_timer: f32,
    kill_timer: f32,
    rocket_warning: bool,
}

#[inline]
fn hit_roll(rng: &mut u64) -> f32 {
    let mut x = *rng;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *rng = x;
    (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f32 / (1u64 << 53) as f32
}

#[inline]
fn segment_hits_sphere(a: Vec3, b: Vec3, center: Vec3, radius: f32) -> bool {
    let ab = b - a;
    let ap = center - a;
    let ab_len2 = ab.length_squared();
    let t = if ab_len2 > 1e-6 {
        (ap.dot(ab) / ab_len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = a + ab * t;
    closest.distance_squared(center) <= radius * radius
}

/// Projectile kind for the current ammo selection.
#[inline]
fn proj_kind(ammo: AmmoKind) -> ProjKind {
    match ammo {
        AmmoKind::Ap => ProjKind::Ap,
        AmmoKind::Incendiary => ProjKind::Incendiary,
    }
}

/// Base damage a player projectile deals to a target with/without armor.
/// `None` = read armor from the enemy at hit time.
#[inline]
fn turret_damage(ammo: AmmoKind, armored: bool) -> f32 {
    ammo.damage(armored)
}

/// Hit Y for each enemy kind (center of mass for the sphere test).
#[inline]
fn enemy_center(kind: EnemyKind, pos: Vec3) -> Vec3 {
    let drop = match kind {
        EnemyKind::Biker => 1.1,
        EnemyKind::Buggy => 0.95,
        EnemyKind::Armored => 1.25,
    };
    pos + vec3(0.0, drop, 0.0)
}

#[cfg(target_os = "android")]
fn android_fire_touch() -> bool {
    let (cx, cy, r) = crate::ui::fire_button_center();
    touches().iter().any(|t| {
        matches!(
            t.phase,
            TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
        ) && {
            let dx = t.position.x - cx;
            let dy = t.position.y - cy;
            dx * dx + dy * dy <= r * r
        }
    })
}

#[cfg(not(target_os = "android"))]
fn android_fire_touch() -> bool {
    false
}

/// Android WORKBENCH toggle button (tap = toggle, not hold).
#[cfg(target_os = "android")]
fn wb_touched() -> bool {
    let (cx, cy, r) = crate::ui::wb_button_center();
    touches().iter().any(|t| {
        matches!(t.phase, TouchPhase::Started) && {
            let dx = t.position.x - cx;
            let dy = t.position.y - cy;
            dx * dx + dy * dy <= (r + 10.0) * (r + 10.0)
        }
    })
}

#[cfg(not(target_os = "android"))]
fn wb_touched() -> bool {
    false
}

impl DrivingState {
    pub fn new() -> DrivingState {
        DrivingState {
            truck: Truck::new(),
            camera: ChaseCamera::new(),
            projectiles: ProjectilePool::new(),
            enemies: EnemyPool::new(),
            particles: ParticlePool::new(),
            tracers: TracerPool::new(),
            turret: TurretState::new(),
            director: Director::new(),
            loot: LootPool::new(),
            upgrades: UpgradeState::new(),
            events: EnemyFrameEvents::new(),
            hit_rng: 0xA5A5_5A5A_3C3C_C3C3,
            time_of_day: 0.34, // mid-morning wasteland light
            distance_m: 0.0,
            best_m: 0.0,
            kills: 0,
            biome_band: biome_at(0.0).band_index(),
            banner_timer: 3.2,
            paused: false,
            workbench_open: false,
            wb_flash: 0.0,
            wrecked: false,
            wreck_boom_done: false,
            fade_in: 1.0,
            hit_timer: 0.0,
            kill_timer: 0.0,
            rocket_warning: false,
        }
    }

    /// Full run reset (R). Keeps the session best distance.
    fn reset_run(&mut self) {
        let best = self.best_m;
        let upgrades = self.upgrades.clone(); // session salvage & workbench ranks persist
        *self = DrivingState::new();
        self.best_m = best;
        self.upgrades = upgrades;
    }

    #[inline]
    fn sun_elevation(&self) -> f32 {
        ((self.time_of_day - 0.25) * std::f32::consts::TAU).sin()
    }

    /// Global ambient multiplier for every lit surface, [0.2, 1.0].
    #[inline]
    fn ambient(&self) -> f32 {
        (0.22 + self.sun_elevation().max(0.0) * 0.82).clamp(0.2, 1.0)
    }

    fn sky_color(&self, biome: Biome) -> Color {
        let elev = self.sun_elevation();
        let day = [0.34, 0.60, 0.82];
        let dusk = [0.88, 0.47, 0.26];
        let night = [0.020, 0.025, 0.055];

        let smoothstep = |a: f32, b: f32, x: f32| -> f32 {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let base = if elev >= 0.0 {
            let k = smoothstep(0.0, 0.6, elev);
            [
                dusk[0] + (day[0] - dusk[0]) * k,
                dusk[1] + (day[1] - dusk[1]) * k,
                dusk[2] + (day[2] - dusk[2]) * k,
            ]
        } else {
            let k = smoothstep(0.0, 0.6, -elev);
            [
                dusk[0] + (night[0] - dusk[0]) * k,
                dusk[1] + (night[1] - dusk[1]) * k,
                dusk[2] + (night[2] - dusk[2]) * k,
            ]
        };

        let haze = haze_color(biome, 1.0);
        Color::new(
            base[0] * 0.88 + haze.r * 0.12,
            base[1] * 0.88 + haze.g * 0.12,
            base[2] * 0.88 + haze.b * 0.12,
            1.0,
        )
    }

    fn on_enemy_killed(&mut self, index: usize) {
        let Some(enemy) = self.enemies.get(index) else {
            return;
        };
        let pos = enemy.pos;
        let kind = enemy.kind;
        let scale = match kind {
            EnemyKind::Biker => 0.75,
            EnemyKind::Buggy => 1.0,
            EnemyKind::Armored => 1.85,
        };
        self.particles.explosion(pos + vec3(0.0, 0.8, 0.0), scale);
        if kind == EnemyKind::Armored {
            self.camera.add_shake(0.30);
        }
        self.enemies.deactivate(index);
        self.kills += 1;
        self.kill_timer = 1.0;

        // salvage spills out of the wreck (bikers drop crumbs, armored cars spill loads)
        let (drops, base) = match kind {
            EnemyKind::Biker => (1, 6),
            EnemyKind::Buggy => (2, 5),
            EnemyKind::Armored => (3, 8),
        };
        for _ in 0..drops {
            let amount = base + (hit_roll(&mut self.hit_rng) * 5.0) as u32;
            self.loot.spawn_scatter(LootKind::Scrap, pos + vec3(0.0, 0.6, 0.0), amount);
        }
        let roll = hit_roll(&mut self.hit_rng);
        match kind {
            EnemyKind::Buggy if roll < 0.25 => {
                self.loot.spawn(LootKind::Fuel, pos + vec3(0.0, 0.8, 0.0), 0);
            }
            EnemyKind::Armored if roll < 0.40 => {
                self.loot.spawn(LootKind::Repair, pos + vec3(0.0, 0.9, 0.0), 0);
            }
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32) {
        let time = get_time() as f32;

        // ---------- global keys ----------
        if (is_key_pressed(KeyCode::Tab) || wb_touched()) && !self.wrecked {
            self.workbench_open = !self.workbench_open;
            self.paused = false;
        }
        #[cfg(target_os = "android")]
        let escape_pressed = is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Back);
        #[cfg(not(target_os = "android"))]
        let escape_pressed = is_key_pressed(KeyCode::Escape);
        if escape_pressed {
            if self.workbench_open {
                self.workbench_open = false;
            } else if !self.wrecked {
                self.paused = !self.paused;
            }
        }

        if is_key_pressed(KeyCode::R) {
            self.reset_run();
            return;
        }

        // cursor grabbing follows pause / wreck / workbench state
        if self.paused || self.wrecked || self.workbench_open {
            set_cursor_grab(false);
            show_mouse(true);
        } else {
            set_cursor_grab(true);
            show_mouse(false);
        }

        if self.paused {
            return;
        }

        // workbench freezes the world (same contract as the pause menu)
        if self.workbench_open {
            self.wb_flash = (self.wb_flash - dt * 1.8).max(0.0);
            return;
        }

        self.fade_in = (self.fade_in - dt * 1.6).max(0.0);
        self.banner_timer = (self.banner_timer - dt).max(0.0);
        self.hit_timer = (self.hit_timer - dt * 2.6).max(0.0);
        self.kill_timer = (self.kill_timer - dt * 1.8).max(0.0);
        self.time_of_day = (self.time_of_day + dt / DAY_LENGTH_S).rem_euclid(1.0);

        // ---------- apply workbench upgrades ----------
        self.truck.damage_reduction = self.upgrades.damage_reduction();
        self.truck.top_speed_mult = self.upgrades.top_speed_mult();
        self.truck.acceleration_mult = self.upgrades.acceleration_mult();
        self.truck.set_hull_bonus(self.upgrades.hull_bonus());
        self.turret.rate_mult = self.upgrades.fire_rate_mult();
        self.turret.cooling_mult = self.upgrades.cooling_mult();
        self.turret.heat_mult = self.upgrades.heat_leak_mult();

        // ---------- wrecked: the world keeps burning, the run is over ----------
        if self.wrecked {
            if !self.wreck_boom_done {
                self.wreck_boom_done = true;
                self.particles.explosion(self.truck.position + vec3(0.0, 1.6, 0.0), 2.6);
                self.particles.explosion(self.truck.position + vec3(0.0, 1.0, 2.2), 1.5);
                self.camera.add_shake(1.25);
            }
            // continuous fireballs on the wreck
            if (time * 17.0).fract() < dt * 17.0 {
                let p = self.truck.position;
                self.particles.burn_touch(p + vec3(0.5, 2.7, -1.1));
                self.particles.burn_touch(p + vec3(-0.6, 2.2, 0.6));
            }
            self.particles.update(dt);
            self.tracers.update(dt);
            self.camera.update(self.truck.position, self.truck.forward(), 0.0, 0.0, dt);
            return;
        }

        // ---------- driving input ----------
        let mut throttle = 0.0f32;
        let mut steer = 0.0f32;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            throttle += 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            throttle -= 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            steer -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            steer += 1.0;
        }
        for touch in touches() {
            if matches!(
                touch.phase,
                TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
            ) {
                let frac = touch.position.x / screen_width();
                if frac < 0.38 {
                    steer -= 1.0;
                } else if frac > 0.62 {
                    steer += 1.0;
                }
            }
        }
        #[cfg(target_os = "android")]
        {
            if throttle == 0.0 {
                throttle = 0.72; // AUTO-GAS
            }
        }
        let steer = steer.clamp(-1.0, 1.0);

        // ---------- turret aiming ----------
        #[cfg(target_os = "android")]
        {
            if let Some(idx) = self.enemies.nearest_to(self.truck.position) {
                if let Some(target) = self.enemies.get(idx) {
                    let dist = target.pos.distance(self.truck.position);
                    let lead = target.velocity() * (dist / 140.0) * 0.62;
                    let aim = target.pos + vec3(0.0, 1.1, 0.0) + lead - self.truck.position;
                    let desired_world = aim.x.atan2(aim.z);
                    self.turret.track(desired_world, self.truck.yaw, dt);
                }
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            // macroquad's mouse_delta_position() is (last_frame - current), i.e. inverted
            let dx = -mouse_delta_position().x;
            self.turret.add_aim(dx);
        }

        if is_key_pressed(KeyCode::Key1) {
            self.turret.set_ammo(AmmoKind::Ap);
        }
        if is_key_pressed(KeyCode::Key2) {
            self.turret.set_ammo(AmmoKind::Incendiary);
        }
        if is_key_pressed(KeyCode::Q) {
            self.turret.cycle_ammo();
        }

        self.truck.set_turret_yaw(self.turret.yaw);
        self.turret.update(dt);

        // ---------- firing ----------
        let fire_held = is_mouse_button_down(MouseButton::Left)
            || is_key_down(KeyCode::Space)
            || android_fire_touch();
        if fire_held && self.turret.try_fire() {
            let (muzzle, dir) = self.truck.turret_muzzle();
            let ammo = self.turret.ammo;
            self.projectiles.spawn(
                muzzle,
                ammo.velocity(dir),
                proj_kind(ammo),
                ProjOwner::Player,
                0.0, // damage resolved at hit (armor-dependent)
                ammo.life(),
            );
            self.particles.muzzle(muzzle, dir);
            self.camera.add_shake(0.038);
        }

        // ---------- physics & damage ----------
        let offroad = self.truck.position.x.abs() > ROAD_HALF + 0.5;
        self.truck.update(dt, throttle, steer, offroad);

        self.distance_m = self.distance_m.max(self.truck.position.z.max(0.0));
        self.best_m = self.best_m.max(self.distance_m);

        let band = biome_at(self.truck.position.z).band_index();
        if band != self.biome_band {
            self.biome_band = band;
            self.banner_timer = 3.2;
        }

        // ---------- hostiles ----------
        self.director.update(dt, self.distance_m, self.truck.position.z, &mut self.enemies, &mut self.loot);
        self.enemies.update(
            dt,
            self.truck.position,
            self.truck.velocity(),
            time,
            &mut self.events,
            &mut self.particles,
        );

        // rockets requested by armored cars
        for rocket in self.events.rockets() {
            self.projectiles.spawn(
                rocket.origin,
                rocket.velocity,
                ProjKind::Rocket,
                ProjOwner::Enemy,
                rocket.damage,
                7.0,
            );
            self.camera.add_shake(0.05);
        }

        // SMG hitscan from bikers
        for shot in self.events.smg_shots() {
            self.tracers.shoot(shot.from, shot.to, Color::new(1.0, 0.88, 0.45, 1.0), 0.10);
            if hit_roll(&mut self.hit_rng) < shot.hit_chance {
                self.truck.apply_damage(shot.damage);
                self.camera.add_shake(0.06);
            }
        }

        // burn-damage kills: sweep any enemy whose hull burned through
        for j in 0..self.enemies.count() {
            let dead = self.enemies.get(j).map(|e| e.hp <= 0.0).unwrap_or(false);
            if dead {
                self.on_enemy_killed(j);
            }
        }

        // ---------- projectiles ----------
        self.projectiles.update(dt);
        self.rocket_warning =
            self.projectiles.any_enemy_rocket_near(self.truck.position, 32.0);

        for i in 0..self.projectiles.capacity() {
            let Some((pos, prev, kind, owner, damage)) = self.projectiles.get(i) else {
                continue;
            };
            match owner {
                ProjOwner::Player => {
                    let mut consumed = false;
                    for j in 0..self.enemies.count() {
                        let Some(enemy) = self.enemies.get(j) else {
                            continue;
                        };
                        let radius = enemy.kind.radius();
                        let center = enemy_center(enemy.kind, enemy.pos);
                        if !segment_hits_sphere(prev, pos, center, radius) {
                            continue;
                        }
                        let armored = enemy.kind.is_armored();
                        let ammo = match kind {
                            ProjKind::Ap => AmmoKind::Ap,
                            _ => AmmoKind::Incendiary,
                        };
                        let mut dealt = turret_damage(ammo, armored);
                        let mut ricocheted = false;
                        if kind == ProjKind::Ap && armored && hit_roll(&mut self.hit_rng) < 0.30 {
                            ricocheted = true;
                            dealt = 12.0;
                        }
                        let died = self.enemies.apply_damage(j, dealt);
                        if kind == ProjKind::Incendiary && !ricocheted {
                            self.enemies.apply_burn(j, 9.0, 3.0);
                        }
                        if ricocheted {
                            self.particles.ricochet(pos);
                        } else {
                            self.particles.impact(pos, kind == ProjKind::Incendiary);
                        }
                        self.hit_timer = 1.0;
                        if died {
                            self.on_enemy_killed(j);
                        }
                        consumed = true;
                        break;
                    }
                    if consumed {
                        self.projectiles.deactivate(i);
                    }
                }
                ProjOwner::Enemy => {
                    if kind != ProjKind::Rocket {
                        continue;
                    }
                    let ground_hit = pos.y <= 0.12;
                    let truck_hit = pos.distance_squared(
                        self.truck.position + vec3(0.0, 1.5, 0.0),
                    ) < 2.7 * 2.7;
                    if !truck_hit && !ground_hit {
                        continue;
                    }
                    let dist = pos.distance(self.truck.position);
                    let direct = if truck_hit { damage } else { 0.0 };
                    let splash = (1.0 - (dist / 7.5).clamp(0.0, 1.0)) * 15.0;
                    let total = (direct + splash).min(38.0);
                    if total > 0.5 {
                        self.truck.apply_damage(total);
                    }
                    self.particles.explosion(pos, 1.15);
                    let near = 1.0 - (dist / 40.0).clamp(0.0, 1.0);
                    self.camera.add_shake(0.34 + 0.28 * near);
                    self.projectiles.deactivate(i);
                }
            }
        }

        // rocket smoke breadcrumbs
        for pos in self.projectiles.rocket_smoke_positions() {
            if hit_roll(&mut self.hit_rng) < 0.55 {
                self.particles.rocket_smoke(pos);
            }
        }

        // ---------- ram collisions (bikers & buggies) ----------
        for j in 0..self.enemies.count() {
            let Some(enemy) = self.enemies.get(j) else {
                continue;
            };
            let kind = enemy.kind;
            if kind == EnemyKind::Armored {
                continue;
            }
            let delta = enemy.pos - self.truck.position;
            let horizontal = vec2(delta.x, delta.z).length();
            if horizontal >= 2.55 || enemy.pos.y >= 1.8 {
                continue;
            }
            match kind {
                EnemyKind::Biker => {
                    self.truck.apply_damage(9.0);
                    self.particles.explosion(enemy.pos + vec3(0.0, 0.6, 0.0), 0.7);
                    self.enemies.deactivate(j);
                    self.kills += 1;
                    self.kill_timer = 1.0;
                    self.camera.add_shake(0.26);
                }
                EnemyKind::Buggy => {
                    self.truck.apply_damage(14.0);
                    let away = vec2(delta.x, delta.z);
                    let away_len = away.length().max(0.001);
                    let away3 = vec3(away.x / away_len, 0.0, away.y / away_len);
                    if let Some(e) = self.enemies.get_mut(j) {
                        e.bounce(away3);
                    }
                    let died = self.enemies.apply_damage(j, 18.0);
                    if died {
                        self.on_enemy_killed(j);
                    }
                    self.camera.add_shake(0.3);
                }
                EnemyKind::Armored => {}
            }
        }

        // ---------- salvage: magnet beam ----------
        let tally = self.loot.update(dt, self.truck.position, self.upgrades.magnet_radius(), &mut self.particles);
        if tally.any() {
            self.upgrades.add_scrap(tally.scrap);
            if tally.repair > 0 {
                self.truck.health = (self.truck.health + tally.repair as f32 * 18.0).min(self.truck.max_health);
            }
            if tally.fuel > 0 {
                self.truck.fuel = (self.truck.fuel + tally.fuel as f32 * 22.0).min(self.truck.max_fuel);
            }
        }

        // ---------- hull integrity ----------
        if self.truck.health <= 0.0 {
            self.wrecked = true;
        }

        // ---------- effects ----------
        self.particles.update(dt);
        self.tracers.update(dt);

        // ---------- chase camera ----------
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);
        self.camera.update(
            self.truck.position,
            self.truck.forward(),
            self.truck.steer,
            speed_norm,
            dt,
        );
    }

    pub fn draw(&mut self) {
        let time = get_time() as f32;
        let z = self.truck.position.z;
        let biome = biome_at(z);
        let ambient = self.ambient();
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);

        // ---------- 3D pass ----------
        clear_background(self.sky_color(biome));
        set_camera(&self.camera.to_camera3d(speed_norm));
        draw_world(z, ambient);
        self.enemies.draw(ambient);
        self.loot.draw(ambient, time);
        // magnet beam: hot filaments from the rig to every attracted pickup
        let truck_top = self.truck.position + vec3(0.0, 2.6, 0.0);
        self.loot.for_each_attracted(|lp| {
            draw_line_3d(truck_top, lp, Color::new(1.0, 0.78, 0.25, 1.0));
        });
        self.projectiles.draw();
        self.truck.draw(ambient);
        self.particles.draw();
        self.tracers.draw();
        set_default_camera();

        // ---------- 2D pass ----------
        ui::draw_hud(&HudData {
            health: self.truck.health,
            max_health: self.truck.max_health,
            fuel: self.truck.fuel,
            max_fuel: self.truck.max_fuel,
            turret_heat: self.turret.heat,
            speed_kmh: self.truck.speed_kmh(),
            distance_m: self.distance_m,
            best_m: self.best_m,
            biome: biome.name(),
            offroad: self.truck.position.x.abs() > ROAD_HALF + 0.5,
            low_fuel: self.truck.fuel / self.truck.max_fuel < 0.15,
            kills: self.kills,
            scrap: self.upgrades.scrap(),
            ammo: self.turret.ammo.label(),
            ammo_short: self.turret.ammo.short(),
            overheated: self.turret.overheated,
        });
        ui::draw_letterbox_and_vignette();

        if self.banner_timer > 0.0 {
            let t = self.banner_timer;
            let alpha = (t / 0.8).min(1.0).min((3.2 - t) / 0.4).clamp(0.0, 1.0);
            ui::draw_biome_banner(biome.name(), alpha);
        }

        if self.rocket_warning && !self.wrecked && !self.workbench_open {
            ui::draw_incoming_warning();
        }

        if !self.paused && !self.wrecked && !self.workbench_open {
            ui::draw_crosshair(self.turret.overheated, self.hit_timer, self.kill_timer);
        }

        if self.fade_in > 0.0 {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(0.0, 0.0, 0.0, self.fade_in * 0.9),
            );
        }

        if self.paused {
            ui::draw_pause_overlay();
        }

        if self.workbench_open {
            ui::draw_workbench(&mut self.upgrades, &mut self.wb_flash);
        }

        if self.wrecked {
            ui::draw_wrecked_overlay(self.distance_m, self.best_m, self.kills, self.upgrades.scrap());
        }
    }
}
