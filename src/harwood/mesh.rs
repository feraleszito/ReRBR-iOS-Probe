//! Adapted GPL-3.0-or-later reRBR stage/mesh.rs conversion. See UPSTREAM.md.
//! Kept: slot layouts, absolute view indices, vertex bases, BGRA conversion,
//! UVs and winding. This first pass retains near/always draws and diffuse only.

use rerbr_asset_formats::lbs;
use rerbr_render_core::{coords::rgba_from_d3dcolor, Vec3};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [u8; 4],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
}

#[derive(Debug)]
pub struct Draw {
    pub indices: Range<usize>,
    pub textures: [Option<u32>; 2],
    pub list: u8,
    /// Original object alpha-test/two-sided state (terrain remains false).
    pub cutout: bool,
    /// Original object texture addressing: 1 wrap, 2 mirror, 3 clamp.
    pub address: u8,
}

#[derive(Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<Draw>,
    pub bad_triangles: usize,
    pub skipped_draws: usize,
    pub far_draws: usize,
    pub shadow_draws: usize,
    pub specular_draws: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Mesh {
    pub fn bounds(&self) -> Result<Bounds, String> {
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for &index in &self.indices {
            let v = self
                .vertices
                .get(index as usize)
                .ok_or("index outside converted mesh")?;
            let p = Vec3::from(v.position);
            if !p.is_finite()
                || p.abs().max_element() > 50_000.0
                || !v.uv0.into_iter().chain(v.uv1).all(f32::is_finite)
            {
                return Err("non-finite/out-of-stage vertex or UV".into());
            }
            min = min.min(p);
            max = max.max(p);
        }
        if self.indices.is_empty() {
            return Err("no near/always triangles".into());
        }
        Ok(Bounds { min, max })
    }

    pub fn vertex_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.vertices.len() * 32);
        for v in &self.vertices {
            bytes.extend(v.position.into_iter().flat_map(f32::to_ne_bytes));
            bytes.extend(v.color);
            bytes.extend(v.uv0.into_iter().chain(v.uv1).flat_map(f32::to_ne_bytes));
        }
        bytes
    }
}

#[derive(Clone, Copy)]
struct SlotLayout {
    color: usize,
    texcoord0: Option<usize>,
    texcoord1: Option<usize>,
}

fn slot_layout(slot: usize) -> Option<SlotLayout> {
    let layout = |color, texcoord0, texcoord1| SlotLayout {
        color,
        texcoord0,
        texcoord1,
    };
    match slot {
        0 => Some(layout(12, None, None)),
        5 | 7 => Some(layout(12, Some(16), Some(24))),
        6 | 8 => Some(layout(24, Some(28), Some(36))),
        _ => None,
    }
}

pub fn block_mesh(block: &lbs::GeometryBlock<'_>) -> Mesh {
    let mut mesh = Mesh::default();
    let mut bases: [Option<usize>; 9] = [None; 9];
    for (slot, view) in block.views.iter().enumerate() {
        let (Some(view), Some(layout)) = (view, slot_layout(slot)) else {
            continue;
        };
        bases[slot] = Some(mesh.vertices.len());
        mesh.vertices
            .extend((0..view.vertex_count).map(|v| repack(view, v, layout)));
    }
    let has_near_or_always = block.chunks.iter().any(|c| matches!(c.list, 1 | 2));
    for chunk in &block.chunks {
        // Never overlay the far LOD on top of near/always geometry in the same block.
        // If a block is authored exclusively as far-LOD backdrop (list == 3 only, e.g.
        // Diamond Creek I/II blocks 0..5, Tanner block 39, Kaihuavaara blocks 0..1),
        // load its genuine original list-3 triangles instead of discarding the block.
        if chunk.list == 3 {
            mesh.far_draws += 1;
            if has_near_or_always {
                continue;
            }
        } else if !matches!(chunk.list, 1 | 2) {
            mesh.skipped_draws += 1;
            continue;
        }
        let (Some(view), Some(base)) = (
            block.views.get(chunk.view).copied().flatten(),
            bases.get(chunk.view).copied().flatten(),
        ) else {
            mesh.skipped_draws += 1;
            continue;
        };
        let first = mesh.indices.len();
        // As upstream: absolute indices into each view; min_index is not added.
        let in_range = |i: usize| i < view.vertex_count;
        for triangle in 0..chunk.prim_count as usize {
            let at = chunk.start_index as usize + 3 * triangle;
            match [at, at + 1, at + 2].map(|i| view.index(i).map(usize::from)) {
                [Some(a), Some(b), Some(c)] if [a, b, c].into_iter().all(in_range) => {
                    mesh.indices.extend([a, b, c].map(|i| (base + i) as u32));
                }
                _ => mesh.bad_triangles += 1,
            }
        }
        if mesh.indices.len() > first {
            if chunk.shadow {
                mesh.shadow_draws += 1;
            }
            if chunk.specular {
                mesh.specular_draws += 1;
            }
            mesh.draws.push(Draw {
                indices: first..mesh.indices.len(),
                textures: [0, 1].map(|i| chunk.textures.get(i).copied()),
                list: chunk.list,
                cutout: false,
                address: 1,
            });
        }
    }
    mesh
}

fn repack(buffers: &lbs::Buffers<'_>, v: usize, fields: SlotLayout) -> Vertex {
    let position = buffers.position(v).unwrap_or([0.0; 3]);
    let color = vertex_bytes(buffers, v, fields.color, 4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map_or([0; 4], rgba_from_d3dcolor);
    Vertex {
        position,
        color,
        uv0: texcoord(buffers, v, fields.texcoord0),
        uv1: texcoord(buffers, v, fields.texcoord1),
    }
}

fn texcoord(buffers: &lbs::Buffers<'_>, v: usize, at: Option<usize>) -> [f32; 2] {
    at.and_then(|at| vertex_bytes(buffers, v, at, 8))
        .map_or([0.0; 2], |b| [f32_at(b, 0), f32_at(b, 4)])
}

fn vertex_bytes<'a>(
    buffers: &lbs::Buffers<'a>,
    v: usize,
    offset: usize,
    len: usize,
) -> Option<&'a [u8]> {
    let start = v.checked_mul(buffers.stride)?.checked_add(offset)?;
    buffers.vertices.get(start..start.checked_add(len)?)
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_render_chunk(view: usize, list: u8, shadow: bool) -> lbs::RenderChunk {
        lbs::RenderChunk {
            view,
            vertex_shader: 0,
            pixel_shader: 0,
            start_index: 0,
            prim_count: 1,
            vertex_count: 3,
            min_index: 0,
            bounds: lbs::Bounds {
                center: [0.0; 3],
                half_extent: [1.0; 3],
            },
            shadow,
            shadow_texture: 0,
            specular: false,
            specular_texture: 0,
            textures: vec![0, 1],
            vertex_constants_flag: false,
            vertex_constants: [0.0; 4],
            vertex_constants_word: 0,
            list,
        }
    }

    #[test]
    fn block_mesh_counts_view_7_shadow_chunk_as_shadow_draw_not_skipped() {
        let stride = lbs::VIEW_STRIDES[7];
        let mut vertices = vec![0u8; stride * 3];
        for (i, pos) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]
            .iter()
            .enumerate()
        {
            let base = i * stride;
            for (axis, v) in pos.iter().enumerate() {
                vertices[base + axis * 4..base + axis * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        let indices: Vec<u8> = [0u16, 1, 2]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut views: [Option<lbs::Buffers<'_>>; 9] = [None; 9];
        views[7] = Some(lbs::Buffers {
            indices: &indices,
            vertices: &vertices,
            vertex_count: 3,
            stride,
        });
        let block = lbs::GeometryBlock {
            views,
            chunks: vec![sample_render_chunk(7, 1, true)],
            bounds: lbs::Bounds {
                center: [0.5, 0.0, 0.5],
                half_extent: [0.5, 0.0, 0.5],
            },
        };
        let mesh = block_mesh(&block);
        assert_eq!(mesh.skipped_draws, 0);
        assert_eq!(mesh.shadow_draws, 1);
        assert_eq!(mesh.draws.len(), 1);
        assert_eq!(mesh.indices.len(), 3);
    }

    #[test]
    fn block_mesh_loads_far_only_list_3_block_and_skips_list_3_when_near_present() {
        let stride = lbs::VIEW_STRIDES[0];
        let vertices = vec![0u8; stride * 3];
        let indices: Vec<u8> = [0u16, 1, 2]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut views: [Option<lbs::Buffers<'_>>; 9] = [None; 9];
        views[0] = Some(lbs::Buffers {
            indices: &indices,
            vertices: &vertices,
            vertex_count: 3,
            stride,
        });
        let far_only = lbs::GeometryBlock {
            views,
            chunks: vec![sample_render_chunk(0, 3, false)],
            bounds: lbs::Bounds {
                center: [0.0; 3],
                half_extent: [1.0; 3],
            },
        };
        let mesh_far = block_mesh(&far_only);
        assert_eq!(mesh_far.indices.len(), 3);
        assert_eq!(mesh_far.draws.len(), 1);
        assert_eq!(mesh_far.draws[0].list, 3);
        assert_eq!(mesh_far.far_draws, 1);
        assert_eq!(mesh_far.skipped_draws, 0);

        let mixed = lbs::GeometryBlock {
            views,
            chunks: vec![
                sample_render_chunk(0, 1, false),
                sample_render_chunk(0, 3, false),
            ],
            bounds: lbs::Bounds {
                center: [0.0; 3],
                half_extent: [1.0; 3],
            },
        };
        let mesh_mixed = block_mesh(&mixed);
        assert_eq!(mesh_mixed.indices.len(), 3);
        assert_eq!(mesh_mixed.draws.len(), 1);
        assert_eq!(mesh_mixed.draws[0].list, 1);
        assert_eq!(mesh_mixed.far_draws, 1);
        assert_eq!(mesh_mixed.skipped_draws, 0);
    }
}
