//! Distance-parameterized interpolation of private LBS points and geometric ground queries.
use super::super::{mesh::Mesh, Camera};
use rerbr_render_core::Vec3;
use std::collections::BTreeMap;

pub const EYE_HEIGHT: f32 = 1.4;
const CELL: f32 = 8.0;
const STEP: f32 = 0.25;

pub struct Ground {
    triangles: Vec<[Vec3; 3]>,
    cells: BTreeMap<(i32, i32), Vec<usize>>,
    cell_size: f32,
    #[cfg(feature = "car-chase-benchmark")]
    planes: Vec<Option<SupportPlane>>,
}
#[cfg(feature = "car-chase-benchmark")]
struct SupportPlane {
    origin: Vec3,
    b: Vec3,
    c: Vec3,
    inverse: f32,
}
#[cfg(feature = "car-chase-benchmark")]
impl SupportPlane {
    fn new([a, b, c]: [Vec3; 3]) -> Option<Self> {
        let b = b - a;
        let c = c - a;
        let det = b.x * c.z - b.z * c.x;
        (det.abs() >= 1e-7).then(|| Self {
            origin: a,
            b,
            c,
            inverse: det.recip(),
        })
    }
    fn height(&self, p: Vec3) -> Option<f32> {
        let d = p - self.origin;
        let u = (d.x * self.c.z - d.z * self.c.x) * self.inverse;
        let v = (self.b.x * d.z - self.b.z * d.x) * self.inverse;
        (u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001)
            .then_some(self.origin.y + self.b.y * u + self.c.y * v)
    }
}
impl Ground {
    #[cfg(feature = "car-chase-benchmark")]
    pub fn support_height(&self, p: Vec3) -> Option<f32> {
        self.cells
            .get(&cell(p, self.cell_size))?
            .iter()
            .filter_map(|&i| self.planes[i].as_ref()?.height(p))
            .max_by(f32::total_cmp)
    }
    #[cfg(feature = "car-chase-benchmark")]
    pub fn segment_clear(&self, from: Vec3, to: Vec3) -> bool {
        let direction = to - from;
        // Conservative XZ segment bounds: use the existing triangle grid instead
        // of tracing every terrain triangle for each near-plane corner.
        let lo = cell(from.min(to), self.cell_size);
        let hi = cell(from.max(to), self.cell_size);
        let mut candidates = std::collections::BTreeSet::new();
        for x in lo.0..=hi.0 {
            for z in lo.1..=hi.1 {
                if let Some(ids) = self.cells.get(&(x, z)) {
                    candidates.extend(ids.iter().copied());
                }
            }
        }
        !candidates.into_iter().any(|i| {
            let [a, b, c] = self.triangles[i];
            let e1 = b - a;
            let e2 = c - a;
            let p = direction.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-7 {
                return false;
            }
            let inv = det.recip();
            let s = from - a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return false;
            }
            let q = s.cross(e1);
            let v = direction.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                return false;
            }
            let t = e2.dot(q) * inv;
            (0.001..0.999).contains(&t)
        })
    }
    pub fn new(triangles: Vec<[Vec3; 3]>) -> Result<Self, String> {
        Self::build(triangles, CELL)
    }
    #[cfg(feature = "car-chase-benchmark")]
    pub fn for_vehicle(triangles: Vec<[Vec3; 3]>) -> Result<Self, String> {
        Self::build(triangles, 2.0)
    }
    fn build(triangles: Vec<[Vec3; 3]>, cell_size: f32) -> Result<Self, String> {
        let mut cells: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (id, tri) in triangles.iter().enumerate() {
            if !tri.iter().all(|p| p.is_finite()) {
                return Err("GROUND: non-finite triangle".into());
            }
            let lo = tri.iter().copied().reduce(Vec3::min).unwrap();
            let hi = tri.iter().copied().reduce(Vec3::max).unwrap();
            let (a, b) = (cell(lo, cell_size), cell(hi, cell_size));
            if (b.0 - a.0 + 1) as i64 * (b.1 - a.1 + 1) as i64 > 100_000 {
                return Err("GROUND: excessive triangle extent".into());
            }
            for x in a.0..=b.0 {
                for z in a.1..=b.1 {
                    cells.entry((x, z)).or_default().push(id);
                }
            }
        }
        #[cfg(feature = "car-chase-benchmark")]
        let planes = triangles.iter().copied().map(SupportPlane::new).collect();
        Ok(Self {
            triangles,
            cells,
            cell_size,
            #[cfg(feature = "car-chase-benchmark")]
            planes,
        })
    }

    /// Highest actual triangle intersected by a vertical ray at this XZ.
    /// No nearest-box/nearest-vertex height guess and no 3m coverage tolerance.
    pub fn height(&self, p: Vec3) -> Option<f32> {
        self.cells
            .get(&cell(p, self.cell_size))?
            .iter()
            .filter_map(|&id| triangle_height(p, self.triangles[id]))
            .max_by(f32::total_cmp)
    }
}
fn cell(p: Vec3, step: f32) -> (i32, i32) {
    ((p.x / step).floor() as i32, (p.z / step).floor() as i32)
}

pub fn triangle_height(p: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let cross = |u: Vec3, v: Vec3| u.x * v.z - u.z * v.x;
    let den = cross(b - a, c - a);
    if den.abs() < 1e-7 {
        return None;
    }
    let u = cross(p - a, c - a) / den;
    let v = cross(b - a, p - a) / den;
    // Tiny barycentric tolerance only handles shared-edge floating point error.
    (u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001)
        .then_some(a.y + (b.y - a.y) * u + (c.y - a.y) * v)
}

#[derive(Clone, Copy)]
struct Arc {
    distance: f32,
    parameter: f32,
    eye_y: f32,
}

pub struct Route {
    points: Vec<Vec3>,
    arc: Vec<Arc>,
    road: Ground,
    terrain: Ground,
    minimum_clearance: f32,
    maximum_clearance: f32,
}

impl Route {
    pub fn new(points: &[Vec3], mesh: &Mesh, roads: &[[Vec3; 3]]) -> Result<Self, String> {
        if points.len() < 2 || points.len() > 10_000 || points.iter().any(|p| !p.is_finite()) {
            return Err("ROUTE: need finite original LBS points".into());
        }
        let points: Vec<_> = points
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, p)| *i == 0 || p.distance(points[*i - 1]) > 0.01)
            .map(|(_, p)| p)
            .collect();
        if points.len() < 2 {
            return Err("ROUTE: no usable distance".into());
        }
        if points.windows(2).any(|p| p[0].distance(p[1]) > 30.0) {
            return Err(
                "ROUTE: gap over 30m in original points; refusing to invent a connection".into(),
            );
        }
        let road = Ground::new(roads.to_vec())?;
        let triangles = mesh
            .indices
            .chunks_exact(3)
            .map(|tri| {
                let mut p = [Vec3::ZERO; 3];
                for (out, &i) in p.iter_mut().zip(tri) {
                    *out = Vec3::from(
                        mesh.vertices
                            .get(i as usize)
                            .ok_or("GROUND: invalid mesh index")?
                            .position,
                    );
                }
                Ok(p)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let terrain = Ground::new(triangles)?;
        let mut raw = Vec::new();
        for i in 0..points.len() - 1 {
            let steps = (points[i].distance(points[i + 1]) / STEP).ceil().max(1.0) as usize;
            for j in 0..steps {
                let parameter = i as f32 + j as f32 / steps as f32;
                let p = curve(&points, parameter);
                let rh=road.height(p).ok_or_else(||format!("ROUTE: interpolated path leaves actual road at segment {i}; no fallback route"))?;
                let h = terrain
                    .height(p)
                    .ok_or("ROUTE: no triangle below interpolated camera")?;
                if (p.y - rh).abs() > 4.0 || (h - rh) > 0.75 {
                    return Err(format!(
                        "ROUTE: ambiguous/elevated terrain over road at segment {i}"
                    ));
                }
                raw.push((parameter, h));
            }
        }
        let last = *points.last().unwrap();
        if road.height(last).is_none() {
            return Err("ROUTE: final point has no road triangle".into());
        }
        raw.push((
            (points.len() - 1) as f32,
            terrain.height(last).ok_or("ROUTE: final ground missing")?,
        ));
        let mut arc = Vec::with_capacity(raw.len());
        let mut length = 0.0;
        let mut previous = None;
        for (i, &(parameter, h)) in raw.iter().enumerate() {
            // Symmetric ~2m height smoothing; runtime clamps to actual terrain+1.4m.
            let window = &raw[i.saturating_sub(8)..(i + 9).min(raw.len())];
            let smooth = window.iter().map(|p| p.1).sum::<f32>() / window.len() as f32;
            let eye_y = smooth.max(h) + EYE_HEIGHT;
            let mut p = curve(&points, parameter);
            p.y = eye_y;
            if let Some(previous) = previous {
                length += p.distance(previous);
            }
            arc.push(Arc {
                distance: length,
                parameter,
                eye_y,
            });
            previous = Some(p);
        }
        if length < 50.0 {
            return Err("ROUTE: less than 50m of real path; benchmark refused".into());
        }
        let mut route = Self {
            points,
            arc,
            road,
            terrain,
            minimum_clearance: f32::INFINITY,
            maximum_clearance: 0.0,
        };
        // Densely verify the distance lookup and heading, without persisting source data.
        for i in 0..=(length / 0.1).ceil() as usize {
            let d = (i as f32 * 0.1).min(length);
            let eye = route.eye(d)?;
            let ground = route
                .terrain
                .height(eye)
                .ok_or("ROUTE: missing ground during verification")?;
            route.minimum_clearance = route.minimum_clearance.min(eye.y - ground);
            route.maximum_clearance = route.maximum_clearance.max(eye.y - ground);
        }
        Ok(route)
    }
    pub fn length(&self) -> f32 {
        self.arc.last().unwrap().distance
    }
    pub fn minimum_clearance(&self) -> f32 {
        self.minimum_clearance
    }
    pub fn maximum_clearance(&self) -> f32 {
        self.maximum_clearance
    }

    pub fn eye(&self, distance: f32) -> Result<Vec3, String> {
        if !distance.is_finite() {
            return Err("ROUTE: non-finite camera distance".into());
        }
        let distance = distance.clamp(0.0, self.length());
        let upper = self
            .arc
            .partition_point(|p| p.distance < distance)
            .clamp(1, self.arc.len() - 1);
        let a = self.arc[upper - 1];
        let b = self.arc[upper];
        let t = ((distance - a.distance) / (b.distance - a.distance).max(1e-6)).clamp(0.0, 1.0);
        let mut p = curve(&self.points, a.parameter + (b.parameter - a.parameter) * t);
        let rh = self
            .road
            .height(p)
            .ok_or("ROUTE: live camera lost road; aborting")?;
        let h = self
            .terrain
            .height(p)
            .ok_or("ROUTE: live camera lost ground; aborting")?;
        if h - rh > 0.75 {
            return Err("ROUTE: live terrain obstructs road; aborting".into());
        }
        // A smoothed curve may under-shoot the faceted mesh. Never let the eye do so.
        let smooth = cubic_scalar(
            self.arc[upper.saturating_sub(2)].eye_y,
            a.eye_y,
            b.eye_y,
            self.arc[(upper + 1).min(self.arc.len() - 1)].eye_y,
            t,
        );
        p.y = smooth.max(h + EYE_HEIGHT);
        Ok(p)
    }
    pub fn camera(&self, distance: f32) -> Result<Camera, String> {
        let eye = self.eye(distance)?;
        let before = self.eye((distance - 2.0).max(0.0))?;
        let after = self.eye((distance + 12.0).min(self.length()))?;
        let forward = (after - before).normalize_or(Vec3::Z);
        Ok(Camera {
            eye,
            target: eye + forward * 12.0,
        })
    }
}

fn cubic_scalar(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    0.5 * ((2.0 * b)
        + (-a + c) * t
        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
        + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
}
fn curve(points: &[Vec3], parameter: f32) -> Vec3 {
    let i = (parameter.floor() as usize).min(points.len() - 2);
    let t = (parameter - i as f32).clamp(0.0, 1.0);
    let [a, b, c, d] = [
        points[i.saturating_sub(1)],
        points[i],
        points[i + 1],
        points[(i + 2).min(points.len() - 1)],
    ];
    Vec3::new(
        cubic_scalar(a.x, b.x, c.x, d.x, t),
        cubic_scalar(a.y, b.y, c.y, d.y, t),
        cubic_scalar(a.z, b.z, c.z, d.z, t),
    )
}
