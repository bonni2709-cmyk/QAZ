//! Wheeled workbench — the upgrade shop bolted to the rig's bed.
//!
//! Immediate-mode overlay drawn while `workbench_open`. The world pauses
//! (same deal as the pause menu); TAB / ESC / Android WB button closes it.
//! Mouse and touch both work: macroquad simulates mouse events from touches,
//! so the BUY buttons hit-test with plain `mouse_position`.

use macroquad::prelude::*;

use crate::systems::upgrades::{UpgradeState, TRACKS};

pub fn draw_workbench(u: &mut UpgradeState, flash: &mut f32) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.025, 0.04, 0.80));

    let pw = (sw * 0.74).min(680.0).max(420.0);
    let ph = 452.0_f32.min(sh * 0.88);
    let px = (sw - pw) * 0.5;
    let py = (sh - ph) * 0.5;

    draw_rectangle(px, py, pw, ph, Color::new(0.085, 0.09, 0.10, 0.97));
    draw_rectangle_lines(px, py, pw, ph, 2.0, Color::new(0.72, 0.47, 0.15, 0.9));
    for (bx, by) in [
        (px + 13.0, py + 13.0),
        (px + pw - 13.0, py + 13.0),
        (px + 13.0, py + ph - 13.0),
        (px + pw - 13.0, py + ph - 13.0),
    ] {
        draw_circle(bx, by, 3.6, Color::new(0.56, 0.56, 0.52, 1.0));
        draw_circle(bx, by, 1.4, Color::new(0.18, 0.18, 0.18, 1.0));
    }

    // ---- header ----
    draw_text(
        "WHEELED WORKBENCH",
        px + 26.0,
        py + 44.0,
        32.0,
        Color::new(0.95, 0.85, 0.6, 1.0),
    );
    draw_text(
        "BED-MOUNT SERVICE RIG · FIELD UPGRADES",
        px + 26.0,
        py + 65.0,
        12.0,
        Color::new(0.62, 0.58, 0.45, 0.9),
    );
    let scrap_txt = format!("SCRAP  {}", u.scrap());
    let sd = measure_text(&scrap_txt, None, 25, 1.0);
    draw_text(
        &scrap_txt,
        px + pw - 26.0 - sd.width,
        py + 45.0,
        25.0,
        Color::new(0.98, 0.72, 0.24, 1.0),
    );
    let hint = "earned from wrecks & crates";
    let hd = measure_text(hint, None, 11, 1.0);
    draw_text(
        hint,
        px + pw - 26.0 - hd.width,
        py + 63.0,
        11.0,
        Color::new(0.6, 0.55, 0.4, 0.85),
    );
    draw_line(
        px + 14.0,
        py + 76.0,
        px + pw - 14.0,
        py + 76.0,
        1.5,
        Color::new(0.72, 0.47, 0.15, 0.5),
    );

    // ---- rows ----
    for (i, track) in TRACKS.iter().enumerate() {
        let track = *track;
        let row_y = py + 88.0 + i as f32 * 78.0;

        draw_text(
            track.name(),
            px + 26.0,
            row_y + 19.0,
            16.0,
            Color::new(0.92, 0.88, 0.76, 1.0),
        );

        // rank pips
        let level = u.level(track);
        for p in 0..3u8 {
            let cx = px + 33.0 + p as f32 * 22.0;
            let cy = row_y + 36.0;
            if p < level {
                draw_circle(cx, cy, 6.2, Color::new(0.95, 0.62, 0.16, 1.0));
                draw_circle(cx, cy, 2.4, Color::new(0.55, 0.3, 0.05, 1.0));
            } else {
                draw_circle_lines(cx, cy, 6.2, 1.5, Color::new(0.5, 0.47, 0.4, 0.9));
            }
        }
        draw_text(
            track.blurb(),
            px + 104.0,
            row_y + 41.0,
            11.0,
            Color::new(0.6, 0.6, 0.55, 0.9),
        );

        // buy button
        let bw = 128.0;
        let bh = 40.0;
        let bx = px + pw - bw - 26.0;
        let by = row_y;
        match u.cost(track) {
            Some(cost) => {
                let ok = u.can_buy(track);
                let fill = if ok {
                    Color::new(0.72, 0.5, 0.12, 0.95)
                } else {
                    Color::new(0.15, 0.145, 0.13, 0.9)
                };
                draw_rectangle(bx, by, bw, bh, fill);
                draw_rectangle_lines(
                    bx,
                    by,
                    bw,
                    bh,
                    1.5,
                    if ok {
                        Color::new(1.0, 0.78, 0.4, 1.0)
                    } else {
                        Color::new(0.4, 0.38, 0.33, 0.9)
                    },
                );
                let label = format!("FIT · {}", cost);
                let ld = measure_text(&label, None, 16, 1.0);
                draw_text(
                    &label,
                    bx + (bw - ld.width) * 0.5,
                    by + 25.0,
                    16.0,
                    if ok {
                        Color::new(0.12, 0.1, 0.04, 1.0)
                    } else {
                        Color::new(0.58, 0.55, 0.47, 0.9)
                    },
                );
                if is_mouse_button_pressed(MouseButton::Left) {
                    let (mx, my) = mouse_position();
                    if mx >= bx && mx <= bx + bw && my >= by && my <= by + bh && u.buy(track) {
                        *flash = 1.0;
                    }
                }
            }
            None => {
                draw_rectangle(bx, by, bw, bh, Color::new(0.12, 0.16, 0.11, 0.9));
                draw_rectangle_lines(bx, by, bw, bh, 1.5, Color::new(0.4, 0.55, 0.35, 0.9));
                let label = "MAXED";
                let ld = measure_text(label, None, 16, 1.0);
                draw_text(
                    label,
                    bx + (bw - ld.width) * 0.5,
                    by + 25.0,
                    16.0,
                    Color::new(0.55, 0.8, 0.45, 0.95),
                );
            }
        }
    }

    // ---- footer ----
    let foot = "TAB / ESC / BACK — CLOSE WORKBENCH";
    let fd = measure_text(foot, None, 12, 1.0);
    draw_text(foot, (sw - fd.width) * 0.5, py + ph - 14.0, 12.0, Color::new(0.55, 0.52, 0.42, 0.9));

    // purchase flash (kept above everything, short-lived)
    if *flash > 0.01 {
        let a = flash.clamp(0.0, 1.0);
        let txt = "WORKSHOP COMPLETE";
        let td = measure_text(txt, None, 28, 1.0);
        draw_text(
            txt,
            (sw - td.width) * 0.5,
            (py * 0.5).max(28.0),
            28.0,
            Color::new(1.0, 0.82, 0.35, a),
        );
    }
}
