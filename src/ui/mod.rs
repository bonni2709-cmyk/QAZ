//! UI layer (immediate mode). Real inventory/menus arrive in Module 6.
pub mod hud;

pub use hud::{
    draw_biome_banner, draw_crosshair, draw_hud, draw_incoming_warning, draw_letterbox_and_vignette,
    draw_pause_overlay, draw_wrecked_overlay, HudData,
};

#[cfg(target_os = "android")]
pub use hud::fire_button_center;
