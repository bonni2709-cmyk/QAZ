//! Driving state — the endless highway run, now with combat.
//!
//! Inputs (desktop): W/↑ gas · S/↓ brake/reverse · A,D/←,→ steer · mouse aims
//! the turret (cursor grabbed) · LMB / Space fire · 1/2/Q ammo · R reset · Esc pause.
//! Inputs (Android): AUTO-GAS, hold screen thirds to steer, auto-aim tracks the
//! nearest hostile, FIRE button bottom-right.
//!
//! The rig wrecks at 0 HULL: the run ends, stats show, R rolls out again.

use macroquad::prelude::*;

use crate::entities::boss::{BossTruck, MinePool};
use crate::entities::drone::BomberDrone;
use crate::entities::worm::CrawlerWorm;
use crate::entities::enemies::{EnemyFrameEvents, EnemyKind, EnemyPool};
use crate::entities::projectiles::{ProjKind, ProjOwner, ProjectilePool};
use crate::entities::truck::Truck;
use crate::systems::chase::ChaseCamera;
use crate::systems::director::Director;
use crate::systems::loot::{LootKind, LootPool};
use crate::systems::particles::{ParticlePool, TracerPool};
use crate::systems::assets::AssetBundle;
use crate::systems::savegame::{self, SaveData, NG_PLUS_MAX};
use crate::states::onfoot::{OnFootOutcome, BAIL_HOLD};
use crate::systems::factions::Faction;
use crate::systems::turret::{AmmoKind, TurretState};
use crate::systems::upgrades::UpgradeState;
use crate::systems::weather::{Weather, WeatherEvent};
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
    assets: AssetBundle,
    director: Director,
    loot: LootPool,
    upgrades: UpgradeState,
    weather: Weather,
    boss: Option<BossTruck>,
    mines: MinePool,
    next_boss_at: f32,
    boss_contact_cd: f32,
    worm: Option<CrawlerWorm>,
    next_worm_at: f32,
    drone: BomberDrone,
    next_drone_at: f32,
    event_banner: f32,
    event_banner_text: &'static str,
    was_overheated: bool,
    prev_wb_flash: f32,
    engine_vol: f32,
    wind_vol: f32,
    save: SaveData,
    save_dirty: bool,
    save_flush_timer: f32,
    abandon_requested: bool,
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
    /// z of the nearest ruin site ahead (always spawns on the shoulder)
    site_z: f32,
    site_seed: u64,
    site_side: f32,
    bail_hold: f32,
    bail_requested: Option<u64>,
    faction: Faction,
    ng_plus: u32,
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
fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// How far off the road axis a ruin site sits (always full-shoulder, never blocking).
const SITE_SIDE_X: f32 = 12.5;

/// Distance (m) at which a site counts as "you are here".
const SITE_REACH: f32 = 26.0;

/// Deterministic roadside marker drawn in the 3D pass. Cheap axis-aligned
/// silhouettes + a pulsing amber beacon column — readable from 250 m out.
fn draw_ruin_marker(x: f32, z: f32, time: f32, ambient: f32) {
    let a = ambient;
    let chunk = |px: f32, py: f32, pz: f32, sx: f32, sy: f32, sz: f32, shade: f32| {
        let c = Color::new(shade * a + 0.02, shade * a * 0.92 + 0.02, shade * a * 0.80 + 0.015, 1.0);
        draw_cube(vec3(px, py, pz), vec3(sx, sy, sz), None, c);
    };
    draw_plane(vec3(x, 0.02, z), vec2(10.0, 10.0), None, Color::new(0.09 * a + 0.02, 0.08 * a + 0.02, 0.07 * a + 0.015, 1.0));
    chunk(x - 1.8, 0.9, z + 0.5, 2.3, 1.8, 1.7, 0.34);
    chunk(x + 2.0, 1.5, z - 1.1, 1.5, 3.0, 1.4, 0.29);
    chunk(x + 0.1, 0.55, z + 2.3, 2.9, 1.1, 1.3, 0.37);
    chunk(x - 0.4, 2.6, z - 0.3, 0.9, 0.8, 0.8, 0.26);
    // hazard beacon — vertical light column so the site reads from far away
    let pulse = 0.5 + 0.5 * (time * 2.1).sin();
    draw_cylinder(
        vec3(x, 0.02, z),
        0.55,
        0.55,
        24.0,
        None,
        Color::new(1.0, 0.62, 0.12, 0.10 + 0.16 * pulse),
    );
    draw_cylinder(
        vec3(x, 0.10, z),
        1.25,
        1.25,
        0.22,
        None,
        Color::new(1.0, 0.55, 0.10, 0.30 + 0.35 * pulse),
    );
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

/// Android BAIL-OUT hold: button only works (and is drawn) near a ruin site.
#[cfg(target_os = "android")]
fn bail_touch_held() -> bool {
    let (cx, cy, r) = crate::ui::bail_button_center();
    touches().iter().any(|t| {
        matches!(t.phase, TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary) && {
            let dx = t.position.x - cx;
            let dy = t.position.y - cy;
            dx * dx + dy * dy <= (r + 10.0) * (r + 10.0)
        }
    })
}

#[cfg(not(target_os = "android"))]
fn bail_touch_held() -> bool {
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
    pub fn new(assets: AssetBundle) -> DrivingState {
        let save = savegame::load();
        let persisted_best = save.best_m;
        let upgrades = UpgradeState::restore(save.scrap, [save.armor, save.turret, save.engine, save.magnet]);
        let mut state = DrivingState {
            truck: Truck::new(),
            camera: ChaseCamera::new(),
            projectiles: ProjectilePool::new(),
            enemies: EnemyPool::new(),
            particles: ParticlePool::new(),
            tracers: TracerPool::new(),
            turret: TurretState::new(),
            assets,
            director: Director::new(),
            loot: LootPool::new(),
            upgrades,
            weather: Weather::new(),
            boss: None,
            mines: MinePool::new(),
            next_boss_at: 1300.0,
            boss_contact_cd: 0.0,
            worm: None,
            next_worm_at: 2050.0,
            drone: BomberDrone::new(),
            next_drone_at: 1150.0,
            site_z: 430.0 + (mix64(save.total_runs as u64 ^ 0xB411_5EED) % 160) as f32,
            site_seed: mix64(0x5EED_B411 ^ (save.total_runs as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
            site_side: if mix64(save.total_runs as u64 ^ 0x5171_E5) & 1 == 0 { -1.0 } else { 1.0 },
            bail_hold: 0.0,
            bail_requested: None,
            faction: Faction::from_biome(biome_at(0.0)),
            ng_plus: save.ng_plus,
            event_banner: 0.0,
            event_banner_text: "",
            was_overheated: false,
            prev_wb_flash: 0.0,
            engine_vol: 0.14,
            wind_vol: 0.0,
            save,
            save_dirty: false,
            save_flush_timer: 0.0,
            abandon_requested: false,
            events: EnemyFrameEvents::new(),
            hit_rng: 0xA5A5_5A5A_3C3C_C3C3,
            time_of_day: 0.34, // mid-morning wasteland light
            distance_m: 0.0,
            best_m: persisted_best,
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
        };
        // NG+ pressure and faction spawn table are not reset between runs —
        // the wasteland remembers what you have survived
        state.director.set_faction(state.faction);
        state.director.set_hp_scale(1.0 + state.ng_plus as f32 * 0.15);
        // loops live until the wreck screen or a run reset
        state.assets.start_loop(&state.assets.music, 0.30);
        state.assets.start_loop(&state.assets.engine, 0.14);
        state.assets.start_loop(&state.assets.wind, 0.0);
        state
    }

    /// Fold the live session into the JSON save and flush it to disk.
    /// `run_finished` bumps the career counters (called on the wreck transition).
    fn persist_save(&mut self, run_finished: bool) {
        let (scrap, ranks) = self.upgrades.snapshot();
        self.save.scrap = scrap;
        self.save.armor = ranks[0];
        self.save.turret = ranks[1];
        self.save.engine = ranks[2];
        self.save.magnet = ranks[3];
        self.save.best_m = self.best_m;
        if run_finished {
            self.save.total_runs = self.save.total_runs.saturating_add(1);
            self.save.total_kills = self.save.total_kills.saturating_add(self.kills as u64);
        }
        self.save_dirty = false;
        self.save_flush_timer = 0.0;
        let _ = savegame::store(&self.save);
    }

    /// Push the ruin-site marker further down the road with a fresh seed.
    fn advance_site(&mut self) {
        self.site_seed = mix64(self.site_seed);
        let h = mix64(self.site_seed ^ 0x5171_7E5E);
        self.site_side = if h & 1 == 0 { -1.0 } else { 1.0 };
        // spacing: 480–820 m, always ahead of wherever the rig currently is
        self.site_z = self.site_z.max(self.truck.position.z) + 480.0 + (mix64(h) % 340) as f32;
        self.bail_hold = 0.0;
    }

    /// NG+ lethality multiplier applied to everything that hurts the rig.
    #[inline]
    fn incoming_scale(&self) -> f32 {
        1.0 + self.ng_plus as f32 * 0.15
    }

    /// True while the rig sits in a ruin site's reach (slow enough to bail).
    fn site_in_reach(&self) -> bool {
        (self.site_z - self.truck.position.z).abs() <= SITE_REACH && self.truck.speed.abs() < 6.5
    }

    /// The app calls this after update(); Some(seed) = swap to the on-foot episode.
    pub fn take_bail_request(&mut self) -> Option<u64> {
        self.bail_requested.take()
    }

    /// Fold an on-foot sortie back into the rig.
    /// Medkits patch +18 HULL each (same as roadside repair pickups); a blackout
    /// already zeroed the bag inside the episode — nothing to hand back but the rig.
    pub fn apply_onfoot_outcome(&mut self, outcome: OnFootOutcome) {
        self.upgrades.add_scrap(outcome.scrap);
        if outcome.medkits > 0 {
            self.truck.health = (self.truck.health + outcome.medkits as f32 * 18.0).min(self.truck.max_health);
        }
        self.event_banner = 3.0;
        self.event_banner_text = if outcome.blacked_out {
            "YOU BLACKED OUT — THE SALVAGE IS GONE"
        } else if outcome.scrap > 0 || outcome.medkits > 0 {
            "SALVAGE SECURED — BACK ON THE ROAD"
        } else {
            "EMPTY HANDED — BUT BREATHING"
        };
        // update() stopped the loops the moment the bail fired — bring them back
        self.assets.start_loop(&self.assets.engine, self.engine_vol);
        self.assets.start_loop(&self.assets.music, 0.30);
        self.assets.start_loop(&self.assets.wind, self.wind_vol);
        self.save_dirty = true;
        self.fade_in = 0.5;
    }

    fn stop_loops(&self) {
        self.assets.stop(&self.assets.engine);
        self.assets.stop(&self.assets.music);
        self.assets.stop(&self.assets.wind);
    }

    /// True when the player asked to bail out to the main menu (M while paused/wrecked).
    pub fn take_menu_request(&mut self) -> bool {
        std::mem::take(&mut self.abandon_requested)
    }

    /// Full run reset (R). Keeps the session best distance.
    fn reset_run(&mut self) {
        self.persist_save(false);
        let save = self.save.clone(); // career counters survive the reset
        let upgrades = self.upgrades.clone();
        let assets = self.assets.clone(); // cheap handle clone
        self.stop_loops();
        *self = DrivingState::new(assets);
        self.save = save; // new() reloads from disk anyway; keep counters identical
        self.best_m = self.save.best_m;
        self.ng_plus = self.save.ng_plus; // survives a failed disk write too
        self.director.set_hp_scale(1.0 + self.ng_plus as f32 * 0.15);
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

    fn sky_color(&self, biome: Biome, storm: f32) -> Color {
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
        let mut c = Color::new(
            base[0] * 0.88 + haze.r * 0.12,
            base[1] * 0.88 + haze.g * 0.12,
            base[2] * 0.88 + haze.b * 0.12,
            1.0,
        );
        // sandstorm swallows the horizon
        if storm > 0.01 {
            let k = storm * 0.62;
            c = Color::new(
                c.r + (0.55 - c.r) * k,
                c.g + (0.39 - c.g) * k,
                c.b + (0.23 - c.b) * k,
                1.0,
            );
        }
        c
    }

    fn on_enemy_killed(&mut self, index: usize) {
        self.assets.explosion(false);
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

    /// The juggernaut blows: cinematic triple explosion, salvage shower, banner.
    fn on_boss_killed(&mut self) {
        self.assets.explosion(true);
        let Some(boss) = self.boss.take() else {
            return;
        };
        let pos = boss.pos;
        self.particles.explosion(pos + vec3(0.0, 1.7, 0.0), 2.5);
        self.particles.explosion(pos + vec3(0.0, 1.0, 2.4), 1.7);
        self.particles.explosion(pos + vec3(0.0, 1.0, -2.6), 1.7);
        self.camera.add_shake(0.9);
        for _ in 0..6 {
            let amount = 8 + (hit_roll(&mut self.hit_rng) * 6.0) as u32;
            self.loot.spawn_scatter(LootKind::Scrap, pos + vec3(0.0, 1.0, 0.0), amount);
        }
        self.loot.spawn(LootKind::Repair, pos + vec3(0.0, 1.2, 0.0), 0);
        self.kills += 1;
        self.kill_timer = 1.0;
        self.event_banner = 3.0;
        self.event_banner_text = "JUGGERNAUT DOWN";
        self.next_boss_at = self.distance_m + 2600.0;
    }

    /// THE CRAWLER ruptures: a wet golden silence, a lot of scrap, one less nightmare.
    fn on_worm_killed(&mut self) {
        self.assets.explosion(false);
        let Some(worm) = self.worm.take() else {
            return;
        };
        let pos = worm.pos();
        self.particles.explosion(pos + vec3(0.0, 0.8, 0.0), 2.4);
        self.particles.explosion(pos + vec3(0.0, 0.5, 2.1), 1.5);
        self.camera.add_shake(0.8);
        for _ in 0..5 {
            let amount = 7 + (hit_roll(&mut self.hit_rng) * 6.0) as u32;
            self.loot.spawn_scatter(LootKind::Scrap, pos + vec3(0.0, 0.6, 0.0), amount);
        }
        self.loot.spawn(LootKind::Fuel, pos + vec3(0.0, 0.9, 0.0), 0);
        self.kills += 1;
        self.kill_timer = 1.0;
        self.event_banner = 3.0;
        self.event_banner_text = "THE CRAWLER BURSTS";
        self.next_worm_at = self.distance_m + 2500.0;
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

        // bail to the menu: M (paused or wrecked), or BACK/ESCAPE while wrecked
        if (self.wrecked && escape_pressed) || (is_key_pressed(KeyCode::M) && (self.wrecked || self.paused)) {
            self.persist_save(false);
            self.stop_loops();
            self.abandon_requested = true;
            return;
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
        self.event_banner = (self.event_banner - dt).max(0.0);
        self.boss_contact_cd = (self.boss_contact_cd - dt).max(0.0);
        self.hit_timer = (self.hit_timer - dt * 2.6).max(0.0);
        self.kill_timer = (self.kill_timer - dt * 1.8).max(0.0);
        self.time_of_day = (self.time_of_day + dt / DAY_LENGTH_S).rem_euclid(1.0);

        // ---------- weather ----------
        match self.weather.update(dt) {
            WeatherEvent::StormInbound => {
                self.event_banner = 2.6;
                self.event_banner_text = "SANDSTORM INBOUND";
            }
            WeatherEvent::StormCleared => {
                self.event_banner = 2.2;
                self.event_banner_text = "THE STORM BREAKS";
            }
            WeatherEvent::None => {}
        }
        let storm = self.weather.intensity();
        if storm > 0.02 {
            // sand drags at the tires; gusts shove the nose around
            self.truck.speed -= self.truck.speed * 0.85 * storm * dt;
            self.truck.apply_wind_gust(self.weather.gust(), dt);
        }

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
        if self.turret.overheated && !self.was_overheated {
            self.assets.play(&self.assets.overheat, 0.70);
        }
        self.was_overheated = self.turret.overheated;

        // ---------- firing ----------
        let fire_held = is_mouse_button_down(MouseButton::Left)
            || is_key_down(KeyCode::Space)
            || android_fire_touch();
        if fire_held && self.turret.try_fire() {
            self.assets.shot(self.turret.ammo == AmmoKind::Incendiary);
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

        // boss rolls in when the odometer crosses the marker
        if self.boss.is_none() && self.distance_m >= self.next_boss_at {
            self.boss = Some(BossTruck::spawn(self.truck.position.z + 150.0));
            self.event_banner = 3.2;
            self.event_banner_text = "JUGGERNAUT TANKER AHEAD";
        }

        // the ground mutters: CRAWLER surfaces on its own odometer
        if self.worm.is_none() && self.distance_m >= self.next_worm_at {
            self.worm = Some(CrawlerWorm::spawn(self.truck.position.z + 165.0));
            self.event_banner = 3.0;
            self.event_banner_text = "SOMETHING BURROWS BENEATH THE ASPHALT";
        }

        // bomber drone: a klaxon, a shadow, three arguments
        if !self.drone.is_active() && self.distance_m >= self.next_drone_at {
            let side = if hit_roll(&mut self.hit_rng) < 0.5 { -1.0 } else { 1.0 };
            self.drone.spawn(self.truck.position.z + 30.0, side);
            self.next_drone_at = self.distance_m + 1900.0;
            self.event_banner = 2.4;
            self.event_banner_text = "BOMBER DRONE OVERHEAD";
            self.assets.play(&self.assets.mine_beep, 0.85);
        }

        let band = biome_at(self.truck.position.z).band_index();
        if band != self.biome_band {
            self.biome_band = band;
            self.banner_timer = 3.2;
            self.faction = Faction::from_biome(biome_at(self.truck.position.z));
            self.director.set_faction(self.faction);
            self.event_banner = 2.4;
            self.event_banner_text = match self.faction {
                Faction::AshCartel => "ASH CARTEL TERRITORY",
                Faction::DustDevils => "DUST DEVILS TERRITORY",
            };
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
                self.truck.apply_damage(shot.damage * self.incoming_scale());
                self.camera.add_shake(0.06);
            }
        }

        // ---------- boss ----------
        if let Some(boss) = &mut self.boss {
            let boss_events = boss.update(dt, self.truck.position, self.truck.speed, &mut self.mines);
            if boss_events.mine_dropped {
                self.camera.add_shake(0.07);
                self.assets.play(&self.assets.mine_beep, 0.40);
            }
            if boss_events.ramming {
                // the tanker is shouldering in — the whole hull shudders with it
                self.camera.add_shake(0.045);
            }
        }
        // escape valve: it wins the footrace or falls absurdly behind → it despawns
        if let Some(boss) = &self.boss {
            let gap = boss.pos.z - self.truck.position.z;
            if gap > 300.0 || gap < -140.0 {
                self.boss = None;
                self.next_boss_at = self.distance_m + 1800.0;
            }
        }
        // 26 tons of fuel don't ask permission
        if let Some(boss) = &self.boss {
            if self.boss_contact_cd <= 0.0
                && boss.horizontal_distance_to(self.truck.position) < 3.6
            {
                self.boss_contact_cd = 1.6;
                self.truck.apply_damage(26.0 * self.incoming_scale());
                self.camera.add_shake(0.5);
                self.assets.play(&self.assets.ram_crash, 0.85);
            }
        }
        // mines tick on their own clock
        let mine_dmg = self.mines.update(dt, self.truck.position, &mut self.particles);
        if mine_dmg > 0.0 {
            self.truck.apply_damage(mine_dmg * self.incoming_scale());
            self.camera.add_shake(0.45);
        }

        // THE CRAWLER: ram forward. Contact is the only attack.
        if let Some(worm) = &mut self.worm {
            let dmg = worm.update(dt, self.truck.position);
            if dmg > 0.0 {
                self.truck.apply_damage(dmg * self.incoming_scale());
                self.camera.add_shake(0.55);
                self.assets.play(&self.assets.ram_crash, 0.75);
            }
        }
        // escape valve: it buries itself again and gives up the chase
        if let Some(worm) = &self.worm {
            let gap = worm.z() - self.truck.position.z;
            if gap > 130.0 || gap < -185.0 {
                self.worm = None;
                self.next_worm_at = self.distance_m + 2400.0;
            }
        }

        // BOMBER DRONE: tick the pass; cook whatever is under it
        let drone_dmg = self.drone.update(dt, self.truck.position, self.incoming_scale(), &mut self.particles);
        if drone_dmg > 0.0 {
            self.truck.apply_damage(drone_dmg);
            self.camera.add_shake(0.4);
            self.assets.play(&self.assets.explosion_small, 0.8);
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
                            self.assets.play(&self.assets.ricochet, 0.65);
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
                    // boss hull test — only if no regular enemy soaked the round
                    if !consumed {
                        if let Some(boss) = &mut self.boss {
                            let center = boss.pos + vec3(0.0, 2.05, 0.0);
                            if segment_hits_sphere(prev, pos, center, BossTruck::RADIUS) {
                                let boss_ammo = match kind {
                                    ProjKind::Ap => AmmoKind::Ap,
                                    _ => AmmoKind::Incendiary,
                                };
                                let incendiary = kind == ProjKind::Incendiary;
                                let dealt = turret_damage(boss_ammo, true);
                                let boss_died = boss.apply_damage(dealt, incendiary);
                                self.particles.impact(pos, incendiary);
                                self.hit_timer = 1.0;
                                if boss_died {
                                    self.on_boss_killed();
                                }
                                consumed = true;
                            }
                        }
                    }
                    // THE CRAWLER: every body segment is a hit sphere
                    if !consumed {
                        if let Some(worm) = &mut self.worm {
                            for (center, radius) in worm.hit_spheres() {
                                if !segment_hits_sphere(prev, pos, center, radius) {
                                    continue;
                                }
                                let worm_ammo = match kind {
                                    ProjKind::Ap => AmmoKind::Ap,
                                    _ => AmmoKind::Incendiary,
                                };
                                let incendiary = kind == ProjKind::Incendiary;
                                let dealt = turret_damage(worm_ammo, true);
                                let worm_died = worm.apply_damage(dealt, incendiary);
                                self.particles.impact(pos, incendiary);
                                self.hit_timer = 1.0;
                                if worm_died {
                                    self.on_worm_killed();
                                }
                                consumed = true;
                                break;
                            }
                        }
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
                        self.truck.apply_damage(total * self.incoming_scale());
                    }
                    self.particles.explosion(pos, 1.15);
                    self.assets.play(&self.assets.explosion_small, 0.70);
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
                    self.truck.apply_damage(9.0 * self.incoming_scale());
                    self.particles.explosion(enemy.pos + vec3(0.0, 0.6, 0.0), 0.7);
                    self.enemies.deactivate(j);
                    self.kills += 1;
                    self.kill_timer = 1.0;
                    self.camera.add_shake(0.26);
                }
                EnemyKind::Buggy => {
                    self.truck.apply_damage(14.0 * self.incoming_scale());
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
            self.save_dirty = true;
            if tally.scrap > 0 {
                self.assets.play(&self.assets.pickup_scrap, 0.50);
            }
            if tally.repair > 0 {
                self.assets.play(&self.assets.pickup_medkit, 0.60);
            }
            if tally.fuel > 0 {
                self.assets.play(&self.assets.pickup_scrap, 0.42);
            }
            let ng_scrap = (tally.scrap as f32 * (1.0 + 0.20 * self.ng_plus as f32)) as u32;
            self.upgrades.add_scrap(ng_scrap);
            if tally.repair > 0 {
                self.truck.health = (self.truck.health + tally.repair as f32 * 18.0).min(self.truck.max_health);
            }
            if tally.fuel > 0 {
                self.truck.fuel = (self.truck.fuel + tally.fuel as f32 * 22.0).min(self.truck.max_fuel);
            }
        }

        // ---------- hull integrity ----------
        if self.truck.health <= 0.0 && !self.wrecked {
            self.wrecked = true;
            // an 800 m+ run feeds the New Game+ ladder — the wasteland escalates
            if self.distance_m >= 800.0 {
                self.save.ng_plus = (self.save.ng_plus + 1).min(NG_PLUS_MAX);
                self.ng_plus = self.save.ng_plus;
                self.event_banner = 3.4;
                self.event_banner_text = "THE WASTELAND ESCALATES — NEW GAME+";
            }
            // the song dies with the rig
            self.assets.explosion(true);
            self.stop_loops();
            self.persist_save(true);
        }

        // ---------- ruin site: hold F to bail out ----------
        {
            let dz = self.site_z - self.truck.position.z;
            if dz < -SITE_REACH {
                // rolled straight past it — the wasteland keeps it for itself
                self.advance_site();
            } else if self.site_in_reach() {
                let bail_held = is_key_down(KeyCode::F) || bail_touch_held();
                if bail_held {
                    self.bail_hold += dt;
                    if self.bail_hold >= BAIL_HOLD {
                        self.stop_loops();
                        self.bail_requested = Some(self.site_seed);
                        self.advance_site(); // consumption: no instant re-bail on return
                        self.bail_hold = 0.0;
                        self.save_dirty = true;
                        return; // freeze this frame — App swaps into the on-foot episode
                    }
                } else {
                    self.bail_hold = (self.bail_hold - dt * 2.0).max(0.0);
                }
            } else {
                self.bail_hold = 0.0;
            }
        }

        // ---------- save flush (debounced: at most one write per 5 s, only when dirty) ----------
        if self.save_dirty {
            self.save_flush_timer += dt;
            if self.save_flush_timer >= 5.0 {
                self.persist_save(false);
            }
        }

        // ---------- effects ----------
        self.particles.update(dt);
        self.tracers.update(dt);

        // ---------- loop levels follow the world ----------
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);
        let engine_target = 0.08 + speed_norm * 0.40;
        if (engine_target - self.engine_vol).abs() > 0.03 {
            self.engine_vol = engine_target;
            self.assets.set_volume(&self.assets.engine, engine_target);
        }
        let wind_target = storm * 0.55;
        if (wind_target - self.wind_vol).abs() > 0.03 {
            self.wind_vol = wind_target;
            self.assets.set_volume(&self.assets.wind, wind_target);
        }

        // ---------- chase camera ----------
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
        let storm = self.weather.intensity();
        let ambient = (self.ambient() * (1.0 - storm * 0.35)).max(0.14);
        let speed_norm = (self.truck.speed / 33.0).clamp(0.0, 1.0);

        // ---------- 3D pass ----------
        clear_background(self.sky_color(biome, storm));
        set_camera(&self.camera.to_camera3d(speed_norm));
        draw_world(
            z,
            ambient,
            self.assets.asphalt.as_ref(),
            self.assets.sand.as_ref(),
            self.assets.concrete.as_ref(),
        );
        let site_dz = self.site_z - z;
        if site_dz.abs() < 260.0 {
            draw_ruin_marker(self.site_side * SITE_SIDE_X, self.site_z, time, ambient);
        }
        self.enemies.draw(ambient);
        if let Some(boss) = &mut self.boss {
            boss.draw(ambient, time);
        }
        if let Some(worm) = &mut self.worm {
            worm.draw(ambient, time);
        }
        self.drone.draw(ambient, time);
        self.mines.draw(time);
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

        // ---------- sandstorm veil + wind streaks (under HUD) ----------
        if storm > 0.02 {
            let sw = screen_width();
            let sh = screen_height();
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.60, 0.42, 0.24, 0.33 * storm));
            let rand = |k: f32| -> f32 {
                let x = (k * 12.9898).sin() * 43758.5453;
                x - x.floor()
            };
            for i in 0..26 {
                let k = i as f32 + 1.0;
                let speed = 340.0 + rand(k) * 260.0;
                let y = rand(k * 1.7 + 3.1) * sh;
                let len = 80.0 + rand(k * 2.3 + 7.7) * 170.0;
                let x = (time * speed + k * 173.0).rem_euclid(sw + len * 2.0) - len;
                let alpha = (0.10 + 0.12 * (rand(k * 3.1) * (time * 2.0 + k).sin() * 0.5 + 0.5)) * storm;
                draw_rectangle(x, y, len, 2.0 + (i % 3) as f32, Color::new(0.85, 0.68, 0.42, alpha));
            }
        }

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
            storm,
            boss_ratio: if let Some(b) = &self.boss {
                b.hp_ratio()
            } else if let Some(w) = &self.worm {
                w.hp_ratio()
            } else {
                -1.0
            },
            boss_name: if self.worm.is_some() {
                CrawlerWorm::NAME
            } else {
                BossTruck::NAME
            },
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

        // persistent faction tag under the horizon line
        {
            let sw2 = screen_width();
            let tag = self.faction.name();
            let d = measure_text(tag, None, 15, 1.0);
            draw_text(tag, (sw2 - d.width) * 0.5, screen_height() * 0.275, 15.0, self.faction.accent());
        }

        if self.event_banner > 0.0 {
            let t = self.event_banner;
            let alpha = (t / 0.5).clamp(0.0, 1.0).min((3.2 - t) / 0.4).clamp(0.0, 1.0).max(0.0);
            let sw = screen_width();
            let dim = measure_text(self.event_banner_text, None, 38, 1.0);
            let x = (sw - dim.width) * 0.5;
            let y = screen_height() * 0.235;
            draw_text(self.event_banner_text, x + 2.0, y + 2.0, 38.0, Color::new(0.0, 0.0, 0.0, alpha * 0.75));
            draw_text(self.event_banner_text, x, y, 38.0, Color::new(1.0, 0.64, 0.2, alpha));
        }

        if self.rocket_warning && !self.wrecked && !self.workbench_open {
            ui::draw_incoming_warning();
        }

        if !self.wrecked && !self.workbench_open && !self.paused {
            let site_dz = self.site_z - z;
            if self.site_in_reach() {
                let sw = screen_width();
                let sh = screen_height();
                #[cfg(target_os = "android")]
                let msg = "RUIN SITE  —  HOLD  [BAIL]  TO STEP OUT";
                #[cfg(not(target_os = "android"))]
                let msg = "RUIN SITE  —  HOLD  [F]  TO BAIL OUT";
                let dim = measure_text(msg, None, 30, 1.0);
                let x = (sw - dim.width) * 0.5;
                let y = sh * 0.70;
                draw_text(msg, x + 2.0, y + 2.0, 30.0, Color::new(0.0, 0.0, 0.0, 0.7));
                draw_text(msg, x, y, 30.0, Color::new(1.0, 0.78, 0.30, 1.0));
                // hold progress
                let pw = sw * 0.24;
                let ph = 10.0;
                let px = (sw - pw) * 0.5;
                let py = y + 12.0;
                draw_rectangle(px - 2.0, py - 2.0, pw + 4.0, ph + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(px, py, pw * (self.bail_hold / BAIL_HOLD).clamp(0.0, 1.0), ph, Color::new(1.0, 0.65, 0.15, 1.0));
                #[cfg(target_os = "android")]
                {
                    let (bx, by, br) = crate::ui::bail_button_center();
                    draw_circle(bx, by, br, Color::new(0.98, 0.62, 0.12, 0.35));
                    draw_circle_lines(bx, by, br, 3.0, Color::new(1.0, 0.72, 0.28, 0.9));
                    let d = measure_text("BAIL", None, 20, 1.0);
                    draw_text("BAIL", bx - d.width * 0.5, by + 7.0, 20.0, Color::new(1.0, 0.88, 0.55, 1.0));
                }
            } else if site_dz > SITE_REACH && site_dz < 150.0 {
                let sw = screen_width();
                let msg = format!("RUIN SITE  {:.0} m", site_dz);
                let d = measure_text(&msg, None, 20, 1.0);
                draw_text(&msg, (sw - d.width) * 0.5, screen_height() * 0.315, 20.0, Color::new(1.0, 0.78, 0.30, 0.80));
            }
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
            if self.wb_flash > 0.0 && self.prev_wb_flash == 0.0 {
                self.assets.play(&self.assets.workbench, 0.60);
                self.persist_save(false);
            }
        }
        self.prev_wb_flash = self.wb_flash;

        if self.wrecked {
            ui::draw_wrecked_overlay(self.distance_m, self.best_m, self.kills, self.upgrades.scrap());
        }
    }
}
