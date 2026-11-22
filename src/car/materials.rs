//! Adapted from reRBR car/shading.rs. See UPSTREAM.md.
use rerbr_asset_formats::{car_parts, ini::Section};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    Opaque,
    Blended,
    CoplanarGlass,
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
    pub path: Option<String>,
    pub pass: Pass,
    pub list: u8,
}
impl Material {
    pub fn untextured() -> Self {
        Self {
            shader: String::new(),
            name: "Unresolved car material".into(),
            path: None,
            pass: Pass::Opaque,
            list: 1,
        }
    }
}
fn resolved_diffuse_and_opacity<'a>(shaders: &'a [Section], name: &str) -> Option<(&'a str, bool)> {
    if let Some(path) = car_parts::diffuse_path(shaders, name) {
        return Some((path, car_parts::opacity_channel(shaders, name)));
    }
    let (scope, sub) = name.split_once("::")?;
    for alt_scope in [scope.replace(' ', "_"), scope.replace('_', " ")] {
        let candidate = format!("{alt_scope}::{sub}");
        if let Some(path) = car_parts::diffuse_path(shaders, &candidate) {
            return Some((path, car_parts::opacity_channel(shaders, &candidate)));
        }
    }
    let set_name = car_parts::section(shaders, "TextureSets")?.get("TextureSet0")?;
    let norm_scope = scope.trim().to_ascii_lowercase().replace(' ', "_");
    let norm_sub = sub.trim().to_ascii_lowercase().replace(' ', "_");
    let suffix = format!("_{norm_scope}_{norm_sub}.dds");
    let path = shaders
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(set_name))
        .flat_map(|s| s.entries.iter())
        .find_map(|(file, path)| {
            let file_norm = file.trim().to_ascii_lowercase().replace(' ', "_");
            file_norm.ends_with(&suffix).then_some(path.as_str())
        })?;
    Some((path, car_parts::opacity_channel(shaders, name)))
}
pub fn resolve(
    shaders: &[Section],
    settings: &[Section],
    shader: &str,
    name: &str,
) -> Result<Material, String> {
    let prefixed = format!("Car{shader}");
    let (effective_shader, forced_opacity) = if car_parts::section(settings, shader).is_some() {
        (shader, false)
    } else if car_parts::section(settings, &prefixed).is_some() {
        (prefixed.as_str(), false)
    } else if let Some(base) = shader
        .strip_suffix(" transparent")
        .or_else(|| shader.strip_suffix("_transparent"))
        .filter(|b| car_parts::section(settings, b).is_some())
    {
        (base, true)
    } else {
        return Err(format!("CAR MATERIAL: unknown shader class {shader}"));
    };
    let (path, opacity) = resolved_diffuse_and_opacity(shaders, name)
        .ok_or_else(|| format!("CAR MATERIAL: no diffuse for {name}"))?;
    let opacity = opacity || forced_opacity;
    Ok(Material {
        shader: effective_shader.into(),
        name: name.into(),
        path: Some(path.replace('/', "\\").to_ascii_lowercase()),
        pass: pass_for(effective_shader, opacity),
        list: list_for(effective_shader, opacity),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rerbr_asset_formats::ini;

    #[test]
    fn resolves_scope_separator_and_declared_texture_set_correspondence() {
        let shaders = ini::parse(
            "[TextureSets]\r\nTextureSet0 = TSDefault\r\n[TSDefault]\r\n\
            yaris_rally2_internal_parts.dds = cars\\yaris_rally2\\textures\\yaris_rally2_internal_parts.dds\r\n\
            yaris_rally2_internal_body.dds = cars\\yaris_rally2\\textures\\yaris_rally2_internal_body.dds\r\n\
            [internal_parts::tire_diffuse_bitmap]\r\nFilename = yaris_rally2_internal_parts.dds\r\n",
        )
        .unwrap();
        let settings = ini::parse("[CarTextureNoDirt]\r\n[CarTire]\r\n").unwrap();

        let parts = resolve(
            &shaders,
            &settings,
            "CarTextureNoDirt",
            "internal parts::tire",
        )
        .expect("space/underscore scope should resolve from car's own shader bitmap");
        assert_eq!(
            parts.path.as_deref(),
            Some("cars\\yaris_rally2\\textures\\yaris_rally2_internal_parts.dds")
        );

        let body = resolve(&shaders, &settings, "CarTire", "internal::body")
            .expect("declared TextureSet0 internal_body.dds should resolve without fallback");
        assert_eq!(
            body.path.as_deref(),
            Some("cars\\yaris_rally2\\textures\\yaris_rally2_internal_body.dds")
        );
    }
}
