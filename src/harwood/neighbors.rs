//! Connected static terrain/road selection. No scenery lanes or simulation.
use super::{
    mesh::{self, Bounds, Mesh},
    texture::upload_plan,
    Camera, LoadedBlock,
};
use crate::asset_probe;
use rerbr_asset_formats::lbs;
use rerbr_render_core::Vec3;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
};

const CONTACT: f32 = 0.02; // Connectivity tolerance only; never weld/change vertices.
const MIN_EDGE: f32 = 0.25;
const MIB: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub target_blocks: usize,
    pub max_draws: usize,
    pub max_mesh_bytes: usize,
    pub max_texture_bytes: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            target_blocks: 20,
            max_draws: 400,
            max_mesh_bytes: 16 * MIB,
            max_texture_bytes: 32 * MIB,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub from: usize,
    pub to: usize,
    pub shared_edges: usize,
    pub max_gap_m: f32,
}

#[derive(Debug)]
pub struct Report {
    pub ids: Vec<usize>,
    pub connections: Vec<Connection>,
    pub component_blocks: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub draws: usize,
    pub texture_references: usize,
    pub texture_bindings: usize,
    pub unique_textures: usize,
    pub mesh_bytes: usize,
    pub texture_bytes: usize,
    pub source_bytes: usize,
    pub all_converted_bytes: usize,
    pub route_samples: usize,
    pub covered_samples: usize,
    pub unmatched_samples: usize,
    pub route_runs_m: Vec<(f32, f32)>,
    pub warnings: Vec<String>,
}
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f,"harwood-neighbors SELECT: IDs={:?} (zero-based); component={}; contacts use <= {CONTACT}m tolerance",self.ids,self.component_blocks)?;
        for c in &self.connections {
            writeln!(f,"harwood-neighbors CONNECT: {} -> {}, shared boundary edges={}, max endpoint gap={}m",c.from,c.to,c.shared_edges,c.max_gap_m)?;
        }
        writeln!(
            f,
            "harwood-neighbors GEOMETRY: blocks={}, vertices={}, triangles={}, draws={}",
            self.ids.len(),
            self.vertices,
            self.triangles,
            self.draws
        )?;
        writeln!(f,"harwood-neighbors TEXTURES: material bindings={}, logical IDs={}, unique resolved DDS={}, reused bindings={}",self.texture_bindings,self.texture_references,self.unique_textures,self.texture_bindings-self.unique_textures)?;
        writeln!(f,"harwood-neighbors MEMORY: VB+IB={} bytes, stored BC mips={} bytes, GPU payload={} bytes; source LBS={}, all converted meshes={}",self.mesh_bytes,self.texture_bytes,self.mesh_bytes+self.texture_bytes,self.source_bytes,self.all_converted_bytes)?;
        writeln!(f,"harwood-neighbors MEMORY: excludes driver/bind-group overhead, surface buffers and depth (logged at viewport resize); CPU also holds graph/parse metadata")?;
        writeln!(f,"harwood-neighbors ROAD: covered samples={}/{}, unmatched by road materials={}, covered runs relative to nearest start sample={:?}m; internal gaps={}",self.covered_samples,self.route_samples,self.unmatched_samples,self.route_runs_m,self.route_runs_m.len().saturating_sub(1))?;
        for warning in &self.warnings {
            writeln!(f, "harwood-neighbors WARNING: {warning}")?;
        }
        Ok(())
    }
}

pub struct LoadedSet {
    pub render: LoadedBlock,
    pub report: Report,
    #[cfg(feature = "harwood-benchmark")]
    pub route_points: Vec<Vec3>,
    #[cfg(feature = "harwood-benchmark")]
    pub road_triangles: Vec<[Vec3; 3]>,
}

struct Node {
    id: usize,
    mesh: Mesh,
    bounds: Bounds,
    roads: Vec<[Vec3; 3]>,
}
impl Node {
    fn mesh_bytes(&self) -> usize {
        self.mesh.vertices.len() * 32 + self.mesh.indices.len() * 4
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct Link {
    edges: usize,
    gap: f32,
}
type Graph = Vec<BTreeMap<usize, Link>>;
type Point = [i32; 3];
type EdgeKey = (Point, Point);
#[derive(Clone, Copy)]
struct Segment {
    a: Vec3,
    b: Vec3,
}

fn key(p: Vec3) -> Point {
    p.to_array().map(|v| (v / CONTACT).round() as i32)
}
fn boundary(mesh: &Mesh) -> BTreeMap<EdgeKey, Segment> {
    let mut edges: BTreeMap<EdgeKey, (usize, Segment)> = BTreeMap::new();
    for tri in mesh.indices.chunks_exact(3) {
        let p: Vec<_> = tri
            .iter()
            .map(|&i| Vec3::from(mesh.vertices[i as usize].position))
            .collect();
        for (a, b) in [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])] {
            if a.distance(b) < MIN_EDGE {
                continue;
            }
            let (qa, qb) = (key(a), key(b));
            if qa == qb {
                continue;
            }
            let (edge, segment) = if qa < qb {
                ((qa, qb), Segment { a, b })
            } else {
                ((qb, qa), Segment { a: b, b: a })
            };
            edges
                .entry(edge)
                .and_modify(|e| e.0 += 1)
                .or_insert((1, segment));
        }
    }
    edges
        .into_iter()
        .filter_map(|(edge, (count, segment))| (count == 1).then_some((edge, segment)))
        .collect()
}

fn graph(nodes: &[Node]) -> Graph {
    let mut owners: BTreeMap<EdgeKey, Vec<(usize, Segment)>> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        for (edge, segment) in boundary(&node.mesh) {
            owners.entry(edge).or_default().push((i, segment));
        }
    }
    let mut graph = vec![BTreeMap::new(); nodes.len()];
    for owners in owners.values() {
        for (i, &(a, sa)) in owners.iter().enumerate() {
            for &(b, sb) in &owners[i + 1..] {
                let gap = sa.a.distance(sb.a).max(sa.b.distance(sb.b));
                if gap > CONTACT {
                    continue;
                }
                for (from, to) in [(a, b), (b, a)] {
                    let link = graph[from].entry(to).or_insert(Link::default());
                    link.edges += 1;
                    link.gap = link.gap.max(gap);
                }
            }
        }
    }
    graph
}

fn component(graph: &Graph, seed: usize) -> BTreeSet<usize> {
    let mut found = BTreeSet::from([seed]);
    let mut pending = vec![seed];
    while let Some(node) = pending.pop() {
        for &next in graph[node].keys() {
            if found.insert(next) {
                pending.push(next);
            }
        }
    }
    found
}

fn near_road(p: Vec3, tri: [Vec3; 3]) -> bool {
    let [a, b, c] = tri;
    let cross = |u: Vec3, v: Vec3| u.x * v.z - u.z * v.x;
    let den = cross(b - a, c - a);
    if den.abs() < 1e-6 {
        return false;
    }
    let u = cross(p - a, c - a) / den;
    let v = cross(b - a, p - a) / den;
    if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
        return (p.y - (a.y + (b.y - a.y) * u + (c.y - a.y) * v)).abs() <= 4.0;
    }
    [(a, b), (b, c), (c, a)].into_iter().any(|(a, b)| {
        let d = b - a;
        let flat = Vec3::new(d.x, 0.0, d.z);
        let t = ((p - a).dot(flat) / flat.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let q = a + d * t;
        let delta = p - q;
        delta.x * delta.x + delta.z * delta.z <= 9.0 && delta.y.abs() <= 4.0
    })
}

struct Sample {
    point: Vec3,
    distance: f32,
    owners: BTreeSet<usize>,
}
fn samples(nodes: &[Node], route: &[Vec3], start: Vec3) -> Vec<Sample> {
    let Some(nearest) = route
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.distance_squared(start)
                .total_cmp(&b.distance_squared(start))
        })
        .map(|(i, _)| i)
    else {
        return Vec::new();
    };
    let mut lengths = vec![0.0; route.len()];
    for i in 1..route.len() {
        lengths[i] = lengths[i - 1] + route[i].distance(route[i - 1]);
    }
    route
        .iter()
        .enumerate()
        .filter_map(|(i, &point)| {
            let distance = lengths[i] - lengths[nearest];
            if !(-100.0..=600.0).contains(&distance) {
                return None;
            }
            let owners = nodes
                .iter()
                .enumerate()
                .filter_map(|(id, node)| {
                    node.roads
                        .iter()
                        .any(|&tri| near_road(point, tri))
                        .then_some(id)
                })
                .collect();
            Some(Sample {
                point,
                distance,
                owners,
            })
        })
        .collect()
}

struct Selection {
    nodes: Vec<usize>,
    parents: Vec<(usize, usize)>,
    reasons: BTreeSet<&'static str>,
}
fn choose(
    nodes: &[Node],
    graph: &Graph,
    route: &[Sample],
    seed: usize,
    budget: Budget,
) -> Result<Selection, String> {
    if !(1..=25).contains(&budget.target_blocks) {
        return Err("target block count must be 1..25".into());
    }
    let mut selected = vec![seed];
    let mut parents = Vec::new();
    let mut reasons = BTreeSet::new();
    let mut mesh_bytes = nodes[seed].mesh_bytes();
    let mut draws = nodes[seed].mesh.draws.len();
    if mesh_bytes > budget.max_mesh_bytes || draws > budget.max_draws {
        return Err("seed block exceeds mesh/draw budget".into());
    }
    while selected.len() < budget.target_blocks {
        let set: BTreeSet<_> = selected.iter().copied().collect();
        let next_sample = route
            .iter()
            .find(|s| !s.owners.is_empty() && s.owners.is_disjoint(&set));
        let target = next_sample.map_or(
            (nodes[seed].bounds.min + nodes[seed].bounds.max) * 0.5,
            |s| s.point,
        );
        let mut frontier = BTreeMap::new();
        for &parent in &selected {
            for &next in graph[parent].keys() {
                if !set.contains(&next) {
                    frontier.entry(next).or_insert(parent);
                }
            }
        }
        let mut candidates = Vec::new();
        for (&next, &parent) in &frontier {
            if draws + nodes[next].mesh.draws.len() > budget.max_draws {
                reasons.insert("draw-call budget");
                continue;
            }
            if mesh_bytes + nodes[next].mesh_bytes() > budget.max_mesh_bytes {
                reasons.insert("mesh-buffer budget");
                continue;
            }
            let b = nodes[next].bounds;
            let direct = next_sample.is_some_and(|s| s.owners.contains(&next));
            let d = target.distance_squared(target.clamp(b.min, b.max));
            let center = (b.min + b.max) * 0.5;
            candidates.push((next, parent, !direct, d, center.distance_squared(target)));
        }
        candidates.sort_by(|a, b| {
            a.2.cmp(&b.2)
                .then(a.3.total_cmp(&b.3))
                .then(a.4.total_cmp(&b.4))
                .then(nodes[a.0].id.cmp(&nodes[b.0].id))
        });
        let Some(&(next, parent, ..)) = candidates.first() else {
            reasons.insert("no remaining connected block fits the budgets");
            break;
        };
        selected.push(next);
        parents.push((parent, next));
        mesh_bytes += nodes[next].mesh_bytes();
        draws += nodes[next].mesh.draws.len();
    }
    Ok(Selection {
        nodes: selected,
        parents,
        reasons,
    })
}

fn needed(nodes: &[Node], selected: &[usize]) -> BTreeSet<u32> {
    selected
        .iter()
        .flat_map(|&id| nodes[id].mesh.draws.iter())
        .flat_map(|d| d.textures.into_iter().flatten())
        .collect()
}

fn trim_textures(
    selection: &mut Selection,
    nodes: &[Node],
    aliases: &BTreeMap<u32, u32>,
    sizes: &BTreeMap<u32, usize>,
    limit: usize,
) -> Result<BTreeSet<u32>, String> {
    loop {
        let retained: BTreeSet<_> = needed(nodes, &selection.nodes)
            .iter()
            .map(|id| aliases[id])
            .collect();
        if retained.iter().map(|id| sizes[id]).sum::<usize>() <= limit {
            return Ok(retained);
        }
        if selection.nodes.len() == 1 {
            return Err("seed textures exceed compressed-texture budget".into());
        }
        selection.nodes.pop();
        selection.parents.pop();
        selection
            .reasons
            .insert("compressed-texture budget: removed last growth leaf");
    }
}
fn append_mesh(out: &mut Mesh, mut input: Mesh) {
    let vertex_base = out.vertices.len() as u32;
    let index_base = out.indices.len();
    out.vertices.append(&mut input.vertices);
    out.indices
        .extend(input.indices.into_iter().map(|i| i + vertex_base));
    for mut draw in input.draws {
        draw.indices.start += index_base;
        draw.indices.end += index_base;
        out.draws.push(draw);
    }
    out.far_draws += input.far_draws;
    out.specular_draws += input.specular_draws;
}

pub fn load(maps: &Path) -> Result<LoadedSet, String> {
    load_with_budget(maps, Budget::default())
}

pub fn load_with_budget(maps: &Path, budget: Budget) -> Result<LoadedSet, String> {
    eprintln!(
        "harwood-neighbors READ: static connected near/always terrain only; budget={budget:?}"
    );
    let names = asset_probe::diffuse_names(maps)?;
    let road_ids: BTreeSet<_> = names
        .iter()
        .filter_map(|(&id, name)| name.contains("road").then_some(id))
        .collect();
    let bytes = asset_probe::read_stage(maps)?;
    let source_bytes = bytes.len();
    let stage = lbs::parse(&bytes).map_err(|e| e.to_string())?;
    let geometry = stage
        .chunks
        .iter()
        .find_map(|c| {
            if let lbs::Chunk::Geometry(g) = c {
                Some(g)
            } else {
                None
            }
        })
        .ok_or("missing geometry")?;
    let pose = stage
        .chunks
        .iter()
        .find_map(|c| {
            if let lbs::Chunk::StartLocation(p) = c {
                Some(p)
            } else {
                None
            }
        })
        .ok_or("missing start pose")?;
    let route: Vec<_> = stage
        .chunks
        .iter()
        .filter_map(|c| {
            if let lbs::Chunk::DrivelinePoints(d) = c {
                Some(d)
            } else {
                None
            }
        })
        .flat_map(|d| d.points.iter().copied().map(Vec3::from))
        .filter(|p| p.is_finite())
        .collect();
    let total_blocks = geometry.blocks.len();
    eprintln!("harwood-neighbors READ PASS: {source_bytes} LBS bytes, {total_blocks} geometry blocks, {} route points", route.len());
    let start = Vec3::from(pose.position);
    let [pitch, yaw, _] = pose.rotation;
    let heading = Vec3::new(
        yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    );
    if !start.is_finite() || !heading.is_finite() {
        return Err("invalid start pose".into());
    }
    let seed_id = super::select_block(&geometry.blocks, start)?;
    let mut nodes = Vec::new();
    let mut warnings = Vec::new();
    for (id, block) in geometry.blocks.iter().enumerate() {
        let mesh = mesh::block_mesh(block);
        if mesh.bad_triangles != 0 || mesh.skipped_draws != 0 {
            warnings.push(format!("excluded block {id}: bad/unsupported geometry"));
            continue;
        }
        let Ok(bounds) = mesh.bounds() else {
            warnings.push(format!("excluded block {id}: empty/invalid geometry"));
            continue;
        };
        let mut roads = Vec::new();
        for draw in &mesh.draws {
            if draw
                .textures
                .iter()
                .flatten()
                .any(|id| road_ids.contains(id))
            {
                for tri in mesh.indices[draw.indices.clone()].chunks_exact(3) {
                    roads.push(
                        [tri[0], tri[1], tri[2]]
                            .map(|i| Vec3::from(mesh.vertices[i as usize].position)),
                    );
                }
            }
        }
        nodes.push(Node {
            id,
            mesh,
            bounds,
            roads,
        });
    }
    drop(stage);
    drop(bytes);
    let all_converted_bytes = nodes.iter().map(Node::mesh_bytes).sum();
    eprintln!("harwood-neighbors CONVERT PASS: {} drawable blocks; {all_converted_bytes} total VB+IB bytes before selection; excluded={}", nodes.len(), warnings.len());
    let seed = nodes
        .iter()
        .position(|n| n.id == seed_id)
        .ok_or("seed conversion failed")?;
    let graph = graph(&nodes);
    let component_blocks = component(&graph, seed).len();
    let route = samples(&nodes, &route, start);
    let mut selection = choose(&nodes, &graph, &route, seed, budget)?;
    let mut shared = asset_probe::load_diffuse_shared(maps, &needed(&nodes, &selection.nodes))?;
    let image_bytes: BTreeMap<_, _> = shared
        .textures
        .iter()
        .map(|(&id, t)| {
            let plan = upload_plan(&t.bytes, u32::MAX)?;
            Ok((id, plan.mips.iter().map(|m| m.len).sum::<usize>()))
        })
        .collect::<Result<_, String>>()?;
    let canonical = trim_textures(
        &mut selection,
        &nodes,
        &shared.aliases,
        &image_bytes,
        budget.max_texture_bytes,
    )?;
    shared.textures.retain(|id, _| canonical.contains(id));
    let texture_bytes = canonical.iter().map(|id| image_bytes[id]).sum();
    let references = needed(&nodes, &selection.nodes).len();
    let selected_set: BTreeSet<_> = selection.nodes.iter().copied().collect();
    let covered: Vec<_> = route
        .iter()
        .map(|s| !s.owners.is_disjoint(&selected_set))
        .collect();
    let mut runs = Vec::new();
    let mut run_start = None;
    for (i, &yes) in covered.iter().enumerate() {
        if yes && run_start.is_none() {
            run_start = Some(i);
        }
        if !yes {
            if let Some(begin) = run_start.take() {
                runs.push((route[begin].distance, route[i - 1].distance));
            }
        }
    }
    if let Some(begin) = run_start {
        runs.push((route[begin].distance, route[route.len() - 1].distance));
    }
    let ids = selection.nodes.iter().map(|&i| nodes[i].id).collect();
    let connections = selection
        .parents
        .iter()
        .map(|&(from, to)| Connection {
            from: nodes[from].id,
            to: nodes[to].id,
            shared_edges: graph[from][&to].edges,
            max_gap_m: graph[from][&to].gap,
        })
        .collect();
    if selection.nodes.len() < budget.target_blocks {
        warnings.extend(selection.reasons.iter().map(|s| s.to_string()));
    }
    if selection.nodes.len() < 15 {
        warnings.push(
            "fewer than 15 blocks: connected frontier/budget limit; do not bridge unsupported gaps"
                .into(),
        );
    }
    if runs.len() > 1 {
        warnings.push("road coverage has internal sample gaps; inspect the reported ranges".into());
    }
    if route.is_empty() {
        warnings.push(
            "no usable LBS route samples; selection fell back to connected spatial proximity"
                .into(),
        );
    }
    if route.iter().any(|s| s.owners.is_empty()) {
        warnings.push("some sampled route points have no matching road-material triangle; continuity there is unproven".into());
    }
    let mut merged = Mesh::default();
    #[cfg(feature = "harwood-benchmark")]
    let road_triangles = nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| selected_set.contains(i))
        .flat_map(|(_, node)| node.roads.iter().copied())
        .collect();
    for (i, node) in nodes.iter_mut().enumerate() {
        if selected_set.contains(&i) {
            append_mesh(&mut merged, std::mem::take(&mut node.mesh));
        }
    }
    for draw in &mut merged.draws {
        for id in draw.textures.iter_mut().flatten() {
            *id = shared.aliases[id];
        }
    }
    let bounds = merged.bounds()?;
    let camera = Camera::framing(bounds, heading);
    eprintln!(
        "harwood-neighbors BOUNDS: min={:?}, max={:?}, span={:?} metres",
        bounds.min,
        bounds.max,
        bounds.max - bounds.min
    );
    let mesh_bytes = merged.vertices.len() * 32 + merged.indices.len() * 4;
    let texture_bindings = merged
        .draws
        .iter()
        .flat_map(|d| d.textures.into_iter().flatten())
        .count();
    let report = Report {
        ids,
        connections,
        component_blocks,
        vertices: merged.vertices.len(),
        triangles: merged.indices.len() / 3,
        draws: merged.draws.len(),
        texture_references: references,
        texture_bindings,
        unique_textures: shared.textures.len(),
        mesh_bytes,
        texture_bytes,
        source_bytes,
        all_converted_bytes,
        route_samples: route.len(),
        covered_samples: covered.iter().filter(|&&s| s).count(),
        unmatched_samples: route.iter().filter(|s| s.owners.is_empty()).count(),
        route_runs_m: runs,
        warnings,
    };
    eprintln!("{report}");
    eprintln!("harwood-neighbors CAMERA: fixed panorama eye={:?}, target={:?}; original world positions unchanged",camera.eye,camera.target);
    Ok(LoadedSet {
        render: LoadedBlock {
            index: seed_id,
            total_blocks,
            start,
            mesh: merged,
            bounds,
            camera,
            textures: shared.textures,
        },
        report,
        #[cfg(feature = "harwood-benchmark")]
        route_points: route.iter().map(|s| s.point).collect(),
        #[cfg(feature = "harwood-benchmark")]
        road_triangles,
    })
}

#[cfg(test)]
#[path = "neighbors_tests.rs"]
mod tests;
