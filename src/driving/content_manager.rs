//! Read-only installed-content catalog and the future import/validation model.
//! Nothing here installs, extracts or enables content: only the descriptor-backed
//! Corolla/Harwood preset is reported as supported and selectable.

use rerbr_content::{Selection, StageDescriptor, VehicleDescriptor, PHYSICS_STAGE_EXTENSIONS};
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
}

#[derive(Debug, Clone)]
pub struct VehicleEntry {
    pub descriptor: VehicleDescriptor,
    pub state: EntryState,
    pub files: FileAudit,
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
        Self {
            root: None,
            vehicles: vec![VehicleEntry {
                descriptor: selection.vehicle.clone(),
                state: if selection.vehicle.validate_current_backend().is_ok() {
                    EntryState::Selected
                } else {
                    EntryState::Unsupported
                },
                files: FileAudit::unchecked(),
            }],
            stages: vec![StageEntry {
                descriptor: selection.stage.clone(),
                state: if selection.stage.validate_current_backend().is_ok() {
                    EntryState::Selected
                } else {
                    EntryState::Unsupported
                },
                files: FileAudit::unchecked(),
            }],
            inspected: false,
        }
    }
    /// Builds the current catalog and performs small read-only existence checks
    /// under the existing private Documents layout. No file content is read.
    pub fn scan(selection: &Selection) -> Self {
        let mut catalog = Self::descriptors(selection);
        let Some(root) = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from) else {
            return catalog;
        };
        catalog.root = Some(root.clone());
        catalog.inspected = true;
        for vehicle in &mut catalog.vehicles {
            vehicle.files = audit_vehicle(&root, &vehicle.descriptor);
            vehicle.state = state_of(
                &vehicle.descriptor.id,
                &selection.vehicle.id,
                vehicle.descriptor.validate_current_backend().is_ok(),
                &vehicle.files,
            );
        }
        for stage in &mut catalog.stages {
            stage.files = audit_stage(&root, &stage.descriptor);
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

/// Stage files remain in the descriptor's existing `Documents/<maps>` directory.
pub fn audit_stage(root: &Path, stage: &StageDescriptor) -> FileAudit {
    let mut paths = Vec::new();
    for extension in PHYSICS_STAGE_EXTENSIONS {
        if let Ok(file) = stage.file(extension) {
            paths.push(file);
        }
    }
    audit_paths(&root.join(&stage.documents_maps), &paths)
}

fn audit_paths(root: &Path, paths: &[String]) -> FileAudit {
    let mut missing = Vec::new();
    let mut present = 0;
    for path in paths {
        if root.join(path).is_file() {
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
        detail: "ONLY COROLLA AND HARWOOD VALIDATED",
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
        assert_eq!(catalog.vehicles.len(), 1);
        assert_eq!(catalog.vehicles[0].state, EntryState::Selected);
        assert_eq!(catalog.stages[0].state, EntryState::Selected);
        assert!(!catalog.inspected);
        assert!(catalog.vehicle("toyota-corolla-wrc").is_some());
        assert!(catalog.stage("harwood-forest").is_some());
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
        assert_eq!(stage.total, PHYSICS_STAGE_EXTENSIONS.len());
        assert!(stage.missing.contains(&"track-20_O.col".to_string()));
        std::fs::create_dir_all(root.join("Harwood/Maps")).unwrap();
        std::fs::write(root.join("Harwood/Maps/track-20_O.lbs"), b"x").unwrap();
        let partial = audit_stage(&root, &selection.stage);
        assert_eq!(partial.present, 1);
        let _ = std::fs::remove_dir_all(&root);
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
