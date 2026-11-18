//! Read-only installed-content catalog and the future import/validation model.
//! Nothing here installs, extracts or enables content: only the descriptor-backed
//! Corolla and the four retail stage presets pass the descriptor gate.

use rerbr_content::{
    Selection, StageDescriptor, VehicleClassification, VehicleDescriptor, PHYSICS_STAGE_EXTENSIONS,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    Selected,
    Installed,
    MissingFiles,
    Unsupported,
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
    pub drivetrain: Option<String>,
    pub fmod_bank: Option<String>,
    pub using_fallback_model: bool,
    pub notes: Vec<String>,
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
                VehicleEntry {
                    descriptor,
                    state,
                    files: FileAudit::unchecked(),
                    tech_sheet: None,
                    classification,
                    drivetrain: None,
                    fmod_bank: None,
                    using_fallback_model: false,
                    notes: Vec::new(),
                }
            })
            .collect();
        if !vehicles
            .iter()
            .any(|v| v.descriptor.id == selection.vehicle.id)
        {
            let classification = selection.vehicle.base_classification();
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
                    drivetrain: None,
                    fmod_bank: None,
                    using_fallback_model: false,
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
        for vehicle in &mut catalog.vehicles {
            vehicle.files = audit_vehicle(root, &vehicle.descriptor);
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
        let rbr_root = root.join("RBR");
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
                if item.using_fallback_model && preset_own_model_exists {
                    existing.using_fallback_model = false;
                } else {
                    let prev_optional = existing.descriptor.optional_textures.clone();
                    let prev_audio = existing.descriptor.audio_index.clone();
                    existing.descriptor = item.descriptor;
                    if existing.descriptor.optional_textures.is_empty() && !prev_optional.is_empty()
                    {
                        existing.descriptor.optional_textures = prev_optional;
                    }
                    if existing.descriptor.audio_index == "Audio/Cars/Cars.ini"
                        && prev_audio.to_ascii_lowercase().ends_with(".bank")
                    {
                        existing.descriptor.audio_index = prev_audio;
                    }
                    existing.using_fallback_model = item.using_fallback_model;
                }
                let files = audit_vehicle(root, &existing.descriptor);
                let supported = existing.descriptor.validate_experimental_ngp().is_ok();
                let state = state_of(
                    &existing.descriptor.id,
                    &selection.vehicle.id,
                    supported,
                    &files,
                );
                existing.files = files;
                existing.tech_sheet = item.tech_sheet;
                existing.classification = item.classification;
                existing.drivetrain = item.drivetrain;
                existing.fmod_bank = item.fmod_bank;
                existing.notes = item.notes;
                existing.state = state;
            } else {
                let files = audit_vehicle(root, &item.descriptor);
                let supported = item.descriptor.validate_experimental_ngp().is_ok();
                let state = state_of(
                    &item.descriptor.id,
                    &selection.vehicle.id,
                    supported,
                    &files,
                );
                catalog.vehicles.push(VehicleEntry {
                    descriptor: item.descriptor,
                    state,
                    files,
                    tech_sheet: item.tech_sheet,
                    classification: item.classification,
                    drivetrain: item.drivetrain,
                    fmod_bank: item.fmod_bank,
                    using_fallback_model: item.using_fallback_model,
                    notes: item.notes,
                });
            }
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
        catalog
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
            self.vehicles.len(),
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
    let sibling = match stage.original_map {
        20 | 22 => Some("N"),
        23 => Some("E"),
        41 | 42 => Some("M"),
        43 | 45 => Some("N"),
        44 | 46 => Some("E"),
        _ => None,
    };
    if let Some(s) = sibling {
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
        assert_eq!(catalog.stages.len(), 30);
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
