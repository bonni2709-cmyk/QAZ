//! In-game HUD and cinematic overlays (macroquad immediate-mode, 2D layer).

use macroquad::prelude::*;

/// Everything the HUD needs for one frame.
pub struct HudData<'a> {
    pub health: f32,
    pub max_health: f32,
    pub fuel: f32,
    pub max_fuel: f32,
    pub turret_heat: f32,
    pub speed_kmh: f32,
    pub distance_m: f32,
    pub best_m: f32,
    pub biome: &'a str,
    pub offroad: bool,
    pub low_fuel: bool,
}

#[inline]
fn ui_scale() -> f32 {
    (screen_height() / 760.0).clamp(0.72, 1.35)
}

fn draw_bar(x: f32, y: f32, w: f32, h: f32, ratio: f32, fill: Color, label: &str, s: f32) {
    let ratio = ratio.clamp(0.0, 1.0);
    draw_rectangle(x - 1.0, y - 1.0, w + 2.0, h + 2.0, Color::new(0.0, 0.0, 0.0, 0.72));
    draw_rectangle(x, y, w, h, Color::new(0.09, 0.09, 0.09, 0.9));
    if ratio > 0.001 {
        draw_rectangle(x, y, w * ratio, h, fill);
        draw_rectangle(x, y, w * ratio, h * 0.42, Color::new(1.0, 1.0, 1.0, 0.22));
    }
    draw_text(label, x + 6.0, y + h - 3.5, 12.5 * s, Color::new(0.92, 0.92, 0.9, 0.95));
}

/// HULL / FUEL / TURRET — left column.
fn draw_status_bars(d: &HudData<'_>, s: f32) {
    let x = 18.0;
    let w = 235.0 * s;
    let h = 15.0 * s;
    let mut y = 20.0;

    let hull_ratio = d.health / d.max_health;
    let hull_color = if hull_ratio > 0.55 {
        Color::new(0.35, 0.78, 0.30, 1.0)
    } else if hull_ratio > 0.25 {
        Color::new(0.85, 0.62, 0.15, 1.0)
    } else {
        Color::new(0.82, 0.22, 0.16, 1.0)
    };
    draw_bar(x, y, w, h, hull_ratio, hull_color, "HULL", s);
    y += 23.0 * s;

    draw_bar(x, y, w, h, d.fuel / d.max_fuel, Color::new(0.9, 0.62, 0.12, 1.0), "FUEL", s);
    y += 23.0 * s;

    let heat_ratio = d.turret_heat.clamp(0.0, 1.0);
    let heat_color = if heat_ratio > 0.8 {
        Color::new(0.9, 0.25, 0.12, 1.0)
    } else {
        Color::new(0.25, 0.72, 0.82, 1.0)
    };
    draw_bar(x, y, w, h, heat_ratio, heat_color, "TURRET TEMP", s);

    if d.low_fuel {
        let blink = (get_time() * 2.4).sin() > 0.0;
        if blink {
            draw_text(
                "! FUEL LOW",
                x,
                y + 40.0 * s,
                19.0 * s,
                Color::new(0.98, 0.66, 0.10, 1.0),
            );
        }
    }
}

/// Distance / best / biome — right column.
fn draw_run_stats(d: &HudData<'_>, s: f32) {
    let sw = screen_width();
    let dist = format!("{}", d.distance_m as u32);
    let dist_dim = measure_text(&dist, None, (44.0 * s) as u16, 1.0);
    draw_text(
        &dist,
        sw - 24.0 - dist_dim.width,
        46.0 * s,
        44.0 * s,
        Color::new(0.96, 0.91, 0.78, 1.0),
    );
    let unit = "METERS";
    let unit_dim = measure_text(unit, None, (13.0 * s) as u16, 1.0);
    draw_text(unit, sw - 24.0 - unit_dim.width, 46.0 * s + 15.0 * s, 13.0 * s, Color::new(0.72, 0.68, 0.55, 0.9));

    let best = format!("BEST  {} m", d.best_m as u32);
    let best_dim = measure_text(&best, None, (15.0 * s) as u16, 1.0);
    draw_text(
        &best,
        sw - 24.0 - best_dim.width,
        46.0 * s + 36.0 * s,
        15.0 * s,
        Color::new(0.65, 0.71, 0.6, 0.9),
    );

    // biome chip
    let chip = d.biome;
    let chip_dim = measure_text(chip, None, (15.0 * s) as u16, 1.0);
    let chip_w = chip_dim.width + 22.0;
    let cx = sw - 24.0 - chip_w;
    let cy = 46.0 * s + 48.0 * s;
    draw_rectangle(cx, cy, chip_w, 22.0 * s, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_rectangle_lines(cx, cy, chip_w, 22.0 * s, 1.5, Color::new(0.85, 0.62, 0.25, 0.8));
    draw_text(chip, cx + 11.0, cy + 16.0 * s, 15.0 * s, Color::new(0.92, 0.72, 0.28, 1.0));
}

/// Big speedometer bottom-center.
fn draw_speedometer(d: &HudData<'_>, s: f32) {
    let sw = screen_width();
    let sh = screen_height();
    let speed = format!("{}", d.speed_kmh as u32);
    let speed_dim = measure_text(&speed, None, (66.0 * s) as u16, 1.0);
    let y = sh - 26.0 * s;
    draw_text(&speed, sw * 0.5 - speed_dim.width * 0.5, y, 66.0 * s, Color::new(0.97, 0.94, 0.86, 1.0));
    let unit = "KM/H";
    let unit_dim = measure_text(unit, None, (13.0 * s) as u16, 1.0);
    draw_text(unit, sw * 0.5 - unit_dim.width * 0.5, y + 16.0 * s, 13.0 * s, Color::new(0.72, 0.68, 0.55, 0.9));
}

pub fn draw_hud(d: &HudData<'_>) {
    let s = ui_scale();
    draw_status_bars(d, s);
    draw_run_stats(d, s);
    draw_speedometer(d, s);

    // FPS readout (bottom-right, small — the 60 FPS contract is monitored)
    let fps = format!("{} FPS", get_fps());
    let dim = measure_text(&fps, None, 15, 1.0);
    draw_text(&fps, screen_width() - 14.0 - dim.width, screen_height() - 10.0, 15.0, Color::new(0.45, 0.75, 0.45, 0.85));

    // control hints (desktop only; Android renders touch zones instead)
    #[cfg(not(target_os = "android"))]
    draw_text(
        "W/S GAS·BRAKE   A/D STEER   R RESET   ESC PAUSE",
        18.0,
        screen_height() - 10.0,
        14.0,
        Color::new(0.55, 0.55, 0.55, 0.75),
    );

    if d.offroad {
        let sw = screen_width();
        let msg = "SOFT SAND — SPEED BLEEDING";
        let dim = measure_text(msg, None, 16, 1.0);
        draw_text(msg, (sw - dim.width) * 0.5, screen_height() * 0.66, 16.0, Color::new(0.9, 0.72, 0.35, 0.95));
    }

    #[cfg(target_os = "android")]
    draw_touch_zones();
}

/// Semi-transparent steer zones for touch devices (Module 1 scope; the full
/// virtual joystick layout arrives with the on-foot gameplay in Module 7).
#[cfg(target_os = "android")]
fn draw_touch_zones() {
    let sh = screen_height();
    let sw = screen_width();
    let zone_w = sw * 0.30;
    let zone_h = sh * 0.42;
    let y = sh - zone_h - 6.0;
    let alpha = 0.10;
    // left zone + chevron
    draw_rectangle(6.0, y, zone_w, zone_h, Color::new(1.0, 1.0, 1.0, alpha));
    draw_rectangle_lines(6.0, y, zone_w, zone_h, 2.0, Color::new(1.0, 1.0, 1.0, 0.22));
    let cy = y + zone_h * 0.5;
    draw_triangle(
        vec2(6.0 + zone_w * 0.5 - 26.0, cy),
        vec2(6.0 + zone_w * 0.5 + 16.0, cy - 34.0),
        vec2(6.0 + zone_w * 0.5 + 16.0, cy + 34.0),
        Color::new(1.0, 1.0, 1.0, 0.30),
    );
    // right zone + chevron
    draw_rectangle(sw - zone_w - 6.0, y, zone_w, zone_h, Color::new(1.0, 1.0, 1.0, alpha));
    draw_rectangle_lines(sw - zone_w - 6.0, y, zone_w, zone_h, 2.0, Color::new(1.0, 1.0, 1.0, 0.35));
    let cx = sw - zone_w - 6.0 + zone_w * 0.5;
    draw_triangle(
        vec2(cx + 26.0, cy),
        vec2(cx - 16.0, cy - 34.0),
        vec2(cx - 16.0, cy + 34.0),
        Color::new(1.0, 1.0, 1.0, 0.30),
    );
    // auto-gas note
    let note = "AUTO-GAS ENGAGED";
    let dim = measure_text(note, None, 13, 1.0);
    draw_text(note, (sw - dim.width) * 0.5, sh - 8.0, 13.0, Color::new(0.6, 0.6, 0.6, 0.6));
}

/// Cinematic letterbox bars + side gradient bands (cheap tilt-shift flavour).
pub fn draw_letterbox_and_vignette() {
    let sw = screen_width();
    let sh = screen_height();

    // letterbox
    draw_rectangle(0.0, 0.0, sw, sh * 0.042, Color::new(0.0, 0.0, 0.0, 0.88));
    draw_rectangle(0.0, sh * 0.928, sw, sh * 0.072, Color::new(0.0, 0.0, 0.0, 0.88));

    // gradient bands top & bottom (fake depth-of-field falloff)
    const STEPS: usize = 7;
    for i in 0..STEPS {
        let t = i as f32 / STEPS as f32;
        let alpha = 0.34 * (1.0 - t);
        let h = sh * 0.028;
        let y = sh * 0.042 + t * sh * 0.135;
        draw_rectangle(0.0, y, sw, h, Color::new(0.02, 0.02, 0.03, alpha));
        let yb = sh * 0.928 - (t + 1.0) * h;
        draw_rectangle(0.0, yb, sw, h, Color::new(0.02, 0.02, 0.03, alpha));
    }

    // side vignette
    const SIDE: usize = 5;
    for i in 0..SIDE {
        let t = i as f32 / SIDE as f32;
        let alpha = 0.30 * (1.0 - t);
        let w = sw * 0.020;
        draw_rectangle(t * sw * 0.075, 0.0, w, sh, Color::new(0.0, 0.0, 0.0, alpha));
        draw_rectangle(sw - (t + 1.0) * w, 0.0, w, sh, Color::new(0.0, 0.0, 0.0, alpha));
    }
}

/// Biome transition banner (top-center, fades in/out).
pub fn draw_biome_banner(text: &str, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if a <= 0.01 {
        return;
    }
    let sw = screen_width();
    let y = screen_height() * 0.135;

    let label = "— NOW ENTERING —";
    let label_dim = measure_text(label, None, 15, 1.0);
    draw_text(label, (sw - label_dim.width) * 0.5, y - 26.0, 15.0, Color::new(0.75, 0.7, 0.55, a * 0.85));

    let dim = measure_text(text, None, 46, 1.0);
    draw_text(text, (sw - dim.width) * 0.5 + 2.0, y + 2.0, 46.0, Color::new(0.0, 0.0, 0.0, a * 0.7));
    draw_text(text, (sw - dim.width) * 0.5, y, 46.0, Color::new(0.95, 0.62, 0.17, a));
}

/// Pause overlay (ESC / Android back).
pub fn draw_pause_overlay() {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.62));

    let title = "PAUSED";
    let dim = measure_text(title, None, 64, 1.0);
    draw_text(title, (sw - dim.width) * 0.5, sh * 0.38, 64.0, Color::new(0.94, 0.88, 0.74, 1.0));

    let lines = [
        "ESC / BACK — RESUME",
        "R — RESTART RUN",
        "",
        "MODULE 1 · 3D CORE & TRANSPORT",
    ];
    let mut y = sh * 0.50;
    for line in lines {
        if line.is_empty() {
            y += 14.0;
            continue;
        }
        let dim = measure_text(line, None, 20, 1.0);
        draw_text(
            line,
            (sw - dim.width) * 0.5,
            y,
            20.0,
            Color::new(0.70, 0.67, 0.55, 0.9),
        );
        y += 30.0;
    }
}
