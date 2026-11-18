//! Full resident Harwood geometry, with per-draw spatial culling at submission.
//! GPL-3.0-or-later adaptations from reRBR stage/{mod,mesh}.rs; see STAGE.md.
use super::{
    mesh::{self, Bounds, Draw, Mesh, Vertex},
    Camera, LoadedBlock,
};
use crate::asset_probe;
use rerbr_asset_formats::lbs;
use rerbr_render_core::{coords, Mat4, Vec3};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Default)]
pub struct Report {
    pub blocks: usize,
    pub objects: usize,
    pub interactive_instances: usize,
    pub withheld_instances: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub draws: usize,
    pub textures: usize,
    pub resident_bytes: usize,
    pub route_points: usize,
    pub supported_points: usize,
    pub collision_matched_points: usize,
    pub maximum_ground_gap: f32,
    pub sky_items_withheld: usize,
    pub water_items_withheld: usize,
    pub animated_objects_withheld: usize,
}
pub struct Stage {
    pub render: LoadedBlock,
    /// Terrain only: vegetation cards must never become camera support surfaces.
    pub ground: Mesh,
    pub route_points: Vec<Vec3>,
    pub report: Report,
}

pub fn append(out: &mut Mesh, mut part: Mesh) {
    let base = out.vertices.len() as u32;
    let first = out.indices.len();
    out.vertices.append(&mut part.vertices);
    out.indices
        .extend(part.indices.into_iter().map(|i| i + base));
    for mut d in part.draws {
        d.indices = d.indices.start + first..d.indices.end + first;
        out.draws.push(d);
    }
}

/// Same baked vertex layout/index semantics as upstream object_mesh/buffer_mesh.
fn object_mesh(
    b: &lbs::Buffers<'_>,
    color: usize,
    uv: usize,
    texture: Option<u32>,
    address: u8,
    placement: Mat4,
) -> Result<Mesh, String> {
    let mut m = Mesh::default();
    if !placement.is_finite() || b.stride < uv + 8 || b.stride < color + 4 {
        return Err("STAGE: invalid object layout/placement".into());
    }
    for i in 0..b.vertex_count {
        let bytes = b
            .vertices
            .get(i * b.stride..(i + 1) * b.stride)
            .ok_or("truncated object vertex")?;
        let f = |at| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let p = placement.transform_point3(Vec3::from(b.position(i).ok_or("object position")?));
        m.vertices.push(Vertex {
            position: p.to_array(),
            color: coords::rgba_from_d3dcolor(bytes[color..color + 4].try_into().unwrap()),
            uv0: [f(uv), f(uv + 4)],
            uv1: [0.0; 2],
        });
    }
    if !b.index_count().is_multiple_of(3) {
        return Err("STAGE: incomplete object triangle".into());
    }
    for i in 0..b.index_count() {
        let v = u32::from(b.index(i).ok_or("object index")?);
        if v as usize >= m.vertices.len() {
            return Err("STAGE: object index out of range".into());
        }
        m.indices.push(v);
    }
    if !m.indices.is_empty() {
        m.draws.push(Draw {
            indices: 0..m.indices.len(),
            textures: [texture, None],
            list: 2,
            cutout: true,
            address,
        });
        m.bounds()?;
    }
    Ok(m)
}

pub fn load(maps: &Path) -> Result<Stage, String> {
    load_selected(maps, &rerbr_content::StageDescriptor::default())
}
pub fn load_selected(maps: &Path, stage: &rerbr_content::StageDescriptor) -> Result<Stage, String> {
    stage.validate_current_backend()?;
    let bytes = asset_probe::read_stage_stem(maps, &stage.stem)?;
    let lbs = lbs::parse(&bytes).map_err(|e| e.to_string())?;
    let mut terrain = Mesh::default();
    let mut scene = Mesh::default();
    let mut report = Report::default();
    let start = lbs
        .chunks
        .iter()
        .find_map(|c| {
            if let lbs::Chunk::StartLocation(s) = c {
                Some(Vec3::from(s.position))
            } else {
                None
            }
        })
        .ok_or("STAGE: missing spawn")?;
    for c in &lbs.chunks {
        if let lbs::Chunk::Geometry(g) = c {
            for (id, block) in g.blocks.iter().enumerate() {
                let mesh = mesh::block_mesh(block);
                if mesh.bad_triangles != 0 || mesh.skipped_draws != 0 {
                    return Err(format!("STAGE: unsupported terrain block {id}"));
                }
                mesh.bounds()?;
                append(&mut terrain, mesh);
                report.blocks += 1;
            }
        }
    }
    // Retain a compact CPU ground index without scenery. GPU geometry is resident once.
    scene.vertices.clone_from(&terrain.vertices);
    scene.indices.clone_from(&terrain.indices);
    scene.draws = terrain
        .draws
        .iter()
        .map(|d| Draw {
            indices: d.indices.clone(),
            textures: d.textures,
            list: d.list,
            cutout: false,
            address: 1,
        })
        .collect();
    let names = asset_probe::diffuse_names_stem(maps, &stage.stem)?;
    for c in &lbs.chunks {
        match c {
            lbs::Chunk::Objects(o) => {
                for item in o.lanes.iter().flat_map(|l| l.runs.iter().flatten()) {
                    let layout = item
                        .vertex_layout()
                        .ok_or("STAGE: unsupported static object layout")?;
                    let kind = lbs::KindState::decode(item.kind);
                    append(
                        &mut scene,
                        object_mesh(
                            &item.buffers,
                            layout.color,
                            layout.texcoord0,
                            item.texture_index(),
                            kind.slot0,
                            Mat4::IDENTITY,
                        )?,
                    );
                    report.objects += 1;
                }
            }
            lbs::Chunk::Interactive(i) => {
                for object in &i.objects {
                    // Match upstream's evidence-based support gate, not guessed materials.
                    let supported = !object.draws.is_empty()
                        && object.draws.iter().all(|d| {
                            d.flags == 0x15
                                && d.vertex_size == 24
                                && d.texture1.is_none()
                                && d.texture0.is_some_and(|t| names.contains_key(&t))
                        });
                    if !supported {
                        report.withheld_instances += object.visibility.len();
                        eprintln!("physics-stage WITHHELD: {} instances={} unsupported interactive material/layout", String::from_utf8_lossy(object.name), object.visibility.len());
                        continue;
                    }
                    for v in &object.visibility {
                        let matrix = Mat4::from_cols_array(&v.matrix);
                        for d in &object.draws {
                            let b = lbs::Buffers {
                                vertices: d.vertices,
                                indices: d.indices,
                                vertex_count: d.vertex_count,
                                stride: d.vertex_size,
                            };
                            append(&mut scene, object_mesh(&b, 12, 16, d.texture0, 1, matrix)?);
                        }
                        report.interactive_instances += 1;
                    }
                }
            }
            lbs::Chunk::Sky(sky) => report.sky_items_withheld += sky.items.len(),
            lbs::Chunk::Water(water) => {
                report.water_items_withheld +=
                    water.faces.iter().map(|f| f.items.len()).sum::<usize>()
            }
            lbs::Chunk::AnimationObjects(a) => report.animated_objects_withheld += a.items.len(),
            _ => (),
        }
    }
    if report.blocks == 0 || (!stage.is_school_stage() && report.blocks != stage.terrain_blocks) {
        return Err(format!(
            "STAGE: expected {} blocks for {}, got {}",
            stage.terrain_blocks, stage.id, report.blocks
        ));
    }
    // Ensure every real route sample has visible terrain underneath before removing
    // the 20-block restriction. This does not claim every object collider is visible.
    let triangles = terrain
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]].map(|i| Vec3::from(terrain.vertices[i as usize].position)))
        .collect();
    let ground = super::benchmark::route::Ground::new(triangles)?;
    let collision = rerbr_sim::drive::collision_triangles_for(
        &rerbr_sim::inputs::Paths {
            maps: maps.to_owned(),
            install: std::path::PathBuf::new(),
            overlay: None,
        },
        stage,
    )?;
    let collision = super::benchmark::route::Ground::new(
        collision
            .into_iter()
            .map(|t| t.map(|p| coords::physics_to_stage().transform_point3(Vec3::from(p))))
            .collect(),
    )?;
    let mut route_points = Vec::new();
    for p in lbs
        .chunks
        .iter()
        .filter_map(|c| {
            if let lbs::Chunk::DrivelinePoints(d) = c {
                Some(d)
            } else {
                None
            }
        })
        .flat_map(|d| &d.points)
    {
        report.route_points += 1;
        let p = Vec3::from(*p);
        route_points.push(p);
        if let Some(height) = ground.support_height(p) {
            report.supported_points += 1;
            if let Some(col) = collision.support_height(p) {
                let gap = (col - height).abs();
                report.maximum_ground_gap = report.maximum_ground_gap.max(gap);
                if gap <= 0.5 {
                    report.collision_matched_points += 1;
                }
            }
        }
    }
    if report.route_points == 0 || report.route_points != report.supported_points {
        return Err(format!(
            "STAGE: route terrain coverage {}/{}; keep physics-drive fallback",
            report.supported_points, report.route_points
        ));
    }
    if report.collision_matched_points != report.route_points {
        return Err(format!(
            "STAGE: visual/COL ground agreement {}/{}; max gap {:.3}m; expansion withheld",
            report.collision_matched_points, report.route_points, report.maximum_ground_gap
        ));
    }
    let needed: BTreeSet<_> = scene
        .draws
        .iter()
        .flat_map(|d| d.textures.into_iter().flatten())
        .collect();
    let shared = asset_probe::load_diffuse_shared_stem(maps, &stage.stem, &needed)?;
    for d in &mut scene.draws {
        for t in d.textures.iter_mut().flatten() {
            *t = shared.aliases[t];
        }
    }
    report.vertices = scene.vertices.len();
    report.triangles = scene.indices.len() / 3;
    report.draws = scene.draws.len();
    report.textures = shared.textures.len();
    report.resident_bytes = scene.vertices.len() * 32
        + scene.indices.len() * 4
        + shared
            .textures
            .values()
            .map(|t| t.bytes.len())
            .sum::<usize>();
    if report.resident_bytes > 192 * 1024 * 1024 {
        return Err("STAGE: resident buffers/BC data exceed 192MiB presentation budget".into());
    }
    let bounds = scene.bounds()?;
    eprintln!("physics-stage CPU COVERAGE: {report:?}; resident estimate excludes physics, CPU ground/grid, depth, drawable and driver allocations; no per-frame file I/O");
    Ok(Stage {
        render: LoadedBlock {
            index: 269,
            total_blocks: report.blocks,
            start,
            mesh: scene,
            bounds,
            camera: Camera {
                eye: start + Vec3::new(0.0, 4.0, 6.0),
                target: start,
                up: Vec3::Y,
            },
            textures: shared.textures,
        },
        ground: terrain,
        route_points,
        report,
    })
}

/// Conservative homogeneous AABB/frustum test for wgpu 0..w reverse-Z clip space.
/// Objects crossing the near plane stay visible; no division by w or corner-only test.
pub fn visible(bounds: Bounds, vp: Mat4, eye: Vec3) -> bool {
    if eye.distance_squared(eye.clamp(bounds.min, bounds.max)) > 450.0f32.powi(2) {
        return false;
    }
    let corners: [_; 8] = std::array::from_fn(|i| {
        vp * Vec3::new(
            if i & 1 == 0 {
                bounds.min.x
            } else {
                bounds.max.x
            },
            if i & 2 == 0 {
                bounds.min.y
            } else {
                bounds.max.y
            },
            if i & 4 == 0 {
                bounds.min.z
            } else {
                bounds.max.z
            },
        )
        .extend(1.0)
    });
    !(0..6).any(|plane| {
        corners.iter().all(|p| match plane {
            0 => p.x < -p.w,
            1 => p.x > p.w,
            2 => p.y < -p.w,
            3 => p.y > p.w,
            4 => p.z < 0.0,
            _ => p.z > p.w,
        })
    })
}

pub fn draw_bounds(mesh: &Mesh, draw: &Draw) -> Bounds {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &i in &mesh.indices[draw.indices.clone()] {
        let p = Vec3::from(mesh.vertices[i as usize].position);
        min = min.min(p);
        max = max.max(p);
    }
    Bounds { min, max }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frustum_keeps_intersections_and_rejects_behind_and_distant() {
        let c = Camera {
            eye: Vec3::ZERO,
            target: Vec3::Z,
            up: Vec3::Y,
        };
        let vp = c.view_projection(1.5);
        let b = |p: Vec3| Bounds {
            min: p - Vec3::splat(0.5),
            max: p + Vec3::splat(0.5),
        };
        assert!(visible(b(Vec3::Z * 10.0), vp, c.eye));
        assert!(visible(b(Vec3::ZERO), vp, c.eye));
        assert!(!visible(b(Vec3::NEG_Z * 10.0), vp, c.eye));
        assert!(!visible(b(Vec3::new(100.0, 0.0, 10.0)), vp, c.eye));
        assert!(!visible(b(Vec3::Z * 500.0), vp, c.eye));
    }
    #[test]
    fn static_object_conversion_preserves_placement_color_uv_and_indices() {
        let mut vertices = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            vertices.extend(p.into_iter().flat_map(f32::to_le_bytes));
            vertices.extend([1, 2, 3, 255]);
            vertices.extend([0.25f32, 0.75].into_iter().flat_map(f32::to_le_bytes));
        }
        let indices = [0, 0, 1, 0, 2, 0];
        let b = lbs::Buffers {
            vertices: &vertices,
            indices: &indices,
            vertex_count: 3,
            stride: 24,
        };
        let m = object_mesh(
            &b,
            12,
            16,
            Some(7),
            3,
            Mat4::from_translation(Vec3::splat(5.0)),
        )
        .unwrap();
        assert_eq!(m.indices, [0, 1, 2]);
        assert_eq!(m.vertices[0].color, [3, 2, 1, 255]);
        assert_eq!(m.vertices[1].position, [6.0, 5.0, 5.0]);
        assert_eq!(m.vertices[0].uv0, [0.25, 0.75]);
        assert!(m.draws[0].cutout);
        let b = lbs::Buffers {
            indices: &[0, 0, 1, 0, 3, 0],
            ..b
        };
        assert!(object_mesh(&b, 12, 16, None, 1, Mat4::IDENTITY).is_err());
    }
}
