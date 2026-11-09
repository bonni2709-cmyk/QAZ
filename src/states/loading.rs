//! Loading screen — the truck turntable behind a 200+ MB asset stream.
//!
//! Module 1 ships with a *simulated* streaming pass over a manifest whose
//! sizes already reflect the shipping budget (≥ 200 MB of 4K textures, WAV
//! audio and high-poly models). Module 5 swaps the simulator for real
//! `load_texture` / `load_sound` calls; the UI stays as-is.

use macroquad::prelude::*;

use crate::entities::truck::Truck;

struct AssetItem {
    name: &'static str,
    size_mb: f32,
}

/// Manifest for the full game. Sums to ~268 MB — comfortably over the ≥ 200 MB shipping target.
const ASSETS: &[AssetItem] = &[
    AssetItem { name: "music/industry_legend_5min.wav", size_mb: 52.3 },
    AssetItem { name: "music/main_theme_industrial.wav", size_mb: 24.6 },
    AssetItem { name: "sfx/v8_engine_loop.wav", size_mb: 22.4 },
    AssetItem { name: "textures/terrain_wang_4k.png", size_mb: 18.2 },
    AssetItem { name: "textures/track_asphalt_4k.png", size_mb: 16.4 },
    AssetItem { name: "textures/metal_hull_4k.png", size_mb: 15.6 },
    AssetItem { name: "textures/sky_clouds_8k.png", size_mb: 14.5 },
    AssetItem { name: "models/truck_highpoly.glb", size_mb: 12.7 },
    AssetItem { name: "models/tanker_boss_highpoly.glb", size_mb: 11.8 },
    AssetItem { name: "models/armoredcar_highpoly.glb", size_mb: 10.2 },
    AssetItem { name: "models/biker_highpoly.glb", size_mb: 9.4 },
    AssetItem { name: "models/buggy_highpoly.glb", size_mb: 9.1 },
    AssetItem { name: "models/crawler_worm_highpoly.glb", size_mb: 7.9 },
    AssetItem { name: "sfx/explosions_pack.wav", size_mb: 8.6 },
    AssetItem { name: "sfx/turret_762_pack.wav", size_mb: 4.2 },
    AssetItem { name: "sfx/railgun_shot.wav", size_mb: 3.1 },
    AssetItem { name: "sfx/tyre_screech_loop.wav", size_mb: 2.7 },
    AssetItem { name: "sfx/ricochet_pack.wav", size_mb: 2.1 },
    AssetItem { name: "sfx/ui_clicks.wav", size_mb: 1.4 },
    AssetItem { name: "textures/codex_spreads_4k.png", size_mb: 6.8 },
    AssetItem { name: "textures/outpost_props_4k.png", size_mb: 5.2 },
    AssetItem { name: "models/outpost_ruins.glb", size_mb: 3.9 },
    AssetItem { name: "models/merchant_convoy.glb", size_mb: 2.6 },
    AssetItem { name: "models/kamikaze_convoy.glb", size_mb: 2.4 },
];

fn total_size_mb() -> f32 {
    ASSETS.iter().map(|a| a.size_mb).sum()
}

pub struct LoadingState {
    loaded_mb: f32,
    total_mb: f32,
    elapsed: f32,
    truck: Truck,
    turntable: f32,
}

impl LoadingState {
    pub fn new() -> LoadingState {
        LoadingState {
            loaded_mb: 0.0,
            total_mb: total_size_mb(),
            elapsed: 0.0,
            truck: Truck::new(),
            turntable: 0.0,
        }
    }

    /// Simulated stream speed, MB/s (jittery, feels like a real IO curve).
    /// Hold Space / Enter / LMB to fast-forward.
    fn stream_rate(&self) -> f32 {
        let mut rate = 44.0 + 26.0 * (self.elapsed * 1.3).sin() + 10.0 * (self.elapsed * 3.7).cos();
        if is_key_down(KeyCode::Space) || is_key_down(KeyCode::Enter) || is_mouse_button_down(MouseButton::Left) {
            rate *= 9.0;
        }
        rate.max(9.0)
    }

    /// Advance the simulation. Returns `true` on the frame streaming completes.
    pub fn update(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        self.turntable += dt * 0.55;
        self.loaded_mb = (self.loaded_mb + self.stream_rate() * dt).min(self.total_mb);
        self.loaded_mb >= self.total_mb
    }

    fn current_asset(&self) -> &'static str {
        let mut acc = 0.0;
        for asset in ASSETS {
            acc += asset.size_mb;
            if acc > self.loaded_mb {
                return asset.name;
            }
        }
        "FINALIZING SHADER CACHE…"
    }

    pub fn draw(&mut self) {
        let sw = screen_width();
        let sh = screen_height();

        clear_background(Color::new(0.035, 0.035, 0.045, 1.0));

        // ---------- 3D turntable ----------
        let orbit = self.turntable;
        let cam_pos = vec3(orbit.sin() * 10.5, 3.4 + (self.elapsed * 0.8).sin() * 0.35, orbit.cos() * 10.5);
        set_camera(&Camera3D {
            position: cam_pos,
            target: vec3(0.0, 1.35, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 42.0_f32.to_radians(),
            z_near: 0.5,
            z_far: 120.0,
            ..Default::default()
        });
        // hangar floor + a single shaft of light
        draw_plane(
            vec3(0.0, 0.0, 0.0),
            vec2(60.0, 60.0),
            None,
            Color::new(0.10, 0.095, 0.085, 1.0),
        );
        draw_plane(
            vec3(0.0, 0.01, 0.0),
            vec2(4.6, 4.6),
            None,
            Color::new(0.16, 0.15, 0.12, 1.0),
        );
        self.truck.yaw = orbit;
        self.truck.draw(0.9);
        set_default_camera();

        // ---------- HUD layer ----------
        let progress = self.loaded_mb / self.total_mb;

        // vignette
        draw_rectangle(0.0, 0.0, sw, sh * 0.16, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(0.0, sh * 0.78, sw, sh * 0.22, Color::new(0.0, 0.0, 0.0, 0.6));

        // title
        let title = "RAILGUN PURSUIT";
        let dim = measure_text(title, None, 84, 1.0);
        let tx = (sw - dim.width) * 0.5;
        draw_text(title, tx + 3.0, sh * 0.155 + 3.0, 84.0, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text(title, tx, sh * 0.155, 84.0, Color::new(0.94, 0.88, 0.74, 1.0));

        let tag = "3 D";
        let tag_dim = measure_text(tag, None, 84, 1.0);
        draw_text(tag, tx + dim.width + 18.0, sh * 0.155, 84.0, Color::new(0.95, 0.55, 0.14, 1.0));
        let _ = tag_dim;

        let sub = "HARDCORE RUN & GUN SURVIVAL";
        let sub_dim = measure_text(sub, None, 21, 1.0);
        draw_text(
            sub,
            (sw - sub_dim.width) * 0.5,
            sh * 0.155 + 30.0,
            21.0,
            Color::new(0.80, 0.72, 0.52, 0.9),
        );

        // ---------- progress block ----------
        let bar_w = (sw * 0.62).min(680.0);
        let bar_h = 26.0;
        let bx = (sw - bar_w) * 0.5;
        let by = sh * 0.815;

        // current asset
        let asset = self.current_asset();
        let asset_dim = measure_text(asset, None, 17, 1.0);
        draw_text(
            asset,
            (sw - asset_dim.width) * 0.5,
            by - 14.0,
            17.0,
            Color::new(0.65, 0.72, 0.66, 0.85),
        );

        draw_rectangle(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, Color::new(0.0, 0.0, 0.0, 0.75));
        draw_rectangle(bx, by, bar_w, bar_h, Color::new(0.10, 0.10, 0.10, 1.0));
        draw_rectangle(bx, by, bar_w * progress, bar_h, Color::new(0.95, 0.55, 0.14, 1.0));
        draw_rectangle(bx, by, bar_w * progress, bar_h * 0.42, Color::new(1.0, 0.78, 0.35, 0.9));
        draw_rectangle_lines(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, 2.0, Color::new(0.55, 0.45, 0.3, 1.0));

        let status = format!(
            "{:.1} / {:.1} MB   —   {}%",
            self.loaded_mb,
            self.total_mb,
            (progress * 100.0) as u32
        );
        let status_dim = measure_text(&status, None, 19, 1.0);
        draw_text(&status, (sw - status_dim.width) * 0.5, by + bar_h + 22.0, 19.0, WHITE);

        let hint = "HOLD SPACE / TAP TO FAST-FORWARD";
        let hint_dim = measure_text(hint, None, 15, 1.0);
        draw_text(
            hint,
            (sw - hint_dim.width) * 0.5,
            by + bar_h + 46.0,
            15.0,
            Color::new(0.55, 0.55, 0.55, 0.8),
        );

        // ---------- footer ----------
        let foot = "MODULE 1 · 3D CORE & TRANSPORT — TARGET 60 FPS · INFINIX NOTE 30 (HELIO G99)";
        let foot_dim = measure_text(foot, None, 15, 1.0);
        draw_text(
            foot,
            (sw - foot_dim.width) * 0.5,
            sh - 12.0,
            15.0,
            Color::new(0.48, 0.44, 0.36, 0.9),
        );
    }
}
