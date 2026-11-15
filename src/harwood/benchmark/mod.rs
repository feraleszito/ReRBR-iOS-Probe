//! App-owned automatic graphics benchmark; no simulation or game data embedded.
mod hud;
pub mod metrics;
pub mod route;

use super::{neighbors, LoadedBlock};
use metrics::{Session, State};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub struct Prepared {
    pub render: LoadedBlock,
    pub route: route::Route,
}

pub fn load(maps: &Path) -> Result<Prepared, String> {
    let set = neighbors::load(maps)?;
    if set.report.ids.len() != 20 || set.report.covered_samples != set.report.route_samples {
        return Err("BENCHMARK: needs the full 20-block workload and covered real route; no artificial bridge allowed".into());
    }
    let route = route::Route::new(&set.route_points, &set.render.mesh, &set.road_triangles)?;
    eprintln!("harwood-benchmark ROUTE PASS: {} original points, {:.2} m camera path, {:.2} m/s over 20 real seconds; checked clearance {:.3}..{:.3} m; no route/mesh data written",set.route_points.len(),route.length(),route.length()/20.0,route.minimum_clearance(),route.maximum_clearance());
    eprintln!("harwood-benchmark WORKLOAD: {} blocks, {} vertices, {} triangles, {} terrain draws + 1 HUD draw per frame, {} unique DDS; unchanged textured terrain pipeline",set.report.ids.len(),set.report.vertices,set.report.triangles,set.report.draws,set.report.unique_textures);
    Ok(Prepared {
        render: set.render,
        route,
    })
}

pub(crate) struct Runtime {
    origin: Instant,
    route: Option<route::Route>,
    session: Option<Session>,
    hud: Option<hud::Hud>,
    hud_failed: bool,
    fatal: bool,
    reason: &'static str,
}
impl Runtime {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            route: None,
            session: None,
            hud: None,
            hud_failed: false,
            fatal: false,
            reason: "",
        }
    }
    pub fn install(&mut self, route: route::Route) {
        self.route = Some(route);
        self.session = None;
    }
    pub fn fail(&mut self) {
        self.fatal = true;
        if let Some(session) = &mut self.session {
            session.abort();
        }
    }
    pub fn restart(&mut self) {
        if self.fatal {
            eprintln!("harwood-benchmark RESTART: load/GPU error requires relaunch; see log");
            return;
        }
        if self.route.is_some() {
            self.session = Some(Session::new(self.origin.elapsed().as_secs_f64()));
            self.reason = "";
            eprintln!("harwood-benchmark RESTART: cached workload retained; 2s warmup then 20s real-time run");
        }
    }
    pub fn interrupt(&mut self, reason: &'static str) {
        if let Some(session) = &mut self.session {
            if matches!(session.state(), State::Running | State::Warmup) {
                session.abort();
                self.reason = reason;
                eprintln!("harwood-benchmark ABORTED: {reason}; no valid result; tap to restart");
            }
        }
    }
    pub fn before_frame(&mut self, camera: &mut super::Camera) -> Result<(), String> {
        let Some(route) = &self.route else {
            return Ok(());
        };
        let now = self.origin.elapsed().as_secs_f64();
        let session = self.session.get_or_insert_with(|| Session::new(now));
        let before = session.state();
        session.begin_frame(now);
        if before == State::Warmup && session.state() == State::Running {
            eprintln!("harwood-benchmark START: 20 real seconds, {:.3}m/s; warmup/uploads excluded, FIFO/HUD included; CPU prep excludes acquisition/submit/present waits; GPU timing unavailable",route.length()/metrics::DURATION as f32);
        }
        if session.state() != State::Aborted {
            *camera = route.camera(session.distance(route.length()))?;
        }
        Ok(())
    }
    pub fn submitted(&mut self, cpu: Duration) {
        if let Some(session) = &mut self.session {
            if session.submitted(self.origin.elapsed().as_secs_f64(), cpu.as_secs_f64()) {
                eprintln!("harwood-benchmark SUMMARY: {}", session.summary().unwrap());
            }
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
            match hud::Hud::new(device, queue, format) {
                Ok(hud) => self.hud = Some(hud),
                Err(error) => {
                    self.hud_failed = true;
                    self.fail();
                    eprintln!("harwood-benchmark {error}");
                }
            }
        }
        if let Some(hud) = &self.hud {
            hud.draw(queue, encoder, target, size, &self.lines());
        }
    }
    fn lines(&self) -> Vec<String> {
        let mut lines = vec!["RERBR HARWOOD BENCHMARK".into()];
        if self.fatal {
            lines.extend([
                "ERROR - CHECK DOCUMENTS LOG".into(),
                "ASSET/GPU ERROR REQUIRES RELAUNCH".into(),
            ]);
            return lines;
        }
        let Some(session) = &self.session else {
            lines.push("LOADING PRIVATE ROAD AND DDS...".into());
            return lines;
        };
        let length = self.route.as_ref().map_or(0.0, route::Route::length);
        match session.state() {
            State::Complete => {
                let s = session.summary().unwrap();
                lines.extend([
                    "COMPLETE - TAP TO RESTART".into(),
                    format!("AVG FPS {:.1}  FRAMES {}", s.average_fps, s.frames),
                    format!(
                        "1S FPS MIN {:.1} MAX {:.1}",
                        s.min_window_fps, s.max_window_fps
                    ),
                    format!("FRAME MEAN {:.2} MS", s.mean_ms),
                    format!("FRAME P95 {:.2} P99 {:.2} MS", s.p95_ms, s.p99_ms),
                    format!(
                        "CPU PREP MEAN {:.2} P95 {:.2} MS",
                        s.cpu_mean_ms, s.cpu_p95_ms
                    ),
                    format!("REAL {:.4} S  DIST {:.1} M", s.duration, length),
                ]);
            }
            State::Aborted => lines.extend([
                "ABORTED - TAP TO RESTART".into(),
                self.reason.into(),
                "NO VALID BENCHMARK RESULT".into(),
            ]),
            State::Warmup => lines.extend([
                "WARMUP 2S - THEN AUTO START".into(),
                "TAP RESTARTS WARMUP AND METRICS".into(),
            ]),
            State::Running => lines.extend([
                format!("RUNNING  FPS {:.1}", session.current_fps()),
                format!(
                    "TIME {:.2}/20.00 S",
                    session.elapsed().min(metrics::DURATION)
                ),
                format!("DIST {:.1}/{:.1} M", session.distance(length), length),
                format!("CPU PREP LAST {:.2} MS", session.last_cpu_ms()),
                "TAP TO RESTART".into(),
            ]),
        }
        lines.push("GPU TIME N/A - NO TIMESTAMPS".into());
        lines.push("FIFO V-SYNC - HUD INCLUDED".into());
        lines
    }
}

#[cfg(test)]
mod tests;
