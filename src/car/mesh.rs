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
    pub wheel_points: [Vec<Vec3>; 4],
    pub body_points: Vec<Vec3>,
    pub drawn: Vec<String>,
    pub skipped: Vec<(String, String)>,
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
    convert_set(elements, parts, shaders, settings, false)
}
pub fn convert_interior(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
) -> Result<Geometry, String> {
    convert_set(elements, parts, shaders, settings, true)
}
fn convert_set(
    elements: &[sgc::Element],
    parts: &[Section],
    shaders: &[Section],
    settings: &[Section],
    interior: bool,
) -> Result<Geometry, String> {
    let worlds = checked_worlds(elements)?;
    let mut result = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
        draws: Vec::new(),
        materials: Vec::new(),
        pivots: [Vec3::ZERO; 4],
        wheel_points: std::array::from_fn(|_| Vec::new()),
        body_points: Vec::new(),
        drawn: Vec::new(),
        skipped: Vec::new(),
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
    for element in elements {
        let sgc::Element::Mesh(mesh) = element else {
            continue;
        };
        let selected = if interior {
            selection::interior(&mesh.node.name, parts)
        } else {
            selection::exterior(&mesh.node.name, parts)
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
        if blocks.len() != mesh.faces.len() {
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
            selection::slot(&mesh.node.name)
        };
        for (face, block) in mesh.faces.iter().zip(&blocks) {
            if face.name != block.material {
                return Err(format!(
                    "CAR MATERIAL: SGC/INI mismatch on {}",
                    mesh.node.name
                ));
            }
            let material = match result
                .materials
                .iter()
                .position(|m| m.shader == block.shader && m.name == block.material)
            {
                Some(i) => i,
                None => {
                    let i = result.materials.len();
                    result.materials.push(materials::resolve(
                        shaders,
                        settings,
                        &block.shader,
                        &block.material,
                    )?);
                    i
                }
            };
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
                    let id = if let Some(&id) = remap.get(&i) {
                        id
                    } else {
                        let p = matrix.transform_point3(Vec3::from(v.position));
                        let normal = normals
                            .transform_vector3(Vec3::from(v.normal))
                            .normalize_or_zero();
                        let uv = mesh
                            .uvs
                            .get(i)
                            .copied()
                            .ok_or("CAR UV: missing original coordinates")?;
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
        }
    }
    result.draws.sort_by_key(|d| d.order); // Original wheel/list/shader rank; retain later opaque draws.
    let body = if interior { "i_body" } else { "e_body" };
    if !result.drawn.iter().any(|n| n == body) {
        return Err(format!("CAR: {body} missing"));
    }
    if interior {
        return Ok(result);
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

fn part_transform(
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
            textures.insert(&m.path);
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
