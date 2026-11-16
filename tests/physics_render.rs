#![cfg(feature = "physics-drive")]
use rerbr_probe::driving::models;
use rerbr_render_core::{coords, Mat4, Quat, Vec3};
use rerbr_sim::diagnostics::Sample;
#[test]
fn original_axis_conversion_interpolates_real_pose_and_keeps_wheel_centres() {
    let a = Sample {
        position: [1.0, 2.0, 3.0],
        orientation: Quat::IDENTITY.to_array(),
        wheel_centre: [[0.7, -1.0, -0.2]; 4],
        ..Default::default()
    };
    let b = Sample {
        position: [5.0, 6.0, 7.0],
        orientation: Quat::from_rotation_z(1.0).to_array(),
        wheel_roll: [1.0; 4],
        ..a.clone()
    };
    let pivots = [Vec3::new(0.7, 0.16, -2.0); 4];
    let m = models(&a, &b, 0.5, pivots, Vec3::ZERO);
    let expected = coords::physics_to_stage()
        * Mat4::from_rotation_translation(Quat::from_rotation_z(0.5), Vec3::new(3.0, 4.0, 5.0));
    assert!(m[0].abs_diff_eq(expected * coords::physics_to_stage(), 1e-5));
    assert!(
        (m[0].determinant() - 1.0).abs() < 1e-5,
        "two mirrors must cancel for CW culling"
    );
    for k in 0..4 {
        assert!(m[k + 1].transform_point3(pivots[k]).abs_diff_eq(
            expected.transform_point3(Vec3::from(a.wheel_centre[k])),
            1e-5
        ));
    }
    let start = models(&a, &b, 0.0, pivots, Vec3::ZERO);
    assert!(start[0]
        .transform_vector3(Vec3::NEG_Z)
        .abs_diff_eq(Vec3::NEG_Z, 1e-5));
}
#[test]
fn interpolation_clamps_and_wheel_wrap_has_no_position_jump() {
    let a = Sample {
        orientation: Quat::IDENTITY.to_array(),
        wheel_roll: [20.0 * std::f32::consts::PI - 0.1; 4],
        ..Default::default()
    };
    let b = Sample {
        wheel_roll: [0.1; 4],
        ..a.clone()
    };
    let m = models(&a, &b, 0.5, [Vec3::ZERO; 4], Vec3::ZERO);
    assert!(m[1].abs_diff_eq(Mat4::IDENTITY, 1e-4));
    assert_eq!(
        models(&a, &b, -1.0, [Vec3::ZERO; 4], Vec3::ZERO),
        models(&a, &b, 0.0, [Vec3::ZERO; 4], Vec3::ZERO)
    );
}
