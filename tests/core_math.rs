//! Integration checks through the optional dependency, without a window or GPU.
#![cfg(feature = "rerbr-3d")]

use rerbr_render_core::{
    camera::{self, Orbit},
    coords, Vec3,
};

#[test]
fn reverse_depth_orders_near_middle_and_far_in_portrait_and_landscape() {
    for aspect in [3.0 / 4.0, 4.0 / 3.0, 16.0 / 9.0] {
        let projection = coords::projection(coords::FOV_DEGREES.to_radians(), aspect);
        let depth = |distance| {
            let clip = projection * Vec3::new(0.0, 0.0, distance).extend(1.0);
            assert!(clip.w > 0.0, "a point in front must have positive clip w");
            clip.z / clip.w
        };
        let (near, middle, far) = (depth(coords::NEAR), depth(10.0), depth(coords::FAR));
        assert!((near - 1.0).abs() < 1e-5);
        assert!(near > middle && middle > far);
        assert!(far.abs() < 1e-5);
        assert_eq!(coords::DEPTH_CLEAR, 0.0);
    }
}

#[test]
fn orbit_centres_its_target_and_recovers_its_eye_at_each_aspect() {
    for aspect in [3.0 / 4.0, 4.0 / 3.0] {
        for yaw in [0.0, 0.7, 2.4] {
            let orbit = Orbit {
                target: Vec3::new(3.0, 1.5, -2.0),
                distance: 6.0,
                yaw,
                pitch: 0.3,
            };
            let matrix = orbit.view_projection(coords::FOV_DEGREES.to_radians(), aspect);
            let target = matrix * orbit.target.extend(1.0);
            assert!(target.w > 0.0);
            assert!((target.x / target.w).abs() < 1e-5);
            assert!((target.y / target.w).abs() < 1e-5);
            assert!((0.0..1.0).contains(&(target.z / target.w)));
            assert!((camera::eye(matrix) - orbit.eye()).length() < 1e-3);
            assert!(orbit
                .lens(coords::FOV_DEGREES.to_radians())
                .view_projection(aspect)
                .abs_diff_eq(matrix, 1e-5));
        }
    }
}
