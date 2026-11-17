//! Private CPU check: real SGC geometry, physics spawn and actual visible terrain/camera.
use super::*;
use rerbr_sim::{drive::Session, inputs::Paths, STACK_BYTES};
#[test]
#[ignore = "private PHYSICS_RBR and PHYSICS_NGP; no GPU, private assets never uploaded"]
fn original_spawn_fits_the_visible_scene_and_camera() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let install = std::path::PathBuf::from(std::env::var_os("PHYSICS_RBR").unwrap());
            let overlay = Some(std::path::PathBuf::from(
                std::env::var_os("PHYSICS_NGP").unwrap(),
            ));
            let paths = Paths {
                maps: install.join("Maps"),
                install,
                overlay,
            };
            let mut session = Session::boot(&paths).unwrap();
            let data = crate::car::prepare(&paths.install, &paths.maps).unwrap();
            let rest = session.sample();
            let pivots = data.car.geometry.pivots;
            let swap = coords::physics_to_stage();
            let offset = (0..4)
                .map(|k| Vec3::from(rest.wheel_centre[k]) - swap.transform_point3(pivots[k]))
                .sum::<Vec3>()
                * 0.25;
            for step in 0..=3600 {
                if step % 720 == 0 {
                    let sample = session.sample();
                    let m = models(&sample, &sample, 1.0, pivots, offset);
                    let position = swap.transform_point3(Vec3::from(sample.position));
                    assert!(
                        data.path.supports_driving(position),
                        "actual physics pose left visible terrain"
                    );
                    let forward = Vec3::new(-m[0].z_axis.x, 0.0, -m[0].z_axis.z).normalize();
                    let (camera, _) = data
                        .path
                        .chase_camera(position + Vec3::Y * 0.5, forward)
                        .unwrap();
                    let vp = crate::car::placement::view_projection(camera, 1.55);
                    for (slot, points) in std::iter::once((0, &data.car.geometry.body_points))
                        .chain(
                            data.car
                                .geometry
                                .wheel_points
                                .iter()
                                .enumerate()
                                .map(|(k, p)| (k + 1, p)),
                        )
                    {
                        for &v in points {
                            let clip = vp * (m[slot] * v.extend(1.0));
                            assert!(clip.w > 0.0);
                            assert!(
                                clip.x.abs() <= clip.w && clip.y.abs() <= clip.w,
                                "car clips view at {step}: {clip:?}"
                            );
                        }
                    }
                    eprintln!(
                        "PHYSICS/RENDER CPU PASS step={step} body={:?} camera={camera:?}",
                        m[0].w_axis
                    );
                }
                if step < 3600 {
                    session
                        .step(rerbr_sim::drive::Control {
                            throttle: 1.0,
                            flags: 4,
                            gear: Some(2),
                            ..Default::default()
                        })
                        .unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(feature = "physics-stage")]
#[test]
#[ignore = "private PHYSICS_RBR, PHYSICS_NGP, RBR_ROOT; original resources never uploaded"]
fn selected_stages_boot_step_and_repeat_without_stale_session_state() {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            let install = std::path::PathBuf::from(std::env::var_os("PHYSICS_RBR").unwrap());
            let maps = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").unwrap()).join("Maps");
            let overlay = Some(std::path::PathBuf::from(
                std::env::var_os("PHYSICS_NGP").unwrap(),
            ));
            let paths = Paths {
                install,
                maps,
                overlay,
            };
            let stages = rerbr_content::StageDescriptor::presets();
            let mut reference = Vec::new();
            for pass in 0..2 {
                for (index, stage) in stages.iter().enumerate() {
                    let selection = rerbr_content::Selection {
                        stage: stage.clone(),
                        ..Default::default()
                    };
                    let mut session = Session::boot_selected(&paths, &selection, false).unwrap();
                    let rest = session.sample();
                    for _ in 0..3600 {
                        let input = rerbr_sim::drive::Control {
                            throttle: 1.0,
                            flags: 4,
                            gear: Some(2),
                            ..Default::default()
                        };
                        let command =
                            controls::pending_selection(input, session.sample().commanded_gear);
                        session.step(command).unwrap();
                    }
                    let sample = session.sample();
                    assert!(session
                        .physics_audit()
                        .scripts
                        .iter()
                        .any(|p| p.ends_with(&format!(
                            "/{}.lsp",
                            rerbr_sim::boot::surface(stage.original_map)
                        ))));
                    assert!(sample.finite());
                    assert!(sample.wheel_ground.iter().any(|&g| g != 0));
                    assert!(
                        sample.speed() > 0.1,
                        "{} failed acceleration: {sample:?}",
                        stage.id
                    );
                    if pass == 0 {
                        let scene =
                            crate::car::prepare_selected(&paths.install, &paths.maps, &selection)
                                .unwrap();
                        let swap = coords::physics_to_stage();
                        let pivots = scene.car.geometry.pivots;
                        let offset = (0..4)
                            .map(|k| {
                                Vec3::from(rest.wheel_centre[k]) - swap.transform_point3(pivots[k])
                            })
                            .sum::<Vec3>()
                            * 0.25;
                        for pose in [&rest, &sample] {
                            let position = swap.transform_point3(Vec3::from(pose.position));
                            assert!(
                                scene.path.supports_driving(position),
                                "{} physics is outside rendered terrain",
                                stage.id
                            );
                            let transforms = models(pose, pose, 1.0, pivots, offset);
                            let forward =
                                Vec3::new(-transforms[0].z_axis.x, 0.0, -transforms[0].z_axis.z)
                                    .normalize();
                            let (camera, _) = scene
                                .path
                                .chase_camera(position + Vec3::Y * 0.5, forward)
                                .unwrap();
                            let vp = crate::car::placement::view_projection(camera, 1.55);
                            for &point in &scene.car.geometry.body_points {
                                let clip = vp * (transforms[0] * point.extend(1.0));
                                assert!(
                                    clip.is_finite()
                                        && clip.w > 0.0
                                        && clip.x.abs() <= clip.w
                                        && clip.y.abs() <= clip.w,
                                    "{} car clips the chase camera",
                                    stage.id
                                );
                            }
                        }
                        reference.push(sample);
                    } else {
                        assert_eq!(
                            sample, reference[index],
                            "reloaded stage differs: {}",
                            stage.id
                        );
                    }
                    eprintln!("SELECTED PHYSICS PASS: {} pass={pass}", stage.id);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
