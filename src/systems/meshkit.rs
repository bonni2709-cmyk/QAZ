//! Low-allocation mesh kit for rotated / articulated geometry.
//!
//! Macroquad's built-in 3D primitives (`draw_cube`, `draw_cylinder`, `draw_plane`,
//! ...) are **axis-aligned** — they carry no rotation. Anything in this game that
//! actually turns (the war-rig truck, its wheels, the turret) is assembled from
//! [`PartMesh`] parts instead:
//!
//! * vertices are stored once, in local space, together with a baked
//!   per-vertex *directional shade* (fake baked sun) and a base color;
//! * every frame `PartMesh::write_transformed` bakes `world_matrix * local_space`
//!   into a reusable `Mesh` buffer and modulates the color by the global
//!   `ambient` factor (this is what makes the day/night cycle tint the truck) —
//!   **zero heap allocations per frame**;
//! * parts are explicitly merged at build time (`merge`), so the whole truck
//!   body is a *single* draw call.

use macroquad::prelude::*;

/// Direction toward the sun. Used to bake per-vertex directional shading once,
/// at build time. Must stay normalized-ish; only the sign of the dot product matters.
const SUN_DIR: Vec3 = Vec3::new(0.45, 0.82, 0.34);

#[inline]
fn sun_dot(normal: Vec3) -> f32 {
    let d = (normal.x * SUN_DIR.x + normal.y * SUN_DIR.y + normal.z * SUN_DIR.z)
        / SUN_DIR.length();
    d.clamp(-1.0, 1.0)
}

/// Shade factor for flat-shaded cuboid faces, picked from the face normal.
#[inline]
fn face_shade(normal: Vec3) -> f32 {
    // wrap the sun dot into [0, 1], bias so back faces are clearly darker
    (0.62 + 0.42 * sun_dot(normal)).clamp(0.42, 1.06)
}

/// A rigid part of a larger articulated model.
///
/// Cheap to re-transform every frame; do **not** put per-frame scratch data
/// anywhere else — `mesh` is overwritten in place by `write_transformed`.
pub struct PartMesh {
    local_positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    shades: Vec<f32>,
    emissive: Vec<bool>,
    base_colors: Vec<[u8; 4]>,
    indices: Vec<u16>,
    /// Re-usable output buffer passed to `draw_mesh` every frame.
    pub mesh: Mesh,
}

impl PartMesh {
    fn from_raw(
        positions: Vec<Vec3>,
        uvs: Vec<Vec2>,
        shades: Vec<f32>,
        emissive: Vec<bool>,
        colors: Vec<Color>,
        indices: Vec<u16>,
    ) -> PartMesh {
        let base_colors: Vec<[u8; 4]> = colors.into_iter().map(Into::into).collect();
        let vertices: Vec<Vertex> = positions
            .iter()
            .zip(uvs.iter())
            .zip(base_colors.iter())
            .map(|((p, uv), c)| Vertex::new2(*p, *uv, Color::from_rgba(c[0], c[1], c[2], c[3])))
            .collect();
        PartMesh {
            local_positions: positions,
            uvs,
            shades,
            emissive,
            base_colors,
            indices: indices.clone(),
            mesh: Mesh {
                vertices,
                indices,
                texture: None,
            },
        }
    }

    /// Merge another part into `self`. Both parts must have offsets already
    /// baked into their local positions (see `cuboid_at` / `cylinder_at`).
    pub fn merge(&mut self, other: &PartMesh) {
        let base = self.local_positions.len() as u16;
        self.local_positions.extend_from_slice(&other.local_positions);
        self.uvs.extend_from_slice(&other.uvs);
        self.shades.extend_from_slice(&other.shades);
        self.emissive.extend_from_slice(&other.emissive);
        self.base_colors.extend_from_slice(&other.base_colors);
        self.indices.extend(other.indices.iter().map(|i| i + base));
        // rebuild the staging buffer with the merged content; this runs once
        // at load time, never per frame.
        self.rebuild_staging();
    }

    fn rebuild_staging(&mut self) {
        let vertices: Vec<Vertex> = self
            .local_positions
            .iter()
            .zip(self.uvs.iter())
            .zip(self.base_colors.iter())
            .map(|((p, uv), c)| {
                Vertex::new2(*p, *uv, Color::from_rgba(c[0], c[1], c[2], c[3]))
            })
            .collect();
        self.mesh = Mesh {
            vertices,
            indices: self.indices.clone(),
            texture: None,
        };
    }

    /// Axis-aligned cuboid baked at `offset` (local-space translation).
    pub fn cuboid_at(size: Vec3, color: Color, offset: Vec3) -> PartMesh {
        Self::cuboid_offset(size, color, offset, false)
    }

    /// Cuboid with emissive flag (headlights / taillights — unaffected by ambient).
    pub fn cuboid_glow_at(size: Vec3, color: Color, offset: Vec3) -> PartMesh {
        Self::cuboid_offset(size, color, offset, true)
    }

    fn cuboid_offset(size: Vec3, color: Color, offset: Vec3, glow: bool) -> PartMesh {
        let hx = size.x * 0.5;
        let hy = size.y * 0.5;
        let hz = size.z * 0.5;

        // (normal, 4 corners ccw)
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            (
                Vec3::new(0.0, 0.0, 1.0),
                [
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(-hx, hy, hz),
                ],
            ),
            (
                Vec3::new(0.0, 0.0, -1.0),
                [
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(hx, hy, -hz),
                ],
            ),
            (
                Vec3::new(1.0, 0.0, 0.0),
                [
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(hx, hy, hz),
                ],
            ),
            (
                Vec3::new(-1.0, 0.0, 0.0),
                [
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(-hx, hy, hz),
                    Vec3::new(-hx, hy, -hz),
                ],
            ),
            (
                Vec3::new(0.0, 1.0, 0.0),
                [
                    Vec3::new(-hx, hy, hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                ],
            ),
            (
                Vec3::new(0.0, -1.0, 0.0),
                [
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(-hx, -hy, hz),
                ],
            ),
        ];

        let mut positions = Vec::with_capacity(24);
        let mut uvs = Vec::with_capacity(24);
        let mut shades = Vec::with_capacity(24);
        let mut emissive = Vec::with_capacity(24);
        let mut colors = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);

        for (normal, corners) in faces {
            let base = positions.len() as u16;
            let shade = face_shade(normal);
            for corner in corners {
                positions.push(corner + offset);
            }
            uvs.extend_from_slice(&[
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ]);
            shades.extend_from_slice(&[shade; 4]);
            emissive.extend_from_slice(&[glow; 4]);
            colors.extend_from_slice(&[color; 4]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        PartMesh::from_raw(positions, uvs, shades, emissive, colors, indices)
    }

    /// Vertical cylinder baked at `offset` (bottom-center origin like macroquad's).
    pub fn cylinder_at(
        radius_top: f32,
        radius_bottom: f32,
        height: f32,
        sides: u8,
        color: Color,
        offset: Vec3,
    ) -> PartMesh {
        let sides = sides.max(3) as usize;
        let step = std::f32::consts::TAU / sides as f32;

        let mut positions = Vec::with_capacity((sides + 1) * 2 + (sides + 2) * 2);
        let mut uvs = Vec::new();
        let mut shades = Vec::new();
        let mut emissive = Vec::new();
        let mut colors = Vec::new();
        let mut indices: Vec<u16> = Vec::new();

        // ---- body ----
        let body_base = positions.len() as u16;
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            let normal = Vec3::new(sin, 0.0, cos);
            let shade = (0.58 + 0.46 * sun_dot(normal)).clamp(0.40, 1.06);
            positions.push(Vec3::new(sin * radius_bottom, 0.0, cos * radius_bottom) + offset);
            positions.push(Vec3::new(sin * radius_top, height, cos * radius_top) + offset);
            uvs.push(Vec2::new(i as f32 / sides as f32, 0.0));
            uvs.push(Vec2::new(i as f32 / sides as f32, 1.0));
            shades.push(shade);
            shades.push(shade);
            emissive.push(false);
            emissive.push(false);
            colors.push(color);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            let b = body_base + i * 2;
            indices.extend_from_slice(&[b, b + 1, b + 2, b + 1, b + 3, b + 2]);
        }

        // ---- top cap ----
        let top_base = positions.len() as u16;
        positions.push(Vec3::new(0.0, height, 0.0) + offset);
        uvs.push(Vec2::new(0.5, 0.5));
        shades.push(1.02);
        emissive.push(false);
        colors.push(color);
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            positions.push(Vec3::new(sin * radius_top, height, cos * radius_top) + offset);
            uvs.push(Vec2::new(0.5 + sin * 0.5, 0.5 + cos * 0.5));
            shades.push(1.02);
            emissive.push(false);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            indices.extend_from_slice(&[top_base, top_base + 1 + i, top_base + 2 + i]);
        }

        // ---- bottom cap ----
        let bot_base = positions.len() as u16;
        positions.push(Vec3::ZERO + offset);
        uvs.push(Vec2::new(0.5, 0.5));
        shades.push(0.45);
        emissive.push(false);
        colors.push(color);
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            positions.push(Vec3::new(sin * radius_bottom, 0.0, cos * radius_bottom) + offset);
            uvs.push(Vec2::new(0.5 + sin * 0.5, 0.5 + cos * 0.5));
            shades.push(0.45);
            emissive.push(false);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            indices.extend_from_slice(&[bot_base, bot_base + 2 + i, bot_base + 1 + i]);
        }

        PartMesh::from_raw(positions, uvs, shades, emissive, colors, indices)
    }

    /// Cylinder along the **X** axis, centered at `offset` — wheels, axles, barrels.
    /// Same construction as `cylinder_at` but with an YZ circle extruded along X.
    pub fn cylinder_along_x_at(
        radius_top: f32,
        radius_bottom: f32,
        length: f32,
        sides: u8,
        color: Color,
        offset: Vec3,
    ) -> PartMesh {
        let sides = sides.max(3) as usize;
        let step = std::f32::consts::TAU / sides as f32;
        let half = length * 0.5;

        let mut positions = Vec::with_capacity((sides + 1) * 2 + (sides + 2) * 2);
        let mut uvs = Vec::new();
        let mut shades = Vec::new();
        let mut emissive = Vec::new();
        let mut colors = Vec::new();
        let mut indices: Vec<u16> = Vec::new();

        // ---- body (circle in YZ, extruded along X) ----
        let body_base = positions.len() as u16;
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            let normal = Vec3::new(0.0, sin, cos); // radial, outward
            let shade = (0.58 + 0.46 * sun_dot(normal)).clamp(0.40, 1.06);
            positions.push(Vec3::new(-half, sin * radius_bottom, cos * radius_bottom) + offset);
            positions.push(Vec3::new(half, sin * radius_top, cos * radius_top) + offset);
            uvs.push(Vec2::new(i as f32 / sides as f32, 0.0));
            uvs.push(Vec2::new(i as f32 / sides as f32, 1.0));
            shades.push(shade);
            shades.push(shade);
            emissive.push(false);
            emissive.push(false);
            colors.push(color);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            let b = body_base + i * 2;
            indices.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
        }

        // ---- +X cap ----
        let cap_pos = positions.len() as u16;
        positions.push(Vec3::new(half, 0.0, 0.0) + offset);
        uvs.push(Vec2::new(0.5, 0.5));
        shades.push((0.62 + 0.42 * sun_dot(Vec3::new(1.0, 0.0, 0.0))).clamp(0.42, 1.06));
        emissive.push(false);
        colors.push(color);
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            positions.push(Vec3::new(half, sin * radius_top, cos * radius_top) + offset);
            uvs.push(Vec2::new(0.5 + sin * 0.5, 0.5 + cos * 0.5));
            shades.push((0.62 + 0.42 * sun_dot(Vec3::new(1.0, 0.0, 0.0))).clamp(0.42, 1.06));
            emissive.push(false);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            indices.extend_from_slice(&[cap_pos, cap_pos + 1 + i, cap_pos + 2 + i]);
        }

        // ---- -X cap ----
        let cap_neg = positions.len() as u16;
        positions.push(Vec3::new(-half, 0.0, 0.0) + offset);
        uvs.push(Vec2::new(0.5, 0.5));
        shades.push(0.45);
        emissive.push(false);
        colors.push(color);
        for i in 0..=sides {
            let a = i as f32 * step;
            let (sin, cos) = a.sin_cos();
            positions.push(Vec3::new(-half, sin * radius_bottom, cos * radius_bottom) + offset);
            uvs.push(Vec2::new(0.5 + sin * 0.5, 0.5 + cos * 0.5));
            shades.push(0.45);
            emissive.push(false);
            colors.push(color);
        }
        for i in 0..sides as u16 {
            indices.extend_from_slice(&[cap_neg, cap_neg + 2 + i, cap_neg + 1 + i]);
        }

        PartMesh::from_raw(positions, uvs, shades, emissive, colors, indices)
    }

    /// Bake `world * local` into the reusable mesh and modulate by `ambient` (0..1.1).
    /// Emissive vertices ignore ambient (headlights stay bright at night).
    pub fn write_transformed(&mut self, world: &Mat4, ambient: f32) {
        let count = self.local_positions.len();
        for i in 0..count {
            let p = world.transform_point3(self.local_positions[i]);
            let c = self.base_colors[i];
            let rgb = if self.emissive[i] {
                [c[0], c[1], c[2]]
            } else {
                let k = self.shades[i] * ambient;
                [
                    (c[0] as f32 * k).min(255.0) as u8,
                    (c[1] as f32 * k).min(255.0) as u8,
                    (c[2] as f32 * k).min(255.0) as u8,
                ]
            };
            self.mesh.vertices[i].position = p;
            self.mesh.vertices[i].color = [rgb[0], rgb[1], rgb[2], c[3]];
            self.mesh.vertices[i].uv = self.uvs[i];
        }
    }

    /// Draw with the transform baked for this frame. Call after `write_transformed`.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

/// `world * local` convenience: translation-only local offset.
#[inline]
pub fn world_at(model: &Mat4, offset: Vec3) -> Mat4 {
    *model * Mat4::from_translation(offset)
}
