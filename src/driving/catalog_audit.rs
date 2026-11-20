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
