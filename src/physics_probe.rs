//! Independent CPU probe over an animated liveness triangle and the existing ASCII HUD.
#[cfg(not(feature = "harwood-benchmark"))]
#[path = "harwood/benchmark/hud.rs"]
mod hud;
#[cfg(feature = "harwood-benchmark")]
use crate::harwood::benchmark::hud;
use rerbr_sim::{inputs::Paths, Progress, Report, STACK_BYTES};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

enum Event {
    Progress(Progress),
    Finished(Result<Report, String>),
}
pub(crate) struct Scene {
    receiver: Option<mpsc::Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    progress: Option<Progress>,
    report: Option<Report>,
    error: Option<String>,
    hud: Option<hud::Hud>,
    hud_failed: bool,
}
impl Scene {
    pub fn new() -> Self {
        let mut scene = Self {
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: None,
            report: None,
            error: None,
            hud: None,
            hud_failed: false,
        };
        scene.restart();
        scene
    }
    pub fn interrupt(&mut self) {
        if self.receiver.is_some() {
            self.cancel.store(true, Ordering::Relaxed);
            self.error = Some("CANCEL REQUESTED - APP INACTIVE".into());
            self.report = None;
        }
    }
    pub fn restart(&mut self) {
        self.poll();
        if self.receiver.is_some() {
            return;
        }
        self.report = None;
        self.error = None;
        self.progress = None;
        let Some(documents) = std::env::var_os("RERBR_PROBE_DOCUMENTS") else {
            self.error = Some("native Documents path unavailable".into());
            return;
        };
        let paths = Paths::documents(std::path::Path::new(&documents));
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        eprintln!("physics-probe START: pure Rust worker; stack={} MiB; sparse logical u32 memory; stationary neutral/brake test; GPU timing N/A",STACK_BYTES/(1024*1024));
        match std::thread::Builder::new()
            .name("pure-physics-probe".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                let result = rerbr_sim::run(&paths, &cancel, |p| {
                    let _ = tx.send(Event::Progress(p));
                });
                let _ = tx.send(Event::Finished(result));
            }) {
            Ok(_) => self.receiver = Some(rx),
            Err(e) => self.error = Some(format!("could not start physics worker: {e}")),
        }
    }
    fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            loop {
                match rx.try_recv() {
                    Ok(Event::Progress(p)) => self.progress = Some(p),
                    Ok(Event::Finished(result)) => {
                        match result {
                            Ok(report) => {
                                eprintln!("physics-probe COMPLETE: {report:?}; endpoint repeatability only, not original-game equivalence or iOS performance certification");
                                self.report = Some(report);
                                self.error = None;
                            }
                            Err(e) => {
                                eprintln!("physics-probe ERROR: {e}");
                                self.error = Some(e);
                            }
                        }
                        finished = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.error = Some("physics worker disconnected".into());
                        finished = true;
                        break;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
        }
    }
    fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            "PHYSICS PROBE - PURE RERBR".into(),
            "COROLLA / HARWOOD / NGP 7.4.778".into(),
        ];
        if self.error.is_some() {
            lines.push("ERROR OR INTERRUPTED - SEE LOG".into());
            lines.push("DOCUMENTS/RERBR-PROBE.LOG".into());
        } else if let Some(report) = &self.report {
            lines.push("TWO PASSES COMPLETE - REPEATABLE".into());
            for (k, s) in report.measured.iter().enumerate() {
                lines.push(format!(
                    "P{} STEP AVG {:.3} P95 {:.3} MS",
                    k + 1,
                    s.mean_ms,
                    s.p95_ms
                ));
                lines.push(format!(
                    "P99 {:.3} MS / {:.2} X REALTIME",
                    s.p99_ms, s.realtime_factor
                ));
            }
            lines.push(format!(
                "DT {:.6} MS / {} STEPS PER PASS",
                report.timestep * 1000.0,
                report.measured[0].steps
            ));
        } else if let Some(p) = &self.progress {
            lines.push(format!("PASS {} / {}", p.pass, p.phase));
            lines.push(format!("PHYSICAL STEPS {}", p.steps));
        } else {
            lines.push("STARTING CPU WORKER".into());
        }
        if let Some(p) = &self.progress {
            lines.push(format!(
                "PAGES {:.1} + TABLE {:.1} MIB",
                p.payload_bytes as f64 / 1048576.0,
                p.metadata_bytes as f64 / 1048576.0
            ));
            if let Some(bytes) = p.footprint_bytes {
                lines.push(format!("APP FOOTPRINT {:.1} MIB", bytes as f64 / 1048576.0));
            }
        }
        lines.push("HOST WALL TIME / GPU N/A".into());
        lines.push(
            if self.receiver.is_some() {
                "RUNNING - TAP IGNORED"
            } else {
                "TAP TO RESTART CPU PROBE"
            }
            .into(),
        );
        lines
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        self.poll();
        if self.hud.is_none() && !self.hud_failed {
            match hud::Hud::new(device, queue, format) {
                Ok(h) => self.hud = Some(h),
                Err(e) => {
                    self.hud_failed = true;
                    self.interrupt();
                    eprintln!("physics-probe HUD ERROR: {e}");
                }
            }
        }
        if let Some(hud) = &self.hud {
            hud.draw(queue, encoder, target, size, &self.lines());
        }
    }
}
impl Drop for Scene {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
