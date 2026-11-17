#![cfg(feature = "physics-drive")]
use rerbr_probe::driving::models;
use rerbr_render_core::{coords, Mat4, Quat, Vec3};
use rerbr_sim::diagnostics::Sample;

#[test]
fn camera_view_preserves_existing_projection_and_rotated_chassis_axes() {
    use rerbr_probe::harwood::look_to_lh;
    let eye = Vec3::new(2.0, 3.0, -4.0);
    let forward = Vec3::new(0.4, -0.2, 1.0).normalize();
    assert!(look_to_lh(eye, forward, Vec3::Y).abs_diff_eq(coords::view_matrix(eye, forward), 1e-5));
    let rotation =
        Quat::from_rotation_y(0.4) * Quat::from_rotation_x(-0.3) * Quat::from_rotation_z(0.5);
    let view = look_to_lh(eye, rotation * Vec3::Z, rotation * Vec3::Y);
    assert!(view.transform_point3(eye).abs_diff_eq(Vec3::ZERO, 1e-5));
    assert!(view
        .transform_vector3(rotation * Vec3::Y)
        .abs_diff_eq(Vec3::Y, 1e-5));
    assert!(view
        .transform_vector3(rotation * Vec3::Z)
        .abs_diff_eq(Vec3::Z, 1e-5));
    let parallel = look_to_lh(eye, Vec3::X, Vec3::X);
    assert!(parallel.is_finite() && (parallel.determinant() - 1.0).abs() < 1e-5);
}
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
#[test]
fn front_wheels_steer_while_rear_wheels_stay_straight() {
    for angle in [-0.4_f32, 0.0, 0.4] {
        let axle = Vec3::new(angle.cos(), angle.sin(), 0.1);
        let a = Sample {
            orientation: Quat::IDENTITY.to_array(),
            steering_angle: 100.0, // Steering-wheel radians must not drive tyre yaw.
            wheel_axle: [
                axle.to_array(),
                (-axle).to_array(),
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
            ],
            ..Default::default()
        };
        let pivot = Vec3::new(0.7, 0.3, -1.0);
        let m = models(&a, &a, 0.5, [pivot; 4], Vec3::ZERO);
        for wheel in &m[1..3] {
            let forward = wheel.transform_vector3(Vec3::NEG_Z);
            assert!(forward.abs_diff_eq(Vec3::new(angle.sin(), 0.0, -angle.cos()), 1e-5));
            assert!((forward.angle_between(Vec3::NEG_Z) - angle.abs()).abs() < 1e-5);
            assert!(wheel.transform_point3(pivot).abs_diff_eq(Vec3::ZERO, 1e-5));
        }
        for wheel in &m[3..5] {
            assert!(wheel
                .transform_vector3(Vec3::NEG_Z)
                .abs_diff_eq(Vec3::NEG_Z, 1e-5));
        }
    }
}

#[test]
fn measured_wheel_heading_interpolates_and_unavailable_axles_are_neutral() {
    use rerbr_probe::driving::wheel_heading;
    assert_eq!(wheel_heading(Vec3::ZERO), 0.0);
    assert_eq!(wheel_heading(Vec3::Z), 0.0);
    assert_eq!(wheel_heading(Vec3::splat(f32::NAN)), 0.0);
    let a = Sample {
        orientation: Quat::IDENTITY.to_array(),
        wheel_axle: [[1.0, 0.0, 0.0]; 4],
        ..Default::default()
    };
    let b = Sample {
        wheel_axle: [[0.4_f32.cos(), 0.4_f32.sin(), 0.0]; 4],
        ..a.clone()
    };
    let m = models(&a, &b, 0.5, [Vec3::ZERO; 4], Vec3::ZERO);
    assert!(m[1]
        .transform_vector3(Vec3::NEG_Z)
        .abs_diff_eq(Vec3::new(0.2_f32.sin(), 0.0, -0.2_f32.cos()), 1e-5));
}
