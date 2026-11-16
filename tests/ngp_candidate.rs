#![cfg(feature = "physics-probe")]
//! These tests inspect private files without installing or altering them.
use rerbr_sim::ngp_candidate::{self, RSF_779_SHA256};
use std::path::PathBuf;

fn install() -> PathBuf {
    std::env::var_os("PHYSICS_RBR")
        .expect("private PHYSICS_RBR")
        .into()
}
fn reference() -> PathBuf {
    PathBuf::from(std::env::var_os("PHYSICS_NGP").expect("private PHYSICS_NGP"))
        .join("Plugins/NGP.dll")
}

#[test]
#[ignore = "private exact NGP DLLs: recognizable 779 must not become enabled; 778 regression stays supported"]
fn exact_779_binary_is_rejected_and_778_baseline_stays_available() {
    let candidate = std::fs::read(install().join("Plugins/NGP.dll")).unwrap();
    assert_eq!(
        ngp_candidate::validate_runtime(&candidate).unwrap_err(),
        ngp_candidate::RUNTIME_BLOCKER
    );
    assert!(ngp_candidate::validate_runtime(&std::fs::read(reference()).unwrap()).is_ok());
}

#[test]
#[ignore = "private physics archive: deterministic inventory, provenance and stock-format detection"]
fn installed_archive_inventory_repeats_without_certifying_stock_cars() {
    let path = install().join("physics.rbz");
    let before = std::fs::read(&path).unwrap();
    let evidence = ngp_candidate::inspect_archive(&path).unwrap();
    assert_eq!(evidence, ngp_candidate::inspect_archive(&path).unwrap());
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert_eq!(evidence.physics_revision, Some(779));
    assert_eq!(evidence.vehicles.len(), 8);
    for vehicle in &evidence.vehicles {
        for surface in &vehicle.surfaces {
            assert!(
                surface.ngp_specs.is_none(),
                "fixture changed; investigate genuine data: {vehicle:?}"
            );
            assert!(!surface.missing_ngp_keys.is_empty());
        }
    }
    eprintln!("NGP779 INVENTORY: {evidence:#?}");
}

#[test]
#[ignore = "private DLLs, release recommended: immutable-read comparison is deterministic, not physics parity"]
fn relocated_779_tables_repeat_exactly_without_runtime_enablement() {
    let path = install().join("Plugins/NGP.dll");
    let evidence = ngp_candidate::compare_binaries(&reference(), &path).unwrap();
    assert_eq!(
        evidence,
        ngp_candidate::compare_binaries(&reference(), &path).unwrap()
    );
    assert_eq!(evidence.candidate.sha256, RSF_779_SHA256);
    assert_eq!(evidence.candidate.supported, None);
    assert_eq!(format!("{:?}", evidence.verdict()), "ReadsUnchanged");
    assert_eq!(evidence.located.len(), 25);
    assert!(ngp_candidate::validate_runtime(&std::fs::read(path).unwrap()).is_err());
    eprintln!("{}", evidence.render());
}
