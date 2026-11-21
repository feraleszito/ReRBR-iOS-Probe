//! Read-only installed-content catalog and the future import/validation model.
//! Nothing here installs, extracts or enables content: only the descriptor-backed
//! Corolla and the four retail stage presets pass the descriptor gate.

use rerbr_content::{
    Selection, StageDescriptor, VehicleClassification, VehicleDescriptor, PHYSICS_STAGE_EXTENSIONS,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    Selected,
    Installed,
    MissingFiles,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveCapability {
    /// Authentic retail RBR or integrated RSF NGP car with required physics + 3D files present.
    Drivable,
    /// Installed 3D `.sgc` model verified for Showroom/Garage visualization only (no physics).
    VisualOnly,
    /// Missing required 3D model or config files.
    Unavailable,
}

impl DriveCapability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Drivable => "DISPONIBLE PARA CONDUCIR",
            Self::VisualOnly => "SOLO VISUALIZACIÓN",
            Self::Unavailable => "ARCHIVOS FALTANTES",
        }
    }
    pub fn short_badge(self) -> &'static str {
        match self {
            Self::Drivable => "CONDUCIBLE",
            Self::VisualOnly => "SOLO VISUAL",
            Self::Unavailable => "INCOMPLETO",
        }
    }
}
impl EntryState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Selected => "SELECTED",
            Self::Installed => "INSTALLED",
            Self::MissingFiles => "MISSING FILES",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileAudit {
    /// False when the private Documents root is unavailable or not inspected.
    pub checked: bool,
    pub present: usize,
    pub total: usize,
    pub missing: Vec<String>,
}
impl FileAudit {
    pub fn unchecked() -> Self {
        Self {
            checked: false,
            present: 0,
            total: 0,
            missing: Vec::new(),
        }
    }
    pub fn summary(&self) -> String {
        if !self.checked {
            "PRIVATE FILES NOT INSPECTED".into()
        } else if self.missing.is_empty() {
            format!("PRIVATE FILES {}/{} PRESENT", self.present, self.total)
        } else {
            format!(
                "PRIVATE FILES {}/{} PRESENT - {} MISSING",
                self.present,
                self.total,
                self.missing.len()
            )
        }
    }
    pub fn missing_label(&self) -> Option<String> {
        (self.checked && !self.missing.is_empty())
            .then(|| format!("MISSING: {}", self.missing.join(", ")))
    }
}

#[derive(Debug, Clone)]
pub struct VehicleEntry {
    pub descriptor: VehicleDescriptor,
    pub state: EntryState,
    pub files: FileAudit,
    pub tech_sheet: Option<super::ngp_proto::VehicleTechSheet>,
    pub classification: VehicleClassification,
    pub year: Option<u32>,
    pub drivetrain: Option<String>,
    pub fmod_bank: Option<String>,
    pub using_fallback_model: bool,
    pub model_missing: bool,
    pub has_3d_model: bool,
    pub notes: Vec<String>,
}

pub fn preset_year_for_vehicle_id(id: &str) -> Option<u32> {
    match id {
        "toyota-corolla-wrc" => Some(1999),
        "subaru-impreza-2000" => Some(2000),
        "peugeot-206-wrc" | "mitsubishi-lancer-evo-vii" => Some(2002),
        "subaru-impreza-2003" | "citroen-xsara" | "hyundai-accent-wrc" => Some(2003),
        "mg-zr-s1600" => Some(2004),
        "ford-focus-rs-wrc-2006" => Some(2006),
        _ => None,
    }
}

impl VehicleEntry {
    pub fn display_name_with_year(&self) -> String {
        match self
            .year
            .or_else(|| preset_year_for_vehicle_id(&self.descriptor.id))
        {
            Some(yr) if !self.descriptor.name.contains(&yr.to_string()) => {
                format!("{} ({yr})", self.descriptor.name)
            }
            _ => self.descriptor.name.clone(),
        }
    }

    /// Honest capability classification distinguishing drivable vehicles (original RBR or
    /// integrated RSF NGP) from visual-only 3D models discovered in `Cars/`.
    pub fn capability(&self, ngp_experimental: bool) -> DriveCapability {
        if self.classification == VehicleClassification::VisualInspectionOnly {
            return if self.has_3d_model && !self.model_missing {
                DriveCapability::VisualOnly
            } else {
                DriveCapability::Unavailable
            };
        }
        let is_ngp = self.descriptor.is_ngp_experimental_car();
        if is_ngp {
            let can_drive = ngp_experimental
                && matches!(
                    self.classification,
                    VehicleClassification::RsfNgpVerified | VehicleClassification::RsfNgpPartial
                )
                && !self.model_missing
                && !self.using_fallback_model
                && (!self.files.checked || self.files.missing.is_empty());
            if can_drive {
                DriveCapability::Drivable
            } else if self.has_3d_model && !self.model_missing {
                DriveCapability::VisualOnly
            } else {
                DriveCapability::Unavailable
            }
        } else {
            let can_drive = self.descriptor.validate_current_backend().is_ok()
                && !self.model_missing
                && (!self.files.checked || self.files.missing.is_empty());
            if can_drive {
                DriveCapability::Drivable
            } else if self.has_3d_model && !self.model_missing {
                DriveCapability::VisualOnly
            } else {
                DriveCapability::Unavailable
            }
        }
    }

    pub fn can_showroom_view(&self) -> bool {
        self.has_3d_model && !self.model_missing && !self.using_fallback_model
    }
}
#[derive(Debug, Clone)]
pub struct StageEntry {
    pub descriptor: StageDescriptor,
    pub state: EntryState,
    pub files: FileAudit,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub root: Option<PathBuf>,
    pub vehicles: Vec<VehicleEntry>,
    pub stages: Vec<StageEntry>,
    pub inspected: bool,
    vehicle_aliases: BTreeMap<usize, usize>,
}
impl Catalog {
    /// Descriptor-only catalog (no filesystem access). Used before an explicit scan.
    pub fn descriptors(selection: &Selection) -> Self {
        let mut vehicles: Vec<VehicleEntry> = VehicleDescriptor::presets()
            .into_iter()
            .map(|descriptor| {
                let descriptor = if descriptor.id == selection.vehicle.id {
                    selection.vehicle.clone()
                } else {
                    descriptor
                };
                let state = state_of(
                    &descriptor.id,
                    &selection.vehicle.id,
                    descriptor.validate_current_backend().is_ok(),
                    &FileAudit::unchecked(),
                );
                let classification = descriptor.base_classification();
                let year = preset_year_for_vehicle_id(&descriptor.id);
                VehicleEntry {
                    descriptor,
                    state,
                    files: FileAudit::unchecked(),
                    tech_sheet: None,
                    classification,
                    year,
                    drivetrain: None,
                    fmod_bank: None,
                    using_fallback_model: false,
                    model_missing: false,
                    has_3d_model: false,
                    notes: Vec::new(),
                }
            })
            .collect();
        if !vehicles
            .iter()
            .any(|v| v.descriptor.id == selection.vehicle.id)
        {
            let classification = selection.vehicle.base_classification();
            let year = preset_year_for_vehicle_id(&selection.vehicle.id);
            vehicles.insert(
                0,
                VehicleEntry {
                    descriptor: selection.vehicle.clone(),
                    state: if selection.vehicle.validate_experimental_ngp().is_ok() {
                        EntryState::Selected
                    } else {
                        EntryState::Unsupported
                    },
                    files: FileAudit::unchecked(),
                    tech_sheet: None,
                    classification,
                    year,
                    drivetrain: None,
                    fmod_bank: None,
                    using_fallback_model: false,
                    model_missing: false,
                    has_3d_model: false,
                    notes: Vec::new(),
                },
            );
        }
        let mut stages: Vec<StageEntry> = StageDescriptor::presets()
            .into_iter()
            .map(|descriptor| {
                let descriptor = if descriptor.id == selection.stage.id {
                    selection.stage.clone()
                } else {
                    descriptor
                };
                let state = state_of(
                    &descriptor.id,
                    &selection.stage.id,
                    descriptor.validate_current_backend().is_ok(),
                    &FileAudit::unchecked(),
                );
                StageEntry {
                    descriptor,
                    state,
                    files: FileAudit::unchecked(),
                }
            })
            .collect();
        if !stages.iter().any(|s| s.descriptor.id == selection.stage.id) {
            stages.insert(
                0,
                StageEntry {
                    descriptor: selection.stage.clone(),
                    state: if selection.stage.validate_current_backend().is_ok() {
                        EntryState::Selected
                    } else {
                        EntryState::Unsupported
                    },
                    files: FileAudit::unchecked(),
                },
            );
        }
        Self {
            root: None,
            vehicles,
            stages,
            inspected: false,
            vehicle_aliases: BTreeMap::new(),
        }
    }
    /// Builds the current catalog and performs small read-only existence checks
    /// under the existing private Documents layout. No file content is read.
    pub fn scan(selection: &Selection) -> Self {
        let Some(root) = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from) else {
            return Self::descriptors(selection);
        };
        Self::scan_root(&root, selection)
    }
    pub fn scan_root(root: &Path, selection: &Selection) -> Self {
        let mut catalog = Self::descriptors(selection);
        catalog.root = Some(root.to_path_buf());
        catalog.inspected = true;
        let rbr_root = root.join("RBR");
        for vehicle in &mut catalog.vehicles {
            vehicle.files = audit_vehicle(root, &vehicle.descriptor);
            vehicle.has_3d_model =
                rerbr_sim::rbr_formats::cars::resolve(&rbr_root, &vehicle.descriptor.model)
                    .is_some();
            vehicle.tech_sheet =
                super::ngp_proto::inspect_vehicle_in_documents(root, &vehicle.descriptor);
            if let Some(sheet) = &vehicle.tech_sheet {
                vehicle.drivetrain = Some(sheet.drivetrain.label().into());
            }
            vehicle.state = state_of(
                &vehicle.descriptor.id,
                &selection.vehicle.id,
                vehicle.descriptor.validate_current_backend().is_ok(),
                &vehicle.files,
            );
        }
        // Dynamically merge installed RSF NGP vehicles (cleanroom NGPCarMenu / RBRCIT discovery)
        for item in super::rsf_catalog::discover(&rbr_root) {
            if item.using_fallback_model {
                eprintln!(
                    "catalog WARNING: {} using fallback 3D model '{}' (notes={:?})",
                    item.descriptor.id, item.descriptor.model, item.notes
                );
            }
            if let Some(existing) = catalog
                .vehicles
                .iter_mut()
                .find(|v| v.descriptor.id == item.descriptor.id)
            {
                let preset_own_model_exists =
                    rerbr_sim::rbr_formats::cars::resolve(&rbr_root, &existing.descriptor.model)
                        .is_some();
                if (item.using_fallback_model || item.model_missing) && preset_own_model_exists {
                    existing.using_fallback_model = false;
                    existing.model_missing = false;
                } else {
                    let prev_optional = existing.descriptor.optional_textures.clone();
                    let prev_audio = existing.descriptor.audio_index.clone();
                    existing.descriptor = item.descriptor;
                    if existing.descriptor.optional_textures.is_empty() && !prev_optional.is_empty()
                    {
                        existing.descriptor.optional_textures = prev_optional;
                    }
                    if (existing.descriptor.audio_index == "Audio/Cars/Cars.ini"
                        || existing.descriptor.audio_index == "AudioFMOD/SIN_BANCO_FMOD.bank")
                        && prev_audio.to_ascii_lowercase().ends_with(".bank")
                        && !prev_audio.contains("SIN_BANCO_FMOD")
                    {
                        existing.descriptor.audio_index = prev_audio;
                    }
                    existing.using_fallback_model = item.using_fallback_model;
                    existing.model_missing = item.model_missing;
                }
                let files = audit_vehicle(root, &existing.descriptor);
                let supported = existing.descriptor.validate_experimental_ngp().is_ok()
                    && !existing.using_fallback_model
                    && !existing.model_missing;
                let state = state_of(
                    &existing.descriptor.id,
                    &selection.vehicle.id,
                    supported,
                    &files,
                );
                existing.files = files;
                existing.tech_sheet = item.tech_sheet;
                existing.classification = item.classification;
                existing.year = item.year.or(existing.year);
                existing.drivetrain = item.drivetrain;
                existing.fmod_bank = item.fmod_bank;
                existing.notes = item.notes;
                existing.has_3d_model =
                    rerbr_sim::rbr_formats::cars::resolve(&rbr_root, &existing.descriptor.model)
                        .is_some();
                existing.state = state;
            } else {
                let mut files = audit_vehicle(root, &item.descriptor);
                if item.pack_missing {
                    if let Some(pack) = item.descriptor.rsf_pack.as_deref() {
                        let rel = format!("rsfdata/cars/{pack}");
                        if !files.missing.iter().any(|m| m.eq_ignore_ascii_case(&rel)) {
                            files.missing.push(rel);
                        }
                        files.total += 1;
                    }
                }
                let supported = item.descriptor.validate_experimental_ngp().is_ok()
                    && !item.using_fallback_model
                    && !item.model_missing
                    && !item.pack_missing;
                let state = state_of(
                    &item.descriptor.id,
                    &selection.vehicle.id,
                    supported,
                    &files,
                );
                let has_3d_model =
                    rerbr_sim::rbr_formats::cars::resolve(&rbr_root, &item.descriptor.model)
                        .is_some();
                catalog.vehicles.push(VehicleEntry {
                    descriptor: item.descriptor,
                    state,
                    files,
                    tech_sheet: item.tech_sheet,
                    classification: item.classification,
                    year: item.year,
                    drivetrain: item.drivetrain,
                    fmod_bank: item.fmod_bank,
                    using_fallback_model: item.using_fallback_model,
                    model_missing: item.model_missing,
                    has_3d_model,
                    notes: item.notes,
                });
            }
        }
        // Discover any additional 3D `.sgc` car folders installed under `Documents/RBR/Cars/`
        // strictly as `VisualInspectionOnly` so they can be viewed in the Title Screen & Garage
        // without enabling unverified driving.
        let existing_models: std::collections::BTreeSet<String> = catalog
            .vehicles
            .iter()
            .map(|v| v.descriptor.model.replace('\\', "/").to_ascii_lowercase())
            .collect();
        let mut existing_ids: std::collections::BTreeSet<String> = catalog
            .vehicles
            .iter()
            .map(|v| v.descriptor.id.clone())
            .collect();
        for item in super::rsf_catalog::discover_visual_car_folders(
            &rbr_root,
            &existing_models,
            &mut existing_ids,
        ) {
            let files = audit_vehicle(root, &item.descriptor);
            catalog.vehicles.push(VehicleEntry {
                descriptor: item.descriptor,
                state: EntryState::Unsupported,
                files,
                tech_sheet: None,
                classification: VehicleClassification::VisualInspectionOnly,
                year: item.year,
                drivetrain: item.drivetrain,
                fmod_bank: None,
                using_fallback_model: false,
                model_missing: false,
                has_3d_model: true,
                notes: item.notes,
            });
        }
        for stage in &mut catalog.stages {
            if let Some(discovered_maps) = resolve_stage_maps_dir(root, &stage.descriptor) {
                stage.descriptor.documents_maps = discovered_maps;
            }
            stage.files = audit_stage(root, &stage.descriptor);
            stage.state = state_of(
                &stage.descriptor.id,
                &selection.stage.id,
                stage.descriptor.validate_current_backend().is_ok(),
                &stage.files,
            );
        }
        catalog.index_vehicle_aliases();
        catalog
    }

    fn index_vehicle_aliases(&mut self) {
        let mut vehicles_by_identity = BTreeMap::new();
        self.vehicle_aliases.clear();
        for (index, entry) in self.vehicles.iter().enumerate() {
            if entry.using_fallback_model || entry.model_missing {
                continue;
            }
            let v = &entry.descriptor;
            let path = |s: &str| s.replace('\\', "/").to_ascii_lowercase();
            let name: String = v
                .name
                .chars()
                .flat_map(char::to_lowercase)
                .filter(|ch| ch.is_alphanumeric())
                .collect();
            let key = (
                name,
                path(&v.model),
                path(&v.parts),
                path(&v.shaders),
                path(&v.shader_settings),
                v.rsf_pack.as_deref().map(str::to_ascii_lowercase),
                entry.year,
                v.tyre_type,
                v.forward_gears,
                path(&v.audio_index),
                &v.category,
            );
            let first = *vehicles_by_identity.entry(key).or_insert(index);
            if first != index {
                self.vehicle_aliases.insert(index, first);
            }
        }
    }

    /// IDs remain available for saved vehicles. Browsing shows one entry per installed car.
    pub fn browse_vehicle_indices(&self, prefer: impl Fn(&VehicleEntry) -> bool) -> Vec<usize> {
        if self.vehicle_aliases.is_empty() {
            return (0..self.vehicles.len()).collect();
        }
        let mut rows = Vec::with_capacity(self.browse_vehicle_count());
        let mut positions = Vec::with_capacity(self.vehicles.len());
        for (index, entry) in self.vehicles.iter().enumerate() {
            if let Some(first) = self.vehicle_aliases.get(&index) {
                let position = positions[*first];
                let row: &mut usize = &mut rows[position];
                if prefer(entry) && !prefer(&self.vehicles[*row]) {
                    *row = index;
                }
                positions.push(position);
            } else {
                positions.push(rows.len());
                rows.push(index);
            }
        }
        rows
    }

    pub fn browse_vehicle_count(&self) -> usize {
        self.vehicles.len() - self.vehicle_aliases.len()
    }
    pub fn summary(&self) -> String {
        if !self.inspected {
            return "CONTENT NOT INSPECTED - DOCUMENTS UNAVAILABLE".into();
        }
        let missing: usize = self
            .vehicles
            .iter()
            .map(|v| v.files.missing.len())
            .chain(self.stages.iter().map(|s| s.files.missing.len()))
            .sum();
        format!(
            "SCAN: {} CARS / {} STAGES / {} MISSING FILES",
            self.browse_vehicle_count(),
            self.stages.len(),
            missing
        )
    }
    pub fn vehicle(&self, id: &str) -> Option<&VehicleEntry> {
        self.vehicles.iter().find(|v| v.descriptor.id == id)
    }
    pub fn stage(&self, id: &str) -> Option<&StageEntry> {
        self.stages.iter().find(|s| s.descriptor.id == id)
    }
    /// All catalog vehicles whose own `.sgc` 3D model is present on disk and eligible for
    /// 3D Showroom / Title Screen / Garage rendering (both drivable and visual-only).
    pub fn showroom_candidates(&self) -> Vec<&VehicleEntry> {
        self.browse_vehicle_indices(|_| false)
            .into_iter()
            .map(|index| &self.vehicles[index])
            .filter(|v| v.can_showroom_view())
            .collect()
    }
}

#[cfg(test)]
#[test]
fn catalog_collapses_aliases_but_retains_legacy_lookup_and_ngp_variants() {
    let mut catalog = Catalog::descriptors(&Selection::default());
    let canonical = catalog.vehicles.len();
    let mut entry = catalog.vehicles.last().unwrap().clone();
    entry.descriptor.id = "rsf-bmw-m3".into();
    entry.descriptor.name = "BMW M3".into();
    entry.descriptor.rsf_pack = Some("BMW_M3_ngp7".into());
    catalog.vehicles.push(entry.clone());

    let alias = canonical + 1;
    entry.descriptor.id = "rsf-bmw-m3-2".into();
    entry.descriptor.name = "BMW-M3".into();
    entry.descriptor.model = entry
        .descriptor
        .model
        .to_ascii_uppercase()
        .replace('/', "\\");
    catalog.vehicles.push(entry.clone());

    let variant = canonical + 2;
    entry.descriptor.id = "rsf-bmw-m3-3".into();
    entry.descriptor.rsf_pack = Some("BMW_M3_Evo_ngp7".into());
    catalog.vehicles.push(entry);
    catalog.index_vehicle_aliases();

    let visible = catalog.browse_vehicle_indices(|_| false);
    assert_eq!(&visible[canonical..], &[canonical, variant]);
    assert_eq!(catalog.browse_vehicle_count(), canonical + 2);
    assert!(catalog.vehicle("rsf-bmw-m3-2").is_some());
    let owned = catalog.browse_vehicle_indices(|entry| entry.descriptor.id == "rsf-bmw-m3-2");
    assert_eq!(&owned[canonical..], &[alias, variant]);
}

fn state_of(id: &str, selected: &str, supported: bool, files: &FileAudit) -> EntryState {
    if !supported {
        EntryState::Unsupported
    } else if files.checked && !files.missing.is_empty() {
        EntryState::MissingFiles
    } else if id == selected {
        EntryState::Selected
    } else {
        EntryState::Installed
    }
}

/// Relative paths are resolved under the existing private vehicle root `Documents/RBR`.
pub fn audit_vehicle(root: &Path, vehicle: &VehicleDescriptor) -> FileAudit {
    let install = root.join("RBR");
    let paths = vehicle
        .required_paths()
        .map(str::to_owned)
        .chain(vehicle.optional_textures.iter().cloned())
        .collect::<Vec<_>>();
    audit_paths(&install, &paths)
}

/// Prefers a complete supported Documents maps directory, then the fewest missing files.
pub fn resolve_stage_maps_dir(root: &Path, stage: &StageDescriptor) -> Option<String> {
    let candidates = if stage.id == "harwood-forest" {
        ["Harwood/Maps", "RBR/Maps", "Maps"]
    } else {
        ["RBR/Maps", "Harwood/Maps", "Maps"]
    };
    // A stray LBS in the preferred folder must not hide a complete installation.
    candidates
        .into_iter()
        .filter_map(|dir| {
            let mut candidate = stage.clone();
            candidate.documents_maps = dir.into();
            let audit = audit_stage(root, &candidate);
            (audit.present > 0).then_some((dir, audit.missing.len()))
        })
        .min_by_key(|(_, missing)| *missing)
        .map(|(dir, _)| dir.to_string())
}

/// Stage files remain in the descriptor's existing `Documents/<maps>` directory.
pub fn audit_stage(root: &Path, stage: &StageDescriptor) -> FileAudit {
    let mut paths = Vec::new();
    for extension in PHYSICS_STAGE_EXTENSIONS {
        if let Ok(file) = stage.file(extension) {
            paths.push(file);
        }
    }
    if let Ok(ini) = stage.file("ini") {
        paths.push(ini);
    }
    paths.push("TextureFilenameMap.dat".into());
    // Known retail overcast presets share DDS files with these sibling archives.
    // Existence only; the loader still parses metadata and verifies ZIP/DDS data.
    paths.push(format!("{}_textures.rbz", stage.stem));
    let siblings: &[&str] = match stage.original_map {
        20 | 22 | 43 | 45 | 51 | 53 | 63 | 65 => &["N"],
        23 | 44 | 46 | 64 | 66 => &["E"],
        41 | 42 | 52 | 56 | 61 | 62 => &["M"],
        54 => &["E", "M"],
        _ => &[],
    };
    for s in siblings {
        paths.push(format!("track-{}_{s}_textures.rbz", stage.original_map));
    }
    let mut audit = audit_paths(&root.join(&stage.documents_maps), &paths);
    let boot = audit_paths(&root.join("RBR"), &stage.additional_boot_paths());
    audit.present += boot.present;
    audit.total += boot.total;
    audit
        .missing
        .extend(boot.missing.into_iter().map(|file| format!("RBR/{file}")));
    audit
}

fn audit_paths(root: &Path, paths: &[String]) -> FileAudit {
    let mut missing = Vec::new();
    let mut present = 0;
    for path in paths {
        if rerbr_asset_formats::cars::resolve(root, path).is_some_and(|p| p.is_file()) {
            present += 1;
        } else {
            missing.push(path.clone());
        }
    }
    FileAudit {
        checked: true,
        present,
        total: paths.len(),
        missing,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Unavailable,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportStep {
    pub label: &'static str,
    pub detail: &'static str,
    pub state: StepState,
}

/// Honest roadmap exposed by the Content Manager. No package format is installable.
pub const IMPORT_STEPS: [ImportStep; 6] = [
    ImportStep {
        label: "FILES PICKER",
        detail: "IPADOS DOCUMENT PICKER NOT CONNECTED",
        state: StepState::Unavailable,
    },
    ImportStep {
        label: "PACKAGE INSPECTION",
        detail: "MANIFEST FORMAT NOT DEFINED",
        state: StepState::Planned,
    },
    ImportStep {
        label: "DEPENDENCY CHECK",
        detail: "CONTENT CATALOG NOT IMPLEMENTED",
        state: StepState::Planned,
    },
    ImportStep {
        label: "PHYSICS VALIDATION",
        detail: "CAPABILITY CHECK NOT IMPLEMENTED",
        state: StepState::Planned,
    },
    ImportStep {
        label: "PRIVATE INSTALL",
        detail: "TRANSACTIONAL INSTALL DISABLED",
        state: StepState::Unavailable,
    },
    ImportStep {
        label: "ACTIVATE CONTENT",
        detail: "IMPORT ACTIVATION DISABLED",
        state: StepState::Unavailable,
    },
];

pub const IMPORT_FORMATS: [&str; 0] = [];

pub fn import_available() -> bool {
    false
}

/// Descriptor gate for the current backend. This is not NGP physics certification.
pub fn physics_report(selection: &Selection) -> (bool, String) {
    match (
        selection.vehicle.validate_current_backend(),
        selection.stage.validate_current_backend(),
    ) {
        (Ok(()), Ok(())) => (
            true,
            "DESCRIPTOR PROFILES ACCEPTED - NGP CRCS NOT CERTIFIED".into(),
        ),
        (Err(error), _) | (_, Err(error)) => (false, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_marks_only_the_shared_validated_profile_selected() {
        let selection = Selection::default();
        let catalog = Catalog::descriptors(&selection);
        assert_eq!(catalog.vehicles.len(), 9);
        assert_eq!(catalog.vehicles[0].state, EntryState::Selected);
        for v in &catalog.vehicles[1..8] {
            assert_eq!(v.state, EntryState::Installed);
            assert!(!v.descriptor.is_experimental());
            assert_eq!(
                v.classification,
                VehicleClassification::OriginalRbrFunctional
            );
        }
        assert_eq!(catalog.vehicles[8].state, EntryState::Unsupported);
        assert!(!catalog.vehicles[8].descriptor.is_original_rbr());
        assert_eq!(catalog.stages.len(), 36);
        assert_eq!(catalog.stages[0].state, EntryState::Selected);
        assert!(!catalog.inspected);
        assert!(catalog.vehicle("toyota-corolla-wrc").is_some());
        assert!(catalog.vehicle("ford-focus-rs-wrc-2006").is_some());
        assert!(catalog.stage("harwood-forest").is_some());
        assert!(catalog.stage("chirdonhead").is_some());
        assert!(catalog.stage("cote-darbroz").is_some());
        assert!(catalog.stage("cote-darbroz-ii").is_some());
        assert!(catalog.vehicle("unvalidated").is_none());
    }

    #[test]
    fn unsupported_descriptors_are_reported_not_listed_as_playable() {
        let selection = Selection {
            vehicle: VehicleDescriptor {
                id: "another-car".into(),
                ..VehicleDescriptor::default()
            },
            ..Selection::default()
        };
        let catalog = Catalog::descriptors(&selection);
        assert_eq!(catalog.vehicles[0].state, EntryState::Unsupported);
        assert!(!physics_report(&selection).0);
    }

    #[test]
    fn vehicle_and_stage_file_audits_use_existing_private_paths() {
        let root = std::env::temp_dir().join(format!("rerbr-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let selection = Selection::default();
        let vehicle = audit_vehicle(&root, &selection.vehicle);
        assert!(vehicle.checked);
        assert_eq!(vehicle.present, 0);
        assert_eq!(vehicle.missing.len(), vehicle.total);
        assert_eq!(
            vehicle.total,
            selection.vehicle.required_paths().count() + selection.vehicle.optional_textures.len()
        );
        let stage = audit_stage(&root, &selection.stage);
        assert_eq!(stage.total, PHYSICS_STAGE_EXTENSIONS.len() + 4);
        assert!(stage.missing.contains(&"track-20_O.col".to_string()));
        std::fs::create_dir_all(root.join("Harwood/Maps")).unwrap();
        std::fs::write(root.join("Harwood/Maps/track-20_O.lbs"), b"x").unwrap();
        let partial = audit_stage(&root, &selection.stage);
        assert_eq!(partial.present, 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn complete_alternate_maps_wins_and_texture_loss_is_not_installed() {
        let root = std::env::temp_dir().join(format!("rerbr-map-choice-{}", std::process::id()));
        let selection = Selection::default();
        std::fs::create_dir_all(root.join("Harwood/Maps")).unwrap();
        std::fs::create_dir_all(root.join("RBR/Maps")).unwrap();
        std::fs::write(root.join("Harwood/Maps/track-20_O.lbs"), b"x").unwrap();
        let missing = audit_stage(
            &root,
            &StageDescriptor {
                documents_maps: "RBR/Maps".into(),
                ..selection.stage.clone()
            },
        )
        .missing;
        for file in missing {
            std::fs::write(root.join("RBR/Maps").join(file), b"x").unwrap();
        }
        let catalog = Catalog::scan_root(&root, &selection);
        let stage = catalog.stage("harwood-forest").unwrap();
        assert_eq!(stage.descriptor.documents_maps, "RBR/Maps");
        assert!(stage.files.missing.is_empty());
        // Remove only our synthetic file in the test's private temporary directory.
        std::fs::remove_file(root.join("RBR/Maps/track-20_N_textures.rbz")).unwrap();
        let catalog = Catalog::scan_root(&root, &selection);
        let stage = catalog.stage("harwood-forest").unwrap();
        assert_eq!(stage.state, EntryState::MissingFiles);
        assert_eq!(stage.files.missing, ["track-20_N_textures.rbz"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tarmac_presets_require_their_original_spectator_models() {
        let root = std::env::temp_dir().join(format!("rerbr-tarmac-deps-{}", std::process::id()));
        let stage = StageDescriptor::cote_darbroz();
        assert!(StageDescriptor::harwood_forest()
            .additional_boot_paths()
            .is_empty());
        let audit = audit_stage(&root, &stage);
        for file in stage.additional_boot_paths() {
            assert!(audit.missing.contains(&format!("RBR/{file}")));
        }
        assert_eq!(
            stage.additional_boot_paths(),
            StageDescriptor::cote_darbroz_ii().additional_boot_paths()
        );
    }

    #[test]
    fn import_model_does_not_advertise_any_installable_format() {
        assert!(!import_available());
        assert!(IMPORT_FORMATS.is_empty());
        assert!(IMPORT_STEPS
            .iter()
            .all(|step| step.state != StepState::Unavailable || !step.detail.is_empty()));
        assert!(physics_report(&Selection::default()).0);
    }
}
