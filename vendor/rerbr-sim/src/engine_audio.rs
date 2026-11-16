//! Authentic Richard Burns Rally granular engine audio synthesizer and vehicle acoustics.
//!
//! Reconstructs original CEngine (vtable 0x732c64) and CCarSound (0x4b06a0):
//! - Pitch-synchronous overlap-add granular synthesis from original .eng banks.
//! - Slew-limited RPM tracking stepping over the original skipped RPM table (0x732cc8).
//! - Continuous 6-RPM-bank crossfading and 12-part load interpolation (load 0 vs load 100).
//! - Catmull-Rom cubic resampling between bank sample rates (44.1 kHz / 48 kHz) and output rate.
//! - Soft-knee compressor (0x4d99f0) above half-scale (16384) with 4:1 slope.
//! - Cockpit interior low-shelf and low-pass filtering based on Cars.ini.
//! - Safe silent fallback for missing or unreadable audio assets.

use std::f32::consts::PI;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use rbr_formats::{audio, cars, eng};

/// rad/s to rpm (0x7319a4).
pub const RPM_PER_RAD_S: f32 = 9.549_296;
/// Maximum RPM the slew target is clamped to (0x731cd4).
pub const MAX_RPM: f32 = 9000.0;
/// Safe upper ceiling for period calculation.
pub const RPM_CEILING: f32 = 20_000.0;
/// Maximum RPM movement per input step (0x4bc740).
pub const SLEW_PER_STEP: i64 = 500;
/// RPM values the slew steps over, 2 at a time (0x732cc8).
pub const SKIPPED_RPM: [u32; 80] = [
    1010, 1024, 1028, 1032, 1061, 1073, 1088, 1109, 1126, 1175, 1216, 1233, 1282, 1287, 1292, 1327, 1331, 1357, 1371,
    1403, 1406, 1409, 1415, 1526, 1542, 1551, 1660, 1694, 1734, 1835, 1882, 1923, 1963, 2023, 2048, 2064, 2077, 2176,
    2285, 2289, 2293, 2313, 2317, 2371, 2441, 2446, 2451, 2455, 2473, 2501, 2559, 2564, 2574, 2579, 2580, 2584, 2662,
    2803, 2827, 2833, 2895, 2908, 3084, 3158, 3159, 3160, 3219, 3341, 3547, 3711, 3919, 3973, 3997, 4096, 4759, 4811,
    4882, 5069, 5128, 5168,
];
/// Firings per second per RPM (4 cylinders, 4 strokes = 2 firings per revolution: 2 / 60 = 0.033333335).
pub const FIRINGS_PER_RPM: f32 = 0.033_333_335;
/// Misfiring cylinder chunk gain (0x4bc7b0).
pub const MISFIRE_GAIN: f32 = 0.25;

/// Telemetry input from the physics engine to the engine audio voice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineNoise {
    /// Target RPM after clamp to [min_rpm..RPM_CEILING].
    pub target_rpm: u32,
    /// Whether the engine has stopped below minimum idle RPM.
    pub stopped: bool,
    /// Engine load, 0 to 100 (throttle * 100).
    pub load: u32,
    /// 4 cylinder firing flags (false indicates misfire).
    pub cylinders: [bool; 4],
}

impl EngineNoise {
    /// Construct from raw 12-word physics engine sound payload.
    pub fn from_payload(p: &[u32; 12], min_rpm: f32) -> Self {
        let f = |i: usize| f32::from_bits(p[i]);
        let rpm = f(0) * RPM_PER_RAD_S;
        let clamped = rpm.clamp(min_rpm, RPM_CEILING);
        Self {
            target_rpm: clamped as u32,
            stopped: rpm < min_rpm,
            load: (f(1) * 100.0).clamp(0.0, 100.0) as u32,
            cylinders: std::array::from_fn(|i| f(8 + i) != 0.0),
        }
    }

    /// Construct directly from simulated RPM, throttle and idle RPM.
    pub fn from_rpm_throttle(rpm: f32, throttle: f32, min_rpm: f32) -> Self {
        let clamped = rpm.clamp(min_rpm, RPM_CEILING);
        Self {
            target_rpm: clamped as u32,
            stopped: rpm < min_rpm,
            load: (throttle * 100.0).clamp(0.0, 100.0) as u32,
            cylinders: [true; 4],
        }
    }
}

/// Slew rate limiter stepping over the skipped RPM table (0x4bc740).
pub fn slew(rpm: u32, target: u32) -> u32 {
    let cur = i64::from(rpm);
    let mut v = i64::from(target).clamp(cur - SLEW_PER_STEP, cur + SLEW_PER_STEP);
    let mut idx = 0;
    let mut t = i64::from(SKIPPED_RPM[0]);
    loop {
        idx += 1;
        if t == v {
            v += 2;
            idx = 0;
        }
        if t > v {
            break;
        }
        match SKIPPED_RPM.get(idx) {
            Some(&next) => t = i64::from(next),
            None => break,
        }
    }
    v.max(0) as u32
}

/// Locate lower and upper RPM banks in an ascending table (0x4bc580, 0x4bc5d0).
pub fn bank_pair(rpms: &[u32], rpm: u32) -> (usize, usize) {
    let lo = rpms.iter().rposition(|&r| r <= rpm).unwrap_or(0);
    let hi = rpms.iter().position(|&r| r > rpm).unwrap_or(rpms.len().saturating_sub(1));
    (lo, hi)
}

/// Calculate crossfade weights between adjacent RPM banks (0x4bc7b0).
pub fn crossfade(rpms: &[u32], rpm: u32) -> ([(usize, f32); 2], usize) {
    let (lo, hi) = bank_pair(rpms, rpm);
    if lo == hi {
        return ([(lo, 1.0), (0, 0.0)], 1);
    }
    let (r_lo, r_hi) = (rpms[lo] as f32, rpms[hi] as f32);
    let up = ((rpm as f32 - r_lo) / (r_hi - r_lo)).clamp(0.0, 1.0);
    ([(hi, up), (lo, 1.0 - up)], 2)
}

/// Output samples per firing period at given rate and RPM (0x4bc9c0).
pub fn chunk_len(rate: u32, rpm: u32) -> usize {
    if rpm == 0 {
        return rate as usize;
    }
    (f64::from(rate) / (f64::from(rpm) * f64::from(FIRINGS_PER_RPM))) as usize
}

/// Seeded pseudo-random generator for grain restarts.
#[derive(Debug, Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    pub fn rand(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 >> 17
    }
}

/// Advance to next grain cycle index, restarting pseudo-randomly at end of bank (0x4bbbb0).
pub fn next_grain(counter: &mut usize, count: usize, rng: &mut Rng) -> usize {
    let u = *counter;
    if u < (count.saturating_sub(3) & !3) {
        *counter = u + 1;
        u
    } else {
        *counter = (((rng.rand() as usize) % count.saturating_sub(11).max(1)) & !3) + 5;
        0
    }
}

/// Catmull-Rom cubic spline interpolation for resampled audio samples (0x4bb3e0).
#[inline]
fn cubic(s0: f32, s1: f32, s2: f32, s3: f32, fr: f32) -> f32 {
    ((((s2 * 2.0 + s0) - (s1 * 5.0 + s3) * 0.5) + ((s1 - s2) * 3.0 - s0 + s3) * 0.5 * fr) * fr + (s2 - s0) * 0.5) * fr
        + s1
}

/// Compute grain half-window width and normalization gain.
fn half_window(part: &eng::Part, row: usize, period: usize, rate: u32) -> (usize, f32) {
    let own = match (part.cycles.get(row), part.cycles.get(row + 1)) {
        (Some(&a), Some(&b)) => ((b - a) * f64::from(rate) / f64::from(part.rate)) as usize,
        _ => period,
    };
    let half = period.min(own).max(1);
    (half, (period as f32 / half as f32).sqrt())
}

/// Overlap-add grain synthesis under a Hann window.
fn add_grain(out: &mut [f32], half: usize, part: &eng::Part, row: usize, rate: u32, gain: f32) {
    let Some(&mark) = part.cycles.get(row) else {
        return;
    };
    let step = f64::from(part.rate) / f64::from(rate);
    let x = &part.samples;
    let at = |i: i64| x.get(i.clamp(0, x.len() as i64 - 1) as usize).map_or(0.0, |&s| f32::from(s));
    for (k, sample) in out.iter_mut().enumerate().take(half * 2) {
        let pos = mark - 1.0 + step * (k as f64 - half as f64);
        let p = pos.floor() as i64;
        let fr = (pos - p as f64) as f32;
        let w = (std::f64::consts::FRAC_PI_2 * k as f64 / half as f64).sin().powi(2) as f32;
        *sample += cubic(at(p - 1), at(p), at(p + 1), at(p + 2), fr) * w * gain;
    }
}

/// Gain scaling from RPM and engine load (0x4d99f0).
pub fn mix_gain(rpm: u32, load: u32) -> f32 {
    let speed = (rpm as f32).min(8000.0) * 0.000_125 * 256.0;
    ((speed + (load << 7) as f32 * 0.01) as i32 + 0x100).max(0x100) as f32 / 256.0
}

/// Soft-knee dynamic compression (-6 dB 4:1 knee above 16384).
pub fn mix_scale(samples: &mut [f32], from: f32, to: f32) {
    const KNEE: f32 = 16384.0;
    const KNEE_WIDTH: f32 = 8192.0;
    const SLOPE: f32 = 0.25;
    let bend = |a: f32| {
        let over = a - (KNEE - KNEE_WIDTH / 2.0);
        if over <= 0.0 {
            a
        } else if over < KNEE_WIDTH {
            a + (SLOPE - 1.0) * over * over / (2.0 * KNEE_WIDTH)
        } else {
            (a - KNEE) * SLOPE + KNEE
        }
    };
    let n = samples.len().max(1) as f32;
    for (i, s) in samples.iter_mut().enumerate() {
        let x = *s * (from + (to - from) * (i + 1) as f32 / n);
        *s = bend(x.abs()).copysign(x);
    }
}

/// Authentic granular engine synthesizer state.
#[derive(Debug, Clone)]
pub struct Engine {
    bank: Arc<eng::Engine>,
    rpms: Vec<u32>,
    rate: u32,
    pub rpm: u32,
    pub load: u32,
    pub cylinders: [bool; 4],
    pub running: bool,
    counters: Vec<[usize; 2]>,
    cylinder: usize,
    rng: Rng,
    left: Vec<f32>,
    overlap: Vec<f32>,
    buf: Vec<f32>,
}

impl Engine {
    pub fn new(bank: Arc<eng::Engine>, rate: u32, seed: u32) -> Self {
        // Runtime telemetry is limited to at least 500 RPM. Reserve the largest
        // overlap, output and pending period before entering the audio callback.
        let capacity = chunk_len(rate, 500).max(1) * 2;
        let rpms = bank.banks.iter().map(eng::Bank::rpm).collect();
        let counters = vec![[0; 2]; bank.banks.len()];
        Self {
            bank,
            rpms,
            rate,
            rpm: 1000,
            load: 0,
            cylinders: [true; 4],
            running: true,
            counters,
            cylinder: 0,
            rng: Rng::new(seed),
            left: Vec::with_capacity(capacity),
            overlap: Vec::with_capacity(capacity),
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Update engine target from telemetry input.
    pub fn input(&mut self, noise: &EngineNoise) {
        self.rpm = slew(self.rpm, noise.target_rpm);
        self.load = noise.load;
        self.cylinders = noise.cylinders;
        self.running = !noise.stopped;
    }

    /// Render mono 16-bit-scaled engine samples into out.
    pub fn render(&mut self, out: &mut [f32]) {
        let mut done = self.left.len().min(out.len());
        out[..done].copy_from_slice(&self.left[..done]);
        self.left.drain(..done);
        while done < out.len() {
            if !self.running || self.rpm == 0 || self.rpms.is_empty() {
                out[done..].fill(0.0);
                self.overlap.clear();
                return;
            }
            let mut chunk = self.take_chunk();
            let n = chunk.len().min(out.len() - done);
            out[done..done + n].copy_from_slice(&chunk[..n]);
            self.left.extend_from_slice(&chunk[n..]);
            chunk.clear();
            self.buf = chunk;
            done += n;
        }
    }

    fn take_chunk(&mut self) -> Vec<f32> {
        let period = chunk_len(self.rate, self.rpm.clamp(500, RPM_CEILING as u32)).max(1);
        let mut out = std::mem::take(&mut self.buf);
        out.clear();
        out.extend_from_slice(&self.overlap);
        out.resize(out.len().max(period * 2), 0.0);
        let (mut load, mut gain) = (self.load, 1.0);
        if !self.cylinders[self.cylinder] {
            load >>= 1;
            gain = MISFIRE_GAIN;
        }
        self.cylinder = (self.cylinder + 1) % 4;
        let mix = (load.min(100) as f32 * 0.01 * 256.0).trunc() / 256.0;
        let (mixes, n) = crossfade(&self.rpms, self.rpm);
        for &(b, weight) in &mixes[..n] {
            let bank = &self.bank.banks[b];
            let (Some(empty), Some(full)) = (bank.parts.first(), bank.parts.get(1).or(bank.parts.first())) else {
                continue;
            };
            for (p, part, part_gain) in [(0, empty, 1.0 - mix), (1, full, mix)] {
                let row = next_grain(&mut self.counters[b][p], part.cycles.len(), &mut self.rng);
                let (half, level) = half_window(part, row, period, self.rate);
                let grain_gain = weight * gain * part_gain * level;
                add_grain(&mut out[period - half..], half, part, row, self.rate, grain_gain);
            }
        }
        self.overlap.clear();
        self.overlap.extend_from_slice(&out[period..]);
        out.truncate(period);
        out
    }
}

/// Direct Form I biquad filter for vehicle cabin acoustics.
#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn lowpass(rate: u32, cutoff: f32) -> Self {
        let w = 2.0 * PI * cutoff.clamp(10.0, rate as f32 * 0.45) / rate as f32;
        let (sin, cos) = (w.sin(), w.cos());
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let b0 = (1.0 - cos) / 2.0;
        Self {
            b: [b0 / a0, (1.0 - cos) / a0, b0 / a0],
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn lowshelf(rate: u32, freq: f32, amp: f32) -> Self {
        let a = amp.max(0.1);
        let w = 2.0 * PI * freq.clamp(10.0, rate as f32 * 0.45) / rate as f32;
        let (sin, cos) = (w.sin(), w.cos());
        let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / 0.707 - 1.0) + 2.0).max(0.0).sqrt();
        let a0 = (a + 1.0) + (a - 1.0) * cos + 2.0 * a.sqrt() * alpha;
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos + 2.0 * a.sqrt() * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos - 2.0 * a.sqrt() * alpha);
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos);
        let a2 = (a + 1.0) + (a - 1.0) * cos - 2.0 * a.sqrt() * alpha;
        Self { b: [b0 / a0, b1 / a0, b2 / a0], a: [a1 / a0, a2 / a0], x: [0.0; 2], y: [0.0; 2] }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.b[0] * input + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x[1] = self.x[0];
        self.x[0] = input;
        self.y[1] = self.y[0];
        self.y[0] = out;
        out
    }
}

/// Cockpit interior filter applying low-shelf and low-pass curves.
#[derive(Debug, Clone)]
pub struct InsideFilter {
    lowpass: Biquad,
    lowshelf: Biquad,
}

impl InsideFilter {
    pub fn new(rate: u32, cutoff: f32, shelf_freq: f32, shelf_amp: f32) -> Self {
        Self { lowpass: Biquad::lowpass(rate, cutoff), lowshelf: Biquad::lowshelf(rate, shelf_freq, shelf_amp) }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let boosted = self.lowshelf.process(sample);
        self.lowpass.process(boosted)
    }
}

/// Parameters parsed from Cars.ini for a specific car slot.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioCarConfig {
    pub engine_bank: String,
    pub inside_cut_freq: f32,
    pub low_shelf_amp: f32,
    pub low_shelf_freq: f32,
}

impl Default for AudioCarConfig {
    fn default() -> Self {
        Self { engine_bank: "subaru".into(), inside_cut_freq: 750.0, low_shelf_amp: 2.0, low_shelf_freq: 200.0 }
    }
}

/// Helper to read a private file bounded by a maximum byte size.
fn read_file(root: &Path, rel_path: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resolved = cars::resolve(root, rel_path).ok_or_else(|| format!("AUDIO MISSING: Documents/RBR/{rel_path}"))?;
    let canonical = resolved.canonicalize().map_err(|e| e.to_string())?;
    let root_canon = root.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root_canon) {
        return Err("AUDIO: private root escape".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(canonical)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("AUDIO: file exceeds size limit".into());
    }
    Ok(bytes)
}

/// Inspect and load authentic audio configuration and engine bank for a vehicle.
pub fn load_for_vehicle(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
) -> Result<(Arc<eng::Engine>, AudioCarConfig), String> {
    let ini_bytes = read_file(root, &vehicle.audio_index, 1024 * 1024)?;
    let sections = audio::parse_ini(&ini_bytes).map_err(|e| e.to_string())?;
    let car_sound = audio::CarSound::read(&sections, vehicle.original_slot)
        .ok_or_else(|| format!("AUDIO: missing Car{} mapping in {}", vehicle.original_slot, vehicle.audio_index))?;
    if car_sound.engine.is_empty() || !car_sound.engine.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err("AUDIO: unsafe engine bank name".into());
    }
    // Use the original reader's defaults as well as its field interpretation.
    let inside_cut = car_sound.inside_cut_frequency;
    let shelf_amp = car_sound.low_shelf_amp;
    let shelf_freq = car_sound.low_shelf_frequency;
    let config = AudioCarConfig {
        engine_bank: car_sound.engine.clone(),
        inside_cut_freq: inside_cut,
        low_shelf_amp: shelf_amp,
        low_shelf_freq: shelf_freq,
    };
    let engine_rel = format!("Audio/Cars/{}.eng", car_sound.engine);
    let eng_bytes = read_file(root, &engine_rel, 16 * 1024 * 1024)?;
    let engine = eng::parse(&eng_bytes).map_err(|e| e.to_string())?;
    if !inside_cut.is_finite() || !shelf_amp.is_finite() || !shelf_freq.is_finite() {
        return Err("AUDIO: nonfinite cabin filter configuration".into());
    }
    if engine.banks.is_empty()
        || !engine.banks.windows(2).all(|banks| banks[0].rpm() < banks[1].rpm())
        || engine.banks.iter().any(|bank| {
            bank.parts.is_empty()
                || bank.parts.iter().any(|part| {
                    part.rpm == 0
                        || part.rate == 0
                        || part.samples.is_empty()
                        || part.cycles.len() < 2
                        || !part.cycles.iter().all(|value| value.is_finite())
                        || !part.cycles.windows(2).all(|values| values[0] < values[1])
                })
        })
    {
        return Err("AUDIO: unsupported engine bank data".into());
    }
    Ok((Arc::new(engine), config))
}

/// Complete authentic vehicle audio state and stereo renderer.
pub struct CarAudio {
    pub engine: Engine,
    pub filter: InsideFilter,
    pub config: AudioCarConfig,
    pub volume: f32,
    pub is_cockpit: bool,
    pub paused: bool,
    pub interrupted: bool,
    pub previous_gear: i32,
    previous_gain: f32,
    mono_scratch: Vec<f32>,
}

impl CarAudio {
    pub fn new(bank: Arc<eng::Engine>, config: AudioCarConfig, rate: u32, seed: u32) -> Self {
        let filter = InsideFilter::new(rate, config.inside_cut_freq, config.low_shelf_freq, config.low_shelf_amp);
        let engine = Engine::new(bank, rate, seed);
        Self {
            engine,
            filter,
            config,
            volume: 0.8,
            is_cockpit: false,
            paused: false,
            interrupted: false,
            previous_gear: 1, // Neutral
            previous_gain: 1.125,
            mono_scratch: vec![0.0; 4096],
        }
    }

    /// Step physics inputs (RPM, throttle, gear) into the engine audio system.
    pub fn step_physics(&mut self, noise: &EngineNoise, gear: i32) {
        self.engine.input(noise);
        self.previous_gear = gear;
    }

    pub fn set_cockpit(&mut self, cockpit: bool) {
        self.is_cockpit = cockpit;
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn set_interrupted(&mut self, interrupted: bool) {
        self.interrupted = interrupted;
    }

    /// Render interleaved stereo float frames [-1.0..1.0] for the audio output callback.
    pub fn render_stereo(&mut self, out: &mut [[f32; 2]]) {
        if out.len() > self.mono_scratch.capacity() {
            out.fill([0.0, 0.0]);
            return;
        }
        if self.paused || self.interrupted || self.volume <= 0.0 {
            out.fill([0.0, 0.0]);
            return;
        }
        let frames = out.len();
        self.mono_scratch.resize(frames, 0.0);
        self.engine.render(&mut self.mono_scratch);
        let next_gain = mix_gain(self.engine.rpm, self.engine.load);
        mix_scale(&mut self.mono_scratch, self.previous_gain, next_gain);
        self.previous_gain = next_gain;
        // Normalize 16-bit range (-32768..32767) to [-1.0..1.0], multiplied by user volume.
        let scale = (self.volume / 32768.0).clamp(0.0, 1.0);
        for (i, frame) in out.iter_mut().enumerate() {
            let mut sample = self.mono_scratch[i];
            if self.is_cockpit {
                sample = self.filter.process(sample);
            }
            let scaled = (sample * scale).clamp(-1.0, 1.0);
            *frame = [scaled, scaled];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_rpm_sweep_keeps_preallocated_render_storage() {
        let mut audio = CarAudio::new(make_test_engine(&[44100, 48000]), AudioCarConfig::default(), 44100, 42);
        let capacities = (
            audio.engine.left.capacity(),
            audio.engine.overlap.capacity(),
            audio.engine.buf.capacity(),
            audio.mono_scratch.capacity(),
        );
        let mut out = [[0.0; 2]; 4096];
        for rpm in [500, 20000, 500, 3000, 9000, 500] {
            audio.engine.rpm = rpm;
            audio.render_stereo(&mut out);
            assert!(out.iter().flatten().all(|sample| sample.is_finite() && sample.abs() <= 1.0));
            assert_eq!(
                capacities,
                (
                    audio.engine.left.capacity(),
                    audio.engine.overlap.capacity(),
                    audio.engine.buf.capacity(),
                    audio.mono_scratch.capacity()
                )
            );
        }
    }

    /// Build a synthetic deterministic engine bank for testing.
    pub fn make_test_engine(rates: &[u32]) -> Arc<eng::Engine> {
        let rpms = [1700, 3000, 4000, 5000, 6000, 7000];
        let banks = rpms
            .iter()
            .enumerate()
            .map(|(i, &rpm)| {
                let rate = rates[i % rates.len()];
                let cycle_step = (f64::from(rate) / (f64::from(rpm) * f64::from(FIRINGS_PER_RPM))) as usize;
                let samples_len = cycle_step * 20;
                let samples: Vec<i16> = (0..samples_len).map(|k| ((k % 64) as f32 / 64.0 * 10000.0) as i16).collect();
                let cycles: Vec<f64> = (0..18).map(|k| (k * cycle_step) as f64).collect();
                let p0 = eng::Part { rpm, load: 0, rate, samples: samples.clone(), cycles: cycles.clone() };
                let p1 = eng::Part { rpm, load: 100, rate, samples, cycles };
                eng::Bank { parts: vec![p0, p1] }
            })
            .collect();
        Arc::new(eng::Engine { version: 0x101, banks, table_a: vec![], table_b: vec![] })
    }

    #[test]
    fn slew_clamps_movement_to_500_and_steps_over_table() {
        assert_eq!(slew(1000, 2000), 1500);
        assert_eq!(slew(3000, 1500), 2500);
        assert_eq!(slew(1000, 1024), 1026);
        assert_eq!(slew(3000, 3158), 3162);
    }

    #[test]
    fn bank_pair_and_crossfade_sum_to_one() {
        let rpms = [1700, 3000, 4000, 5000, 6000, 7000];
        assert_eq!(bank_pair(&rpms, 1000), (0, 0));
        assert_eq!(bank_pair(&rpms, 1700), (0, 1));
        assert_eq!(bank_pair(&rpms, 3500), (1, 2));
        assert_eq!(bank_pair(&rpms, 8000), (5, 5));

        let (mix, count) = crossfade(&rpms, 3500);
        assert_eq!(count, 2);
        assert!((mix[0].1 + mix[1].1 - 1.0).abs() < 1e-6);
        assert_eq!(mix[0].0, 2); // 4000
        assert_eq!(mix[1].0, 1); // 3000
        assert_eq!(mix[0].1, 0.5);
    }

    #[test]
    fn chunk_len_is_reciprocal_to_rpm() {
        assert_eq!(chunk_len(44100, 3000), 440);
        assert_eq!(chunk_len(44100, 6000), 220);
        assert_eq!(chunk_len(48000, 1000), 1439);
    }

    #[test]
    fn engine_audio_renders_deterministically() {
        let bank = make_test_engine(&[44100, 48000]);
        let mut audio = CarAudio::new(bank, AudioCarConfig::default(), 44100, 42);
        audio.step_physics(&EngineNoise::from_rpm_throttle(3500.0, 0.8, 500.0), 2);
        let mut out1 = vec![[0.0f32; 2]; 1024];
        audio.render_stereo(&mut out1);
        assert!(out1.iter().any(|f| f[0].abs() > 0.001));
        assert!(out1.iter().all(|f| f[0].is_finite() && f[1].is_finite()));
    }

    #[test]
    fn pause_and_interruption_produce_silence() {
        let bank = make_test_engine(&[44100]);
        let mut audio = CarAudio::new(bank, AudioCarConfig::default(), 44100, 42);
        audio.step_physics(&EngineNoise::from_rpm_throttle(3000.0, 1.0, 500.0), 2);
        audio.set_paused(true);
        let mut out = vec![[1.0f32; 2]; 512];
        audio.render_stereo(&mut out);
        assert!(out.iter().all(|f| f[0] == 0.0 && f[1] == 0.0));

        audio.set_paused(false);
        audio.set_interrupted(true);
        audio.render_stereo(&mut out);
        assert!(out.iter().all(|f| f[0] == 0.0 && f[1] == 0.0));
    }

    #[test]
    fn volume_setting_scales_output() {
        let bank = make_test_engine(&[44100]);
        let mut audio = CarAudio::new(bank.clone(), AudioCarConfig::default(), 44100, 42);
        for _ in 0..5 {
            audio.step_physics(&EngineNoise::from_rpm_throttle(3000.0, 1.0, 500.0), 2);
        }
        audio.set_volume(1.0);
        let mut full = vec![[0.0f32; 2]; 2048];
        audio.render_stereo(&mut full);

        let mut audio_half = CarAudio::new(bank, AudioCarConfig::default(), 44100, 42);
        for _ in 0..5 {
            audio_half.step_physics(&EngineNoise::from_rpm_throttle(3000.0, 1.0, 500.0), 2);
        }
        audio_half.set_volume(0.5);
        let mut half = vec![[0.0f32; 2]; 2048];
        audio_half.render_stereo(&mut half);

        let full_max = full.iter().map(|f| f[0].abs()).fold(0.0f32, f32::max);
        let half_max = half.iter().map(|f| f[0].abs()).fold(0.0f32, f32::max);
        assert!(full_max > 0.0);
        assert!((half_max / full_max - 0.5).abs() < 0.05);
    }

    #[test]
    fn cockpit_filter_shapes_spectrum() {
        let mut filter = InsideFilter::new(44100, 750.0, 200.0, 2.0);
        // Test high frequency attenuation
        let mut high_freq_energy = 0.0f32;
        for i in 0..1000 {
            let sample = (2.0 * PI * 5000.0 * i as f32 / 44100.0).sin();
            let filtered = filter.process(sample);
            high_freq_energy += filtered.powi(2);
        }
        // At 5 kHz (well above 750 Hz cutoff), energy should be significantly attenuated
        assert!(high_freq_energy < 50.0);
    }
}
