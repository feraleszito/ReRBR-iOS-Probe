#![cfg(feature = "rerbr-3d")]

#[path = "../src/scene_geometry.rs"]
mod scene_geometry;

use rerbr_render_core::Vec3;
use scene_geometry::{floor_and_cube, view_projection, EYE};

#[test]
fn procedural_mesh_has_valid_finite_nondegenerate_triangles() {
    let mesh = floor_and_cube();
    assert_eq!(mesh.vertices.len(), 28);
    assert_eq!(mesh.indices.len(), 42);
    for vertex in &mesh.vertices {
        assert!(vertex.position.iter().all(|v| v.is_finite()));
        assert!(vertex.color.iter().all(|v| (0.0..=1.0).contains(v)));
    }
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
            .map(|index| Vec3::from(mesh.vertices[usize::from(index)].position));
        assert!((b - a).cross(c - a).length() > 1e-5);
    }
}

#[test]
fn cube_faces_point_outward_and_floor_faces_point_up() {
    let mesh = floor_and_cube();
    for (index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
            .map(|i| Vec3::from(mesh.vertices[usize::from(i)].position));
        let normal = (b - a).cross(c - a).normalize();
        if index < 12 {
            let centre = (a + b + c) / 3.0;
            assert!(normal.dot(centre - Vec3::new(0.0, 0.5, 0.0)) > 0.49);
        } else {
            assert!(normal.dot(Vec3::Y) > 0.99);
            assert_eq!((a.y, b.y, c.y), (0.0, 0.0, 0.0));
        }
    }
}

#[test]
fn visible_faces_project_clockwise_for_the_rerbr_culling_convention() {
    assert_eq!(rerbr_render_core::coords::FRONT_FACE, wgpu::FrontFace::Cw);
    let mesh = floor_and_cube();
    let matrix = view_projection(1.55);
    let mut visible = 0;
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
            .map(|i| Vec3::from(mesh.vertices[usize::from(i)].position));
        if (b - a).cross(c - a).dot(EYE - (a + b + c) / 3.0) <= 0.0 {
            continue;
        }
        let [a, b, c] = [a, b, c].map(|p| {
            let clip = matrix * p.extend(1.0);
            clip.truncate() / clip.w
        });
        let signed_area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        assert!(signed_area < 0.0, "visible front faces must be clockwise");
        visible += 1;
    }
    assert_eq!(visible, 8); // Three visible cube faces and the floor, two triangles each.
}

#[test]
fn fixed_camera_keeps_the_whole_cube_inside_portrait_and_landscape_views() {
    let mesh = floor_and_cube();
    for aspect in [0.65, 0.75, 4.0 / 3.0, 1.55] {
        let matrix = view_projection(aspect);
        for vertex in &mesh.vertices[..24] {
            let clip = matrix * Vec3::from(vertex.position).extend(1.0);
            assert!(clip.w > 0.0);
            let ndc = clip.truncate() / clip.w;
            assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0, "{aspect}: {ndc:?}");
            assert!((0.0..1.0).contains(&ndc.z));
        }
    }
}

#[test]
fn reverse_depth_places_the_cube_ahead_of_floor_on_the_same_view_ray() {
    let cube = Vec3::new(0.0, 0.5, -0.5);
    let ray = cube - EYE;
    let floor_distance = -EYE.y / ray.y;
    assert!(floor_distance > 1.0);
    let floor = EYE + ray * floor_distance;
    let matrix = view_projection(1.55);
    let project = |point: Vec3| {
        let clip = matrix * point.extend(1.0);
        clip.truncate() / clip.w
    };
    let (cube, floor) = (project(cube), project(floor));
    assert!((cube.x - floor.x).abs() < 1e-5 && (cube.y - floor.y).abs() < 1e-5);
    assert!(cube.z > floor.z && floor.z > 0.0);
}
