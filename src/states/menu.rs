//! Main menu (Module 6) — the rig idles behind the title, the wasteland waits.
//!
//! Single-action arcade menu: roll out, or admire the stats of your last
//! death. All figures come straight off the JSON save (systems::savegame) —
//! the menu is the proof that persistence is real, not session-local.

use macroquad::prelude::*;

use crate::entities::truck::Truck;
use crate::systems::codex::{self, ENTRIES};
use crate::systems::savegame::{self, SaveData};

pub struct MenuState {
    truck: Truck,
    orbit: f32,
    elapsed: f32,
    fade_in: f32,
    save: SaveData,
    codex_open: bool,
}

impl MenuState {
    pub fn new() -> MenuState {
        MenuState {
            truck: Truck::new(),
            orbit: 0.6,
            elapsed: 0.0,
            fade_in: 1.0,
            save: savegame::load(),
            codex_open: false,
        }
    }

    /// Tick the menu. Returns `true` on the frame the player commits to a run.
    pub fn update(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        self.orbit += dt * 0.55;
        self.fade_in = (self.fade_in - dt * 1.8).max(0.0);

        if is_key_pressed(KeyCode::C) {
            self.codex_open = !self.codex_open;
        }
        let escape = {
            #[cfg(target_os = "android")]
            {
                is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Back)
            }
            #[cfg(not(target_os = "android"))]
            {
                is_key_pressed(KeyCode::Escape)
            }
        };
        if escape && self.codex_open {
            self.codex_open = false;
        }
        if self.codex_open {
            return false;
        }

        let start = is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::Space)
            || is_mouse_button_pressed(MouseButton::Left);
        // require at least half a second so an accidental click-through from
        // the loading state cannot launch a run by itself
        start && self.elapsed > 0.5
    }

    pub fn draw(&mut self) {
        let sw = screen_width();
        let sh = screen_height();

        clear_background(Color::new(0.030, 0.028, 0.034, 1.0));

        // ---------- 3D backdrop: the rig on the turntable ----------
        let cam_pos = vec3(
            self.orbit.sin() * 10.8,
            3.1 + (self.elapsed * 0.7).sin() * 0.3,
            self.orbit.cos() * 10.8,
        );
        set_camera(&Camera3D {
            position: cam_pos,
            target: vec3(0.0, 1.45, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 40.0_f32.to_radians(),
            z_near: 0.5,
            z_far: 140.0,
            ..Default::default()
        });
        draw_plane(
            vec3(0.0, 0.0, 0.0),
            vec2(70.0, 70.0),
            None,
            Color::new(0.085, 0.080, 0.070, 1.0),
        );
        // hazard ring on the hangar floor
        draw_plane(
            vec3(0.0, 0.012, 0.0),
            vec2(5.6, 5.6),
            None,
            Color::new(0.135, 0.125, 0.085, 1.0),
        );
        self.truck.yaw = self.orbit;
        self.truck.draw(0.85);
        set_default_camera();

        // ---------- grading: vignette bars ----------
        draw_rectangle(0.0, 0.0, sw, sh * 0.14, Color::new(0.0, 0.0, 0.0, 0.68));
        draw_rectangle(0.0, sh * 0.70, sw, sh * 0.30, Color::new(0.0, 0.0, 0.0, 0.72));

        // ---------- title ----------
        let title = "RAILGUN PURSUIT";
        let dim = measure_text(title, None, 78, 1.0);
        let tx = (sw - dim.width - 66.0) * 0.5;
        draw_text(title, tx + 3.0, sh * 0.145 + 3.0, 78.0, Color::new(0.0, 0.0, 0.0, 0.85));
        draw_text(title, tx, sh * 0.145, 78.0, Color::new(0.94, 0.88, 0.74, 1.0));
        draw_text(
            "3-D",
            tx + dim.width + 16.0,
            sh * 0.145,
            78.0,
            Color::new(0.95, 0.55, 0.14, 1.0),
        );

        let sub = "HARDCORE RUN & GUN SURVIVAL ON THE DEAD HIGHWAY";
        let sub_dim = measure_text(sub, None, 17, 1.0);
        draw_text(
            sub,
            (sw - sub_dim.width) * 0.5,
            sh * 0.145 + 30.0,
            17.0,
            Color::new(0.72, 0.66, 0.47, 0.9),
        );

        // ---------- hazard divider ----------
        let div_y = sh * 0.235;
        let bar_half = (sw * 0.34).min(430.0);
        draw_rectangle(sw * 0.5 - bar_half, div_y, bar_half * 2.0, 3.0, Color::new(0.95, 0.55, 0.14, 0.85));
        let notch = 22.0;
        let mut nx = sw * 0.5 - bar_half + 6.0;
        while nx < sw * 0.5 + bar_half - 6.0 {
            draw_line(nx, div_y + 3.0, nx + 10.0, div_y + 12.0, 3.0, Color::new(0.95, 0.55, 0.14, 0.55));
            nx += notch;
        }

        // ---------- call to action ----------
        let blink = (get_time() * 2.3).sin() > -0.35;
        if blink {
            let prompt = "PRESS ENTER / TAP TO ROLL OUT";
            let prompt_dim = measure_text(prompt, None, 26, 1.0);
            draw_text(
                prompt,
                (sw - prompt_dim.width) * 0.5,
                sh * 0.775,
                26.0,
                Color::new(1.0, 0.82, 0.35, 0.97),
            );
        }

        // ---------- service record (straight off the JSON save) ----------
        let stats_y = sh * 0.845;
        let mut stats = vec![
            format!("BEST   {:.0} m", self.save.best_m),
            format!("SCRAP  {}", self.save.scrap),
            format!("RUNS   {}", self.save.total_runs),
            format!("KILLS  {}", self.save.total_kills),
        ];
        if self.save.ng_plus > 0 {
            stats.push(format!("NG+  ◉{}", self.save.ng_plus));
        }
        let cell_w = (sw * 0.9).min(720.0) / stats.len() as f32;
        let start_x = (sw - cell_w * stats.len() as f32) * 0.5;
        for (i, stat) in stats.iter().enumerate() {
            let d = measure_text(stat, None, 21, 1.0);
            let cx = start_x + cell_w * i as f32 + cell_w * 0.5;
            draw_text(stat, cx - d.width * 0.5, stats_y, 21.0, Color::new(0.82, 0.77, 0.60, 0.95));
        }

        // ---------- footer honesty strip ----------
        let left = "PROGRESS SAVED LOCALLY (JSON)";
        draw_text(
            left,
            14.0,
            sh - 10.0,
            14.0,
            Color::new(0.42, 0.40, 0.33, 0.85),
        );
        let right = "v0.1.0 · MODULE 8";
        let right_dim = measure_text(right, None, 14, 1.0);
        draw_text(
            right,
            sw - right_dim.width - 14.0,
            sh - 10.0,
            14.0,
            Color::new(0.42, 0.40, 0.33, 0.85),
        );

        // ---------- codex hint ----------
        let hint = "[C]  FIELD CODEX";
        draw_text(hint, 14.0, sh * 0.68, 17.0, Color::new(0.55, 0.52, 0.42, 0.75));

        // ---------- codex overlay ----------
        if self.codex_open {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.05, 0.048, 0.045, 0.88));
            let panel_w = (sw * 0.82).min(880.0);
            let px = (sw - panel_w) * 0.5;
            let py = sh * 0.055;
            let ph = sh * 0.83;
            draw_rectangle(px - 2.0, py - 2.0, panel_w + 4.0, ph + 4.0, Color::new(0.95, 0.55, 0.14, 0.55));
            draw_rectangle(px, py, panel_w, ph, Color::new(0.09, 0.087, 0.082, 0.97));

            let unlocked = codex::unlocked_count(&self.save);
            let header = format!("FIELD CODEX   {} / {}", unlocked, ENTRIES.len());
            let hd = measure_text(&header, None, 30, 1.0);
            draw_text(&header, px + (panel_w - hd.width) * 0.5, py + 46.0, 30.0, Color::new(0.95, 0.72, 0.28, 1.0));

            let mut col_x = px + 30.0;
            let mut y = py + 92.0;
            let line_h = 20.0f32;
            let body_size = 15.0f32;
            for entry in ENTRIES {
                if y > py + ph - 30.0 {
                    col_x = px + panel_w * 0.5 + 22.0;
                    y = py + 92.0;
                }
                if entry.unlocked(&self.save) {
                    draw_text(entry.title, col_x, y, 19.0, Color::new(0.94, 0.82, 0.45, 1.0));
                    y += 22.0;
                    for line in entry.body {
                        draw_text(line, col_x + 12.0, y, body_size, Color::new(0.78, 0.75, 0.66, 0.95));
                        y += line_h;
                    }
                    y += 16.0;
                } else {
                    draw_text("▮▮▮▮▮  CLASSIFIED", col_x, y, 17.0, Color::new(0.38, 0.36, 0.30, 0.8));
                    y += 22.0 + line_h * 2.0 + 16.0;
                }
            }

            let foot = "[C] / [ESC] / TAP-OUT — BACK TO THE RIG";
            let fd = measure_text(foot, None, 14, 1.0);
            draw_text(foot, px + (panel_w - fd.width) * 0.5, py + ph - 12.0, 14.0, Color::new(0.5, 0.48, 0.4, 0.85));
        }

        // hard fade from black on entry
        if self.fade_in > 0.0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, self.fade_in));
        }
    }
}
