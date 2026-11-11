//! Runtime asset bundle (Module 5) — real files, generated at build time.
//!
//! The pack is produced by `tools/gen_assets.py` (deterministic; see the repo
//! root) into `assets/`, and `build.rs` renders this module's manifest with the
//! real paths + real byte sizes for the loading screen. When the pack is
//! absent (fresh clone, generator never ran) every slot stays `None` — the
//! game runs, silent and untextured; nothing panics.
//!
//! The streamed files are *procedural placeholders*: real bytes, real
//! resolution, deterministic generation — not hand-authored 4K PBR masters or
//! a professionally produced industrial track. See README.md ("Honesty").

use macroquad::audio::{load_sound, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::experimental::coroutines::{start_coroutine, Coroutine};
use macroquad::prelude::*;
use macroquad::texture::load_texture;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// One entry of the build-time manifest: path relative to `assets/` + real bytes on disk.
pub struct AssetManifestEntry {
    pub name: &'static str,
    pub bytes: u64,
}

// Rendered by build.rs from assets/manifest.json (empty pack + warning when absent).
include!(concat!(env!("OUT_DIR"), "/asset_manifest.rs"));

/// Total bytes of the generated pack (0 when the generator never ran).
pub fn total_bytes() -> u64 {
    ASSET_TOTAL_BYTES
}

/// Manifest entries in generator order (deterministic).
pub fn manifest() -> &'static [AssetManifestEntry] {
    ASSET_MANIFEST
}

fn bytes_of(name: &str) -> u64 {
    let mut i = 0;
    while i < ASSET_MANIFEST.len() {
        if ASSET_MANIFEST[i].name == name {
            return ASSET_MANIFEST[i].bytes;
        }
        i += 1;
    }
    0
}

/// Bytes streamed so far, shared between the loader coroutine and the loading screen.
pub struct LoadProgress {
    loaded: AtomicU64,
}

impl LoadProgress {
    fn new() -> Self {
        Self {
            loaded: AtomicU64::new(0),
        }
    }

    pub fn loaded_bytes(&self) -> u64 {
        self.loaded.load(Ordering::Relaxed)
    }
}

/// Everything the game streams at boot. All slots optional — missing pack ⇒ silent run.
#[derive(Clone)]
pub struct AssetBundle {
    pub music: Option<Sound>,
    pub wind: Option<Sound>,
    pub engine: Option<Sound>,
    pub shot_ap: Option<Sound>,
    pub shot_inc: Option<Sound>,
    pub explosion_big: Option<Sound>,
    pub explosion_small: Option<Sound>,
    pub ricochet: Option<Sound>,
    pub pickup_scrap: Option<Sound>,
    pub pickup_medkit: Option<Sound>,
    pub overheat: Option<Sound>,
    pub mine_beep: Option<Sound>,
    pub ram_crash: Option<Sound>,
    pub workbench: Option<Sound>,
    pub grinder: Option<Sound>,
    pub pry: Option<Sound>,
    pub asphalt: Option<Texture2D>,
    pub sand: Option<Texture2D>,
    pub concrete: Option<Texture2D>,
}

impl AssetBundle {
    /// All slots `None` — the game degrades to its vertex-coloured, silent self.
    pub fn empty() -> Self {
        Self {
            music: None,
            wind: None,
            engine: None,
            shot_ap: None,
            shot_inc: None,
            explosion_big: None,
            explosion_small: None,
            ricochet: None,
            pickup_scrap: None,
            pickup_medkit: None,
            overheat: None,
            mine_beep: None,
            ram_crash: None,
            workbench: None,
            grinder: None,
            pry: None,
            asphalt: None,
            sand: None,
            concrete: None,
        }
    }

    async fn load_sound_slot(progress: &LoadProgress, path: &'static str) -> Option<Sound> {
        let s = load_sound(path).await.ok();
        progress.loaded.fetch_add(bytes_of(path), Ordering::Relaxed);
        s
    }

    async fn load_texture_slot(progress: &LoadProgress, path: &'static str) -> Option<Texture2D> {
        let t = load_texture(path).await.ok();
        progress.loaded.fetch_add(bytes_of(path), Ordering::Relaxed);
        t
    }

    async fn load_all(progress: Arc<LoadProgress>) -> AssetBundle {
        let p = progress.as_ref();
        // music first — it is ~93 % of the pack, so the bar is honest early
        let music = Self::load_sound_slot(p, "music/industry_legend_5min.wav").await;
        let wind = Self::load_sound_slot(p, "audio/wind_loop.wav").await;
        let engine = Self::load_sound_slot(p, "audio/engine_loop.wav").await;
        let shot_ap = Self::load_sound_slot(p, "audio/shot_ap.wav").await;
        let shot_inc = Self::load_sound_slot(p, "audio/shot_inc.wav").await;
        let explosion_big = Self::load_sound_slot(p, "audio/explosion_big.wav").await;
        let explosion_small = Self::load_sound_slot(p, "audio/explosion_small.wav").await;
        let ricochet = Self::load_sound_slot(p, "audio/ricochet.wav").await;
        let pickup_scrap = Self::load_sound_slot(p, "audio/pickup_scrap.wav").await;
        let pickup_medkit = Self::load_sound_slot(p, "audio/pickup_medkit.wav").await;
        let overheat = Self::load_sound_slot(p, "audio/overheat.wav").await;
        let mine_beep = Self::load_sound_slot(p, "audio/mine_beep.wav").await;
        let ram_crash = Self::load_sound_slot(p, "audio/ram_crash.wav").await;
        let workbench = Self::load_sound_slot(p, "audio/workbench.wav").await;
        let grinder = Self::load_sound_slot(p, "audio/grinder.wav").await;
        let pry = Self::load_sound_slot(p, "audio/pry.wav").await;
        let asphalt = Self::load_texture_slot(p, "textures/asphalt_4k.jpg").await;
        let sand = Self::load_texture_slot(p, "textures/sand_4k.jpg").await;
        let concrete = Self::load_texture_slot(p, "textures/concrete_4k.jpg").await;
        AssetBundle {
            music,
            wind,
            engine,
            shot_ap,
            shot_inc,
            explosion_big,
            explosion_small,
            ricochet,
            pickup_scrap,
            pickup_medkit,
            overheat,
            mine_beep,
            ram_crash,
            workbench,
            grinder,
            pry,
            asphalt,
            sand,
            concrete,
        }
    }

    // ---------- playback helpers: no allocation, everything optional ----------

    pub fn play(&self, slot: &Option<Sound>, volume: f32) {
        if let Some(s) = slot {
            play_sound(s, PlaySoundParams { looped: false, volume });
        }
    }

    pub fn start_loop(&self, slot: &Option<Sound>, volume: f32) {
        if let Some(s) = slot {
            play_sound(s, PlaySoundParams { looped: true, volume });
        }
    }

    pub fn set_volume(&self, slot: &Option<Sound>, volume: f32) {
        if let Some(s) = slot {
            set_sound_volume(s, volume);
        }
    }

    pub fn stop(&self, slot: &Option<Sound>) {
        if let Some(s) = slot {
            stop_sound(s);
        }
    }

    pub fn shot(&self, incendiary: bool) {
        if incendiary {
            self.play(&self.shot_inc, 0.80);
        } else {
            self.play(&self.shot_ap, 0.90);
        }
    }

    pub fn explosion(&self, big: bool) {
        if big {
            self.play(&self.explosion_big, 1.0);
        } else {
            self.play(&self.explosion_small, 0.55);
        }
    }
}

/// Kick off the background stream. The counter is what the loading screen watches.
pub fn start_loading() -> (Arc<LoadProgress>, Coroutine<AssetBundle>) {
    let progress = Arc::new(LoadProgress::new());
    let loader = progress.clone();
    let handle = start_coroutine(async move { AssetBundle::load_all(loader).await });
    (progress, handle)
}
