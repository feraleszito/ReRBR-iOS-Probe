//! GPL-3.0-or-later. Pure reRBR boot/step probe; see UPSTREAM.md.
pub mod boot;
pub mod diagnostics;
pub mod drive;
mod handles;
pub mod inputs;
pub mod measure;

use rbr_gameplay::control::CarControl;
use rbr_gameplay::stage::{Stage, State, NGP_LOAD_SUBSTEPS};
use rbr_gameplay::weather::{self, Draws, Preset, Season};
use rbr_hybrid::pure::{self, Playback};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub const STACK_BYTES: usize = 32 * 1024 * 1024;
pub const MEASURED_STEPS: u32 = 720;

/// Preserve the pure host's actual stop reason across the app's worker boundary.
pub fn describe_stop(error: &(dyn std::any::Any + Send)) -> String {
    if let Some(stop) = error.downcast_ref::<pure::Stopped>() {
        stop.0.to_string()
    } else if let Some(s) = error.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = error.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "unknown Rust panic".into()
    }
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub phase: String,
    pub pass: u32,
    pub steps: u32,
    pub payload_bytes: usize,
    pub metadata_bytes: usize,
    pub footprint_bytes: Option<u64>,
}
impl Progress {
    fn new(phase: &str, pass: u32) -> Self {
        Self { phase: phase.into(), pass, steps: 0, payload_bytes: 0, metadata_bytes: 0, footprint_bytes: footprint() }
    }
}
#[derive(Debug, Clone)]
pub struct Report {
    pub driving_samples: Box<[Vec<diagnostics::Sample>; 2]>,
    /// Physical world displacement during the measured interval, not a scripted pose.
    pub displacement_metres: [f64; 2],
    pub boot_seconds: [f64; 2],
    pub measured: [measure::Summary; 2],
    pub fingerprints: [String; 2],
    pub timestep: f32,
    pub peak_payload_bytes: usize,
    pub max_sampled_footprint_bytes: Option<u64>,
}
impl Report {
    pub fn deterministic(&self) -> bool {
        self.fingerprints[0] == self.fingerprints[1]
    }
}

/// On iOS the native shell reports TASK_VM_INFO.phys_footprint. Other hosts report unavailable.
pub fn footprint() -> Option<u64> {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn rerbr_probe_physical_footprint() -> u64;
        }
        // SAFETY: native function takes no arguments and writes no Rust state.
        let value = unsafe { rerbr_probe_physical_footprint() };
        (value != 0).then_some(value)
    }
    #[cfg(not(target_os = "ios"))]
    {
        None
    }
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("physics-probe cancelled; no valid timing result".into())
    } else {
        Ok(())
    }
}
fn status(playback: &Playback, phase: &str, pass: u32, steps: u32) -> Progress {
    let pages = playback.pure.mem.pages().expect("boot uses pages");
    Progress {
        steps,
        payload_bytes: pages.allocated_bytes(),
        metadata_bytes: pages.metadata_bytes(),
        ..Progress::new(phase, pass)
    }
}
/// Must run on the dedicated STACK_BYTES worker. Two independent boots; no global environment mutation.
pub fn run(paths: &inputs::Paths, cancel: &AtomicBool, mut notify: impl FnMut(Progress)) -> Result<Report, String> {
    run_selected(paths, cancel, &mut notify, false)
}

/// CPU-only check: five seconds in first gear at full throttle, then five braking.
/// This is not the playable mode or proof of a complete driving loop.
pub fn run_acceleration(
    paths: &inputs::Paths,
    cancel: &AtomicBool,
    mut notify: impl FnMut(Progress),
) -> Result<Report, String> {
    run_selected(paths, cancel, &mut notify, true)
}

fn run_selected(
    paths: &inputs::Paths,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Progress),
    moving: bool,
) -> Result<Report, String> {
    eprintln!(
        "physics-probe HOST: arch={} os={} debug_assertions={} panic_unwind={} stack_bytes={STACK_BYTES}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        cfg!(debug_assertions),
        cfg!(panic = "unwind")
    );
    if cfg!(panic = "abort") {
        return Err(
            "physics-probe requires panic=unwind to catch a pure-host stop; use CARGO_PROFILE_RELEASE_PANIC=unwind"
                .into(),
        );
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_inner(paths, cancel, notify, moving))) {
        Ok(result) => result,
        Err(error) => {
            let reason = if let Some(stop) = error.downcast_ref::<pure::Stopped>() {
                stop.0.to_string()
            } else if let Some(s) = error.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = error.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown Rust panic".into()
            };
            Err(format!("physics-probe STOP: {reason}; no original executable fallback"))
        }
    }
}
fn run_inner(
    paths: &inputs::Paths,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Progress),
    moving: bool,
) -> Result<Report, String> {
    cancelled(cancel)?;
    notify(Progress::new("CHECK PRIVATE FILES", 0));
    let checked = inputs::check(paths)?;
    let data = paths.data();
    let season = Season::load(&data)?;
    // Good weather's list chooses N here. Select a real dry O season row, not fabricated weather words.
    let row = season
        .rows
        .iter()
        .position(|r| r.map == 20 && r.letter == 3 && r.wetness == 0)
        .ok_or("no original dry Harwood O weather row")?;
    let weather = weather::quick_race(&season, 20, Preset::Random, Draws { row: Some(row), wear: 1 })?;
    eprintln!(
        "physics-probe WEATHER: original season row {row}, {} dry/normal; fixed valid random-weather selection",
        weather.stage()
    );
    let race = boot::Race::quick_race(20, 6, 4, weather, checked.physics);
    let stage_name = paths.maps.join("track-20_O").to_string_lossy().into_owned();
    let mut report = Report {
        driving_samples: Default::default(),
        displacement_metres: [0.0; 2],
        boot_seconds: [0.0; 2],
        measured: [measure::Summary::default(); 2],
        fingerprints: Default::default(),
        timestep: 0.0,
        peak_payload_bytes: 0,
        max_sampled_footprint_bytes: None,
    };
    for pass in 1..=2 {
        cancelled(cancel)?;
        notify(Progress::new("BOOT ORIGINAL ENGINE", pass));
        let start = Instant::now();
        let image = boot::build(&data, &stage_name, State::Load.value(), &race)?;
        let mut playback = Playback::from_image(image.mem.clone(), image.regs, image.slot, image.control_at, false);
        playback.pure.ngp_base = image.ngp.base;
        playback.pure.direct = true;
        playback.pure.untrack_allocator();
        boot::update_a720(&playback.pure, &image);
        boot::ngp_init(&mut playback.pure.mem.clone(), &image);
        report.boot_seconds[(pass - 1) as usize] = start.elapsed().as_secs_f64();
        let dt = rbr_hybrid::timekeeper::layout::max_time_step(&playback.pure.mem);
        if dt.to_bits() != 0x3ab6_0b61 {
            return Err(format!("unexpected initialized NGP timestep: {dt:?}"));
        }
        report.timestep = dt;
        eprintln!(
            "physics-probe BOOT PASS {pass}: {:.3}s; built {:?}; dt={dt:.10}; no DLL instructions executed",
            start.elapsed().as_secs_f64(),
            image.built
        );
        // Original load state, 38 calls of StepForward(40), and original Stage transition rules.
        // This probe has no Game/menu/audio/frame-finish loop and does not claim driving equivalence.
        let mut stage = Stage::quick_race(20, 6).load_ready_with(NGP_LOAD_SUBSTEPS);
        let mut settled = 0;
        while stage.state() == State::Load {
            cancelled(cancel)?;
            pure::set_app_state(&mut playback.pure.mem.clone(), stage.state().value());
            playback.step(None, 40);
            settled += 40;
            stage = stage.load_tick(40);
            let memory = status(&playback, "SETTLE ORIGINAL CONTACTS", pass, settled);
            report.max_sampled_footprint_bytes = report.max_sampled_footprint_bytes.max(memory.footprint_bytes);
            notify(memory);
        }
        if settled != 1520 {
            return Err("unexpected load settle count".into());
        }
        stage = stage.go().countdown_tick(dt);
        if stage.state() != State::Driving {
            return Err("original stage transition did not enter driving".into());
        }
        pure::set_app_state(&mut playback.pure.mem.clone(), stage.state().value());
        // Keep the existing stationary probe's braked/reverse payload unchanged.
        // Original gear indices: 0 reverse, 1 neutral, 2 first (rbr_recording::hud).
        let control = if moving {
            CarControl { throttle: 1.0, flags: 0x04, gear: Some(2), ..Default::default() }
        } else {
            CarControl { brake: 1.0, clutch: 1.0, flags: 0x04, gear: Some(0), ..Default::default() }
        };
        let initial_position: [f32; 3] = playback.pure.mem.f32s(image.car + 0x140);
        let mut payload = [0; 32];
        for (k, w) in control.words().iter().enumerate() {
            payload[4 * k..4 * k + 4].copy_from_slice(&w.unwrap_or(0).to_le_bytes());
        }
        let initial_clock = pure::clock(&playback.pure.mem);
        let measured_steps = if moving { 7200 } else { MEASURED_STEPS };
        if moving {
            report.driving_samples[(pass - 1) as usize].push(diagnostics::read(&playback, image.control_at, 0));
        }
        let mut times = Vec::with_capacity(measured_steps as usize);
        let timed = Instant::now();
        for step in 0..measured_steps {
            cancelled(cancel)?;
            // Five seconds accelerating in first gear, then five braking with clutch released from the drivetrain.
            if moving && step == 3600 {
                let brake = CarControl { brake: 1.0, clutch: 1.0, flags: 0x04, gear: Some(2), ..Default::default() };
                for (k, w) in brake.words().iter().enumerate() {
                    payload[4 * k..4 * k + 4].copy_from_slice(&w.unwrap_or(0).to_le_bytes());
                }
            }
            let before = Instant::now();
            playback.step(Some(&payload), 1);
            times.push(before.elapsed().as_secs_f64());
            if moving && (step + 1) % 720 == 0 {
                let sample = diagnostics::read(&playback, image.control_at, step + 1);
                if !sample.finite() {
                    return Err("nonfinite driving diagnostic state".into());
                }
                eprintln!("DRIVING pass={pass} {sample:?}");
                report.driving_samples[(pass - 1) as usize].push(sample);
            }
            if step % 72 == 71 {
                let memory = status(&playback, "MEASURE REAL FIXED STEPS", pass, step + 1);
                report.max_sampled_footprint_bytes = report.max_sampled_footprint_bytes.max(memory.footprint_bytes);
                notify(memory);
            }
        }
        let elapsed = timed.elapsed().as_secs_f64();
        let advanced = pure::clock(&playback.pure.mem) - initial_clock;
        if (advanced - f64::from(dt) * f64::from(measured_steps)).abs() > 1e-6 {
            return Err(format!("physics clock mismatch: {advanced}"));
        }
        let final_state = pure::frame_bytes(&playback.pure.mem);
        let mem = &playback.pure.mem;
        let position: [f32; 3] = mem.f32s(image.car + 0x140);
        if !position.into_iter().all(f32::is_finite) {
            return Err("nonfinite physical car position".into());
        }
        let k = (pass - 1) as usize;
        report.displacement_metres[k] = position
            .iter()
            .zip(initial_position)
            .map(|(&a, b)| (f64::from(a) - f64::from(b)).powi(2))
            .sum::<f64>()
            .sqrt();
        report.measured[k] = measure::summarize(&times, dt, elapsed)?;
        report.fingerprints[k] = format!("{:x}", Sha256::digest(&final_state));
        let memory = status(&playback, "PASS COMPLETE", pass, measured_steps);
        report.peak_payload_bytes = report.peak_payload_bytes.max(memory.payload_bytes);
        report.max_sampled_footprint_bytes = report.max_sampled_footprint_bytes.max(memory.footprint_bytes);
        eprintln!("physics-probe PASS {pass}: {:?}; sparse payload={} metadata={}; endpoint FRM1 SHA256={}; fingerprint covers car/wheels only", report.measured[k], memory.payload_bytes, memory.metadata_bytes, report.fingerprints[k]);
        notify(memory);
    }
    if !report.deterministic() {
        return Err(format!(
            "physics-probe deterministic endpoint mismatch: {} != {}",
            report.fingerprints[0], report.fingerprints[1]
        ));
    }
    Ok(report)
}
