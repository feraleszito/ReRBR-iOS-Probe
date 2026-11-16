#![cfg(any(feature = "physics-probe", feature = "physics-drive"))]
use rerbr_sim::{
    diagnostics::Sample,
    drive::{Control, Session},
    inputs::Paths,
    STACK_BYTES,
};

fn trial(steering: f32) -> Vec<Sample> {
    let documents = std::env::var_os("PHYSICS_DOCUMENTS").expect("private PHYSICS_DOCUMENTS");
    let mut session = Session::boot(&Paths::documents(std::path::Path::new(&documents))).unwrap();
    let mut samples = vec![session.sample()];
    let triangles =
        rerbr_sim::drive::collision_triangles(&Paths::documents(std::path::Path::new(&documents)))
            .unwrap();
    for step in 0..14400 {
        let input = if step < 3600 {
            Control {
                throttle: 1.0,
                steering: if step > 2160 { steering } else { 0.0 },
                flags: 4,
                gear: Some(2),
                ..Default::default()
            }
        } else {
            Control {
                brake: 1.0,
                clutch: 1.0,
                steering,
                flags: 4,
                gear: Some(2),
                ..Default::default()
            }
        };
        session.step(input).unwrap();
        if (step + 1) % 720 == 0 {
            let sample = session.sample();
            assert!(sample.finite());
            // Independent vertical intersection with actual COL triangles beneath contacting wheel centres.
            for k in 0..4 {
                if sample.wheel_ground[k] == 0 {
                    continue;
                }
                let [qx, qy, qz, qw] = sample.orientation;
                let [x, y, z] = sample.wheel_centre[k];
                let world = [
                    sample.position[0]
                        + (1.0 - 2.0 * (qy * qy + qz * qz)) * x
                        + 2.0 * (qx * qy - qw * qz) * y
                        + 2.0 * (qx * qz + qw * qy) * z,
                    sample.position[1]
                        + 2.0 * (qx * qy + qw * qz) * x
                        + (1.0 - 2.0 * (qx * qx + qz * qz)) * y
                        + 2.0 * (qy * qz - qw * qx) * z,
                    sample.position[2]
                        + 2.0 * (qx * qz - qw * qy) * x
                        + 2.0 * (qy * qz + qw * qx) * y
                        + (1.0 - 2.0 * (qx * qx + qy * qy)) * z,
                ];
                assert!(
                    triangles.iter().any(|&t| col_support(world, t)),
                    "no actual COL surface near grounded wheel {k} at {world:?}"
                );
            }
            samples.push(sample);
        }
    }
    assert!(session.payload_bytes() < 64 * 1024 * 1024);
    samples
}
fn col_support(p: [f32; 3], [a, b, c]: [[f32; 3]; 3]) -> bool {
    let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if d.abs() < 1e-8 {
        return false;
    }
    let u = ((b[1] - c[1]) * (p[0] - c[0]) + (c[0] - b[0]) * (p[1] - c[1])) / d;
    let v = ((c[1] - a[1]) * (p[0] - c[0]) + (a[0] - c[0]) * (p[1] - c[1])) / d;
    let w = 1.0 - u - v;
    let z = u * a[2] + v * b[2] + w * c[2];
    u >= -1e-4 && v >= -1e-4 && w >= -1e-4 && (0.0..1.0).contains(&(p[2] - z))
}

#[cfg(feature = "physics-drive")]
#[test]
#[ignore = "private Documents: real touchscreen controls into physics, repeated acceleration and braking"]
fn touchscreen_pedals_drive_stop_and_accelerate_again() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let root = std::env::var_os("PHYSICS_DOCUMENTS").unwrap();
            let mut session =
                Session::boot(&Paths::documents(std::path::Path::new(&root))).unwrap();
            let mut controls = rerbr_probe::driving::controls::Controls::new();
            controls.touch(1, 0.9, 0.95, true, false);
            for step in 0..14400 {
                if step == 3600 || step == 10800 {
                    controls.clear();
                    controls.touch(2, 0.1, 0.95, true, false);
                }
                if step == 7200 {
                    controls.clear();
                    controls.touch(1, 0.9, 0.95, true, false);
                }
                session.step(controls.input(0.0, 1.0 / 720.0)).unwrap();
                if (step + 1) % 3600 == 0 {
                    let sample = session.sample();
                    eprintln!("TOUCH DRIVE {} {sample:?}", step + 1);
                    assert!(sample.finite());
                    if step == 3599 || step == 10799 {
                        assert!(
                            sample.speed() > 1.0,
                            "failed to accelerate via touch pedals"
                        );
                    } else {
                        assert!(sample.speed() < 0.1, "failed to brake via touch pedals");
                    }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "private Documents CPU steering/contact/stability regression"]
fn original_steering_changes_heading_and_path_deterministically() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let straight = trial(0.0);
            let left = trial(-0.25);
            let right = trial(0.25);
            assert_eq!(left, trial(-0.25));
            assert_eq!(right, trial(0.25));
            for (label, samples) in [("straight", &straight), ("left", &left), ("right", &right)] {
                eprintln!(
                    "DRIVE REGRESSION {label}: start={:?}; 5s={:?}; 20s={:?}",
                    samples[0], samples[5], samples[20]
                );
                assert!(samples.iter().take(5).any(|s| s.speed() > 1.0));
                assert!(samples[20].speed() < 0.1);
                assert!(samples[5].wheel_ground.iter().any(|&f| f != 0));
                assert!(
                    samples.iter().any(|s| s
                        .wheel_travel
                        .iter()
                        .zip(samples[0].wheel_travel)
                        .any(|(&a, b)| (a - b).abs() > 1e-4)),
                    "suspension did not respond"
                );
            }
            assert_ne!(left[5].orientation, straight[5].orientation);
            assert_ne!(right[5].orientation, straight[5].orientation);
            assert_ne!(left[5].position, right[5].position);
            // Signed horizontal turn in physics XY: forward is rotated body -Y.
            let heading = |s: &Sample| {
                let [x, y, z, w] = s.orientation;
                [2.0 * (w * z - x * y), -(1.0 - 2.0 * (x * x + z * z))]
            };
            let cross = |a: [f32; 2], b: [f32; 2]| a[0] * b[1] - a[1] * b[0];
            let l = cross(heading(&left[0]), heading(&left[5]));
            let r = cross(heading(&right[0]), heading(&right[5]));
            assert!(
                l * r < 0.0,
                "steering signs did not produce opposite turns: {l}, {r}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "private Documents: sixty-second original physics stability and repeatability"]
fn sustained_sixty_seconds_remains_finite_and_bounded() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let root = std::env::var_os("PHYSICS_DOCUMENTS").unwrap();
            let paths = Paths::documents(std::path::Path::new(&root));
            let mut previous = None;
            for pass in 0..2 {
                let mut session = Session::boot(&paths).unwrap();
                let mut samples = Vec::new();
                for step in 0..43200 {
                    let input = if step < 21600 {
                        Control {
                            throttle: 0.4,
                            flags: 4,
                            gear: Some(2),
                            ..Default::default()
                        }
                    } else {
                        Control {
                            brake: 1.0,
                            clutch: 1.0,
                            flags: 4,
                            gear: Some(2),
                            ..Default::default()
                        }
                    };
                    session.step(input).unwrap();
                    if (step + 1) % 720 == 0 {
                        let s = session.sample();
                        assert!(s.finite());
                        assert!(session.payload_bytes() < 64 * 1024 * 1024);
                        samples.push(s);
                    }
                }
                eprintln!(
                    "SUSTAINED pass={pass}: 30s={:?}; 60s={:?}; allocated={}",
                    samples[29],
                    samples[59],
                    session.payload_bytes()
                );
                assert!(samples.iter().any(|s| s.speed() > 1.0));
                assert!(samples[59].speed() < 0.1);
                if let Some(before) = previous {
                    assert_eq!(before, samples);
                }
                previous = Some(samples);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
