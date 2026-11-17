use super::{
    metrics::{percentile_ms, Session, State},
    route::{Ground, Route, EYE_HEIGHT},
    *,
};
use crate::harwood::mesh::{Mesh, Vertex};
use rerbr_render_core::Vec3;

fn plane(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> (Mesh, Vec<[Vec3; 3]>) {
    let p = [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]].map(Vec3::from);
    (
        Mesh {
            vertices: p
                .map(|p| Vertex {
                    position: p.to_array(),
                    color: [255; 4],
                    uv0: [0.0; 2],
                    uv1: [0.0; 2],
                })
                .to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Default::default()
        },
        vec![[p[0], p[1], p[2]], [p[0], p[2], p[3]]],
    )
}
fn straight() -> Route {
    let (mesh, roads) = plane(-20.0, 20.0, -10.0, 100.0, 0.0);
    Route::new(
        &[0.0, 5.0, 23.0, 48.0, 70.0].map(|z| Vec3::new(0.0, 0.0, z)),
        &mesh,
        &roads,
    )
    .unwrap()
}
fn run(dt: f64) -> Session {
    let mut s = Session::new(0.0);
    s.begin_frame(2.0);
    for i in 0..100_000 {
        s.begin_frame(2.0 + i as f64 * dt);
        if s.submitted(2.0 + (i + 1) as f64 * dt, 0.002) {
            break;
        }
    }
    assert_eq!(s.state(), State::Complete);
    s
}

#[test]
fn arc_length_removes_original_point_spacing_and_clamps_ends() {
    let route = straight();
    assert!((route.length() - 70.0).abs() < 0.01);
    for d in [0.0, 7.0, 21.0, 36.0, 69.9, 70.0] {
        let eye = route.eye(d).unwrap();
        assert!((eye.z - d).abs() < 0.005, "distance lookup");
        assert!((eye.y - EYE_HEIGHT).abs() < 1e-5);
        let camera = route.camera(d).unwrap();
        assert!((camera.target - camera.eye).normalize().dot(Vec3::Z) > 0.9999);
    }
    assert_eq!(route.eye(-2.0).unwrap(), route.eye(0.0).unwrap());
    assert_eq!(route.eye(100.0).unwrap(), route.eye(70.0).unwrap());
    assert!(route.eye(f32::NAN).is_err());
}

#[test]
fn curved_position_and_heading_are_continuous_by_distance() {
    let (mesh, roads) = plane(-30.0, 70.0, -20.0, 100.0, 0.0);
    let route = Route::new(
        &[
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 20.0],
            [5.0, 0.0, 40.0],
            [20.0, 0.0, 60.0],
            [40.0, 0.0, 75.0],
        ]
        .map(Vec3::from),
        &mesh,
        &roads,
    )
    .unwrap();
    let ds = route.length() / 1000.0;
    let mut previous = route.camera(0.0).unwrap();
    for i in 1..=1000 {
        let camera = route.camera(ds * i as f32).unwrap();
        let travelled = camera.eye.distance(previous.eye);
        assert!((travelled - ds).abs() < ds * 0.04);
        assert!(
            (camera.target - camera.eye)
                .normalize()
                .dot((previous.target - previous.eye).normalize())
                > 0.999
        );
        previous = camera;
    }
}

#[test]
fn vertical_ray_uses_triangle_plane_and_highest_surface() {
    let slope = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(0.0, 0.0, 10.0),
    ];
    assert!((route::triangle_height(Vec3::new(2.0, 99.0, 2.0), slope).unwrap() - 2.0).abs() < 1e-6);
    assert!(route::triangle_height(Vec3::new(9.0, 0.0, 9.0), slope).is_none());
    let higher = slope.map(|p| p + Vec3::Y * 3.0);
    let ground = Ground::new(vec![slope, higher]).unwrap();
    assert!((ground.height(Vec3::new(2.0, 0.0, 2.0)).unwrap() - 5.0).abs() < 1e-6);
    assert!(ground.height(Vec3::splat(100.0)).is_none());
}

#[test]
fn real_road_gaps_and_ambiguous_surfaces_are_rejected() {
    let (mesh, mut roads) = plane(-10.0, 10.0, -5.0, 20.0, 0.0);
    let (terrain, rest) = plane(-10.0, 10.0, 30.0, 90.0, 0.0);
    roads.extend(rest);
    let points = [0.0, 15.0, 30.0, 45.0, 60.0, 75.0].map(|z| Vec3::new(0.0, 0.0, z));
    assert!(Route::new(&points, &terrain, &roads).is_err());
    let (raised, flat) = plane(-10.0, 10.0, -5.0, 90.0, 3.0);
    let road = flat
        .iter()
        .map(|tri| tri.map(|p| p - Vec3::Y * 3.0))
        .collect::<Vec<_>>();
    assert!(Route::new(&points, &raised, &road).is_err());
    assert!(Route::new(&[Vec3::ZERO, Vec3::Z * 60.0], &mesh, &road).is_err());
    assert!(Route::new(&[Vec3::NAN, Vec3::ZERO], &mesh, &road).is_err());
}

#[test]
fn wall_clock_duration_and_distance_do_not_depend_on_frame_count() {
    let slow = run(0.25);
    let fast = run(0.125);
    assert_eq!(slow.summary().unwrap().frames, 80);
    assert_eq!(fast.summary().unwrap().frames, 160);
    assert_eq!(slow.summary().unwrap().duration, 20.0);
    assert_eq!(fast.summary().unwrap().duration, 20.0);
    assert_eq!(run(1.0 / 30.0).summary().unwrap().frames, 600);
    assert_eq!(run(1.0 / 60.0).summary().unwrap().frames, 1200);
    let mut s = Session::new(0.0);
    s.begin_frame(2.0);
    s.begin_frame(9.0);
    assert!((s.distance(600.0) - 210.0).abs() < 0.01);
    s.begin_frame(25.0);
    assert_eq!(s.distance(600.0), 600.0);
}

#[test]
fn metrics_use_full_one_second_windows_and_nearest_rank_percentiles() {
    let s = run(0.25);
    let summary = s.summary().unwrap();
    assert_eq!(summary.average_fps, 4.0);
    assert_eq!(summary.min_window_fps, 4.0);
    assert_eq!(summary.max_window_fps, 4.0);
    assert_eq!(summary.mean_ms, 250.0);
    assert_eq!(summary.p95_ms, 250.0);
    assert_eq!(summary.p99_ms, 250.0);
    assert!((summary.cpu_mean_ms - 2.0).abs() < 1e-9);
    let data = (1..=100).map(|v| v as f64 / 1000.0).collect::<Vec<_>>();
    assert_eq!(percentile_ms(&data, 0.95), 95.0);
    assert_eq!(percentile_ms(&data, 0.99), 99.0);
}

#[test]
fn stalls_and_end_overshoot_are_retained_in_results() {
    let mut s = Session::new(0.0);
    s.begin_frame(2.0);
    s.submitted(2.1, 0.003);
    s.begin_frame(6.0);
    s.submitted(6.1, 0.004);
    s.begin_frame(22.0);
    assert!(s.submitted(22.5, 0.005));
    let r = s.summary().unwrap();
    assert_eq!(r.duration, 20.5);
    assert_eq!(r.frames, 3);
    assert_eq!(r.min_window_fps, 0.0);
    assert!(r.p99_ms > 16_000.0);
    assert!((r.average_fps - 3.0 / 20.5).abs() < 1e-9);
}

#[test]
fn warmup_abort_restart_and_bad_clocks_do_not_leak_samples() {
    let mut s = Session::new(0.0);
    s.begin_frame(1.0);
    assert!(!s.submitted(1.1, 0.1));
    assert_eq!(s.state(), State::Warmup);
    s.begin_frame(2.0);
    s.submitted(2.2, 0.001);
    s.abort();
    assert!(s.summary().is_none());
    s.restart(8.0);
    s.begin_frame(10.0);
    assert!(s.submitted(30.0, 0.002));
    assert_eq!(s.summary().unwrap().frames, 1);
    s.restart(40.0);
    assert!(s.summary().is_none());
    assert_eq!(s.elapsed(), 0.0);
    s.begin_frame(42.0);
    s.submitted(41.0, 0.1);
    assert_eq!(s.state(), State::Aborted);
    s.restart(0.0);
    s.begin_frame(2.0);
    s.submitted(3.0, f64::NAN);
    assert_eq!(s.state(), State::Aborted);
}

#[test]
fn runtime_restarts_cached_route_and_invalidates_interrupted_runs() {
    let mut runtime = Runtime::new();
    runtime.install(straight());
    let mut camera = crate::harwood::Camera {
        eye: Vec3::ZERO,
        target: Vec3::Z,
        up: Vec3::Y,
    };
    runtime.before_frame(&mut camera).unwrap();
    runtime.interrupt("TEST INTERRUPTION");
    assert_eq!(runtime.session.as_ref().unwrap().state(), State::Aborted);
    runtime.restart();
    assert_eq!(runtime.session.as_ref().unwrap().state(), State::Warmup);
    assert!(runtime.session.as_ref().unwrap().summary().is_none());
    assert!(runtime.lines().iter().any(|line| line.contains("WARMUP")));
    runtime.fail();
    runtime.restart();
    assert!(runtime.fatal);
}

#[test]
fn hud_has_bounded_finite_vertices_and_original_binary_font() {
    let atlas = hud::atlas();
    assert_eq!(atlas.len(), 512 * 512);
    assert!(atlas.iter().filter(|&&v| v > 127).count() > 5000);
    for size in [(1488, 2266), (2266, 1488)] {
        let data = hud::vertices(&vec!["X".repeat(100); 20], size);
        assert!(data.len() / 20 < 8192);
        assert_eq!(data.len() % 20, 0);
        for vertex in data.chunks_exact(20) {
            for field in vertex[..16].chunks_exact(4) {
                assert!(f32::from_ne_bytes(field.try_into().unwrap()).is_finite());
            }
        }
    }
}
