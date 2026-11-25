//! Capa compartida en Rust para importación universal incremental (carpetas y paquetes `.zip`),
//! normalización de rutas sensible a mayúsculas/minúsculas (iOS APFS / Android ext4),
//! protección de runtime certificado NGP 7.4.778 y administración de almacenamiento.
//!
//! - Reutilizada por el adaptador nativo de iOS (`UIDocumentPickerViewController` + URLs de seguridad)
//!   y por el adaptador Android SAF (`SafResourceImporter` / JNI).
//! - Nunca incluye ni genera activos comerciales: solo filtra, valida y organiza archivos legítimos
//!   proporcionados por el usuario hacia `<Documents>/RBR`.

use super::content_manager::{StageEntry, VehicleEntry};
use rerbr_asset_formats::zip::ZipArchive;
use rerbr_sim::rbr_formats::{cars, ngp};
use rerbr_sim::sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

/// Umbral para comparar contenido exacto (SHA-256) además del tamaño en archivos de configuración/binarios.
const SMALL_FILE_HASH_THRESHOLD: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportProgress {
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub source_label: String,
    pub files_copied: u32,
    pub files_reused: u32,
    pub files_protected: u32,
    pub files_ignored: u32,
    pub bytes_copied: u64,
    pub bytes_reused: u64,
    pub notes: Vec<String>,
}

impl ImportSummary {
    pub fn total_relevant_files(&self) -> u32 {
        self.files_copied + self.files_reused + self.files_protected
    }

    pub fn total_relevant_bytes(&self) -> u64 {
        self.bytes_copied + self.bytes_reused
    }

    pub fn headline_es(&self) -> String {
        let total = self.total_relevant_files();
        let mib = self.total_relevant_bytes() as f64 / (1024.0 * 1024.0);
        if self.files_protected > 0 {
            format!(
                "IMPORTACIÓN COMPLETADA: {} NUEVOS · {} REUTILIZADOS · {} PROTEGIDOS ({:.1} MiB)",
                self.files_copied, self.files_reused, self.files_protected, mib
            )
        } else {
            format!(
                "IMPORTACIÓN COMPLETADA: {total} ARCHIVOS ({} NUEVOS · {} REUTILIZADOS · {:.1} MiB)",
                self.files_copied, self.files_reused, mib
            )
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageBreakdown {
    pub total_bytes: u64,
    pub total_files: u32,
    pub vehicles_bytes: u64,
    pub stages_bytes: u64,
    pub audio_bytes: u64,
    pub shared_core_bytes: u64,
    pub temp_part_bytes: u64,
    pub temp_part_files: u32,
}

impl StorageBreakdown {
    pub fn scan_env() -> Self {
        let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from);
        match doc.as_deref() {
            Some(d) => Self::scan_documents(d),
            None => Self::default(),
        }
    }

    pub fn scan_documents(documents_dir: &Path) -> Self {
        let rbr = documents_dir.join("RBR");
        if !rbr.is_dir() {
            return Self::default();
        }
        let mut out = Self::default();
        Self::walk_dir(&rbr, &rbr, &mut out);
        out
    }

    fn walk_dir(root: &Path, dir: &Path, out: &mut Self) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                Self::walk_dir(root, &path, out);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".part") {
                out.temp_part_files += 1;
                out.temp_part_bytes += size;
                out.total_bytes += size;
                continue;
            }
            // Ignorar archivos de progreso/récords del usuario en el cómputo de recursos comerciales
            if matches!(
                name_str.as_ref(),
                "personal_bests.json" | "leaderboards.json" | "vehicle_cameras.json"
            ) || name_str.starts_with("career")
            {
                continue;
            }
            out.total_files += 1;
            out.total_bytes += size;

            let rel = path
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let low = rel.to_ascii_lowercase();
            if low.starts_with("cars/") && low != "cars/cars.ini"
                || low.starts_with("rsfdata/cars/")
            {
                out.vehicles_bytes += size;
            } else if low.starts_with("maps/") && low != "maps/texturefilenamemap.dat" {
                out.stages_bytes += size;
            } else if low.starts_with("audiofmod/") || low.starts_with("audio/") {
                out.audio_bytes += size;
            } else {
                out.shared_core_bytes += size;
            }
        }
    }

    pub fn summary_line(&self) -> String {
        format!(
            "TOTAL {} ({} ARCHIVOS) · COCHES {} · TRAMOS {} · AUDIO {} · MOTOR {}",
            format_bytes(self.total_bytes),
            self.total_files,
            format_bytes(self.vehicles_bytes),
            format_bytes(self.stages_bytes),
            format_bytes(self.audio_bytes),
            format_bytes(self.shared_core_bytes),
        )
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Normaliza una ruta relativa arbitraria (de carpeta o entrada `.zip`) a su ruta canónica bajo `RBR/`.
/// Devuelve `None` si la ruta es insegura (`..`, unidades, ocultos macOS) o no es un recurso requerido por el motor.
pub fn normalize_rbr_rel_path(raw_path: &str) -> Option<String> {
    let unified = raw_path.replace('\\', "/");
    let trimmed = unified
        .trim()
        .trim_start_matches("./")
        .trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains('\0') || trimmed.contains(':') {
        return None;
    }

    let raw_parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if raw_parts.is_empty() {
        return None;
    }
    for &seg in &raw_parts {
        if seg == "."
            || seg == ".."
            || seg.eq_ignore_ascii_case("__MACOSX")
            || seg.eq_ignore_ascii_case(".DS_Store")
            || seg.eq_ignore_ascii_case("Thumbs.db")
            || seg.starts_with("._")
        {
            return None;
        }
    }

    // Si hay algún segmento "RBR" (p. ej. "MiPack/RBR/Cars/..." o "RBR/Maps/..."), recortar hasta él.
    let mut parts: &[&str] = &raw_parts;
    if let Some(rbr_idx) = parts.iter().position(|s| s.eq_ignore_ascii_case("RBR")) {
        parts = &parts[rbr_idx + 1..];
        if parts.is_empty() {
            return None;
        }
    } else if parts.len() >= 2 && !is_recognized_top_segment(parts[0]) {
        // Soportar contenedor exterior único en ZIPs (ej. "RBR_NGP-7.4.778.507/Plugins/NGP.dll" o "MinimalPack/Cars/...").
        if is_recognized_top_segment(parts[1]) {
            parts = &parts[1..];
        } else if parts.len() == 2 && has_ext_ci(parts[1], &["sgc", "ini", "dds"]) {
            // ZIP de un solo modelo 3D sin carpeta `Cars/` exterior: `<Modelo>/<coche>.sgc`
            return normalize_car_subpath(parts[0], &parts[1..]);
        } else if parts.len() == 3
            && parts[1].eq_ignore_ascii_case("textures")
            && has_ext_ci(parts[2], &["dds"])
        {
            return normalize_car_subpath(parts[0], &parts[1..]);
        }
    }

    let head = parts[0];
    if parts.len() == 1 {
        if head.eq_ignore_ascii_case("NGP.dll") {
            return Some("Plugins/NGP.dll".into());
        }
        if head.eq_ignore_ascii_case("physics.rbz") {
            return Some("physics.rbz".into());
        }
        if head.eq_ignore_ascii_case("misc.rbz") {
            return Some("misc.rbz".into());
        }
        if head.eq_ignore_ascii_case("Scripts.rbz") {
            return Some("Scripts.rbz".into());
        }
        if head.eq_ignore_ascii_case("Strings.rbz") {
            return Some("Strings.rbz".into());
        }
        if head.eq_ignore_ascii_case("season.dat") {
            return Some("season.dat".into());
        }
        if head.eq_ignore_ascii_case("TextureFilenameMap.dat") {
            return Some("Maps/TextureFilenameMap.dat".into());
        }
        if is_stage_asset_filename(head) && head.to_ascii_lowercase().starts_with("track-") {
            return Some(format!("Maps/{head}"));
        }
        if has_ext_ci(head, &["bank"]) {
            return Some(format!("AudioFMOD/{head}"));
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Plugins") {
        if parts.len() == 2 && parts[1].eq_ignore_ascii_case("NGP.dll") {
            return Some("Plugins/NGP.dll".into());
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Cars") {
        if parts.len() == 2 && parts[1].eq_ignore_ascii_case("Cars.ini") {
            return Some("Cars/Cars.ini".into());
        }
        if parts.len() >= 3 {
            return normalize_car_subpath(parts[1], &parts[2..]);
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Harwood")
        && parts.len() == 3
        && parts[1].eq_ignore_ascii_case("Maps")
        && is_stage_asset_filename(parts[2])
    {
        if parts[2].eq_ignore_ascii_case("TextureFilenameMap.dat") {
            return Some("Maps/TextureFilenameMap.dat".into());
        }
        return Some(format!("Maps/{}", parts[2]));
    }

    if head.eq_ignore_ascii_case("Maps") {
        if parts.len() == 2 && is_stage_asset_filename(parts[1]) {
            if parts[1].eq_ignore_ascii_case("TextureFilenameMap.dat") {
                return Some("Maps/TextureFilenameMap.dat".into());
            }
            return Some(format!("Maps/{}", parts[1]));
        }
        return None;
    }

    if head.eq_ignore_ascii_case("rsfdata") && parts.len() >= 3 {
        let sub = parts[1];
        if sub.eq_ignore_ascii_case("cache") && parts.len() == 3 {
            let f = parts[2];
            for canon in [
                "cars.json",
                "carmodels.json",
                "cargroups.json",
                "audiobank.ini",
            ] {
                if f.eq_ignore_ascii_case(canon) {
                    return Some(format!("rsfdata/cache/{canon}"));
                }
            }
            return None;
        }
        if sub.eq_ignore_ascii_case("cars") && parts.len() <= 6 {
            let fname = *parts.last()?;
            if has_ext_ci(fname, &["lsp", "ini", "json", "txt", "zip", "7z", "rbz"]) {
                let rest = parts[2..].join("/");
                return Some(format!("rsfdata/cars/{rest}"));
            }
        }
        return None;
    }

    if head.eq_ignore_ascii_case("AudioFMOD") {
        if parts.len() == 2 && has_ext_ci(parts[1], &["bank"]) {
            return Some(format!("AudioFMOD/{}", parts[1]));
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Audio") && parts.len() >= 3 {
        let sub = parts[1];
        let fname = *parts.last()?;
        if sub.eq_ignore_ascii_case("Cars") && parts.len() <= 4 {
            if parts.len() == 3 && fname.eq_ignore_ascii_case("Cars.ini") {
                return Some("Audio/Cars/Cars.ini".into());
            }
            if has_ext_ci(fname, &["eng", "ini", "wav", "ogg"]) {
                return Some(format!("Audio/Cars/{}", parts[2..].join("/")));
            }
        } else if sub.eq_ignore_ascii_case("Speech") && parts.len() <= 5 {
            if has_ext_ci(fname, &["wav", "ogg", "ini"]) {
                return Some(format!("Audio/Speech/{}", parts[2..].join("/")));
            }
        } else if sub.eq_ignore_ascii_case("Music")
            && parts.len() == 3
            && has_ext_ci(fname, &["ogg", "wav"])
        {
            return Some(format!("Audio/Music/{fname}"));
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Sgs") {
        if parts.len() == 2 {
            let low = parts[1].to_ascii_lowercase();
            if low.contains("spectator_") && low.ends_with(".sgs") {
                return Some(format!("Sgs/{}", parts[1]));
            }
        }
        return None;
    }

    if head.eq_ignore_ascii_case("Generic") {
        if parts.len() == 2 {
            let low = parts[1].to_ascii_lowercase();
            let is_atlas = (low.starts_with("pc_generic_") && low.ends_with(".bnk"))
                || (low.starts_with("pc_")
                    && (low.ends_with("_01.dds") || low.ends_with("_05.dds")));
            if is_atlas {
                return Some(format!("Generic/{}", parts[1]));
            }
        }
        return None;
    }

    None
}

fn normalize_car_subpath(car_folder: &str, tail: &[&str]) -> Option<String> {
    if car_folder.is_empty() {
        return None;
    }
    if tail.len() == 1 && has_ext_ci(tail[0], &["sgc", "ini", "dds"]) {
        return Some(format!("Cars/{car_folder}/{}", tail[0]));
    }
    if tail.len() == 2 && tail[0].eq_ignore_ascii_case("textures") && has_ext_ci(tail[1], &["dds"])
    {
        return Some(format!("Cars/{car_folder}/textures/{}", tail[1]));
    }
    None
}

fn is_recognized_top_segment(seg: &str) -> bool {
    seg.eq_ignore_ascii_case("Plugins")
        || seg.eq_ignore_ascii_case("Cars")
        || seg.eq_ignore_ascii_case("Maps")
        || seg.eq_ignore_ascii_case("Harwood")
        || seg.eq_ignore_ascii_case("rsfdata")
        || seg.eq_ignore_ascii_case("AudioFMOD")
        || seg.eq_ignore_ascii_case("Audio")
        || seg.eq_ignore_ascii_case("Sgs")
        || seg.eq_ignore_ascii_case("Generic")
        || seg.eq_ignore_ascii_case("NGP.dll")
        || seg.eq_ignore_ascii_case("physics.rbz")
        || seg.eq_ignore_ascii_case("misc.rbz")
        || seg.eq_ignore_ascii_case("Scripts.rbz")
        || seg.eq_ignore_ascii_case("Strings.rbz")
        || seg.eq_ignore_ascii_case("season.dat")
}

pub fn is_stage_asset_filename(name: &str) -> bool {
    if name.eq_ignore_ascii_case("TextureFilenameMap.dat") {
        return true;
    }
    let low = name.to_ascii_lowercase();
    low.ends_with("_textures.rbz")
        || has_ext_ci(name, &["lbs", "col", "trk", "mat", "dls", "fnc", "ini"])
}

fn has_ext_ci(name: &str, exts: &[&str]) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    exts.iter().any(|cand| ext.eq_ignore_ascii_case(cand))
}

/// Resuelve la ruta de destino bajo `<Documents>/RBR` reutilizando cualquier directorio/archivo
/// existente que difiera solo en mayúsculas/minúsculas (crítico en APFS de iOS y ext4 de Android).
pub fn resolve_case_preserving_dest(rbr_root: &Path, canonical_rel: &str) -> PathBuf {
    let mut cur = rbr_root.to_path_buf();
    for seg in canonical_rel.split('/').filter(|s| !s.is_empty()) {
        if let Ok(entries) = fs::read_dir(&cur) {
            if let Some(existing) = entries
                .flatten()
                .map(|e| e.file_name())
                .find(|n| n.to_str().is_some_and(|s| s.eq_ignore_ascii_case(seg)))
            {
                cur.push(existing);
                continue;
            }
        }
        cur.push(seg);
    }
    cur
}

/// Comprueba si un archivo nuclear ya instalado (`Plugins/NGP.dll` o `physics.rbz`) pertenece al
/// runtime certificado NGP 7.4.778.507 y NO debe ser sobrescrito por una versión incompatible (p. ej. 7.5.779).
pub fn should_preserve_existing_core_file(
    rbr_root: &Path,
    canonical_rel: &str,
    incoming_path: &Path,
) -> Option<String> {
    if canonical_rel.eq_ignore_ascii_case("Plugins/NGP.dll") {
        let existing_path = cars::resolve(rbr_root, "Plugins/NGP.dll")?;
        let existing_bytes = fs::read(existing_path).ok()?;
        if ngp::validate(&existing_bytes).is_ok() {
            let incoming_ok = fs::read(incoming_path)
                .map(|b| ngp::validate(&b).is_ok())
                .unwrap_or(false);
            if !incoming_ok {
                return Some(
                    "CONSERVADO Plugins/NGP.dll 7.4.778 CERTIFICADO (SE IGNORÓ NGP.dll INCOMPATIBLE)"
                        .into(),
                );
            }
        }
    } else if canonical_rel.eq_ignore_ascii_case("physics.rbz") {
        let existing_path = cars::resolve(rbr_root, "physics.rbz")?;
        let existing_rev = ngp::compat::physics_revision(&existing_path).ok().flatten();
        if existing_rev == Some(778) {
            let incoming_rev = ngp::compat::physics_revision(incoming_path).ok().flatten();
            if incoming_rev != Some(778) {
                return Some(
                    "CONSERVADO physics.rbz REV 778 CERTIFICADO (SE IGNORÓ REVISIÓN DISTINTA)"
                        .into(),
                );
            }
        }
    }
    None
}

fn files_identical(existing: &Path, incoming_size: u64, incoming_sha256: Option<[u8; 32]>) -> bool {
    let Ok(meta) = fs::metadata(existing) else {
        return false;
    };
    if !meta.is_file() || meta.len() != incoming_size {
        return false;
    }
    if let Some(inc_hash) = incoming_sha256 {
        let Ok(ex_hash) = sha256_file(existing) else {
            return false;
        };
        return ex_hash == inc_hash;
    }
    true
}

fn sha256_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

/// Detecta qué prefijo lógico aplica al importar una carpeta seleccionada por el usuario
/// (instalación completa `RBR/`, subcarpeta `Cars/`, carpeta suelta de un modelo `<Coche>/`, `Maps/`, `AudioFMOD/`, `rsfdata/`).
fn detect_directory_import_base(selected_dir: &Path) -> Option<(PathBuf, Option<String>)> {
    if looks_like_rbr_root(selected_dir) {
        return Some((selected_dir.to_path_buf(), None));
    }
    if let Some(child_rbr) = cars::resolve(selected_dir, "RBR") {
        if child_rbr.is_dir() && looks_like_rbr_root(&child_rbr) {
            return Some((child_rbr, None));
        }
    }

    let folder_name = selected_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if folder_name.eq_ignore_ascii_case("Maps")
        || cars::resolve(selected_dir, "TextureFilenameMap.dat").is_some()
        || dir_has_ext(selected_dir, "lbs")
    {
        return Some((selected_dir.to_path_buf(), Some("Maps".into())));
    }
    if folder_name.eq_ignore_ascii_case("Cars") || cars::resolve(selected_dir, "Cars.ini").is_some()
    {
        return Some((selected_dir.to_path_buf(), Some("Cars".into())));
    }
    if folder_name.eq_ignore_ascii_case("AudioFMOD") || dir_has_ext(selected_dir, "bank") {
        return Some((selected_dir.to_path_buf(), Some("AudioFMOD".into())));
    }
    if folder_name.eq_ignore_ascii_case("rsfdata")
        || cars::resolve(selected_dir, "cache/cars.json").is_some()
    {
        return Some((selected_dir.to_path_buf(), Some("rsfdata".into())));
    }
    if dir_has_ext(selected_dir, "sgc") && !folder_name.is_empty() {
        return Some((
            selected_dir.to_path_buf(),
            Some(format!("Cars/{folder_name}")),
        ));
    }

    // Buscar un nivel adicional de contenedor (p. ej. carpeta extraída de un ZIP)
    if let Ok(entries) = fs::read_dir(selected_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if looks_like_rbr_root(&path) {
                    return Some((path, None));
                }
                if let Some(nested_rbr) = cars::resolve(&path, "RBR") {
                    if nested_rbr.is_dir() && looks_like_rbr_root(&nested_rbr) {
                        return Some((nested_rbr, None));
                    }
                }
            }
        }
    }
    None
}

fn looks_like_rbr_root(dir: &Path) -> bool {
    for marker in [
        "physics.rbz",
        "misc.rbz",
        "Scripts.rbz",
        "Strings.rbz",
        "NGP.dll",
        "Plugins/NGP.dll",
        "Cars",
        "Maps",
        "rsfdata",
        "AudioFMOD",
        "Audio",
        "Sgs",
        "Generic",
    ] {
        if cars::resolve(dir, marker).is_some() {
            return true;
        }
    }
    false
}

fn dir_has_ext(dir: &Path, ext: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_type().is_ok_and(|t| t.is_file())
            && e.file_name()
                .to_str()
                .is_some_and(|n| has_ext_ci(n, &[ext]))
    })
}

/// Importa incrementalmente una carpeta compatible hacia `<documents_dir>/RBR`.
pub fn import_directory_into_documents(
    src_dir: &Path,
    documents_dir: &Path,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary, String> {
    if !src_dir.is_dir() {
        return Err(format!(
            "LA RUTA SELECCIONADA NO ES UNA CARPETA ACCESIBLE: {}",
            src_dir.display()
        ));
    }
    let label = src_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Carpeta RBR")
        .to_string();

    progress(ImportProgress {
        files_done: 0,
        files_total: 0,
        bytes_done: 0,
        detail: format!("ANALIZANDO CARPETA: {label}"),
    });

    let Some((base_dir, virtual_prefix)) = detect_directory_import_base(src_dir) else {
        return Err(
            "CARPETA NO RECONOCIDA COMO INSTALACIÓN O PAQUETE RBR/RSF (FALTAN Cars/, Maps/, Plugins/NGP.dll O physics.rbz)"
                .into(),
        );
    };

    let dst_rbr = documents_dir.join("RBR");
    // Evitar auto-copia si el usuario seleccionó la propia carpeta Documents/RBR
    if let (Ok(src_canon), Ok(dst_canon)) = (base_dir.canonicalize(), dst_rbr.canonicalize()) {
        if src_canon == dst_canon && virtual_prefix.is_none() {
            return Err("LA CARPETA SELECCIONADA YA ES EL DIRECTORIO INTERNO RBR DE LA APP".into());
        }
    }

    let mut planned: Vec<(PathBuf, String, u64)> = Vec::new();
    let mut seen_ci: HashSet<String> = HashSet::new();
    let mut ignored_count = 0u32;
    collect_directory_plan(
        &base_dir,
        &base_dir,
        virtual_prefix.as_deref(),
        &mut planned,
        &mut seen_ci,
        &mut ignored_count,
    );

    if planned.is_empty() {
        return Err(
            "NO SE ENCONTRARON ARCHIVOS COMPATIBLES DE VEHÍCULOS, TRAMOS, FÍSICAS NI AUDIO EN LA CARPETA"
                .into(),
        );
    }

    fs::create_dir_all(&dst_rbr)
        .map_err(|e| format!("NO SE PUDO CREAR DIRECTORIO INTERNO RBR: {e}"))?;

    let mut summary = ImportSummary {
        source_label: label,
        files_ignored: ignored_count,
        ..ImportSummary::default()
    };
    let total = planned.len() as u32;
    let mut buf = vec![0u8; 128 * 1024];

    for (idx, (src_file, canon_rel, src_size)) in planned.into_iter().enumerate() {
        let done_idx = idx as u32;
        if let Some(protect_note) =
            should_preserve_existing_core_file(&dst_rbr, &canon_rel, &src_file)
        {
            summary.files_protected += 1;
            if !summary.notes.contains(&protect_note) {
                summary.notes.push(protect_note.clone());
            }
            progress(ImportProgress {
                files_done: done_idx + 1,
                files_total: total,
                bytes_done: summary.total_relevant_bytes(),
                detail: protect_note,
            });
            continue;
        }

        let dst_file = resolve_case_preserving_dest(&dst_rbr, &canon_rel);
        let small_hash = if src_size <= SMALL_FILE_HASH_THRESHOLD && dst_file.is_file() {
            sha256_file(&src_file).ok()
        } else {
            None
        };
        if files_identical(&dst_file, src_size, small_hash) {
            summary.files_reused += 1;
            summary.bytes_reused += src_size;
            progress(ImportProgress {
                files_done: done_idx + 1,
                files_total: total,
                bytes_done: summary.total_relevant_bytes(),
                detail: format!("REUTILIZADO (SIN DUPLICAR): {canon_rel}"),
            });
            continue;
        }

        progress(ImportProgress {
            files_done: done_idx,
            files_total: total,
            bytes_done: summary.total_relevant_bytes(),
            detail: format!("IMPORTANDO: {canon_rel}"),
        });

        let written = copy_file_atomically(&src_file, &dst_file, &mut buf)
            .map_err(|e| format!("ERROR AL IMPORTAR {canon_rel}: {e}"))?;
        summary.files_copied += 1;
        summary.bytes_copied += written;

        progress(ImportProgress {
            files_done: done_idx + 1,
            files_total: total,
            bytes_done: summary.total_relevant_bytes(),
            detail: format!("INSTALADO: {canon_rel}"),
        });
    }

    Ok(summary)
}

fn collect_directory_plan(
    base_dir: &Path,
    cur_dir: &Path,
    virtual_prefix: Option<&str>,
    planned: &mut Vec<(PathBuf, String, u64)>,
    seen_ci: &mut HashSet<String>,
    ignored_count: &mut u32,
) {
    let Ok(entries) = fs::read_dir(cur_dir) else {
        return;
    };
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());

    for entry in items {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            let dir_name = entry.file_name();
            let dir_str = dir_name.to_string_lossy();
            // Podar enseguida carpetas masivas irrelevantes de una instalación PC de RBR/RSF
            if cur_dir == base_dir
                && virtual_prefix.is_none()
                && (dir_str.eq_ignore_ascii_case("Replays")
                    || dir_str.eq_ignore_ascii_case("Replay")
                    || dir_str.eq_ignore_ascii_case("Screenshots")
                    || dir_str.eq_ignore_ascii_case("SavedGames")
                    || dir_str.eq_ignore_ascii_case("RichardBurnsRally_SSE")
                    || dir_str.eq_ignore_ascii_case("Support"))
            {
                continue;
            }
            collect_directory_plan(
                base_dir,
                &path,
                virtual_prefix,
                planned,
                seen_ci,
                ignored_count,
            );
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(base_dir) else {
            continue;
        };
        let rel_slash = rel.to_string_lossy().replace('\\', "/");
        let logical = match virtual_prefix {
            Some(pref) => format!("{pref}/{rel_slash}"),
            None => rel_slash,
        };
        if let Some(canon) = normalize_rbr_rel_path(&logical) {
            let key = canon.to_ascii_lowercase();
            if seen_ci.insert(key) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                planned.push((path, canon, size));
            }
        } else {
            *ignored_count += 1;
        }
    }
}

/// Importa incrementalmente un archivo `.zip` desde disco hacia `<documents_dir>/RBR`.
pub fn import_zip_into_documents(
    zip_path: &Path,
    documents_dir: &Path,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary, String> {
    let label = zip_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Paquete.zip")
        .to_string();
    let file = File::open(zip_path)
        .map_err(|e| format!("NO SE PUDO ABRIR EL ARCHIVO ZIP ({label}): {e}"))?;
    import_zip_reader_into_documents(file, &label, documents_dir, progress)
}

/// Importa incrementalmente cualquier flujo `.zip` con `Read + Seek` (archivo POSIX en iOS o FD en Android).
pub fn import_zip_reader_into_documents<R: Read + Seek>(
    reader: R,
    label: &str,
    documents_dir: &Path,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary, String> {
    progress(ImportProgress {
        files_done: 0,
        files_total: 0,
        bytes_done: 0,
        detail: format!("ANALIZANDO PAQUETE ZIP: {label}"),
    });

    let mut archive = ZipArchive::new(reader)
        .map_err(|e| format!("ARCHIVO ZIP INVÁLIDO O CORRUPTO ({label}): {e}"))?;

    let mut valid_indices: Vec<(usize, String, u64)> = Vec::new();
    let mut seen_ci: HashSet<String> = HashSet::new();
    let mut ignored_count = 0u32;

    for idx in 0..archive.len() {
        let Ok(entry) = archive.by_index_raw(idx) else {
            continue;
        };
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let raw_name = String::from_utf8_lossy(entry.name_raw()).into_owned();
        let size = entry.size();
        drop(entry);
        if let Some(canon) = normalize_rbr_rel_path(&raw_name) {
            let key = canon.to_ascii_lowercase();
            if seen_ci.insert(key) {
                valid_indices.push((idx, canon, size));
            }
        } else {
            ignored_count += 1;
        }
    }

    if valid_indices.is_empty() {
        return Err(format!(
            "EL ZIP '{label}' NO CONTIENE RECURSOS RBR/RSF COMPATIBLES (Cars/, Maps/, Plugins/NGP.dll, AudioFMOD/)"
        ));
    }

    let dst_rbr = documents_dir.join("RBR");
    fs::create_dir_all(&dst_rbr)
        .map_err(|e| format!("NO SE PUDO CREAR DIRECTORIO INTERNO RBR: {e}"))?;

    let mut summary = ImportSummary {
        source_label: label.to_string(),
        files_ignored: ignored_count,
        ..ImportSummary::default()
    };
    let total = valid_indices.len() as u32;
    let mut buf = vec![0u8; 128 * 1024];

    for (pos, (entry_idx, canon_rel, uncompressed_size)) in valid_indices.into_iter().enumerate() {
        let done_idx = pos as u32;
        let dst_file = resolve_case_preserving_dest(&dst_rbr, &canon_rel);

        // Si es un archivo grande (>4 MiB) y ya coincide exactamente en bytes, evitar reextraerlo.
        if uncompressed_size > SMALL_FILE_HASH_THRESHOLD
            && files_identical(&dst_file, uncompressed_size, None)
        {
            summary.files_reused += 1;
            summary.bytes_reused += uncompressed_size;
            progress(ImportProgress {
                files_done: done_idx + 1,
                files_total: total,
                bytes_done: summary.total_relevant_bytes(),
                detail: format!("REUTILIZADO (SIN DUPLICAR): {canon_rel}"),
            });
            continue;
        }

        progress(ImportProgress {
            files_done: done_idx,
            files_total: total,
            bytes_done: summary.total_relevant_bytes(),
            detail: format!("EXTRAYENDO: {canon_rel}"),
        });

        let mut zf = archive
            .by_index(entry_idx)
            .map_err(|e| format!("ERROR DECOMPRIMIENDO {canon_rel}: {e}"))?;

        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("NO SE PUDO CREAR CARPETA PARA {canon_rel}: {e}"))?;
        }
        let part_path = temp_part_path(&dst_file);
        let (written, incoming_hash) = match stream_to_part_file(
            &mut zf,
            &part_path,
            &mut buf,
            uncompressed_size <= SMALL_FILE_HASH_THRESHOLD,
        ) {
            Ok(res) => res,
            Err(e) => {
                let _ = fs::remove_file(&part_path);
                return Err(format!("ERROR EXTRAYENDO {canon_rel}: {e}"));
            }
        };
        drop(zf);

        // Protección de NGP 7.4.778 certificado antes de confirmar el reemplazo
        if let Some(protect_note) =
            should_preserve_existing_core_file(&dst_rbr, &canon_rel, &part_path)
        {
            let _ = fs::remove_file(&part_path);
            summary.files_protected += 1;
            if !summary.notes.contains(&protect_note) {
                summary.notes.push(protect_note.clone());
            }
            progress(ImportProgress {
                files_done: done_idx + 1,
                files_total: total,
                bytes_done: summary.total_relevant_bytes(),
                detail: protect_note,
            });
            continue;
        }

        if incoming_hash.is_some() && files_identical(&dst_file, written, incoming_hash) {
            let _ = fs::remove_file(&part_path);
            summary.files_reused += 1;
            summary.bytes_reused += written;
            progress(ImportProgress {
                files_done: done_idx + 1,
                files_total: total,
                bytes_done: summary.total_relevant_bytes(),
                detail: format!("IDÉNTICO (CONSERVADO): {canon_rel}"),
            });
            continue;
        }

        commit_part_file(&part_path, &dst_file)
            .map_err(|e| format!("NO SE PUDO CONFIRMAR {canon_rel}: {e}"))?;
        summary.files_copied += 1;
        summary.bytes_copied += written;

        progress(ImportProgress {
            files_done: done_idx + 1,
            files_total: total,
            bytes_done: summary.total_relevant_bytes(),
            detail: format!("EXTRAÍDO: {canon_rel}"),
        });
    }

    Ok(summary)
}

fn temp_part_path(dst: &Path) -> PathBuf {
    let fname = dst
        .file_name()
        .map(|n| format!("{}.part", n.to_string_lossy()))
        .unwrap_or_else(|| "import.part".into());
    dst.with_file_name(fname)
}

fn copy_file_atomically(src: &Path, dst: &Path, buf: &mut [u8]) -> io::Result<u64> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = temp_part_path(dst);
    let mut reader = File::open(src)?;
    let res = stream_to_part_file(&mut reader, &part, buf, false);
    match res {
        Ok((written, _)) => {
            commit_part_file(&part, dst)?;
            Ok(written)
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

fn stream_to_part_file<R: Read>(
    reader: &mut R,
    part: &Path,
    buf: &mut [u8],
    compute_hash: bool,
) -> io::Result<(u64, Option<[u8; 32]>)> {
    let mut out = File::create(part)?;
    let mut hasher = compute_hash.then(Sha256::new);
    let mut total = 0u64;
    loop {
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        if let Some(h) = hasher.as_mut() {
            h.update(&buf[..n]);
        }
        total += n as u64;
    }
    out.sync_all()?;
    drop(out);
    Ok((total, hasher.map(|h| h.finalize().into())))
}

fn commit_part_file(part: &Path, dst: &Path) -> io::Result<()> {
    if dst.is_file() {
        let _ = fs::remove_file(dst);
    }
    if let Err(e) = fs::rename(part, dst) {
        let _ = fs::remove_file(part);
        return Err(e);
    }
    Ok(())
}

/// Elimina únicamente archivos temporales `.part` huérfanos en `<documents_dir>/RBR`.
pub fn clean_temp_part_files(documents_dir: &Path) -> Result<(u32, u64), String> {
    let rbr = documents_dir.join("RBR");
    if !rbr.is_dir() {
        return Ok((0, 0));
    }
    let mut removed_files = 0u32;
    let mut freed_bytes = 0u64;
    clean_parts_recursive(&rbr, &mut removed_files, &mut freed_bytes);
    Ok((removed_files, freed_bytes))
}

fn clean_parts_recursive(dir: &Path, files: &mut u32, bytes: &mut u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            clean_parts_recursive(&path, files, bytes);
        } else if ft.is_file() && entry.file_name().to_string_lossy().ends_with(".part") {
            let sz = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if fs::remove_file(&path).is_ok() {
                *files += 1;
                *bytes += sz;
            }
        }
    }
}

/// Desinstala un tramo importado conservando cualquier archivo compartido (`TextureFilenameMap.dat`
/// y paquetes `*_textures.rbz` usados por variantes hermanas aún instaladas).
pub fn remove_stage_assets(
    documents_dir: &Path,
    stage_id: &str,
    all_stages: &[StageEntry],
) -> Result<u64, String> {
    let Some(target) = all_stages.iter().find(|s| s.descriptor.id == stage_id) else {
        return Err("TRAMO NO ENCONTRADO EN CATÁLOGO".into());
    };
    let rbr = documents_dir.join("RBR");
    let Some(maps_dir) = cars::resolve(&rbr, "Maps") else {
        return Ok(0);
    };
    let stem = target.descriptor.stem.as_str();
    let mut freed = 0u64;

    for ext in ["lbs", "col", "trk", "mat", "dls", "fnc", "ini"] {
        if let Some(f) = cars::resolve(&maps_dir, &format!("{stem}.{ext}")) {
            if let Ok(m) = fs::metadata(&f) {
                if fs::remove_file(&f).is_ok() {
                    freed += m.len();
                }
            }
        }
    }

    // Solo borrar `<stem>_textures.rbz` si ninguna otra etapa aún instalada comparte el mismo prefijo `track-XX_`
    let prefix = stem.rsplit_once('_').map(|(p, _)| p).unwrap_or(stem);
    let sibling_installed = all_stages.iter().any(|other| {
        other.descriptor.id != stage_id
            && other.files.present > 0
            && other.descriptor.stem.starts_with(&format!("{prefix}_"))
    });
    if !sibling_installed {
        if let Some(tex) = cars::resolve(&maps_dir, &format!("{stem}_textures.rbz")) {
            if let Ok(m) = fs::metadata(&tex) {
                if fs::remove_file(&tex).is_ok() {
                    freed += m.len();
                }
            }
        }
    }
    Ok(freed)
}

/// Desinstala los recursos exclusivos de un vehículo conservando carpetas 3D o bancos FMOD
/// si están compartidos con otra variante instalada en el catálogo.
pub fn remove_vehicle_assets(
    documents_dir: &Path,
    vehicle_id: &str,
    all_vehicles: &[VehicleEntry],
) -> Result<u64, String> {
    let Some(target) = all_vehicles.iter().find(|v| v.descriptor.id == vehicle_id) else {
        return Err("VEHÍCULO NO ENCONTRADO EN CATÁLOGO".into());
    };
    let rbr = documents_dir.join("RBR");
    let mut freed = 0u64;

    // 1. Paquete de físicas RSF si no lo comparte otra variante
    if let Some(pack) = target.descriptor.rsf_pack.as_deref() {
        let shared_pack = all_vehicles.iter().any(|other| {
            other.descriptor.id != vehicle_id
                && other.files.present > 0
                && other
                    .descriptor
                    .rsf_pack
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(pack))
        });
        if !shared_pack {
            for candidate in [
                format!("rsfdata/cars/{pack}"),
                format!("rsfdata/cars/{pack}.zip"),
            ] {
                if let Some(path) = cars::resolve(&rbr, &candidate) {
                    freed += remove_path_bytes(&path);
                }
            }
        }
    }

    // 2. Carpeta 3D `Cars/<folder>` solo si ningún otro vehículo instalado usa esa misma carpeta
    if let Some(folder) = extract_car_folder_name(&target.descriptor.model) {
        let shared_folder = all_vehicles.iter().any(|other| {
            other.descriptor.id != vehicle_id
                && other.files.present > 0
                && extract_car_folder_name(&other.descriptor.model)
                    .is_some_and(|f| f.eq_ignore_ascii_case(&folder))
        });
        if !shared_folder {
            if let Some(dir) = cars::resolve(&rbr, &format!("Cars/{folder}")) {
                freed += remove_path_bytes(&dir);
            }
        }
    }

    // 3. Banco FMOD solo si ningún otro vehículo instalado lo referencia
    if let Some(bank) = target.fmod_bank.as_deref() {
        let shared_bank = all_vehicles.iter().any(|other| {
            other.descriptor.id != vehicle_id
                && other.files.present > 0
                && other
                    .fmod_bank
                    .as_deref()
                    .is_some_and(|b| b.eq_ignore_ascii_case(bank))
        });
        if !shared_bank {
            let bank_file = if bank.to_ascii_lowercase().ends_with(".bank") {
                bank.to_string()
            } else {
                format!("{bank}.bank")
            };
            if let Some(p) = cars::resolve(&rbr, &format!("AudioFMOD/{bank_file}")) {
                freed += remove_path_bytes(&p);
            }
        }
    }

    Ok(freed)
}

fn extract_car_folder_name(model_path: &str) -> Option<String> {
    let parts: Vec<&str> = model_path
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("Cars") {
        Some(parts[1].to_string())
    } else {
        None
    }
}

fn remove_path_bytes(path: &Path) -> u64 {
    if path.is_file() {
        let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if fs::remove_file(path).is_ok() {
            return len;
        }
        return 0;
    }
    if path.is_dir() {
        let mut sum = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for e in entries.flatten() {
                sum += remove_path_bytes(&e.path());
            }
        }
        let _ = fs::remove_dir_all(path);
        return sum;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rerbr_asset_formats::zip::write::SimpleFileOptions;
    use rerbr_asset_formats::zip::ZipWriter;
    use std::io::Cursor;

    #[test]
    fn normalizer_accepts_universal_rbr_layouts_and_blocks_traversal_and_bloat() {
        assert_eq!(
            normalize_rbr_rel_path("RBR/Cars/FORD_FOCUS_2006/focus06.sgc").as_deref(),
            Some("Cars/FORD_FOCUS_2006/focus06.sgc")
        );
        assert_eq!(
            normalize_rbr_rel_path("MiCarpeta/rbr/cars/impreza03/Textures/subaru_external.dds")
                .as_deref(),
            Some("Cars/impreza03/textures/subaru_external.dds")
        );
        assert_eq!(
            normalize_rbr_rel_path("RBR_NGP-7.4.778.507/Plugins/NGP.dll").as_deref(),
            Some("Plugins/NGP.dll")
        );
        assert_eq!(
            normalize_rbr_rel_path("NGP.dll").as_deref(),
            Some("Plugins/NGP.dll")
        );
        assert_eq!(
            normalize_rbr_rel_path("FORD_FOCUS_2006/focus06.sgc").as_deref(),
            Some("Cars/FORD_FOCUS_2006/focus06.sgc")
        );
        assert_eq!(
            normalize_rbr_rel_path("Maps\\track-20_O_textures.rbz").as_deref(),
            Some("Maps/track-20_O_textures.rbz")
        );
        assert_eq!(
            normalize_rbr_rel_path("rsfdata/cache/CARS.JSON").as_deref(),
            Some("rsfdata/cache/cars.json")
        );

        // Bloqueo de traversal, DLLs ajenas, ejecutables, previews y skins
        assert_eq!(normalize_rbr_rel_path("../Cars/Cars.ini"), None);
        assert_eq!(normalize_rbr_rel_path("RBR/../../etc/passwd"), None);
        assert_eq!(
            normalize_rbr_rel_path("RBR/RichardBurnsRally_SSE.exe"),
            None
        );
        assert_eq!(normalize_rbr_rel_path("RBR/Plugins/Pacenote.dll"), None);
        assert_eq!(
            normalize_rbr_rel_path("RBR/Cars/FORD_FOCUS_2006/carparts/hood/preview.jpg"),
            None
        );
        assert_eq!(normalize_rbr_rel_path("__MACOSX/RBR/physics.rbz"), None);
    }

    #[test]
    fn zip_importer_extracts_valid_entries_deduplicates_incremental_runs_and_accounts_storage() {
        let tmp = std::env::temp_dir().join(format!("rerbr-univ-zip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let mut raw_zip = Cursor::new(Vec::new());
        {
            let mut zw = ZipWriter::new(&mut raw_zip);
            let opts = SimpleFileOptions::default();
            zw.start_file("Pack/RBR/Cars/Cars.ini", opts).unwrap();
            zw.write_all(b"[Car00]\n").unwrap();
            zw.start_file("Pack/RBR/Cars/TOYOTA_COROLLA/corolla.sgc", opts)
                .unwrap();
            zw.write_all(&[1u8; 2048]).unwrap();
            zw.start_file("Pack/RBR/Maps/track-20_O.lbs", opts).unwrap();
            zw.write_all(&[2u8; 4096]).unwrap();
            zw.start_file("Pack/RBR/AudioFMOD/Focus_Mk2_WRC.bank", opts)
                .unwrap();
            zw.write_all(&[3u8; 1024]).unwrap();
            // Bloat e intento de traversal: deben ignorarse limpiamente
            zw.start_file("Pack/RBR/Replays/test.rpl", opts).unwrap();
            zw.write_all(b"junk").unwrap();
            zw.start_file("../escape.txt", opts).unwrap();
            zw.write_all(b"nope").unwrap();
            zw.finish().unwrap();
        }

        let zip_bytes = raw_zip.into_inner();
        let s1 = import_zip_reader_into_documents(
            Cursor::new(zip_bytes.clone()),
            "test1.zip",
            &tmp,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(s1.files_copied, 4);
        assert_eq!(s1.files_reused, 0);
        assert_eq!(s1.files_ignored, 2);
        assert!(!tmp.join("escape.txt").exists());

        // Segunda importación idéntica: 0 escrituras duplicadas, 4 archivos reutilizados
        let s2 = import_zip_reader_into_documents(
            Cursor::new(zip_bytes),
            "test1.zip",
            &tmp,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(s2.files_copied, 0);
        assert_eq!(s2.files_reused, 4);

        let storage = StorageBreakdown::scan_documents(&tmp);
        assert_eq!(storage.total_files, 4);
        assert_eq!(storage.vehicles_bytes, 2048);
        assert_eq!(storage.stages_bytes, 4096);
        assert_eq!(storage.audio_bytes, 1024);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn certified_ngp74_dll_and_physics_rbz_are_protected_against_incompatible_overwrite_if_local_present(
    ) {
        let ngp74_dir = Path::new(r"C:\Users\adria\Downloads\RBR_NGP-7.4.778.507");
        let rsf75_dir = Path::new(r"C:\Richard Burns Rally");
        if !ngp74_dir.is_dir() || !rsf75_dir.join("Plugins").join("NGP.dll").is_file() {
            return;
        }

        let tmp = std::env::temp_dir().join(format!("rerbr-ngp74-protect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // 1. Importar runtime certificado NGP 7.4.778.507
        let first = import_directory_into_documents(ngp74_dir, &tmp, &mut |_| {}).unwrap();
        assert!(first.files_copied >= 2);

        // 2. Intentar importar un Plugins/NGP.dll 7.5 encima: debe quedar protegido el 7.4!
        let fake_75_pkg = tmp.join("incoming75");
        fs::create_dir_all(fake_75_pkg.join("Plugins")).unwrap();
        fs::copy(
            rsf75_dir.join("Plugins").join("NGP.dll"),
            fake_75_pkg.join("Plugins").join("NGP.dll"),
        )
        .unwrap();
        if rsf75_dir.join("physics.rbz").is_file() {
            fs::copy(
                rsf75_dir.join("physics.rbz"),
                fake_75_pkg.join("physics.rbz"),
            )
            .unwrap();
        }

        let second = import_directory_into_documents(&fake_75_pkg, &tmp, &mut |_| {}).unwrap();
        assert!(second.files_protected >= 1);
        let status = rerbr_sim::ngp_candidate::inspect_runtime_dir(&tmp.join("RBR"));
        assert_eq!(
            status.kind,
            rerbr_sim::ngp_candidate::NgpRuntimeKind::Verified74
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ios_c_abi_batch_import_handles_zip_and_folder_and_live_updates_epoch() {
        use crate::driving::resource_import::{
            query_android_saf, rerbr_ios_import_begin_batch, rerbr_ios_import_finish_batch,
            rerbr_ios_import_process_path, rerbr_ios_set_persisted_folder_state, SafPhase,
        };
        use std::ffi::CString;

        let tmp = std::env::temp_dir().join(format!("rerbr-ios-cabi-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let docs = tmp.join("Documents");
        let ext_folder = tmp.join("iCloudFolder").join("RBR").join("Sgs");
        fs::create_dir_all(&docs).unwrap();
        fs::create_dir_all(&ext_folder).unwrap();
        fs::write(ext_folder.join("br_spectator_00.sgs"), [7u8; 512]).unwrap();

        let zip_path = tmp.join("car_and_stage.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zw = ZipWriter::new(f);
            let opts = SimpleFileOptions::default();
            zw.start_file("RBR/Cars/Cars.ini", opts).unwrap();
            zw.write_all(b"[Car00]\n").unwrap();
            zw.start_file("RBR/Maps/TextureFilenameMap.dat", opts)
                .unwrap();
            zw.write_all(b"texmap").unwrap();
            zw.finish().unwrap();
        }

        let prev_docs = std::env::var_os("RERBR_PROBE_DOCUMENTS");
        std::env::set_var("RERBR_PROBE_DOCUMENTS", &docs);

        let epoch_before = query_android_saf().completion_epoch;
        let label_c = CString::new("iCloud RBR").unwrap();
        let zip_c = CString::new(zip_path.to_str().unwrap()).unwrap();
        let dir_c = CString::new(tmp.join("iCloudFolder").to_str().unwrap()).unwrap();

        unsafe {
            rerbr_ios_set_persisted_folder_state(1, label_c.as_ptr());
            rerbr_ios_import_begin_batch(2, label_c.as_ptr());
            rerbr_ios_import_process_path(zip_c.as_ptr(), 0, 0, 2);
            rerbr_ios_import_process_path(dir_c.as_ptr(), 1, 1, 2);
        }
        rerbr_ios_import_finish_batch();

        let snap = query_android_saf();
        assert_eq!(snap.phase, SafPhase::Done);
        assert!(snap.has_persisted_uri);
        assert_eq!(snap.persisted_label, "iCloud RBR");
        assert_eq!(snap.files_done, 3);
        assert_eq!(snap.completion_epoch, epoch_before.wrapping_add(1));
        assert!(docs.join("RBR/Cars/Cars.ini").is_file());
        assert!(docs.join("RBR/Maps/TextureFilenameMap.dat").is_file());
        assert!(docs.join("RBR/Sgs/br_spectator_00.sgs").is_file());

        match prev_docs {
            Some(v) => std::env::set_var("RERBR_PROBE_DOCUMENTS", v),
            None => std::env::remove_var("RERBR_PROBE_DOCUMENTS"),
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn shared_dependencies_preserved_when_removing_stage_or_vehicle_and_temp_parts_cleaned() {
        use crate::driving::content_manager::Catalog;
        use rerbr_content::{Selection, StageDescriptor};

        let tmp =
            std::env::temp_dir().join(format!("rerbr-shared-deps-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let maps = tmp.join("RBR/Maps");
        fs::create_dir_all(&maps).unwrap();
        fs::write(maps.join("TextureFilenameMap.dat"), b"shared-map").unwrap();

        // Dos tramos instalados en Maps/: Harwood Forest (`track-20_O`) y Chirdonhead (`track-22_O`),
        // más una variante hermana en el mapa 20 para verificar preservación de `*_textures.rbz` y `TextureFilenameMap.dat`.
        for prefix in ["track-20", "track-22"] {
            for w in ["O", "N", "E", "M"] {
                fs::write(maps.join(format!("{prefix}_{w}_textures.rbz")), [9u8; 256]).unwrap();
            }
            for ext in ["lbs", "col", "trk", "mat", "dls", "fnc", "ini"] {
                fs::write(maps.join(format!("{prefix}_O.{ext}")), [5u8; 128]).unwrap();
            }
        }
        // Archivo huérfano `.part`
        fs::write(maps.join("interrupted.rbz.part"), [1u8; 1024]).unwrap();

        let sel = Selection {
            stage: StageDescriptor::harwood_forest(),
            ..Selection::default()
        };
        let mut cat = Catalog::scan_root(&tmp, &sel);
        let ready_before = cat
            .stages
            .iter()
            .filter(|s| s.files.checked && s.files.missing.is_empty())
            .count();
        assert_eq!(
            ready_before, 2,
            "Harwood Forest y Chirdonhead deben estar instalados (ready={ready_before})"
        );

        // Limpiar temporales .part sin tocar tramos
        let (parts_removed, bytes_removed) = clean_temp_part_files(&tmp).unwrap();
        assert_eq!(parts_removed, 1);
        assert_eq!(bytes_removed, 1024);

        // Añadir también una variante meteorológica hermana (`track-20_N`) en memoria para probar que
        // desinstalar una variante hermana nunca elimina `track-20_*_textures.rbz` de Harwood Forest.
        for ext in ["lbs", "col", "trk", "mat", "dls", "fnc", "ini"] {
            fs::write(maps.join(format!("track-20_N.{ext}")), [4u8; 128]).unwrap();
        }
        let mut weather_sibling = cat.stage("harwood-forest").unwrap().clone();
        weather_sibling.descriptor.id = "harwood-forest-noon".into();
        weather_sibling.descriptor.stem = "track-20_N".into();
        cat.stages.push(weather_sibling);

        let freed_sibling = remove_stage_assets(&tmp, "harwood-forest-noon", &cat.stages).unwrap();
        assert!(freed_sibling > 0);
        assert!(
            maps.join("track-20_N_textures.rbz").is_file(),
            "El archivo *_textures.rbz compartido con Harwood Forest debe conservarse"
        );

        // Desinstalar Chirdonhead (no seleccionado) y comprobar que Harwood Forest y TextureFilenameMap.dat siguen intactos
        let freed_chirdon = remove_stage_assets(&tmp, "chirdonhead", &cat.stages).unwrap();
        assert!(freed_chirdon > 0);
        assert!(
            maps.join("TextureFilenameMap.dat").is_file(),
            "TextureFilenameMap.dat compartido nunca debe borrarse"
        );

        let cat_after = Catalog::scan_root(&tmp, &sel);
        let harwood = cat_after.stage("harwood-forest").unwrap();
        assert!(
            harwood.files.missing.is_empty(),
            "Harwood Forest debe seguir 100% íntegro tras liberar otros tramos: {:?}",
            harwood.files.missing
        );
        assert!(
            !cat_after
                .stage("chirdonhead")
                .unwrap()
                .files
                .missing
                .is_empty(),
            "Chirdonhead liberado debe pasar a estado incompleto"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn full_rsf_installation_scans_multiple_vehicles_and_stages_if_local_present() {
        use crate::driving::content_manager::{Catalog, UniversalStatus};
        use rerbr_content::Selection;

        let pc_rbr = Path::new(r"C:\Richard Burns Rally");
        if !pc_rbr.join("Maps").is_dir() || !pc_rbr.join("Cars").is_dir() {
            return;
        }
        // Crear envoltorio temporal con enlace/copia mínima de metadatos no es necesario:
        // verificamos directamente `rsf_catalog::discover` sobre la instalación completa en modo solo lectura.
        let discovered = crate::driving::rsf_catalog::discover(pc_rbr);
        assert!(
            discovered.len() > 5,
            "La instalación RSF real debe detectar múltiples vehículos instalados (detectados {})",
            discovered.len()
        );

        // Verificar también un entorno multi-vehículo y multi-tramo sintético completo en Documents/RBR
        let tmp = std::env::temp_dir().join(format!("rerbr-multicar-stage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let maps = tmp.join("RBR/Maps");
        fs::create_dir_all(&maps).unwrap();
        fs::write(maps.join("TextureFilenameMap.dat"), b"x").unwrap();
        // Instalar 2 tramos de países distintos: Harwood Forest (track-20_O) y Chirdonhead (track-22_O)
        for prefix in ["track-20", "track-22"] {
            for w in ["O", "N", "E", "M"] {
                fs::write(maps.join(format!("{prefix}_{w}_textures.rbz")), b"rbz").unwrap();
            }
            for ext in ["lbs", "col", "trk", "mat", "dls", "fnc", "ini"] {
                fs::write(maps.join(format!("{prefix}_O.{ext}")), b"st").unwrap();
            }
        }
        // Y dejar 1 tercer tramo incompleto (Côte d'Arbroz `track-10_O` sin texturas)
        fs::write(maps.join("track-10_O.lbs"), b"partial").unwrap();

        let cat = Catalog::scan_root(&tmp, &Selection::default());
        let sum = cat.universal_summary(true);
        assert_eq!(sum.stages_installed, 2);
        assert!(sum.stages_incomplete >= 1);
        assert_eq!(
            cat.stage("harwood-forest").unwrap().universal_status(),
            UniversalStatus::Installed
        );
        assert_eq!(
            cat.stage("chirdonhead").unwrap().universal_status(),
            UniversalStatus::Installed
        );
        assert_eq!(
            cat.stage("cote-darbroz").unwrap().universal_status(),
            UniversalStatus::Incomplete
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn main_menu_leaderboards_and_storage_ui_flow_renders_within_vertex_budget() {
        use crate::driving::{
            leaderboard::{LeaderboardSubmission, VerificationStatus},
            menu,
            records::{RecordKey, RunSubmission},
        };
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
        use rerbr_content::{Selection, StageDescriptor, VehicleDescriptor};

        let mut app = crate::driving::Runtime::new_for_catalog(false);
        let sel_a = Selection {
            vehicle: VehicleDescriptor::ford_focus_2006(),
            stage: StageDescriptor::harwood_forest(),
        };
        let sel_b = Selection {
            vehicle: VehicleDescriptor::toyota_corolla_wrc(),
            stage: StageDescriptor::chirdonhead(),
        };
        app.leaderboards.submit(&LeaderboardSubmission {
            run: RunSubmission {
                key: RecordKey::from_selection(&sel_a, "rsf-ngp-7.4.778:focus06", true),
                car_name: sel_a.vehicle.name.clone(),
                stage_name: sel_a.stage.name.clone(),
                driving_ms: 199_450,
                penalty_ms: 0,
                false_start: false,
                splits_ms: vec![65_000, 132_000],
                splits_passed: 2,
                splits_total: 2,
                finished: true,
                finish_crossed_forward: true,
                visual_only: false,
                is_school: false,
            },
            pilot_name: "PILOTO TEST".into(),
            car_category: "WRC".into(),
            sim_steps: (199_450 * 720) / 1000,
            stage_length_m: 4800,
        });
        let out_b = app.leaderboards.submit(&LeaderboardSubmission {
            run: RunSubmission {
                key: RecordKey::from_selection(&sel_b, "rbr-retail-1.02", true),
                car_name: sel_b.vehicle.name.clone(),
                stage_name: sel_b.stage.name.clone(),
                driving_ms: 212_100,
                penalty_ms: 10_000,
                false_start: true,
                splits_ms: vec![70_000, 141_000],
                splits_passed: 2,
                splits_total: 2,
                finished: true,
                finish_crossed_forward: true,
                visual_only: false,
                is_school: false,
            },
            pilot_name: "PILOTO TEST".into(),
            car_category: "WRC".into(),
            sim_steps: (212_100 * 720) / 1000,
            stage_length_m: 5100,
        });
        assert!(out_b.accepted);
        app.last_leaderboard_result = Some(out_b);

        // 1. Desde el Menú Principal, pulsar el botón superior TIEMPOS abre Page::Leaderboards
        app.apply_command(menu::Command::EnterMainMenu);
        assert_eq!(app.menu.page, menu::Page::Main);
        {
            let ctx = app.context();
            let rows = app.menu.rows(&ctx);
            let lb_btn = menu::Menu::main_leaderboards_rect();
            assert_eq!(
                app.menu.press_with_ctx(
                    f64::from(lb_btn.center_x()),
                    f64::from(lb_btn.y + lb_btn.h * 0.5),
                    &ctx,
                    &rows
                ),
                Some(menu::Command::Open(menu::Page::Leaderboards))
            );
            let res_btn = menu::Menu::main_content_rect();
            assert_eq!(
                app.menu.press_with_ctx(
                    f64::from(res_btn.center_x()),
                    f64::from(res_btn.y + res_btn.h * 0.5),
                    &ctx,
                    &rows
                ),
                Some(menu::Command::Open(menu::Page::Content))
            );
        }

        // 2. Renderizar Leaderboards en los 3 modos de filtro dentro del presupuesto de vértices
        app.apply_command(menu::Command::Open(menu::Page::Leaderboards));
        assert_eq!(app.menu.page, menu::Page::Leaderboards);
        for _ in 0..3 {
            let ctx = app.context();
            let rows = app.menu.rows(&ctx);
            assert!(rows.len() >= 4);
            let mut canvas = Canvas::new((2556, 1179));
            app.menu.draw(&mut canvas, &ctx, "CLASIFICACIÓN LOCAL");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
            app.apply_command(menu::Command::LeaderboardCycleFilter);
        }
        assert!(app
            .leaderboards
            .entries()
            .iter()
            .all(|e| e.verification == VerificationStatus::LocalOnly));

        // 3. Renderizar ContentStorage y StageComplete dentro del presupuesto de vértices
        app.apply_command(menu::Command::Open(menu::Page::ContentStorage));
        {
            let ctx = app.context();
            let mut canvas = Canvas::new((2556, 1179));
            app.menu.draw(&mut canvas, &ctx, "ALMACENAMIENTO");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
        }
        app.menu.show(menu::Page::StageComplete);
        {
            let ctx = app.context();
            let rows = app.menu.rows(&ctx);
            assert!(rows.iter().any(|r| r.label == "PUESTO EN TRAMO"));
            assert!(rows.iter().any(|r| r.label == "VER TABLA DE CLASIFICACIÓN"));
            let mut canvas = Canvas::new((2556, 1179));
            app.menu.draw(&mut canvas, &ctx, "META");
            assert!(canvas.finish().len() / 20 <= MAX_CANVAS_VERTICES);
        }
    }
}
