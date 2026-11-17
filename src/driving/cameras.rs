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
}
#[cfg(test)]
mod tests {
    use super::*;
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
