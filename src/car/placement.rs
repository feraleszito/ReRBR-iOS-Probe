//! Geometric presentation pose, not vehicle dynamics or a suspension simulation.
use super::mesh::Geometry;
use crate::harwood::{
    benchmark::route::{Ground, Route},
    mesh::Mesh,
    Camera,
};
use rerbr_render_core::{coords, Mat4, Vec3};

const CONTACT: f32 = 0.005;
pub const MAX_WHEEL_TRAVEL: f32 = 0.25;
/// The original reRBR playback chase FOV; previous modes keep their own FOV.
pub const CHASE_FOV: f32 = 65.0;
pub fn view_projection(camera: Camera, aspect: f32) -> Mat4 {
    view_projection_with_fov(camera, aspect, CHASE_FOV)
}
pub fn view_projection_with_fov(camera: Camera, aspect: f32, fov_degrees: f32) -> Mat4 {
    let forward = (camera.target - camera.eye).normalize_or(Vec3::Z);
    let up = camera.up.normalize_or(Vec3::Y);
    let fov = if fov_degrees.is_finite() {
        fov_degrees.clamp(30.0, 110.0)
    } else {
        CHASE_FOV
    };
    coords::projection(fov.to_radians(), aspect)
        * crate::harwood::look_to_lh(camera.eye, forward, up)
}

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub models: [Mat4; 5],
    pub camera: Camera,
    pub max_wheel_travel: f32,
    pub min_wheel_clearance: f32,
    pub boom: f32,
}
pub struct ChasePath {
    route: Option<Route>,
    ground: Ground,
    pivots: [Vec3; 4],
    contacts: [Vec3; 4],
    wheels: [Vec<Vec3>; 4],
    body: Vec<Vec3>,
    reference: Vec3,
}
#[derive(Debug, Clone, Copy)]
pub struct Validation {
    pub samples: usize,
    pub min_wheel_clearance: f32,
    pub max_wheel_travel: f32,
    pub min_boom: f32,
}
impl ChasePath {
    #[cfg(feature = "physics-stage")]
    pub(crate) fn camera_supported(&self, position: Vec3) -> bool {
        self.ground
            .support_height(position)
            .is_some_and(|y| position.y >= y + 0.05)
    }
    #[cfg(feature = "physics-stage")]
    pub(crate) fn camera_clear(&self, camera: Camera, fov: f32, size: (u32, u32)) -> bool {
        if !camera.eye.is_finite()
            || !camera.target.is_finite()
            || !self.camera_supported(camera.eye)
        {
            return false;
        }
        let forward = (camera.target - camera.eye).normalize_or(Vec3::NEG_Z);
        let right = camera.up.cross(forward).normalize_or(Vec3::X);
        let up = forward.cross(right).normalize_or(Vec3::Y);
        let near = camera.eye + forward * 0.05;
        let hy = 0.05 * (fov.clamp(30.0, 110.0).to_radians() * 0.5).tan();
        let hx = hy * size.0.max(1) as f32 / size.1.max(1) as f32;
        [
            near - right * hx - up * hy,
            near + right * hx - up * hy,
            near - right * hx + up * hy,
            near + right * hx + up * hy,
        ]
        .iter()
        .all(|&p| self.camera_supported(p) && self.ground.segment_clear(camera.target, p))
    }
    #[cfg(feature = "physics-drive")]
    pub(crate) fn supports_driving(&self, position: Vec3) -> bool {
        self.ground.support_height(position).is_some()
    }
    pub fn new(route: Route, terrain: &Mesh, car: &Geometry) -> Result<Self, String> {
        Self::with_route(Some(route), terrain, car)
    }
    /// Live physics needs ground/camera queries, not a fabricated benchmark spline.
    #[cfg(feature = "physics-stage")]
    pub fn for_driving(terrain: &Mesh, car: &Geometry) -> Result<Self, String> {
        Self::with_route(None, terrain, car)
    }
    fn with_route(route: Option<Route>, terrain: &Mesh, car: &Geometry) -> Result<Self, String> {
        let triangles = terrain
            .indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]].map(|i| Vec3::from(terrain.vertices[i as usize].position)))
            .collect();
        let ground = Ground::for_vehicle(triangles)?;
        let low = car
            .wheel_points
            .iter()
            .map(|p| p.iter().map(|p| p.y).fold(f32::INFINITY, f32::min))
            .collect::<Vec<_>>();
        let contacts = std::array::from_fn(|k| Vec3::new(car.pivots[k].x, low[k], car.pivots[k].z));
        let average = car.pivots.iter().copied().sum::<Vec3>() * 0.25;
        let floor = low.iter().sum::<f32>() * 0.25;
        let body = car
            .body_points
            .iter()
            .copied()
            .filter(|p| p.y <= floor + 0.55)
            .collect();
        Ok(Self {
            route,
            ground,
            pivots: car.pivots,
            contacts,
            wheels: car.wheel_points.clone(),
            body,
            reference: Vec3::new(average.x, floor, average.z),
        })
    }
    pub fn length(&self) -> f32 {
        self.route.as_ref().map_or(0.0, Route::length)
    }
    pub fn pivots(&self) -> [Vec3; 4] {
        self.pivots
    }
    fn floor(&self, p: Vec3) -> Result<f32, String> {
        self.ground
            .support_height(p)
            .ok_or_else(|| format!("CAR GROUND: unsupported pose at XZ ({:.2},{:.2})", p.x, p.z))
    }
    pub fn frame(&self, distance: f32) -> Result<Frame, String> {
        let route = self
            .route
            .as_ref()
            .ok_or("CAR: no automatic benchmark route in live stage mode")?;
        let eye = route.eye(distance)?;
        let center = Vec3::new(eye.x, self.floor(eye)?, eye.z); // Never use driver-eye height as car origin.
        let path_camera = route.camera(distance)?;
        let direction = path_camera.target - path_camera.eye;
        self.frame_at_pose(center, Vec3::new(direction.x, 0.0, direction.z))
    }
    /// Static parked vehicle showcase pose on terrain for visual diagnostics without simulation physics.
    #[cfg(feature = "physics-drive")]
    pub fn parked_frame(&self, anchor: Vec3, heading: Vec3) -> Result<Frame, String> {
        let center = Vec3::new(anchor.x, self.floor(anchor)?, anchor.z);
        self.frame_at_pose(center, heading)
    }
    fn frame_at_pose(&self, center: Vec3, heading: Vec3) -> Result<Frame, String> {
        let forward = Vec3::new(heading.x, 0.0, heading.z).normalize_or(Vec3::Z);
        let mut body = coords::car_to_stage(Vec3::ZERO, forward);
        body.w_axis = (center - body.transform_vector3(self.reference)).extend(1.0);
        for _ in 0..3 {
            let mut supported = [Vec3::ZERO; 4];
            for (i, p) in self.contacts.iter().enumerate() {
                let mut world = body.transform_point3(*p);
                world.y = self.floor(world)?;
                supported[i] = world;
            }
            let front = (supported[0] + supported[1]) * 0.5;
            let back = (supported[2] + supported[3]) * 0.5;
            let left = (supported[0] + supported[2]) * 0.5 - (supported[1] + supported[3]) * 0.5;
            let forward = (front - back).normalize_or(forward);
            let up = left.cross(forward).normalize_or(Vec3::Y);
            if up.y < 0.5 {
                return Err("CAR GROUND: unsafe ground plane tilt".into());
            }
            let forward = (forward - up * forward.dot(up)).normalize_or(forward);
            let left = forward.cross(up).normalize_or(-Vec3::X);
            body = Mat4::from_cols(
                left.extend(0.0),
                up.extend(0.0),
                (-forward).extend(0.0),
                Vec3::ZERO.extend(1.0),
            );
            body.w_axis = (center - body.transform_vector3(self.reference)).extend(1.0);
        }
        let mut lift = 0.0f32;
        for &p in &self.body {
            let world = body.transform_point3(p);
            lift = lift.max(self.floor(world)? + CONTACT - world.y);
        }
        body.w_axis.y += lift;
        let mut models = [body; 5];
        let mut max_wheel_travel = 0.0f32;
        let mut min_wheel_clearance = f32::INFINITY;
        for (k, points) in self.wheels.iter().enumerate() {
            let mut shift = f32::NEG_INFINITY;
            for &p in points {
                let world = body.transform_point3(p);
                shift = shift.max(self.floor(world)? + CONTACT - world.y);
            }
            if shift.abs() > MAX_WHEEL_TRAVEL {
                return Err(format!("CAR GROUND: wheel {k} needs {shift:.3}m visual travel (limit {MAX_WHEEL_TRAVEL}m); no suspension fallback"));
            }
            models[k + 1] = Mat4::from_translation(Vec3::Y * shift) * body;
            max_wheel_travel = max_wheel_travel.max(shift.abs());
            for &p in points {
                let world = models[k + 1].transform_point3(p);
                min_wheel_clearance = min_wheel_clearance.min(world.y - self.floor(world)?);
            }
        }
        let forward = -body.z_axis.truncate();
        let target = center + Vec3::Y * 0.5;
        let (camera, boom) = self.chase_camera(target, forward)?;
        Ok(Frame {
            models,
            camera,
            max_wheel_travel,
            min_wheel_clearance,
            boom,
        })
    }
    pub(crate) fn chase_camera(
        &self,
        target: Vec3,
        forward: Vec3,
    ) -> Result<(Camera, f32), String> {
        for step in 0..=10 {
            let boom = 6.0 - step as f32 * 0.25;
            let mut eye = target - forward * boom + Vec3::Y * 2.0; // 2.5m above road; target is at 0.5m.
            let Some(height) = self.ground.support_height(eye) else {
                continue;
            };
            eye.y = eye.y.max(height + 0.6);
            if eye.y - target.y > 5.0 {
                continue;
            }
            let f = (target - eye).normalize();
            let r = Vec3::Y.cross(f).normalize();
            let u = f.cross(r);
            let near = eye + f * coords::NEAR;
            let hx = (CHASE_FOV.to_radians() * 0.5).tan() * coords::NEAR;
            let hy = hx / 0.65;
            let points = [
                eye,
                near - r * hx - u * hy,
                near + r * hx - u * hy,
                near - r * hx + u * hy,
                near + r * hx + u * hy,
            ];
            if points.iter().all(|&p| {
                self.ground
                    .support_height(p)
                    .is_some_and(|h| p.y >= h + 0.05)
                    && self.ground.segment_clear(target, p)
            }) {
                return Ok((
                    Camera {
                        eye,
                        target,
                        up: Vec3::Y,
                    },
                    boom,
                ));
            }
        }
        Err(
            "CHASE CAMERA: no supported unobstructed boom within 3.5..6m; refusing unsafe view"
                .into(),
        )
    }
    pub fn validate(&self) -> Result<Validation, String> {
        let count = (self.length() / 0.5).ceil() as usize;
        let mut v = Validation {
            samples: count + 1,
            min_wheel_clearance: f32::INFINITY,
            max_wheel_travel: 0.0,
            min_boom: 6.0,
        };
        for i in 0..=count {
            let d = (i as f32 * 0.5).min(self.length());
            let f = self
                .frame(d)
                .map_err(|e| format!("{e}; route distance={d:.2}m"))?;
            v.min_wheel_clearance = v.min_wheel_clearance.min(f.min_wheel_clearance);
            v.max_wheel_travel = v.max_wheel_travel.max(f.max_wheel_travel);
            v.min_boom = v.min_boom.min(f.boom);
        }
        eprintln!("car-chase POSE/CAMERA PASS: {v:?}; geometric wheel travel only, no physics; live checks repeat each frame");
        Ok(v)
    }
}
