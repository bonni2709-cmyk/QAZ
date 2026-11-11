//! On-foot scavenging (Module 7) — the "BAIL OUT" loop.
//!
//! Wrecked outposts spawn along the highway ("RUIN SITES"). Slow down within
//! ~26 m of one and hold F (Android: hold the BAIL button) to bail out of the
//! rig. On foot your only leverage is two tools — the angle grinder guts
//! crates, the screwdriver cracks the safe (using the wrong one is ~3.2×
//! slower). Marauder skiffs circle the ruin and take potshots; your NERVE is
//! the meter that matters. Anything you grab stays in your bag until you
//! reach the EXFIL ring — blacking out drops the lot.
//!
//! Layout generation is a pure function of the site seed (unit-tested below);
//! per-frame draw goes through the reused mesh kit, zero per-frame allocation.

use macroquad::prelude::*;

use crate::systems::assets::AssetBundle;
use crate::systems::meshkit::PartMesh;
use crate::systems::particles::ParticlePool;

// ---------------- tuning ----------------
const NERVE_MAX: f32 = 100.0;
const TIME_LIMIT: f32 = 75.0;
const PLAYER_SPEED: f32 = 5.4;
const ARENA_RADIUS: f32 = 32.0;
pub const BAIL_HOLD: f32 = 1.2; // hold F at the highway prompt
const EXFIL_RADIUS: f32 = 2.6;
const EXFIL_HOLD: f32 = 0.8;
const GRINDER_CRATE_S: f32 = 2.2;
const SCREWDRIVER_SAFE_S: f32 = 4.5;
const MISMATCH: f32 = 3.2;
const PILE_COUNT: usize = 5;
const CRATE_COUNT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Grinder,
    Screwdriver,
}

fn tool_rate(tool: Tool, kind: InteractKind) -> f32 {
    match (tool, kind) {
        (Tool::Grinder, InteractKind::Crate) => 1.0 / GRINDER_CRATE_S,
        (Tool::Screwdriver, InteractKind::Safe) => 1.0 / SCREWDRIVER_SAFE_S,
        (Tool::Grinder, InteractKind::Safe) => 1.0 / (SCREWDRIVER_SAFE_S * MISMATCH),
        (Tool::Screwdriver, InteractKind::Crate) => 1.0 / (GRINDER_CRATE_S * MISMATCH),
        (_, InteractKind::Pile) => 1.0, // piles are instant
    }
}

// ---------------- deterministic site layout (pure — unit-tested) ----------------

#[derive(Clone, Debug, PartialEq)]
pub struct WallSpec {
    pub offset: Vec3,
    pub yaw: f32,
    pub size: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SiteLayout {
    pub walls: Vec<WallSpec>,
    pub piles: [Vec3; PILE_COUNT],
    pub crates: [Vec3; CRATE_COUNT],
    pub safe: Vec3,
    pub exfil: Vec3,
}

#[inline]
fn wrap_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    x
}

fn site_hash(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

fn site_range(seed: u64, lo: f32, hi: f32) -> f32 {
    lo + (site_hash(seed) % 10_000) as f32 / 10_000.0 * (hi - lo)
}

/// Walls, piles, crates, safe and the exfil ring — all from one seed.
/// Same seed ⇒ byte-identical layout (that is the whole point).
pub fn site_layout(seed: u64) -> SiteLayout {
    let mut walls = Vec::with_capacity(9);
    for i in 0..9u64 {
        let s = site_hash(seed ^ (i + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let angle = site_range(s, 0.0, std::f32::consts::TAU);
        let dist = site_range(s ^ 0x55, 7.0, 24.5);
        let height = site_range(s ^ 0x22, 1.1, 3.6);
        walls.push(WallSpec {
            offset: Vec3::new(angle.cos() * dist, height * 0.5 - 0.18, angle.sin() * dist),
            yaw: angle + site_range(s ^ 0x44, -0.45, 0.45),
            size: Vec3::new(site_range(s ^ 0x11, 1.3, 4.8), height, site_range(s ^ 0x33, 0.55, 1.15)),
        });
    }
    let point = |tag: u64, min_d: f32, max_d: f32| -> Vec3 {
        let s = site_hash(seed ^ tag);
        let a = site_range(s, 0.0, std::f32::consts::TAU);
        let d = site_range(s ^ 0x77, min_d, max_d);
        Vec3::new(a.cos() * d, 0.0, a.sin() * d)
    };
    SiteLayout {
        walls,
        piles: [
            point(0xA1, 5.0, 27.0),
            point(0xA2, 5.0, 27.0),
            point(0xA3, 5.0, 27.0),
            point(0xA4, 5.0, 27.0),
            point(0xA5, 5.0, 27.0),
        ],
        crates: [
            point(0xC1, 4.0, 22.0),
            point(0xC2, 4.0, 22.0),
            point(0xC3, 4.0, 22.0),
            point(0xC4, 4.0, 22.0),
        ],
        safe: point(0x5A_FE, 5.0, 11.0),
        exfil: Vec3::new(0.0, 0.0, -ARENA_RADIUS + 3.5),
    }
}

// ---------------- interactables ----------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InteractKind {
    Pile,
    Crate,
    Safe,
}

#[derive(Clone, Copy)]
struct Interactable {
    pos: Vec3,
    kind: InteractKind,
    progress: f32,
    done: bool,
    scrap: u32,
}


#[derive(Clone, Copy)]
struct SkiffShot {
    pos: Vec3,
    vel: Vec3,
    life: f32,
}

struct Skiff {
    angle: f32,
    radius: f32,
    height: f32,
    cooldown: f32,
    telegraph: f32,
}

impl Skiff {
    fn position(&self) -> Vec3 {
        Vec3::new(
            self.angle.cos() * self.radius,
            self.height + (self.angle * 2.3).sin() * 0.9,
            self.angle.sin() * self.radius,
        )
    }
}

/// What happened when the on-foot episode ended.
#[derive(Clone, Copy)]
pub struct OnFootOutcome {
    pub scrap: u32,
    pub medkits: u32,
    pub blacked_out: bool,
}

pub struct OnFoot {
    layout: SiteLayout,
    walls: Vec<(Mat4, PartMesh)>,
    crate_meshes: Vec<PartMesh>,
    pile_meshes: Vec<PartMesh>,
    safe_body: PartMesh,
    safe_glow: PartMesh,
    exfil_ring: PartMesh,
    exfil_beam: PartMesh,
    skiff_body: PartMesh,
    skiff_eye: PartMesh,
    leg_l: PartMesh,
    leg_r: PartMesh,
    torso: PartMesh,
    head: PartMesh,
    arm: PartMesh,

    player: Vec3,
    player_yaw: f32,
    camera_yaw: f32,
    walk_phase: f32,
    move_speed: f32,

    nerve: f32,
    timer: f32,
    bag_scrap: u32,
    bag_medkits: u32,
    tool: Tool,
    interact: Vec<Interactable>,

    skiffs: [Skiff; 3],
    shots: [Option<SkiffShot>; 8],
    rng: u64,

    exfil_hold: f32,
    work_snd_cd: f32,
    spark_cd: f32,
    shake: f32,
    elapsed: f32,

    outcome: Option<OnFootOutcome>,
    particles: ParticlePool,
}

impl OnFoot {
    pub fn new(seed: u64) -> OnFoot {
        let layout = site_layout(seed);

        // static wall meshes: geometry centred on the wall's pivot, world baked once
        let walls = layout
            .walls
            .iter()
            .map(|w| {
                let mesh = PartMesh::cuboid_at(w.size, Color::new(0.40, 0.37, 0.32, 1.0), Vec3::ZERO);
                let center = Vec3::new(w.offset.x, w.size.y * 0.5 - 0.18, w.offset.z);
                let world = Mat4::from_translation(center) * Mat4::from_rotation_y(w.yaw);
                (world, mesh)
            })
            .collect();

        // interactables, seeded per crate/pile
        let mut interact = Vec::with_capacity(PILE_COUNT + CRATE_COUNT + 1);
        for (i, p) in layout.piles.iter().enumerate() {
            let s = site_hash(seed ^ (0x51 + i as u64));
            interact.push(Interactable {
                pos: *p,
                kind: InteractKind::Pile,
                progress: 0.0,
                done: false,
                scrap: 5 + (s % 5) as u32,
            });
        }
        for (i, c) in layout.crates.iter().enumerate() {
            let s = site_hash(seed ^ (0xC7 + i as u64));
            interact.push(Interactable {
                pos: *c,
                kind: InteractKind::Crate,
                progress: 0.0,
                done: false,
                scrap: 10 + (s % 9) as u32,
            });
        }
        interact.push(Interactable {
            pos: layout.safe,
            kind: InteractKind::Safe,
            progress: 0.0,
            done: false,
            scrap: 46 + (site_hash(seed ^ 0x5AF1) % 25) as u32,
        });

        let pile_meshes = layout
            .piles
            .iter()
            .map(|p| PartMesh::cuboid_at(Vec3::new(0.75, 0.4, 0.75), Color::new(0.55, 0.45, 0.22, 1.0), *p + Vec3::new(0.0, 0.18, 0.0)))
            .collect();
        let crate_meshes = layout
            .crates
            .iter()
            .map(|c| PartMesh::cuboid_at(Vec3::new(0.9, 0.75, 0.9), Color::new(0.46, 0.38, 0.17, 1.0), *c + Vec3::new(0.0, 0.38, 0.0)))
            .collect();

        let mut skiff_phase = site_hash(seed ^ 0x5CA1);
        let mut next_phase = || {
            skiff_phase = site_hash(skiff_phase);
            skiff_phase
        };
        let skiffs = [0, 1, 2].map(|i| {
            let p = next_phase();
            Skiff {
                angle: i as f32 * 2.1 + (p % 100) as f32 * 0.02,
                radius: 24.0 + (site_hash(p) % 80) as f32 * 0.08,
                height: 7.5 + (site_hash(p ^ 0x9) % 30) as f32 * 0.09,
                cooldown: 3.0 + i as f32 * 1.7,
                telegraph: 0.0,
            }
        });

        OnFoot {
            layout,
            walls,
            crate_meshes,
            pile_meshes,
            safe_body: PartMesh::cuboid_at(Vec3::new(1.15, 0.95, 0.9), Color::new(0.23, 0.25, 0.30, 1.0), Vec3::new(0.0, 0.48, 0.0)),
            safe_glow: PartMesh::cuboid_glow_at(Vec3::new(1.25, 0.16, 1.0), Color::new(1.0, 0.62, 0.15, 1.0), Vec3::new(0.0, 0.98, 0.0)),
            exfil_ring: PartMesh::cylinder_at(2.6, 2.6, 0.10, 14, Color::new(0.15, 0.75, 0.70, 1.0), Vec3::new(0.0, 0.05, 0.0)),
            exfil_beam: PartMesh::cuboid_glow_at(Vec3::new(0.5, 9.0, 0.5), Color::new(0.2, 0.95, 0.85, 1.0), Vec3::new(0.0, 4.5, 0.0)),
            skiff_body: PartMesh::cuboid_at(Vec3::new(1.15, 0.38, 1.6), Color::new(0.21, 0.20, 0.23, 1.0), Vec3::new(0.0, 0.0, 0.0)),
            skiff_eye: PartMesh::cuboid_glow_at(Vec3::new(0.5, 0.14, 0.2), Color::new(1.0, 0.15, 0.10, 1.0), Vec3::new(0.0, -0.05, 0.78)),
            leg_l: PartMesh::cuboid_at(Vec3::new(0.20, 0.80, 0.20), Color::new(0.28, 0.26, 0.22, 1.0), Vec3::new(0.0, -0.40, 0.0)),
            leg_r: PartMesh::cuboid_at(Vec3::new(0.20, 0.80, 0.20), Color::new(0.28, 0.26, 0.22, 1.0), Vec3::new(0.0, -0.40, 0.0)),
            torso: PartMesh::cuboid_at(Vec3::new(0.58, 0.74, 0.34), Color::new(0.36, 0.33, 0.26, 1.0), Vec3::new(0.0, 0.38, 0.0)),
            head: PartMesh::cuboid_at(Vec3::new(0.32, 0.32, 0.32), Color::new(0.62, 0.48, 0.34, 1.0), Vec3::new(0.0, 0.94, 0.0)),
            arm: PartMesh::cuboid_at(Vec3::new(0.15, 0.58, 0.15), Color::new(0.30, 0.27, 0.21, 1.0), Vec3::new(0.0, -0.29, 0.0)),
            player: Vec3::new(0.0, 0.0, -ARENA_RADIUS + 3.5),
            player_yaw: 0.0,
            camera_yaw: 0.0,
            walk_phase: 0.0,
            move_speed: 0.0,
            nerve: NERVE_MAX,
            timer: TIME_LIMIT,
            bag_scrap: 0,
            bag_medkits: 0,
            tool: Tool::Grinder,
            interact,
            skiffs,
            shots: [None; 8],
            rng: site_hash(seed ^ 0xBADC_0FFE) | 1,
            exfil_hold: 0.0,
            work_snd_cd: 0.0,
            spark_cd: 0.0,
            shake: 0.0,
            elapsed: 0.0,
            outcome: None,
            particles: ParticlePool::new(),
        }
    }

    fn xorshift(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    fn exfil_pos(&self) -> Vec3 {
        self.layout.exfil
    }

    /// Distance from the player to the exfil ring (horizontal).
    fn exfil_distance(&self) -> f32 {
        let d = self.player - self.exfil_pos();
        vec2(d.x, d.z).length()
    }

    fn safe_pos(&self) -> Vec3 {
        self.layout.safe
    }

    /// Index of the closest unfinished interactable in reach, if any.
    fn nearest_interactable(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut best_d = f32::MAX;
        for (i, it) in self.interact.iter().enumerate() {
            if it.done {
                continue;
            }
            let d = vec2(it.pos.x - self.player.x, it.pos.z - self.player.z).length();
            let reach = match it.kind {
                InteractKind::Pile => 1.5,
                _ => 1.9,
            };
            if d < reach && d < best_d {
                best = Some(i);
                best_d = d;
            }
        }
        best
    }

    pub fn take_outcome(&mut self) -> Option<OnFootOutcome> {
        self.outcome.take()
    }

    fn finish_extracted(&mut self) {
        self.outcome = Some(OnFootOutcome {
            scrap: self.bag_scrap,
            medkits: self.bag_medkits,
            blacked_out: false,
        });
    }

    fn spawn_shot(&mut self, from: Vec3, vel: Vec3) {
        for slot in self.shots.iter_mut() {
            if slot.is_none() {
                *slot = Some(SkiffShot {
                    pos: from,
                    vel,
                    life: 3.6,
                });
                return;
            }
        }
    }

    pub fn update(&mut self, dt: f32, assets: &AssetBundle) {
        self.elapsed += dt;
        self.shake = (self.shake - dt * 1.6).max(0.0);
        self.work_snd_cd = (self.work_snd_cd - dt).max(0.0);
        self.spark_cd = (self.spark_cd - dt).max(0.0);

        // ---------- run clock ----------
        self.timer -= dt;
        if self.timer <= 0.0 {
            // the storm of the wasteland does not wait
            self.finish_extracted();
            return;
        }

        // ---------- camera orbit ----------
        let mouse = mouse_delta_position();
        self.camera_yaw += mouse.x * 0.0045;

        // ---------- movement (camera space) ----------
        let mut ix = 0.0f32;
        let mut iz = 0.0f32;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            iz += 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            iz -= 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            ix -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            ix += 1.0;
        }

        #[cfg(target_os = "android")]
        {
            // auto-walk; steer with screen thirds (mirrors the truck HUD)
            iz += 0.85;
            let sw = screen_width();
            for touch in touches() {
                if touch.position.x < sw * 0.34 {
                    ix -= 1.0;
                } else if touch.position.x > sw * 0.66 {
                    ix += 1.0;
                }
            }
        }

        let fwd = Vec3::new(self.camera_yaw.sin(), 0.0, self.camera_yaw.cos());
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        let mut move_dir = fwd * iz + right * ix;
        let len = move_dir.length();
        self.move_speed = 0.0;
        if len > 0.01 {
            move_dir /= len;
            self.move_speed = PLAYER_SPEED;
            self.player += move_dir * (PLAYER_SPEED * dt);
            let heading = move_dir.x.atan2(move_dir.z);
            let delta = wrap_angle(heading - self.player_yaw);
            self.player_yaw += delta * (dt * 10.0).min(1.0);
            self.walk_phase += dt * 9.5;
        }

        // arena boundary
        let flat = vec2(self.player.x, self.player.z);
        let flat_len = flat.length();
        if flat_len > ARENA_RADIUS {
            let k = ARENA_RADIUS / flat_len;
            self.player.x *= k;
            self.player.z *= k;
        }

        // soft collision with the safe so you do not clip through it
        let safe_flat = vec2(self.player.x - self.safe_pos().x, self.player.z - self.safe_pos().z);
        if safe_flat.length() < 1.15 {
            let n = safe_flat / safe_flat.length().max(0.001);
            self.player.x = self.safe_pos().x + n.x * 1.15;
            self.player.z = self.safe_pos().z + n.y * 1.15;
        }

        // ---------- tools ----------
        if is_key_pressed(KeyCode::Key1) {
            self.tool = Tool::Grinder;
        }
        if is_key_pressed(KeyCode::Key2) {
            self.tool = Tool::Screwdriver;
        }

        #[cfg(target_os = "android")]
        let e_pressed = is_key_pressed(KeyCode::E) || touches().iter().any(|t| t.phase == TouchPhase::Started);
        #[cfg(not(target_os = "android"))]
        let e_pressed = is_key_pressed(KeyCode::E);
        #[cfg(target_os = "android")]
        let android_work = touches().iter().any(|t| {
            matches!(t.phase, TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary)
                && t.position.x > screen_width() * 0.62
                && t.position.y > screen_height() * 0.55
        });
        #[cfg(target_os = "android")]
        let work_held = is_mouse_button_down(MouseButton::Left) || is_key_down(KeyCode::Space) || android_work;
        #[cfg(not(target_os = "android"))]
        let work_held = is_mouse_button_down(MouseButton::Left) || is_key_down(KeyCode::Space);

        // ---------- interactions ----------
        let nearest = self.nearest_interactable();
        if let Some(i) = nearest {
            let kind = self.interact[i].kind;
            match kind {
                InteractKind::Pile => {
                    if e_pressed {
                        let got = self.interact[i].scrap;
                        self.interact[i].done = true;
                        self.bag_scrap += got;
                        assets.play(&assets.pickup_scrap, 0.55);
                        self.particles.ricochet(self.interact[i].pos + vec3(0.0, 0.35, 0.0));
                    }
                }
                InteractKind::Crate | InteractKind::Safe => {
                    if work_held {
                        let rate = tool_rate(self.tool, kind);
                        self.interact[i].progress += dt * rate;
                        // sparks + tool noise while the blades are spinning
                        if self.spark_cd <= 0.0 {
                            self.spark_cd = 0.06;
                            self.particles.impact(self.interact[i].pos + vec3(0.0, 0.55, 0.0), false);
                        }
                        if self.work_snd_cd <= 0.0 {
                            self.work_snd_cd = 0.42;
                            match self.tool {
                                Tool::Grinder => assets.play(&assets.grinder, 0.5),
                                Tool::Screwdriver => assets.play(&assets.pry, 0.5),
                            }
                        }
                        if self.interact[i].progress >= 1.0 {
                            self.interact[i].done = true;
                            self.bag_scrap += self.interact[i].scrap;
                            match kind {
                                InteractKind::Safe => {
                                    self.bag_medkits += 1;
                                    assets.explosion(false);
                                    assets.play(&assets.pickup_medkit, 0.6);
                                }
                                _ => {
                                    assets.explosion(false);
                                }
                            }
                            self.particles.explosion(self.interact[i].pos + vec3(0.0, 0.7, 0.0), 0.8);
                        }
                    }
                }
            }
        }

        // ---------- exfil ----------
        if self.exfil_distance() < EXFIL_RADIUS {
            if is_key_down(KeyCode::E) || work_held {
                self.exfil_hold += dt;
                if self.exfil_hold >= EXFIL_HOLD {
                    self.finish_extracted();
                    return;
                }
            } else {
                self.exfil_hold = (self.exfil_hold - dt * 2.0).max(0.0);
            }
        } else {
            self.exfil_hold = 0.0;
        }

        // ---------- marauder skiffs ----------
        let eye = vec3(0.0, 1.0, 0.0);
        for skiff in self.skiffs.iter_mut() {
            skiff.angle += 0.185 * dt;
            skiff.cooldown -= dt;
            if skiff.telegraph > 0.0 {
                skiff.telegraph -= dt;
                if skiff.telegraph <= 0.0 {
                    let pos = skiff.position();
                    let target = self.player + eye;
                    let dir = target - pos;
                    let len = dir.length();
                    if len > 0.1 {
                        let jitter = 0.94 + (self.xorshift() % 100) as f32 * 0.0012;
                        self.spawn_shot(pos, (dir / len) * (23.0 * jitter));
                        assets.play(&assets.shot_inc, 0.35);
                    }
                    skiff.cooldown = 5.0 + (self.xorshift() % 1000) as f32 * 0.004;
                }
            } else if skiff.cooldown <= 0.0 {
                skiff.telegraph = 0.85;
            }
        }

        // ---------- skiff shots ----------
        for slot in self.shots.iter_mut() {
            let Some(shot) = slot else {
                continue;
            };
            shot.pos += shot.vel * dt;
            shot.life -= dt;
            let dist = (shot.pos - (self.player + eye)).length();
            let ground = shot.pos.y <= 0.12;
            if dist < 1.15 {
                self.nerve -= 13.0;
                self.shake = (self.shake + 0.55).min(0.9);
                assets.play(&assets.explosion_small, 0.6);
                self.particles.explosion(shot.pos, 0.45);
                *slot = None;
                continue;
            }
            if ground || shot.life <= 0.0 {
                self.particles.impact(shot.pos, false);
                *slot = None;
            }
        }

        // ---------- blackout ----------
        if self.nerve <= 0.0 {
            self.nerve = 0.0;
            assets.play(&assets.ram_crash, 0.8);
            self.outcome = Some(OnFootOutcome {
                scrap: 0,
                medkits: 0,
                blacked_out: true,
            });
        }

        self.particles.update(dt);
    }

    fn is_working_on_something(&self) -> bool {
        #[cfg(target_os = "android")]
        let work_held = is_mouse_button_down(MouseButton::Left)
            || is_key_down(KeyCode::Space)
            || touches().iter().any(|t| {
                matches!(t.phase, TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary)
                    && t.position.x > screen_width() * 0.62
                    && t.position.y > screen_height() * 0.55
            });
        #[cfg(not(target_os = "android"))]
        let work_held = is_mouse_button_down(MouseButton::Left) || is_key_down(KeyCode::Space);
        if !work_held {
            return false;
        }
        matches!(
            self.nearest_interactable(),
            Some(i) if self.interact[i].kind != InteractKind::Pile
        )
    }

    fn is_telegraphing(&self) -> bool {
        self.skiffs.iter().any(|s| s.telegraph > 0.0)
    }

    pub fn draw(&mut self, ambient: f32, assets: &AssetBundle) {
        let a = ambient.clamp(0.0, 1.0);
        clear_background(Color::new(0.20 + 0.35 * a, 0.14 + 0.26 * a, 0.08 + 0.18 * a, 1.0));

        let eye = self.player + vec3(0.0, 1.15, 0.0);
        let cam_fwd = vec3(self.camera_yaw.sin(), 0.0, self.camera_yaw.cos());
        let mut cam_pos = self.player - cam_fwd * 5.3 + vec3(0.0, 2.75, 0.0);
        cam_pos.y = cam_pos.y.max(1.4);
        if self.shake > 0.01 {
            let t = self.elapsed * 31.0;
            cam_pos += vec3((t * 1.3).sin() * self.shake * 0.16, (t * 1.9).cos() * self.shake * 0.12, 0.0);
        }
        set_camera(&Camera3D {
            position: cam_pos,
            target: eye,
            up: vec3(0.0, 1.0, 0.0),
            fovy: 62.0_f32.to_radians(),
            z_near: 0.3,
            z_far: 240.0,
            ..Default::default()
        });

        // ---------- ground (sand) ----------
        match assets.sand.as_ref() {
            Some(tex) => draw_plane(
                vec3(0.0, 0.0, 0.0),
                vec2(90.0, 90.0),
                Some(tex),
                Color::new(a, a, a, 1.0),
            ),
            None => draw_plane(
                vec3(0.0, 0.0, 0.0),
                vec2(90.0, 90.0),
                None,
                Color::new(0.46 * a + 0.05, 0.35 * a + 0.04, 0.20 * a + 0.03, 1.0),
            ),
        }

        // ---------- ruins (static, world baked at build) ----------
        for (world, mesh) in self.walls.iter_mut() {
            mesh.write_transformed(world, a);
            mesh.draw();
        }

        // ---------- piles (0..PILE_COUNT of interact) ----------
        for (i, pile) in self.pile_meshes.iter_mut().enumerate() {
            if self.interact[i].done {
                continue;
            }
            pile.write_transformed(&Mat4::IDENTITY, a);
            pile.draw();
        }

        // ---------- crates (PILE_COUNT..PILE_COUNT+CRATE_COUNT) ----------
        for (i, crate_mesh) in self.crate_meshes.iter_mut().enumerate() {
            if self.interact[PILE_COUNT + i].done {
                continue;
            }
            crate_mesh.write_transformed(&Mat4::IDENTITY, a);
            crate_mesh.draw();
        }

        // ---------- safe (body + pulsing glow strip) ----------
        let safe_i = PILE_COUNT + CRATE_COUNT;
        if !self.interact[safe_i].done {
            let safe_world = Mat4::from_translation(self.safe_pos());
            self.safe_body.write_transformed(&safe_world, a);
            self.safe_body.draw();
            let pulse = 1.0 + 0.16 * (self.elapsed * 4.2).sin();
            let glow_world = Mat4::from_translation(self.safe_pos()) * Mat4::from_scale(Vec3::splat(pulse));
            self.safe_glow.write_transformed(&glow_world, a);
            self.safe_glow.draw();
        }

        // ---------- exfil ring + beam ----------
        let exfil = self.exfil_pos();
        let ring_world = Mat4::from_translation(exfil);
        self.exfil_ring.write_transformed(&ring_world, a);
        self.exfil_ring.draw();
        let beam_pulse = 0.75 + 0.25 * (self.elapsed * 2.4).sin();
        let beam_world = Mat4::from_translation(exfil) * Mat4::from_scale(Vec3::new(beam_pulse, 1.0, beam_pulse));
        self.exfil_beam.write_transformed(&beam_world, a);
        self.exfil_beam.draw();

        // ---------- marauder skiffs ----------
        for skiff in self.skiffs.iter() {
            let pos = skiff.position();
            let yaw = (self.player.x - pos.x).atan2(self.player.z - pos.z);
            let world = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
            self.skiff_body.write_transformed(&world, a);
            self.skiff_body.draw();
            self.skiff_eye.write_transformed(&world, a);
            self.skiff_eye.draw();
            if skiff.telegraph > 0.0 {
                let k = 0.5 + 0.5 * (self.elapsed * 14.0).sin();
                draw_sphere(pos + vec3(0.0, -0.25, 0.0), 0.32 + 0.12 * k, None, Color::new(1.0, 0.2, 0.1, 0.9));
            }
        }

        // ---------- skiff tracers ----------
        for slot in self.shots.iter() {
            let Some(shot) = slot else {
                continue;
            };
            let dir = shot.vel.normalize_or_zero();
            draw_line_3d(shot.pos - dir * 2.2, shot.pos, Color::new(1.0, 0.55, 0.2, 1.0));
            draw_sphere(shot.pos, 0.16, None, Color::new(1.0, 0.7, 0.3, 1.0));
        }

        // ---------- the scavenger ----------
        let root = Mat4::from_translation(self.player) * Mat4::from_rotation_y(self.player_yaw);
        let swing = if self.move_speed > 0.5 { self.walk_phase.sin() * 0.55 } else { 0.0 };
        let bob = if self.move_speed > 0.5 { (self.walk_phase * 2.0).sin() * 0.045 } else { 0.0 };
        let upper = root * Mat4::from_translation(vec3(0.0, bob, 0.0));

        let leg_l_world = root * Mat4::from_translation(vec3(-0.17, 0.84, 0.0)) * Mat4::from_rotation_x(swing);
        let leg_r_world = root * Mat4::from_translation(vec3(0.17, 0.84, 0.0)) * Mat4::from_rotation_x(-swing);
        self.leg_l.write_transformed(&leg_l_world, a);
        self.leg_l.draw();
        self.leg_r.write_transformed(&leg_r_world, a);
        self.leg_r.draw();
        self.torso.write_transformed(&upper, a);
        self.torso.draw();
        self.head.write_transformed(&upper, a);
        self.head.draw();

        let working = self.is_working_on_something();
        let arm_swing = if working {
            -1.15 + (self.elapsed * 17.0).sin() * 0.18
        } else {
            0.15 + (self.walk_phase * 2.0).sin() * 0.10
        };
        let arm_world = upper * Mat4::from_translation(vec3(0.37, 0.70, 0.0)) * Mat4::from_rotation_x(arm_swing);
        self.arm.write_transformed(&arm_world, a);
        self.arm.draw();
        self.particles.draw();

        set_default_camera();

        // ---------- telegraph vignette ----------
        if self.is_telegraphing() {
            let sw = screen_width();
            let sh = screen_height();
            let k = 0.10 + 0.10 * (self.elapsed * 14.0).sin().abs();
            draw_rectangle(0.0, 0.0, sw, sh * 0.055, Color::new(0.9, 0.1, 0.05, k));
            draw_rectangle(0.0, sh * 0.945, sw, sh * 0.055, Color::new(0.9, 0.1, 0.05, k));
            draw_rectangle(0.0, 0.0, sw * 0.035, sh, Color::new(0.9, 0.1, 0.05, k));
            draw_rectangle(sw * 0.965, 0.0, sw * 0.035, sh, Color::new(0.9, 0.1, 0.05, k));
        }

        self.draw_hud(assets);
    }

    fn draw_hud(&self, assets: &AssetBundle) {
        let _ = assets;
        let sw = screen_width();
        let sh = screen_height();
        let m = sw.min(sh);
        let u = |f: f32| -> f32 { m * f };

        // ---------- top-left: mission header ----------
        draw_text("RUIN SITE // SALVAGE RUN", u(0.035), u(0.052), u(0.030), Color::new(1.0, 0.78, 0.30, 0.95));
        draw_text(
            "grab it, and get out before the wasteland takes you",
            u(0.035),
            u(0.082),
            u(0.015),
            Color::new(0.9, 0.82, 0.65, 0.65),
        );

        // ---------- NERVE bar (top-center) ----------
        let bar_w = sw * 0.30;
        let bar_h = u(0.020).max(10.0);
        let bx = (sw - bar_w) * 0.5;
        let by = u(0.030);
        let ratio = (self.nerve / NERVE_MAX).clamp(0.0, 1.0);
        draw_rectangle(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, Color::new(0.0, 0.0, 0.0, 0.55));
        let nrv = if ratio > 0.35 {
            Color::new(0.95, 0.72, 0.18, 1.0)
        } else {
            let k = 0.55 + 0.45 * (self.elapsed * 9.0).sin().abs();
            Color::new(0.95 * k + 0.25, 0.12, 0.08, 1.0)
        };
        draw_rectangle(bx, by, bar_w * ratio, bar_h, nrv);
        draw_rectangle_lines(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, 2.0, Color::new(0.0, 0.0, 0.0, 0.8));
        let label = format!("NERVE  {:>3.0}", self.nerve);
        let d = measure_text(&label, None, (u(0.020)) as u16, 1.0);
        draw_text(&label, (sw - d.width) * 0.5, by + bar_h + u(0.022), u(0.020), Color::new(1.0, 0.9, 0.7, 0.9));

        // ---------- timer (top-right) ----------
        let secs = self.timer.max(0.0) as u32;
        let tlabel = format!("{:02}:{:02}", secs / 60, secs % 60);
        let urgent = secs <= 15;
        let tcol = if urgent {
            Color::new(1.0, 0.30 + 0.3 * (self.elapsed * 8.0).sin().abs(), 0.15, 1.0)
        } else {
            Color::new(0.95, 0.9, 0.8, 0.95)
        };
        let d = measure_text(&tlabel, None, (u(0.040)) as u16, 1.0);
        draw_text(&tlabel, sw - d.width - u(0.035), u(0.062), u(0.040), tcol);

        // ---------- bag (bottom-left) ----------
        let bag_y = sh - u(0.075);
        draw_text("BAG", u(0.035), bag_y, u(0.017), Color::new(0.75, 0.72, 0.62, 0.8));
        draw_text(
            &format!("+{} SCRAP", self.bag_scrap),
            u(0.035),
            bag_y + u(0.028),
            u(0.023),
            Color::new(1.0, 0.80, 0.28, 1.0),
        );
        if self.bag_medkits > 0 {
            draw_text(
                &format!("+{} MEDKIT", self.bag_medkits),
                u(0.035),
                bag_y + u(0.052),
                u(0.018),
                Color::new(0.45, 0.95, 0.55, 1.0),
            );
        }

        // ---------- tool belt (bottom-center) ----------
        let tool_y = sh - u(0.045);
        let tw = u(0.155);
        let th = u(0.040);
        let gap = u(0.012);
        let x0 = (sw - (tw * 2.0 + gap)) * 0.5;
        for (i, (name, tool)) in [("1  GRINDER", Tool::Grinder), ("2  SCREWDRIVER", Tool::Screwdriver)].iter().enumerate() {
            let x = x0 + i as f32 * (tw + gap);
            let active = self.tool == *tool;
            let bg = if active {
                Color::new(0.98, 0.66, 0.16, 0.92)
            } else {
                Color::new(0.12, 0.11, 0.09, 0.78)
            };
            draw_rectangle(x, tool_y - th, tw, th, bg);
            draw_rectangle_lines(x, tool_y - th, tw, th, 2.0, Color::new(0.0, 0.0, 0.0, 0.6));
            let col = if active {
                Color::new(0.10, 0.08, 0.05, 1.0)
            } else {
                Color::new(0.82, 0.78, 0.66, 0.9)
            };
            let d = measure_text(name, None, (th * 0.52) as u16, 1.0);
            draw_text(name, x + (tw - d.width) * 0.5, tool_y - th * 0.28, th * 0.52, col);
        }

        // ---------- context prompt (center-bottom, above tool belt) ----------
        let prompt_y = sh - u(0.115);
        if let Some(i) = self.nearest_interactable() {
            let it = &self.interact[i];
            match it.kind {
                InteractKind::Pile => {
                    let d = measure_text("[E] GRAB SCRAP", None, (u(0.024)) as u16, 1.0);
                    draw_text("[E] GRAB SCRAP", (sw - d.width) * 0.5, prompt_y, u(0.024), Color::new(1.0, 0.92, 0.55, 1.0));
                }
                InteractKind::Crate | InteractKind::Safe => {
                    let right = matches!(
                        (self.tool, it.kind),
                        (Tool::Grinder, InteractKind::Crate) | (Tool::Screwdriver, InteractKind::Safe)
                    );
                    let verb = match it.kind {
                        InteractKind::Crate => "GRINDING CRATE",
                        _ => "PRYING SAFE",
                    };
                    // progress bar
                    let pw = sw * 0.22;
                    let ph = u(0.014).max(8.0);
                    let px = (sw - pw) * 0.5;
                    let py = prompt_y - u(0.006);
                    draw_rectangle(px - 2.0, py - 2.0, pw + 4.0, ph + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
                    draw_rectangle(px, py, pw * it.progress.clamp(0.0, 1.0), ph, Color::new(1.0, 0.75, 0.22, 1.0));
                    let want = if it.kind == InteractKind::Crate { "GRINDER" } else { "SCREWDRIVER" };
                    let msg = if right {
                        format!("{verb}...  hold LMB")
                    } else {
                        format!("WRONG TOOL — {want} WORKS x3.2 FASTER")
                    };
                    let col = if right {
                        Color::new(1.0, 0.85, 0.35, 1.0)
                    } else {
                        Color::new(1.0, 0.38, 0.25, 1.0)
                    };
                    let d = measure_text(&msg, None, (u(0.020)) as u16, 1.0);
                    draw_text(&msg, (sw - d.width) * 0.5, prompt_y - u(0.030), u(0.020), col);
                }
            }
        }

        // ---------- exfil prompt ----------
        if self.exfil_distance() < EXFIL_RADIUS + 0.4 {
            let pw = sw * 0.24;
            let ph = u(0.016).max(9.0);
            let px = (sw - pw) * 0.5;
            let py = sh * 0.68;
            draw_rectangle(px - 2.0, py - 2.0, pw + 4.0, ph + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(px, py, pw * (self.exfil_hold / EXFIL_HOLD).clamp(0.0, 1.0), ph, Color::new(0.25, 0.95, 0.85, 1.0));
            let msg = "HOLD E  —  EXFIL";
            let d = measure_text(msg, None, (u(0.024)) as u16, 1.0);
            draw_text(msg, (sw - d.width) * 0.5, py - u(0.010), u(0.024), Color::new(0.55, 1.0, 0.92, 1.0));
        }

        // ---------- Android work button ----------
        #[cfg(target_os = "android")]
        {
            let bx = sw * 0.80;
            let by = sh * 0.72;
            let r = u(0.075);
            draw_circle(bx, by, r, Color::new(0.98, 0.66, 0.16, 0.30));
            draw_circle_lines(bx, by, r, 3.0, Color::new(0.98, 0.66, 0.16, 0.85));
            let d = measure_text("WORK", None, (u(0.024)) as u16, 1.0);
            draw_text("WORK", bx - d.width * 0.5, by + u(0.008), u(0.024), Color::new(1.0, 0.9, 0.6, 1.0));
        }

        // ---------- controls hint (desktop only, fades out) ----------
        #[cfg(not(target_os = "android"))]
        if self.elapsed < 6.0 {
            let alpha = ((6.0 - self.elapsed) / 2.0).clamp(0.0, 1.0) * 0.75;
            let msg = "WASD move  ·  mouse look  ·  LMB work  ·  1/2 tools  ·  E grab / exfil";
            let d = measure_text(msg, None, (u(0.016)) as u16, 1.0);
            draw_text(msg, (sw - d.width) * 0.5, sh - u(0.012), u(0.016), Color::new(0.9, 0.88, 0.8, alpha));
        }
    }
}

// ---------------- tests ----------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_layout_is_deterministic() {
        let a = site_layout(0xDEAD_BEEF_CAFE_1234);
        let b = site_layout(0xDEAD_BEEF_CAFE_1234);
        assert_eq!(a, b, "same seed must reproduce an identical layout");
    }

    #[test]
    fn site_layout_changes_with_seed() {
        let a = site_layout(1);
        let b = site_layout(2);
        assert_ne!(a, b, "different seeds should produce different sites");
    }

    #[test]
    fn site_layout_keeps_geometry_in_bounds() {
        for seed in [7u64, 42, 0xFFFF_0000_AAAA_5555, 123456789] {
            let l = site_layout(seed);
            for w in &l.walls {
                let d = vec2(w.offset.x, w.offset.z).length();
                assert!(d > 6.5 && d < 25.0, "wall out of radius: {d}");
                assert!(w.size.y > 1.0 && w.size.y < 3.7, "wall height out of range");
                assert!(w.offset.y.abs() < 3.7, "wall pivot float out of range");
            }
            for p in &l.piles {
                let d = vec2(p.x, p.z).length();
                assert!(d >= 4.9 && d <= 27.1, "pile out of radius: {d}");
            }
            for c in &l.crates {
                let d = vec2(c.x, c.z).length();
                assert!(d >= 3.9 && d <= 22.1, "crate out of radius: {d}");
            }
            let d = vec2(l.safe.x, l.safe.z).length();
            assert!(d >= 4.9 && d <= 11.1, "safe out of radius: {d}");
            // exfil hugs the arena edge opposite the spawn heading
            assert!((l.exfil.z + (ARENA_RADIUS - 3.5)).abs() < 0.01);
        }
    }

    #[test]
    fn tool_rates_favour_the_right_tool() {
        assert!(tool_rate(Tool::Grinder, InteractKind::Crate) > tool_rate(Tool::Screwdriver, InteractKind::Crate));
        assert!(tool_rate(Tool::Screwdriver, InteractKind::Safe) > tool_rate(Tool::Grinder, InteractKind::Safe));
        // wrong tool is exactly MISMATCH× slower
        let r = tool_rate(Tool::Screwdriver, InteractKind::Crate);
        assert!((r * MISMATCH - tool_rate(Tool::Grinder, InteractKind::Crate)).abs() < 1e-6);
    }

    #[test]
    fn wrap_angle_folds_into_pi() {
        use std::f32::consts::PI;
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-5 || (wrap_angle(3.0 * PI) + PI).abs() < 1e-5);
        assert!((wrap_angle(0.3) - 0.3).abs() < 1e-6);
        assert!(wrap_angle(100.0).abs() <= PI + 1e-5);
    }

    #[test]
    fn interactables_are_ordered_piles_crates_safe() {
        // the draw pass relies on this exact ordering for its mesh indices
        let layout = site_layout(99);
        let o = OnFoot::new(99);
        assert_eq!(o.interact.len(), PILE_COUNT + CRATE_COUNT + 1);
        for (i, it) in o.interact[..PILE_COUNT].iter().enumerate() {
            assert_eq!(it.kind, InteractKind::Pile);
            assert_eq!(it.pos, layout.piles[i]);
            assert!(!it.done);
        }
        for (i, it) in o.interact[PILE_COUNT..PILE_COUNT + CRATE_COUNT].iter().enumerate() {
            assert_eq!(it.kind, InteractKind::Crate);
            assert_eq!(it.pos, layout.crates[i]);
        }
        assert_eq!(o.interact[PILE_COUNT + CRATE_COUNT].kind, InteractKind::Safe);
        assert_eq!(o.interact[PILE_COUNT + CRATE_COUNT].pos, layout.safe);
    }

    #[test]
    fn skiff_positions_orbit_the_arena() {
        let o = OnFoot::new(11);
        for skiff in &o.skiffs {
            let p = skiff.position();
            let d = vec2(p.x, p.z).length();
            assert!((d - skiff.radius).abs() < 0.01, "skiff must stay on its orbit");
            assert!(p.y > 5.0 && p.y < 12.0, "skiff altitude out of band: {}", p.y);
        }
    }

    #[test]
    fn arena_spawn_is_at_the_exfil_ring() {
        // the scavenger drops in *on* the exfil pad — reaching it again is the goal
        let o = OnFoot::new(123);
        assert!((o.player - o.exfil_pos()).length() < 0.01);
    }

    #[test]
    fn fresh_run_state() {
        let o = OnFoot::new(5);
        assert_eq!(o.nerve, NERVE_MAX);
        assert_eq!(o.timer, TIME_LIMIT);
        assert_eq!(o.bag_scrap, 0);
        assert!(o.outcome.is_none());
        assert_eq!(o.tool, Tool::Grinder);
    }
}
