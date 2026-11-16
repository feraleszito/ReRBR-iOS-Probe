//! Partial-stage driving presentation. Physics owns every vehicle pose.
pub mod controls;
#[cfg(test)]
mod private_tests;
mod worker;
use crate::{
    car::{
        mesh::Geometry,
        placement::{ChasePath, Frame},
    },
    harwood::{
        benchmark::hud::{vertices, Hud},
        Camera,
    },
};
use controls::{Action, Controls};
use rerbr_render_core::{coords, Mat4, Quat, Vec3};
use rerbr_sim::diagnostics::Sample;
use std::time::{Duration, Instant};
use winit::event::{Touch, TouchPhase};

pub struct Runtime {
    worker: worker::Worker,
    pub controls: Controls,
    pivots: Option<[Vec3; 4]>,
    offset: Option<Vec3>,
    last: Instant,
    tilt: f64,
    camera: Option<Camera>,
    fps: f64,
    cpu_ms: f64,
    footprint: Option<u64>,
    footprint_at: Instant,
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
impl Runtime {
    pub fn new() -> Self {
        Self {
            worker: worker::Worker::new(),
            controls: Controls::new(),
            pivots: None,
            offset: None,
            last: Instant::now(),
            tilt: f64::NAN,
            camera: None,
            fps: 0.0,
            cpu_ms: 0.0,
            footprint: None,
            footprint_at: Instant::now(),
        }
    }
    pub fn install(&mut self, geometry: &Geometry) {
        self.pivots = Some(geometry.pivots);
    }
    pub fn pause(&mut self) {
        self.controls.clear();
        self.worker.state.lock().unwrap().paused = true;
        motion(false);
    }
    pub fn touch(&mut self, t: Touch, size: (u32, u32)) {
        let action = self.controls.touch(
            t.id,
            t.location.x / f64::from(size.0.max(1)),
            t.location.y / f64::from(size.1.max(1)),
            t.phase == TouchPhase::Started,
            matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
        );
        match action {
            Some(Action::Pause) => {
                let mut s = self.worker.state.lock().unwrap();
                if self.pivots.is_some() && self.tilt.is_finite() && s.error.is_none() {
                    s.paused = !s.paused;
                }
                self.controls.clear();
            }
            Some(Action::Calibrate) => {
                if self.tilt.is_finite() {
                    self.controls.centre = self.tilt;
                    self.controls.clear();
                }
            }
            Some(Action::Restart) => {
                if self.worker.state.lock().unwrap().rest.is_some() {
                    self.worker = worker::Worker::new();
                    self.offset = None;
                    self.camera = None;
                    self.controls.clear();
                }
            }
            Some(Action::SensDown) => {
                self.controls.sensitivity = (self.controls.sensitivity - 0.25).max(0.25)
            }
            Some(Action::SensUp) => {
                self.controls.sensitivity = (self.controls.sensitivity + 0.25).min(5.0)
            }
            Some(Action::DeadDown) => {
                self.controls.dead_zone = (self.controls.dead_zone - 0.01).max(0.0)
            }
            Some(Action::DeadUp) => {
                self.controls.dead_zone = (self.controls.dead_zone + 0.01).min(0.2)
            }
            None => (),
        }
    }
    pub fn frame(&mut self, path: &ChasePath) -> Option<Frame> {
        let now = Instant::now();
        let seconds = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        if seconds > 0.0 {
            self.fps = self.fps * 0.9 + 0.1 / seconds;
        }
        self.tilt = motion(true);
        if now >= self.footprint_at {
            self.footprint = rerbr_sim::footprint();
            self.footprint_at = now + Duration::from_secs(1);
        }
        let (pair, rest, paused) = {
            let mut s = self.worker.state.lock().unwrap();
            if !self.tilt.is_finite() {
                s.paused = true;
                self.controls.clear();
            }
            s.input = self.controls.input(self.tilt, seconds);
            (s.pair.clone(), s.rest.clone(), s.paused)
        };
        let ((a, b, stamp), rest, pivots) = (pair?, rest?, self.pivots?);
        let swap = coords::physics_to_stage();
        let offset = *self.offset.get_or_insert_with(|| {
            (0..4)
                .map(|k| Vec3::from(rest.wheel_centre[k]) - swap.transform_point3(pivots[k]))
                .sum::<Vec3>()
                * 0.25
        });
        let alpha = if paused {
            1.0
        } else {
            (stamp.elapsed().as_secs_f64() * 720.0).clamp(0.0, 1.0) as f32
        };
        let models = models(&a, &b, alpha, pivots, offset);
        let position =
            swap.transform_point3(Vec3::from(a.position).lerp(Vec3::from(b.position), alpha));
        let forward =
            Vec3::new(-models[0].z_axis.x, 0.0, -models[0].z_axis.z).normalize_or(Vec3::NEG_Z);
        let forward = self.camera.map_or(forward, |c| {
            Vec3::new(c.target.x - c.eye.x, 0.0, c.target.z - c.eye.z)
                .normalize_or(forward)
                .lerp(forward, (1.0 - (-seconds / 0.1).exp()) as f32)
                .normalize_or(forward)
        });
        if !path.supports_driving(position) {
            let mut s = self.worker.state.lock().unwrap();
            s.paused = true;
            s.error = Some("VISIBLE TERRAIN LIMIT - RESET STAGE".into());
            return None;
        }
        let target = position + Vec3::Y * 0.5;
        let target = self.camera.map_or(target, |c| {
            c.target
                .lerp(target, (1.0 - (-seconds / 0.06).exp()) as f32)
        });
        match path.chase_camera(target, forward) {
            Ok((camera, boom)) => {
                self.camera = Some(camera);
                Some(Frame {
                    models,
                    camera,
                    boom,
                    max_wheel_travel: b.wheel_travel.iter().map(|x| x.abs()).fold(0.0, f32::max),
                    min_wheel_clearance: 0.0,
                })
            }
            Err(e) => {
                let mut s = self.worker.state.lock().unwrap();
                s.paused = true;
                s.error = Some(format!("PARTIAL STAGE CAMERA: {e}; RESET"));
                None
            }
        }
    }
    pub fn submitted(&mut self, cpu: Duration) {
        self.cpu_ms = cpu.as_secs_f64() * 1000.0;
    }
    pub fn overlay(
        &self,
        hud: &Hud,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let s = self.worker.state.lock().unwrap();
        let mut lines = vec![
            "PHYSICS DRIVE - PARTIAL STAGE - FIRST GEAR".into(),
            format!(
                "{} FPS {:.1} CPU {:.2} MS",
                if s.paused { "PAUSED" } else { "RUNNING" },
                self.fps,
                self.cpu_ms
            ),
        ];
        if let Some((_, b, _)) = &s.pair {
            lines.push(format!(
                "{:.1} KM/H TIME {:.1} S RPM {:.0}",
                b.speed() * 3.6,
                f64::from(b.step) / 720.0,
                b.rpm
            ));
        }
        lines.push(format!(
            "SENS {:.2} DEAD {:.2} TILT {}",
            self.controls.sensitivity,
            self.controls.dead_zone,
            if self.tilt.is_finite() {
                "OK"
            } else {
                "UNAVAILABLE"
            }
        ));
        lines.push(format!(
            "PAGES {:.1} MIB GPU TIME N/A",
            s.payload_bytes as f64 / 1048576.0
        ));
        lines.push(format!("PHYS STEP+READ {:.3} MS", s.last_step_ms));
        if let Some(bytes) = self.footprint {
            lines.push(format!("APP FOOTPRINT {:.1} MIB", bytes as f64 / 1048576.0));
        }
        if let Some(e) = &s.error {
            lines.push(e.clone());
        } else if s.rest.is_none() {
            lines.push("LOADING PHYSICS - PLEASE WAIT".into());
        }
        let mut bytes = region(&lines, size, [0.02, 0.29, 0.96, 0.27]);
        for (text, r) in [
            ("PAUSE / GO", [0.01, 0.02, 0.18, 0.11]),
            ("CALIBRATE", [0.21, 0.02, 0.18, 0.11]),
            ("RESET STAGE", [0.41, 0.02, 0.18, 0.11]),
            ("SENS -", [0.61, 0.02, 0.18, 0.11]),
            ("SENS +", [0.81, 0.02, 0.18, 0.11]),
            ("DEAD -", [0.66, 0.17, 0.15, 0.09]),
            ("DEAD +", [0.83, 0.17, 0.15, 0.09]),
            ("BRAKE", [0.02, 0.65, 0.38, 0.31]),
            ("HB", [0.43, 0.65, 0.14, 0.31]),
            ("THROTTLE", [0.60, 0.65, 0.38, 0.31]),
        ] {
            bytes.extend(region(&[text.into()], size, r));
        }
        hud.draw_bytes(queue, encoder, target, &bytes);
    }
}
/// Same mirror/pose/model composition as reRBR playback/frames.rs and playback::model_to_stage.
pub fn models(a: &Sample, b: &Sample, t: f32, pivots: [Vec3; 4], offset: Vec3) -> [Mat4; 5] {
    let t = t.clamp(0.0, 1.0);
    let swap = coords::physics_to_stage();
    let q = Quat::from_array(a.orientation)
        .normalize()
        .slerp(Quat::from_array(b.orientation).normalize(), t);
    let car = swap
        * Mat4::from_rotation_translation(
            q,
            Vec3::from(a.position).lerp(Vec3::from(b.position), t),
        );
    std::array::from_fn(|i| {
        if i == 0 {
            car * Mat4::from_translation(offset) * swap
        } else {
            let k = i - 1;
            let roll = a.wheel_roll[k]
                + ((b.wheel_roll[k] - a.wheel_roll[k] + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI)
                    * t;
            car * Mat4::from_translation(
                Vec3::from(a.wheel_centre[k]).lerp(Vec3::from(b.wheel_centre[k]), t),
            ) * swap
                * Mat4::from_axis_angle(Vec3::NEG_X, roll)
                * Mat4::from_translation(-pivots[k])
        }
    })
}
fn region(lines: &[String], size: (u32, u32), r: [f32; 4]) -> Vec<u8> {
    let mut bytes = vertices(lines, size);
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for v in bytes.chunks_exact(20) {
        for k in 0..2 {
            let n = f32::from_ne_bytes(v[k * 4..k * 4 + 4].try_into().unwrap());
            min[k] = min[k].min(n);
            max[k] = max[k].max(n);
        }
    }
    let h = 640.0 * size.1.max(1) as f32 / size.0.max(1) as f32;
    let width = (lines
        .iter()
        .take(12)
        .map(|l| l.len().min(42))
        .max()
        .unwrap_or(1) as f32
        * 14.0
        + 40.0)
        .max(40.0);
    let height = lines.len().min(12) as f32 * 20.0 + 16.0;
    let scale = (r[2] * 640.0 / width).min(r[3] * h / height);
    for (index, v) in bytes.chunks_exact_mut(20).enumerate() {
        let x = f32::from_ne_bytes(v[0..4].try_into().unwrap());
        let y = f32::from_ne_bytes(v[4..8].try_into().unwrap());
        let (x, y) = if index < 6 {
            (
                -1.0 + 2.0 * r[0] + (x - min[0]) / (max[0] - min[0]) * 2.0 * r[2],
                1.0 - 2.0 * r[1] - (max[1] - y) / (max[1] - min[1]) * 2.0 * r[3],
            )
        } else {
            let px = r[0] * 640.0
                + (r[2] * 640.0 - width * scale) * 0.5
                + ((x + 1.0) * 320.0 - 10.0) * scale;
            let py =
                r[1] * h + (r[3] * h - height * scale) * 0.5 + ((1.0 - y) * h * 0.5 - 24.0) * scale;
            (px / 320.0 - 1.0, 1.0 - py * 2.0 / h)
        };
        v[0..4].copy_from_slice(&x.to_ne_bytes());
        v[4..8].copy_from_slice(&y.to_ne_bytes());
    }
    bytes
}
fn motion(enabled: bool) -> f64 {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn rerbr_drive_tilt(enabled: i32) -> f64;
        }
        unsafe { rerbr_drive_tilt(i32::from(enabled)) }
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = enabled;
        f64::NAN
    }
}
