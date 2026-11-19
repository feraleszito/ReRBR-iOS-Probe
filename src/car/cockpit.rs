//! Original interior mesh set and the original steering node's local Z axis.
//! Presentation reads the physical rack angle; no animation writes back to physics.
#[cfg(feature = "physics-stage")]
use rerbr_render_core::coords;
use rerbr_render_core::Mat4;

/// Same node-conjugated local-Z turn as upstream characters/incar.rs::wheel_turn.
/// Angle is the physical rack travel divided by the original rack ratio, in radians.
pub fn steering_transform(node: Mat4, angle: f32) -> Mat4 {
    node * Mat4::from_rotation_z(angle) * node.inverse()
}

#[cfg(feature = "physics-stage")]
pub fn install(
    mut car: super::Car,
    root: &std::path::Path,
    elements: &[rerbr_asset_formats::sgc::Element],
    parts: &[rerbr_asset_formats::ini::Section],
    shaders: &[rerbr_asset_formats::ini::Section],
    settings: &[rerbr_asset_formats::ini::Section],
) -> super::Car {
    use crate::{
        asset_probe::{TextureData, TextureInfo},
        harwood::texture::upload_plan,
    };
    use rerbr_asset_formats::sgc;
    use std::collections::{BTreeMap, BTreeSet};
    let prepared = (|| {
        let geometry = super::mesh::convert_interior_for_mode(
            elements,
            parts,
            shaders,
            settings,
            car.vehicle.is_ngp_experimental_car(),
        )?;
        let worlds = super::mesh::checked_worlds(elements)?;
        let node = elements
            .iter()
            .find_map(|e| {
                let node = match e {
                    sgc::Element::Mesh(m) => &m.node,
                    sgc::Element::Node(n) => n,
                    _ => return None,
                };
                (node.name == "i_steeringwheel").then(|| coords::affine(worlds[&node.id]))
            })
            .ok_or("COCKPIT: missing original i_steeringwheel node")?;
        if !node.is_finite()
            || node.determinant().abs() < 1e-8
            || !geometry.draws.iter().any(|d| d.slot == 5)
        {
            return Err("COCKPIT: invalid original steering-wheel geometry/transform".to_string());
        }
        let mut textures = BTreeMap::new();
        for path in geometry
            .materials
            .iter()
            .map(|m| &m.path)
            .collect::<BTreeSet<_>>()
        {
            if car.textures.contains_key(path) {
                continue;
            }
            let bytes = super::read(root, &car.vehicle, path, 64 * 1024 * 1024)?;
            let plan = upload_plan(&bytes, u32::MAX)?;
            let first = plan.mips.first().ok_or("COCKPIT DDS: missing mips")?;
            let info = TextureInfo {
                request: path.clone(),
                resolved: path.clone(),
                width: first.width,
                height: first.height,
                mip_count: plan.mips.len() as u32,
                format: plan.format,
            };
            textures.insert(path.clone(), TextureData { info, bytes });
        }
        Ok::<_, String>((geometry, node, textures))
    })();
    match prepared {
        Ok((geometry, node, textures)) => {
            eprintln!(
                "COCKPIT: original interior meshes={:?}; additional textures={:?}",
                geometry.drawn,
                textures.keys().collect::<Vec<_>>()
            );
            car.files.extend(textures.keys().cloned());
            car.textures.extend(textures);
            car.geometry.append(geometry);
            car.cockpit = Some(node);
        }
        Err(error) => {
            eprintln!("COCKPIT UNAVAILABLE: {error}; exterior driving preserved");
            car.cockpit_error = Some(error);
        }
    }
    car
}

#[cfg(test)]
mod tests {
    use super::*;
    use rerbr_render_core::Vec3;
    #[test]
    fn steering_keeps_original_pivot_and_turns_in_both_directions() {
        let node = Mat4::from_translation(Vec3::new(0.3, 0.8, -0.4)) * Mat4::from_rotation_x(0.4);
        let pivot = node.transform_point3(Vec3::ZERO);
        let rim = node.transform_point3(Vec3::X * 0.2);
        let a = steering_transform(node, 0.7);
        let b = steering_transform(node, -0.7);
        assert!(a.transform_point3(pivot).abs_diff_eq(pivot, 1e-6));
        assert!(a.transform_point3(rim).distance(b.transform_point3(rim)) > 0.2);
        assert!((a * b).abs_diff_eq(Mat4::IDENTITY, 1e-5));
        assert!(steering_transform(node, 0.0).abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }
}
