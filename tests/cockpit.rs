#![cfg(feature = "physics-stage")]
use rerbr_probe::car;
use rerbr_render_core::{Mat4, Vec3};

#[test]
#[ignore = "private RBR_ROOT: actual original interior, textures and animated steering node"]
fn original_corolla_interior_has_valid_draws_and_steering_pivot() {
    let root = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").unwrap());
    let car = car::load(&root).unwrap();
    let node = car
        .cockpit
        .unwrap_or_else(|| panic!("{:?}", car.cockpit_error));
    assert!(car.geometry.drawn.iter().any(|n| n == "i_body"));
    assert!(car.geometry.drawn.iter().any(|n| n == "i_steeringwheel"));
    assert!(car.geometry.draws.iter().any(|d| d.interior && d.slot == 5));
    for draw in &car.geometry.draws {
        assert!(draw.slot < 6);
        if draw.slot == 5 {
            assert!(draw.interior);
        }
        for &index in &car.geometry.indices[draw.indices.clone()] {
            assert!((index as usize) < car.geometry.vertices.len());
        }
    }
    let pivot = node.transform_point3(Vec3::ZERO);
    assert!(pivot.is_finite());
    let full_lock = car::cockpit::steering_transform(node, 1.4);
    assert!(full_lock.transform_point3(pivot).abs_diff_eq(pivot, 1e-5));
    assert!(!full_lock.abs_diff_eq(Mat4::IDENTITY, 1e-3));
    eprintln!(
        "ORIGINAL COCKPIT: meshes={:?}; private files={:?}; pivot={pivot:?}",
        car.geometry
            .drawn
            .iter()
            .filter(|n| n.starts_with("i_"))
            .collect::<Vec<_>>(),
        car.files
    );
}

#[test]
#[ignore = "private PHYSICS_DOCUMENTS: actual rack angle responds to physics steering state"]
fn cockpit_angle_tracks_original_rack_in_both_directions() {
    use rerbr_sim::{
        drive::{Control, Session},
        inputs::Paths,
        STACK_BYTES,
    };
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let paths = Paths::documents(&std::path::PathBuf::from(
                std::env::var_os("PHYSICS_DOCUMENTS").unwrap(),
            ));
            let mut session = Session::boot(&paths).unwrap();
            for _ in 0..1440 {
                session.step(Control::default()).unwrap();
            }
            for direction in [-0.5_f32, 0.5, 0.0] {
                for _ in 0..144 {
                    session
                        .step(Control {
                            steering: direction,
                            ..Default::default()
                        })
                        .unwrap();
                }
                let sample = session.sample();
                assert!(sample.finite());
                assert!((sample.steering_received - direction).abs() < 1e-5);
                assert!((sample.steering_angle - direction * 1.4).abs() < 1e-4);
                eprintln!(
                    "ACTUAL RACK: input={} steering-wheel angle={} rad",
                    sample.steering_received, sample.steering_angle
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
