//! Synthetic fixtures only. Original assets are opt-in integration inputs.
use super::*;
use rerbr_asset_formats::{ini, sgc};
use rerbr_render_core::Vec3;

fn node(name: &str, id: i32, x: f32, y: f32, z: f32) -> sgc::Node {
    sgc::Node {
        name: name.into(),
        id,
        parent: 0,
        transform: sgc::Transform {
            scale: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: 0.0,
            translation: [1.0; 3],
            axis2: [0.0, 0.0, 1.0],
            angle2: 0.0,
        },
        world: [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]],
    }
}
fn triangle(node: sgc::Node) -> sgc::Mesh {
    sgc::Mesh {
        node,
        damageable: false,
        centre: [0.0; 3],
        half_extent: [1.0; 3],
        stream_flags: 5,
        vertices: [[-0.1, -0.35, -0.1], [-0.1, -0.35, 0.1], [0.1, -0.35, 0.1]]
            .map(|position| sgc::Vertex {
                position,
                normal: [0.0, 1.0, 0.0],
            })
            .to_vec(),
        uvs: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
        aux: Vec::new(),
        triangles: vec![[0, 1, 2]],
        faces: vec![sgc::Face {
            name: "paint".into(),
            prim_count: 1,
            min_vertex: 0,
            num_vertices: 3,
            start_index: 0,
        }],
        morph_targets: Vec::new(),
        morph_lists: Vec::new(),
    }
}
fn fixture() -> (
    Vec<sgc::Element>,
    Vec<ini::Section>,
    Vec<ini::Section>,
    Vec<ini::Section>,
) {
    let mut root = node("root", 0, 0.0, 0.0, 0.0);
    root.parent = -1;
    let mut elements = vec![
        sgc::Element::Node(root),
        sgc::Element::Mesh(triangle(node("e_body", 1, 0.0, 1.0, 0.0))),
    ];
    let mut parts =
        String::from("[e_body]\nFaceBlock0_Shader=CarTextureNoDirt\nFaceBlock0_Material=paint\n");
    for (i, side) in selection::SIDES.iter().enumerate() {
        let name = format!("e_wheel_gravel_{side}");
        elements.push(sgc::Element::Mesh(triangle(node(
            &name,
            i as i32 + 2,
            if i % 2 == 0 { 0.8 } else { -0.8 },
            0.35,
            if i < 2 { -1.2 } else { 1.2 },
        ))));
        parts.push_str(&format!(
            "[{name}]\nFaceBlock0_Shader=CarTextureNoDirt\nFaceBlock0_Material=paint\n"
        ));
    }
    (elements,ini::parse(&parts).unwrap(),ini::parse("[paint]\nOpacityChannel=false\n[paint_diffuse_bitmap]\nFilename=base.dds\n[TextureSets]\nTextureSet0=TSDefault\n[TSDefault]\nbase.dds=Cars/fixture/base.dds\n").unwrap(),ini::parse("[CarTextureNoDirt]\nKa=1\n").unwrap())
}
fn geometry() -> mesh::Geometry {
    let (e, p, s, c) = fixture();
    mesh::convert(&e, &p, &s, &c).unwrap()
}
pub(super) fn path_fixture(slope: f32) -> placement::ChasePath {
    let p = [[-20.0, -30.0], [-20.0, 130.0], [20.0, 130.0], [20.0, -30.0]]
        .map(|[x, z]| Vec3::new(x, z * slope, z));
    let mesh = crate::harwood::mesh::Mesh {
        vertices: p
            .map(|p| crate::harwood::mesh::Vertex {
                position: p.to_array(),
                color: [255; 4],
                uv0: [0.0; 2],
                uv1: [0.0; 2],
            })
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    };
    let road = vec![[p[0], p[1], p[2]], [p[0], p[2], p[3]]];
    let route = crate::harwood::benchmark::route::Route::new(
        &[0.0, 20.0, 40.0, 60.0, 80.0].map(|z| Vec3::new(0.0, z * slope, z)),
        &mesh,
        &road,
    )
    .unwrap();
    placement::ChasePath::new(route, &mesh, &geometry()).unwrap()
}

#[test]
fn synthetic_sgc_reader_to_original_transform_and_converter() {
    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend(s.as_bytes());
        out.push(0);
    }
    fn header(out: &mut Vec<u8>, n: &sgc::Node) {
        string(out, &n.name);
        out.extend(n.id.to_le_bytes());
        out.extend(n.parent.to_le_bytes());
        out.extend(n.transform.words().into_iter().flat_map(f32::to_le_bytes));
        out.extend(n.world.into_iter().flatten().flat_map(f32::to_le_bytes));
    }
    let (elements, p, s, c) = fixture();
    let mut bytes = sgc::BANNER.to_vec();
    for e in elements {
        let mut body = Vec::new();
        let (kind, subtype) = match e {
            sgc::Element::Node(n) => {
                header(&mut body, &n);
                (3, 13)
            }
            sgc::Element::Mesh(m) => {
                header(&mut body, &m.node);
                body.extend(
                    m.centre
                        .into_iter()
                        .chain(m.half_extent)
                        .flat_map(f32::to_le_bytes),
                );
                body.extend(3i32.to_le_bytes());
                body.extend(5u32.to_le_bytes());
                for v in m.vertices {
                    body.extend(
                        v.position
                            .into_iter()
                            .chain(v.normal)
                            .flat_map(f32::to_le_bytes),
                    );
                }
                body.extend(m.uvs.into_iter().flatten().flat_map(f32::to_le_bytes));
                body.extend(1i32.to_le_bytes());
                body.extend([0u16, 1, 2].into_iter().flat_map(u16::to_le_bytes));
                body.extend(1i32.to_le_bytes());
                string(&mut body, "paint");
                body.extend([1u32, 0, 3, 0, 0].into_iter().flat_map(u32::to_le_bytes));
                (0, 1)
            }
            _ => unreachable!(),
        };
        bytes.extend(
            [12u32, kind, subtype, 1, body.len() as u32]
                .into_iter()
                .flat_map(u32::to_le_bytes),
        );
        bytes.extend(body);
    }
    let model = sgc::parse(&bytes).unwrap();
    let g = mesh::convert(&model, &p, &s, &c).unwrap();
    assert_eq!(g.stats(None).triangles, 5);
    assert_eq!(g.stats(None).draws, 5);
    assert_eq!(g.vertices.len(), 15);
    assert_eq!(g.vertices[0].uv, [0.0, 0.0]);
    assert!((g.vertices[0].position[1] - 0.65).abs() < 1e-6);
    assert!(sgc::parse(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn hierarchy_rejects_cycles_and_composes_ancestors() {
    let (mut e, p, s, c) = fixture();
    let mut parent = node("parent", 20, 3.0, 0.0, 0.0);
    parent.parent = 0;
    e.push(sgc::Element::Node(parent));
    let sgc::Element::Mesh(m) = &mut e[1] else {
        panic!()
    };
    m.node.parent = 20;
    m.node.world[0][3] = 2.0;
    let g = mesh::convert(&e, &p, &s, &c).unwrap();
    assert!((g.vertices[0].position[0] - 4.9).abs() < 1e-5);
    let sgc::Element::Node(parent) = e.last_mut().unwrap() else {
        panic!()
    };
    parent.parent = 1;
    assert!(mesh::checked_worlds(&e).is_err());
}

#[test]
fn selection_keeps_four_gravel_wheels_and_original_switch_state() {
    let (_, mut p, _, _) = fixture();
    p.extend(ini::parse("[e_lamp_off]\nSwitch=true\nSwitchBox=light_lf\nPredicate=off\nFaceBlock0_Shader=CarLight\nFaceBlock0_Material=paint\n[e_lamp_on]\nSwitch=true\nSwitchBox=light_lf\nPredicate=on\nFaceBlock0_Shader=CarLight\nFaceBlock0_Material=paint\n").unwrap());
    assert!(selection::exterior("e_lamp_off", &p).is_ok());
    assert!(selection::exterior("e_lamp_on", &p).is_err());
    for name in [
        "i_body",
        "ghost_e_body",
        "Plane_shadow",
        "texture_dirt",
        "pc_e_premove",
        "k_glass",
        "d_body",
        "e_wheel_tarmac_lf",
    ] {
        assert!(selection::exterior(name, &p).is_err());
    }
    for (i, side) in selection::SIDES.iter().enumerate() {
        assert_eq!(selection::wheel(&format!("e_wheel_gravel_{side}")), Some(i));
    }
    assert_eq!(selection::slot("e_brakedisc_rf"), 2);
}

#[test]
fn conversion_preserves_indices_uv_normals_and_reflected_winding() {
    let (mut e, p, s, c) = fixture();
    let sgc::Element::Mesh(m) = &mut e[1] else {
        panic!()
    };
    m.node.world[0][0] = -2.0;
    m.vertices[0].normal = [1.0, 1.0, 0.0];
    let g = mesh::convert(&e, &p, &s, &c).unwrap();
    assert_eq!(&g.indices[..3], &[0, 1, 2]);
    assert_eq!(g.vertices[2].uv, [1.0, 1.0]);
    let n = Vec3::from(g.vertices[0].normal);
    assert!(n.dot(Vec3::new(-0.5, 1.0, 0.0).normalize()) > 0.99999);
    assert!(g.indices.iter().all(|&i| (i as usize) < g.vertices.len()));
    assert_eq!(g.vertex_bytes().len(), g.vertices.len() * 32);
}

#[test]
fn invalid_ranges_missing_uv_and_materials_fail_without_substitutes() {
    let (mut e, p, s, c) = fixture();
    let sgc::Element::Mesh(m) = &mut e[1] else {
        panic!()
    };
    m.triangles[0][2] = 99;
    assert!(mesh::convert(&e, &p, &s, &c).is_err());
    let (e, p, _, c) = fixture();
    assert!(mesh::convert(&e, &p, &[], &c).is_err());
    let (mut e, p, s, c) = fixture();
    let sgc::Element::Mesh(m) = &mut e[1] else {
        panic!()
    };
    m.uvs.clear();
    assert!(mesh::convert(&e, &p, &s, &c).is_err());
    assert!(read(
        std::path::Path::new("."),
        &rerbr_content::VehicleDescriptor::default(),
        "../private.dds",
        10
    )
    .is_err());
}

#[test]
fn original_pass_rules_do_not_assume_all_glass_is_transparent() {
    use materials::{pass_for, Pass};
    assert_eq!(pass_for("CarGlass", false), Pass::Opaque);
    assert_eq!(pass_for("CarGlass", true), Pass::Blended);
    assert!(!Pass::Blended.depth_write());
    assert!(Pass::Blended.cull());
    assert!(Pass::Plastic.blended() && Pass::Plastic.depth_write() && !Pass::Plastic.cull());
    assert!(!Pass::InteriorGlass.cull() && !Pass::InteriorGlass.depth_write());
}

#[test]
fn original_model_forward_and_pivots_remain_in_the_proper_frame() {
    let g = geometry();
    assert!(g.pivots[0].x > g.pivots[1].x && g.pivots[0].z < g.pivots[2].z);
    let m = rerbr_render_core::coords::car_to_stage(Vec3::new(4.0, 2.0, 8.0), Vec3::Z);
    assert!((m.transform_vector3(Vec3::NEG_Z) - Vec3::Z).length() < 1e-6);
    assert!((m.determinant() - 1.0).abs() < 1e-6);
}

#[test]
fn geometric_road_pose_and_chase_have_support_on_sloped_terrain() {
    let path = path_fixture(0.05);
    let mut previous = path.frame(0.0).unwrap();
    for d in [0.1, 5.0, 20.0, 40.0, 60.0, 79.0] {
        let f = path.frame(d).unwrap();
        assert!(f.min_wheel_clearance > 0.0049 && f.min_wheel_clearance < 0.0051);
        assert!(f.max_wheel_travel < placement::MAX_WHEEL_TRAVEL);
        assert_eq!(f.boom, 6.0);
        assert!((f.models[0].determinant() - 1.0).abs() < 1e-5);
        assert!(
            (f.camera.target - f.camera.eye)
                .normalize()
                .dot((previous.camera.target - previous.camera.eye).normalize())
                > 0.99
        );
        previous = f;
    }
}

#[test]
fn geometric_segment_query_detects_obstruction() {
    let g = crate::harwood::benchmark::route::Ground::new(vec![[
        Vec3::new(-2.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(0.0, 3.0, 0.0),
    ]])
    .unwrap();
    assert!(!g.segment_clear(Vec3::new(0.0, 1.0, -1.0), Vec3::new(0.0, 1.0, 1.0)));
    assert!(g.segment_clear(Vec3::new(5.0, 1.0, -1.0), Vec3::new(5.0, 1.0, 1.0)));
}

#[test]
fn cached_support_planes_match_original_triangle_queries() {
    let ground = crate::harwood::benchmark::route::Ground::new(vec![
        [
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(-10.0, 2.0, 10.0),
            Vec3::new(10.0, 3.0, 10.0),
        ],
        [
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(10.0, 3.0, 10.0),
            Vec3::new(10.0, 1.0, -10.0),
        ],
    ])
    .unwrap();
    for x in -20..=20 {
        for z in -20..=20 {
            let p = Vec3::new(x as f32 * 0.5, 0.0, z as f32 * 0.5);
            let old = ground.height(p).unwrap();
            let new = ground.support_height(p).unwrap();
            assert!((old - new).abs() < 1e-5);
        }
    }
}

#[test]
fn comparison_reuses_identical_camera_and_does_not_switch_mid_run() {
    let p = path_fixture(0.0);
    let mut comparison = runtime::Comparison::default();
    let a = p.frame(30.0).unwrap();
    comparison.restart(false);
    assert!(comparison.car_visible);
    comparison.restart(true);
    assert!(!comparison.car_visible);
    let b = p.frame(30.0).unwrap();
    assert_eq!(a.camera.eye, b.camera.eye);
    assert_eq!(a.camera.target, b.camera.target);
    assert_eq!(a.models, b.models);
    comparison.restart(true);
    assert!(comparison.car_visible);
}

#[test]
fn missing_wheel_is_a_real_diagnostic_error() {
    let (mut e, p, s, c) = fixture();
    e.pop();
    assert!(mesh::convert(&e, &p, &s, &c).is_err());
}

#[test]
fn unsupported_chase_eye_is_rejected_instead_of_using_the_driver_camera() {
    let p = [[-20.0, -2.0], [-20.0, 100.0], [20.0, 100.0], [20.0, -2.0]]
        .map(|[x, z]| Vec3::new(x, 0.0, z));
    let terrain = crate::harwood::mesh::Mesh {
        vertices: p
            .map(|p| crate::harwood::mesh::Vertex {
                position: p.to_array(),
                color: [255; 4],
                uv0: [0.0; 2],
                uv1: [0.0; 2],
            })
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    };
    let road = vec![[p[0], p[1], p[2]], [p[0], p[2], p[3]]];
    let route = crate::harwood::benchmark::route::Route::new(
        &[0.0, 20.0, 40.0, 60.0, 80.0].map(|z| Vec3::new(0.0, 0.0, z)),
        &terrain,
        &road,
    )
    .unwrap();
    let chase = placement::ChasePath::new(route, &terrain, &geometry()).unwrap();
    assert!(chase.frame(0.0).unwrap_err().contains("CHASE CAMERA"));
}
