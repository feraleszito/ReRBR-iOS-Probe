use rerbr_sim::{
    diagnostics::Sample,
    drive::{Control, Session},
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

pub struct Shared {
    pub input: Control,
    pub paused: bool,
    pub error: Option<String>,
    pub pair: Option<(Sample, Sample, Instant)>,
    pub rest: Option<Sample>,
    pub payload_bytes: usize,
    pub last_step_ms: f64,
}
pub struct Worker {
    pub state: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
}
impl Worker {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(Shared {
            input: Control::default(),
            paused: true,
            error: None,
            pair: None,
            rest: None,
            payload_bytes: 0,
            last_step_ms: 0.0,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let s = state.clone();
        let stop_thread = stop.clone();
        let spawn = std::thread::Builder::new()
            .name("physics-drive-720".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(&s, &stop_thread)
                }));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e),
                    Err(e) => Some(rerbr_sim::describe_stop(e.as_ref())),
                };
                if let Some(e) = error {
                    eprintln!("physics-drive STOP: {e}");
                    let mut s = s.lock().unwrap();
                    s.error = Some(e);
                    s.paused = true;
                }
            });
        if let Err(e) = spawn {
            state.lock().unwrap().error = Some(format!("physics worker: {e}"));
        }
        Self { state, stop }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
fn run(shared: &Mutex<Shared>, stop: &AtomicBool) -> Result<(), String> {
    let documents = std::env::var_os("RERBR_PROBE_DOCUMENTS").ok_or("Documents unavailable")?;
    let paths = Paths::documents(std::path::Path::new(&documents));
    let mut session = Session::boot(&paths)?;
    let mut previous = session.sample();
    if !valid(&previous) {
        return Err("invalid settled physics pose".into());
    }
    {
        let mut s = shared.lock().unwrap();
        s.rest = Some(previous.clone());
        s.payload_bytes = session.payload_bytes();
        s.pair = Some((previous.clone(), previous.clone(), Instant::now()));
    }
    let dt = f64::from(session.dt());
    let mut last = Instant::now();
    let mut debt = 0.0;
    while !stop.load(Ordering::Relaxed) {
        let (input, paused) = {
            let s = shared.lock().unwrap();
            (s.input, s.paused)
        };
        let now = Instant::now();
        let elapsed = now.duration_since(last).as_secs_f64();
        last = now;
        if paused {
            debt = 0.0;
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        debt += elapsed;
        if debt > 0.15 {
            let mut s = shared.lock().unwrap();
            s.paused = true;
            s.error = Some("CPU fell behind >150ms; paused without skipping physics".into());
            debt = 0.0;
            continue;
        }
        while debt >= dt {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let began = Instant::now();
            session.step(input)?;
            let sample = session.sample();
            if !valid(&sample) {
                return Err("nonfinite physical pose".into());
            }
            if sample.step % 720 == 0 {
                eprintln!("physics-drive STEP {}: pos={:?} velocity={:?} rpm={:.0} gear={} input=({:.2},{:.2},{:.2}) ground={:?} strut={:?}",sample.step,sample.position,sample.velocity,sample.rpm,sample.gear,input.throttle,input.brake,input.steering,sample.wheel_ground,sample.wheel_travel);
            }
            let mut s = shared.lock().unwrap();
            s.last_step_ms = began.elapsed().as_secs_f64() * 1000.0;
            s.payload_bytes = session.payload_bytes();
            s.pair = Some((previous, sample.clone(), Instant::now()));
            previous = sample;
            debt -= dt;
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    Ok(())
}
fn valid(sample: &Sample) -> bool {
    sample.finite() && (0.5..1.5).contains(&sample.orientation.iter().map(|q| q * q).sum::<f32>())
}
