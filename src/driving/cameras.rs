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
#[cfg(test)]
mod tests {
    use super::*;
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
}
