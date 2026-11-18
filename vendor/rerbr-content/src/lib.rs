//! Owned runtime descriptors shared by presentation and physics. No assets, importer or registry.
//! Current backend accepts the original Corolla and four retail stages; NGP parity is not certified.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleClassification {
    /// Original base-game RBR car executed on authentic 720 Hz physics.
    OriginalRbrFunctional,
    /// Dynamic RSF / NGP car with plaintext `.lsp` pack, verified 3D model, and runnable NGP 7.4 runtime physics.
    RsfNgpVerified,
    /// RSF / NGP car with partial assets or runtime gaps (e.g., fallback model, NGP 7.5 unverified revision, FMOD absent).
    RsfNgpPartial,
    /// Model inspection only (encrypted pack or missing physics runtime).
    VisualInspectionOnly,
}

impl VehicleClassification {
    pub fn badge(self) -> &'static str {
        match self {
            Self::OriginalRbrFunctional => "RBR ORIGINAL",
            Self::RsfNgpVerified => "RSF NGP",
            Self::RsfNgpPartial => "NGP PARCIAL",
            Self::VisualInspectionOnly => "VISUAL",
        }
    }
}

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
    /// Display group/category (e.g. `WRC 2.0`, `Rally 2`, `Group A7`) from RSF `cargroups.json`.
    pub category: Option<String>,
    /// Pack folder/zip name under `rsfdata/cars/` for dynamically discovered RSF NGP vehicles.
    pub rsf_pack: Option<String>,
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
        if validate_relative_path(path).is_err() {
            return false;
        }
        if self
            .required_paths()
            .chain(self.optional_textures.iter().map(String::as_str))
            .any(|p| same_path(p, path))
        {
            return true;
        }
        if self.is_ngp_experimental_car() {
            let norm = path.replace('\\', "/").to_ascii_lowercase();
            let model_norm = self.model.replace('\\', "/").to_ascii_lowercase();
            if let Some((folder, _)) = model_norm.rsplit_once('/') {
                let prefix = format!("{folder}/");
                if norm.starts_with(&prefix) && norm.ends_with(".dds") {
                    return true;
                }
            }
        }
        false
    }
    pub fn validate_schema(&self) -> Result<(), String> {
        validate_identity(&self.id, &self.name)?;
        if self.original_slot > 7 || !(1..=8).contains(&self.forward_gears) {
            return Err("content vehicle slot/gear count is invalid".into());
        }
        for path in self
            .required_paths()
            .chain(self.optional_textures.iter().map(String::as_str))
            .chain(std::iter::once(self.audio_index.as_str()))
        {
            validate_relative_path(path)?;
        }
        if let Some(pack) = &self.rsf_pack {
            if pack.is_empty()
                || pack == "."
                || pack == ".."
                || pack.contains(['/', '\\', ':'])
                || pack.chars().any(char::is_control)
            {
                return Err(format!("invalid RSF pack identifier: {pack:?}"));
            }
        }
        Ok(())
    }
    pub fn is_original_rbr(&self) -> bool {
        Self::original_rbr_presets().iter().any(|preset| {
            self.id == preset.id
                && self.original_slot == preset.original_slot
                && self.name == preset.name
                && self.forward_gears == preset.forward_gears
                && self.model == preset.model
                && self.parts == preset.parts
                && self.rsf_pack.is_none()
        })
    }
    pub fn is_validated_reference(&self) -> bool {
        self.id == "toyota-corolla-wrc"
    }
    /// Only dynamically loaded / non-retail NGP vehicles are experimental; all 8 base RBR cars are functional.
    pub fn is_experimental(&self) -> bool {
        self.is_ngp_experimental_car()
    }
    pub fn is_ngp_experimental_car(&self) -> bool {
        self.rsf_pack.is_some() || self.id == "ford-focus-rs-wrc-2006"
    }
    pub fn base_classification(&self) -> VehicleClassification {
        if self.is_original_rbr() {
            VehicleClassification::OriginalRbrFunctional
        } else if self.is_ngp_experimental_car() {
            VehicleClassification::RsfNgpPartial
        } else {
            VehicleClassification::VisualInspectionOnly
        }
    }
    /// Backend compatibility gate: accepts all 8 retail RBR cars (with non-Corolla in experimental mode)
    /// while blocking Ford Focus 2006 and external unvalidated mods.
    pub fn validate_current_backend(&self) -> Result<(), String> {
        self.validate_schema()?;
        if !self.is_original_rbr() {
            return Err(format!(
                "unsupported vehicle profile '{}'; only original RBR retail vehicles are supported",
                self.id
            ));
        }
        Ok(())
    }
    /// Experimental NGP compatibility gate: explicitly opts in `ford-focus-rs-wrc-2006` in addition
    /// to original retail cars, never confusing it with certified RBR retail physics.
    pub fn validate_experimental_ngp(&self) -> Result<(), String> {
        self.validate_schema()?;
        if self.is_original_rbr() || self.is_ngp_experimental_car() {
            Ok(())
        } else {
            Err(format!(
                "unsupported experimental vehicle profile '{}'",
                self.id
            ))
        }
    }
}
impl StageDescriptor {
    pub fn is_original_rbr(&self) -> bool {
        Self::presets().iter().any(|preset| {
            self.id == preset.id
                && self.original_map == preset.original_map
                && self.stem == preset.stem
        })
    }
    pub fn is_validated_reference(&self) -> bool {
        matches!(self.original_map, 20 | 22 | 41 | 46)
    }
    pub fn is_experimental(&self) -> bool {
        self.is_original_rbr() && !self.is_validated_reference()
    }
    pub fn surface(&self) -> &'static str {
        match self.original_map {
            41..=46 | 51..=56 => "Tarmac",
            _ => "Gravel",
        }
    }
    pub fn country(&self) -> &'static str {
        match self.original_map {
            10..=15 => "Finland",
            20..=25 => "United Kingdom",
            41..=46 => "France",
            51..=56 => "Japan",
            61..=66 => "United States",
            _ => "Unknown",
        }
    }
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
    pub fn validate_for_mode(&self, experimental_ngp: bool) -> Result<(), String> {
        if experimental_ngp {
            self.vehicle.validate_experimental_ngp()?;
            self.stage.validate_current_backend()
        } else {
            self.validate_current_backend()
        }
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
            category: Some("RBR Original".into()),
            rsf_pack: None,
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
                "focus_wrc06_external_dynamic_mud.dds",
                "focus_wrc06_external_mud.dds",
                "focus_wrc06_external_scrape.dds",
                "focus_wrc06_extra_lights.dds",
            ]
            .map(|n| format!("Cars/FORD_FOCUS_2006/textures/{n}"))
            .into(),
            audio_index: "AudioFMOD/Focus_Mk2_WRC.bank".into(),
            category: Some("WRC 2.0".into()),
            rsf_pack: Some("Ford_Focus_Mk_II_RS_WRC_2006_ngp6".into()),
        }
    }
    fn original_rbr(
        id: &str,
        name: &str,
        original_slot: u32,
        forward_gears: u32,
        folder: &str,
        prefix: &str,
        tex_prefix: &str,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            original_slot,
            tyre_type: 4,
            forward_gears,
            index: "Cars/Cars.ini".into(),
            model: format!("Cars/{folder}/{prefix}.sgc"),
            parts: format!("Cars/{folder}/{prefix}.ini"),
            shaders: format!("Cars/{folder}/{prefix}_shaders.ini"),
            shader_settings: format!("Cars/{folder}/{prefix}_shader_settings_clear.ini"),
            required_textures: [
                format!("{tex_prefix}_external.dds"),
                format!("{tex_prefix}_external_dynamic.dds"),
                format!("{tex_prefix}_external_transparent.dds"),
            ]
            .map(|n| format!("Cars/{folder}/Textures/{n}"))
            .into(),
            optional_textures: [
                format!("{tex_prefix}_internal.dds"),
                format!("{tex_prefix}_internal_transparent.dds"),
                format!("{tex_prefix}_internal_scrape.dds"),
            ]
            .map(|n| format!("Cars/{folder}/Textures/{n}"))
            .into(),
            audio_index: "Audio/Cars/Cars.ini".into(),
            category: Some("RBR Original".into()),
            rsf_pack: None,
        }
    }
    /// All 8 original base-game Richard Burns Rally cars.
    pub fn original_rbr_presets() -> [Self; 8] {
        [
            Self::toyota_corolla_wrc(),
            Self::original_rbr(
                "subaru-impreza-2003",
                "Subaru Impreza WRC 2003",
                5,
                6,
                "impreza03",
                "impreza03",
                "impreza03",
            ),
            Self::original_rbr(
                "subaru-impreza-2000",
                "Subaru Impreza WRC 2000",
                7,
                6,
                "impreza00",
                "impreza00",
                "impreza00",
            ),
            // Retail Physics/p_206/{gravel,tarmac,snow}.lsp configure NumberOfGears 7 (R, N, 1..5).
            Self::original_rbr(
                "peugeot-206-wrc",
                "Peugeot 206 WRC",
                4,
                5,
                "Peugeot_206",
                "Peugeot_206",
                "e_p206",
            ),
            Self::original_rbr(
                "mitsubishi-lancer-evo-vii",
                "Mitsubishi Lancer Evo VII",
                3,
                6,
                "Lancer_EVO_VII",
                "lancer_evo_vII",
                "Lancer_wrc",
            ),
            Self::original_rbr(
                "hyundai-accent-wrc",
                "Hyundai Accent WRC",
                1,
                6,
                "accent",
                "accent",
                "accent",
            ),
            Self::original_rbr("mg-zr-s1600", "MG ZR Super 1600", 2, 6, "zr", "zr", "zr"),
            Self::original_rbr(
                "citroen-xsara",
                "Citroen Xsara (Car00)",
                0,
                6,
                "xsara",
                "xsara",
                "xsara",
            ),
        ]
    }
    pub fn presets() -> Vec<Self> {
        let mut cars = Self::original_rbr_presets().to_vec();
        cars.push(Self::ford_focus_2006());
        cars
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
        if matches!(self.original_map, 41..=46) {
            (0..4)
                .map(|i| format!("Sgs/mo_spectator_{i:02}.sgs"))
                .collect()
        } else {
            Vec::new()
        }
    }
    pub fn validated_presets() -> [Self; 4] {
        [
            Self::harwood_forest(),
            Self::chirdonhead(),
            Self::cote_darbroz(),
            Self::cote_darbroz_ii(),
        ]
    }
    pub fn presets() -> Vec<Self> {
        vec![
            Self::harwood_forest(),
            Self::chirdonhead(),
            Self::cote_darbroz(),
            Self::cote_darbroz_ii(),
            // United Kingdom
            Self::new_rbr(21, "falstone", "Falstone", 193),
            Self::new_rbr(23, "shepherds-shield", "Shepherds Shield", 250),
            Self::new_rbr(24, "harwood-forest-ii", "Harwood Forest II", 282),
            Self::new_rbr(25, "chirdonhead-ii", "Chirdonhead II", 242),
            // France (Mont Blanc)
            Self::new_rbr(42, "joux-verte", "Joux Verte", 158),
            Self::new_rbr(43, "bisanne", "Bisanne", 112),
            Self::new_rbr(44, "joux-plane", "Joux Plane", 321),
            Self::new_rbr(45, "joux-verte-ii", "Joux Verte II", 158),
            // Finland (Arctic)
            Self::new_rbr(10, "kaihuavaara", "Kaihuavaara", 154),
            Self::new_rbr(11, "mustaselka", "Mustaselka", 182),
            Self::new_rbr(12, "sikakama", "Sikakama", 257),
            Self::new_rbr(13, "autiovaara", "Autiovaara", 177),
            Self::new_rbr(14, "kaihuavaara-ii", "Kaihuavaara II", 154),
            Self::new_rbr(15, "mustaselka-ii", "Mustaselka II", 182),
            // Japan (Hokkaido)
            Self::new_rbr(51, "noiker", "Noiker", 357),
            Self::new_rbr(52, "sipirkakim", "Sipirkakim", 344),
            Self::new_rbr(53, "pirka-menoko", "Pirka Menoko", 183),
            Self::new_rbr(54, "tanner", "Tanner", 131),
            Self::new_rbr(55, "noiker-ii", "Noiker II", 357),
            Self::new_rbr(56, "tanner-ii", "Tanner II", 130),
            // United States
            Self::new_rbr(61, "fraizer-wells", "Fraizer Wells", 150),
            Self::new_rbr(62, "prospect-ridge", "Prospect Ridge", 232),
            Self::new_rbr(63, "diamond-creek", "Diamond Creek", 139),
            Self::new_rbr(64, "hualapai-nation", "Hualapai Nation", 332),
            Self::new_rbr(65, "prospect-ridge-ii", "Prospect Ridge II", 232),
            Self::new_rbr(66, "diamond-creek-ii", "Diamond Creek II", 139),
        ]
    }
    fn new_rbr(map: u32, id: &str, name: &str, terrain_blocks: usize) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            original_map: map,
            stem: format!("track-{map}_O"),
            documents_maps: "RBR/Maps".into(),
            weather_letter: 3,
            wetness: 0,
            wear: 1,
            terrain_blocks,
        }
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
    fn original_slots_are_unique_and_original_cars_supported_with_focus_blocked() {
        let cars = VehicleDescriptor::original_rbr_presets();
        let mut slots = std::collections::BTreeSet::new();
        for car in &cars {
            car.validate_schema().unwrap();
            assert!(slots.insert(car.original_slot));
            assert!(car.validate_current_backend().is_ok());
            assert_eq!(car.is_validated_reference(), car.id == "toyota-corolla-wrc");
            assert!(!car.is_experimental());
            assert_eq!(
                car.base_classification(),
                VehicleClassification::OriginalRbrFunctional
            );
        }
        assert_eq!(
            slots.into_iter().collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert_eq!(VehicleDescriptor::presets().len(), 9);
        assert!(VehicleDescriptor::ford_focus_2006()
            .validate_current_backend()
            .is_err());

        // Regression test: Peugeot 206 WRC (Car04) uses 5 forward gears (NumberOfGears 7: R, N, 1..5).
        let p206 = cars
            .iter()
            .find(|c| c.id == "peugeot-206-wrc")
            .expect("peugeot-206-wrc preset must exist");
        assert_eq!(p206.original_slot, 4);
        assert_eq!(p206.forward_gears, 5);
        assert!(p206.validate_current_backend().is_ok());
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
