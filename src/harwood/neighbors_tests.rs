use super::super::mesh::{Draw, Vertex};
use super::*;

fn tile(id: usize, x: f32, z: f32, texture: u32) -> Node {
    let p = [
        [x, 0.0, z],
        [x, 0.0, z + 10.0],
        [x + 10.0, 0.0, z + 10.0],
        [x + 10.0, 0.0, z],
    ];
    let vertices = p
        .map(|position| Vertex {
            position,
            color: [255; 4],
            uv0: [0.0; 2],
            uv1: [0.0; 2],
        })
        .to_vec();
    let mesh = Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        draws: vec![Draw {
            indices: 0..6,
            textures: [Some(texture), None],
            list: 1,
        }],
        ..Default::default()
    };
    Node {
        id,
        bounds: mesh.bounds().unwrap(),
        mesh,
        roads: vec![
            [p[0], p[1], p[2]].map(Vec3::from),
            [p[0], p[2], p[3]].map(Vec3::from),
        ],
    }
}

#[test]
fn contacts_require_real_edges_not_overlapping_bounds_consecutive_ids_or_corners() {
    let mut fake = tile(270, 100.0, 100.0, 1);
    fake.bounds = tile(0, 0.0, 0.0, 1).bounds;
    let nodes = vec![
        tile(269, 0.0, 0.0, 1),
        fake,
        tile(7, 10.0, 0.0, 1),
        tile(120, 20.0, 0.0, 1),
        tile(121, 0.0, -10.0, 1),
        tile(99, -10.0, 10.0, 1),
    ];
    let graph = graph(&nodes);
    assert!(graph[0].contains_key(&2));
    assert!(!graph[0].contains_key(&1));
    assert!(!graph[0].contains_key(&5)); // Corner contact only.
    assert_eq!(graph[0][&2].edges, 1);
    let route = samples(
        &nodes,
        &[
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(15.0, 0.0, 5.0),
            Vec3::new(25.0, 0.0, 5.0),
        ],
        Vec3::new(5.0, 0.0, 5.0),
    );
    let selected = choose(
        &nodes,
        &graph,
        &route,
        0,
        Budget {
            target_blocks: 3,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        selected
            .nodes
            .iter()
            .map(|&i| nodes[i].id)
            .collect::<Vec<_>>(),
        [269, 7, 120]
    );
    assert_eq!(selected.parents, [(0, 2), (2, 3)]);
}

#[test]
fn geometric_contact_does_not_join_elevated_parallel_surfaces() {
    let a = tile(0, 0.0, 0.0, 1);
    let mut b = tile(1, 10.0, 0.0, 1);
    for v in &mut b.mesh.vertices {
        v.position[1] += 3.0;
    }
    let graph = graph(&[a, b]);
    assert!(graph[0].is_empty());
}

#[test]
fn budgets_reduce_connected_prefix_and_reject_an_over_budget_seed() {
    let nodes = vec![
        tile(80, 0.0, 0.0, 1),
        tile(2, 10.0, 0.0, 1),
        tile(60, 20.0, 0.0, 1),
    ];
    let graph = graph(&nodes);
    let selected = choose(
        &nodes,
        &graph,
        &[],
        0,
        Budget {
            target_blocks: 3,
            max_draws: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(selected.nodes.len(), 2);
    assert_eq!(selected.parents.len(), 1);
    assert!(selected.reasons.contains("draw-call budget"));
    assert!(choose(
        &nodes,
        &graph,
        &[],
        0,
        Budget {
            max_mesh_bytes: 1,
            ..Default::default()
        }
    )
    .is_err());
    assert!(choose(
        &nodes,
        &graph,
        &[],
        0,
        Budget {
            target_blocks: 26,
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn merge_offsets_indices_and_ranges_without_moving_vertices() {
    let a = tile(0, 0.0, 0.0, 1);
    let b = tile(1, 10.0, 0.0, 2);
    let position = b.mesh.vertices[0].position;
    let mut mesh = a.mesh;
    append_mesh(&mut mesh, b.mesh);
    assert_eq!(mesh.vertices[4].position, position);
    assert_eq!(&mesh.indices[6..], &[4, 5, 6, 4, 6, 7]);
    assert_eq!(mesh.draws[1].indices, 6..12);
    assert_eq!(mesh.draws[1].textures, [Some(2), None]);
}

#[test]
fn texture_budget_prunes_growth_leaves_and_keeps_aliases_shared() {
    let nodes = vec![
        tile(0, 0.0, 0.0, 1),
        tile(1, 10.0, 0.0, 2),
        tile(2, 20.0, 0.0, 3),
    ];
    let graph = graph(&nodes);
    let mut selected = choose(
        &nodes,
        &graph,
        &[],
        0,
        Budget {
            target_blocks: 3,
            ..Default::default()
        },
    )
    .unwrap();
    let aliases = BTreeMap::from([(1, 1), (2, 2), (3, 2)]);
    let sizes = BTreeMap::from([(1, 100usize), (2, 500)]);
    let kept = trim_textures(&mut selected, &nodes, &aliases, &sizes, 100).unwrap();
    assert_eq!(kept, BTreeSet::from([1]));
    assert_eq!(selected.nodes, [0]);
    assert!(selected.parents.is_empty());
    assert!(trim_textures(&mut selected, &nodes, &aliases, &sizes, 99).is_err());
}

#[test]
fn route_coverage_tests_triangles_and_height_not_only_boxes() {
    let triangle = [
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 10.0),
    ];
    assert!(near_road(Vec3::new(2.0, 0.0, 2.0), triangle));
    assert!(!near_road(Vec3::new(9.0, 0.0, 9.0), triangle));
    assert!(!near_road(Vec3::new(2.0, 6.0, 2.0), triangle));
}

#[test]
#[ignore = "requires private Maps; explicitly set HARWOOD_MAPS"]
fn real_dds_aliases_share_one_payload() {
    let maps = std::env::var_os("HARWOOD_MAPS").expect("set HARWOOD_MAPS");
    let shared =
        asset_probe::load_diffuse_shared(Path::new(&maps), &BTreeSet::from([0, 12, 22, 23]))
            .unwrap();
    assert_eq!(shared.textures.len(), 2);
    assert_eq!(shared.aliases[&0], shared.aliases[&22]);
    assert_eq!(shared.aliases[&12], shared.aliases[&23]);
}
