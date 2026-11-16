//! iOS and platform audio runtime for authentic Richard Burns Rally engine audio.
//!
//! Provides thread-safe communication between the 720 Hz physics worker, the main UIKit loop,
//! and the realtime CoreAudio / RemoteIO audio render callback.

use rerbr_sim::engine_audio::{self, CarAudio, EngineNoise};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Pre-allocated scratch buffer capacity to prevent allocations in the audio callback.
const PREALLOC_SCRATCH_FRAMES: usize = 4096;

/// State of the audio runtime.
struct AudioRuntime {
    car_audio: Option<CarAudio>,
    status: String,
    volume: f32,
    paused: bool,
    interrupted: bool,
    is_cockpit: bool,
    sample_rate: u32,
    stereo_scratch: Vec<[f32; 2]>,
}

impl AudioRuntime {
    fn new(volume: f32) -> Self {
        Self {
            car_audio: None,
            status: "AUDIO UNINITIALIZED".into(),
            volume: volume.clamp(0.0, 1.0),
            paused: true,
            interrupted: false,
            is_cockpit: false,
            sample_rate: 44100,
            stereo_scratch: vec![[0.0, 0.0]; PREALLOC_SCRATCH_FRAMES],
        }
    }
}

static AUDIO_RUNTIME: OnceLock<Arc<Mutex<AudioRuntime>>> = OnceLock::new();

fn runtime() -> &'static Arc<Mutex<AudioRuntime>> {
    AUDIO_RUNTIME.get_or_init(|| Arc::new(Mutex::new(AudioRuntime::new(0.8))))
}

/// Initialize authentic audio for the selected vehicle.
/// Safe: if private audio assets are missing or invalid, initializes into silent mode without crashing.
pub fn init(documents: &Path, vehicle: &rerbr_content::VehicleDescriptor, sample_rate: u32) {
    let rt = runtime();
    let mut lock = rt.lock().unwrap();
    lock.sample_rate = sample_rate.max(11025);

    // Private game assets are placed in Documents/RBR/ or directly in Documents/.
    let candidate_roots = [documents.join("RBR"), documents.to_path_buf()];

    let mut loaded = None;
    let mut last_error = "audio assets not found".to_string();

    for root in &candidate_roots {
        if !root.is_dir() {
            continue;
        }
        match engine_audio::load_for_vehicle(root, vehicle) {
            Ok((bank, config)) => {
                eprintln!(
                    "AUDIO INIT: loaded bank '{}' ({} RPM banks) from {:?}",
                    config.engine_bank,
                    bank.banks.len(),
                    root
                );
                loaded = Some((bank, config));
                break;
            }
            Err(e) => {
                last_error = e;
            }
        }
    }

    if let Some((bank, config)) = loaded {
        let mut car_audio = CarAudio::new(bank, config, lock.sample_rate, 42);
        car_audio.set_volume(lock.volume);
        car_audio.set_paused(lock.paused);
        car_audio.set_interrupted(lock.interrupted);
        car_audio.set_cockpit(lock.is_cockpit);
        lock.status = "AUTHENTIC ENGINE AUDIO READY".into();
        lock.car_audio = Some(car_audio);
    } else {
        eprintln!("AUDIO INIT: running in silent fallback mode: {last_error}");
        lock.status = format!("AUDIO SILENT: {last_error}");
        lock.car_audio = None;
    }
}

/// Update physics telemetry inputs from the 720 Hz simulation thread.
pub fn update_engine_state(noise: &EngineNoise, gear: i32) {
    let rt = runtime();
    if let Ok(mut lock) = rt.try_lock() {
        if let Some(audio) = lock.car_audio.as_mut() {
            audio.step_physics(noise, gear);
        }
    }
}

/// Set whether the camera is inside the cockpit to apply interior acoustics.
pub fn set_view_is_cockpit(is_cockpit: bool) {
    let rt = runtime();
    if let Ok(mut lock) = rt.lock() {
        lock.is_cockpit = is_cockpit;
        if let Some(audio) = lock.car_audio.as_mut() {
            audio.set_cockpit(is_cockpit);
        }
    }
}

/// Update user master volume setting (0.0 to 1.0).
pub fn set_master_volume(volume: f32) {
    let rt = runtime();
    if let Ok(mut lock) = rt.lock() {
        let clamped = volume.clamp(0.0, 1.0);
        lock.volume = clamped;
        if let Some(audio) = lock.car_audio.as_mut() {
            audio.set_volume(clamped);
        }
    }
}

/// Pause or resume engine audio playback.
pub fn set_paused(paused: bool) {
    let rt = runtime();
    if let Ok(mut lock) = rt.lock() {
        lock.paused = paused;
        if let Some(audio) = lock.car_audio.as_mut() {
            audio.set_paused(paused);
        }
    }
}

/// Handle platform audio interruption (phone call, alarm, Siri).
pub fn set_interrupted(interrupted: bool) {
    let rt = runtime();
    if let Ok(mut lock) = rt.lock() {
        lock.interrupted = interrupted;
        if let Some(audio) = lock.car_audio.as_mut() {
            audio.set_interrupted(interrupted);
        }
    }
}

/// Query human-readable audio system status.
pub fn status() -> String {
    let rt = runtime();
    rt.lock()
        .map_or_else(|_| "AUDIO ERROR".into(), |lock| lock.status.clone())
}

/// Render interleaved stereo float audio frames for audio output.
/// Uses a non-blocking lock and bounded, preallocated buffers. Unsupported callback
/// sizes or rates produce silence; device timing still requires hardware validation.
pub fn render(out: &mut [f32], frames: usize, sample_rate: u32) {
    out.fill(0.0);
    if frames > PREALLOC_SCRATCH_FRAMES || frames > out.len() / 2 {
        return;
    }
    let rt = runtime();
    // Non-blocking try_lock: real-time audio thread never priority-inverts or waits on locks.
    let mut lock = match rt.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            out.fill(0.0);
            return;
        }
    };

    if lock.sample_rate != sample_rate {
        return;
    }

    if lock.paused || lock.interrupted || lock.volume <= 0.0 || lock.car_audio.is_none() {
        out.fill(0.0);
        return;
    }

    let mut scratch = std::mem::take(&mut lock.stereo_scratch);
    if let Some(audio) = lock.car_audio.as_mut() {
        let render_slice = &mut scratch[..frames];
        render_slice.fill([0.0, 0.0]);
        audio.render_stereo(render_slice);
        for (i, frame) in render_slice.iter().enumerate() {
            if i * 2 + 1 < out.len() {
                out[i * 2] = frame[0];
                out[i * 2 + 1] = frame[1];
            }
        }
    } else {
        out.fill(0.0);
    }
    lock.stereo_scratch = scratch;
}

// -----------------------------------------------------------------------------
// Native iOS C ABI Exports
// -----------------------------------------------------------------------------

/// Called by the native iOS RemoteIO / AudioUnit render callback.
///
/// # Safety
///
/// `out_stereo` must either be null or point to an allocated, properly aligned,
/// and mutable buffer of at least `frames * 2` contiguous `f32` samples.
#[no_mangle]
pub unsafe extern "C" fn rerbr_audio_render(out_stereo: *mut f32, frames: u32, sample_rate: u32) {
    if out_stereo.is_null() || frames == 0 {
        return;
    }
    let len = frames as usize * 2;
    let slice = std::slice::from_raw_parts_mut(out_stereo, len);
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        render(slice, frames as usize, sample_rate);
    }))
    .is_err()
    {
        slice.fill(0.0);
    }
}

/// Initialize callback storage on the calling thread before RemoteIO starts.
#[no_mangle]
pub extern "C" fn rerbr_audio_prepare() -> i32 {
    let _ = runtime();
    1
}

/// Set audio pause state from iOS lifecycle or winit.
#[no_mangle]
pub extern "C" fn rerbr_audio_set_paused(paused: i32) {
    set_paused(paused != 0);
}

/// Set audio interruption state from AVAudioSession notifications.
#[no_mangle]
pub extern "C" fn rerbr_audio_set_interrupted(interrupted: i32) {
    set_interrupted(interrupted != 0);
}

/// Set audio volume from native settings or slider.
#[no_mangle]
pub extern "C" fn rerbr_audio_set_volume(volume: f32) {
    set_master_volume(volume);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contention_and_unsupported_callbacks_are_silent_without_blocking_physics() {
        rerbr_audio_prepare();
        let guard = runtime().lock().unwrap();
        let noise = EngineNoise::from_rpm_throttle(3000.0, 1.0, 500.0);
        // This would deadlock with the old blocking telemetry lock.
        update_engine_state(&noise, 2);
        let mut out = [1.0; 8];
        render(&mut out, 4, 44100);
        assert_eq!(out, [0.0; 8]);
        drop(guard);
        for (frames, rate) in [(5, 44100), (4097, 44100), (4, 48000)] {
            out.fill(1.0);
            render(&mut out, frames, rate);
            assert_eq!(out, [0.0; 8]);
        }
    }
}
