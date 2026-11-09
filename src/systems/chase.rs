//! Chase camera with speed-reactive FOV and smoothed follow.
//!
//! Sits high and behind the war rig, leads the view down-road, widens FOV with
//! speed (arcade sense of velocity) and adds a whisper of low-frequency sway
//! so the frame never feels static. Tilt-shift flavour is faked in 2D
//! (letterbox + gradient bands) inside `ui::hud`.

use macroquad::prelude::*;

pub struct ChaseCamera {
    pub position: Vec3,
    pub target: Vec3,
    fovy_deg: f32,
    initialized: bool,
    sway_phase: f32,
}

impl ChaseCamera {
    pub fn new() -> ChaseCamera {
        ChaseCamera {
            position: vec3(0.0, 6.0, -12.0),
            target: Vec3::ZERO,
            fovy_deg: 55.0,
            initialized: false,
            sway_phase: 0.0,
        }
    }

    pub fn update(&mut self, truck_pos: Vec3, forward: Vec3, steer: f32, speed_norm: f32, dt: f32) {
        let right = Vec3::new(-forward.z, 0.0, forward.x); // right = forward × up (Y-up)
        let back = forward * -1.0;

        // desired framing: 11.5 m back, 5.3 m up, slight counter-lean into steer
        let desired_pos = truck_pos
            + back * 11.5
            + vec3(0.0, 5.3, 0.0)
            + right * (steer * -1.6);
        let desired_target = truck_pos + forward * 6.5 + vec3(0.0, 1.1, 0.0);

        if !self.initialized {
            self.position = desired_pos;
            self.target = desired_target;
            self.initialized = true;
            return;
        }

        // framerate-independent exponential smoothing
        let pos_blend = 1.0 - (-dt * 4.2).exp();
        let tgt_blend = 1.0 - (-dt * 6.5).exp();
        self.position += (desired_pos - self.position) * pos_blend;
        self.target += (desired_target - self.target) * tgt_blend;

        // speed-reactive FOV: 55° planted → 68° flat out
        self.fovy_deg = 55.0 + speed_norm.clamp(0.0, 1.0) * 13.0;

        self.sway_phase += dt * (1.4 + speed_norm * 2.6);
    }

    /// Low-amplitude breathing sway, applied at render/build time.
    fn sway(&self, speed_norm: f32) -> Vec3 {
        let amp = 0.06 + speed_norm * 0.10;
        vec3(
            self.sway_phase.sin() * amp,
            (self.sway_phase * 1.7 + 1.3).sin() * amp * 0.6,
            0.0,
        )
    }

    pub fn to_camera3d(&self, speed_norm: f32) -> Camera3D {
        let sway = self.sway(speed_norm);
        Camera3D {
            position: self.position + sway,
            target: self.target + sway * 0.4,
            up: vec3(0.0, 1.0, 0.0),
            fovy: self.fovy_deg.to_radians(),
            z_near: 0.5,
            z_far: 420.0,
            ..Default::default()
        }
    }
}
