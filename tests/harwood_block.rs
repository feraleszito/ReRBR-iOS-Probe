#![cfg(feature = "harwood-block")]

use rerbr_asset_formats::{dds, lbs};
use rerbr_probe::harwood::{self, mesh, texture::upload_plan, Camera};
use rerbr_render_core::Vec3;

fn bounds(center: [f32; 3], half: [f32; 3]) -> lbs::Bounds {
    lbs::Bounds {
        center,
        half_extent: half,
    }
}

fn draw(slot: usize) -> lbs::RenderChunk {
    lbs::RenderChunk {
        view: slot,
        vertex_shader: 12,
        pixel_shader: 5,
        start_index: 0,
        prim_count: 1,
        vertex_count: 3,
        min_index: 7,
        bounds: bounds([0.0; 3], [1.0; 3]),
        shadow: false,
        shadow_texture: 0,
        specular: false,
        specular_texture: 0,
        textures: vec![2, 3],
        vertex_constants_flag: false,
        vertex_constants: [0.0; 4],
        vertex_constants_word: 0,
        list: 1,
    }
}

fn block(draw: lbs::RenderChunk) -> lbs::GeometryBlock<'static> {
    lbs::GeometryBlock {
        views: [None; 9],
        chunks: vec![draw],
        bounds: bounds([0.0; 3], [1.0; 3]),
    }
}

#[test]
fn conversion_preserves_position_color_uv_absolute_indices_and_view_bases() {
    for slot in [5, 6, 7, 8] {
        let stride = lbs::VIEW_STRIDES[slot];
        let mut source = vec![0u8; stride * 3];
        for vertex in 0..3 {
            let b = &mut source[vertex * stride..(vertex + 1) * stride];
            for (i, value) in [vertex as f32, 2.0, 3.0].into_iter().enumerate() {
                b[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            let color = if matches!(slot, 6 | 8) { 24 } else { 12 };
            b[color..color + 4].copy_from_slice(&[40, 30, 20, 128]);
            let uv = if matches!(slot, 6 | 8) { 28 } else { 16 };
            for (i, value) in [0.25f32, 0.5, 1.25, 1.5].into_iter().enumerate() {
                b[uv + i * 4..uv + i * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let prefix = vec![0u8; 32];
        let prefix_indices = [0u8; 6];
        let indices: Vec<_> = [2u16, 0, 1]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut b = block(draw(slot));
        b.views[0] = Some(lbs::Buffers {
            indices: &prefix_indices,
            vertices: &prefix,
            vertex_count: 2,
            stride: 16,
        });
        b.views[slot] = Some(lbs::Buffers {
            indices: &indices,
            vertices: &source,
            vertex_count: 3,
            stride,
        });
        let mesh = mesh::block_mesh(&b);
        assert_eq!(mesh.indices, [4, 2, 3]); // No winding flip or addition of min_index=7.
        assert_eq!(mesh.vertices[2].position, [0.0, 2.0, 3.0]);
        assert_eq!(mesh.vertices[2].color, [20, 30, 40, 128]);
        assert_eq!(mesh.vertices[2].uv0, [0.25, 0.5]);
        assert_eq!(mesh.vertices[2].uv1, [1.25, 1.5]);
        assert_eq!(mesh.draws[0].textures, [Some(2), Some(3)]);
        assert_eq!(mesh.vertex_bytes().len(), 5 * 32);
        assert_eq!(mesh.bad_triangles, 0);
        assert!(mesh.bounds().is_ok());
    }
}

#[test]
fn conversion_rejects_bad_indices_and_omits_far_lod() {
    let source = vec![0u8; 96];
    let bad: Vec<_> = [0u16, 1, 3]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut b = block(draw(5));
    b.views[5] = Some(lbs::Buffers {
        indices: &bad,
        vertices: &source,
        vertex_count: 3,
        stride: 32,
    });
    let result = mesh::block_mesh(&b);
    assert_eq!(result.bad_triangles, 1);
    assert!(result.indices.is_empty());
    b.chunks[0].list = 3;
    let result = mesh::block_mesh(&b);
    assert_eq!(result.far_draws, 1);
    assert_eq!(result.bad_triangles, 0);
    assert!(result.draws.is_empty());
}

#[test]
fn selection_uses_start_bounds_and_excludes_empty_or_far_only_blocks() {
    let mut large = block(draw(5));
    large.bounds = bounds([25.0, 0.0, 0.0], [30.0, 3.0, 3.0]);
    let mut close_center = block(draw(5));
    close_center.bounds = bounds([1.0, 0.0, 0.0], [0.1; 3]);
    let mut far_only = block(draw(5));
    far_only.chunks[0].list = 3;
    assert_eq!(
        harwood::select_block(&[large, close_center, far_only], Vec3::ZERO).unwrap(),
        0
    );
    assert!(harwood::select_block(&[], Vec3::ZERO).is_err());
}

fn assert_framed(camera: Camera, b: mesh::Bounds) {
    for aspect in [0.65, 0.75, 4.0 / 3.0, 1.55, 2.0] {
        let vp = camera.view_projection(aspect);
        for x in [b.min.x, b.max.x] {
            for y in [b.min.y, b.max.y] {
                for z in [b.min.z, b.max.z] {
                    let p = vp * Vec3::new(x, y, z).extend(1.0);
                    assert!(p.w > 0.0);
                    let ndc = p.truncate() / p.w;
                    assert!(
                        ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0 && ndc.z > 0.0 && ndc.z < 1.0,
                        "{aspect}: {ndc:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fixed_camera_frames_a_stage_sized_block_in_portrait_and_landscape() {
    let b = mesh::Bounds {
        min: Vec3::new(-420.0, 70.0, 1400.0),
        max: Vec3::new(-300.0, 90.0, 1550.0),
    };
    let camera = Camera::framing(b, Vec3::Z);
    assert_framed(camera, b);
    // A flat test block must remain large enough to inspect, not just fit as a dot.
    let matrix = camera.view_projection(1.55);
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for x in [b.min.x, b.max.x] {
        for y in [b.min.y, b.max.y] {
            for z in [b.min.z, b.max.z] {
                let p = matrix * Vec3::new(x, y, z).extend(1.0);
                low = low.min(p.truncate() / p.w);
                high = high.max(p.truncate() / p.w);
            }
        }
    }
    assert!((high - low).truncate().max_element() > 1.0);
    let vp = camera.view_projection(1.0);
    let direction = (camera.target - camera.eye).normalize();
    let depth = |distance: f32| {
        let p = vp * (camera.eye + direction * distance).extend(1.0);
        p.z / p.w
    };
    assert!(depth(1.0) > depth(100.0));
}

fn synthetic_dds(dxt3: bool) -> Vec<u8> {
    let block = if dxt3 { 16 } else { 8 };
    let mut file = vec![0u8; 128 + 7 * block]; // 8x8,4x4,2x2,1x1 => 4+1+1+1 blocks.
    file[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x21007),
        (12, 8),
        (16, 8),
        (28, 4),
        (76, 32),
        (80, 4),
        (108, 0x401008),
    ] {
        file[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    file[84..88].copy_from_slice(if dxt3 { b"DXT3" } else { b"DXT1" });
    file
}

#[test]
fn bc_upload_layout_covers_every_mip_including_sub_block_tails() {
    for dxt3 in [false, true] {
        let bytes = synthetic_dds(dxt3);
        let plan = upload_plan(&bytes, 2048).unwrap();
        assert_eq!(plan.mips.len(), 4);
        assert_eq!(
            plan.mips.iter().map(|m| m.copy_width).collect::<Vec<_>>(),
            [8, 4, 4, 4]
        );
        assert_eq!(
            plan.mips.iter().map(|m| m.block_rows).collect::<Vec<_>>(),
            [2, 1, 1, 1]
        );
        assert_eq!(plan.mips.last().unwrap().width, 1);
        assert_eq!(
            plan.mips.last().unwrap().offset + plan.mips.last().unwrap().len,
            bytes.len()
        );
        assert_eq!(plan.mips[0].bytes_per_row, if dxt3 { 32 } else { 16 });
        for m in &plan.mips {
            assert_eq!(m.len, m.bytes_per_row as usize * m.block_rows as usize);
        }
        assert!(upload_plan(&bytes, 4).is_err());
        assert!(upload_plan(&bytes[..bytes.len() - 1], 2048).is_err());
    }
}

#[test]
#[ignore = "requires private Maps; set HARWOOD_MAPS and run explicitly"]
fn real_start_block_and_materials() {
    let maps = std::env::var_os("HARWOOD_MAPS").expect("set HARWOOD_MAPS");
    let block = harwood::load(std::path::Path::new(&maps)).unwrap();
    assert_eq!(block.total_blocks, 282);
    assert!(!block.mesh.indices.is_empty());
    assert_eq!(block.mesh.bad_triangles, 0);
    assert_eq!(block.mesh.skipped_draws, 0);
    assert_framed(block.camera, block.bounds);
    let info: Vec<_> = block.texture_info().collect();
    assert!(info.iter().any(|(_, t)| t.request.contains("roader")));
    assert!(info.iter().any(|(_, t)| t.request.contains("ground")));
    assert!(info.iter().any(|(_, t)| t.format == dds::Format::Dxt1));
    assert!(info.iter().any(|(_, t)| t.format == dds::Format::Dxt3));
    for &i in &block.mesh.indices {
        assert!((i as usize) < block.mesh.vertices.len());
    }
    let vp = block.camera.view_projection(1.55);
    let mut clockwise = 0;
    let mut counterclockwise = 0;
    for triangle in block.mesh.indices.chunks_exact(3) {
        let p: Vec<_> = triangle
            .iter()
            .map(|&i| {
                let clip = vp * Vec3::from(block.mesh.vertices[i as usize].position).extend(1.0);
                clip.truncate() / clip.w
            })
            .collect();
        let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[1].y - p[0].y) * (p[2].x - p[0].x);
        if area < -1e-10 {
            clockwise += 1;
        }
        if area > 1e-10 {
            counterclockwise += 1;
        }
    }
    println!("projected winding: CW={clockwise}, CCW={counterclockwise}");
    assert!(
        clockwise > 0,
        "the fixed camera must see clockwise front faces"
    );
}
