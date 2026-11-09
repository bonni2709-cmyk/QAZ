//! UI layer (immediate mode). Real inventory/menus arrive in Module 6.
pub mod hud;

pub use hud::{
    draw_biome_banner, draw_hud, draw_letterbox_and_vignette, draw_pause_overlay, HudData,
};
