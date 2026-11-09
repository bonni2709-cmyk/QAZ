//! Procedural endless road — biome rotation, deterministic chunk props.
//!
//! The highway runs along +Z forever. Everything is drawn **statelessly**:
//! for each visible chunk we re-derive props from a deterministic hash of the
//! chunk index, so there is nothing to allocate, store or leak. Distances in
//! meters, world coordinates in macroquad's Y-up convention.

use macroquad::prelude::*;

/// Length of one road chunk, meters.
pub const CHUNK_LEN: f32 = 20.0;
/// How many chunks ahead/behind the truck are drawn.
const VIEW_CHUNKS: i64 = 15;
/// Asphalt half-width.
pub const ROAD_HALF: f32 = 3.6;
/// Length of the seamless biome cycle, meters.
const BIOME_CYCLE: f32 = 2_200.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    /// Baked red dunes, cacti, wrecked cars — the default badlands.
    Wasteland,
    /// Rotten green fog, pools of sludge, dead trees.
    ToxicSwamp,
    /// Collapsed concrete skeletons of a dead metropolis.
    RuinedCity,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Wasteland => "WASTELAND",
            Biome::ToxicSwamp => "TOXIC SWAMP",
            Biome::RuinedCity => "RUINED CITY",
        }
    }

    /// 0-based band index inside one biome cycle, used for transition banners.
    pub fn band_index(self) -> u32 {
        match self {
            Biome::Wasteland => 0,
            Biome::ToxicSwamp => 1,
            Biome::RuinedCity => 2,
        }
    }
}

pub fn biome_at(z: f32) -> Biome {
    let t = z.rem_euclid(BIOME_CYCLE);
    if t < 900.0 {
        Biome::Wasteland
    } else if t < 1_500.0 {
        Biome::ToxicSwamp
    } else {
        Biome::RuinedCity
    }
}

/// Deterministic 64-bit hash (splitmix64) — stable world across sessions.
fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Hash → [0, 1).
fn hash01(seed: u64) -> f32 {
    (hash64(seed) >> 11) as f32 / (1u64 << 53) as f32
}

/// Uniform in [lo, hi).
fn hash_range(seed: u64, lo: f32, hi: f32) -> f32 {
    lo + hash01(seed) * (hi - lo)
}

#[inline]
fn shaded(color: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::new(
        (color.r * k).min(1.0),
        (color.g * k).min(1.0),
        (color.b * k).min(1.0),
        color.a,
    )
}

#[inline]
fn sand_color(biome: Biome) -> Color {
    match biome {
        Biome::Wasteland => Color::new(0.72, 0.55, 0.30, 1.0),
        Biome::ToxicSwamp => Color::new(0.30, 0.36, 0.17, 1.0),
        Biome::RuinedCity => Color::new(0.36, 0.34, 0.32, 1.0),
    }
}

#[inline]
fn asphalt_color() -> Color {
    Color::new(0.145, 0.145, 0.155, 1.0)
}

#[inline]
fn sky_haze(biome: Biome) -> Color {
    match biome {
        Biome::Wasteland => Color::new(0.85, 0.62, 0.38, 1.0),
        Biome::ToxicSwamp => Color::new(0.45, 0.58, 0.30, 1.0),
        Biome::RuinedCity => Color::new(0.55, 0.52, 0.47, 1.0),
    }
}

/// Color the scene fog/background should fade into for a biome.
pub fn haze_color(biome: Biome, ambient: f32) -> Color {
    shaded(sky_haze(biome), (ambient * 0.85 + 0.1).min(1.0))
}

/// Draw the ground + asphalt + markings + props for all visible chunks.
/// `ambient` ∈ [0, 1.05] — scales every static color (cheap fake lighting).
pub fn draw_world(center_z: f32, ambient: f32) {
    let first = (center_z / CHUNK_LEN).floor() as i64 - 1;
    for i in first..first + VIEW_CHUNKS {
        draw_chunk(i, ambient);
    }
}

fn draw_chunk(index: i64, ambient: f32) {
    let z0 = index as f32 * CHUNK_LEN;
    let zc = z0 + CHUNK_LEN * 0.5;
    let biome = biome_at(zc);

    let amb = ambient.clamp(0.0, 1.0);

    // ---- ground slab (wide, cheap, one plane per chunk) ----
    draw_plane(
        vec3(0.0, 0.0, zc),
        vec2(90.0, CHUNK_LEN * 0.5),
        None,
        shaded(sand_color(biome), amb),
    );

    // ---- asphalt ----
    draw_plane(
        vec3(0.0, 0.03, zc),
        vec2(ROAD_HALF, CHUNK_LEN * 0.5),
        None,
        shaded(asphalt_color(), (amb * 0.9 + 0.08).min(1.0)),
    );

    // ---- edge lines ----
    let line = shaded(Color::new(0.82, 0.78, 0.58, 1.0), amb);
    draw_plane(vec3(-(ROAD_HALF - 0.22), 0.06, zc), vec2(0.09, CHUNK_LEN * 0.5), None, line);
    draw_plane(vec3(ROAD_HALF - 0.22, 0.06, zc), vec2(0.09, CHUNK_LEN * 0.5), None, line);

    // ---- dashed center line (every other chunk) ----
    if index.rem_euclid(2) == 0 {
        draw_plane(
            vec3(0.0, 0.06, zc),
            vec2(0.12, CHUNK_LEN * 0.28),
            None,
            shaded(Color::new(0.85, 0.82, 0.66, 1.0), amb),
        );
    }

    // ---- roadside props, seeded by chunk index ----
    let seed = hash64(index as u64 ^ 0x5A51_9E37_1F2B_3C4D);
    let props = 2 + (hash64(seed) % 3) as u32; // 2..=4 props per chunk per side
    for side in 0..2i32 {
        let sign = if side == 0 { -1.0f32 } else { 1.0 };
        for p in 0..props {
            let s = seed ^ hash64((p as u64 + 1) * 0x100_0193) ^ (side as u64) << 32;
            let px = sign * hash_range(s, ROAD_HALF + 1.6, 38.0);
            let pz = z0 + hash_range(hash64(s ^ 0x77), 1.0, CHUNK_LEN - 1.0);
            draw_prop(biome, s, px, pz, amb);
        }
    }
}

fn draw_prop(biome: Biome, seed: u64, x: f32, z: f32, amb: f32) {
    match biome {
        Biome::Wasteland => draw_wasteland_prop(seed, x, z, amb),
        Biome::ToxicSwamp => draw_swamp_prop(seed, x, z, amb),
        Biome::RuinedCity => draw_city_prop(seed, x, z, amb),
    }
}

fn draw_wasteland_prop(seed: u64, x: f32, z: f32, amb: f32) {
    let kind = hash64(seed) % 4;
    match kind {
        // saguaro-ish cactus
        0 => {
            let h = hash_range(hash64(seed ^ 1), 1.1, 2.4);
            let green = shaded(Color::new(0.22, 0.38, 0.16, 1.0), amb);
            draw_cylinder(vec3(x, 0.0, z), 0.16, 0.2, h, None, green);
            draw_cylinder(vec3(x + 0.28, h * 0.42, z), 0.09, 0.11, h * 0.45, None, green);
            draw_cylinder(vec3(x - 0.26, h * 0.55, z), 0.08, 0.1, h * 0.38, None, green);
        }
        // sun-baked boulder
        1 => {
            let s = hash_range(hash64(seed ^ 2), 0.5, 1.5);
            draw_cube(
                vec3(x, s * 0.32, z),
                vec3(s * 1.4, s * 0.65, s),
                None,
                shaded(Color::new(0.48, 0.40, 0.32, 1.0), amb),
            );
        }
        // wrecked car husk
        2 => {
            let rust = shaded(Color::new(0.42, 0.20, 0.09, 1.0), amb);
            let dark = shaded(Color::new(0.16, 0.13, 0.12, 1.0), amb);
            draw_cube(vec3(x, 0.42, z), vec3(1.8, 0.55, 3.6), None, rust);
            draw_cube(vec3(x, 0.85, z - 0.2), vec3(1.6, 0.35, 1.9), None, dark);
        }
        // telephone pole
        _ => {
            let wood = shaded(Color::new(0.28, 0.2, 0.12, 1.0), amb);
            draw_cylinder(vec3(x, 0.0, z), 0.09, 0.12, 5.2, None, wood);
            draw_cube(vec3(x, 4.55, z), vec3(1.5, 0.14, 0.14), None, wood);
        }
    }
}

fn draw_swamp_prop(seed: u64, x: f32, z: f32, amb: f32) {
    let kind = hash64(seed) % 4;
    match kind {
        // bubbling sludge pool
        0 => {
            draw_plane(
                vec3(x, 0.05, z),
                vec2(hash_range(hash64(seed ^ 3), 1.2, 2.6), hash_range(hash64(seed ^ 4), 1.2, 2.2)),
                None,
                shaded(Color::new(0.27, 0.47, 0.13, 1.0), amb * 1.15),
            );
        }
        // dead tree
        1 => {
            let bark = shaded(Color::new(0.23, 0.17, 0.12, 1.0), amb);
            let h = hash_range(hash64(seed ^ 5), 1.6, 3.4);
            draw_cylinder(vec3(x, 0.0, z), 0.12, 0.2, h, None, bark);
            draw_cube(vec3(x + 0.45, h * 0.72, z), vec3(1.0, 0.09, 0.09), None, bark);
            draw_cube(vec3(x - 0.4, h * 0.5, z + 0.15), vec3(0.85, 0.08, 0.08), None, bark);
        }
        // toxic barrel cluster
        2 => {
            let barrel_a = shaded(Color::new(0.31, 0.44, 0.19, 1.0), amb);
            let barrel_b = shaded(Color::new(0.45, 0.40, 0.16, 1.0), amb);
            draw_cylinder(vec3(x, 0.0, z), 0.3, 0.3, 0.9, None, barrel_a);
            draw_cylinder(vec3(x + 0.68, 0.0, z + 0.2), 0.3, 0.3, 0.9, None, barrel_b);
            draw_cylinder(vec3(x + 0.3, 0.88, z + 0.1), 0.3, 0.3, 0.9, None, barrel_a);
        }
        // half-sunk signpost
        _ => {
            let wood = shaded(Color::new(0.33, 0.3, 0.22, 1.0), amb);
            draw_cylinder(vec3(x, 0.0, z), 0.07, 0.09, 1.9, None, wood);
            draw_cube(vec3(x, 1.65, z), vec3(1.1, 0.4, 0.06), None, wood);
        }
    }
}

fn draw_city_prop(seed: u64, x: f32, z: f32, amb: f32) {
    let kind = hash64(seed) % 4;
    let concrete = shaded(Color::new(0.42, 0.40, 0.37, 1.0), amb);
    let dark = shaded(Color::new(0.16, 0.15, 0.14, 1.0), amb);
    match kind {
        // collapsed tower block
        0 => {
            let h = hash_range(hash64(seed ^ 6), 3.5, 9.0);
            let w = hash_range(hash64(seed ^ 7), 2.0, 4.5);
            draw_cube(vec3(x, h * 0.5, z), vec3(w, h, w * 0.9), None, concrete);
            // blown-out window band
            draw_cube(vec3(x, h * 0.72, z), vec3(w * 1.02, h * 0.22, w * 0.92), None, dark);
        }
        // low ruin / rubble pile
        1 => {
            let s = hash_range(hash64(seed ^ 8), 0.8, 2.0);
            draw_cube(vec3(x, s * 0.35, z), vec3(s * 1.6, s * 0.7, s * 1.2), None, concrete);
            draw_cube(vec3(x + s * 0.8, s * 0.18, z + 0.4), vec3(s * 0.7, s * 0.36, s * 0.5), None, dark);
        }
        // rebar column
        2 => {
            let h = hash_range(hash64(seed ^ 9), 1.5, 4.2);
            draw_cube(vec3(x, h * 0.5, z), vec3(0.5, h, 0.5), None, concrete);
        }
        // burnt-out bus skeleton
        _ => {
            draw_cube(vec3(x, 0.7, z), vec3(2.2, 1.15, 6.0), None, dark);
            draw_cube(vec3(x, 1.35, z), vec3(2.0, 0.18, 5.6), None, concrete);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_bands_cover_cycle() {
        assert_eq!(biome_at(0.0), Biome::Wasteland);
        assert_eq!(biome_at(899.0), Biome::Wasteland);
        assert_eq!(biome_at(900.0), Biome::ToxicSwamp);
        assert_eq!(biome_at(1499.0), Biome::ToxicSwamp);
        assert_eq!(biome_at(1500.0), Biome::RuinedCity);
        assert_eq!(biome_at(2199.0), Biome::RuinedCity);
        assert_eq!(biome_at(BIOME_CYCLE), Biome::Wasteland);
        assert_eq!(biome_at(-1.0), Biome::RuinedCity); // rem_euclid wraps
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(hash64(42), hash64(42));
        assert_ne!(hash64(1), hash64(2));
    }

    #[test]
    fn hash01_in_unit_interval() {
        for i in 0..512u64 {
            let v = hash01(i);
            assert!((0.0..1.0).contains(&v));
        }
    }
}
