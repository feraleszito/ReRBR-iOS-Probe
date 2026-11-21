//! ReRBR car/meshes.rs conversion adapted to a shared compact buffer, no damage.
use super::{
    materials::{self, Material},
    selection,
};
use rerbr_asset_formats::{car_parts, ini::Section, sgc};
use rerbr_render_core::{coords, Mat4, Vec3};
use std::{
    collections::{BTreeSet, HashMap},
    ops::Range,
};

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}
#[derive(Debug, Clone)]
pub struct Draw {
    pub indices: Range<usize>,
    pub material: usize,
    pub slot: u32,
    pub order: (u8, usize),
    pub interior: bool,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub vertices: usize,
    pub triangles: usize,
    pub draws: usize,
    pub textures: usize,
}
#[derive(Debug)]
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<Draw>,
    pub materials: Vec<Material>,
    pub pivots: [Vec3; 4],
    pub wheel_visual_offsets: [Vec3; 4],
    pub door_dorsals: Option<[(Vec3, Vec3); 2]>,
    pub wheel_points: [Vec<Vec3>; 4],
    pub body_points: Vec<Vec3>,
    pub drawn: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub unresolved_faces: Vec<String>,
}

pub fn checked_worlds(elements: &[sgc::Element]) -> Result<HashMap<i32, [[f32; 4]; 3]>, String> {
    let mut nodes = HashMap::new();
    for e in elements {
        let n = match e {
            sgc::Element::Mesh(m) => &m.node,
            sgc::Element::Node(n) => n,
            _ => continue,
        };
        if nodes.insert(n.id, n).is_some() {
            return Err(format!("CAR HIERARCHY: duplicate node {}", n.id));
        }
    }
    for &id in nodes.keys() {
        let mut seen = BTreeSet::new();
        let mut next = id;
        while let Some(n) = nodes.get(&next) {
            if !seen.insert(next) {
                return Err("CAR HIERARCHY: cycle".into());
            }
            if n.parent < 0 {
                break;
            }
            next = n.parent;
        }
    }
    Ok(sgc::chained_worlds(elements))
}
pub fn convert(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
) -> Result<Geometry, String> {
    convert_set(elements, parts, shaders, settings, false, false)
}
pub fn convert_for_mode(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
    allow_material_override: bool,
) -> Result<Geometry, String> {
    convert_set(
        elements,
        parts,
        shaders,
        settings,
        false,
        allow_material_override,
    )
}
#[cfg(any(not(feature = "physics-stage"), test))]
pub fn convert_interior(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
) -> Result<Geometry, String> {
    convert_set(elements, parts, shaders, settings, true, false)
}
#[cfg(feature = "physics-stage")]
pub fn convert_interior_for_mode(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
    allow_material_override: bool,
) -> Result<Geometry, String> {
    convert_set(
        elements,
        parts,
        shaders,
        settings,
        true,
        allow_material_override,
    )
}
fn convert_set(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
    interior: bool,
    allow_material_override: bool,
) -> Result<Geometry, String> {
    let worlds = checked_worlds(elements)?;
    let mut result = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
        draws: Vec::new(),
        materials: Vec::new(),
        pivots: [Vec3::ZERO; 4],
        wheel_visual_offsets: [Vec3::ZERO; 4],
        door_dorsals: None,
        wheel_points: std::array::from_fn(|_| Vec::new()),
        body_points: Vec::new(),
        drawn: Vec::new(),
        skipped: Vec::new(),
        unresolved_faces: Vec::new(),
    };
    let mut found = [false; 4];
    for (k, side) in selection::SIDES.iter().enumerate() {
        let name = format!("e_wheel_gravel_{side}");
        if let Some(n) = elements.iter().find_map(|e| match e {
            sgc::Element::Mesh(m) if m.node.name == name => Some(&m.node),
            _ => None,
        }) {
            let w = worlds[&n.id];
            result.pivots[k] = Vec3::new(w[0][3], w[1][3], w[2][3]);
            found[k] = true;
        }
    }
    if found.iter().any(|v| !*v) {
        return Err("CAR WHEELS: four original gravel nodes required".into());
    }
    for i in 0..4 {
        for j in i + 1..4 {
            if !result.pivots[i].is_finite() || result.pivots[i].distance(result.pivots[j]) < 0.2 {
                return Err(
                    "CAR WHEELS: invalid/stacked nodes; only original Corolla supported".into(),
                );
            }
        }
    }
    let mid_z = result.pivots.iter().map(|p| p.z).sum::<f32>() * 0.25;
    let corners_ordered = result.pivots[0].x > 0.1
        && result.pivots[0].z < mid_z
        && result.pivots[1].x < -0.1
        && result.pivots[1].z < mid_z
        && result.pivots[2].x > 0.1
        && result.pivots[2].z > mid_z
        && result.pivots[3].x < -0.1
        && result.pivots[3].z > mid_z;
    if !corners_ordered {
        let mut reordered = [Vec3::ZERO; 4];
        let mut assigned = [false; 4];
        for p in result.pivots {
            let corner = match (p.x >= 0.0, p.z < mid_z) {
                (true, true) => 0,
                (false, true) => 1,
                (true, false) => 2,
                (false, false) => 3,
            };
            if !assigned[corner] {
                reordered[corner] = p;
                assigned[corner] = true;
            }
        }
        if assigned.iter().all(|&a| a) {
            result.pivots = reordered;
        }
    }
    if !interior {
        for (k, side) in selection::SIDES.iter().enumerate() {
            let hang_name = format!("e_wheelhang_{side}");
            if let Some(n) = elements.iter().find_map(|e| match e {
                sgc::Element::Mesh(m) if m.node.name == hang_name => Some(&m.node),
                _ => None,
            }) {
                let w = worlds[&n.id];
                let hang_pos = Vec3::new(w[0][3], w[1][3], w[2][3]);
                // Some community SGC models (e.g. Audi 200 quattro GrpA) place e_wheel_gravel_lf/rf
                // behind the actual hub/suspension arch (e_wheelhang_lf/rf) while physics uses NGP wheel_centre.
                // Record the visual Z alignment offset so showroom & body-offset match the wheel arches.
                if k < 2
                    && hang_pos.z < result.pivots[k].z - 0.10
                    && (hang_pos.z - result.pivots[k].z).abs() < 0.35
                {
                    result.wheel_visual_offsets[k] =
                        Vec3::new(0.0, 0.0, hang_pos.z - result.pivots[k].z);
                }
            }
        }
    }
    let mut door_l_acc = (Vec3::ZERO, 0usize);
    let mut door_r_acc = (Vec3::ZERO, 0usize);
    let mesh_names: BTreeSet<_> = elements
        .iter()
        .filter_map(|element| match element {
            sgc::Element::Mesh(mesh) => Some(mesh.node.name.as_str()),
            _ => None,
        })
        .collect();
    for element in elements {
        let sgc::Element::Mesh(mesh) = element else {
            continue;
        };
        let selected = if interior {
            selection::interior(&mesh.node.name, parts)
        } else {
            selection::exterior_with_lamps(&mesh.node.name, parts, |name| mesh_names.contains(name))
        };
        let part = match selected {
            Ok(part) => part,
            Err(why) => {
                result.skipped.push((mesh.node.name.clone(), why.into()));
                continue;
            }
        };
        let matrix = part_transform(mesh, elements, &worlds);
        if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 {
            return Err(format!("CAR TRANSFORM: invalid {}", mesh.node.name));
        }
        let normals = matrix.inverse().transpose();
        let blocks = car_parts::face_blocks(part);
        if !allow_material_override && blocks.len() != mesh.faces.len() {
            return Err(format!(
                "CAR FACES: {} has {} mesh faces but {} material bindings",
                mesh.node.name,
                mesh.faces.len(),
                blocks.len()
            ));
        }
        let mut remap = HashMap::new();
        let slot = if interior {
            if mesh.node.name == "i_steeringwheel" {
                5
            } else {
                0
            }
        } else {
            let named_slot = selection::slot(&mesh.node.name);
            if !corners_ordered && named_slot > 0 {
                let w = worlds[&mesh.node.id];
                let pos = Vec3::new(w[0][3], w[1][3], w[2][3]);
                match (pos.x >= 0.0, pos.z < mid_z) {
                    (true, true) => 1,
                    (false, true) => 2,
                    (true, false) => 3,
                    (false, false) => 4,
                }
            } else {
                named_slot
            }
        };
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            let indexed = blocks.get(face_idx);
            let resolved = if allow_material_override {
                let matches =
                    |block: &&car_parts::FaceBlock| block.material.eq_ignore_ascii_case(&face.name);
                let block = indexed.filter(matches).or_else(|| {
                    let mut named = blocks.iter().filter(matches);
                    let first = named.next()?;
                    named
                        .all(|other| other.shader.eq_ignore_ascii_case(&first.shader))
                        .then_some(first)
                });
                if let Some(block) = block {
                    materials::resolve(shaders, settings, &block.shader, &block.material)
                } else if let Some(block) = indexed.filter(|_| blocks.len() == mesh.faces.len()) {
                    // A complete table can bind by index. Some exporters leave the
                    // original editor material name in the SGC after editing the INI.
                    materials::resolve(shaders, settings, &block.shader, &block.material).or_else(
                        |_| materials::resolve(shaders, settings, &block.shader, &face.name),
                    )
                } else {
                    Err("no corresponding FaceBlock".into())
                }
            } else {
                let block = &blocks[face_idx];
                if face.name != block.material {
                    return Err(format!(
                        "CAR MATERIAL: SGC/INI mismatch on {} (sgc='{}' ini='{}')",
                        mesh.node.name, face.name, block.material
                    ));
                }
                materials::resolve(shaders, settings, &block.shader, &block.material)
            };
            let resolved = match resolved {
                Ok(material) => material,
                Err(err) if allow_material_override => {
                    result.unresolved_faces.push(format!(
                        "{} FaceBlock{face_idx} '{}': {err}",
                        mesh.node.name, face.name
                    ));
                    Material::untextured()
                }
                Err(err) => return Err(err),
            };
            let material = result
                .materials
                .iter()
                .position(|m| {
                    m.shader == resolved.shader
                        && m.name == resolved.name
                        && m.pass == resolved.pass
                })
                .unwrap_or_else(|| {
                    let index = result.materials.len();
                    result.materials.push(resolved);
                    index
                });
            let textured = result.materials[material].path.is_some();
            let start = face.start_index as usize;
            let triangles = if start.is_multiple_of(3) {
                mesh.triangles
                    .get(start / 3..start / 3 + face.prim_count as usize)
            } else {
                None
            }
            .ok_or("CAR INDEX: invalid face range")?;
            let first = result.indices.len();
            for triangle in triangles {
                for &index in triangle {
                    let i = usize::from(index);
                    let v = mesh
                        .vertices
                        .get(i)
                        .ok_or("CAR INDEX: outside SGC vertices")?;
                    let uv = match mesh.uvs.get(i) {
                        Some(uv) => *uv,
                        // A solid surface has no UV dependency.
                        None if !textured => [0.0; 2],
                        None => return Err("CAR UV: missing original coordinates".into()),
                    };
                    let id = if let Some(&id) = remap.get(&i) {
                        id
                    } else {
                        let p = matrix.transform_point3(Vec3::from(v.position));
                        let normal = normals
                            .transform_vector3(Vec3::from(v.normal))
                            .normalize_or_zero();
                        if !p.is_finite()
                            || p.abs().max_element() > 50.0
                            || !normal.is_finite()
                            || !uv.into_iter().all(f32::is_finite)
                        {
                            return Err("CAR VERTEX: invalid original data/transform".into());
                        }
                        let id = result.vertices.len() as u32;
                        result.vertices.push(Vertex {
                            position: p.to_array(),
                            normal: normal.to_array(),
                            uv,
                        });
                        remap.insert(i, id);
                        id
                    };
                    result.indices.push(id);
                }
            }
            if result.indices.len() > first {
                result.draws.push(Draw {
                    indices: first..result.indices.len(),
                    material,
                    slot,
                    order: (result.materials[material].list, material),
                    interior,
                });
            }
        }
        if !remap.is_empty() {
            result.drawn.push(mesh.node.name.clone());
            if !interior && (mesh.node.name == "e_door_l" || mesh.node.name == "e_door_r") {
                for &idx in remap.values() {
                    let p = Vec3::from(result.vertices[idx as usize].position);
                    if mesh.node.name == "e_door_l" {
                        door_l_acc.0 += p;
                        door_l_acc.1 += 1;
                    } else {
                        door_r_acc.0 += p;
                        door_r_acc.1 += 1;
                    }
                }
            }
        }
    }
    result.draws.sort_by_key(|d| d.order); // Original wheel/list/shader rank; retain later opaque draws.
    if interior {
        if !result.drawn.iter().any(|n| n == "i_body" || n == "i_bodyi") {
            return Err("CAR: i_body missing".into());
        }
        return Ok(result);
    } else if !result.drawn.iter().any(|n| n == "e_body") {
        return Err("CAR: e_body missing".into());
    }
    if door_l_acc.1 > 0 && door_r_acc.1 > 0 {
        let lc = door_l_acc.0 / door_l_acc.1 as f32;
        let rc = door_r_acc.0 / door_r_acc.1 as f32;
        if lc.x.abs() > 0.35 && rc.x.abs() > 0.35 {
            result.door_dorsals = Some([
                (lc, Vec3::new(lc.x.signum(), 0.0, 0.0)),
                (rc, Vec3::new(rc.x.signum(), 0.0, 0.0)),
            ]);
        }
    }
    let mut referenced: [BTreeSet<u32>; 5] = std::array::from_fn(|_| BTreeSet::new());
    for d in &result.draws {
        referenced[d.slot as usize].extend(result.indices[d.indices.clone()].iter().copied());
    }
    for (k, indices) in referenced.iter().enumerate() {
        let p: Vec<_> = indices
            .iter()
            .map(|&i| Vec3::from(result.vertices[i as usize].position))
            .collect();
        if k == 0 {
            result.body_points = p;
        } else {
            if p.is_empty() {
                return Err("CAR: gravel wheel has no converted geometry".into());
            }
            result.wheel_points[k - 1] = p;
        }
    }
    Ok(result)
}

pub(crate) fn part_transform(
    mesh: &sgc::Mesh,
    elements: &[sgc::Element],
    worlds: &HashMap<i32, [[f32; 4]; 3]>,
) -> Mat4 {
    let name = mesh.node.name.as_str();
    let side = match name {
        "e_door_l" | "i_door_l" => Some(("l", 1.0)),
        "e_door_r" | "i_door_r" => Some(("r", -1.0)),
        _ => None,
    };
    if let Some((side, sign)) = side {
        let door = format!("e_door_{side}");
        if let Some(id) = elements.iter().find_map(|e| match e {
            sgc::Element::Mesh(m) if m.node.name == door => Some(m.node.id),
            _ => None,
        }) {
            let w = worlds[&id];
            return coords::hinged_panel_to_car(
                worlds[&mesh.node.id],
                mesh.node.transform.words(),
                [w[0][3], w[1][3], w[2][3]],
                sign,
            );
        }
    }
    coords::part_to_car(worlds[&mesh.node.id], mesh.node.transform.words())
}
impl Geometry {
    /// Append another mesh set without changing the exterior's wheel/body fit data.
    pub fn append(&mut self, mut other: Self) {
        let vertex = self.vertices.len() as u32;
        let index = self.indices.len();
        let material = self.materials.len();
        self.vertices.append(&mut other.vertices);
        self.indices
            .extend(other.indices.into_iter().map(|i| i + vertex));
        self.materials.append(&mut other.materials);
        self.draws.extend(other.draws.into_iter().map(|mut d| {
            d.indices = d.indices.start + index..d.indices.end + index;
            d.material += material;
            d.order.1 += material;
            d
        }));
        self.draws.sort_by_key(|d| d.order);
        self.drawn.append(&mut other.drawn);
        self.unresolved_faces.append(&mut other.unresolved_faces);
    }
    pub fn stats(&self, blended: Option<bool>) -> Stats {
        let mut vertices = BTreeSet::new();
        let mut textures = BTreeSet::new();
        let mut triangles = 0;
        let mut draws = 0;
        for d in &self.draws {
            let m = &self.materials[d.material];
            if blended.is_some_and(|b| b != m.pass.blended()) {
                continue;
            }
            vertices.extend(self.indices[d.indices.clone()].iter().copied());
            textures.extend(m.path.as_ref());
            triangles += d.indices.len() / 3;
            draws += 1;
        }
        Stats {
            vertices: vertices.len(),
            triangles,
            draws,
            textures: textures.len(),
        }
    }
    pub fn vertex_bytes(&self) -> Vec<u8> {
        self.vertices
            .iter()
            .flat_map(|v| {
                v.position
                    .into_iter()
                    .chain(v.normal)
                    .chain(v.uv)
                    .flat_map(f32::to_ne_bytes)
            })
            .collect()
    }
}
