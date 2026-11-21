//! Read-only duplicate audit of the merged vehicle catalog (presets + RSF + visual folders).
use super::content_manager::{Catalog, VehicleEntry};
use rerbr_content::Selection;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn norm_model(v: &VehicleEntry) -> String {
    v.descriptor.model.replace('\\', "/").to_ascii_lowercase()
}

fn norm_name(v: &VehicleEntry) -> String {
    v.descriptor
        .name
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Groups of entries that are *really* the same vehicle: identical model file, identical NGP pack
/// (or both without one) and the same normalised display name. Different packs/years sharing a
/// model are legitimate variants and are never grouped.
pub fn true_duplicate_groups(catalog: &Catalog) -> Vec<Vec<usize>> {
    let mut by_key: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
    for (i, v) in catalog.vehicles.iter().enumerate() {
        let pack = v
            .descriptor
            .rsf_pack
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase();
        by_key
            .entry((norm_model(v), pack, norm_name(v)))
            .or_default()
            .push(i);
    }
    by_key.into_values().filter(|g| g.len() > 1).collect()
}

/// Dumps the full catalog and candidate duplicates for a Documents root (env `CATALOG_AUDIT_DOCS`).
#[test]
#[ignore]
fn dump_catalog_duplicates() {
    let Some(docs) = std::env::var_os("CATALOG_AUDIT_DOCS").map(PathBuf::from) else {
        return;
    };
    let catalog = Catalog::scan_root(&docs, &Selection::default());
    println!("CATALOG {} vehicles", catalog.vehicles.len());
    for (i, v) in catalog.vehicles.iter().enumerate() {
        println!(
            "CAT {i:>3} id={:<44} name={:<40} model={} pack={:?} cls={:?}",
            v.descriptor.id,
            v.descriptor.name,
            v.descriptor.model,
            v.descriptor.rsf_pack,
            v.classification
        );
    }
    let mut by_model: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, v) in catalog.vehicles.iter().enumerate() {
        by_model.entry(norm_model(v)).or_default().push(i);
    }
    for (m, g) in by_model.iter().filter(|(_, g)| g.len() > 1) {
        println!(
            "SHARED MODEL {m}: {:?}",
            g.iter()
                .map(|&i| (
                    catalog.vehicles[i].descriptor.id.clone(),
                    catalog.vehicles[i].descriptor.rsf_pack.clone()
                ))
                .collect::<Vec<_>>()
        );
    }
    println!(
        "TRUE DUPLICATE GROUPS: {:?}",
        true_duplicate_groups(&catalog)
    );
}

/// Logs installation-local catalog, metadata and complete NGP dependencies without booting a pack.
#[test]
#[ignore = "requires a private Documents/RBR directory"]
fn audit_rsf_physics_inventory() {
    use super::ngp_experimental::{inspect_pack_state, PackAuditState};
    use rerbr_sim::{
        ngp_candidate::inspect_car_pack,
        rbr_formats::{car_pack::Pack, cars, lsp, rsf},
    };
    use std::collections::BTreeSet;

    let docs = PathBuf::from(
        std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS"),
    );
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(&docs, &Selection::default());
    let metadata = rsf::Install::open(&root).ok();
    let visible: BTreeSet<_> = catalog
        .browse_vehicle_indices(|_| false)
        .into_iter()
        .collect();
    let emit = |kind: &str, values: &[&str]| {
        let fields = values
            .iter()
            .map(|f| f.replace(['\t', '\r', '\n'], " "))
            .collect::<Vec<_>>();
        println!("INVENTORY\t{kind}\t{}", fields.join("\t"));
    };
    let retail = cars::resolve(&root, "Cars/Cars.ini")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| cars::parse(&text).ok())
        .unwrap_or_default();
    for (index, entry) in catalog.vehicles.iter().enumerate() {
        let d = &entry.descriptor;
        emit(
            "vehicle",
            &[
                &d.id,
                &d.name,
                &d.model,
                d.rsf_pack.as_deref().unwrap_or(""),
                &visible.contains(&index).to_string(),
                &format!("{:?}", entry.classification),
                &entry.files.missing.join("; "),
            ],
        );
        if d.is_original_rbr() {
            let slot = retail.iter().find(|e| e.index == d.original_slot as usize);
            emit(
                "retail",
                &[
                    &d.id,
                    &d.original_slot.to_string(),
                    slot.map_or("", |s| s.file_name.as_str()),
                ],
            );
            continue;
        }
        if d.rsf_pack.is_none() && !d.is_ngp_experimental_car() {
            emit("physics", &[&d.id, "unknown", "", "", ""]);
            continue;
        }
        let state = inspect_pack_state(&root, d);
        let path = match &state {
            PackAuditState::Missing { expected } => {
                emit("physics", &[&d.id, "missing", expected, "", ""]);
                continue;
            }
            PackAuditState::Incompatible { path, reason } => {
                emit(
                    "physics",
                    &[&d.id, "incomplete", &path.display().to_string(), reason, ""],
                );
                continue;
            }
            PackAuditState::Ready { path } | PackAuditState::Incomplete { path, .. } => path,
        };
        let pack = Pack::open(path).expect("resolved pack");
        let mut problems = Vec::new();
        let mut inspect = |name: &str| match pack.read(name) {
            Ok(bytes)
                if !bytes.is_empty()
                    && (!name.ends_with(".lsp")
                        || lsp::parse(&String::from_utf8_lossy(&bytes)).is_ok()) => {}
            Ok(_) => problems.push(format!("{name} (empty or invalid)")),
            Err(_) => problems.push(name.to_owned()),
        };
        inspect("common.lsp");
        inspect("repair.ini");
        for surface in ["gravel", "tarmac", "snow"] {
            inspect(&format!("{surface}.lsp"));
            inspect(&format!("r_{surface}.lsp"));
            inspect(&format!("setups/d_{surface}.lsp"));
        }
        let mut metadata_id = String::new();
        let mut association = "pending";
        if let (Some(metadata), Some(name)) = (&metadata, &d.rsf_pack) {
            let mut matches = metadata
                .cars
                .iter()
                .filter(|car| car.pack.eq_ignore_ascii_case(name));
            if let (Some(car), None) = (matches.next(), matches.next()) {
                metadata_id = format!("{}/{}", car.id, car.model);
                let model = metadata.models.iter().find(|model| model.id == car.model);
                if model.is_some_and(|m| {
                    rerbr_content::same_path(
                        &format!("Cars/{}/{}.sgc", m.path, m.filename),
                        &d.model,
                    )
                }) && entry.has_3d_model
                    && !entry.model_missing
                    && !entry.using_fallback_model
                {
                    association = "verified";
                }
            }
        }
        if !problems.is_empty() || !matches!(state, PackAuditState::Ready { .. }) {
            association = "incomplete";
        }
        let pack_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        emit(
            "physics",
            &[
                &d.id,
                association,
                &pack_path,
                &problems.join("; "),
                &metadata_id,
            ],
        );
        if let Ok(evidence) = inspect_car_pack(path) {
            for surface in evidence.surfaces {
                emit(
                    "surface",
                    &[
                        &d.id,
                        &surface.surface,
                        &surface.car_id.map_or(String::new(), |v| v.to_string()),
                        &surface.common_sha256,
                        surface.surface_sha256.as_deref().unwrap_or(""),
                        surface.options_sha256.as_deref().unwrap_or(""),
                        evidence.title.as_deref().unwrap_or(""),
                    ],
                );
            }
        }
    }
}
