#![cfg(feature = "physics-probe")]
use rerbr_sim::{inputs::Paths, run, STACK_BYTES};
use std::{path::Path, sync::atomic::AtomicBool};

#[test]
#[ignore = "requires private PHYSICS_DOCUMENTS; CPU acceleration diagnostic only"]
fn actual_acceleration_displaces_the_physical_car() {
    let documents = std::env::var_os("PHYSICS_DOCUMENTS").expect("PHYSICS_DOCUMENTS");
    let paths = Paths::documents(Path::new(&documents));
    let report = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(move || rerbr_sim::run_acceleration(&paths, &AtomicBool::new(false), |_| {}))
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    eprintln!("ACCELERATION CPU REPORT: {report:?}");
    assert!(report.deterministic());
    assert_eq!(
        report.driving_samples[0], report.driving_samples[1],
        "sampled trajectories differ"
    );
    for samples in report.driving_samples.iter() {
        assert_eq!(samples.len(), 11);
        assert!(samples.iter().all(|s| s.finite()));
        assert!(
            samples[5].speed() > 5.0,
            "failed to accelerate: {:?}",
            samples[5]
        );
        assert_eq!(
            samples[5].gear, 2,
            "first gear is index 2, not index 1 (neutral)"
        );
        assert!(samples[5].throttle_received > 0.5);
        assert_eq!(samples[6].brake_received, 1.0);
        assert_eq!(samples[6].payload_clutch, 1.0);
        assert!(samples[6].brake_torque > 0.0);
        assert!(
            samples[10].speed() < 0.1,
            "failed to brake: {:?}",
            samples[10]
        );
        assert!(samples[5].wheel_spin.iter().any(|w| w.abs() > 1.0));
    }
    assert!(
        report
            .displacement_metres
            .iter()
            .all(|d| d.is_finite() && *d > 0.05),
        "no verified physical displacement: {:?}",
        report.displacement_metres
    );
}

#[test]
fn private_roots_are_explicit_and_separate_from_app_bundle() {
    let p = Paths::documents(Path::new("sandbox/Documents"));
    assert_eq!(p.install, Path::new("sandbox/Documents/RBR"));
    assert_eq!(p.maps, Path::new("sandbox/Documents/Harwood/Maps"));
    assert!(p.overlay.is_none());
}
#[test]
fn cancellation_before_start_does_not_require_or_open_private_files() {
    let paths = Paths::documents(Path::new("absent-sandbox"));
    assert!(run(&paths, &AtomicBool::new(true), |_| {})
        .unwrap_err()
        .contains("cancelled"));
}
#[test]
fn missing_private_data_reports_the_file_instead_of_faking_physics() {
    let paths = Paths::documents(
        &std::env::temp_dir().join(format!("missing-rbr-probe-{}", std::process::id())),
    );
    let error = run(&paths, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert!(
        error.contains("missing private RBR/Plugins/NGP.dll"),
        "{error}"
    );
}
#[test]
#[ignore = "needs PHYSICS_DOCUMENTS or private PHYSICS_RBR and PHYSICS_NGP; never upload these resources"]
fn actual_corolla_boot_contacts_steps_and_repeatability() {
    let paths = if let Some(documents) = std::env::var_os("PHYSICS_DOCUMENTS") {
        Paths::documents(Path::new(&documents))
    } else {
        let install =
            std::path::PathBuf::from(std::env::var_os("PHYSICS_RBR").expect("PHYSICS_RBR"));
        let overlay = Some(std::path::PathBuf::from(
            std::env::var_os("PHYSICS_NGP").expect("PHYSICS_NGP"),
        ));
        Paths {
            maps: install.join("Maps"),
            install,
            overlay,
        }
    };
    let report = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(move || run(&paths, &AtomicBool::new(false), |_| {}))
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    assert!(report.deterministic());
    assert_eq!(report.timestep.to_bits(), 0x3ab6_0b61);
    assert_eq!(report.measured[0].steps, 720);
    assert_eq!(report.measured[1].steps, 720);
    assert!(report.peak_payload_bytes < 64 * 1024 * 1024);
    assert!(report
        .measured
        .iter()
        .all(|s| s.mean_ms.is_finite() && s.mean_ms > 0.0));
}
