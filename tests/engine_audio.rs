//! Deterministic unit and integration tests for authentic RBR engine audio.

#![cfg(feature = "physics-drive")]

use rerbr_sim::engine_audio::{
    self, bank_pair, chunk_len, crossfade, mix_gain, mix_scale, next_grain, slew, AudioCarConfig,
    CarAudio, Engine, EngineNoise, Rng, FIRINGS_PER_RPM,
};
use rerbr_sim::rbr_formats;
use std::sync::Arc;

#[test]
#[ignore = "private PHYSICS_DOCUMENTS: verify audio reads cannot change the deterministic trajectory"]
fn audio_telemetry_preserves_physics_samples() {
    let documents = std::env::var_os("PHYSICS_DOCUMENTS").expect("private PHYSICS_DOCUMENTS");
    std::thread::Builder::new()
        .stack_size(rerbr_sim::STACK_BYTES)
        .spawn(move || {
            let paths = rerbr_sim::inputs::Paths::documents(std::path::Path::new(&documents));
            let trial = |with_audio: bool| {
                let mut session = rerbr_sim::drive::Session::boot(&paths).unwrap();
                let mut audio =
                    CarAudio::new(synthetic_bank(), AudioCarConfig::default(), 44100, 42);
                let mut out = [[0.0; 2]; 64];
                let mut samples = Vec::new();
                for step in 0..1440 {
                    session
                        .step(rerbr_sim::drive::Control {
                            throttle: 0.7,
                            gear: Some(2),
                            flags: 4,
                            ..Default::default()
                        })
                        .unwrap();
                    if with_audio {
                        let before = session.sample();
                        audio.step_physics(&session.engine_noise(500.0), before.gear);
                        audio.render_stereo(&mut out);
                        assert_eq!(
                            before,
                            session.sample(),
                            "audio read changed physics at {step}"
                        );
                    }
                    if step % 72 == 0 {
                        samples.push(session.sample());
                    }
                }
                samples
            };
            assert_eq!(trial(false), trial(true));
        })
        .unwrap()
        .join()
        .unwrap();
}

fn synthetic_bank() -> Arc<rbr_formats::eng::Engine> {
    let rpms = [1700, 3000, 4000, 5000, 6000, 7000];
    let rates = [44100, 44100, 44100, 44100, 48000, 48000];
    let banks = rpms
        .iter()
        .zip(rates)
        .map(|(&rpm, rate)| {
            let period = (f64::from(rate) / (f64::from(rpm) * f64::from(FIRINGS_PER_RPM))) as usize;
            let samples: Vec<i16> = (0..period * 16)
                .map(|i| ((i % 100) as f32 / 100.0 * 8000.0) as i16)
                .collect();
            let cycles: Vec<f64> = (0..14).map(|k| (k * period) as f64).collect();
            let p0 = rbr_formats::eng::Part {
                rpm,
                load: 0,
                rate,
                samples: samples.clone(),
                cycles: cycles.clone(),
            };
            let p1 = rbr_formats::eng::Part {
                rpm,
                load: 100,
                rate,
                samples,
                cycles,
            };
            rbr_formats::eng::Bank {
                parts: vec![p0, p1],
            }
        })
        .collect();
    Arc::new(rbr_formats::eng::Engine {
        version: 0x101,
        banks,
        table_a: vec![],
        table_b: vec![],
    })
}

#[test]
fn slew_rate_limits_rpm_transitions_and_steps_over_table() {
    // Slew steps by at most 500 per tick.
    assert_eq!(slew(1000, 3000), 1500);
    assert_eq!(slew(1500, 3000), 2000);
    assert_eq!(slew(3000, 1000), 2500);

    // Slew steps over values in SKIPPED_RPM table (e.g. 1024 -> 1026).
    assert_eq!(slew(1000, 1024), 1026);
    assert_eq!(slew(3000, 3158), 3162);
}

#[test]
fn bank_selection_and_crossfades_across_rpm_spectrum() {
    let rpms = [1700, 3000, 4000, 5000, 6000, 7000];

    // Below lowest bank: uses bank 0 alone.
    assert_eq!(bank_pair(&rpms, 1000), (0, 0));
    let (fade, n) = crossfade(&rpms, 1000);
    assert_eq!(n, 1);
    assert_eq!(fade[0], (0, 1.0));

    // Exactly at bank 1 (3000 RPM).
    assert_eq!(bank_pair(&rpms, 3000), (1, 2));
    let (fade, n) = crossfade(&rpms, 3000);
    assert_eq!(n, 2);
    assert_eq!(fade[0], (2, 0.0));
    assert_eq!(fade[1], (1, 1.0));

    // Halfway between 4000 and 5000 RPM (4500 RPM).
    let (fade, n) = crossfade(&rpms, 4500);
    assert_eq!(n, 2);
    assert_eq!(fade[0], (3, 0.5));
    assert_eq!(fade[1], (2, 0.5));

    // Above highest bank: uses bank 5 alone.
    assert_eq!(bank_pair(&rpms, 8500), (5, 5));
    let (fade, n) = crossfade(&rpms, 8500);
    assert_eq!(n, 1);
    assert_eq!(fade[0], (5, 1.0));
}

#[test]
fn period_lengths_follow_engine_speed() {
    // 3000 RPM: 44100 / (3000 * 0.033333335) = 440.999 -> 440 samples
    assert_eq!(chunk_len(44100, 3000), 440);
    // 6000 RPM: half the period (220 samples)
    assert_eq!(chunk_len(44100, 6000), 220);
    // 48000 Hz at 6000 RPM: 48000 / (6000 * 0.033333335) = 239 samples
    assert_eq!(chunk_len(48000, 6000), 239);
}

#[test]
fn grain_counter_progresses_and_restarts_at_random() {
    let mut rng = Rng::new(12345);
    let mut counter = 0;
    let count = 32;
    for expected in 0..28 {
        assert_eq!(next_grain(&mut counter, count, &mut rng), expected);
    }
    // At threshold (28): grain is 0 and counter restarts into pseudo-random cycle.
    assert_eq!(next_grain(&mut counter, count, &mut rng), 0);
    assert!(counter >= 5 && counter < count);
}

#[test]
fn compressor_soft_knee_and_gain_curve() {
    assert_eq!(mix_gain(1000, 0), 1.125);
    assert_eq!(mix_gain(8000, 100), 2.5);

    let mut samples = [1000.0, 16384.0, 24000.0, -24000.0];
    mix_scale(&mut samples, 1.0, 1.0);
    // Linear region: 1000 remains 1000
    assert_eq!(samples[0], 1000.0);
    // Soft knee compression: 24000 is compressed
    assert!(samples[2] < 24000.0 && samples[2] > 16384.0);
    // Symmetric compression for negative polarity
    assert_eq!(samples[3], -samples[2]);
}

#[test]
fn engine_audio_continuity_and_transitions() {
    let bank = synthetic_bank();
    let mut car_audio = CarAudio::new(bank, AudioCarConfig::default(), 44100, 999);

    // Accelerating from 1500 to 5000 RPM over multiple frames.
    for step in 0..20 {
        let rpm = 1500.0 + step as f32 * 200.0;
        let noise = EngineNoise::from_rpm_throttle(rpm, 1.0, 500.0);
        car_audio.step_physics(&noise, 2);
        let mut buffer = vec![[0.0f32; 2]; 256];
        car_audio.render_stereo(&mut buffer);

        // Verify audio buffer properties: finite, bounded, non-empty.
        assert!(buffer.iter().all(|f| f[0].is_finite() && f[1].is_finite()));
        assert!(buffer
            .iter()
            .all(|f| f[0].abs() <= 1.0 && f[1].abs() <= 1.0));
    }

    // Deceleration off throttle (overrun).
    for _ in 0..10 {
        let noise = EngineNoise::from_rpm_throttle(2500.0, 0.0, 500.0);
        car_audio.step_physics(&noise, 2);
        let mut buffer = vec![[0.0f32; 2]; 256];
        car_audio.render_stereo(&mut buffer);
        assert!(buffer.iter().all(|f| f[0].is_finite() && f[1].is_finite()));
    }
}

#[test]
fn cylinder_misfire_modulates_gain() {
    let bank = synthetic_bank();
    let mut eng_normal = Engine::new(bank.clone(), 44100, 1);
    eng_normal.rpm = 3000;
    eng_normal.cylinders = [true; 4];
    let mut buf_normal = vec![0.0f32; 1000];
    eng_normal.render(&mut buf_normal);

    let mut eng_misfire = Engine::new(bank, 44100, 1);
    eng_misfire.rpm = 3000;
    eng_misfire.cylinders = [false, false, false, false];
    let mut buf_misfire = vec![0.0f32; 1000];
    eng_misfire.render(&mut buf_misfire);

    let energy_normal: f32 = buf_normal.iter().map(|s| s * s).sum();
    let energy_misfire: f32 = buf_misfire.iter().map(|s| s * s).sum();
    assert!(energy_misfire < energy_normal * 0.35);
}

#[test]
fn pause_and_volume_control_deterministic() {
    let bank = synthetic_bank();
    let mut car_audio = CarAudio::new(bank, AudioCarConfig::default(), 44100, 77);
    for _ in 0..5 {
        car_audio.step_physics(&EngineNoise::from_rpm_throttle(3500.0, 1.0, 500.0), 3);
    }

    // When paused: exact digital silence.
    car_audio.set_paused(true);
    let mut silent = vec![[1.0f32; 2]; 512];
    car_audio.render_stereo(&mut silent);
    assert!(silent.iter().all(|f| f[0] == 0.0 && f[1] == 0.0));

    // When unpaused and volume = 0: exact silence.
    car_audio.set_paused(false);
    car_audio.set_volume(0.0);
    car_audio.render_stereo(&mut silent);
    assert!(silent.iter().all(|f| f[0] == 0.0 && f[1] == 0.0));

    // When unpaused with positive volume: produces audible audio.
    car_audio.set_volume(0.8);
    let mut audible = vec![[0.0f32; 2]; 1024];
    car_audio.render_stereo(&mut audible);
    assert!(audible.iter().any(|f| f[0].abs() > 0.001));
}

#[test]
fn missing_assets_returns_clean_error_without_panicking() {
    let temp_empty = std::env::temp_dir().join(format!("rbr-audio-empty-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_empty);
    let vehicle = rerbr_content::VehicleDescriptor::default();

    let result = engine_audio::load_for_vehicle(&temp_empty, &vehicle);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("AUDIO MISSING"));

    let _ = std::fs::remove_dir_all(&temp_empty);
}

#[test]
#[ignore = "requires explicit private RBR_ROOT with original engine audio"]
fn private_rbr_audio_bank_parses_if_present() {
    let root = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").expect("private RBR_ROOT"));

    let vehicle = rerbr_content::VehicleDescriptor::default();
    let (bank, config) =
        engine_audio::load_for_vehicle(&root, &vehicle).expect("private bank must load");

    assert_eq!(config.engine_bank, "subaru");
    assert_eq!(bank.banks.len(), 6);
    let total_parts: usize = bank.banks.iter().map(|b| b.parts.len()).sum();
    assert_eq!(total_parts, 12);

    let rates: Vec<u32> = bank
        .banks
        .iter()
        .flat_map(|b| b.parts.iter().map(|p| p.rate))
        .collect();
    assert!(rates.contains(&44100));
    assert!(rates.contains(&48000));
    let mut audio = CarAudio::new(bank, config, 44100, 42);
    for _ in 0..8 {
        audio.step_physics(&EngineNoise::from_rpm_throttle(3500.0, 0.8, 500.0), 2);
    }
    let mut samples = [[0.0; 2]; 4096];
    audio.render_stereo(&mut samples);
    assert!(samples
        .iter()
        .flatten()
        .all(|s| s.is_finite() && s.abs() <= 1.0));
    assert!(
        samples.iter().flatten().any(|s| s.abs() > 0.0001),
        "private bank rendered silence"
    );
}
