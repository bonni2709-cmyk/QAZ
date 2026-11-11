//! Loading screen — the *real* asset stream (Module 5).
//!
//! The pack is produced at build time by `tools/gen_assets.py` (deterministic)
//! into `assets/`; `build.rs` renders the manifest with real paths and real
//! byte sizes, and the progress bar below is driven by the actual bytes the
//! macroquad file IO has delivered — not a simulation. When the pack is absent
//! (fresh clone, generator never ran) the screen honestly shows an empty
//! manifest and says how to fix it, instead of inventing sizes.
//!
//! Hold Space / Enter / click (Android: tap & hold) to fast-forward once the
//! stream is resident. Sizes shown are on-disk bytes of the generated pack.

use macroquad::experimental::coroutines::Coroutine;
use macroquad::prelude::*;

use crate::entities::truck::Truck;
use crate::systems::assets::{self, AssetBundle};
use std::sync::Arc;

pub struct LoadingState {
    elapsed: f32,
    truck: Truck,
    turntable: f32,
    progress: Arc<assets::LoadProgress>,
    loader: Option<Coroutine<AssetBundle>>,
    bundle: Option<AssetBundle>,
    stream_done: bool,
}

impl LoadingState {
    pub fn new() -> LoadingState {
        let (progress, loader) = assets::start_loading();
        LoadingState {
            elapsed: 0.0,
            truck: Truck::new(),
            turntable: 0.0,
            progress,
            loader: Some(loader),
            bundle: None,
            stream_done: false,
        }
    }

    /// Hand the resident pack to the next state (None when the pack was absent).
    pub fn take_bundle(&mut self) -> AssetBundle {
        self.bundle.take().unwrap_or_else(AssetBundle::empty)
    }

    fn total_bytes(&self) -> u64 {
        assets::total_bytes()
    }

    fn progress_ratio(&self) -> f32 {
        let total = self.total_bytes();
        if total == 0 {
            return if self.elapsed > 1.0 { 1.0 } else { self.elapsed };
        }
        (self.progress.loaded_bytes() as f32 / total as f32).clamp(0.0, 1.0)
    }

    /// Advance the stream. Returns `true` on the frame the run may start.
    pub fn update(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        self.turntable += dt * 0.55;

        if !self.stream_done {
            let finished = self.loader.as_ref().map(|l| l.is_done()).unwrap_or(true);
            if finished {
                if let Some(handle) = self.loader.take() {
                    self.bundle = handle.retrieve();
                }
                self.stream_done = true;
            }
        }

        let total = self.total_bytes();
        let stream_ready = if total == 0 {
            self.elapsed > 1.2 // dry-manifest beat so the warning is readable
        } else {
            self.stream_done && self.progress.loaded_bytes() >= total
        };
        let fast_forward =
            is_key_down(KeyCode::Space) || is_key_down(KeyCode::Enter) || is_mouse_button_down(MouseButton::Left);
        stream_ready && (fast_forward || self.elapsed > 1.0)
    }

    fn current_asset(&self) -> &'static str {
        let loaded = self.progress.loaded_bytes();
        let mut acc = 0u64;
        for entry in assets::manifest() {
            acc += entry.bytes;
            if acc > loaded {
                return entry.name;
            }
        }
        "FINALIZING SHADER CACHE…"
    }

    pub fn draw(&mut self) {
        let sw = screen_width();
        let sh = screen_height();
        let total_bytes = self.total_bytes();
        let total_mb = total_bytes as f32 / 1e6;
        let loaded_mb = (total_mb * self.progress_ratio()).min(total_mb);
        let progress = self.progress_ratio();

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
        draw_text(tag, tx + dim.width + 18.0, sh * 0.155, 84.0, Color::new(0.95, 0.55, 0.14, 1.0));

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

        // current asset (real file being streamed, or the honest empty-pack note)
        if total_bytes == 0 {
            let warn = "NO ASSET PACK FOUND — RUN  python3 tools/gen_assets.py";
            let warn_dim = measure_text(warn, None, 16, 1.0);
            draw_text(
                warn,
                (sw - warn_dim.width) * 0.5,
                by - 14.0,
                16.0,
                Color::new(0.95, 0.45, 0.35, 0.95),
            );
        } else {
            let asset = self.current_asset();
            let asset_dim = measure_text(asset, None, 17, 1.0);
            draw_text(
                asset,
                (sw - asset_dim.width) * 0.5,
                by - 14.0,
                17.0,
                Color::new(0.65, 0.72, 0.66, 0.85),
            );
        }

        draw_rectangle(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, Color::new(0.0, 0.0, 0.0, 0.75));
        draw_rectangle(bx, by, bar_w, bar_h, Color::new(0.10, 0.10, 0.10, 1.0));
        draw_rectangle(bx, by, bar_w * progress, bar_h, Color::new(0.95, 0.55, 0.14, 1.0));
        draw_rectangle(bx, by, bar_w * progress, bar_h * 0.42, Color::new(1.0, 0.78, 0.35, 0.9));
        draw_rectangle_lines(bx - 2.0, by - 2.0, bar_w + 4.0, bar_h + 4.0, 2.0, Color::new(0.55, 0.45, 0.3, 1.0));

        let status = if total_bytes == 0 {
            "0 / 0 MB   —   PACK NOT GENERATED".to_string()
        } else {
            format!(
                "{:.1} / {:.1} MB   —   {}%",
                loaded_mb,
                total_mb,
                (progress * 100.0) as u32
            )
        };
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
        let foot = "MODULE 6 · BUILD-TIME ASSET PACK (PROCEDURAL) · JSON SAVES — TARGET 60 FPS · INFINIX NOTE 30";
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
