#![cfg(feature = "physics-drive")]
use rerbr_probe::driving::controls::{pending_selection, Controls};
use rerbr_sim::{diagnostics::Sample, drive::Session, inputs::Paths, STACK_BYTES};

fn paths() -> Paths {
    Paths::documents(&std::path::PathBuf::from(
        std::env::var_os("PHYSICS_DOCUMENTS").expect("private PHYSICS_DOCUMENTS"),
    ))
}

fn manual() -> Vec<Sample> {
    let mut session = Session::boot(&paths()).unwrap();
    eprintln!(
        "SELECTED PHYSICS INPUT AUDIT: {:?}",
        session.physics_audit()
    );
    let ratios = session.gear_ratios().unwrap();
    assert_eq!(
        ratios.len(),
        8,
        "Corolla must expose R/N and six forward gears"
    );
    assert_eq!(ratios[1], 0.0);
    assert!(ratios[0] * ratios[2] < 0.0);
    eprintln!("CONFIGURED ORIGINAL GEAR BANDS: {ratios:?}");
    let mut controls = Controls::new();
    let mut samples = Vec::new();
    // Original start controls are inhibited briefly after the load/Driving transition.
    for _ in 0..1440 {
        session.step(Default::default()).unwrap();
    }
    // Select every gear while declutched at rest. All changes go through the real input payload.
    for gear in [2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0] {
        controls.selected_gear = gear;
        controls.touch(1, 0.9, 0.49, true, false);
        for _ in 0..720 {
            let previous = session.sample();
            let command = pending_selection(
                controls.input(0.0, f64::from(session.dt())),
                previous.commanded_gear,
            );
            session.step(command).unwrap();
        }
        let sample = session.sample();
        assert!(sample.finite());
        assert_eq!(
            sample.gear, gear as i32,
            "gear selection failed: {sample:?}"
        );
        assert_eq!(sample.payload_clutch, 1.0);
        samples.push(sample);
    }
    let start = session.sample();
    controls.clear();
    controls.touch(4, 0.9, 0.95, true, false);
    for _ in 0..1440 {
        let command = pending_selection(
            controls.input(0.0, f64::from(session.dt())),
            session.sample().commanded_gear,
        );
        session.step(command).unwrap();
    }
    let reverse = session.sample();
    let [x, y, z, w] = start.orientation;
    let forward = [
        2.0 * (w * z - x * y),
        -(1.0 - 2.0 * (x * x + z * z)),
        -2.0 * (y * z + w * x),
    ];
    let distance: f32 = (0..3)
        .map(|k| (reverse.position[k] - start.position[k]) * forward[k])
        .sum();
    assert!(
        distance < -0.5,
        "reverse did not move backwards: {distance}m, {reverse:?}"
    );
    samples.push(reverse);
    samples
}

fn loaded_sequence() -> Vec<Sample> {
    let mut session = Session::boot(&paths()).unwrap();
    let mut controls = Controls::new();
    for _ in 0..1440 {
        session.step(Default::default()).unwrap();
    }
    controls.touch(1, 0.9, 0.95, true, false);
    let mut samples = Vec::new();
    // Exercise the real input path under throttle, without any automatic RPM policy.
    for gear in [2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2] {
        controls.selected_gear = gear;
        for _ in 0..360 {
            let command = pending_selection(
                controls.input(0.0, f64::from(session.dt())),
                session.sample().commanded_gear,
            );
            session.step(command).unwrap();
        }
        let sample = session.sample();
        assert!(sample.finite());
        assert_eq!(sample.gear, gear as i32, "loaded gear request: {sample:?}");
        assert_eq!(sample.throttle_received, 1.0);
        assert_eq!(sample.payload_clutch, 0.0);
        samples.push(sample);
    }
    assert!(samples.iter().any(|sample| sample.speed() > 1.0));
    samples
}

#[test]
#[ignore = "private Documents: all forward input shifts under load; not NGP compatibility certification"]
fn every_forward_gear_up_and_down_under_throttle_repeats_exactly() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let first = loaded_sequence();
            assert_eq!(first, loaded_sequence());
            eprintln!(
                "MANUAL LOADED SEQUENCE (step, actual gear, RPM, m/s): {:?}",
                first
                    .iter()
                    .map(|s| (s.step, s.gear, s.rpm, s.speed()))
                    .collect::<Vec<_>>()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "private Documents, original physics: R/N/all six gears and deterministic clutch"]
fn all_gears_are_accepted_and_repeat_exactly() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let first = manual();
            assert_eq!(first, manual());
            eprintln!("TRANSMISSION: all 8 gear indices accepted; 13 transitions repeat exactly");
        })
        .unwrap()
        .join()
        .unwrap();
}

fn accelerate(automatic: bool) -> Vec<Sample> {
    let mut session = Session::boot_with_transmission(&paths(), automatic).unwrap();
    let mut controls = Controls::new();
    controls.automatic = automatic;
    controls.touch(1, 0.9, 0.95, true, false);
    let mut samples = vec![session.sample()];
    for tick in 0..7200 {
        if !automatic && tick == 2880 {
            controls.touch(2, 0.9, 0.63, true, false);
        }
        let command = pending_selection(
            controls.input(0.0, f64::from(session.dt())),
            session.sample().commanded_gear,
        );
        session.step(command).unwrap();
        if (tick + 1) % 720 == 0 {
            let sample = session.sample();
            assert!(sample.finite());
            samples.push(sample);
        }
    }
    assert!(samples.iter().any(|s| s.speed() > 5.0));
    assert!(
        samples.iter().any(|s| s.gear >= 3),
        "did not shift above first: {samples:?}"
    );
    samples
}

#[test]
#[ignore = "private Documents: manual and original automatic shifts under load"]
fn loaded_manual_shifts_are_deterministic_and_automatic_is_gated() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            {
                let samples = accelerate(false);
                assert_eq!(samples, accelerate(false));
                eprintln!(
                    "LOADED manual: {:?}",
                    samples
                        .iter()
                        .map(|s| (s.step, s.gear, s.rpm, s.speed()))
                        .collect::<Vec<_>>()
                );
            }
            match Session::boot_with_transmission(&paths(), true) {
                Err(error) => assert!(
                    error.contains("original VCU upshift thresholds are empty"),
                    "{error}"
                ),
                Ok(_) => {
                    let samples = accelerate(true);
                    assert_eq!(samples, accelerate(true));
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
