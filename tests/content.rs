use rerbr_probe::content::Selection;
#[cfg(feature = "physics-probe")]
use rerbr_probe::content::{StageDescriptor, VehicleDescriptor};

#[test]
fn descriptors_are_shared_owned_values_without_physics_or_graphics_requirements() {
    let selected = Selection::default();
    selected.validate_current_backend().unwrap();
    assert_eq!(selected.stage.file("lbs").unwrap(), "track-20_O.lbs");
    assert_eq!(selected.vehicle.forward_gears, 6);
    assert_eq!(selected.vehicle.required_paths().count(), 8);
}

#[cfg(feature = "physics-probe")]
#[test]
fn future_profiles_are_rejected_before_opening_private_files() {
    let absent = std::env::temp_dir().join("rerbr-absent-content-test-root");
    let unsupported = Selection {
        vehicle: VehicleDescriptor {
            id: "unvalidated-car".into(),
            ..VehicleDescriptor::default()
        },
        ..Selection::default()
    };
    assert!(
        rerbr_sim::inputs::Paths::for_selection(&absent, &unsupported)
            .unwrap_err()
            .contains("unsupported vehicle")
    );
    #[cfg(feature = "car-chase-benchmark")]
    assert!(
        rerbr_probe::car::load_vehicle(&absent, &unsupported.vehicle)
            .err()
            .unwrap()
            .contains("unsupported vehicle")
    );
    let bad_stage = StageDescriptor {
        original_map: 21,
        ..StageDescriptor::default()
    };
    #[cfg(feature = "physics-stage")]
    assert!(
        rerbr_probe::harwood::stage::load_selected(&absent, &bad_stage)
            .err()
            .unwrap()
            .contains("unsupported stage")
    );
    let invalid = Selection {
        stage: bad_stage,
        ..Selection::default()
    };
    assert!(rerbr_sim::inputs::Paths::for_selection(&absent, &invalid)
        .unwrap_err()
        .contains("unsupported stage"));
}

#[cfg(feature = "physics-probe")]
#[test]
fn selected_paths_preserve_the_existing_private_documents_layout() {
    let root = std::path::Path::new("sandbox/Documents");
    let selected = rerbr_sim::inputs::Paths::for_selection(root, &Selection::default()).unwrap();
    let legacy = rerbr_sim::inputs::Paths::documents(root);
    assert_eq!(selected.install, root.join("RBR"));
    assert_eq!(selected.maps, root.join("Harwood/Maps"));
    assert_eq!(
        (selected.install, selected.maps, selected.overlay),
        (legacy.install, legacy.maps, legacy.overlay)
    );
}
