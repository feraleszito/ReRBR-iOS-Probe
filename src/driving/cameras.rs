//! Read-only car-relative camera mounts from the owner's original Corolla INI.
use crate::harwood::Camera;
use rerbr_asset_formats::ini::Section;
use rerbr_render_core::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Chase,
    Bonnet,
    CockpitFallback,
}
impl View {
    pub fn next(self) -> Self {
        match self {
            Self::Chase => Self::Bonnet,
            Self::Bonnet => Self::CockpitFallback,
            Self::CockpitFallback => Self::Chase,
        }
    }
    pub fn index(self) -> u8 {
        match self {
            Self::Chase => 0,
            Self::Bonnet => 1,
            Self::CockpitFallback => 2,
        }
    }
    pub fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Bonnet,
            2 => Self::CockpitFallback,
            _ => Self::Chase,
        }
    }
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Chase => "CHASE",
            Self::Bonnet => "BONNET",
            Self::CockpitFallback => "COCKPIT",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Chase => "CHASE",
            Self::Bonnet => "BONNET",
            Self::CockpitFallback => "COCKPIT VIEW ONLY - NO INTERIOR",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Mount {
    pub eye: Vec3,
    pub target: Vec3,
}
#[derive(Debug, Clone, Copy)]
pub struct Mounts {
    pub bonnet: Mount,
    pub cockpit: Mount,
}
impl Mounts {
    /// Safe geometric fallback mounts used purely for visual-only showroom cars whose `.ini`
    /// omits standard camera sections. Never alters driving camera mounts when present.
    pub fn fallback() -> Self {
        Self {
            bonnet: Mount {
                eye: Vec3::new(0.0, 1.05, 0.35),
                target: Vec3::new(0.0, 0.95, 5.0),
            },
            cockpit: Mount {
                eye: Vec3::new(-0.32, 1.05, -0.15),
                target: Vec3::new(-0.32, 0.98, 5.0),
            },
        }
    }
    pub fn parse(parts: &[Section]) -> Result<Self, String> {
        let mount = |name: &str| -> Result<Mount, String> {
            let part = parts
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("CAMERA: missing [{name}]"))?;
            let vector = |key: &str| -> Result<Vec3, String> {
                let values = part
                    .get(key)
                    .ok_or("CAMERA: missing vector")?
                    .split_whitespace()
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                if values.len() != 3 || !values.iter().all(|v| v.is_finite()) {
                    return Err("CAMERA: invalid vector".into());
                }
                Ok(Vec3::from_slice(&values))
            };
            let m = Mount {
                eye: vector("Pos")?,
                target: vector("Target")?,
            };
            if m.eye.distance_squared(m.target) < 0.01 {
                return Err("CAMERA: coincident eye/target".into());
            }
            Ok(m)
        };
        Ok(Self {
            bonnet: mount("Cam_bonnet2")?,
            cockpit: mount("Cam_internal")?,
        })
    }
    pub fn camera(self, view: View, model: Mat4) -> Option<Camera> {
        self.camera_with_cockpit(view, model, None)
    }
    pub fn camera_with_cockpit(
        self,
        view: View,
        model: Mat4,
        cockpit_node: Option<Mat4>,
    ) -> Option<Camera> {
        let mount = match view {
            View::Chase => return None,
            View::Bonnet => self.bonnet,
            View::CockpitFallback => {
                if let Some(node) = cockpit_node {
                    let wheel_pos = node.w_axis.truncate();
                    let forward =
                        (self.cockpit.target - self.cockpit.eye).normalize_or(Vec3::NEG_Z);
                    // Pull the interior eye behind the steering wheel pivot so the wheel is visible.
                    let eye = Vec3::new(wheel_pos.x, wheel_pos.y + 0.16, wheel_pos.z + 0.50);
                    Mount {
                        eye,
                        target: eye + forward * 3.0,
                    }
                } else {
                    self.cockpit
                }
            }
        };
        let up = model.transform_vector3(Vec3::Y).normalize_or(Vec3::Y);
        Some(Camera {
            eye: model.transform_point3(mount.eye),
            target: model.transform_point3(mount.target),
            up,
        })
    }
    pub fn camera_with_override(
        self,
        view: View,
        model: Mat4,
        cockpit_node: Option<Mat4>,
        ov: CameraOverride,
    ) -> Option<Camera> {
        self.camera_with_cockpit(view, model, cockpit_node)
            .map(|base| ov.apply(base, model))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraOverride {
    pub fov_degrees: f32,
    pub pos_offset: Vec3,
    pub target_offset: Vec3,
}
impl CameraOverride {
    pub fn default_for(_view: View) -> Self {
        Self {
            fov_degrees: 65.0,
            pos_offset: Vec3::ZERO,
            target_offset: Vec3::ZERO,
        }
    }
    /// Position and target XYZ are offsets in the car model's local frame.
    pub fn apply(self, base: Camera, model: Mat4) -> Camera {
        let ov = self.sanitized();
        let eye = base.eye + model.transform_vector3(ov.pos_offset);
        let mut target = base.target + model.transform_vector3(ov.target_offset);
        if eye.distance_squared(target) < 0.01 {
            target = eye + (base.target - base.eye).normalize_or(Vec3::NEG_Z);
        }
        Camera {
            eye,
            target,
            up: base.up,
        }
    }
    pub fn sanitized(mut self) -> Self {
        self.fov_degrees = if self.fov_degrees.is_finite() {
            self.fov_degrees.clamp(30.0, 110.0)
        } else {
            65.0
        };
        let clamp_vec = |v: Vec3| {
            Vec3::new(
                if v.x.is_finite() {
                    v.x.clamp(-3.0, 3.0)
                } else {
                    0.0
                },
                if v.y.is_finite() {
                    v.y.clamp(-3.0, 3.0)
                } else {
                    0.0
                },
                if v.z.is_finite() {
                    v.z.clamp(-3.0, 3.0)
                } else {
                    0.0
                },
            )
        };
        self.pos_offset = clamp_vec(self.pos_offset);
        self.target_offset = clamp_vec(self.target_offset);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraOverrides {
    pub chase: CameraOverride,
    pub bonnet: CameraOverride,
    pub cockpit: CameraOverride,
}
impl Default for CameraOverrides {
    fn default() -> Self {
        Self {
            chase: CameraOverride::default_for(View::Chase),
            bonnet: CameraOverride::default_for(View::Bonnet),
            cockpit: CameraOverride::default_for(View::CockpitFallback),
        }
    }
}
impl CameraOverrides {
    pub fn for_view(&self, view: View) -> CameraOverride {
        match view {
            View::Chase => self.chase,
            View::Bonnet => self.bonnet,
            View::CockpitFallback => self.cockpit,
        }
    }
    pub fn for_view_mut(&mut self, view: View) -> &mut CameraOverride {
        match view {
            View::Chase => &mut self.chase,
            View::Bonnet => &mut self.bonnet,
            View::CockpitFallback => &mut self.cockpit,
        }
    }
    pub fn reset_view(&mut self, view: View) {
        *self.for_view_mut(view) = CameraOverride::default_for(view);
    }
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some((prefix, field)) = key.split_once('.') else {
                continue;
            };
            let view = match prefix {
                "chase" => View::Chase,
                "bonnet" => View::Bonnet,
                "cockpit" => View::CockpitFallback,
                _ => continue,
            };
            let Ok(v) = value.trim().parse::<f32>() else {
                continue;
            };
            let target = out.for_view_mut(view);
            match field {
                "fov" => target.fov_degrees = v,
                "px" => target.pos_offset.x = v,
                "py" => target.pos_offset.y = v,
                "pz" => target.pos_offset.z = v,
                "tx" => target.target_offset.x = v,
                "ty" => target.target_offset.y = v,
                "tz" => target.target_offset.z = v,
                _ => {}
            }
            *target = target.sanitized();
        }
        out
    }
    pub fn serialize(&self) -> String {
        let mut s = String::new();
        for (name, ov) in [
            ("chase", self.chase),
            ("bonnet", self.bonnet),
            ("cockpit", self.cockpit),
        ] {
            let ov = ov.sanitized();
            s.push_str(&format!(
                "{name}.fov={:.2}\n{name}.px={:.3}\n{name}.py={:.3}\n{name}.pz={:.3}\n{name}.tx={:.3}\n{name}.ty={:.3}\n{name}.tz={:.3}\n",
                ov.fov_degrees,
                ov.pos_offset.x,
                ov.pos_offset.y,
                ov.pos_offset.z,
                ov.target_offset.x,
                ov.target_offset.y,
                ov.target_offset.z,
            ));
        }
        s
    }
    fn path() -> Option<std::path::PathBuf> {
        std::env::var_os("RERBR_PROBE_DOCUMENTS")
            .map(|d| std::path::PathBuf::from(d).join("rerbr-camera-overrides.txt"))
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |s| Self::parse(&s))
    }
    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Err(error) = std::fs::write(path, self.serialize()) {
                eprintln!("CAMERA SETTINGS: {error}");
            }
        }
    }
}

/// Independent per-vehicle camera overrides keyed by stable `vehicle.id`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleCameraStore {
    pub global_defaults: CameraOverrides,
    pub per_vehicle: std::collections::BTreeMap<String, CameraOverrides>,
}

impl VehicleCameraStore {
    pub fn for_vehicle(&self, vehicle_id: &str) -> CameraOverrides {
        self.per_vehicle
            .get(vehicle_id)
            .copied()
            .unwrap_or(self.global_defaults)
    }

    pub fn for_vehicle_mut(&mut self, vehicle_id: &str) -> &mut CameraOverrides {
        let fallback = self.global_defaults;
        self.per_vehicle
            .entry(vehicle_id.to_string())
            .or_insert(fallback)
    }

    pub fn reset_vehicle_view(&mut self, vehicle_id: &str, view: View) {
        let ov = self.for_vehicle_mut(vehicle_id);
        ov.reset_view(view);
    }

    pub fn serialize(&self) -> String {
        let mut out = String::from("[global]\n");
        out.push_str(&self.global_defaults.serialize());
        for (id, ov) in &self.per_vehicle {
            let clean_id = id.trim();
            if clean_id.is_empty() {
                continue;
            }
            out.push_str(&format!("[vehicle:{clean_id}]\n"));
            out.push_str(&ov.serialize());
        }
        out
    }

    pub fn parse(text: &str) -> Self {
        let mut store = Self::default();
        let mut current_vehicle: Option<String> = None;
        let mut buf = String::new();
        let flush = |store: &mut Self, target: &Option<String>, chunk: &str| {
            if chunk.trim().is_empty() {
                return;
            }
            let parsed = CameraOverrides::parse(chunk);
            if let Some(id) = target {
                store.per_vehicle.insert(id.clone(), parsed);
            } else {
                store.global_defaults = parsed;
            }
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "[global]" {
                flush(&mut store, &current_vehicle, &buf);
                buf.clear();
                current_vehicle = None;
            } else if let Some(rest) = trimmed
                .strip_prefix("[vehicle:")
                .and_then(|s| s.strip_suffix(']'))
            {
                flush(&mut store, &current_vehicle, &buf);
                buf.clear();
                let id = rest.trim();
                if !id.is_empty() {
                    current_vehicle = Some(id.to_string());
                }
            } else {
                buf.push_str(line);
                buf.push('\n');
            }
        }
        flush(&mut store, &current_vehicle, &buf);
        store
    }

    fn path_v2() -> Option<std::path::PathBuf> {
        std::env::var_os("RERBR_PROBE_DOCUMENTS")
            .map(|d| std::path::PathBuf::from(d).join("rerbr-camera-overrides-v2.txt"))
    }

    pub fn load() -> Self {
        if let Some(v2) = Self::path_v2().and_then(|p| std::fs::read_to_string(p).ok()) {
            return Self::parse(&v2);
        }
        Self {
            global_defaults: CameraOverrides::load(),
            per_vehicle: std::collections::BTreeMap::new(),
        }
    }

    pub fn save(&self) {
        if let Some(path) = Self::path_v2() {
            if let Err(error) = std::fs::write(path, self.serialize()) {
                eprintln!("CAMERA STORE SAVE: {error}");
            }
        }
    }
}
/// Smooth cinematic presentation camera sweeping around the rally car at stage start.
/// At progress = 1.0, cleanly eases into the target chase camera.
pub fn presentation_camera(
    target: Vec3,
    car_forward: Vec3,
    progress: f32,
    chase_camera: Camera,
) -> Camera {
    let t = progress.clamp(0.0, 1.0);
    // Smoothstep: s = 3*t^2 - 2*t^3
    let s = t * t * (3.0 - 2.0 * t);
    let car_fwd = Vec3::new(car_forward.x, 0.0, car_forward.z).normalize_or(Vec3::NEG_Z);
    let car_right = Vec3::new(car_fwd.z, 0.0, -car_fwd.x).normalize_or(Vec3::X);

    // Initial low-slung front-three-quarter presentation view (RBR rally presentation feel)
    let start_offset = car_fwd * 3.6 + car_right * 3.0 + Vec3::Y * 0.45;
    let start_eye = target + start_offset;
    let start_target = target + car_fwd * 0.75 + Vec3::Y * 0.15;

    let eye = start_eye.lerp(chase_camera.eye, s);
    let cam_target = start_target.lerp(chase_camera.target, s);
    let up = Vec3::Y.lerp(chase_camera.up, s).normalize_or(Vec3::Y);

    Camera {
        eye,
        target: cam_target,
        up,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_camera_smoothly_eases_into_chase_camera() {
        let chase = Camera {
            eye: Vec3::new(0.0, 2.0, 5.0),
            target: Vec3::new(0.0, 0.8, 0.0),
            up: Vec3::Y,
        };
        let car_target = Vec3::new(0.0, 0.5, 0.0);
        let car_forward = Vec3::NEG_Z;

        let start = presentation_camera(car_target, car_forward, 0.0, chase);
        assert!(start.eye.is_finite() && start.target.is_finite());
        // Front-side perspective at t=0
        assert!(
            start.eye.z < car_target.z,
            "start eye should be in front of car"
        );

        let mid = presentation_camera(car_target, car_forward, 0.5, chase);
        assert!(mid.eye.is_finite() && mid.target.is_finite());

        let end = presentation_camera(car_target, car_forward, 1.0, chase);
        assert!((end.eye.x - chase.eye.x).abs() < 1e-4);
        assert!((end.eye.y - chase.eye.y).abs() < 1e-4);
        assert!((end.eye.z - chase.eye.z).abs() < 1e-4);
        assert!((end.target.x - chase.target.x).abs() < 1e-4);
        assert!((end.target.y - chase.target.y).abs() < 1e-4);
        assert!((end.target.z - chase.target.z).abs() < 1e-4);
    }
    #[test]
    fn every_position_and_target_axis_is_local_persistent_and_resettable() {
        let base = Camera {
            eye: Vec3::new(0.0, 2.0, 4.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
        };
        let model = Mat4::from_rotation_y(0.7) * Mat4::from_rotation_z(0.3);
        let mut overrides = CameraOverrides::default();
        for view in [View::Chase, View::Bonnet, View::CockpitFallback] {
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                let ov = CameraOverride {
                    pos_offset: axis * 0.25,
                    target_offset: axis * -0.5,
                    fov_degrees: 80.0,
                };
                let camera = ov.apply(base, model);
                assert!(camera
                    .eye
                    .abs_diff_eq(base.eye + model.transform_vector3(axis * 0.25), 1e-6));
                assert!(camera
                    .target
                    .abs_diff_eq(base.target + model.transform_vector3(axis * -0.5), 1e-6));
                *overrides.for_view_mut(view) = ov;
                assert_eq!(CameraOverrides::parse(&overrides.serialize()), overrides);
            }
            overrides.reset_view(view);
            assert_eq!(overrides.for_view(view), CameraOverride::default_for(view));
        }
        let bad = CameraOverrides::parse("chase.fov=NaN\nbonnet.px=inf\ncockpit.tz=100");
        assert_eq!(bad.chase.fov_degrees, 65.0);
        assert_eq!(bad.bonnet.pos_offset.x, 0.0);
        assert_eq!(bad.cockpit.target_offset.z, 3.0);
        let collapse = CameraOverride {
            pos_offset: Vec3::new(0.0, -2.0, -4.0),
            target_offset: Vec3::ZERO,
            fov_degrees: 65.0,
        }
        .apply(base, Mat4::IDENTITY);
        assert!(collapse.eye.is_finite() && collapse.target.is_finite());
        assert!(collapse.eye.distance_squared(collapse.target) >= 0.9);
    }

    #[test]
    fn mounts_follow_body_without_changing_it_and_cycle() {
        let text="[Cam_bonnet2]\nPos=0 1 -1\nTarget=0 1 -4\n[Cam_internal]\nPos=0.3 1 -1\nTarget=0.3 1 -4\n";
        let m = Mounts::parse(&rerbr_asset_formats::ini::parse(text).unwrap()).unwrap();
        let body = Mat4::from_rotation_y(0.5)
            * Mat4::from_rotation_z(0.35)
            * Mat4::from_rotation_x(-0.2)
            * Mat4::from_translation(Vec3::splat(10.0));
        let saved = body;
        let c = m.camera(View::Bonnet, body).unwrap();
        assert_eq!(c.eye, body.transform_point3(m.bonnet.eye));
        assert!(c
            .up
            .abs_diff_eq(body.transform_vector3(Vec3::Y).normalize(), 1e-5));
        assert!(!c.up.abs_diff_eq(Vec3::Y, 1e-2));
        assert_eq!(body, saved);
        let wheel_node = Mat4::from_translation(Vec3::new(0.33, 0.78, -0.75));
        let cockpit = m
            .camera_with_cockpit(View::CockpitFallback, Mat4::IDENTITY, Some(wheel_node))
            .unwrap();
        assert!(
            cockpit.eye.z > -0.75,
            "cockpit eye should sit behind steering wheel"
        );
        assert!(
            cockpit.target.z < -0.75,
            "cockpit target should look forward past steering wheel"
        );
        assert_eq!(View::Chase.next().next().next(), View::Chase);
        assert_eq!(View::from_index(2), View::CockpitFallback);
        assert_eq!(View::Bonnet.index(), 1);
        assert_eq!(View::from_index(9), View::Chase);
        assert!(m.camera(View::Chase, body).is_none());
        assert!(Mounts::parse(&[]).is_err());
    }

    #[test]
    fn per_vehicle_camera_store_isolates_cars_and_persists_across_restarts() {
        let mut store = VehicleCameraStore::default();
        store
            .for_vehicle_mut("toyota-corolla-wrc")
            .for_view_mut(View::Chase)
            .pos_offset
            .y = 0.45;
        store
            .for_vehicle_mut("rsf-ford-fiesta-rally4")
            .for_view_mut(View::Bonnet)
            .fov_degrees = 74.0;

        let encoded = store.serialize();
        let restored = VehicleCameraStore::parse(&encoded);
        assert!(
            (restored
                .for_vehicle("toyota-corolla-wrc")
                .for_view(View::Chase)
                .pos_offset
                .y
                - 0.45)
                .abs()
                < 1e-3
        );
        assert_eq!(
            restored
                .for_vehicle("rsf-ford-fiesta-rally4")
                .for_view(View::Chase)
                .pos_offset
                .y,
            0.0,
            "Modifying Corolla camera must NEVER leak into Fiesta Rally4"
        );
        assert!(
            (restored
                .for_vehicle("rsf-ford-fiesta-rally4")
                .for_view(View::Bonnet)
                .fov_degrees
                - 74.0)
                .abs()
                < 1e-2
        );
        assert_eq!(
            restored.for_vehicle("rsf-hyundai-i20-coupe-wrc-2020"),
            CameraOverrides::default()
        );
    }
}
