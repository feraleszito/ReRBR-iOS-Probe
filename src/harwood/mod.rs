//! Real terrain diagnostics: single block or optional connected neighbors.

#[cfg(feature = "harwood-benchmark")]
pub mod benchmark;
mod gpu;
pub mod mesh;
#[cfg(feature = "harwood-neighbors")]
pub mod neighbors;
#[cfg(feature = "physics-stage")]
pub mod stage;
pub mod texture;

#[cfg(feature = "physics-stage")]
pub const MODE_NAME: &str = "physics-stage";
#[cfg(all(feature = "physics-drive", not(feature = "physics-stage")))]
pub const MODE_NAME: &str = "physics-drive";
#[cfg(all(feature = "car-chase-benchmark", not(feature = "physics-drive")))]
pub const MODE_NAME: &str = "car-chase-benchmark";
#[cfg(all(feature = "harwood-benchmark", not(feature = "car-chase-benchmark")))]
pub const MODE_NAME: &str = "harwood-benchmark";
#[cfg(all(feature = "harwood-neighbors", not(feature = "harwood-benchmark")))]
pub const MODE_NAME: &str = "harwood-neighbors";
#[cfg(not(feature = "harwood-neighbors"))]
pub const MODE_NAME: &str = "harwood-block";
#[cfg(feature = "car-chase-benchmark")]
pub(crate) use gpu::upload;
pub(crate) use gpu::Scene;

use crate::asset_probe::{self, TextureData};
use mesh::{Bounds, Mesh};
use rerbr_asset_formats::lbs;
use rerbr_render_core::{coords, Mat4, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct LoadedBlock {
    pub index: usize,
    pub total_blocks: usize,
    pub start: Vec3,
    pub mesh: Mesh,
    pub bounds: Bounds,
    pub camera: Camera,
    pub(crate) textures: BTreeMap<u32, TextureData>,
}

impl LoadedBlock {
    pub fn texture_info(&self) -> impl Iterator<Item = (u32, &asset_probe::TextureInfo)> {
        self.textures
            .iter()
            .map(|(&id, texture)| (id, &texture.info))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
}

impl Camera {
    pub fn framing(bounds: Bounds, heading: Vec3) -> Self {
        let target = (bounds.min + bounds.max) * 0.5;
        let forward = Vec3::new(heading.x, 0.0, heading.z).normalize_or(Vec3::Z);
        let side = Vec3::Y.cross(forward);
        let offset = (-forward + side * 0.65 + Vec3::Y * 1.15).normalize();
        // Fit actual box corners, rather than a tall sphere around mostly flat
        // terrain. A fixed eye sized for aspect 2.0 also fits portrait/narrower views.
        let look = -offset;
        let right = Vec3::Y.cross(look).normalize();
        let up = look.cross(right);
        let tan_x = (coords::FOV_DEGREES.to_radians() * 0.5).tan();
        let tan_y = (coords::vertical_fov(coords::FOV_DEGREES.to_radians(), 2.0) * 0.5).tan();
        let mut distance = 5.0f32;
        for x in [bounds.min.x, bounds.max.x] {
            for y in [bounds.min.y, bounds.max.y] {
                for z in [bounds.min.z, bounds.max.z] {
                    let p = Vec3::new(x, y, z) - target;
                    let depth = p.dot(look);
                    distance = distance
                        .max(p.dot(right).abs() / tan_x - depth)
                        .max(p.dot(up).abs() / tan_y - depth)
                        .max(coords::NEAR - depth);
                }
            }
        }
        let distance = distance * 1.1;
        Self {
            eye: target + offset * distance,
            target,
            up: Vec3::Y,
        }
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.view_projection_with_fov(aspect, coords::FOV_DEGREES)
    }
    pub fn view_projection_with_fov(&self, aspect: f32, fov_degrees: f32) -> Mat4 {
        let forward = (self.target - self.eye).normalize_or(Vec3::Z);
        let up = self.up.normalize_or(Vec3::Y);
        let fov = if fov_degrees.is_finite() {
            fov_degrees.clamp(30.0, 110.0)
        } else {
            coords::FOV_DEGREES
        };
        coords::projection(fov.to_radians(), aspect) * look_to_lh(self.eye, forward, up)
    }
}

/// Left-handed world-to-view matrix with an arbitrary camera up vector.
pub fn look_to_lh(eye: Vec3, forward: Vec3, up: Vec3) -> Mat4 {
    let f = forward.normalize_or(Vec3::Z);
    let fallback_up = if f.y.abs() < 0.9 { Vec3::Y } else { Vec3::Z };
    let s = up
        .cross(f)
        .try_normalize()
        .unwrap_or_else(|| fallback_up.cross(f).normalize());
    let u = f.cross(s);
    Mat4::from_cols_array(&[
        s.x,
        u.x,
        f.x,
        0.0,
        s.y,
        u.y,
        f.y,
        0.0,
        s.z,
        u.z,
        f.z,
        0.0,
        -s.dot(eye),
        -u.dot(eye),
        -f.dot(eye),
        1.0,
    ])
}

/// Closest nonempty near-LOD block by distance to its AABB, then its centre.
/// This uses the LBS start position, not an assumed world origin or block number.
pub fn select_block(blocks: &[lbs::GeometryBlock<'_>], start: Vec3) -> Result<usize, String> {
    blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            let c = Vec3::from(b.bounds.center);
            let h = Vec3::from(b.bounds.half_extent);
            c.is_finite()
                && h.is_finite()
                && h.min_element() >= 0.0
                && b.chunks
                    .iter()
                    .any(|d| matches!(d.list, 1 | 2) && d.prim_count > 0)
        })
        .min_by(|(_, a), (_, b)| {
            let score = |b: &lbs::GeometryBlock<'_>| {
                let c = Vec3::from(b.bounds.center);
                let h = Vec3::from(b.bounds.half_extent);
                (
                    (start - start.clamp(c - h, c + h)).length_squared(),
                    (start - c).length_squared(),
                )
            };
            let (ad, ac) = score(a);
            let (bd, bc) = score(b);
            ad.total_cmp(&bd).then(ac.total_cmp(&bc))
        })
        .map(|(i, _)| i)
        .ok_or_else(|| "no drawable block near start".into())
}

pub fn load(maps: &Path) -> Result<LoadedBlock, String> {
    eprintln!("harwood-block READ: track-20_O.lbs");
    let (index, total_blocks, start, forward, mesh) = {
        let bytes = asset_probe::read_stage(maps)?;
        let stage = lbs::parse(&bytes).map_err(|e| format!("LBS parse: {e}"))?;
        let geometry = stage
            .chunks
            .iter()
            .find_map(|c| match c {
                lbs::Chunk::Geometry(g) => Some(g),
                _ => None,
            })
            .ok_or("missing geometry")?;
        let pose = stage
            .chunks
            .iter()
            .find_map(|c| match c {
                lbs::Chunk::StartLocation(p) => Some(p),
                _ => None,
            })
            .ok_or("missing start location")?;
        let start = Vec3::from(pose.position);
        let [pitch, yaw, _] = pose.rotation;
        let forward = Vec3::new(
            yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        )
        .normalize_or_zero();
        if !start.is_finite() || !forward.is_finite() || forward == Vec3::ZERO {
            return Err("invalid start pose".into());
        }
        let index = select_block(&geometry.blocks, start)?;
        eprintln!("harwood-block READ PASS: {} bytes; {} blocks; start={start:?}; selected={index} (zero-based)", bytes.len(), geometry.blocks.len());
        (
            index,
            geometry.blocks.len(),
            start,
            forward,
            mesh::block_mesh(&geometry.blocks[index]),
        )
    };
    if mesh.bad_triangles != 0 || mesh.skipped_draws != 0 {
        return Err(format!(
            "CONVERT: bad triangles={}, unsupported draws={}",
            mesh.bad_triangles, mesh.skipped_draws
        ));
    }
    let bounds = mesh.bounds()?;
    let camera = Camera::framing(bounds, forward);
    eprintln!("harwood-block CONVERT PASS: vertices={}, triangles={}, draws={}, far draws omitted={}, specular draws (diffuse only)={}; bounds={bounds:?}", mesh.vertices.len(), mesh.indices.len()/3, mesh.draws.len(), mesh.far_draws, mesh.specular_draws);
    eprintln!(
        "harwood-block CAMERA: fixed eye={:?}, target={:?}; LH/Y-up, CW, reverse-Z",
        camera.eye, camera.target
    );
    let needed: BTreeSet<_> = mesh
        .draws
        .iter()
        .flat_map(|d| d.textures.into_iter().flatten())
        .collect();
    let textures = asset_probe::load_diffuse(maps, &needed)?;
    for (id, data) in &textures {
        // Validate source layout now; upload repeats with the actual device limit.
        let _plan = texture::upload_plan(&data.bytes, u32::MAX)?;
        eprintln!(
            "harwood-block DDS READ PASS: Texture{id}: {} -> {} | {:?} {}x{} mips={}",
            data.info.request,
            data.info.resolved,
            data.info.format,
            data.info.width,
            data.info.height,
            data.info.mip_count
        );
    }
    Ok(LoadedBlock {
        index,
        total_blocks,
        start,
        mesh,
        bounds,
        camera,
        textures,
    })
}
