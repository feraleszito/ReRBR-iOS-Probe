//! iOS and platform audio runtime for authentic Richard Burns Rally engine audio.
//!
//! Provides thread-safe communication between the 720 Hz physics worker, the main UIKit loop,
//! and the realtime CoreAudio / RemoteIO audio render callback.

use rerbr_sim::engine_audio::{self, CarAudio, EngineNoise};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Pre-allocated scratch buffer capacity to prevent allocations in the audio callback.
const PREALLOC_SCRATCH_FRAMES: usize = 4096;
const MAX_SPEECH_QUEUE: usize = 16;

/// Sequential stereo PCM voice queue for authentic RBR co-driver pacenotes.
#[derive(Default)]
struct SpeechPlayer {
    queue: VecDeque<Arc<Vec<[i16; 2]>>>,
    current: Option<(Arc<Vec<[i16; 2]>>, usize)>,
    active: bool,
}

impl SpeechPlayer {
    fn enqueue_batch(&mut self, clips: Vec<Arc<Vec<[i16; 2]>>>) {
        for clip in clips {
            if clip.is_empty() {
                continue;
            }
            if self.queue.len() >= MAX_SPEECH_QUEUE {
                self.queue.pop_front();
            }
            self.queue.push_back(clip);
        }
    }

    fn clear(&mut self) {
        self.queue.clear();
        self.current = None;
    }

    fn mix_into(&mut self, out: &mut [f32], frames: usize, codriver_volume: f32) {
        if !self.active || codriver_volume <= 0.0 {
            return;
        }
        let scale = codriver_volume / 32768.0;
        for i in 0..frames {
            while self.current.is_none() {
                let Some(next) = self.queue.pop_front() else {
                    return;
                };
                if !next.is_empty() {
                    self.current = Some((next, 0));
                }
            }
            let (clip, cursor) = self.current.as_mut().expect("current checked non-empty");
            let frame = clip[*cursor];
            *cursor += 1;
            if *cursor >= clip.len() {
                self.current = None;
            }
            if i * 2 + 1 < out.len() {
                out[i * 2] = (out[i * 2] + f32::from(frame[0]) * scale).clamp(-1.0, 1.0);
                out[i * 2 + 1] = (out[i * 2 + 1] + f32::from(frame[1]) * scale).clamp(-1.0, 1.0);
            }
        }
    }
}

/// Menu and replay music track streaming decoded 16-bit PCM stereo samples.
struct MenuMusicTrack {
    samples: Arc<Vec<[i16; 2]>>,
    cursor: usize,
    gain: f32,
    active: bool,
}

impl MenuMusicTrack {
    fn new(samples: Arc<Vec<[i16; 2]>>, active: bool) -> Self {
        Self {
            samples,
            cursor: 0,
            gain: 0.0,
            active,
        }
    }

    fn mix_into(&mut self, out: &mut [f32], frames: usize, sample_rate: u32, music_volume: f32) {
        if self.samples.is_empty() || music_volume <= 0.0 {
            self.gain = 0.0;
            return;
        }
        let target = if self.active { 1.0 } else { 0.0 };
        if self.gain <= 0.0001 && target == 0.0 {
            self.gain = 0.0;
            return;
        }
        // ~250 ms smooth fade-in / fade-out
        let step = 4.0 / sample_rate.max(11025) as f32;
        let len = self.samples.len();
        for i in 0..frames {
            if self.gain < target {
                self.gain = (self.gain + step).min(target);
            } else if self.gain > target {
                self.gain = (self.gain - step).max(target);
            }
            if self.gain <= 0.0001 && target == 0.0 {
                self.gain = 0.0;
                break;
            }
            let frame = self.samples[self.cursor];
            self.cursor += 1;
            if self.cursor >= len {
                self.cursor = 0;
            }
            let scale = (self.gain * music_volume) / 32768.0;
            if i * 2 + 1 < out.len() {
                out[i * 2] = (out[i * 2] + f32::from(frame[0]) * scale).clamp(-1.0, 1.0);
                out[i * 2 + 1] = (out[i * 2 + 1] + f32::from(frame[1]) * scale).clamp(-1.0, 1.0);
            }
        }
    }
}

/// State of the audio runtime.
/// Terrain surface classification for modular surface & tyre skid feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SurfaceKind {
    #[default]
    Gravel,
    Tarmac,
}

/// Real-time allocation-free surface tyre rolling & slip synthesizer driven by 720 Hz telemetry.
#[derive(Clone, Copy, Debug)]
struct SurfaceAudio {
    surface: SurfaceKind,
    speed_mps: f32,
    slip: f32,
    grounded_wheels: usize,
    rng: u32,
    lp_state: f32,
    gravel_pulse: f32,
}

impl Default for SurfaceAudio {
    fn default() -> Self {
        Self {
            surface: SurfaceKind::Gravel,
            speed_mps: 0.0,
            slip: 0.0,
            grounded_wheels: 4,
            rng: 0x1357_9bdf,
            lp_state: 0.0,
            gravel_pulse: 0.0,
        }
    }
}

impl SurfaceAudio {
    #[inline]
    fn next_noise(&mut self) -> f32 {
        // xorshift32 in [-1.0, 1.0]
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = if x == 0 { 0x6d2b_79f5 } else { x };
        (self.rng as i32) as f32 / i32::MAX as f32
    }

    fn mix_into(&mut self, out: &mut [f32], frames: usize, volume: f32, is_cockpit: bool) {
        if volume <= 0.0 || self.grounded_wheels == 0 || self.speed_mps < 1.0 {
            return;
        }
        let ground_scale = (self.grounded_wheels as f32 / 4.0).clamp(0.25, 1.0);
        let cockpit_atten = if is_cockpit { 0.55 } else { 0.90 };
        let rolling = (self.speed_mps / 45.0).clamp(0.0, 1.0);
        let slip = self.slip.clamp(0.0, 1.0);
        let alpha = match self.surface {
            SurfaceKind::Gravel => 0.14 + rolling * 0.10,
            SurfaceKind::Tarmac => 0.06 + slip * 0.18,
        };
        let level = (0.04 * rolling + 0.11 * slip) * volume * ground_scale * cockpit_atten;
        for i in 0..frames {
            let n = self.next_noise();
            self.lp_state += alpha * (n - self.lp_state);
            let sample = match self.surface {
                SurfaceKind::Gravel => {
                    // Sparse pebble impacts modulated cleanly by speed + slip
                    if self.rng & 0x1ff == 0 {
                        self.gravel_pulse = 0.6 + rolling * 0.4;
                    }
                    self.gravel_pulse *= 0.992;
                    (self.lp_state * 0.75 + n * self.gravel_pulse * 0.45) * level
                }
                SurfaceKind::Tarmac => (self.lp_state * (0.45 + slip * 0.75)) * level,
            };
            if i * 2 + 1 < out.len() {
                out[i * 2] = (out[i * 2] + sample).clamp(-1.0, 1.0);
                out[i * 2 + 1] = (out[i * 2 + 1] + sample).clamp(-1.0, 1.0);
            }
        }
    }
}

struct AudioRuntime {
    car_audio: Option<CarAudio>,
    surface_audio: SurfaceAudio,
    status: String,
    volume: f32,
    music_volume: f32,
    codriver_volume: f32,
    speech: SpeechPlayer,
    codriver_status: String,
    menu_music: Option<MenuMusicTrack>,
    menu_music_active: bool,
    menu_music_loading: bool,
    menu_music_status: String,
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
            surface_audio: SurfaceAudio::default(),
            status: "AUDIO UNINITIALIZED".into(),
            volume: volume.clamp(0.0, 1.0),
            music_volume: 0.70,
            codriver_volume: 0.85,
            speech: SpeechPlayer::default(),
            codriver_status: "COPILOTO INACTIVO".into(),
            menu_music: None,
            menu_music_active: false,
            menu_music_loading: false,
            menu_music_status: "MÚSICA NO CARGADA".into(),
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

/// Inspects an RSF/NGP vehicle's FMOD bank without substituting retail `.eng` sounds.
/// Preserves co-driver voice and modular surface feedback even when FSB5 Vorbis is muted.
pub fn init_rsf_fmod(
    documents: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
    sample_rate: u32,
) -> String {
    let report = super::fmod_audit::inspect_bank(&documents.join("RBR"), &vehicle.audio_index);
    let summary = report.summary();
    let rt = runtime();
    if let Ok(mut lock) = rt.lock() {
        lock.sample_rate = sample_rate;
        lock.car_audio = None;
        lock.status = summary.clone();
    }
    summary
}

/// Update live surface, speed, slip, and ground contact telemetry for the modular surface channel.
pub fn update_surface_state(
    surface: SurfaceKind,
    speed_mps: f32,
    wheel_slip: f32,
    grounded_wheels: usize,
) {
    if let Ok(mut lock) = runtime().try_lock() {
        lock.surface_audio.surface = surface;
        lock.surface_audio.speed_mps = if speed_mps.is_finite() {
            speed_mps.max(0.0)
        } else {
            0.0
        };
        lock.surface_audio.slip = if wheel_slip.is_finite() {
            wheel_slip.clamp(0.0, 1.0)
        } else {
            0.0
        };
        lock.surface_audio.grounded_wheels = grounded_wheels;
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
        let music_active = lock.menu_music_active;
        if let Some(track) = lock.menu_music.as_mut() {
            track.active = music_active && paused;
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

/// Query menu music status.
pub fn menu_music_status() -> String {
    let rt = runtime();
    rt.lock().map_or_else(
        |_| "ERROR MÚSICA".into(),
        |lock| lock.menu_music_status.clone(),
    )
}

/// Initialize menu music from Documents asynchronously.
/// Supports authentic RBR `Audio/Music/ba.wav` or custom menu tracks.
pub fn init_menu_music(documents: &Path) {
    let candidates = [
        documents.join("RBR/Audio/Music/ba.wav"),
        documents.join("Audio/Music/ba.wav"),
        documents.join("RBR/Audio/Music/menu.wav"),
        documents.join("Audio/Music/menu.wav"),
    ];
    let found = candidates.into_iter().find(|p| p.is_file());
    let path = match found {
        Some(p) => p,
        None => {
            let ogg_candidates = [
                documents.join("RBR/Audio/Music/ba.ogg"),
                documents.join("Audio/Music/ba.ogg"),
            ];
            if let Ok(mut lock) = runtime().lock() {
                if ogg_candidates.iter().any(|p| p.is_file()) {
                    lock.menu_music_status = "ba.ogg PRESENTE (REQUIERE ba.wav PCM)".into();
                } else {
                    lock.menu_music_status = "MÚSICA NO INSTALADA (OPCIONAL)".into();
                }
            }
            return;
        }
    };

    {
        let mut lock = match runtime().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if lock.menu_music.is_some() || lock.menu_music_loading {
            return;
        }
        lock.menu_music_loading = true;
        lock.menu_music_status = "CARGANDO MÚSICA...".into();
    }

    let _ = std::thread::Builder::new()
        .name("rerbr-menu-music".into())
        .spawn(move || {
            let decoded = load_menu_music_file(&path);
            if let Ok(mut lock) = runtime().lock() {
                lock.menu_music_loading = false;
                match decoded {
                    Ok(samples) if !samples.is_empty() => {
                        let active = lock.menu_music_active && lock.paused;
                        lock.menu_music = Some(MenuMusicTrack::new(Arc::new(samples), active));
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("ba.wav");
                        lock.menu_music_status = format!("MÚSICA ACTIVA ({name})");
                    }
                    Ok(_) => {
                        lock.menu_music_status = "MÚSICA VACÍA".into();
                    }
                    Err(e) => {
                        eprintln!("MENU MUSIC ERROR: {e}");
                        lock.menu_music_status = format!("ERROR MÚSICA: {e}");
                    }
                }
            }
        });
}

/// Enable or disable menu music playback smoothly.
pub fn set_menu_music_active(active: bool) {
    if let Ok(mut lock) = runtime().lock() {
        let paused = lock.paused;
        lock.menu_music_active = active;
        if let Some(track) = lock.menu_music.as_mut() {
            track.active = active && paused;
        }
    }
}

/// Independent music volume (`0.0..=1.0`), separate from engine master volume.
pub fn set_music_volume(volume: f32) {
    if let Ok(mut lock) = runtime().lock() {
        lock.music_volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.70
        };
    }
}

/// Independent co-driver speech volume (`0.0..=1.0`), separate from engine and music volumes.
pub fn set_codriver_volume(volume: f32) {
    if let Ok(mut lock) = runtime().lock() {
        lock.codriver_volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.85
        };
    }
}

/// Enable or pause co-driver playback without dropping queued notes.
pub fn set_codriver_active(active: bool) {
    if let Ok(mut lock) = runtime().lock() {
        lock.speech.active = active;
    }
}

/// Record human-readable co-driver voice bank status.
pub fn set_codriver_status(status: impl Into<String>) {
    if let Ok(mut lock) = runtime().lock() {
        lock.codriver_status = status.into();
    }
}

/// Query human-readable co-driver status.
pub fn codriver_status() -> String {
    runtime().lock().map_or_else(
        |_| "COPILOTO ERROR".into(),
        |lock| lock.codriver_status.clone(),
    )
}

/// Enqueue pre-decoded stereo co-driver speech segments to play strictly sequentially.
pub fn enqueue_speech_batch(clips: Vec<Arc<Vec<[i16; 2]>>>) {
    if clips.is_empty() {
        return;
    }
    if let Ok(mut lock) = runtime().lock() {
        lock.speech.enqueue_batch(clips);
    }
}

/// Clear active and queued co-driver callouts (stage start, restart, or exit).
pub fn clear_speech_queue() {
    if let Ok(mut lock) = runtime().lock() {
        lock.speech.clear();
    }
}

/// Decodes standard RIFF/WAVE PCM or IEEE-float stereo audio files.
pub fn decode_wav_file(path: &Path) -> Result<Vec<[i16; 2]>, String> {
    load_menu_music_file(path)
}

/// Decodes standard RIFF/WAVE PCM or IEEE-float stereo audio files.
pub fn load_menu_music_file(path: &Path) -> Result<Vec<[i16; 2]>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    decode_wav_stereo(&bytes)
}

fn decode_wav_stereo(bytes: &[u8]) -> Result<Vec<[i16; 2]>, String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("WAV: cabecera RIFF/WAVE no válida".into());
    }
    let mut pos = 12usize;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits = 0u16;
    let mut format_tag = 1u16;
    let mut data_slice: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let payload_start = pos + 8;
        let payload_end = payload_start.saturating_add(size).min(bytes.len());
        if id == b"fmt " && payload_end - payload_start >= 16 {
            let fmt = &bytes[payload_start..payload_end];
            format_tag = u16::from_le_bytes([fmt[0], fmt[1]]);
            channels = u16::from_le_bytes([fmt[2], fmt[3]]);
            sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
            bits = u16::from_le_bytes([fmt[14], fmt[15]]);
        } else if id == b"data" {
            data_slice = Some(&bytes[payload_start..payload_end]);
        }
        pos = payload_start.saturating_add(size + (size & 1));
    }
    let data = data_slice.ok_or("WAV: no se encontró chunk data")?;
    if channels == 0 || sample_rate == 0 {
        return Err("WAV: canal o sample rate no válido".into());
    }
    let ch = usize::from(channels);
    let mut stereo = Vec::new();
    if format_tag == 1 && bits == 16 {
        for frame in data.chunks_exact(ch * 2) {
            let l = i16::from_le_bytes([frame[0], frame[1]]);
            let r = if ch >= 2 {
                i16::from_le_bytes([frame[2], frame[3]])
            } else {
                l
            };
            stereo.push([l, r]);
        }
    } else if format_tag == 1 && bits == 8 {
        for frame in data.chunks_exact(ch) {
            let l = (i16::from(frame[0]) - 128) << 8;
            let r = if ch >= 2 {
                (i16::from(frame[1]) - 128) << 8
            } else {
                l
            };
            stereo.push([l, r]);
        }
    } else if format_tag == 3 && bits == 32 {
        for frame in data.chunks_exact(ch * 4) {
            let fl = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
            let fr = if ch >= 2 {
                f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]])
            } else {
                fl
            };
            stereo.push([
                (fl * 32767.0).clamp(-32768.0, 32767.0) as i16,
                (fr * 32767.0).clamp(-32768.0, 32767.0) as i16,
            ]);
        }
    } else {
        return Err(format!(
            "WAV: formato no soportado (tag {format_tag}, {bits} bits)"
        ));
    }
    if sample_rate != 44100 && !stereo.is_empty() {
        stereo = resample_linear(&stereo, sample_rate, 44100);
    }
    Ok(stereo)
}

fn resample_linear(input: &[[i16; 2]], src_rate: u32, dst_rate: u32) -> Vec<[i16; 2]> {
    let out_len = ((input.len() as u64 * u64::from(dst_rate)) / u64::from(src_rate)) as usize;
    let mut out = Vec::with_capacity(out_len);
    let ratio = src_rate as f64 / dst_rate as f64;
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = (pos as usize).min(input.len() - 1);
        let next = (idx + 1).min(input.len() - 1);
        let t = (pos - idx as f64) as f32;
        let l = f32::from(input[idx][0]) * (1.0 - t) + f32::from(input[next][0]) * t;
        let r = f32::from(input[idx][1]) * (1.0 - t) + f32::from(input[next][1]) * t;
        out.push([l as i16, r as i16]);
    }
    out
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

    if lock.interrupted {
        out.fill(0.0);
        return;
    }

    if !lock.paused && lock.volume > 0.0 && lock.car_audio.is_some() {
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
        }
        lock.stereo_scratch = scratch;
    } else if lock.paused {
        let music_vol = lock.music_volume;
        if let Some(track) = lock.menu_music.as_mut() {
            track.mix_into(out, frames, sample_rate, music_vol);
        }
    }
    if !lock.paused && lock.volume > 0.0 {
        let vol = lock.volume;
        let is_cockpit = lock.is_cockpit;
        lock.surface_audio.mix_into(out, frames, vol, is_cockpit);
    }
    if !lock.paused && lock.codriver_volume > 0.0 {
        let codriver_vol = lock.codriver_volume;
        lock.speech.mix_into(out, frames, codriver_vol);
    }
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

    #[test]
    fn speech_queue_plays_sequentially_with_independent_volume() {
        let mut player = SpeechPlayer {
            active: true,
            ..SpeechPlayer::default()
        };
        let clip_a = Arc::new(vec![[16384, -16384], [16384, -16384]]);
        let clip_b = Arc::new(vec![[8192, -8192], [8192, -8192]]);
        player.enqueue_batch(vec![clip_a, clip_b]);

        let mut out = [0.0f32; 8];
        player.mix_into(&mut out, 4, 0.5);
        // First 2 frames come from clip_a scaled by 0.5 / 32768.0 => +/-0.25
        assert!((out[0] - 0.25).abs() < 1e-4);
        assert!((out[1] + 0.25).abs() < 1e-4);
        // Next 2 frames immediately advance to clip_b => +/-0.125
        assert!((out[4] - 0.125).abs() < 1e-4);
        assert!((out[5] + 0.125).abs() < 1e-4);

        // Zero codriver_volume produces silence even with active clips queued.
        out.fill(0.0);
        player.enqueue_batch(vec![Arc::new(vec![[16384, 16384]; 4])]);
        player.mix_into(&mut out, 4, 0.0);
        assert_eq!(out, [0.0; 8]);

        // Clearing queue leaves silence immediately when active volume resumes.
        player.clear();
        player.mix_into(&mut out, 4, 1.0);
        assert_eq!(out, [0.0; 8]);
    }
}
