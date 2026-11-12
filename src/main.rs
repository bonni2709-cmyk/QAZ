//! RAILGUN PURSUIT 3D — hardcore 3D run-and-gun & survival.
//!
//! State flow: Loading → Menu → Driving ⇄ OnFoot (hold F at a ruin site).
//! Scope landed through Module 7: 3D core, turret combat, loot economy,
//! sandstorms & the JUGGERNAUT, build-time asset pack, menus & JSON saves,
//! on-foot ruin-site salvage sorties.

mod entities;
mod states;
mod systems;
mod ui;
mod world;

use macroquad::prelude::*;

/// On-device crash screen. `panic = "unwind"` + `set_panic_handler` let macroquad
/// catch panics in game code and show this instead of a black flash — the phone
/// itself then tells us what broke, no adb needed.
async fn panic_screen(message: String, backtrace: String) {
    loop {
        clear_background(Color::new(0.16, 0.03, 0.03, 1.0));
        draw_text("RAILGUN PURSUIT 3D — CRASHED", 24.0, 44.0, 34.0, Color::new(1.0, 0.85, 0.35, 1.0));
        draw_text("SCREENSHOT THIS SCREEN AND SEND IT", 24.0, 74.0, 18.0, WHITE);

        let mut y = 112.0;
        // the panic message is the important part: keep it big
        for line in message.lines().take(6) {
            draw_text(line, 24.0, y, 17.0, Color::new(1.0, 0.55, 0.35, 1.0));
            y += 21.0;
        }
        y += 8.0;
        for line in backtrace.lines() {
            if y > screen_height() - 18.0 {
                break;
            }
            draw_text(line, 24.0, y, 12.0, Color::new(0.75, 0.72, 0.66, 0.9));
            y += 15.0;
        }
        next_frame().await;
    }
}

#[macroquad::main("RAILGUN PURSUIT 3D")]
async fn main() {
    set_panic_handler(panic_screen);

    let mut app = states::App::new();
    loop {
        app.frame();
        next_frame().await;
    }
}
