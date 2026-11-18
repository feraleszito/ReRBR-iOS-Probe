//! Pure-Rust FMOD Studio `.bank` synthesizer for RSF / NGP rally cars.
//!
//! Evaluates parsed `fmod::Bank` events (`engine_int`, `engine_ext`, `turbo`,
//! `ignition`, `gear_int`, `gear_ext`, `transmission`), parameter sheets,
//! `AutoPitch`, `Automation` curves, and `Bus` gain hierarchies using decoded
//! PCM16 and FSB5-Vorbis samples. Zero heap allocations inside `render_stereo`.

use std::path::Path;
use std::sync::Arc;

use rerbr_sim::rbr_formats::fmod::{self, Bank, Layer, Property};

use super::{fmod_audit, fsb5_vorbis_setup, vorbis};

const MAX_ACTIVE_ONESHOTS: usize = 8;

#[derive(Debug, Clone)]
pub struct FmodAudioStats {
    pub bank_path: String,
    pub total_samples: usize,
    pub decoded_samples: usize,
    pub failed_samples: usize,
    pub active_events: Vec<String>,
    pub engine_int_layers: usize,
    pub engine_ext_layers: usize,
    pub turbo_layers: usize,
    pub last_rpm: f32,
    pub last_throttle: f32,
    pub last_boost: f32,
    pub telemetry_updates: u64,
    pub rendered_frames: u64,
    pub peak_output: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FmodTelemetryInput {
    pub rpm: f32,
    pub throttle: f32,
    pub brake: f32,
    pub speed_mps: f32,
    pub gear: i32,
    pub wheel_speed_mps: f32,
}

#[derive(Clone)]
struct DecodedSample {
    rate: u32,
    pcm: Arc<Vec<[f32; 2]>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EventRole {
    EngineInt,
    EngineExt,
    Turbo,
    Transmission,
    Ignition,
    GearInt,
    GearExt,
    BlowoffInt,
    BlowoffExt,
    BackfireInt,
    BackfireExt,
}

struct LoopVoice {
    layer: Layer,
    role: EventRole,
    playlist_cursor: usize,
    active_sample: usize,
    play_pos: f64,
    current_gain: f32,
    target_gain: f32,
    current_pitch: f32,
    target_pitch: f32,
    was_in_region: bool,
    cooldown_samples: usize,
}

struct OneShotVoice {
    active: bool,
    sample_idx: usize,
    play_pos: f64,
    gain: f32,
    pitch: f32,
}

impl Default for OneShotVoice {
    fn default() -> Self {
        Self {
            active: false,
            sample_idx: 0,
            play_pos: 0.0,
            gain: 0.0,
            pitch: 1.0,
        }
    }
}

struct BankBusInfo {
    parent: Option<usize>,
    volume_db: f32,
    automations: Vec<fmod::Automation>,
}

/// Real-time FMOD vehicle audio engine.
pub struct FmodVehicleAudio {
    bank_path: String,
    output_sample_rate: u32,
    samples: Vec<Option<DecodedSample>>,
    buses: Vec<BankBusInfo>,
    param_names_lower: Vec<String>,
    param_ranges: Vec<(f32, f32)>,
    param_values: Vec<f32>,
    loop_voices: Vec<LoopVoice>,
    oneshot_layers: Vec<(EventRole, Layer, usize)>,
    oneshots: [OneShotVoice; MAX_ACTIVE_ONESHOTS],
    cockpit_mix: f32,
    telemetry: FmodTelemetryInput,
    prev_gear: i32,
    prev_throttle: f32,
    boost_state: f32,
    fluctuation_state: f32,
    fluctuation_decay_per_frame: f32,
    exhaustpop_state: f32,
    ignition_pending: bool,
    bov_cooldown_frames: usize,
    gear_cooldown_frames: usize,
    total_samples: usize,
    decoded_samples: usize,
    failed_samples: usize,
    active_events: Vec<String>,
    engine_int_layers: usize,
    engine_ext_layers: usize,
    turbo_layers: usize,
    telemetry_updates: u64,
    rendered_frames: u64,
    peak_output: f32,
}

impl FmodVehicleAudio {
    pub fn load_from_documents(
        documents: &Path,
        bank_rel: &str,
        output_sample_rate: u32,
    ) -> Result<Self, String> {
        let rbr_root = documents.join("RBR");
        let roots = [rbr_root.as_path(), documents];
        let report = fmod_audit::inspect_bank_with_roots(&roots, bank_rel);
        if !report.is_playable() {
            return Err(report.summary());
        }
        let bank_file = report
            .resolved_bank_path
            .ok_or_else(|| "FMOD: falta ruta .bank resuelta".to_string())?;
        let str_file = report
            .resolved_strings_path
            .ok_or_else(|| "FMOD: falta ruta .strings.bank resuelta".to_string())?;

        let bank_bytes = std::fs::read(&bank_file).map_err(|e| format!("FMOD read bank: {e}"))?;
        let str_bytes = std::fs::read(&str_file).map_err(|e| format!("FMOD read strings: {e}"))?;
        let bank = fmod::parse(bank_bytes, &str_bytes).map_err(|e| format!("FMOD parse: {e}"))?;

        Self::from_parsed_bank(bank, &roots, bank_rel, output_sample_rate)
    }

    pub fn from_parsed_bank(
        bank: Bank,
        roots: &[&Path],
        bank_rel: &str,
        output_sample_rate: u32,
    ) -> Result<Self, String> {
        let total_samples = bank.samples.len();
        let mut decoded_samples = 0usize;
        let mut failed_samples = 0usize;
        let mut samples = Vec::with_capacity(total_samples);

        for sample in &bank.samples {
            let raw = bank.sample_data(sample);
            let decoded = match sample.codec {
                fmod::VORBIS => decode_vorbis_sample(roots, sample, raw),
                fmod::PCM16 => decode_pcm16_sample(sample, raw),
                other => Err(format!("codec FSB5 no soportado ({other})")),
            };
            match decoded {
                Ok(pcm) if !pcm.is_empty() => {
                    decoded_samples += 1;
                    samples.push(Some(DecodedSample {
                        rate: sample.rate.max(8000),
                        pcm: Arc::new(pcm),
                    }));
                }
                _ => {
                    failed_samples += 1;
                    samples.push(None);
                }
            }
        }

        if decoded_samples == 0 {
            return Err(format!(
                "FMOD: 0/{total_samples} muestras decodificadas en {bank_rel}"
            ));
        }

        let buses: Vec<BankBusInfo> = bank
            .buses
            .iter()
            .map(|b| BankBusInfo {
                parent: b.parent,
                volume_db: b.volume_db,
                automations: b.automations.clone(),
            })
            .collect();

        let param_names_lower: Vec<String> = bank
            .params
            .iter()
            .map(|p| p.name.trim().to_ascii_lowercase())
            .collect();
        let param_ranges: Vec<(f32, f32)> = bank.params.iter().map(|p| (p.min, p.max)).collect();
        let mut param_values: Vec<f32> = bank
            .params
            .iter()
            .map(|p| default_param_value(&p.name, p.min, p.max))
            .collect();
        if param_values.len() != bank.params.len() {
            param_values.resize(bank.params.len(), 0.0);
        }

        let mut loop_voices = Vec::new();
        let mut oneshot_layers = Vec::new();
        let mut engine_int_layers = 0usize;
        let mut engine_ext_layers = 0usize;
        let mut turbo_layers = 0usize;
        let mut active_events = Vec::new();
        let mut fluctuation_length = 0.28f32;

        for ev in &bank.events {
            let Some(role) = classify_event(&ev.name) else {
                continue;
            };
            if !ev.layers.is_empty() {
                let short = ev.name.strip_prefix("event:/").unwrap_or(&ev.name);
                if !active_events.iter().any(|e| e == short) {
                    active_events.push(short.to_string());
                }
            }
            if matches!(role, EventRole::EngineInt | EventRole::EngineExt) {
                if let Some(len) = ev.fluctuation_length {
                    if len > 0.02 {
                        fluctuation_length = len;
                    }
                }
            }
            for (layer_idx, layer) in ev.layers.iter().enumerate() {
                let first_playable = layer
                    .samples
                    .iter()
                    .copied()
                    .find(|&idx| samples.get(idx).and_then(|s| s.as_ref()).is_some());
                let Some(active_sample) = first_playable else {
                    continue;
                };
                match role {
                    EventRole::EngineInt => engine_int_layers += 1,
                    EventRole::EngineExt => engine_ext_layers += 1,
                    EventRole::Turbo => turbo_layers += 1,
                    _ => {}
                }
                let is_continuous = layer.loop_count < 0
                    && matches!(
                        role,
                        EventRole::EngineInt
                            | EventRole::EngineExt
                            | EventRole::Turbo
                            | EventRole::Transmission
                    );
                if is_continuous {
                    // Slight stagger so duplicate layers in a bank do not phase-lock cleanly.
                    let sample_len = samples[active_sample]
                        .as_ref()
                        .map_or(1, |s| s.pcm.len().max(1));
                    let start_pos = ((layer_idx * 1979) % sample_len) as f64;
                    loop_voices.push(LoopVoice {
                        layer: layer.clone(),
                        role,
                        playlist_cursor: 0,
                        active_sample,
                        play_pos: start_pos,
                        current_gain: 0.0,
                        target_gain: 0.0,
                        current_pitch: 1.0,
                        target_pitch: 1.0,
                        was_in_region: false,
                        cooldown_samples: 0,
                    });
                } else {
                    oneshot_layers.push((role, layer.clone(), 0));
                }
            }
        }

        let sr = output_sample_rate.max(11025);
        let fluctuation_decay_per_frame = 1.0 / (fluctuation_length.max(0.05) * sr as f32);

        let mut synth = Self {
            bank_path: bank_rel.replace('\\', "/"),
            output_sample_rate: sr,
            samples,
            buses,
            param_names_lower,
            param_ranges,
            param_values,
            loop_voices,
            oneshot_layers,
            oneshots: Default::default(),
            cockpit_mix: 1.0,
            telemetry: FmodTelemetryInput {
                rpm: 1050.0,
                throttle: 0.0,
                brake: 0.0,
                speed_mps: 0.0,
                gear: 1,
                wheel_speed_mps: 0.0,
            },
            prev_gear: 1,
            prev_throttle: 0.0,
            boost_state: 0.0,
            fluctuation_state: 0.0,
            fluctuation_decay_per_frame,
            exhaustpop_state: 0.0,
            ignition_pending: true,
            bov_cooldown_frames: 0,
            gear_cooldown_frames: 0,
            total_samples,
            decoded_samples,
            failed_samples,
            active_events,
            engine_int_layers,
            engine_ext_layers,
            turbo_layers,
            telemetry_updates: 0,
            rendered_frames: 0,
            peak_output: 0.0,
        };
        synth.trigger_ignition();
        synth.recompute_targets();
        Ok(synth)
    }

    pub fn set_sample_rate(&mut self, output_sample_rate: u32) {
        self.output_sample_rate = output_sample_rate.max(11025);
        self.fluctuation_decay_per_frame = 1.0 / (0.28 * self.output_sample_rate as f32);
    }

    pub fn update_telemetry(&mut self, input: FmodTelemetryInput) {
        self.telemetry_updates += 1;
        let rpm = input.rpm.clamp(0.0, 12000.0);
        let throttle = input.throttle.clamp(0.0, 1.0);
        let brake = input.brake.clamp(0.0, 1.0);
        let gear_changed = self.telemetry_updates > 1 && input.gear != self.prev_gear;
        let throttle_lift = (self.prev_throttle - throttle) > 0.28 && rpm > 3200.0;
        let throttle_snap = (throttle - self.prev_throttle) > 0.30 && rpm > 2500.0;

        if gear_changed || throttle_snap {
            self.fluctuation_state = 1.0;
        }
        if throttle_lift {
            self.exhaustpop_state = 1.0;
        } else {
            self.exhaustpop_state *= 0.92;
        }

        // Physical turbo boost approximation from RPM and load.
        let boost_target = ((rpm - 1700.0) / 4200.0).clamp(0.0, 1.0) * (0.12 + 0.88 * throttle);
        if boost_target > self.boost_state {
            self.boost_state += (boost_target - self.boost_state) * 0.14;
        } else {
            self.boost_state += (boost_target - self.boost_state) * 0.06;
        }

        if gear_changed && self.gear_cooldown_frames == 0 {
            self.trigger_gear_shift();
            self.gear_cooldown_frames = (self.output_sample_rate / 8) as usize;
        }
        if (throttle_lift || gear_changed)
            && self.boost_state > 0.25
            && rpm > 3300.0
            && self.bov_cooldown_frames == 0
        {
            self.trigger_blowoff_and_bangs(throttle);
            self.bov_cooldown_frames = (self.output_sample_rate / 3) as usize;
        }

        self.telemetry = FmodTelemetryInput {
            rpm,
            throttle,
            brake,
            speed_mps: input.speed_mps,
            gear: input.gear,
            wheel_speed_mps: input.wheel_speed_mps,
        };
        self.prev_gear = input.gear;
        self.prev_throttle = throttle;

        if self.ignition_pending {
            self.ignition_pending = false;
        }
        self.sync_params();
        self.recompute_targets();
    }

    pub fn stats(&self) -> FmodAudioStats {
        FmodAudioStats {
            bank_path: self.bank_path.clone(),
            total_samples: self.total_samples,
            decoded_samples: self.decoded_samples,
            failed_samples: self.failed_samples,
            active_events: self.active_events.clone(),
            engine_int_layers: self.engine_int_layers,
            engine_ext_layers: self.engine_ext_layers,
            turbo_layers: self.turbo_layers,
            last_rpm: self.telemetry.rpm,
            last_throttle: self.telemetry.throttle,
            last_boost: self.boost_state,
            telemetry_updates: self.telemetry_updates,
            rendered_frames: self.rendered_frames,
            peak_output: self.peak_output,
        }
    }

    pub fn mix_into(&mut self, out: &mut [f32], frames: usize, volume: f32, is_cockpit: bool) {
        if frames == 0 || out.len() < frames * 2 {
            return;
        }
        self.rendered_frames = self.rendered_frames.saturating_add(frames as u64);
        let cockpit_target = if is_cockpit { 1.0f32 } else { 0.0f32 };
        let sr = f64::from(self.output_sample_rate.max(11025));

        if self.fluctuation_state > 0.001 {
            self.fluctuation_state = (self.fluctuation_state
                - self.fluctuation_decay_per_frame * frames as f32)
                .max(0.0);
            self.sync_fluctuation_only();
            self.recompute_targets();
        }
        self.bov_cooldown_frames = self.bov_cooldown_frames.saturating_sub(frames);
        self.gear_cooldown_frames = self.gear_cooldown_frames.saturating_sub(frames);

        // ~12ms gain smoothing & ~25ms pitch/view smoothing per frame.
        let gain_alpha = (1.0 - (-1.0 / (0.012 * sr as f32)).exp()).clamp(0.001, 0.25);
        let pitch_alpha = (1.0 - (-1.0 / (0.015 * sr as f32)).exp()).clamp(0.001, 0.25);
        let view_alpha = (1.0 - (-1.0 / (0.035 * sr as f32)).exp()).clamp(0.001, 0.25);

        let master = volume.clamp(0.0, 2.0) * 0.42;

        for frame in 0..frames {
            self.cockpit_mix += (cockpit_target - self.cockpit_mix) * view_alpha;
            let mut sum_l = 0.0f32;
            let mut sum_r = 0.0f32;

            for voice in &mut self.loop_voices {
                voice.cooldown_samples = voice.cooldown_samples.saturating_sub(1);
                let role_gain = match voice.role {
                    EventRole::EngineInt => self.cockpit_mix,
                    EventRole::EngineExt => (1.0 - self.cockpit_mix) * 2.0,
                    EventRole::Turbo => 0.55 + 0.35 * self.cockpit_mix,
                    EventRole::Transmission => 0.45 + 0.30 * self.cockpit_mix,
                    _ => 0.0,
                };
                let desired_gain = voice.target_gain * role_gain;
                voice.current_gain += (desired_gain - voice.current_gain) * gain_alpha;
                voice.current_pitch += (voice.target_pitch - voice.current_pitch) * pitch_alpha;

                if voice.current_gain < 0.0005 {
                    continue;
                }
                let Some(decoded) = self
                    .samples
                    .get(voice.active_sample)
                    .and_then(|s| s.as_ref())
                else {
                    continue;
                };
                let [sl, sr_sample] = sample_loop_linear(&decoded.pcm, voice.play_pos);
                sum_l += sl * voice.current_gain;
                sum_r += sr_sample * voice.current_gain;

                let step = f64::from(voice.current_pitch) * (f64::from(decoded.rate) / sr);
                voice.play_pos += step;
                let len = decoded.pcm.len() as f64;
                if voice.play_pos >= len {
                    voice.play_pos %= len.max(1.0);
                }
            }

            for os in &mut self.oneshots {
                if !os.active {
                    continue;
                }
                let Some(decoded) = self.samples.get(os.sample_idx).and_then(|s| s.as_ref()) else {
                    os.active = false;
                    continue;
                };
                if os.play_pos as usize >= decoded.pcm.len() {
                    os.active = false;
                    continue;
                }
                let [sl, sr_sample] = sample_oneshot_linear(&decoded.pcm, os.play_pos);
                sum_l += sl * os.gain;
                sum_r += sr_sample * os.gain;
                let step = f64::from(os.pitch) * (f64::from(decoded.rate) / sr);
                os.play_pos += step;
                if os.play_pos as usize >= decoded.pcm.len() {
                    os.active = false;
                }
            }

            // Soft tanh limiter preserves authentic peak punch without harsh digital clipping.
            let out_l = (sum_l * master).tanh();
            let out_r = (sum_r * master).tanh();
            let peak = out_l.abs().max(out_r.abs());
            if peak > self.peak_output {
                self.peak_output = peak;
            }
            out[frame * 2] = (out[frame * 2] + out_l).clamp(-1.0, 1.0);
            out[frame * 2 + 1] = (out[frame * 2 + 1] + out_r).clamp(-1.0, 1.0);
        }
    }

    fn sync_params(&mut self) {
        let speed_kmh = (self.telemetry.speed_mps * 3.6).abs();
        let wheel_kmh = (self.telemetry.wheel_speed_mps * 3.6).abs();
        for (idx, name) in self.param_names_lower.iter().enumerate() {
            let (min, max) = self.param_ranges[idx];
            let raw = match name.as_str() {
                "rpms" | "rpm" => self.telemetry.rpm,
                "throttle" | "load" => self.telemetry.throttle,
                "brake" => self.telemetry.brake,
                "boost" | "turbo" => self.boost_state,
                "fluctuation" => self.fluctuation_state,
                "gear" => self.telemetry.gear as f32,
                "car_speed" | "speed" => speed_kmh,
                "wheel_speed" => wheel_kmh,
                "exhaustpop" => self.exhaustpop_state,
                "distance" => 1.5,
                "event cone angle" => 122.0,
                "state" | "valve" | "battery" | "startup" => 1.0,
                "suspension_damage" => 0.0,
                _ => self.param_values[idx],
            };
            self.param_values[idx] = raw.clamp(min, max);
        }
    }

    fn sync_fluctuation_only(&mut self) {
        for (idx, name) in self.param_names_lower.iter().enumerate() {
            if name == "fluctuation" {
                let (min, max) = self.param_ranges[idx];
                self.param_values[idx] = self.fluctuation_state.clamp(min, max);
            }
        }
    }

    fn recompute_targets(&mut self) {
        for voice in &mut self.loop_voices {
            let in_region = regions_active(&voice.layer, &self.param_values);
            if !in_region {
                voice.target_gain = 0.0;
                voice.was_in_region = false;
                continue;
            }
            if !voice.was_in_region && !voice.layer.samples.is_empty() {
                // Multi-instrument playlist cycles when re-entering its sheet region.
                voice.playlist_cursor = (voice.playlist_cursor + 1) % voice.layer.samples.len();
                let candidate = voice.layer.samples[voice.playlist_cursor];
                if self
                    .samples
                    .get(candidate)
                    .and_then(|s| s.as_ref())
                    .is_some()
                {
                    voice.active_sample = candidate;
                }
            }
            voice.was_in_region = true;

            let mut total_db = voice.layer.volume_db;
            let mut linear_gain = 1.0f32;
            let mut total_semi = voice.layer.semitones;

            for auto in &voice.layer.automations {
                if let Some(&pval) = self.param_values.get(auto.param) {
                    let is_distance = self
                        .param_names_lower
                        .get(auto.param)
                        .is_some_and(|n| n == "distance");
                    match auto.property {
                        Property::VolumeDb => {
                            total_db += evaluate_volume_curve(&auto.points, pval, !is_distance);
                        }
                        Property::Gain => {
                            linear_gain *= evaluate_curve(&auto.points, pval).clamp(0.0, 2.0);
                        }
                        Property::Semitones => total_semi += evaluate_curve(&auto.points, pval),
                        Property::Effect(_) => {}
                    }
                }
            }

            // Walk layer bus hierarchy (skip distance-only attenuation if huge negative outside event).
            let mut bus_cursor = voice.layer.bus;
            let mut depth = 0;
            while let Some(bi) = bus_cursor {
                if depth >= 16 {
                    break;
                }
                depth += 1;
                let Some(bus) = self.buses.get(bi) else {
                    break;
                };
                total_db += bus.volume_db;
                for auto in &bus.automations {
                    if let Some(&pval) = self.param_values.get(auto.param) {
                        if auto.property == Property::VolumeDb {
                            let is_distance = self
                                .param_names_lower
                                .get(auto.param)
                                .is_some_and(|n| n == "distance");
                            total_db += evaluate_volume_curve(&auto.points, pval, !is_distance);
                        }
                    }
                }
                bus_cursor = bus.parent;
            }

            let gain = if total_db <= -79.5 {
                0.0
            } else {
                db_to_linear(total_db) * linear_gain
            };

            let auto_pitch = if let Some(ap) = voice.layer.auto_pitch {
                let pval = self.param_values.get(ap.param).copied().unwrap_or(0.0);
                if ap.reference > 1e-4 {
                    ap.min + (1.0 - ap.min) * (pval / ap.reference)
                } else {
                    1.0
                }
            } else {
                1.0
            };

            let pitch = (auto_pitch * 2.0f32.powf(total_semi / 12.0)).clamp(0.15, 4.0);
            voice.target_gain = gain.clamp(0.0, 2.0);
            voice.target_pitch = pitch;
        }
    }

    fn trigger_ignition(&mut self) {
        self.trigger_role_oneshot(EventRole::Ignition, 0.70, 1.0);
    }

    fn trigger_gear_shift(&mut self) {
        let preferred = if self.cockpit_mix >= 0.5 {
            EventRole::GearInt
        } else {
            EventRole::GearExt
        };
        if !self.trigger_role_oneshot(preferred, 0.65, 1.0) {
            let fallback = if preferred == EventRole::GearInt {
                EventRole::GearExt
            } else {
                EventRole::GearInt
            };
            self.trigger_role_oneshot(fallback, 0.55, 1.0);
        }
    }

    fn trigger_blowoff_and_bangs(&mut self, throttle: f32) {
        let cockpit = self.cockpit_mix >= 0.5;
        let bov_role = if cockpit {
            EventRole::BlowoffInt
        } else {
            EventRole::BlowoffExt
        };
        if !self.trigger_role_oneshot(bov_role, 0.55, 1.0) {
            // Many RSF banks (including Focus_Mk2_WRC) place BOV1..3 & Bangs inside EngineExt/Backfire!
            for idx in 0..self.oneshot_layers.len() {
                let (role, layer, cursor) = &mut self.oneshot_layers[idx];
                if !matches!(
                    *role,
                    EventRole::EngineExt | EventRole::BackfireInt | EventRole::BackfireExt
                ) {
                    continue;
                }
                if layer.samples.is_empty() || !regions_active(layer, &self.param_values) {
                    continue;
                }
                let s_idx = layer.samples[*cursor % layer.samples.len()];
                *cursor = (*cursor + 1) % layer.samples.len();
                let gain = (db_to_linear(layer.volume_db) * (0.35 + (1.0 - throttle) * 0.35))
                    .clamp(0.05, 0.75);
                Self::push_oneshot_slot(&mut self.oneshots, &self.samples, s_idx, gain, 1.0);
            }
        }
    }

    fn trigger_role_oneshot(&mut self, want: EventRole, extra_scale: f32, pitch: f32) -> bool {
        let mut triggered = false;
        for (role, layer, cursor) in &mut self.oneshot_layers {
            if *role != want || layer.samples.is_empty() {
                continue;
            }
            let s_idx = layer.samples[*cursor % layer.samples.len()];
            *cursor = (*cursor + 1) % layer.samples.len();
            let gain = (db_to_linear(layer.volume_db) * extra_scale).clamp(0.0, 1.0);
            if gain > 0.005 {
                Self::push_oneshot_slot(&mut self.oneshots, &self.samples, s_idx, gain, pitch);
                triggered = true;
            }
        }
        triggered
    }

    fn push_oneshot_slot(
        slots: &mut [OneShotVoice; MAX_ACTIVE_ONESHOTS],
        samples: &[Option<DecodedSample>],
        sample_idx: usize,
        gain: f32,
        pitch: f32,
    ) {
        if samples.get(sample_idx).and_then(|s| s.as_ref()).is_none() {
            return;
        }
        let target = if let Some(free) = slots.iter_mut().find(|s| !s.active) {
            free
        } else {
            &mut slots[0]
        };
        *target = OneShotVoice {
            active: true,
            sample_idx,
            play_pos: 0.0,
            gain,
            pitch,
        };
    }
}

fn classify_event(name: &str) -> Option<EventRole> {
    let lower = name.trim().to_ascii_lowercase();
    let leaf = lower.rsplit('/').next().unwrap_or(&lower);
    match leaf {
        "engine_int" => Some(EventRole::EngineInt),
        "engine_ext" => Some(EventRole::EngineExt),
        "turbo" => Some(EventRole::Turbo),
        "transmission" => Some(EventRole::Transmission),
        "ignition" => Some(EventRole::Ignition),
        "gear_int" => Some(EventRole::GearInt),
        "gear_ext" => Some(EventRole::GearExt),
        "blowoff_int" => Some(EventRole::BlowoffInt),
        "blowoff_ext" => Some(EventRole::BlowoffExt),
        "backfire_int" => Some(EventRole::BackfireInt),
        "backfire_ext" => Some(EventRole::BackfireExt),
        _ => None,
    }
}

fn default_param_value(name: &str, min: f32, max: f32) -> f32 {
    let n = name.trim().to_ascii_lowercase();
    let val: f32 = match n.as_str() {
        "rpms" | "rpm" => 1050.0,
        "throttle" | "load" | "brake" | "boost" | "turbo" | "fluctuation" => 0.0,
        "distance" => 1.5,
        "event cone angle" => 122.0,
        "state" | "valve" | "battery" | "startup" => 1.0,
        _ => 0.0,
    };
    val.clamp(min, max)
}

fn regions_active(layer: &Layer, params: &[f32]) -> bool {
    layer.regions.iter().all(|r| {
        params
            .get(r.param)
            .is_some_and(|&v| v >= r.start - 1e-4 && v <= r.end + 1e-4)
    })
}

fn evaluate_volume_curve(points: &[[f32; 2]], x: f32, amplitude_fades: bool) -> f32 {
    if points.is_empty() {
        return 0.0;
    }
    if x <= points[0][0] {
        return points[0][1];
    }
    let last = points[points.len() - 1];
    if x >= last[0] {
        return last[1];
    }
    for win in points.windows(2) {
        let [x0, y0] = win[0];
        let [x1, y1] = win[1];
        if x <= x1 {
            let span = x1 - x0;
            if span.abs() < 1e-6 {
                return y1;
            }
            let t = ((x - x0) / span).clamp(0.0, 1.0);
            // FMOD Studio `CURV` stores per-segment shape curvature (e.g. equal-power / amplitude
            // faders for On/Off-throttle crossfades and cone-angle attenuation). Since rbr-formats
            // exposes only the endpoints (y in dB), interpolating wide fades (>= 18 dB dropping
            // below -18 dB) linearly in amplitude prevents a -25 dB mid-throttle dip/silence hole.
            if amplitude_fades && (y1 - y0).abs() >= 18.0 && y0.min(y1) <= -18.0 {
                let g0 = db_to_linear(y0);
                let g1 = db_to_linear(y1);
                let g = g0 + (g1 - g0) * t;
                if g <= 1e-4 {
                    return -80.0;
                }
                return 20.0 * g.log10();
            }
            return y0 + (y1 - y0) * t;
        }
    }
    last[1]
}

fn evaluate_curve(points: &[[f32; 2]], x: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }
    if x <= points[0][0] {
        return points[0][1];
    }
    let last = points[points.len() - 1];
    if x >= last[0] {
        return last[1];
    }
    for win in points.windows(2) {
        let [x0, y0] = win[0];
        let [x1, y1] = win[1];
        if x <= x1 {
            let span = x1 - x0;
            if span.abs() < 1e-6 {
                return y1;
            }
            let t = ((x - x0) / span).clamp(0.0, 1.0);
            return y0 + (y1 - y0) * t;
        }
    }
    last[1]
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    if db <= -79.5 {
        0.0
    } else {
        10.0f32.powf(db.clamp(-80.0, 24.0) / 20.0)
    }
}

#[inline]
fn sample_loop_linear(pcm: &[[f32; 2]], pos: f64) -> [f32; 2] {
    let len = pcm.len();
    if len == 0 {
        return [0.0, 0.0];
    }
    let i0 = (pos as usize) % len;
    let i1 = if i0 + 1 < len { i0 + 1 } else { 0 };
    let frac = (pos - pos.floor()) as f32;
    let a = pcm[i0];
    let b = pcm[i1];
    [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
}

#[inline]
fn sample_oneshot_linear(pcm: &[[f32; 2]], pos: f64) -> [f32; 2] {
    let len = pcm.len();
    let i0 = pos as usize;
    if i0 >= len {
        return [0.0, 0.0];
    }
    let i1 = (i0 + 1).min(len - 1);
    let frac = (pos - pos.floor()) as f32;
    let a = pcm[i0];
    let b = pcm[i1];
    [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
}

fn decode_vorbis_sample(
    roots: &[&Path],
    sample: &fmod::Sample,
    raw: &[u8],
) -> Result<Vec<[f32; 2]>, String> {
    let crc = sample
        .vorbis_crc
        .ok_or_else(|| format!("muestra {} sin CRC Vorbis", sample.name))?;
    let setup_storage: Vec<u8>;
    let setup = if let Some(builtin) = fsb5_vorbis_setup::setup_header_for_crc(crc) {
        builtin
    } else if let Some(ext) = fmod_audit::find_external_vorbis_setup(roots, crc) {
        setup_storage = ext;
        &setup_storage
    } else {
        return Err(format!("CRC Vorbis {crc:#010x} no disponible"));
    };
    vorbis::decode_fsb5_vorbis_to_stereo(
        raw,
        sample.channels as usize,
        sample.rate,
        sample.frames as usize,
        setup,
    )
}

fn decode_pcm16_sample(sample: &fmod::Sample, raw: &[u8]) -> Result<Vec<[f32; 2]>, String> {
    let ch = usize::from(sample.channels.max(1));
    let mut out = Vec::with_capacity(sample.frames as usize);
    let norm = 1.0f32 / 32768.0f32;
    for chunk in raw.chunks_exact(ch * 2) {
        let l = f32::from(i16::from_le_bytes([chunk[0], chunk[1]])) * norm;
        let r = if ch >= 2 {
            f32::from(i16::from_le_bytes([chunk[2], chunk[3]])) * norm
        } else {
            l
        };
        out.push([l, r]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_2006_fmod_synth_responds_to_rpm_load_and_gear_shifts() {
        let rbr = Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("AudioFMOD/Focus_Mk2_WRC.bank").is_file() {
            return;
        }
        let bank_bytes = std::fs::read(rbr.join("AudioFMOD/Focus_Mk2_WRC.bank")).unwrap();
        let str_bytes = std::fs::read(rbr.join("AudioFMOD/Focus_Mk2_WRC.strings.bank")).unwrap();
        let bank = fmod::parse(bank_bytes, &str_bytes).unwrap();
        let mut synth =
            FmodVehicleAudio::from_parsed_bank(bank, &[rbr], "AudioFMOD/Focus_Mk2_WRC.bank", 44100)
                .unwrap();

        let stats = synth.stats();
        assert_eq!(stats.decoded_samples, 44);
        assert_eq!(stats.failed_samples, 0);
        assert!(stats.engine_int_layers > 0);
        assert!(stats.engine_ext_layers > 0);
        assert!(stats.turbo_layers > 0);

        let measure_rms =
            |s: &mut FmodVehicleAudio, telemetry: FmodTelemetryInput, cockpit: bool| {
                s.update_telemetry(telemetry);
                let mut buf = vec![0.0f32; 4096 * 2];
                // Warm smoothing filters across a couple of callbacks, then measure steady output.
                s.mix_into(&mut buf, 4096, 1.0, cockpit);
                buf.fill(0.0);
                s.mix_into(&mut buf, 4096, 1.0, cockpit);
                assert!(buf.iter().all(|v| v.is_finite()));
                let energy: f32 = buf.iter().map(|v| v * v).sum();
                (energy / buf.len() as f32).sqrt()
            };

        let idle_rms = measure_rms(
            &mut synth,
            FmodTelemetryInput {
                rpm: 1050.0,
                throttle: 0.0,
                brake: 0.0,
                speed_mps: 0.0,
                gear: 1,
                wheel_speed_mps: 0.0,
            },
            true,
        );
        let load_mid_rms = measure_rms(
            &mut synth,
            FmodTelemetryInput {
                rpm: 4200.0,
                throttle: 0.9,
                brake: 0.0,
                speed_mps: 28.0,
                gear: 3,
                wheel_speed_mps: 28.0,
            },
            true,
        );
        let load_high_rms = measure_rms(
            &mut synth,
            FmodTelemetryInput {
                rpm: 7100.0,
                throttle: 1.0,
                brake: 0.0,
                speed_mps: 48.0,
                gear: 4,
                wheel_speed_mps: 48.0,
            },
            false,
        );
        assert!(idle_rms > 0.005, "idle RMS too low: {idle_rms}");
        assert!(load_mid_rms > 0.01, "mid load RMS too low: {load_mid_rms}");
        assert!(
            load_high_rms > 0.01,
            "high load ext RMS too low: {load_high_rms}"
        );
        assert!(
            load_mid_rms > idle_rms * 1.15 || load_high_rms > idle_rms * 1.15,
            "expected higher RMS under load than idle: idle={idle_rms:.4} mid={load_mid_rms:.4} high={load_high_rms:.4}"
        );

        let final_stats = synth.stats();
        assert!(final_stats.rendered_frames > 0);
        assert!(final_stats.peak_output > 0.01);
        eprintln!(
            "FOCUS FMOD SYNTH: decoded={}/{} idle_rms={:.4} mid_rms={:.4} high_ext_rms={:.4} peak={:.4}",
            final_stats.decoded_samples,
            final_stats.total_samples,
            idle_rms,
            load_mid_rms,
            load_high_rms,
            final_stats.peak_output
        );
    }

    #[test]
    fn focus_2006_fmod_matrix_and_overrun_have_no_silence_zones() {
        let rbr = Path::new(r"C:\Richard Burns Rally");
        let focus_bank = rbr.join("AudioFMOD/Focus_Mk2_WRC.bank");
        if !focus_bank.is_file() {
            return;
        }
        for (bank_name, expected_samples) in [
            ("AudioFMOD/Focus_Mk2_WRC.bank", 44usize),
            ("AudioFMOD/306_maxi_B.bank", 67usize),
        ] {
            if !rbr.join(bank_name).is_file() {
                continue;
            }
            let mut synth = FmodVehicleAudio::load_from_documents(rbr, bank_name, 44100).unwrap();
            let stats = synth.stats();
            assert_eq!(stats.decoded_samples, expected_samples);
            assert!(!stats.active_events.is_empty());

            let rpms = [2000.0f32, 4000.0, 6000.0];
            let throttles = [0.0f32, 0.25, 0.50, 0.75, 1.0];
            for cockpit in [true, false] {
                for &rpm in &rpms {
                    for &thr in &throttles {
                        synth.update_telemetry(FmodTelemetryInput {
                            rpm,
                            throttle: thr,
                            brake: 0.0,
                            speed_mps: rpm * 0.008,
                            gear: 3,
                            wheel_speed_mps: rpm * 0.008,
                        });
                        let mut buf = vec![0.0f32; 2048 * 2];
                        for _ in 0..4 {
                            buf.fill(0.0);
                            synth.mix_into(&mut buf, 2048, 1.0, cockpit);
                        }
                        let rms =
                            (buf.iter().map(|v| v * v).sum::<f32>() / buf.len() as f32).sqrt();
                        assert!(
                            rms.is_finite() && rms >= 0.012,
                            "silence dip in {bank_name} cockpit={cockpit} rpm={rpm} thr={thr}: rms={rms:.5}"
                        );
                    }
                }
            }

            // Continuous acceleration -> throttle release / overrun sweep from 6500 down to 1200 RPM.
            let mut buf = vec![0.0f32; 1024 * 2];
            for step in 0..32 {
                let (rpm, thr) = if step < 12 {
                    (1800.0 + step as f32 * 400.0, 0.55)
                } else {
                    (6500.0 - (step - 12) as f32 * 260.0, 0.0)
                };
                synth.update_telemetry(FmodTelemetryInput {
                    rpm,
                    throttle: thr,
                    brake: if thr == 0.0 { 0.3 } else { 0.0 },
                    speed_mps: rpm * 0.008,
                    gear: if step > 18 { 2 } else { 3 },
                    wheel_speed_mps: rpm * 0.008,
                });
                buf.fill(0.0);
                synth.mix_into(&mut buf, 1024, 1.0, true);
                let rms = (buf.iter().map(|v| v * v).sum::<f32>() / buf.len() as f32).sqrt();
                assert!(
                    rms.is_finite() && rms >= 0.010,
                    "silence dip during ramp/overrun step {step} (rpm={rpm}, thr={thr}) in {bank_name}: rms={rms:.5}"
                );
            }
        }
    }
}
