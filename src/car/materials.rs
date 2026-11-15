//! Adapted from reRBR car/shading.rs. See UPSTREAM.md.
use rerbr_asset_formats::{car_parts, ini::Section};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    Opaque,
    Blended,
    Plastic,
    InteriorGlass,
    TwoSided,
}
impl Pass {
    pub fn blended(self) -> bool {
        !matches!(self, Self::Opaque | Self::TwoSided)
    }
    pub fn depth_write(self) -> bool {
        matches!(self, Self::Opaque | Self::Plastic | Self::TwoSided)
    }
    pub fn cull(self) -> bool {
        !matches!(self, Self::Plastic | Self::InteriorGlass | Self::TwoSided)
    }
}
pub fn pass_for(shader: &str, opacity: bool) -> Pass {
    match (shader, opacity) {
        ("CarIntGlass", true) => Pass::InteriorGlass,
        ("CarIntGlass", false) => Pass::TwoSided,
        ("CarPlastic", true) => Pass::Plastic,
        (_, true) => Pass::Blended,
        (_, false) => Pass::Opaque,
    }
}
pub fn list_for(shader: &str, opacity: bool) -> u8 {
    match shader {
        "CarRim" | "CarTire" => 0,
        "CarIntGlass" => 2,
        "CarGlass" | "CarPlastic" | "CarLight" if opacity => 2,
        _ => 1,
    }
}
#[derive(Debug, Clone)]
pub struct Material {
    pub shader: String,
    pub name: String,
    pub path: String,
    pub pass: Pass,
    pub list: u8,
}
pub fn resolve(
    shaders: &[Section],
    settings: &[Section],
    shader: &str,
    name: &str,
) -> Result<Material, String> {
    if car_parts::section(settings, shader).is_none() {
        return Err(format!("CAR MATERIAL: unknown shader class {shader}"));
    }
    let path = car_parts::diffuse_path(shaders, name)
        .ok_or_else(|| format!("CAR MATERIAL: no diffuse for {name}"))?;
    let opacity = car_parts::opacity_channel(shaders, name);
    Ok(Material {
        shader: shader.into(),
        name: name.into(),
        path: path.replace('/', "\\").to_ascii_lowercase(),
        pass: pass_for(shader, opacity),
        list: list_for(shader, opacity),
    })
}
