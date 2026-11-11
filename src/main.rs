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

#[macroquad::main("RAILGUN PURSUIT 3D")]
async fn main() {
    let mut app = states::App::new();
    loop {
        app.frame();
        next_frame().await;
    }
}
