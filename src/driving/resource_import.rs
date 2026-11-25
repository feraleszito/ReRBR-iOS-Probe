//! Auditoría universal de recursos RBR/RSF y adaptador multiplataforma (iOS `UIDocumentPickerViewController` + Android SAF).
//!
//! - Comprueba qué archivos exige realmente el motor antes y después de importar.
//! - Expone diagnósticos claros en español (instalado, incompleto, incompatible, espacio ocupado).
//! - En iOS/iPadOS enlaza con `UIDocumentPickerViewController` + URLs de seguridad + `NSFileCoordinator`,
//!   ejecutando la importación incremental/ZIP en la capa compartida de Rust (`universal_importer`).
//! - En Android enlaza con `SafResourceImporter` (Storage Access Framework) + extractor Rust por FD/JNI.

use super::content_manager::{Catalog, DriveCapability, UniversalCatalogSummary};
use super::universal_importer::{self, StorageBreakdown};
use rerbr_sim::ngp_candidate::{self, NgpRuntimeKind};
use rerbr_sim::rbr_formats::cars;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SafPhase {
    #[default]
    Idle,
    Running,
    Done,
    Error,
}

impl SafPhase {
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Running,
            2 => Self::Done,
            3 => Self::Error,
            _ => Self::Idle,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "EN ESPERA",
            Self::Running => "IMPORTANDO...",
            Self::Done => "COMPLETADO",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SafRuntimeSnapshot {
    pub phase: SafPhase,
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub detail: String,
    pub completion_epoch: u64,
    pub has_persisted_uri: bool,
    pub persisted_label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceHealthReport {
    pub target_rbr_dir: String,
    pub ngp74_ready: bool,
    pub ngp_status_label: String,
    pub drivable_vehicles: usize,
    pub ready_stages: usize,
    pub ready_to_drive: bool,
    pub headline: String,
    pub missing_items: Vec<String>,
    pub catalog_summary: UniversalCatalogSummary,
    pub storage: StorageBreakdown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AndroidImportView {
    pub saf: SafRuntimeSnapshot,
    pub health: ResourceHealthReport,
}

impl AndroidImportView {
    pub fn platform_picker_tag() -> &'static str {
        if cfg!(target_os = "ios") {
            "ARCHIVOS DE iOS / iCLOUD"
        } else if cfg!(target_os = "android") {
            "ANDROID SAF"
        } else {
            "SISTEMA LOCAL"
        }
    }

    pub fn banner_label(&self) -> String {
        match self.saf.phase {
            SafPhase::Running => {
                let mib = self.saf.bytes_done as f64 / (1024.0 * 1024.0);
                if self.saf.files_total > 0 {
                    format!(
                        "IMPORTANDO RECURSOS RBR ({}/{} · {:.1} MiB)...",
                        self.saf.files_done, self.saf.files_total, mib
                    )
                } else {
                    format!("IMPORTANDO RECURSOS RBR ({:.1} MiB)...", mib)
                }
            }
            SafPhase::Error => {
                if self.saf.detail.is_empty() {
                    format!(
                        "ERROR AL IMPORTAR — PULSA PARA ABRIR {}",
                        Self::platform_picker_tag()
                    )
                } else {
                    format!("{} — PULSA PARA IMPORTAR", self.saf.detail)
                }
            }
            SafPhase::Idle | SafPhase::Done => {
                if self.health.ready_to_drive {
                    self.health.headline.clone()
                } else if let Some(first) = self.health.missing_items.first() {
                    format!("FALTAN RECURSOS ({first}) · PULSA PARA IMPORTAR")
                } else {
                    format!(
                        "SIN RECURSOS RBR — PULSA PARA IMPORTAR ({})",
                        Self::platform_picker_tag()
                    )
                }
            }
        }
    }
}

// Estado compartido en memoria para el adaptador iOS (`UIDocumentPickerViewController` + hilo GCD)
struct IosBatchAccumulator {
    snapshot: SafRuntimeSnapshot,
    batch_copied: u32,
    batch_reused: u32,
    batch_protected: u32,
    batch_bytes: u64,
    had_error: bool,
}

static IOS_IMPORT: Mutex<IosBatchAccumulator> = Mutex::new(IosBatchAccumulator {
    snapshot: SafRuntimeSnapshot {
        phase: SafPhase::Idle,
        files_done: 0,
        files_total: 0,
        bytes_done: 0,
        detail: String::new(),
        completion_epoch: 0,
        has_persisted_uri: false,
        persisted_label: String::new(),
    },
    batch_copied: 0,
    batch_reused: 0,
    batch_protected: 0,
    batch_bytes: 0,
    had_error: false,
});

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn rerbr_ios_pick_rbr_folder() -> std::ffi::c_int;
    fn rerbr_ios_pick_rbr_zip() -> std::ffi::c_int;
    fn rerbr_ios_reimport_persisted_folder() -> std::ffi::c_int;
}

unsafe fn cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

/// # Safety
/// `utf8_label` debe ser nulo o apuntar a una cadena C terminada en `\0` válida durante la llamada.
#[no_mangle]
pub unsafe extern "C" fn rerbr_ios_set_persisted_folder_state(
    available: std::ffi::c_int,
    utf8_label: *const std::ffi::c_char,
) {
    let label = cstr_to_string(utf8_label);
    let mut guard = IOS_IMPORT.lock().unwrap();
    guard.snapshot.has_persisted_uri = available != 0;
    guard.snapshot.persisted_label = label;
}

/// # Safety
/// `utf8_label` debe ser nulo o apuntar a una cadena C terminada en `\0` válida durante la llamada.
#[no_mangle]
pub unsafe extern "C" fn rerbr_ios_import_begin_batch(
    count: u32,
    utf8_label: *const std::ffi::c_char,
) {
    let label = cstr_to_string(utf8_label);
    let mut guard = IOS_IMPORT.lock().unwrap();
    guard.snapshot.phase = SafPhase::Running;
    guard.snapshot.files_done = 0;
    guard.snapshot.files_total = count;
    guard.snapshot.bytes_done = 0;
    guard.snapshot.detail = format!("PREPARANDO ACCESO SEGURO iOS: {label}");
    guard.batch_copied = 0;
    guard.batch_reused = 0;
    guard.batch_protected = 0;
    guard.batch_bytes = 0;
    guard.had_error = false;
}

/// # Safety
/// `utf8_path` debe ser nulo o apuntar a una ruta POSIX terminada en `\0` válida durante la llamada.
#[no_mangle]
pub unsafe extern "C" fn rerbr_ios_import_process_path(
    utf8_path: *const std::ffi::c_char,
    is_directory: std::ffi::c_int,
    batch_index: u32,
    batch_total: u32,
) {
    let src_str = cstr_to_string(utf8_path);
    let Some(docs_os) = std::env::var_os("RERBR_PROBE_DOCUMENTS") else {
        let mut guard = IOS_IMPORT.lock().unwrap();
        guard.had_error = true;
        guard.snapshot.phase = SafPhase::Error;
        guard.snapshot.detail = "CARPETA DOCUMENTS DE iOS NO DISPONIBLE".into();
        return;
    };
    let docs_dir = PathBuf::from(docs_os);
    let src_path = PathBuf::from(&src_str);
    let base_bytes = IOS_IMPORT.lock().unwrap().batch_bytes;

    let mut progress_cb = |prog: universal_importer::ImportProgress| {
        let mut guard = IOS_IMPORT.lock().unwrap();
        guard.snapshot.phase = SafPhase::Running;
        guard.snapshot.files_done = prog.files_done;
        guard.snapshot.files_total = prog.files_total;
        guard.snapshot.bytes_done = base_bytes + prog.bytes_done;
        guard.snapshot.detail = if batch_total > 1 {
            format!("[{}/{}] {}", batch_index + 1, batch_total, prog.detail)
        } else {
            prog.detail
        };
    };

    let result = if is_directory != 0 || src_path.is_dir() {
        universal_importer::import_directory_into_documents(&src_path, &docs_dir, &mut progress_cb)
    } else {
        universal_importer::import_zip_into_documents(&src_path, &docs_dir, &mut progress_cb)
    };

    match result {
        Ok(sum) => {
            let mut guard = IOS_IMPORT.lock().unwrap();
            guard.batch_copied += sum.files_copied;
            guard.batch_reused += sum.files_reused;
            guard.batch_protected += sum.files_protected;
            guard.batch_bytes += sum.total_relevant_bytes();
            guard.snapshot.bytes_done = guard.batch_bytes;
            guard.snapshot.detail = sum.headline_es();
        }
        Err(err) => {
            let mut guard = IOS_IMPORT.lock().unwrap();
            guard.had_error = true;
            guard.snapshot.phase = SafPhase::Error;
            guard.snapshot.detail = err;
        }
    }
}

#[no_mangle]
pub extern "C" fn rerbr_ios_import_finish_batch() {
    let mut guard = IOS_IMPORT.lock().unwrap();
    if guard.had_error {
        return;
    }
    let total_files = guard.batch_copied + guard.batch_reused + guard.batch_protected;
    let mib = guard.batch_bytes as f64 / (1024.0 * 1024.0);
    guard.snapshot.phase = SafPhase::Done;
    guard.snapshot.files_done = total_files;
    guard.snapshot.files_total = total_files;
    guard.snapshot.bytes_done = guard.batch_bytes;
    guard.snapshot.detail = if guard.batch_protected > 0 {
        format!(
            "IMPORTACIÓN iOS COMPLETADA: {} NUEVOS · {} REUTILIZADOS · {} PROTEGIDOS ({:.1} MiB)",
            guard.batch_copied, guard.batch_reused, guard.batch_protected, mib
        )
    } else {
        format!(
            "IMPORTACIÓN iOS COMPLETADA: {} NUEVOS · {} REUTILIZADOS ({:.1} MiB)",
            guard.batch_copied, guard.batch_reused, mib
        )
    };
    guard.snapshot.completion_epoch = guard.snapshot.completion_epoch.wrapping_add(1);
}

/// # Safety
/// `utf8_message` debe ser nulo o apuntar a una cadena C terminada en `\0` válida durante la llamada.
#[no_mangle]
pub unsafe extern "C" fn rerbr_ios_import_cancel_or_fail(
    is_error: std::ffi::c_int,
    utf8_message: *const std::ffi::c_char,
) {
    let msg = cstr_to_string(utf8_message);
    let mut guard = IOS_IMPORT.lock().unwrap();
    if is_error != 0 {
        guard.had_error = true;
        guard.snapshot.phase = SafPhase::Error;
    } else if guard.snapshot.phase == SafPhase::Running {
        guard.snapshot.phase = SafPhase::Idle;
    }
    if !msg.is_empty() {
        guard.snapshot.detail = msg;
    }
}

pub fn query_android_saf() -> SafRuntimeSnapshot {
    #[cfg(target_os = "android")]
    {
        let st = crate::android::saf_status();
        SafRuntimeSnapshot {
            phase: SafPhase::from_code(st.state),
            files_done: st.files_done,
            files_total: st.files_total,
            bytes_done: st.bytes_done,
            detail: st.detail,
            completion_epoch: st.completion_epoch,
            has_persisted_uri: st.has_persisted_uri,
            persisted_label: st.persisted_label,
        }
    }
    #[cfg(target_os = "ios")]
    {
        IOS_IMPORT.lock().unwrap().snapshot.clone()
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        IOS_IMPORT.lock().unwrap().snapshot.clone()
    }
}

pub fn request_open_rbr_folder() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        if crate::android::open_rbr_tree_picker() {
            Ok(())
        } else {
            Err("NO SE PUDO ABRIR EL SELECTOR DE CARPETAS ANDROID SAF".into())
        }
    }
    #[cfg(target_os = "ios")]
    {
        // SAFETY: despacha la presentación de UIDocumentPickerViewController en el hilo principal de UIKit.
        if unsafe { rerbr_ios_pick_rbr_folder() } != 0 {
            Ok(())
        } else {
            Err("NO SE PUDO ABRIR EL SELECTOR DE CARPETAS DE iOS".into())
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err("SELECTOR NATIVO DISPONIBLE EN iOS/iPadOS Y ANDROID".into())
    }
}

pub fn request_open_rbr_zip() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        if crate::android::open_rbr_zip_picker() {
            Ok(())
        } else {
            Err("NO SE PUDO ABRIR EL SELECTOR DE ARCHIVOS .ZIP ANDROID SAF".into())
        }
    }
    #[cfg(target_os = "ios")]
    {
        // SAFETY: despacha la presentación de UIDocumentPickerViewController en el hilo principal de UIKit.
        if unsafe { rerbr_ios_pick_rbr_zip() } != 0 {
            Ok(())
        } else {
            Err("NO SE PUDO ABRIR EL SELECTOR DE ARCHIVOS .ZIP DE iOS".into())
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err("SELECTOR NATIVO DISPONIBLE EN iOS/iPadOS Y ANDROID".into())
    }
}

pub fn request_reimport_persisted_folder() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        if crate::android::reimport_persisted_rbr_tree() {
            Ok(())
        } else {
            Err("NO SE PUDO INICIAR LA REIMPORTACIÓN SAF".into())
        }
    }
    #[cfg(target_os = "ios")]
    {
        // SAFETY: resuelve el marcador de seguridad guardado en NSUserDefaults en el hilo principal.
        if unsafe { rerbr_ios_reimport_persisted_folder() } != 0 {
            Ok(())
        } else {
            Err("NO SE PUDO INICIAR LA REIMPORTACIÓN DE CARPETA EN iOS".into())
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err("REIMPORTACIÓN NATIVA DISPONIBLE EN iOS/iPadOS Y ANDROID".into())
    }
}

const CORE_REQUIRED_FILES: &[&str] = &[
    "physics.rbz",
    "misc.rbz",
    "Scripts.rbz",
    "Cars/Cars.ini",
    "Sgs/br_spectator_00.sgs",
    "Sgs/br_spectator_01.sgs",
    "Sgs/br_spectator_02.sgs",
    "Sgs/br_spectator_03.sgs",
];

impl ResourceHealthReport {
    pub fn inspect_env(content: &Catalog, ngp_experimental: bool) -> Self {
        let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from);
        Self::inspect(doc.as_deref(), content, ngp_experimental)
    }

    pub fn inspect(documents: Option<&Path>, content: &Catalog, ngp_experimental: bool) -> Self {
        let catalog_summary = content.universal_summary(ngp_experimental);
        let Some(doc_root) = documents else {
            return Self {
                target_rbr_dir: "ALMACENAMIENTO PRIVADO NO DISPONIBLE".into(),
                ngp74_ready: false,
                ngp_status_label: "SIN ALMACENAMIENTO".into(),
                drivable_vehicles: 0,
                ready_stages: 0,
                ready_to_drive: false,
                headline: "ERROR: ALMACENAMIENTO DE LA APLICACIÓN NO DISPONIBLE".into(),
                missing_items: vec!["RERBR_PROBE_DOCUMENTS NO DEFINIDO".into()],
                catalog_summary,
                storage: StorageBreakdown::default(),
            };
        };

        let storage = StorageBreakdown::scan_documents(doc_root);
        let rbr_root = doc_root.join("RBR");
        let target_rbr_dir = rbr_root.display().to_string();
        let mut missing_items = Vec::new();

        // 1. Verificar NGP 7.4.778.507
        let ngp_probe = ngp_candidate::inspect_runtime_dir(&rbr_root);
        let (ngp74_ready, ngp_status_label) = match ngp_probe.kind {
            NgpRuntimeKind::Verified74 => (true, "NGP 7.4.778.507 VERIFICADO".to_string()),
            NgpRuntimeKind::Detected75Unverified => {
                missing_items.push(
                    "NGP.dll ES 7.5.779 (NO SOPORTADO) — USA NGP 7.4.778.507 EN RBR/Plugins/NGP.dll"
                        .into(),
                );
                (
                    false,
                    "BLOQUEADO: NGP 7.5 DETECTADO (SE REQUIERE 7.4.778)".to_string(),
                )
            }
            NgpRuntimeKind::Missing => {
                missing_items.push("FALTA RBR/Plugins/NGP.dll (VERSIÓN 7.4.778.507)".into());
                (false, "FALTA Plugins/NGP.dll".to_string())
            }
            NgpRuntimeKind::UnsupportedBinary => {
                missing_items.push(format!(
                    "Plugins/NGP.dll INCOMPATIBLE: {}",
                    ngp_probe.summary
                ));
                (false, "Plugins/NGP.dll INCOMPATIBLE".to_string())
            }
        };

        // 2. Verificar archivos nucleares compartidos del motor
        let mut core_ok = true;
        for &rel in CORE_REQUIRED_FILES {
            if cars::resolve(&rbr_root, rel).is_none() {
                core_ok = false;
                missing_items.push(format!("FALTA RBR/{rel}"));
            }
        }

        // 3. Contar vehículos conducibles y tramos listos
        let drivable_entries: Vec<_> = content
            .vehicles
            .iter()
            .filter(|v| {
                v.capability(ngp_experimental) == DriveCapability::Drivable
                    && (!content.inspected || v.files.missing.is_empty())
            })
            .collect();
        let drivable_vehicles = drivable_entries.len();

        let has_retail_drivable = drivable_entries
            .iter()
            .any(|v| !v.descriptor.is_ngp_experimental_car());

        if drivable_vehicles == 0 {
            if let Some(inc) = content.non_drivable_vehicles.first() {
                missing_items.push(format!(
                    "VEHÍCULO {} ({}): {}",
                    inc.name,
                    inc.status.label_es(),
                    inc.reason
                ));
            } else if cars::resolve(&rbr_root, "Cars/FORD_FOCUS_2006").is_some() {
                for req in [
                    "Cars/FORD_FOCUS_2006/focus06.sgc",
                    "Cars/FORD_FOCUS_2006/focus06.ini",
                    "Cars/FORD_FOCUS_2006/focus06_shaders.ini",
                    "Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds",
                    "Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal.dds",
                    "rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6/common.lsp",
                    "AudioFMOD/Focus_Mk2_WRC.bank",
                    "AudioFMOD/Focus_Mk2_WRC.strings.bank",
                ] {
                    if cars::resolve(&rbr_root, req).is_none() {
                        missing_items.push(format!("FALTA RBR/{req}"));
                    }
                }
            } else {
                missing_items.push(
                    "SIN COCHE COMPLETO: COPIA Cars/<Coche> + rsfdata/cars/<Pack> + AudioFMOD/<Banco>.bank"
                        .into(),
                );
            }
        }

        let ready_stages = content
            .stages
            .iter()
            .filter(|s| s.files.checked && s.files.missing.is_empty())
            .count();

        if ready_stages == 0 {
            if let Some(partial) = content
                .stages
                .iter()
                .filter(|s| s.files.checked && s.files.present > 0 && !s.files.missing.is_empty())
                .max_by_key(|s| s.files.present)
            {
                for m in partial.files.missing.iter().take(5) {
                    missing_items.push(format!(
                        "TRAMO {} INCOMPLETO: FALTA RBR/Maps/{m}",
                        partial.descriptor.name
                    ));
                }
            } else {
                missing_items.push(
                    "SIN TRAMO COMPLETO EN RBR/Maps (EJ. Harwood: track-20_O.* + track-20_O_textures.rbz + track-20_N_textures.rbz + TextureFilenameMap.dat)"
                        .into(),
                );
            }
        }

        let physics_backend_ok = ngp74_ready || has_retail_drivable;
        let ready_to_drive =
            core_ok && physics_backend_ok && drivable_vehicles > 0 && ready_stages > 0;

        let headline = if ready_to_drive {
            format!(
                "LISTO PARA CONDUCIR: {drivable_vehicles} COCHE(S) · {ready_stages} TRAMO(S) · {}",
                universal_importer::format_bytes(storage.total_bytes)
            )
        } else {
            format!(
                "RECURSOS INCOMPLETOS ({drivable_vehicles} COCHES · {ready_stages} TRAMOS) — IMPORTA CARPETA O .ZIP"
            )
        };

        Self {
            target_rbr_dir,
            ngp74_ready,
            ngp_status_label,
            drivable_vehicles,
            ready_stages,
            ready_to_drive,
            headline,
            missing_items,
            catalog_summary,
            storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driving::{
        audio, codriver::CoDriverSession, controls::Controls, layout::DrivingLayout, menu,
        pacenote_icons::PacenoteIconAtlas,
    };
    use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
    use rerbr_content::Selection;

    #[test]
    fn empty_directory_reports_exact_missing_core_car_and_stage_files() {
        let tmp =
            std::env::temp_dir().join(format!("rerbr-empty-import-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let sel = Selection::default();
        let catalog = Catalog::scan_root_for_app(&tmp, &sel, true);
        let report = ResourceHealthReport::inspect(Some(&tmp), &catalog, true);

        assert!(!report.ready_to_drive);
        assert!(!report.ngp74_ready);
        assert_eq!(report.drivable_vehicles, 0);
        assert_eq!(report.ready_stages, 0);
        assert!(report
            .missing_items
            .iter()
            .any(|m| m.contains("Plugins/NGP.dll")));
        assert!(report
            .missing_items
            .iter()
            .any(|m| m.contains("physics.rbz")));
        assert!(report
            .missing_items
            .iter()
            .any(|m| m.contains("SIN COCHE COMPLETO")));
        assert!(report
            .missing_items
            .iter()
            .any(|m| m.contains("SIN TRAMO COMPLETO")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn partial_stage_without_sibling_textures_reports_exact_missing_rbz() {
        let tmp =
            std::env::temp_dir().join(format!("rerbr-partial-stage-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let maps = tmp.join("RBR").join("Maps");
        std::fs::create_dir_all(&maps).unwrap();
        std::fs::write(maps.join("TextureFilenameMap.dat"), b"dummy").unwrap();
        for ext in ["lbs", "col", "trk", "mat", "dls", "fnc", "ini"] {
            std::fs::write(maps.join(format!("track-20_O.{ext}")), b"dummy").unwrap();
        }
        std::fs::write(maps.join("track-20_O_textures.rbz"), b"dummy").unwrap();

        let sel = Selection::default();
        let catalog = Catalog::scan_root_for_app(&tmp, &sel, true);
        let report = ResourceHealthReport::inspect(Some(&tmp), &catalog, true);

        assert_eq!(report.ready_stages, 0);
        assert!(
            report
                .missing_items
                .iter()
                .any(|m| m.contains("track-20_N_textures.rbz")),
            "Debe señalar exactamente track-20_N_textures.rbz como faltante: {:?}",
            report.missing_items
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ngp75_dll_is_explicitly_rejected_with_clear_74_requirement() {
        let rsf_75 = Path::new(r"C:\Richard Burns Rally\Plugins\NGP.dll");
        if !rsf_75.is_file() {
            return;
        }
        let tmp = std::env::temp_dir().join(format!("rerbr-ngp75-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugins = tmp.join("RBR").join("Plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::copy(rsf_75, plugins.join("NGP.dll")).unwrap();

        let sel = Selection::default();
        let catalog = Catalog::scan_root_for_app(&tmp, &sel, true);
        let report = ResourceHealthReport::inspect(Some(&tmp), &catalog, true);
        assert!(!report.ngp74_ready);
        assert!(report.ngp_status_label.contains("NGP 7.5"));
        assert!(report.missing_items.iter().any(|m| m.contains("7.4.778")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn android_saf_ui_exposes_import_banners_buttons_and_fits_vertex_budget() {
        let tmp =
            std::env::temp_dir().join(format!("rerbr-android-ui-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let sel = Selection::default();
        let catalog = Catalog::scan_root_for_app(&tmp, &sel, true);
        let health = ResourceHealthReport::inspect(Some(&tmp), &catalog, true);
        let import_view = AndroidImportView {
            saf: SafRuntimeSnapshot {
                phase: SafPhase::Idle,
                has_persisted_uri: true,
                persisted_label: "primary:Download/RBR".into(),
                ..Default::default()
            },
            health,
        };

        let mut app = crate::driving::Runtime::new();
        app.content = catalog;
        app.android_import = Some(import_view);
        app.controls.touch_steering = true;

        // 1. Main Menu shows clickable SAF import banner when resources are missing
        app.apply_command(menu::Command::EnterMainMenu);
        assert_eq!(app.menu.page, menu::Page::Main);
        {
            let ctx = app.context();
            assert!(menu::Menu::show_main_android_import_banner(&ctx));
            let banner = menu::Menu::main_android_import_rect();
            let rows = app.menu.rows(&ctx);
            assert_eq!(
                app.menu.press_with_ctx(
                    f64::from(banner.center_x()),
                    f64::from(banner.y + banner.h * 0.5),
                    &ctx,
                    &rows
                ),
                Some(menu::Command::Open(menu::Page::ContentImport))
            );
            let mut canvas = Canvas::new((2400, 1080));
            app.menu.draw(&mut canvas, &ctx, "SIN RECURSOS");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
        }

        // 2. Empty Garage shows clickable SAF import button
        app.apply_command(menu::Command::Open(menu::Page::Garage));
        {
            let ctx = app.context();
            let mut canvas = Canvas::new((2400, 1080));
            app.menu.draw(&mut canvas, &ctx, "SIN COCHES");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
        }

        // 3. ContentImport exposes folder picker, zip picker, and persisted folder re-import
        app.apply_command(menu::Command::Open(menu::Page::ContentImport));
        {
            let ctx = app.context();
            let rows = app.menu.rows(&ctx);
            assert!(rows.iter().any(|r| matches!(
                r.kind,
                menu::RowKind::Action {
                    command: menu::Command::ImportAndroidFolder,
                    enabled: true
                }
            )));
            assert!(rows.iter().any(|r| matches!(
                r.kind,
                menu::RowKind::Action {
                    command: menu::Command::ImportAndroidZip,
                    enabled: true
                }
            )));
            assert!(rows.iter().any(|r| matches!(
                r.kind,
                menu::RowKind::Action {
                    command: menu::Command::ReimportAndroidFolder,
                    enabled: true
                }
            )));
            let mut canvas = Canvas::new((2400, 1080));
            app.menu.draw(&mut canvas, &ctx, "SAF LISTO");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
        }

        // 4. Controls page exposes Touch Steering toggle on Android
        app.apply_command(menu::Command::Open(menu::Page::Controls));
        {
            let ctx = app.context();
            let rows = app.menu.rows(&ctx);
            assert!(rows.iter().any(|r| matches!(
                r.kind,
                menu::RowKind::Action {
                    command: menu::Command::ToggleTouchSteering,
                    enabled: true
                }
            )));
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Prueba integral de extremo a extremo sobre el paquete mínimo real (63 archivos, ~95.7 MiB):
    /// 1. Catálogo `public-beta` y `ResourceHealthReport` (0 archivos faltantes, sin leer C:\Richard Burns Rally).
    /// 2. Carga 3D real del vehículo (`FORD_FOCUS_2006/focus06.sgc`, exterior + cockpit + 15 DDS) y tramo
    ///    (`Harwood Forest` `track-20_O` + `track-20_N_textures.rbz`).
    /// 3. Validación Vulkan/wgpu de texturas, geometría, shaders WGSL y renderizado con y sin compresión BC.
    /// 4. Arranque de físicas NGP 7.4.778.507 a 720 Hz, encendido de motor (`Control::aux = 1`),
    ///    aceleración táctil, dirección táctil (`< IZQ` / `DER >`), sensor de inclinación (`tilt`) y frenado.
    /// 5. Decodificación del banco FMOD (`Focus_Mk2_WRC.bank`) y mezcla estéreo PCM a 44.1 kHz (`rerbr_audio_render`).
    #[test]
    fn minimal_pack_one_car_one_stage_boots_starts_engine_drives_steers_and_renders_audio() {
        let pack_root = std::env::var_os("RERBR_MINIMAL_PACK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    r"C:\Users\adria\.gemini\antigravity\scratch\rbr-android-onecar-onestage",
                )
            });
        let rbr_dir = pack_root.join("RBR");
        if !rbr_dir.join("Plugins").join("NGP.dll").is_file() {
            eprintln!(
                "SKIP minimal_pack_one_car_one_stage: paquete minimo no presente en {}",
                pack_root.display()
            );
            return;
        }

        // 1. Verificar catálogo public-beta y salud de recursos exclusivamente sobre el paquete mínimo
        let default_sel = Selection::default();
        let initial_cat = Catalog::scan_root_for_app(&pack_root, &default_sel, true);
        let focus_entry = initial_cat
            .vehicle("ford-focus-rs-wrc-2006")
            .expect("Ford Focus RS WRC 2006 debe estar listo y conducible en el paquete mínimo");
        assert_eq!(focus_entry.capability(true), DriveCapability::Drivable);
        assert!(focus_entry.files.missing.is_empty());

        let harwood_entry = initial_cat
            .stage("harwood-forest")
            .expect("Harwood Forest en catálogo");
        assert!(
            harwood_entry.files.checked && harwood_entry.files.missing.is_empty(),
            "Harwood Forest no debe tener archivos faltantes: {:?}",
            harwood_entry.files.missing
        );

        let selection = Selection {
            vehicle: focus_entry.descriptor.clone(),
            stage: harwood_entry.descriptor.clone(),
        };
        let catalog = Catalog::scan_root_for_app(&pack_root, &selection, true);
        let health = ResourceHealthReport::inspect(Some(&pack_root), &catalog, true);
        assert!(
            health.ready_to_drive,
            "Health report debe marcar ready_to_drive: {health:?}"
        );
        assert!(health.ngp74_ready);
        assert_eq!(health.drivable_vehicles, 1);
        assert_eq!(health.ready_stages, 1);
        assert!(health.missing_items.is_empty());

        // 2. Verificar atlas de iconos visuales y copiloto (.dls) desde el paquete mínimo
        let atlas = PacenoteIconAtlas::load_from_documents(&pack_root)
            .expect("Atlas PC_Generic_1280.bnk + DDS debe cargar desde el paquete mínimo");
        assert!(atlas.width > 0 && atlas.height > 0 && !atlas.rgba8.is_empty());
        let maps_dir = pack_root.join(&selection.stage.documents_maps);
        let codriver =
            CoDriverSession::load_for_stage(&pack_root, &maps_dir, &selection.stage.stem);
        match codriver.status {
            crate::driving::codriver::VoiceBankStatus::Missing { note_count }
            | crate::driving::codriver::VoiceBankStatus::Ready { note_count, .. }
            | crate::driving::codriver::VoiceBankStatus::RequiresWavConversion {
                note_count, ..
            } => {
                assert!(
                    note_count > 20,
                    "track-20_O.dls debe contener notas reales del tramo (leídas {note_count})"
                );
            }
        }

        // 3. Cargar geometría 3D real del coche + cockpit + tramo + texturas y validar en wgpu (con y sin BC)
        for features in [
            wgpu::Features::empty(),
            wgpu::Features::TEXTURE_COMPRESSION_BC,
        ] {
            let prepared =
                crate::car::prepare_selected_for_mode(&rbr_dir, &maps_dir, &selection, true)
                    .expect("Carga 3D de Focus 2006 + Harwood Forest desde paquete mínimo");
            assert!(
                prepared.car.cockpit.is_some(),
                "Cockpit e i_steeringwheel deben estar listos: {:?}",
                prepared.car.cockpit_error
            );
            assert!(!prepared.car.textures.is_empty());
            assert!(!prepared.terrain.textures.is_empty());

            let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor {
                required_features: features,
                ..Default::default()
            });
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let (_uploaded, terrain_loaded) =
                crate::car::runtime::Runtime::upload_prepared(prepared, &device, &queue, format)
                    .expect("Subida GPU wgpu de coche + tramo debe tener éxito");
            assert!(terrain_loaded.total_blocks > 0);
            queue.submit([]);
            let gpu_err = futures_executor::block_on(scope.pop());
            assert!(
                gpu_err.is_none(),
                "Cero errores de validación wgpu/Vulkan ({features:?}): {gpu_err:?}"
            );
        }

        // 4. Arrancar físicas NGP 7.4.778.507 a 720 Hz exclusivamente desde el paquete mínimo (pila STACK_BYTES)
        let pack_root_sim = pack_root.clone();
        let rbr_dir_sim = rbr_dir.clone();
        let sel_sim = selection.clone();
        std::thread::Builder::new()
            .name("test-ngp74-onecar-onestage".into())
            .stack_size(rerbr_sim::STACK_BYTES)
            .spawn(move || {
                let sim_paths = rerbr_sim::inputs::Paths::for_mode(&pack_root_sim, &sel_sim, true)
                    .expect("Paths::for_mode sobre paquete mínimo");
                let pack_path = rbr_dir_sim
                    .join("rsfdata")
                    .join("cars")
                    .join("Ford_Focus_Mk_II_RS_WRC_2006_ngp6");
                let mut session =
                    rerbr_sim::drive::Session::boot_experimental_pack_with_overrides(
                        &sim_paths,
                        &sel_sim,
                        &pack_path,
                        false,
                        true,
                        rerbr_sim::drive::BootOverrides::default(),
                    )
                    .expect("Arranque NGP 7.4.778.507 sobre paquete mínimo");

                let rest = session.sample();
                assert!(rest.position.iter().all(|v| v.is_finite()));
                assert!(rest.wheel_ground.iter().all(|&g| g != 0));
                assert!(
                    rest.rpm > 800.0,
                    "Motor en ralentí al asentar coche en salida: rpm={}",
                    rest.rpm
                );

                // Verificar botón táctil de encendido (engine_start_rect -> Control.aux = 1)
                let layout = DrivingLayout::default();
                let mut controls = Controls::new();
                controls.touch_steering = true;
                controls
                    .configure_vehicle(&sel_sim.vehicle)
                    .expect("configurar Focus 6 marchas");
                let start_btn = layout
                    .engine_start_rect()
                    .expect("botón START en barra superior");
                controls.touch_layout(
                    &layout,
                    10,
                    f64::from(start_btn.center_x()),
                    f64::from(start_btn.y + start_btn.h * 0.5),
                    true,
                    false,
                );
                let starter_ctrl = controls.input_in(&layout, f64::NAN, 1.0 / 60.0);
                assert_eq!(
                    starter_ctrl.aux, 1,
                    "Botón de arranque activa bit aux=1 de NGP"
                );
                for _ in 0..30 {
                    session.step(starter_ctrl).expect("step starter");
                    assert!(session.sample().rpm > 800.0);
                }
                controls.touch_layout(&layout, 10, 0.0, 0.0, false, true);

                // 5. Subir a 1ª marcha (gear=2 en índice RBR: 0=R, 1=N, 2=1ª) y ACELERAR con pedal táctil
                let gup = layout.gear_up;
                controls.touch_layout(
                    &layout,
                    11,
                    f64::from(gup.center_x()),
                    f64::from(gup.y + gup.h * 0.5),
                    true,
                    false,
                );
                controls.touch_layout(&layout, 11, 0.0, 0.0, false, true);
                assert_eq!(controls.selected_gear, 2, "Seleccionada 1ª marcha");

                controls.touch_layout(
                    &layout,
                    1,
                    f64::from(layout.throttle.center_x()),
                    f64::from(layout.throttle.bottom() - 0.01),
                    true,
                    false,
                );

                let mut peak_speed_mps = 0.0f64;
                let mut peak_rpm = 0.0f32;
                let mut last_sample = rest;
                // Incluye la cuenta atrás 3-2-1-GO (~2s) y 2.5s de aceleración real en 1ª/2ª marcha (3240 pasos a 720 Hz)
                for step_idx in 0..3240 {
                    if step_idx == 2300 {
                        controls.touch_layout(
                            &layout,
                            12,
                            f64::from(gup.center_x()),
                            f64::from(gup.y + gup.h * 0.5),
                            true,
                            false,
                        );
                        controls.touch_layout(&layout, 12, 0.0, 0.0, false, true);
                    }
                    let raw_input = controls.input_in(&layout, f64::NAN, 1.0 / 720.0);
                    let input = crate::driving::controls::pending_selection(
                        raw_input,
                        last_sample.commanded_gear,
                    );
                    session.step(input).expect("step throttle");
                    last_sample = session.sample();
                    peak_speed_mps = peak_speed_mps.max(last_sample.speed());
                    peak_rpm = peak_rpm.max(last_sample.rpm);
                }
                assert!(
                    peak_speed_mps > 12.0,
                    "El coche debe acelerar con fuerza en el tramo (peak_speed_mps={peak_speed_mps:.2}, gear={}, rpm={:.0})",
                    last_sample.gear,
                    peak_rpm
                );

                // 6. GIRAR a izquierda y derecha mediante botones táctiles (< IZQ / DER >) y sensor tilt
                let sl = layout.steer_left_rect();
                controls.touch_layout(
                    &layout,
                    2,
                    f64::from(sl.center_x()),
                    f64::from(sl.y + sl.h * 0.5),
                    true,
                    false,
                );
                for _ in 0..180 {
                    let inp = crate::driving::controls::pending_selection(
                        controls.input_in(&layout, f64::NAN, 1.0 / 720.0),
                        last_sample.commanded_gear,
                    );
                    session.step(inp).expect("step left steer");
                    last_sample = session.sample();
                }
                let left_steer_angle = last_sample.steering_angle;
                assert!(
                    left_steer_angle.abs() > 0.05 && last_sample.steering_received < -0.5,
                    "Dirección táctil izquierda debe girar las ruedas: angle={left_steer_angle}, rx={}",
                    last_sample.steering_received
                );
                controls.touch_layout(&layout, 2, 0.0, 0.0, false, true);

                // Girar a la derecha usando inclinación del sensor (tilt = +0.35 rad)
                for _ in 0..180 {
                    let inp = crate::driving::controls::pending_selection(
                        controls.input_in(&layout, 0.35, 1.0 / 720.0),
                        last_sample.commanded_gear,
                    );
                    session.step(inp).expect("step tilt steer");
                    last_sample = session.sample();
                }
                assert!(
                    last_sample.steering_received > 0.4
                        && last_sample.steering_angle.signum() != left_steer_angle.signum(),
                    "Sensor de inclinación debe invertir el giro hacia la derecha: prev={left_steer_angle}, now={}, rx={}",
                    last_sample.steering_angle,
                    last_sample.steering_received
                );

                // 7. FRENAR a fondo soltando acelerador y pulsando BRAKE
                let speed_before_brake = last_sample.speed();
                controls.touch_layout(&layout, 1, 0.0, 0.0, false, true);
                controls.touch_layout(
                    &layout,
                    3,
                    f64::from(layout.brake.center_x()),
                    f64::from(layout.brake.bottom() - 0.01),
                    true,
                    false,
                );
                for _ in 0..1080 {
                    let inp = crate::driving::controls::pending_selection(
                        controls.input_in(&layout, 0.0, 1.0 / 720.0),
                        last_sample.commanded_gear,
                    );
                    session.step(inp).expect("step brake");
                    last_sample = session.sample();
                }
                assert!(
                    last_sample.speed() < speed_before_brake * 0.60,
                    "El freno táctil debe desacelerar contundentemente el coche en grava: antes={speed_before_brake:.2} m/s, después={:.2} m/s",
                    last_sample.speed()
                );
            })
            .expect("spawn ngp74 test thread")
            .join()
            .expect("ngp74 test thread succeeded");

        // 8. Inicializar FMOD desde el paquete mínimo y renderizar audio estéreo 44.1 kHz (mismo camino JNI GameAudio)
        let fmod_status = audio::init_rsf_fmod(&pack_root, &selection.vehicle, 44100);
        assert!(
            fmod_status.contains("44/44"),
            "Focus_Mk2_WRC.bank debe decodificar las 44 muestras Vorbis: {fmod_status}"
        );
        audio::set_master_volume(0.9);
        audio::set_menu_music_active(false);
        audio::set_paused(false);
        audio::update_fmod_telemetry(5800.0, 0.85, 0.0, 18.0, 3, 18.5);
        audio::update_surface_state(audio::SurfaceKind::Gravel, 18.0, 0.25, 4);

        let mut pcm = vec![0.0f32; 2048 * 2];
        unsafe {
            audio::rerbr_audio_render(pcm.as_mut_ptr(), 2048, 44100);
        }
        audio::set_paused(true);
        assert!(pcm.iter().all(|s| s.is_finite()));
        let peak_amp = pcm.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak_amp > 0.01,
            "El mezclador de audio Android/FMOD debe producir señal audible no nula: peak={peak_amp}"
        );
    }
}
