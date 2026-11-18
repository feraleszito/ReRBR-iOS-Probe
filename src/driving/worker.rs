use rerbr_sim::{
    diagnostics::Sample,
    drive::{Control, Session, StageTiming},
    inputs::Paths,
    STACK_BYTES,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Shared {
    pub input: Control,
    pub paused: bool,
    pub error: Option<String>,
    pub pair: Option<(Sample, Sample, Instant)>,
    pub rest: Option<Sample>,
    pub payload_bytes: usize,
    pub last_step_ms: f64,
    pub physics_status: Option<String>,
    pub timing: StageTiming,
}
pub struct Worker {
    pub state: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        self.thread.is_none()
    }
    pub fn idle() -> Self {
        Self {
            state: Arc::new(Mutex::new(Shared {
                input: Control::default(),
                paused: true,
                error: None,
                pair: None,
                rest: None,
                payload_bytes: 0,
                last_step_ms: 0.0,
                physics_status: None,
                timing: StageTiming::default(),
            })),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
    pub fn new_selected(selection: rerbr_content::Selection, automatic: bool) -> Self {
        Self::spawn_inner(selection, automatic, None)
    }
    pub fn new_experimental_pack(
        selection: rerbr_content::Selection,
        pack_path: std::path::PathBuf,
        automatic: bool,
    ) -> Self {
        Self::spawn_inner(selection, automatic, Some(pack_path))
    }
    fn spawn_inner(
        selection: rerbr_content::Selection,
        automatic: bool,
        experimental_pack: Option<std::path::PathBuf>,
    ) -> Self {
        let state = Arc::new(Mutex::new(Shared {
            input: Control::default(),
            paused: true,
            error: None,
            pair: None,
            rest: None,
            payload_bytes: 0,
            last_step_ms: 0.0,
            physics_status: None,
            timing: StageTiming::default(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let s = state.clone();
        let stop_thread = stop.clone();
        let spawn = std::thread::Builder::new()
            .name("physics-drive-720".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        &s,
                        &stop_thread,
                        automatic,
                        &selection,
                        experimental_pack.as_deref(),
                    )
                }));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e),
                    Err(e) => Some(rerbr_sim::describe_stop(e.as_ref())),
                };
                if let Some(e) = error.filter(|_| !stop_thread.load(Ordering::Relaxed)) {
                    super::audio::set_paused(true);
                    eprintln!("physics-drive STOP: {e}");
                    let mut s = s.lock().unwrap();
                    s.error = Some(e);
                    s.paused = true;
                }
            });
        let thread = match spawn {
            Ok(thread) => Some(thread),
            Err(e) => {
                state.lock().unwrap().error = Some(format!("physics worker: {e}"));
                None
            }
        };
        Self {
            state,
            stop,
            thread,
        }
    }
    pub fn shutdown(&mut self) {
        super::audio::set_paused(true);
        self.stop.store(true, Ordering::Relaxed);
        // A boot cannot be interrupted inside original code. It retires on its
        // own worker; never wait for it on the Metal/UI thread.
        self.thread.take();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn run(
    shared: &Mutex<Shared>,
    stop: &AtomicBool,
    automatic: bool,
    selection: &rerbr_content::Selection,
    experimental_pack: Option<&std::path::Path>,
) -> Result<(), String> {
    static OWNER: Mutex<()> = Mutex::new(());
    let _owner = loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match OWNER.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(std::sync::TryLockError::Poisoned(e)) => break e.into_inner(),
        }
    };
    let documents = std::env::var_os("RERBR_PROBE_DOCUMENTS").ok_or("Documents unavailable")?;
    let doc_path = std::path::Path::new(&documents);
    let paths = Paths::for_mode(doc_path, selection, experimental_pack.is_some())?;
    let mut session = match experimental_pack {
        Some(pack_path) => {
            Session::boot_experimental_pack(&paths, selection, pack_path, automatic)?
        }
        None => Session::boot_selected(&paths, selection, automatic)?,
    };
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    if !selection.vehicle.is_ngp_experimental_car() {
        super::audio::init(doc_path, &selection.vehicle, 44100);
    }
    let mut previous = session.sample();
    if !valid(&previous) {
        return Err("invalid settled physics pose".into());
    }
    {
        let mut s = shared.lock().unwrap();
        s.rest = Some(previous.clone());
        s.physics_status = Some(session.physics_audit().summary());
        s.payload_bytes = session.payload_bytes();
        s.timing = session.timing();
        s.pair = Some((previous.clone(), previous.clone(), Instant::now()));
    }
    let dt = f64::from(session.dt());
    let mut last = Instant::now();
    let mut clock = SimulationClock::default();
    while !stop.load(Ordering::Relaxed) {
        let paused = {
            let s = shared.lock().unwrap();
            s.paused
        };
        let now = Instant::now();
        let elapsed = now.duration_since(last).as_secs_f64();
        last = now;
        clock.advance(elapsed, paused);
        if paused {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        while clock.debt >= dt {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            // Read controls at every simulation boundary, including during catch-up.
            let input = {
                let s = shared.lock().unwrap();
                if s.paused {
                    clock.advance(0.0, true);
                    break;
                }
                s.input
            };
            let began = Instant::now();
            let command = super::controls::pending_selection(input, previous.commanded_gear);
            session.step(command)?;
            let sample = session.sample();
            if !valid(&sample) {
                return Err("nonfinite physical pose".into());
            }
            let noise = session.engine_noise(500.0);
            super::audio::update_engine_state(&noise, sample.gear);
            if cfg!(debug_assertions) && sample.step % 720 == 0 {
                eprintln!("physics-drive STEP {}: pos={:?} velocity={:?} rpm={:.0} gear={} input=({:.2},{:.2},{:.2}) ground={:?} strut={:?}",sample.step,sample.position,sample.velocity,sample.rpm,sample.gear,input.throttle,input.brake,input.steering,sample.wheel_ground,sample.wheel_travel);
            }
            let mut s = shared.lock().unwrap();
            s.last_step_ms = began.elapsed().as_secs_f64() * 1000.0;
            s.payload_bytes = session.payload_bytes();
            s.timing = session.timing();
            // Publish the simulated boundary, retaining fractional fixed-step debt.
            let boundary = now
                .checked_sub(Duration::from_secs_f64((clock.debt - dt).max(0.0)))
                .unwrap_or(now);
            s.pair = Some((previous, sample.clone(), boundary));
            previous = sample;
            clock.debt -= dt;
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    Ok(())
}

/// Wall time schedules original fixed steps; it never changes their duration or drops them.
#[derive(Default)]
struct SimulationClock {
    debt: f64,
    running: bool,
}
impl SimulationClock {
    fn advance(&mut self, elapsed: f64, paused: bool) {
        if paused || !self.running {
            // The interval crossing resume includes paused time and is not simulated.
            self.debt = 0.0;
        } else {
            self.debt += elapsed;
        }
        self.running = !paused;
    }
}

fn valid(sample: &Sample) -> bool {
    sample.finite() && (0.5..1.5).contains(&sample.orientation.iter().map(|q| q * q).sum::<f32>())
}

#[cfg(test)]
mod tests {
    use super::SimulationClock;

    #[test]
    fn pause_and_resume_exclude_paused_wall_time_and_old_debt() {
        let mut clock = SimulationClock::default();
        clock.advance(12.0, true);
        clock.advance(0.002, false);
        assert_eq!(clock.debt, 0.0);
        clock.advance(0.001, false);
        assert_eq!(clock.debt, 0.001);
        clock.advance(0.0, true);
        clock.advance(100.0, false);
        assert_eq!(clock.debt, 0.0);
    }

    #[test]
    fn slow_rendering_preserves_every_fixed_step_and_fraction() {
        let dt = f64::from(f32::from_bits(0x3ab6_0b61));
        let mut clock = SimulationClock::default();
        clock.advance(0.0, false);
        clock.advance(dt * 360.5, false);
        let mut steps = 0;
        while clock.debt >= dt {
            clock.debt -= dt;
            steps += 1;
        }
        assert_eq!(steps, 360);
        assert!((clock.debt - dt * 0.5).abs() < 1e-12);
        clock.advance(dt * 0.5, false);
        assert!((clock.debt - dt).abs() < 1e-12);
    }
}
