//! Independent chase benchmark. Reuses stage-4 clock, metrics and original HUD.
use super::{
    gpu::Gpu,
    mesh::Stats,
    placement::{ChasePath, Frame},
};
use crate::harwood::{
    benchmark::{
        hud::Hud,
        metrics::{Session, State},
    },
    Camera,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct Comparison {
    pub car_visible: bool,
}
impl Default for Comparison {
    fn default() -> Self {
        Self { car_visible: true }
    }
}
impl Comparison {
    #[cfg(any(not(feature = "physics-drive"), test))]
    pub fn restart(&mut self, complete: bool) {
        if complete {
            self.car_visible = !self.car_visible;
        }
    }
    pub fn label(self) -> &'static str {
        if self.car_visible {
            "B TERRAIN AND COROLLA"
        } else {
            "A TERRAIN ONLY"
        }
    }
}
pub(crate) struct Runtime {
    #[cfg(feature = "physics-drive")]
    pub(crate) driving: crate::driving::Runtime,
    origin: Instant,
    path: Option<ChasePath>,
    session: Option<Session>,
    car: Option<Gpu>,
    hud: Option<Hud>,
    hud_failed: bool,
    frame: Option<Frame>,
    comparison: Comparison,
    terrain: Stats,
    hud_vertices: usize,
    fatal: bool,
    reason: &'static str,
}
pub(crate) struct Uploaded {
    selection: rerbr_content::Selection,
    path: ChasePath,
    car: Gpu,
    terrain: Stats,
    #[cfg(feature = "physics-drive")]
    pivots: [rerbr_render_core::Vec3; 4],
    #[cfg(feature = "physics-stage")]
    mounts: crate::driving::cameras::Mounts,
    #[cfg(feature = "physics-stage")]
    cockpit: Option<rerbr_render_core::Mat4>,
    #[cfg(feature = "physics-stage")]
    cockpit_error: Option<String>,
    #[cfg(feature = "physics-stage")]
    stage_cameras: Option<crate::driving::replay::StageCameras>,
}
impl Runtime {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "physics-drive")]
            driving: crate::driving::Runtime::new(),
            origin: Instant::now(),
            path: None,
            session: None,
            car: None,
            hud: None,
            hud_failed: false,
            frame: None,
            comparison: Comparison::default(),
            terrain: Stats::default(),
            hud_vertices: 0,
            fatal: false,
            reason: "",
        }
    }
    pub(crate) fn upload_prepared(
        prepared: super::Prepared,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<(Uploaded, crate::harwood::LoadedBlock), String> {
        let super::Prepared {
            path,
            car,
            selection,
            terrain,
            stage_cameras,
        } = prepared;
        selection.validate_current_backend()?;
        if car.vehicle != selection.vehicle {
            return Err("selected vehicle does not match prepared geometry".into());
        }
        let stats = Stats {
            vertices: terrain.mesh.vertices.len(),
            triangles: terrain.mesh.indices.len() / 3,
            draws: terrain.mesh.draws.len(),
            textures: terrain.textures.len(),
        };
        #[cfg(feature = "physics-drive")]
        let pivots = car.geometry.pivots;
        #[cfg(feature = "physics-stage")]
        let (mounts, cockpit, cockpit_error) =
            (car.cameras, car.cockpit, car.cockpit_error.clone());
        let car = Gpu::new(device, queue, format, car)?;
        Ok((
            Uploaded {
                selection,
                path,
                car,
                terrain: stats,
                #[cfg(feature = "physics-drive")]
                pivots,
                #[cfg(feature = "physics-stage")]
                mounts,
                #[cfg(feature = "physics-stage")]
                cockpit,
                #[cfg(feature = "physics-stage")]
                cockpit_error,
                #[cfg(feature = "physics-stage")]
                stage_cameras,
            },
            terrain,
        ))
    }
    pub(crate) fn install_uploaded(&mut self, uploaded: Uploaded) -> Result<(), String> {
        uploaded.selection.validate_current_backend()?;
        #[cfg(feature = "physics-drive")]
        {
            if self.driving.selection() != &uploaded.selection {
                return Err("stale scene selection".into());
            }
            self.driving.install_pivots(uploaded.pivots);
        }
        #[cfg(feature = "physics-stage")]
        {
            self.driving.mounts = Some(uploaded.mounts);
            self.driving.cockpit_ready = uploaded.cockpit.is_some();
            self.driving.cockpit_node = uploaded.cockpit;
            self.driving.cockpit_error = uploaded.cockpit_error;
            self.driving.stage_cameras = uploaded.stage_cameras;
        }
        self.car = Some(uploaded.car);
        self.path = Some(uploaded.path);
        self.terrain = uploaded.terrain;
        self.session = None;
        self.frame = None;
        self.fatal = false;
        self.reason = "";
        Ok(())
    }
    #[cfg(not(feature = "physics-drive"))]
    pub fn install(
        &mut self,
        prepared: super::Prepared,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<crate::harwood::LoadedBlock, String> {
        let (uploaded, terrain) = Self::upload_prepared(prepared, device, queue, format)?;
        self.install_uploaded(uploaded)?;
        Ok(terrain)
    }
    #[cfg(feature = "physics-drive")]
    pub fn release_scene(&mut self) {
        self.car = None;
        self.path = None;
        self.frame = None;
        self.session = None;
        self.terrain = Stats::default();
        self.fatal = false;
        self.reason = "";
    }
    pub fn fail(&mut self) {
        #[cfg(feature = "physics-drive")]
        self.driving.pause();
        self.fatal = true;
        if let Some(s) = &mut self.session {
            s.abort();
        }
    }
    #[cfg(feature = "physics-drive")]
    pub fn asset_error(&mut self, error: String) {
        self.driving.asset_error(error);
    }
    #[cfg(any(not(feature = "physics-drive"), test))]
    pub fn restart(&mut self) {
        if self.fatal {
            return;
        }
        if self.path.is_some() {
            self.comparison.restart(
                self.session
                    .as_ref()
                    .is_some_and(|s| s.state() == State::Complete),
            );
            self.session = Some(Session::new(self.origin.elapsed().as_secs_f64()));
            self.reason = "";
            eprintln!(
                "car-chase RESTART: {}, cached resources and identical camera path; 2s warmup",
                self.comparison.label()
            );
        }
    }
    pub fn interrupt(&mut self, reason: &'static str) {
        #[cfg(feature = "physics-drive")]
        self.driving.pause();
        if let Some(s) = &mut self.session {
            if matches!(s.state(), State::Warmup | State::Running) {
                s.abort();
                self.reason = reason;
                eprintln!("car-chase ABORTED: {reason}");
            }
        }
    }
    pub fn before_frame(&mut self, camera: &mut Camera) -> Result<(), String> {
        #[cfg(feature = "physics-drive")]
        {
            if let Some(path) = &self.path {
                if let Some(frame) = self.driving.frame(path) {
                    *camera = frame.camera;
                    self.frame = Some(frame);
                }
            }
            Ok(())
        }
        #[cfg(not(feature = "physics-drive"))]
        {
            let Some(path) = &self.path else {
                return Ok(());
            };
            let now = self.origin.elapsed().as_secs_f64();
            let s = self.session.get_or_insert_with(|| Session::new(now));
            let previous = s.state();
            s.begin_frame(now);
            if previous == State::Warmup && s.state() == State::Running {
                eprintln!("car-chase START: {}; 20 real seconds, {:.3}m/s; CPU preparation separate; GPU timing unavailable; unchanged camera/ground work in A and B",self.comparison.label(),path.length()/20.0);
            }
            if s.state() != State::Aborted {
                let frame = path.frame(s.distance(path.length()))?;
                *camera = frame.camera;
                self.frame = Some(frame);
            }
            Ok(())
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw_car(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        #[cfg(feature = "physics-stage")]
        let interior = self.driving.view == crate::driving::cameras::View::CockpitFallback;
        #[cfg(not(feature = "physics-stage"))]
        let interior = false;
        #[cfg(feature = "physics-drive")]
        let steering = self.driving.steering_angle;
        #[cfg(not(feature = "physics-drive"))]
        let steering = 0.0;
        #[cfg(feature = "physics-drive")]
        let fov = Some(self.driving.camera_fov_degrees());
        #[cfg(not(feature = "physics-drive"))]
        let fov = None;
        if self.comparison.car_visible && !self.fatal {
            if let (Some(gpu), Some(frame)) = (&self.car, &self.frame) {
                gpu.draw_with_fov(
                    queue, encoder, target, depth, size, format, frame, interior, steering, fov,
                );
            }
        }
    }
    pub fn submitted(&mut self, cpu: Duration) {
        #[cfg(feature = "physics-drive")]
        self.driving.submitted(cpu);
        if let Some(s) = &mut self.session {
            if s.submitted(self.origin.elapsed().as_secs_f64(), cpu.as_secs_f64()) {
                eprintln!(
                    "car-chase SUMMARY {}: {}",
                    self.comparison.label(),
                    s.summary().unwrap()
                );
                let (opaque, transparent) = self.submitted_stats();
                eprintln!("car-chase COUNTERS: terrain={:?}; car opaque={opaque:?}; car transparent={transparent:?}; HUD vertices={}, triangles={}, draws=1, atlas=1; car textures stay allocated in A",self.terrain,self.hud_vertices,self.hud_vertices/3);
            }
        }
    }
    fn submitted_stats(&self) -> (Stats, Stats) {
        if self.comparison.car_visible {
            self.car
                .as_ref()
                .map_or((Stats::default(), Stats::default()), |g| {
                    (g.opaque, g.transparent)
                })
        } else {
            (Stats::default(), Stats::default())
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        if self.hud.is_none() && !self.hud_failed {
            match Hud::new(device, queue, format) {
                Ok(h) => self.hud = Some(h),
                Err(e) => {
                    self.hud_failed = true;
                    self.fail();
                    eprintln!("car-chase HUD: {e}");
                }
            }
        }
        if let Some(hud) = &self.hud {
            #[cfg(feature = "physics-drive")]
            {
                self.driving.overlay(hud, queue, encoder, target, size);
            }
            #[cfg(not(feature = "physics-drive"))]
            {
                self.hud_vertices = hud.draw(queue, encoder, target, size, &self.lines());
            }
        }
    }
    #[cfg(not(feature = "physics-drive"))]
    fn lines(&self) -> Vec<String> {
        let mut l = vec![format!("CAR CHASE {}", self.comparison.label())];
        if self.fatal {
            l.extend([
                "ERROR - SEE DOCUMENTS LOG".into(),
                "CORRECT PRIVATE DATA AND RELAUNCH".into(),
            ]);
            return l;
        }
        let Some(s) = &self.session else {
            l.push("LOADING ORIGINAL COROLLA AND HARWOOD".into());
            return l;
        };
        let length = self.path.as_ref().map_or(0.0, ChasePath::length);
        match s.state() {
            State::Complete => {
                let r = s.summary().unwrap();
                l.extend([
                    format!(
                        "AVG {:.1} MIN {:.0} MAX {:.0} FPS",
                        r.average_fps, r.min_window_fps, r.max_window_fps
                    ),
                    format!(
                        "FRAME {:.2} P95 {:.2} P99 {:.2} MS",
                        r.mean_ms, r.p95_ms, r.p99_ms
                    ),
                    format!("CPU {:.2} P95 {:.2} MS", r.cpu_mean_ms, r.cpu_p95_ms),
                    format!("{} FRAMES {:.4} S {:.1} M", r.frames, r.duration, length),
                ]);
            }
            State::Running => l.extend([
                format!(
                    "RUN FPS {:.1} TIME {:.2}/20 S",
                    s.current_fps(),
                    s.elapsed().min(20.0)
                ),
                format!("DIST {:.1}/{:.1} M", s.distance(length), length),
                format!("CPU PREP {:.2} MS GPU N/A", s.last_cpu_ms()),
            ]),
            State::Warmup => l.push("WARMUP 2S THEN AUTOMATIC 20S RUN".into()),
            State::Aborted => l.extend(["ABORTED - NO VALID RESULT".into(), self.reason.into()]),
        }
        let (o, t) = self.submitted_stats();
        l.push(stat_line("TRN", self.terrain));
        l.push(stat_line("OPA", o));
        l.push(stat_line("TRA", t));
        l.push(format!(
            "HUD PREV V{} T{} D1 X1",
            self.hud_vertices,
            self.hud_vertices / 3
        ));
        l.push("GPU N/A - FIFO - HUD INCLUDED".into());
        l.push(
            if s.state() == State::Complete {
                "TAP: RESTART WITH OTHER A/B VIEW"
            } else {
                "TAP: RESTART SAME A/B VIEW"
            }
            .into(),
        );
        l
    }
}
#[cfg(not(feature = "physics-drive"))]
fn stat_line(name: &str, s: Stats) -> String {
    format!(
        "{name} V{} T{} D{} X{}",
        s.vertices, s.triangles, s.draws, s.textures
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_restart_switches_ab_and_clears_metrics_without_reloading_path() {
        let mut r = Runtime::new();
        r.path = Some(crate::car::tests::path_fixture(0.0));
        let mut s = Session::new(0.0);
        s.begin_frame(2.0);
        assert!(s.submitted(22.0, 0.001));
        r.session = Some(s);
        r.restart();
        assert!(!r.comparison.car_visible);
        assert_eq!(r.session.as_ref().unwrap().state(), State::Warmup);
        assert!(r.session.as_ref().unwrap().summary().is_none());
        r.interrupt("TEST");
        r.restart();
        assert!(!r.comparison.car_visible);
        assert!(r.path.is_some());
    }
}
