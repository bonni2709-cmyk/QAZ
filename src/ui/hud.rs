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
    pub kills: u32,
    pub ammo: &'a str,
    pub ammo_short: &'a str,
    pub overheated: bool,
    pub scrap: u32,
}

#[inline]
fn ui_scale() -> f32 {
    (screen_height() / 760.0).clamp(0.72, 1.35)
}

/// Width of the two ammo chips + gap, used to right-align the ammo name.
#[inline]
fn chip_w_offset(s: f32) -> f32 {
    52.0 * s + 8.0
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
    y += 26.0 * s;

    // ammo selector chips (1 = AP tungsten, 2 = incendiary)
    for (i, short) in ["AP", "INC"].iter().enumerate() {
        let chip_w = 52.0 * s;
        let active = d.ammo_short == *short;
        let cx = x + i as f32 * (chip_w + 8.0);
        draw_rectangle(cx, y, chip_w, 17.0 * s, if active { Color::new(0.85, 0.56, 0.16, 0.95) } else { Color::new(0.07, 0.07, 0.07, 0.75) });
        draw_rectangle_lines(cx, y, chip_w, 17.0 * s, 1.5, if active { Color::new(1.0, 0.8, 0.4, 1.0) } else { Color::new(0.4, 0.4, 0.38, 0.9) });
        let label = format!("{} {}", i + 1, short);
        let dim = measure_text(&label, None, (12.0 * s) as u16, 1.0);
        draw_text(&label, cx + (chip_w - dim.width) * 0.5, y + 12.5 * s, 12.0 * s, if active { Color::new(0.1, 0.08, 0.03, 1.0) } else { Color::new(0.75, 0.72, 0.65, 0.9) });
    }
    let ammo_text = d.ammo;
    draw_text(ammo_text, x + 2.0 * chip_w_offset(s), y + 13.0 * s, 12.5 * s, Color::new(0.68, 0.65, 0.55, 0.85));

    if d.overheated {
        let blink = (get_time() * 5.0).sin() > -0.2;
        if blink {
            draw_text("!! OVERHEATED !!", x, y + 34.0 * s, 17.0 * s, Color::new(0.98, 0.3, 0.12, 1.0));
        }
    }

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

    let kills = format!("KILLS  {}", d.kills);
    let kills_dim = measure_text(&kills, None, (15.0 * s) as u16, 1.0);
    draw_text(
        &kills,
        sw - 24.0 - kills_dim.width,
        cy + 22.0 * s + 18.0 * s,
        15.0 * s,
        Color::new(0.95, 0.55, 0.3, 0.95),
    );

    let scrap = format!("SCRAP  {}", d.scrap);
    let scrap_dim = measure_text(&scrap, None, (15.0 * s) as u16, 1.0);
    draw_text(
        &scrap,
        sw - 24.0 - scrap_dim.width,
        cy + 22.0 * s + 36.0 * s,
        15.0 * s,
        Color::new(0.98, 0.72, 0.24, 0.95),
    );
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
        "W/S GAS·BRAKE   A/D STEER   LMB FIRE   1/2 AMMO   R RESET   TAB WORKBENCH   ESC PAUSE",
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
    // FIRE button (bottom-right, above the steer zone)
    let fx = sw - 84.0;
    let fy = sh * 0.44;
    draw_circle(fx, fy, 56.0, Color::new(0.75, 0.18, 0.10, 0.34));
    draw_circle_lines(fx, fy, 56.0, 3.0, Color::new(1.0, 0.42, 0.22, 0.75));
    let fire_dim = measure_text("FIRE", None, 22, 1.0);
    draw_text("FIRE", fx - fire_dim.width * 0.5, fy + 8.0, 22.0, Color::new(1.0, 0.75, 0.6, 0.95));
    // WORKBENCH button (top-left, below the gauges)
    let (wb_x, wb_y, wb_r) = wb_button_center();
    draw_circle(wb_x, wb_y, wb_r, Color::new(0.55, 0.4, 0.12, 0.4));
    draw_circle_lines(wb_x, wb_y, wb_r, 3.0, Color::new(1.0, 0.72, 0.28, 0.8));
    let wb_dim = measure_text("WB", None, 22, 1.0);
    draw_text("WB", wb_x - wb_dim.width * 0.5, wb_y + 7.0, 22.0, Color::new(1.0, 0.82, 0.45, 0.95));
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
        "1 / 2 — SWAP AMMO (AP / INCENDIARY)",
        "TAB — WHEELED WORKBENCH (upgrades)",
        "",
        "MODULE 3 · SALVAGE & WORKBENCH",
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

// ---------------------------------------------------------------------------
// Module 2 — combat overlays
// ---------------------------------------------------------------------------

/// Android FIRE button geometry (hit-tested in the driving state).
#[cfg(target_os = "android")]
pub fn fire_button_center() -> (f32, f32, f32) {
    (screen_width() - 84.0, screen_height() * 0.44, 56.0)
}

/// Android WORKBENCH button geometry (hit-tested in the driving state).
#[cfg(target_os = "android")]
pub fn wb_button_center() -> (f32, f32, f32) {
    (96.0, 196.0, 34.0)
}

/// Center crosshair + hit / kill markers.
/// `hit_t`, `kill_t` ∈ [0,1] — remaining flash intensity.
pub fn draw_crosshair(overheated: bool, hit_t: f32, kill_t: f32) {
    let sw = screen_width();
    let sh = screen_height();
    let cx = sw * 0.5;
    let cy = sh * 0.5;

    let base = if overheated {
        Color::new(0.95, 0.35, 0.15, 0.95)
    } else {
        Color::new(0.95, 0.92, 0.75, 0.9)
    };
    // ring + dot + 4 ticks
    draw_circle_lines(cx, cy, 11.0, 1.75, base);
    draw_circle(cx, cy, 1.6, base);
    let tick = |dx: f32, dy: f32| {
        draw_line(cx + dx * 16.0, cy + dy * 16.0, cx + dx * 23.0, cy + dy * 23.0, 1.75, base);
    };
    tick(1.0, 0.0);
    tick(-1.0, 0.0);
    tick(0.0, 1.0);
    tick(0.0, -1.0);

    // hit marker — white X flash
    if hit_t > 0.01 {
        let k = hit_t.clamp(0.0, 1.0);
        let size = 12.0 + (1.0 - k) * 9.0;
        let gap = 5.0 + (1.0 - k) * 3.0;
        let color = Color::new(1.0, 1.0, 1.0, k);
        for sx in [-1.0f32, 1.0] {
            for sy in [-1.0f32, 1.0] {
                draw_line(
                    cx + sx * gap,
                    cy + sy * gap,
                    cx + sx * size,
                    cy + sy * size,
                    2.5,
                    color,
                );
            }
        }
    }

    // kill marker — bigger red X
    if kill_t > 0.01 {
        let k = kill_t.clamp(0.0, 1.0);
        let size = 18.0 + (1.0 - k) * 13.0;
        let gap = 7.0 + (1.0 - k) * 4.0;
        let color = Color::new(0.98, 0.28, 0.16, k);
        for sx in [-1.0f32, 1.0] {
            for sy in [-1.0f32, 1.0] {
                draw_line(
                    cx + sx * gap,
                    cy + sy * gap,
                    cx + sx * size,
                    cy + sy * size,
                    3.5,
                    color,
                );
            }
        }
    }
}

/// Flashing warning while an enemy rocket is inbound.
pub fn draw_incoming_warning() {
    let sw = screen_width();
    let blink = (get_time() * 6.0).sin() > -0.1;
    if !blink {
        return;
    }
    let text = "! INCOMING !";
    let dim = measure_text(text, None, 30, 1.0);
    draw_text(
        text,
        (sw - dim.width) * 0.5,
        screen_height() * 0.27,
        30.0,
        Color::new(0.98, 0.45, 0.12, 1.0),
    );
}

/// Wrecked overlay — run over, statistics, restart prompt.
pub fn draw_wrecked_overlay(distance_m: f32, best_m: f32, kills: u32, scrap: u32) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.12, 0.02, 0.01, 0.55));

    let title = "WRECKED";
    let dim = measure_text(title, None, 88, 1.0);
    draw_text(title, (sw - dim.width) * 0.5, sh * 0.33, 88.0, Color::new(0.86, 0.22, 0.10, 1.0));
    let sub = "THE RIG IS DONE. THE WASTELAND KEEPS YOUR BONES.";
    let sub_dim = measure_text(sub, None, 17, 1.0);
    draw_text(sub, (sw - sub_dim.width) * 0.5, sh * 0.33 + 34.0, 17.0, Color::new(0.75, 0.6, 0.5, 0.9));

    let stat1 = format!("DISTANCE   {} m", distance_m as u32);
    let stat2 = format!("BEST       {} m", best_m as u32);
    let stat3 = format!("KILLS      {}", kills);
    let stat4 = format!("SCRAP BANK {}", scrap);
    for (i, stat) in [stat1, stat2, stat3, stat4].iter().enumerate() {
        let dim = measure_text(stat, None, 26, 1.0);
        draw_text(
            stat,
            (sw - dim.width) * 0.5,
            sh * 0.46 + i as f32 * 34.0,
            26.0,
            Color::new(0.93, 0.88, 0.75, 1.0),
        );
    }

    let prompt = "PRESS R TO ROLL OUT AGAIN";
    let blink = (get_time() * 2.6).sin() > -0.25;
    if blink {
        let dim = measure_text(prompt, None, 23, 1.0);
        draw_text(
            prompt,
            (sw - dim.width) * 0.5,
            sh * 0.78,
            23.0,
            Color::new(0.96, 0.72, 0.24, 1.0),
        );
    }
}
