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

#[test]
fn vehicle_texture_upload_converts_bgra_and_keeps_lamp_alpha() {
    let mut source = vec![0; 128];
    source[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (12, 1),
        (16, 1),
        (76, 32),
        (80, 0x41),
        (88, 32),
        (92, 0xff0000),
        (96, 0xff00),
        (100, 0xff),
        (104, 0xff000000),
    ] {
        source[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    source.extend_from_slice(&[12, 34, 56, 78]);
    let texture = prepare_texture("Cars/test/lamp.dds", "Cars/test/lamp.dds", source).unwrap();
    assert_eq!(texture.info.format, dds::Format::Rgba8);
    assert_eq!(dds::parse(&texture.bytes).unwrap().data, &[56, 34, 12, 78]);
}

#[test]
fn auxiliary_lamp_faces_keep_their_own_materials_and_unmatched_geometry() {
    let (mut elements, mut parts, mut shaders, mut settings) = fixture();
    let mut lamp = triangle(node("e_aux_lamp", 6, 0.0, 0.5, 1.0));
    for (index, name) in ["glass", "paint", "unmapped"].into_iter().enumerate() {
        let mut face = lamp.faces[0].clone();
        face.name = name.into();
        face.start_index = (3 * index) as u32;
        if index != 0 {
            lamp.triangles.push([0, 1, 2]);
            lamp.faces.push(face);
        } else {
            lamp.faces[0] = face;
        }
    }
    lamp.vertices.extend_from_within(..3);
    lamp.triangles[2] = [3, 4, 5];
    elements.push(sgc::Element::Mesh(lamp));
    parts.extend(ini::parse("[e_aux_lamp]\nFaceBlock0_Shader=CarTextureNoDirt\nFaceBlock0_Material=paint\nFaceBlock1_Shader=CarLight\nFaceBlock1_Material=glass\n").unwrap());
    shaders.extend(ini::parse("[glass]\nOpacityChannel=true\n[glass_diffuse_bitmap]\nFilename=lamp.dds\n[TSDefault]\nlamp.dds=Cars/fixture/lamp.dds\n").unwrap());
    settings.extend(ini::parse("[CarLight]\n").unwrap());

    let geometry = mesh::convert_for_mode(&elements, &parts, &shaders, &settings, true).unwrap();
    let material = |first| {
        let draw = geometry
            .draws
            .iter()
            .find(|d| d.indices.start == first)
            .unwrap();
        &geometry.materials[draw.material]
    };
    assert_eq!(
        material(15).path.as_deref(),
        Some("cars\\fixture\\lamp.dds")
    );
    assert_eq!(material(15).pass, materials::Pass::Blended);
    assert_eq!(
        material(18).path.as_deref(),
        Some("cars\\fixture\\base.dds")
    );
    assert!(material(21).path.is_none());
    assert_eq!(geometry.stats(None).triangles, 8);
    assert_eq!(geometry.unresolved_faces.len(), 1);
    assert!(geometry.unresolved_faces[0].contains("e_aux_lamp FaceBlock2"));
}

#[test]
fn merged_face_uses_first_slot_only_when_surplus_slots_agree_on_material() {
    let (mut elements, mut parts, shaders, mut settings) = fixture();
    let sgc::Element::Mesh(body) = &mut elements[1] else {
        panic!("fixture body mesh missing");
    };
    body.faces[0].name = "editor alias".into();
    let part = &mut parts[0];
    part.entries.extend([
        ("FaceBlock1_Shader".into(), "CarRim".into()),
        ("FaceBlock1_Material".into(), "PAINT".into()),
    ]);
    settings.extend(ini::parse("[CarRim]\n").unwrap());
    let converted = mesh::convert_for_mode(&elements, &parts, &shaders, &settings, true).unwrap();
    let body_draw = converted.draws.iter().find(|draw| draw.slot == 0).unwrap();
    let material = &converted.materials[body_draw.material];
    assert_eq!(material.path.as_deref(), Some("cars\\fixture\\base.dds"));
    assert_eq!(material.shader, "CarTextureNoDirt");
    assert!(converted.unresolved_faces.is_empty());

    parts[0].entries.last_mut().unwrap().1 = "different material".into();
    let converted = mesh::convert_for_mode(&elements, &parts, &shaders, &settings, true).unwrap();
    assert_eq!(converted.unresolved_faces.len(), 1);
    assert_eq!(converted.stats(None).triangles, 5);
}

#[test]
fn showroom_texture_preserves_isolation_and_verified_aliases() {
    let root = std::env::temp_dir().join(format!("rerbr-showroom-texture-{}", std::process::id()));
    std::fs::create_dir_all(root.join("Cars/Selected/textures")).unwrap();
    std::fs::create_dir_all(root.join("Cars/Other/textures")).unwrap();
    let local = root.join("Cars/Selected/textures/paint.dds");
    std::fs::write(&local, b"selected").unwrap();
    std::fs::write(root.join("Cars/Other/textures/paint.dds"), b"other").unwrap();

    let foreign = "Cars/Other/textures/paint.dds";
    let mut source = ShowroomTextures {
        root: &root,
        model_dir: "Cars/Selected",
        model: ("Cars/Selected/car.sgc", b"selected model"),
        shaders: ("Cars/Selected/car_shaders.ini", b"selected shader mapping"),
        aliases: BTreeMap::new(),
    };
    let (resolved, bytes) = source.read(foreign).unwrap();
    assert_eq!(resolved, "Cars/Selected/textures/paint.dds");
    assert_eq!(bytes, b"selected");
    assert!(read_showroom_file(&root, "Cars/Selected", foreign, 128).is_err());
    std::fs::remove_file(&local).unwrap();
    assert!(source.read(foreign).is_err());
    assert!(source.read("../paint.dds").is_err());

    // A same-named model alone does not establish that its texture belongs here.
    std::fs::write(root.join("Cars/Other/car.sgc"), source.model.1).unwrap();
    std::fs::write(
        root.join("Cars/Other/car_shaders.ini"),
        b"unrelated mapping",
    )
    .unwrap();
    source.aliases.clear();
    assert!(source.read(foreign).is_err());

    // The exact declared DDS wins over a local texture with different bytes.
    std::fs::write(root.join("Cars/Other/car_shaders.ini"), source.shaders.1).unwrap();
    std::fs::write(local, b"local skin").unwrap();
    source.aliases.clear();
    assert_eq!(
        source.read(foreign).unwrap(),
        (foreign.into(), b"other".to_vec())
    );
    std::fs::remove_file(root.join("Cars/Other/textures/paint.dds")).unwrap();
    assert_eq!(
        source.read(foreign).unwrap(),
        (
            "Cars/Selected/textures/paint.dds".into(),
            b"local skin".to_vec()
        )
    );
    std::fs::remove_dir_all(root).unwrap();
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

#[test]
fn focus_2006_loads_own_synthetic_sgc_and_textures_never_impreza() {
    fn encode_sgc(elements: &[sgc::Element]) -> Vec<u8> {
        fn push_str(out: &mut Vec<u8>, s: &str) {
            out.extend(s.as_bytes());
            out.push(0);
        }
        fn push_header(out: &mut Vec<u8>, n: &sgc::Node) {
            push_str(out, &n.name);
            out.extend(n.id.to_le_bytes());
            out.extend(n.parent.to_le_bytes());
            out.extend(n.transform.words().into_iter().flat_map(f32::to_le_bytes));
            out.extend(n.world.into_iter().flatten().flat_map(f32::to_le_bytes));
        }
        let mut bytes = sgc::BANNER.to_vec();
        for e in elements {
            let mut body = Vec::new();
            let (kind, subtype) = match e {
                sgc::Element::Node(n) => {
                    push_header(&mut body, n);
                    (3, 13)
                }
                sgc::Element::Mesh(m) => {
                    push_header(&mut body, &m.node);
                    body.extend(
                        m.centre
                            .into_iter()
                            .chain(m.half_extent)
                            .flat_map(f32::to_le_bytes),
                    );
                    body.extend(3i32.to_le_bytes());
                    body.extend(5u32.to_le_bytes());
                    for v in &m.vertices {
                        body.extend(
                            v.position
                                .into_iter()
                                .chain(v.normal)
                                .flat_map(f32::to_le_bytes),
                        );
                    }
                    body.extend(m.uvs.iter().flatten().copied().flat_map(f32::to_le_bytes));
                    body.extend(1i32.to_le_bytes());
                    body.extend([0u16, 1, 2].into_iter().flat_map(u16::to_le_bytes));
                    body.extend(1i32.to_le_bytes());
                    push_str(&mut body, "paint");
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
        bytes
    }

    fn minimal_dxt1_4x4() -> Vec<u8> {
        let mut dds = vec![0u8; 128 + 8];
        dds[0..4].copy_from_slice(b"DDS ");
        dds[4..8].copy_from_slice(&124u32.to_le_bytes());
        dds[8..12].copy_from_slice(&0x1007u32.to_le_bytes());
        dds[12..16].copy_from_slice(&4u32.to_le_bytes());
        dds[16..20].copy_from_slice(&4u32.to_le_bytes());
        dds[28..32].copy_from_slice(&1u32.to_le_bytes());
        dds[76..80].copy_from_slice(&32u32.to_le_bytes());
        dds[80..84].copy_from_slice(&0x4u32.to_le_bytes());
        dds[84..88].copy_from_slice(b"DXT1");
        dds
    }

    let root = std::env::temp_dir().join(format!("rerbr-focus-car-load-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let rbr = root.join("RBR");
    std::fs::create_dir_all(rbr.join("Cars/impreza03")).unwrap();
    std::fs::write(
        rbr.join("Cars/Cars.ini"),
        "[Car05]\r\nFileName = \"Cars\\impreza03\\impreza03.sgc\"\r\nIniFile = \"Cars\\impreza03\\impreza03.ini\"\r\nShaderFile = \"Cars\\impreza03\\impreza03_shaders.ini\"\r\nShaderSettings = \"Cars\\impreza03\\impreza03_shader_settings\"\r\nTexturePath = \"Cars\\impreza03\\Textures\\\"\r\n",
    )
    .unwrap();
    // Corrupt impreza03.sgc so any accidental read of Subaru fails immediately.
    std::fs::write(rbr.join("Cars/impreza03/impreza03.sgc"), b"NOT_FOCUS").unwrap();

    let focus = rerbr_content::VehicleDescriptor::ford_focus_2006();
    std::fs::create_dir_all(rbr.join("Cars/FORD_FOCUS_2006/textures")).unwrap();
    let (elements, _, _, _) = fixture();
    std::fs::write(rbr.join(&focus.model), encode_sgc(&elements)).unwrap();
    let mut parts = String::from(
        "[Cam_external]\nFOV=1.2\nNear=0.1\nPos=0 2 3\nTarget=0 1 -1\nUp=0 1 0\n\
         [Cam_bonnet2]\nFOV=1.9\nNear=0.15\nPos=0 1 -2\nTarget=0 1 -5\nUp=0 1 0\n\
         [Cam_internal]\nFOV=1.2\nNear=0.1\nPos=0 1 -1\nTarget=0 1 -4\nUp=0 1 0\n\
         [e_body]\nFaceBlock0_Shader=CarTextureNoDirt\nFaceBlock0_Material=paint\n",
    );
    for side in selection::SIDES {
        parts.push_str(&format!(
            "[e_wheel_gravel_{side}]\nFaceBlock0_Shader=CarTextureNoDirt\nFaceBlock0_Material=paint\n"
        ));
    }
    std::fs::write(rbr.join(&focus.parts), parts).unwrap();
    std::fs::write(
        rbr.join(&focus.shaders),
        "[paint]\nOpacityChannel=false\n[paint_diffuse_bitmap]\nFilename=focus_wrc06_external.dds\n[TextureSets]\nTextureSet0=TSDefault\n[TSDefault]\nfocus_wrc06_external.dds=Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds\n",
    )
    .unwrap();
    std::fs::write(
        rbr.join(&focus.shader_settings),
        "[CarTextureNoDirt]\nKa=1\n",
    )
    .unwrap();
    std::fs::write(
        rbr.join("Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds"),
        minimal_dxt1_4x4(),
    )
    .unwrap();

    let car = load_vehicle_for_mode(&rbr, &focus, true)
        .expect("Ford Focus 2006 must load its own FORD_FOCUS_2006 SGC/textures");
    assert_eq!(car.vehicle.id, "ford-focus-rs-wrc-2006");
    assert!(car.files.iter().any(|f| f.contains("FORD_FOCUS_2006")));
    assert!(!car.files.iter().any(|f| f.contains("impreza03")));
    assert!(car
        .textures
        .keys()
        .any(|k| k.contains("focus_wrc06_external.dds")));
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(feature = "physics-stage")]
#[test]
fn real_pc_toyota_models_and_cockpits_load_without_fallback() {
    let rbr = Path::new(r"C:\Richard Burns Rally");
    if !rbr.is_dir() {
        return;
    }

    // 1. Toyota GR Yaris Rally2 (exterior + cockpit material resolution)
    if rbr.join("Cars/yaris_rally2/yaris_rally2.sgc").is_file() {
        let mut yaris_r2 = rerbr_content::VehicleDescriptor::ford_focus_2006();
        yaris_r2.id = "rsf-toyota-gr-yaris-rally2".into();
        yaris_r2.name = "Toyota GR Yaris Rally2".into();
        yaris_r2.model = "Cars/yaris_rally2/yaris_rally2.sgc".into();
        yaris_r2.parts = "Cars/yaris_rally2/yaris_rally2.ini".into();
        yaris_r2.shaders = "Cars/yaris_rally2/yaris_rally2_shaders.ini".into();
        yaris_r2.shader_settings =
            "Cars/yaris_rally2/yaris_rally2_shader_settings_clear.ini".into();
        yaris_r2.required_textures.clear();
        yaris_r2.optional_textures.clear();
        yaris_r2.audio_index = "AudioFMOD/Peugeot_208_Rally4.bank".into();
        yaris_r2.rsf_pack = Some("Toyota_GR_Yaris_Rally2_ngp7".into());

        let car = load_vehicle_for_mode(rbr, &yaris_r2, true)
            .expect("Toyota GR Yaris Rally2 model must load cleanly");
        assert!(
            car.cockpit_error.is_none(),
            "Yaris Rally2 cockpit must load cleanly: {:?}",
            car.cockpit_error
        );
        assert!(
            car.cockpit.is_some(),
            "Yaris Rally2 must have steering node"
        );
        assert!(car.geometry.pivots.iter().all(|p| p.length() > 0.5));
        eprintln!(
            "YARIS RALLY2 3D PASS: draws={} textures={} cockpit={} pivots={:?}",
            car.geometry.drawn.len(),
            car.textures.len(),
            car.cockpit.is_some(),
            car.geometry.pivots
        );
    }

    // 2. Toyota Yaris WRC 2018 (exterior + cockpit if present on PC)
    if rbr.join("Cars/YARIS_WRC18/yaris_wrc.sgc").is_file() {
        let mut yaris_wrc = rerbr_content::VehicleDescriptor::ford_focus_2006();
        yaris_wrc.id = "rsf-toyota-yaris-wrc-2018".into();
        yaris_wrc.name = "Toyota Yaris WRC 2018".into();
        yaris_wrc.model = "Cars/YARIS_WRC18/yaris_wrc.sgc".into();
        yaris_wrc.parts = "Cars/YARIS_WRC18/yaris_wrc.ini".into();
        yaris_wrc.shaders = "Cars/YARIS_WRC18/yaris_wrc_shaders.ini".into();
        yaris_wrc.shader_settings = "Cars/YARIS_WRC18/yaris_wrc_shader_settings_clear.ini".into();
        yaris_wrc.required_textures.clear();
        yaris_wrc.optional_textures.clear();
        yaris_wrc.audio_index = "AudioFMOD/SIN_BANCO_FMOD.bank".into();
        yaris_wrc.rsf_pack = Some("Toyota_Yaris_WRC_2018_ngp6".into());

        let car = load_vehicle_for_mode(rbr, &yaris_wrc, true)
            .expect("Toyota Yaris WRC 2018 model must load cleanly");
        assert!(
            car.cockpit_error.is_none(),
            "Yaris WRC 2018 cockpit error: {:?}",
            car.cockpit_error
        );
        assert!(
            car.cockpit.is_some(),
            "Yaris WRC 2018 must have steering node"
        );
        eprintln!(
            "YARIS WRC 2018 3D PASS: draws={} textures={} cockpit={} pivots={:?}",
            car.geometry.drawn.len(),
            car.textures.len(),
            car.cockpit.is_some(),
            car.geometry.pivots
        );
    }
}

#[test]
fn audit_all_installed_rsf_showroom_models_if_pc_present() {
    let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
    let cars_dir = rbr.join("Cars");
    if !cars_dir.is_dir() {
        return;
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&cars_dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut total_with_sgc = 0usize;
    let mut passed = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();
    for dir in dirs {
        let Some(folder) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let sgc = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sgc")));
        let Some(sgc_path) = sgc else {
            continue;
        };
        let Some(stem) = sgc_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        total_with_sgc += 1;
        let mut desc = rerbr_content::VehicleDescriptor::ford_focus_2006();
        desc.id = format!("audit-{}", folder.to_ascii_lowercase());
        desc.name = folder.to_string();
        desc.model = format!("Cars/{folder}/{stem}.sgc");
        desc.parts = format!("Cars/{folder}/{stem}.ini");
        desc.shaders = format!("Cars/{folder}/{stem}_shaders.ini");
        desc.shader_settings = format!("Cars/{folder}/{stem}_shader_settings_clear.ini");
        desc.required_textures.clear();
        desc.optional_textures.clear();
        match load_showroom_model(rbr, &desc) {
            Ok(car) => {
                passed += 1;
                let _ = car;
            }
            Err(err) => {
                failures.push((format!("{folder}/{stem}"), err));
            }
        }
    }
    eprintln!("=== 3D SHOWROOM AUDIT: {passed}/{total_with_sgc} SGC models loaded cleanly ===");
    for (name, err) in &failures {
        eprintln!("  FAIL {name}: {err}");
    }
    assert!(
        passed >= 80,
        "expected most installed RSF SGC models to load cleanly, got {passed}/{total_with_sgc}"
    );
}

#[test]
fn verify_audi_i20_bmw_c2_and_all_cockpits_if_pc_present() {
    let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
    let transfer_rbr = std::path::Path::new(
        r"C:\Users\adria\Desktop\RBR Mobile - Autos Listos para iPad\LISTOS PARA CONDUCIR\RBRRRRR",
    );
    if !rbr.join("Cars").is_dir() || !transfer_rbr.join("rsfdata/cache/cars.json").is_file() {
        return;
    }
    // 1. Audi 200 quattro GrpA: front wheel visual offset aligns with e_wheelhang_lf/rf
    let audi_desc = crate::driving::rsf_catalog::discover(transfer_rbr)
        .into_iter()
        .find(|c| c.descriptor.id == "rsf-audi-200-quattro-grpa")
        .expect("Audi 200 quattro GrpA in catalog")
        .descriptor;
    let audi = load_vehicle_for_mode(rbr, &audi_desc, true).expect("Audi 200 loads");
    assert!(
        audi.geometry.wheel_visual_offsets[0].z < -0.10
            && audi.geometry.wheel_visual_offsets[1].z < -0.10,
        "Audi 200 front wheel visual Z offset must shift forward into front wheel arch: {:?}",
        audi.geometry.wheel_visual_offsets
    );

    // 2. Hyundai i20 Coupe WRC 2020: swapped rf/lb nodes corrected to true geometric corners
    let i20_desc = crate::driving::rsf_catalog::discover(transfer_rbr)
        .into_iter()
        .find(|c| c.descriptor.id == "rsf-hyundai-i20-coupe-wrc-2020")
        .expect("Hyundai i20 WRC 2020 in catalog")
        .descriptor;
    let i20 = load_vehicle_for_mode(rbr, &i20_desc, true).expect("Hyundai i20 WRC 2020 loads");
    let p = i20.geometry.pivots;
    let mid_z = (p[0].z + p[1].z + p[2].z + p[3].z) * 0.25;
    assert!(p[0].x > 0.1 && p[0].z < mid_z, "LF pivot wrong: {:?}", p[0]);
    assert!(
        p[1].x < -0.1 && p[1].z < mid_z,
        "RF pivot wrong: {:?}",
        p[1]
    );
    assert!(p[2].x > 0.1 && p[2].z > mid_z, "LB pivot wrong: {:?}", p[2]);
    assert!(
        p[3].x < -0.1 && p[3].z > mid_z,
        "RB pivot wrong: {:?}",
        p[3]
    );
    assert!(i20.cockpit.is_some(), "i20 WRC 2020 cockpit must load");

    // 3. Audit ALL transfer-ready RSF vehicles' cockpits (including BMW M3 E30, C2 R2 Max, Polo WRC)
    let mut ok_count = 0usize;
    let mut total = 0usize;
    for car in crate::driving::rsf_catalog::discover(transfer_rbr) {
        total += 1;
        let loaded = load_vehicle_for_mode(rbr, &car.descriptor, true)
            .unwrap_or_else(|e| panic!("Failed to load {}: {e}", car.descriptor.id));
        if loaded.cockpit.is_some() {
            ok_count += 1;
        } else {
            eprintln!(
                "COCKPIT AUDIT {}: MISSING ({:?})",
                car.descriptor.id, loaded.cockpit_error
            );
        }
        if matches!(
            car.descriptor.id.as_str(),
            "rsf-bmw-m3-e30-grpa" | "rsf-citroen-c2-r2-max" | "rsf-vw-polo-r-wrc-2016"
        ) {
            assert!(
                loaded.cockpit.is_some(),
                "{} cockpit must now load cleanly, got {:?}",
                car.descriptor.id,
                loaded.cockpit_error
            );
        }
    }
    eprintln!("=== COCKPIT AUDIT SUMMARY: {ok_count}/{total} RSF cars have working cockpits ===");
    assert_eq!(
        ok_count, total,
        "All {total} transfer-ready RSF cars should have working cockpits"
    );
}
