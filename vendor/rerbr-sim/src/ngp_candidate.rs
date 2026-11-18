//! Read-only candidate investigation. This module cannot enable a new runtime engine.
//! Equal tables, labels, CarId and archive CRCs never certify original NGP vehicle CRCs.
use rbr_formats::{car_pack, car_specs::CarSpecs, lsp, ngp, rbz};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

pub const RSF_779_SHA256: &str = "cf351cdf99f880631ed2cb1d9b6b5c9db7d7c233ad905740393c37df967801ec";
pub const RUNTIME_BLOCKER: &str = "NGP 7.5.779.508 is recognized but NOT enabled: original loaded-value vehicle CRC checks are unported and 7.5 physics parity is unverified; use the read-only ngp-candidate-audit diagnostic. The separate 7.4 regression baseline remains available.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NgpRuntimeKind {
    Verified74,
    Detected75Unverified,
    UnsupportedBinary,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgpRuntimeStatus {
    pub kind: NgpRuntimeKind,
    pub sha256: Option<String>,
    pub file_version: Option<String>,
    pub physics_revision: Option<u32>,
    pub summary: String,
}

/// Inspects `Plugins/NGP.dll` and `physics.rbz` (`PhysicsRevision_NGP`) inside `rbr_root` read-only.
/// Distinguishes certified NGP 7.4.778, detected NGP 7.5.779.508 (tables equal to 7.4, 7.5 CRC unported),
/// unsupported binaries, and missing installations.
pub fn inspect_runtime_dir(rbr_root: &Path) -> NgpRuntimeStatus {
    let physics_revision = rbr_formats::cars::resolve(rbr_root, "physics.rbz")
        .and_then(|p| ngp::compat::physics_revision(&p).ok().flatten());
    let Some(dll_path) = rbr_formats::cars::resolve(rbr_root, "Plugins/NGP.dll") else {
        return NgpRuntimeStatus {
            kind: NgpRuntimeKind::Missing,
            sha256: None,
            file_version: None,
            physics_revision,
            summary: "FALTA Plugins/NGP.dll".into(),
        };
    };
    let bytes = match std::fs::read(&dll_path) {
        Ok(b) => b,
        Err(e) => {
            return NgpRuntimeStatus {
                kind: NgpRuntimeKind::Missing,
                sha256: None,
                file_version: None,
                physics_revision,
                summary: format!("ERROR Plugins/NGP.dll: {e}"),
            };
        }
    };
    let id = ngp::compat::identify(&bytes);
    if ngp::validate(&bytes).is_ok() {
        let ver = id.file_version.as_deref().unwrap_or("7.4.778.507");
        let summary = format!("NGP {ver} VERIFICADO (REV {})", physics_revision.unwrap_or(0));
        return NgpRuntimeStatus {
            kind: NgpRuntimeKind::Verified74,
            sha256: Some(id.sha256),
            file_version: id.file_version,
            physics_revision,
            summary,
        };
    }
    if id.sha256 == RSF_779_SHA256 {
        let ver = id.file_version.as_deref().unwrap_or("7.5.779.508");
        let summary =
            format!("NGP {ver} DETECTADO (TABLAS == 7.4 / CRC 7.5 NO PORTADO / REV {})", physics_revision.unwrap_or(0));
        return NgpRuntimeStatus {
            kind: NgpRuntimeKind::Detected75Unverified,
            sha256: Some(id.sha256),
            file_version: id.file_version,
            physics_revision,
            summary,
        };
    }
    NgpRuntimeStatus {
        kind: NgpRuntimeKind::UnsupportedBinary,
        sha256: Some(id.sha256),
        file_version: id.file_version.clone(),
        physics_revision,
        summary: format!("NGP.DLL NO CERTIFICADA ({})", id.file_version.as_deref().unwrap_or("DESCONOCIDA")),
    }
}

/// Keep upstream's allowlist intact. Recognizing the exact installed binary adds
/// an actionable diagnostic, not a new supported hash or an execution path.
pub fn validate_runtime(bytes: &[u8]) -> Result<(), String> {
    match ngp::validate(bytes) {
        Ok(()) => Ok(()),
        Err(_) if format!("{:x}", Sha256::digest(bytes)) == RSF_779_SHA256 => Err(RUNTIME_BLOCKER.into()),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceEvidence {
    pub surface: String,
    /// The surface file's `CarId`, the value NGP's loaded-value walk compares against (unported).
    pub car_id: Option<u32>,
    pub common_sha256: String,
    pub surface_sha256: Option<String>,
    pub options_sha256: Option<String>,
    /// Original format parser can read headline NGP specs. Not a CRC verdict.
    pub ngp_specs: Option<CarSpecs>,
    pub missing_ngp_keys: Vec<String>,
}

/// A standalone NGP car pack (the RSF layout: one folder of `common.lsp`, `<surface>.lsp`,
/// `r_<surface>.lsp` and `setups/d_<surface>.lsp`), read in place.
#[derive(Debug, Clone, PartialEq)]
pub struct CarPackEvidence {
    /// The archive's SHA-256 when the pack is a file.
    pub sha256: Option<String>,
    /// The pack's text file: its first line (the car's name).
    pub title: Option<String>,
    /// The same file in full (revision, date, specifications, model credit).
    pub about: Option<String>,
    pub surfaces: Vec<SurfaceEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleEvidence {
    pub folder: String,
    /// Stock cars also have CarId. Presence or a nonzero value is not proof.
    pub car_id: Option<u32>,
    pub surfaces: Vec<SurfaceEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveEvidence {
    pub sha256: String,
    pub physics_revision: Option<u32>,
    pub vehicles: Vec<VehicleEvidence>,
}

fn read_entry<R: Read + std::io::Seek>(zip: &mut rbz::ZipArchive<R>, name: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    // Reading to EOF verifies the ZIP entry CRC. This is NOT NGP's loaded-value CRC.
    rbz::by_name(zip, name)
        .map_err(|e| format!("{name}: {e}"))?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{name}: {e}"))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(format!("{name}: physics script exceeds diagnostic read limit"));
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn car_id(nodes: &[lsp::Node]) -> Option<u32> {
    nodes
        .windows(2)
        .find_map(|pair| match pair {
            [lsp::Node::Atom(key), lsp::Node::Atom(value)] if key == "CarId" => value.parse().ok(),
            _ => None,
        })
        .or_else(|| {
            nodes.iter().find_map(|node| match node {
                lsp::Node::List(children) => car_id(children),
                _ => None,
            })
        })
}

/// Inventory every car folder in one archive; never extract, install, or change
/// any script. Sorted output supports reproducible evidence and regression tests.
pub fn inspect_archive(path: &Path) -> Result<ArchiveEvidence, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = rbz::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|e| e.to_string())?;
    let mut folders = Vec::new();
    for index in 0..zip.len() {
        let name = zip
            .by_index(index)
            .map_err(|e| e.to_string())?
            .name()
            .map_err(|e| e.to_string())?
            .replace('\\', "/")
            .to_lowercase();
        if let Some(folder) = name.strip_suffix("common.lsp") {
            if folder.starts_with("physics/") {
                folders.push(folder.to_string());
            }
        }
    }
    folders.sort();
    folders.dedup();
    let mut vehicles = Vec::new();
    for folder in folders {
        let name = format!("{folder}common.lsp");
        let common_bytes = read_entry(&mut zip, &name)?;
        let common = std::str::from_utf8(&common_bytes).map_err(|e| format!("{name}: {e}"))?;
        let common_nodes = lsp::parse(common).map_err(|e| format!("{name}: {e}"))?;
        let mut surfaces = Vec::new();
        for surface in ["gravel", "tarmac", "snow"] {
            let surface_name = format!("{folder}{surface}.lsp");
            let surface_bytes = if rbz::by_name(&mut zip, &surface_name).is_ok() {
                Some(read_entry(&mut zip, &surface_name)?)
            } else {
                None
            };
            let surface_text =
                surface_bytes.as_deref().map(std::str::from_utf8).transpose().map_err(|e| e.to_string())?;
            let surface_nodes = surface_text.map(|text| lsp::parse(text).map_err(|e| e.to_string())).transpose()?;
            let options_name = format!("{folder}r_{surface}.lsp");
            let exists = rbz::by_name(&mut zip, &options_name).is_ok();
            let options_bytes = if exists { Some(read_entry(&mut zip, &options_name)?) } else { None };
            let options = options_bytes.as_deref().map(std::str::from_utf8).transpose().map_err(|e| e.to_string())?;
            let mut scripts = vec![(name.clone(), common_nodes.clone())];
            if let Some(nodes) = &surface_nodes {
                scripts.push((surface_name, nodes.clone()));
            }
            if let Some(text) = options {
                scripts.push((options_name, lsp::parse(text).map_err(|e| e.to_string())?));
            }
            let audit = crate::physics_audit::PhysicsAudit::inspect(&scripts);
            surfaces.push(SurfaceEvidence {
                surface: surface.into(),
                car_id: surface_nodes.as_ref().and_then(|nodes| car_id(nodes)),
                common_sha256: sha256_hex(&common_bytes),
                surface_sha256: surface_bytes.as_ref().map(|b| sha256_hex(b)),
                options_sha256: options_bytes.as_ref().map(|b| sha256_hex(b)),
                ngp_specs: options.and_then(|options| CarSpecs::parse(common, options)),
                missing_ngp_keys: audit.missing_ngp_keys,
            });
        }
        vehicles.push(VehicleEvidence { folder, car_id: car_id(&common_nodes), surfaces });
    }
    Ok(ArchiveEvidence { sha256: sha256_hex(&bytes), physics_revision: ngp::compat::physics_revision(path)?, vehicles })
}

/// Whether a pack (folder, or zip whose entries sit in a `<pack>/` folder) holds `name`.
fn pack_has(path: &Path, name: &str) -> bool {
    if path.is_dir() {
        return rbr_formats::cars::resolve(path, name).is_some_and(|file| file.is_file());
    }
    let Ok(zip) = rbz::open(path) else {
        return false;
    };
    let want = name.replace('\\', "/");
    let found = zip.file_names().flatten().any(|entry| {
        entry.replace('\\', "/").split_once('/').is_some_and(|(_, rest)| rest.eq_ignore_ascii_case(&want))
    });
    found
}

/// Audit one car pack in place: the metadata text, and each surface's scripts and `CarId`. The pack may be a folder
/// or the RSF zip whose entries sit in a `<pack>/` folder; nothing is extracted, written or executed.
pub fn inspect_car_pack(path: &Path) -> Result<CarPackEvidence, String> {
    let pack = car_pack::Pack::open(path)?;
    if !pack_has(path, "common.lsp") {
        return Err(format!("{}: no common.lsp (not an NGP car pack)", path.display()));
    }
    let common_bytes = pack.read("common.lsp")?;
    let common = std::str::from_utf8(&common_bytes).map_err(|e| format!("common.lsp: {e}"))?;
    let common_nodes = lsp::parse(common).map_err(|e| format!("common.lsp: {e}"))?;
    let mut surfaces = Vec::new();
    for surface in ["gravel", "tarmac", "snow"] {
        let surface_name = format!("{surface}.lsp");
        if !pack_has(path, &surface_name) {
            continue;
        }
        let surface_bytes = pack.read(&surface_name)?;
        let surface_text = std::str::from_utf8(&surface_bytes).map_err(|e| format!("{surface_name}: {e}"))?;
        let surface_nodes = lsp::parse(surface_text).map_err(|e| format!("{surface_name}: {e}"))?;
        let options_name = format!("r_{surface}.lsp");
        let options_bytes = if pack_has(path, &options_name) { Some(pack.read(&options_name)?) } else { None };
        let options = options_bytes.as_deref().map(std::str::from_utf8).transpose().map_err(|e| e.to_string())?;
        let mut scripts = vec![("common.lsp".to_string(), common_nodes.clone()), (surface_name, surface_nodes.clone())];
        if let Some(text) = options {
            scripts.push((options_name, lsp::parse(text).map_err(|e| e.to_string())?));
        }
        let audit = crate::physics_audit::PhysicsAudit::inspect(&scripts);
        surfaces.push(SurfaceEvidence {
            surface: surface.into(),
            car_id: car_id(&surface_nodes),
            common_sha256: sha256_hex(&common_bytes),
            surface_sha256: Some(sha256_hex(&surface_bytes)),
            options_sha256: options_bytes.as_ref().map(|b| sha256_hex(b)),
            ngp_specs: options.and_then(|options| CarSpecs::parse(common, options)),
            missing_ngp_keys: audit.missing_ngp_keys,
        });
    }
    let sha256 = if path.is_file() {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Some(sha256_hex(&bytes))
    } else {
        None
    };
    Ok(CarPackEvidence { sha256, title: pack.title(), about: pack.about(), surfaces })
}

/// Comparing immutable reads never changes the runtime allowlist or executes DLL code.
pub fn compare_binaries(reference: &Path, candidate: &Path) -> Result<ngp::compat::Report, String> {
    let reference = std::fs::read(reference).map_err(|e| e.to_string())?;
    let candidate = std::fs::read(candidate).map_err(|e| e.to_string())?;
    ngp::compat::compare(&reference, &candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_car_id_is_not_ngp_physics_or_crc_proof() {
        let nodes = lsp::parse("(\"Car\" (\"CAR_ROOT\" CarId 7))").unwrap();
        assert_eq!(car_id(&nodes), Some(7));
        let audit = crate::physics_audit::PhysicsAudit::inspect(&[("common.lsp".into(), nodes)]);
        assert_eq!(audit.missing_ngp_keys.len(), 34);
        assert!(CarSpecs::parse("(\"Car\" (\"CAR_ROOT\" CarId 7))", "()").is_none());
    }

    #[test]
    fn car_pack_reports_the_surface_car_id_and_metadata() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rerbr-ngp-pack-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("common.lsp"), "(\"Car\" (\"CAR_ROOT\" MaxTorque_NGP 400 Features_NGP 6))").unwrap();
        std::fs::write(dir.join("gravel.lsp"), "(\"Car\" (\"CAR_ROOT\" CarId 745426673))").unwrap();
        std::fs::write(dir.join("r_gravel.lsp"), "(\"Options\" (\"Car\" (\"CAR_ROOT\" CarId 9)))").unwrap();
        std::fs::write(dir.join("Ford Focus"), "Ford Focus Mk II RS WRC 2006\nrevision 1 / 2019-11-15\n").unwrap();
        let evidence = inspect_car_pack(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(evidence.title.as_deref(), Some("Ford Focus Mk II RS WRC 2006"));
        assert_eq!(evidence.surfaces.len(), 1);
        assert_eq!(evidence.surfaces[0].car_id, Some(745_426_673));
        assert_eq!(evidence.surfaces[0].surface, "gravel");
    }

    #[test]
    fn labels_do_not_enable_a_runtime_binary() {
        assert!(validate_runtime(b"NGP 7.5.779.508 PhysicsRevision_NGP 779").is_err());
        assert!(validate_runtime(&[]).is_err());
        assert!(RUNTIME_BLOCKER.contains("NOT enabled"));
    }

    #[test]
    fn inspect_runtime_dir_distinguishes_74_and_75_if_pc_present() {
        let dir74 = Path::new(r"C:\Users\adria\Desktop\RBR");
        if dir74.join("Plugins/NGP.dll").is_file() {
            let st74 = inspect_runtime_dir(dir74);
            assert_eq!(st74.kind, NgpRuntimeKind::Verified74);
            assert!(st74.summary.contains("7.4.778"));
        }
        let dir75 = Path::new(r"C:\Richard Burns Rally");
        if dir75.join("Plugins/NGP.dll").is_file() {
            let st75 = inspect_runtime_dir(dir75);
            assert_eq!(st75.kind, NgpRuntimeKind::Detected75Unverified);
            assert!(st75.summary.contains("7.5.779"));
        }
    }

    #[test]
    #[ignore = "full PE byte-scan comparison between 7.4 and 7.5; run with --release -- --ignored"]
    fn ngp_75_779_vs_74_778_binary_and_physics_revision_report_if_present() {
        let dll74 = Path::new(r"C:\Users\adria\Desktop\RBR\Plugins\NGP.dll");
        let dll75 = Path::new(r"C:\Richard Burns Rally\Plugins\NGP.dll");
        if !dll74.is_file() || !dll75.is_file() {
            return;
        }
        let report = compare_binaries(dll74, dll75).expect("compare NGP 7.4 vs 7.5");
        eprintln!("=== NGP 7.4.778 vs 7.5.779 BINARY REPORT ===\n{}", report.render());
        let rev74 = ngp::compat::physics_revision(Path::new(r"C:\Users\adria\Desktop\RBR\physics.rbz")).unwrap();
        let rev75 = ngp::compat::physics_revision(Path::new(r"C:\Richard Burns Rally\physics.rbz")).unwrap();
        eprintln!("physics.rbz PhysicsRevision_NGP: 7.4={rev74:?} vs RSF/7.5={rev75:?}");
    }
}
