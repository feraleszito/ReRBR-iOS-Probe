//! Owned runtime descriptors shared by presentation and physics. No assets, importer or registry.
//! Current backend accepts the original Corolla and four retail stages; NGP parity is not certified.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleDescriptor {
    pub id: String,
    pub name: String,
    pub original_slot: u32,
    pub tyre_type: u32,
    pub forward_gears: u32,
    pub index: String,
    pub model: String,
    pub parts: String,
    pub shaders: String,
    pub shader_settings: String,
    pub required_textures: Vec<String>,
    pub optional_textures: Vec<String>,
    /// The original per-car engine bank is resolved through this mapping, not guessed from the name.
    pub audio_index: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDescriptor {
    pub id: String,
    pub name: String,
    pub original_map: u32,
    pub stem: String,
    /// Relative to Documents; the legacy layout remains unchanged.
    pub documents_maps: String,
    pub weather_letter: u32,
    pub wetness: u32,
    pub wear: u32,
    pub terrain_blocks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    pub vehicle: VehicleDescriptor,
    pub stage: StageDescriptor,
}

pub const PHYSICS_STAGE_EXTENSIONS: [&str; 6] = ["lbs", "col", "trk", "mat", "dls", "fnc"];
pub const DEFAULT_STAGE_STEM: &str = "track-20_O";
pub const DEFAULT_TERRAIN_BLOCKS: usize = 282;

/// Path-schema check only. Not a ZIP extractor, symlink validator or complete installer security policy.
pub fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with(['/', '\\'])
        || path.contains(':')
        || path.chars().any(char::is_control)
    {
        return Err(format!(
            "content path must be private-root relative: {path:?}"
        ));
    }
    if path
        .split(['/', '\\'])
        .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(format!(
            "content path contains an invalid component: {path:?}"
        ));
    }
    Ok(())
}
fn validate_identity(id: &str, name: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || name.trim().is_empty()
        || name.chars().any(char::is_control)
    {
        return Err("content needs a stable lowercase ID and a readable name".into());
    }
    Ok(())
}
pub fn same_path(a: &str, b: &str) -> bool {
    a.replace('\\', "/")
        .eq_ignore_ascii_case(&b.replace('\\', "/"))
}
impl VehicleDescriptor {
    pub fn required_paths(&self) -> impl Iterator<Item = &str> {
        [
            &self.index,
            &self.model,
            &self.parts,
            &self.shaders,
            &self.shader_settings,
        ]
        .into_iter()
        .map(String::as_str)
        .chain(self.required_textures.iter().map(String::as_str))
    }
    pub fn allows(&self, path: &str) -> bool {
        validate_relative_path(path).is_ok()
            && self
                .required_paths()
                .chain(self.optional_textures.iter().map(String::as_str))
                .any(|p| same_path(p, path))
    }
    pub fn validate_schema(&self) -> Result<(), String> {
        validate_identity(&self.id, &self.name)?;
        if self.original_slot == 0 || !(1..=8).contains(&self.forward_gears) {
            return Err("content vehicle slot/gear count is invalid".into());
        }
        for path in self
            .required_paths()
            .chain(self.optional_textures.iter().map(String::as_str))
            .chain(std::iter::once(self.audio_index.as_str()))
        {
            validate_relative_path(path)?;
        }
        Ok(())
    }
    /// Temporary backend compatibility gate, separate from manifest/path validation.
    pub fn validate_current_backend(&self) -> Result<(), String> {
        self.validate_schema()?;
        if self != &Self::default() {
            return Err(format!("unsupported vehicle profile '{}'; this milestone validates only the original Corolla", self.id));
        }
        Ok(())
    }
}
impl StageDescriptor {
    pub fn file(&self, extension: &str) -> Result<String, String> {
        self.validate_schema()?;
        if extension.is_empty() || !extension.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("invalid stage file extension".into());
        }
        Ok(format!("{}.{}", self.stem, extension))
    }
    pub fn validate_schema(&self) -> Result<(), String> {
        validate_identity(&self.id, &self.name)?;
        validate_relative_path(&self.documents_maps)?;
        validate_relative_path(&self.stem)?;
        if self.stem.contains(['/', '\\']) || self.original_map == 0 || self.terrain_blocks == 0 {
            return Err("content stage ID/stem/weather profile is invalid".into());
        }
        Ok(())
    }
    pub fn validate_current_backend(&self) -> Result<(), String> {
        self.validate_schema()?;
        let allowed_maps = ["Harwood/Maps", "RBR/Maps", "Maps"]
            .into_iter()
            .any(|p| same_path(&self.documents_maps, p));
        let matches_preset = allowed_maps
            && Self::presets().iter().any(|preset| {
                self.id == preset.id
                    && self.name == preset.name
                    && self.original_map == preset.original_map
                    && self.stem == preset.stem
                    && self.weather_letter == preset.weather_letter
                    && self.wetness == preset.wetness
                    && self.wear == preset.wear
                    && self.terrain_blocks == preset.terrain_blocks
            });
        if !matches_preset {
            return Err(format!(
                "unsupported stage profile '{}'; only validated presets are enabled",
                self.id
            ));
        }
        Ok(())
    }
}
impl Selection {
    pub fn validate_current_backend(&self) -> Result<(), String> {
        self.vehicle.validate_current_backend()?;
        self.stage.validate_current_backend()
    }
}
impl Default for VehicleDescriptor {
    fn default() -> Self {
        Self {
            id: "toyota-corolla-wrc".into(),
            name: "Toyota Corolla WRC".into(),
            original_slot: 6,
            tyre_type: 4,
            forward_gears: 6,
            index: "Cars/Cars.ini".into(),
            model: "Cars/COROLLA/corolla.sgc".into(),
            parts: "Cars/COROLLA/corolla.ini".into(),
            shaders: "Cars/COROLLA/corolla_shaders.ini".into(),
            shader_settings: "Cars/COROLLA/corolla_shader_settings_clear.ini".into(),
            required_textures: [
                "corolla_external.dds",
                "corolla_external_dynamic.dds",
                "corolla_external_transparent.dds",
            ]
            .map(|n| format!("Cars/COROLLA/textures/{n}"))
            .into(),
            optional_textures: [
                "corolla_internal.dds",
                "corolla_internal_transparent.dds",
                "corolla_internal_scrape.dds",
                "e_p206_internal_kwindscreen.dds",
            ]
            .map(|n| format!("Cars/COROLLA/textures/{n}"))
            .into(),
            audio_index: "Audio/Cars/Cars.ini".into(),
        }
    }
}
impl VehicleDescriptor {
    pub fn toyota_corolla_wrc() -> Self {
        Self::default()
    }
    /// Ford Focus Mk II RS WRC 2006 (RSF NGP6/7.5 + FMOD bank).
    /// Included in the catalog for honest status reporting; blocked by `validate_current_backend()`.
    pub fn ford_focus_2006() -> Self {
        Self {
            id: "ford-focus-rs-wrc-2006".into(),
            name: "Ford Focus RS WRC 2006".into(),
            original_slot: 1,
            tyre_type: 4,
            forward_gears: 5,
            index: "Cars/Cars.ini".into(),
            model: "Cars/FORD_FOCUS_2006/focus06.sgc".into(),
            parts: "Cars/FORD_FOCUS_2006/focus06.ini".into(),
            shaders: "Cars/FORD_FOCUS_2006/focus06_shaders.ini".into(),
            shader_settings: "Cars/FORD_FOCUS_2006/focus06_shader_settings_clear.ini".into(),
            required_textures: [
                "focus_wrc06_external.dds",
                "focus_wrc06_external_dynamic.dds",
                "focus_wrc06_external_transparent.dds",
            ]
            .map(|n| format!("Cars/FORD_FOCUS_2006/textures/{n}"))
            .into(),
            optional_textures: [
                "focus_wrc06_internal.dds",
                "focus_wrc06_internal_transparent.dds",
                "focus_wrc06_internal_scrape.dds",
                "focus_wrc06_internal_kwindscreen.dds",
                "focus_wrc06_parts.dds",
            ]
            .map(|n| format!("Cars/FORD_FOCUS_2006/textures/{n}"))
            .into(),
            audio_index: "AudioFMOD/Focus_Mk2_WRC.bank".into(),
        }
    }
    pub fn presets() -> [Self; 2] {
        [Self::toyota_corolla_wrc(), Self::ford_focus_2006()]
    }
}
impl Default for StageDescriptor {
    fn default() -> Self {
        Self::harwood_forest()
    }
}
impl StageDescriptor {
    pub fn harwood_forest() -> Self {
        Self {
            id: "harwood-forest".into(),
            name: "Harwood Forest".into(),
            original_map: 20,
            stem: DEFAULT_STAGE_STEM.into(),
            documents_maps: "Harwood/Maps".into(),
            weather_letter: 3,
            wetness: 0,
            wear: 1,
            terrain_blocks: DEFAULT_TERRAIN_BLOCKS,
        }
    }
    pub fn chirdonhead() -> Self {
        Self {
            id: "chirdonhead".into(),
            name: "Chirdonhead".into(),
            original_map: 22,
            stem: "track-22_O".into(),
            documents_maps: "RBR/Maps".into(),
            weather_letter: 3,
            wetness: 0,
            wear: 1,
            terrain_blocks: 229,
        }
    }
    pub fn cote_darbroz() -> Self {
        Self {
            id: "cote-darbroz".into(),
            name: "Cote d'Arbroz".into(),
            original_map: 41,
            stem: "track-41_O".into(),
            documents_maps: "RBR/Maps".into(),
            weather_letter: 3,
            wetness: 0,
            wear: 1,
            terrain_blocks: 147,
        }
    }
    pub fn cote_darbroz_ii() -> Self {
        Self {
            id: "cote-darbroz-ii".into(),
            name: "Cote d'Arbroz II".into(),
            original_map: 46,
            stem: "track-46_O".into(),
            documents_maps: "RBR/Maps".into(),
            weather_letter: 3,
            wetness: 0,
            wear: 1,
            terrain_blocks: 144,
        }
    }
    /// Original Montblanc flip-spectator models needed to build the car's handles.
    /// Britain models remain part of the legacy common boot requirements.
    pub fn additional_boot_paths(&self) -> Vec<String> {
        if matches!(self.original_map, 41 | 46) {
            (0..4)
                .map(|i| format!("Sgs/mo_spectator_{i:02}.sgs"))
                .collect()
        } else {
            Vec::new()
        }
    }
    pub fn presets() -> [Self; 4] {
        [
            Self::harwood_forest(),
            Self::chirdonhead(),
            Self::cote_darbroz(),
            Self::cote_darbroz_ii(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validated_default_preserves_legacy_layout_and_dependencies() {
        let selection = Selection::default();
        selection.validate_current_backend().unwrap();
        assert_eq!(selection.stage.file("col").unwrap(), "track-20_O.col");
        assert_eq!(selection.stage.documents_maps, "Harwood/Maps");
        assert_eq!(selection.vehicle.required_paths().count(), 8);
        assert!(selection
            .vehicle
            .allows("cars\\corolla\\textures\\COROLLA_INTERNAL.dds"));
        assert!(!selection.vehicle.allows("Cars/OTHER/other.sgc"));
    }
    #[test]
    fn paths_cannot_escape_private_roots() {
        for path in [
            "../car.sgc",
            "Cars/../x",
            "C:\\game\\car.sgc",
            "/absolute",
            "\\\\server\\share",
            "a//b",
            "a/./b",
            "a\0b",
        ] {
            assert!(validate_relative_path(path).is_err(), "{path:?}");
        }
        assert!(same_path(
            "Cars/COROLLA/corolla.sgc",
            "cars\\corolla\\COROLLA.sgc"
        ));
    }
    #[test]
    fn runtime_owned_profiles_are_not_confused_with_validated_support() {
        let other = Selection {
            vehicle: VehicleDescriptor {
                id: String::from("another-car"),
                ..VehicleDescriptor::default()
            },
            ..Selection::default()
        };
        other.vehicle.validate_schema().unwrap();
        assert!(other
            .validate_current_backend()
            .unwrap_err()
            .contains("unsupported vehicle"));
        let mismatched = Selection {
            vehicle: VehicleDescriptor {
                original_slot: 5,
                ..VehicleDescriptor::default()
            },
            ..Selection::default()
        };
        assert!(mismatched.validate_current_backend().is_err());
        let stage = StageDescriptor {
            original_map: 21,
            ..StageDescriptor::default()
        };
        assert!(stage
            .validate_current_backend()
            .unwrap_err()
            .contains("unsupported stage"));
        assert!(stage.file("../col").is_err());
    }
}
