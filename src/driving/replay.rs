//! Authentic replay recording, playback, and stage TV camera tracking.
use crate::harwood::Camera;
use rerbr_render_core::{coords, Mat4, Quat, Vec3};

/// One recorded frame of visual car motion and performance telemetry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayFrame {
    pub time: f32,
    pub models: [Mat4; 5],
    pub driveline_distance: f32,
    pub speed_mps: f32,
    pub rpm: f32,
    pub gear: i8,
}

impl ReplayFrame {
    pub fn car_position(&self) -> Vec3 {
        self.models[0].transform_point3(Vec3::ZERO)
    }

    /// Interpolates between frame `self` and frame `other` by fraction `t` in [0.0, 1.0].
    /// Without pivots, interpolates matrix translations directly (valid for origin-centered parts).
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        self.lerp_with_pivots(other, t, None)
    }

    /// Interpolates between frame `self` and frame `other` while keeping wheel hubs centered
    /// at their car-local pivots so high-speed wheel rotation never orbits around the chassis.
    pub fn lerp_with_pivots(&self, other: &Self, t: f32, pivots: Option<[Vec3; 4]>) -> Self {
        let t = t.clamp(0.0, 1.0);
        let time = self.time + (other.time - self.time) * t;
        let driveline_distance =
            self.driveline_distance + (other.driveline_distance - self.driveline_distance) * t;
        let speed_mps = self.speed_mps + (other.speed_mps - self.speed_mps) * t;
        let rpm = self.rpm + (other.rpm - self.rpm) * t;
        let gear = if t < 0.5 { self.gear } else { other.gear };

        let mut models = [Mat4::IDENTITY; 5];
        for (slot, (model, (m0, m1))) in models
            .iter_mut()
            .zip(self.models.iter().zip(&other.models))
            .enumerate()
        {
            let q0 = Quat::from_mat4(m0).normalize();
            let q1 = Quat::from_mat4(m1).normalize();
            let q = q0.slerp(q1, t);

            if slot > 0 {
                if let Some(pivots) = pivots {
                    let pivot = pivots[slot - 1];
                    let hub0 = m0.transform_point3(pivot);
                    let hub1 = m1.transform_point3(pivot);
                    let hub = hub0.lerp(hub1, t);
                    *model = Mat4::from_translation(hub)
                        * Mat4::from_quat(q)
                        * Mat4::from_translation(-pivot);
                    continue;
                }
            }

            let p0 = m0.w_axis.truncate();
            let p1 = m1.w_axis.truncate();
            let p = p0.lerp(p1, t);
            *model = Mat4::from_rotation_translation(q, p);
        }

        Self {
            time,
            models,
            driveline_distance,
            speed_mps,
            rpm,
            gear,
        }
    }
}

/// A completed race replay holding the recorded frame stream.
#[derive(Debug, Clone, Default)]
pub struct Replay {
    pub frames: Vec<ReplayFrame>,
    pub total_duration: f32,
    pub pivots: Option<[Vec3; 4]>,
}

impl Replay {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Samples the replay stream at timestamp `time` using sub-frame interpolation.
    pub fn sample_at(&self, time: f32) -> Option<ReplayFrame> {
        self.sample_at_with_pivots(time, self.pivots)
    }

    /// Samples the replay stream using explicit fallback wheel pivots if none were saved on `Replay`.
    pub fn sample_at_with_pivots(
        &self,
        time: f32,
        fallback_pivots: Option<[Vec3; 4]>,
    ) -> Option<ReplayFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let time = time.clamp(0.0, self.total_duration);
        if time <= self.frames[0].time {
            return Some(self.frames[0]);
        }
        if time >= self.frames.last().unwrap().time {
            return Some(*self.frames.last().unwrap());
        }

        let idx = self
            .frames
            .windows(2)
            .position(|w| w[0].time <= time && time <= w[1].time)
            .unwrap_or(0);

        let f0 = &self.frames[idx];
        let f1 = &self.frames[idx + 1];
        let span = f1.time - f0.time;
        let t = if span > 1e-4 {
            (time - f0.time) / span
        } else {
            0.0
        };
        Some(f0.lerp_with_pivots(f1, t, self.pivots.or(fallback_pivots)))
    }
}

/// Lightweight frame recorder active during live stage driving.
#[derive(Debug)]
pub struct ReplayRecorder {
    frames: Vec<ReplayFrame>,
    last_record_time: f32,
    pivots: Option<[Vec3; 4]>,
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            last_record_time: -1.0,
            pivots: None,
        }
    }
}

impl ReplayRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_pivots(&mut self, pivots: [Vec3; 4]) {
        self.pivots = Some(pivots);
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.last_record_time = -1.0;
        self.pivots = None;
    }

    /// Records a frame if at least ~1/120th of a second has elapsed since the last recorded frame.
    pub fn record(
        &mut self,
        time: f32,
        models: [Mat4; 5],
        driveline_distance: f32,
        speed_mps: f32,
        rpm: f32,
        gear: i8,
    ) {
        if time < 0.0 {
            return;
        }
        // Keep sub-180 deg wheel rotation resolution even past 220 km/h on high-refresh displays.
        if self.last_record_time >= 0.0 && (time - self.last_record_time) < (1.0 / 125.0) {
            return;
        }
        self.last_record_time = time;
        self.frames.push(ReplayFrame {
            time,
            models,
            driveline_distance,
            speed_mps,
            rpm,
            gear,
        });
    }

    pub fn finish(&mut self) -> Option<Replay> {
        if self.frames.len() < 2 {
            return None;
        }
        let total_duration = self.frames.last().map_or(0.0, |f| f.time);
        Some(Replay {
            frames: std::mem::take(&mut self.frames),
            total_duration,
            pivots: self.pivots.take(),
        })
    }
}

/// One stage TV replay camera derived from `.dls` AnimData.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCamera {
    pub id: u32,
    pub pos_render: Vec3,
    pub driveline_distance: f32,
    pub fov_degrees: f32,
    pub damping: f32,
    pub mode: u32,
    pub priority: f32,
}

/// Loaded stage TV cameras sorted along the stage driveline.
#[derive(Debug, Clone, Default)]
pub struct StageCameras {
    pub cameras: Vec<StageCamera>,
}

impl StageCameras {
    /// Extracts authentic stage cameras from `.dls` Table 3 + Table 2 and correlates them
    /// to driveline arc distance from `.trk`.
    pub fn parse(
        dls_bytes: &[u8],
        driveline: Option<&rerbr_sim::rbr_formats::trk::DriveLineData>,
    ) -> Result<Self, String> {
        let dls_file = rerbr_sim::rbr_formats::dls::parse(dls_bytes).map_err(|e| e.to_string())?;
        let swap = coords::physics_to_stage();

        let mut cameras = Vec::new();

        for cam in &dls_file.table_3 {
            if let Some(trig) = dls_file.table_2.iter().find(|t| t.ids[0] == cam.id) {
                // In .dls: pos is in .trk physics coordinates [x, y, z] (z-up).
                // Converted to stage rendering coordinates [x, z, y] (y-up).
                let pos_render = swap.transform_point3(Vec3::from(trig.position));

                // Correlate with nearest driveline arc distance if driveline chunk is present
                let mut driveline_distance = 0.0f32;
                if let Some(dl) = driveline {
                    let mut min_d_sq = f32::MAX;
                    for rec in &dl.records {
                        let dx = rec.vectors[0].x - trig.position[0];
                        let dy = rec.vectors[0].y - trig.position[1];
                        let d_sq = dx * dx + dy * dy;
                        if d_sq < min_d_sq {
                            min_d_sq = d_sq;
                            driveline_distance = rec.value;
                        }
                    }
                }

                let fov = f32::from_bits(cam.words_2_3[0]);
                let fov_degrees = if fov.is_finite() && fov > 5.0 && fov < 120.0 {
                    fov
                } else {
                    50.0
                };

                let damping = f32::from_bits(cam.words_2_3[1]).clamp(0.0, 1.0);
                let priority = f32::from_bits(cam.extra).clamp(0.1, 5.0);

                cameras.push(StageCamera {
                    id: cam.id,
                    pos_render,
                    driveline_distance,
                    fov_degrees,
                    damping,
                    mode: cam.mode,
                    priority,
                });
            }
        }

        // Sort cameras monotonically along the stage
        cameras.sort_by(|a, b| a.driveline_distance.total_cmp(&b.driveline_distance));
        Ok(Self { cameras })
    }

    /// Attempts to load and parse `.dls` and `.trk` files from `maps` for the specified stage `stem`.
    pub fn load_for_stage(maps: &std::path::Path, stem: &str) -> Option<Self> {
        let entries = std::fs::read_dir(maps).ok()?;
        let mut dls_bytes = None;
        let mut trk_bytes = None;
        let dls_name = format!("{stem}.dls").to_ascii_lowercase();
        let trk_name = format!("{stem}.trk").to_ascii_lowercase();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name == dls_name {
                dls_bytes = std::fs::read(entry.path()).ok();
            } else if name == trk_name {
                trk_bytes = std::fs::read(entry.path()).ok();
            }
        }
        let dls = dls_bytes?;
        let trk = trk_bytes.and_then(|b| rerbr_sim::rbr_formats::trk::parse(&b).ok());
        let dl = trk.as_ref().and_then(|t| {
            t.chunks.iter().find_map(|c| {
                if let rerbr_sim::rbr_formats::trk::ChunkPayload::DriveLine(dl) = &c.payload {
                    Some(dl)
                } else {
                    None
                }
            })
        });
        Self::parse(&dls, dl).ok()
    }

    /// Returns the (1-based index, total count) of the currently active TV camera for `dist`.
    pub fn active_camera_index(&self, dist: f32) -> Option<(usize, usize)> {
        if self.cameras.is_empty() {
            return None;
        }
        let (idx, _) = self
            .cameras
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a.driveline_distance - dist).abs();
                let db = (b.driveline_distance - dist).abs();
                da.total_cmp(&db)
            })
            .unwrap_or((0, &self.cameras[0]));
        Some((idx + 1, self.cameras.len()))
    }

    /// Selects the optimal authentic camera for the car's current driveline distance.
    pub fn camera_for_distance(&self, dist: f32, car_render_pos: Vec3) -> Option<(Camera, f32)> {
        if self.cameras.is_empty() {
            return None;
        }

        // Choose the camera whose coverage window matches the car's progress:
        // A camera covers from halfway between the previous camera and itself, up to
        // halfway to the next camera (with slight hysteresis).
        let best_cam = self
            .cameras
            .iter()
            .min_by(|a, b| {
                let da = (a.driveline_distance - dist).abs();
                let db = (b.driveline_distance - dist).abs();
                da.total_cmp(&db)
            })
            .unwrap_or(&self.cameras[0]);

        let eye = best_cam.pos_render;
        // Aim at car centre of mass
        let target = car_render_pos + Vec3::Y * 0.75;
        let forward = (target - eye).normalize_or(Vec3::Z);
        let right = Vec3::Y.cross(forward).normalize_or(Vec3::X);
        let up = forward.cross(right).normalize_or(Vec3::Y);

        Some((Camera { eye, target, up }, best_cam.fov_degrees))
    }
}

/// Selectable camera perspective during replay playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayCameraMode {
    #[default]
    StageTV,
    Chase,
    Bonnet,
}

impl ReplayCameraMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::StageTV => Self::Chase,
            Self::Chase => Self::Bonnet,
            Self::Bonnet => Self::StageTV,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::StageTV => "TV CAM",
            Self::Chase => "CHASE CAM",
            Self::Bonnet => "BONNET CAM",
        }
    }
}

/// State of replay playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
    Finished,
}

/// Active replay player driving the simulation visualization.
#[derive(Debug, Clone, Default)]
pub struct ReplayPlayer {
    pub replay: Option<Replay>,
    pub stage_cameras: Option<StageCameras>,
    pub pivots: Option<[Vec3; 4]>,
    pub current_time: f32,
    pub state: PlaybackState,
    pub camera_mode: ReplayCameraMode,
}

impl ReplayPlayer {
    pub fn new(replay: Replay, stage_cameras: Option<StageCameras>) -> Self {
        let pivots = replay.pivots;
        Self {
            replay: Some(replay),
            stage_cameras,
            pivots,
            current_time: 0.0,
            state: PlaybackState::Playing,
            camera_mode: ReplayCameraMode::StageTV,
        }
    }

    pub fn with_pivots(
        replay: Replay,
        stage_cameras: Option<StageCameras>,
        pivots: Option<[Vec3; 4]>,
    ) -> Self {
        let pivots = replay.pivots.or(pivots);
        Self {
            replay: Some(replay),
            stage_cameras,
            pivots,
            current_time: 0.0,
            state: PlaybackState::Playing,
            camera_mode: ReplayCameraMode::StageTV,
        }
    }

    pub fn is_active(&self) -> bool {
        self.replay.is_some()
    }

    pub fn total_duration(&self) -> f32 {
        self.replay.as_ref().map_or(0.0, |r| r.total_duration)
    }

    pub fn progress(&self) -> f32 {
        let total = self.total_duration();
        if total > 0.0 {
            (self.current_time / total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn step(&mut self, dt: f32) {
        if self.state != PlaybackState::Playing {
            return;
        }
        let total = self.total_duration();
        self.current_time += dt;
        if self.current_time >= total {
            self.current_time = total;
            self.state = PlaybackState::Finished;
        }
    }

    pub fn toggle_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => self.state = PlaybackState::Paused,
            PlaybackState::Paused => self.state = PlaybackState::Playing,
            PlaybackState::Finished => {
                self.current_time = 0.0;
                self.state = PlaybackState::Playing;
            }
        }
    }

    pub fn restart(&mut self) {
        self.current_time = 0.0;
        self.state = PlaybackState::Playing;
    }

    pub fn seek(&mut self, time: f32) {
        let total = self.total_duration();
        self.current_time = time.clamp(0.0, total);
        if self.current_time < total && self.state == PlaybackState::Finished {
            self.state = PlaybackState::Playing;
        }
    }

    pub fn cycle_camera(&mut self) {
        self.camera_mode = self.camera_mode.cycle();
    }

    pub fn current_frame(&self) -> Option<ReplayFrame> {
        self.replay
            .as_ref()
            .and_then(|r| r.sample_at_with_pivots(self.current_time, self.pivots))
    }

    /// Evaluates the active replay camera perspective.
    pub fn current_camera(
        &self,
        car_pos: Vec3,
        car_model: Mat4,
        chase_camera_fn: impl Fn(Mat4) -> Camera,
        bonnet_camera_fn: impl Fn(Mat4) -> Camera,
    ) -> (Camera, f32) {
        match self.camera_mode {
            ReplayCameraMode::StageTV => {
                let dist = self.current_frame().map_or(0.0, |f| f.driveline_distance);
                if let Some((cam, fov)) = self
                    .stage_cameras
                    .as_ref()
                    .and_then(|sc| sc.camera_for_distance(dist, car_pos))
                {
                    (cam, fov)
                } else {
                    // Fall back to chase view if stage has no TV cameras
                    (chase_camera_fn(car_model), 65.0)
                }
            }
            ReplayCameraMode::Chase => (chase_camera_fn(car_model), 65.0),
            ReplayCameraMode::Bonnet => (bonnet_camera_fn(car_model), 70.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_recorder_and_interpolation() {
        let mut recorder = ReplayRecorder::new();
        let m0 = [Mat4::IDENTITY; 5];
        let mut m1 = [Mat4::IDENTITY; 5];
        m1[0] = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));

        recorder.record(0.0, m0, 0.0, 10.0, 2000.0, 1);
        recorder.record(1.0, m1, 20.0, 20.0, 4000.0, 2);

        let replay = recorder.finish().expect("finish replay");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay.total_duration, 1.0);

        // Test midpoint sample at 0.5s
        let sample = replay.sample_at(0.5).expect("sample");
        assert!((sample.time - 0.5).abs() < 1e-4);
        assert!((sample.driveline_distance - 10.0).abs() < 1e-4);
        assert!((sample.speed_mps - 15.0).abs() < 1e-4);
        assert!((sample.rpm - 3000.0).abs() < 1e-4);
        let pos = sample.car_position();
        assert!((pos.x - 5.0).abs() < 1e-4);
    }

    #[test]
    fn replay_player_state_transitions() {
        let mut recorder = ReplayRecorder::new();
        recorder.record(0.0, [Mat4::IDENTITY; 5], 0.0, 0.0, 1000.0, 1);
        recorder.record(2.0, [Mat4::IDENTITY; 5], 100.0, 50.0, 6000.0, 3);
        let replay = recorder.finish().unwrap();

        let mut player = ReplayPlayer::new(replay, None);
        assert_eq!(player.state, PlaybackState::Playing);

        player.step(1.0);
        assert_eq!(player.current_time, 1.0);
        assert_eq!(player.state, PlaybackState::Playing);

        player.toggle_pause();
        assert_eq!(player.state, PlaybackState::Paused);
        player.step(1.0);
        assert_eq!(player.current_time, 1.0); // Does not advance while paused

        player.toggle_pause();
        assert_eq!(player.state, PlaybackState::Playing);

        player.step(1.5);
        assert_eq!(player.current_time, 2.0); // Clamped to duration
        assert_eq!(player.state, PlaybackState::Finished);

        player.restart();
        assert_eq!(player.current_time, 0.0);
        assert_eq!(player.state, PlaybackState::Playing);
    }

    #[test]
    fn camera_mode_cycle() {
        let mode = ReplayCameraMode::StageTV;
        assert_eq!(mode.cycle(), ReplayCameraMode::Chase);
        assert_eq!(mode.cycle().cycle(), ReplayCameraMode::Bonnet);
        assert_eq!(mode.cycle().cycle().cycle(), ReplayCameraMode::StageTV);
    }

    #[test]
    fn high_speed_wheel_interpolation_preserves_hub_center_without_orbiting() {
        // Representative Focus 2006 / Peugeot 306 Maxi wheel pivots in car-local space.
        let pivots = [
            Vec3::new(-0.78, 0.33, -1.29),
            Vec3::new(0.78, 0.33, -1.29),
            Vec3::new(-0.78, 0.33, 1.31),
            Vec3::new(0.78, 0.33, 1.31),
        ];
        let compose_wheel = |car_pos: Vec3, hub_local: Vec3, steer: f32, roll: f32, pivot: Vec3| {
            Mat4::from_translation(car_pos + hub_local)
                * Mat4::from_rotation_y(steer)
                * Mat4::from_axis_angle(Vec3::NEG_X, roll)
                * Mat4::from_translation(-pivot)
        };

        // Simulate 50 m/s (180 km/h) across one 1/60 s frame: roll advances ~2.6 rad (~149 deg).
        let hub0 = [
            Vec3::new(-0.78, 0.33, -1.29),
            Vec3::new(0.78, 0.33, -1.29),
            Vec3::new(-0.78, 0.33, 1.31),
            Vec3::new(0.78, 0.33, 1.31),
        ];
        let hub1 = [
            Vec3::new(-0.78, 0.39, -1.29),
            Vec3::new(0.78, 0.36, -1.29),
            Vec3::new(-0.78, 0.31, 1.31),
            Vec3::new(0.78, 0.34, 1.31),
        ];
        let car0 = Vec3::new(10.0, 2.0, -50.0);
        let car1 = Vec3::new(10.2, 2.05, -50.833);
        let roll0 = 0.2_f32;
        let roll1 = 2.8_f32;
        let steer0 = 0.15_f32;
        let steer1 = 0.22_f32;

        let m0 = [
            Mat4::from_translation(car0),
            compose_wheel(car0, hub0[0], steer0, roll0, pivots[0]),
            compose_wheel(car0, hub0[1], steer0, roll0, pivots[1]),
            compose_wheel(car0, hub0[2], 0.0, roll0, pivots[2]),
            compose_wheel(car0, hub0[3], 0.0, roll0, pivots[3]),
        ];
        let m1 = [
            Mat4::from_translation(car1),
            compose_wheel(car1, hub1[0], steer1, roll1, pivots[0]),
            compose_wheel(car1, hub1[1], steer1, roll1, pivots[1]),
            compose_wheel(car1, hub1[2], 0.0, roll1, pivots[2]),
            compose_wheel(car1, hub1[3], 0.0, roll1, pivots[3]),
        ];

        let mut recorder = ReplayRecorder::new();
        recorder.set_pivots(pivots);
        recorder.record(0.0, m0, 0.0, 50.0, 6500.0, 5);
        recorder.record(1.0 / 60.0, m1, 0.833, 50.0, 6500.0, 5);
        let replay = recorder.finish().expect("finish high-speed replay");
        let player = ReplayPlayer::new(replay, None);

        // Verify at t = 0.5 (mid-frame) that wheel hubs lie on the exact straight chord between frames.
        let mut p = player;
        p.seek(0.5 / 60.0);
        let mid = p.current_frame().expect("mid frame");
        for k in 0..4 {
            let expected_hub = (car0 + hub0[k]).lerp(car1 + hub1[k], 0.5);
            let actual_hub = mid.models[k + 1].transform_point3(pivots[k]);
            let err = actual_hub.distance(expected_hub);
            assert!(
                err < 1e-4,
                "wheel {k} hub drifted/orbited by {err} m at high speed"
            );
            // Also verify rotation continues spinning smoothly (distinct from both endpoints).
            let q_mid = Quat::from_mat4(&mid.models[k + 1]).normalize();
            let q0 = Quat::from_mat4(&m0[k + 1]).normalize();
            let q1 = Quat::from_mat4(&m1[k + 1]).normalize();
            assert!(q_mid.angle_between(q0) > 0.5);
            assert!(q_mid.angle_between(q1) > 0.5);
        }
    }
}
