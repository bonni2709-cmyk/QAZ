//! RAILGUN PURSUIT 3D — hardcore 3D run-and-gun & survival.
//!
//! Module 1 scope: 3D core, chase-camera truck physics, procedural road with
//! biome rotation, loading screen, HUD. No combat yet (Module 2).

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
