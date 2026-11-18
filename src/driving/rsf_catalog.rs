//! Cleanroom NGPCarMenu / RBRCIT-inspired dynamic RSF vehicle discovery & classification.
//!
//! Reads the user's private `Documents/RBR/rsfdata/cache/{cars.json, carmodels.json, cargroups.json,
//! cars_data.json}` and `Documents/RBR/rsfdata/cars/` via `rerbr_sim::rbr_formats::rsf::Install`,
//! correlates each pack with its 3D model, gearbox, FMOD audio bank, and NGP runtime compatibility,
//! and emits validated descriptors without hardcoding individual RSF cars.

use super::ngp_proto::{inspect_pack_path_result, VehicleTechSheet};
use rerbr_content::{VehicleClassification, VehicleDescriptor};
use rerbr_sim::rbr_formats::{
    car_pack::{self, Pack},
    cars::{self, CarEntry},
    rsf,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredRsfVehicle {
    pub descriptor: VehicleDescriptor,
    pub classification: VehicleClassification,
    pub tech_sheet: Option<VehicleTechSheet>,
    pub drivetrain: Option<String>,
    pub year: Option<u32>,
    pub fmod_bank: Option<String>,
    pub fmod_present: bool,
    pub using_fallback_model: bool,
    pub notes: Vec<String>,
}

/// Converts an arbitrary RSF car name / pack folder into a safe lowercase `VehicleDescriptor::id`.
pub fn slugify_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = true;
    for b in raw.bytes() {
        let ch = b.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(char::from(ch));
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "rsf-car".into()
    } else if out == "ford-focus-mk-ii-rs-wrc-2006"
        || out.starts_with("ford-focus-mk-ii-rs-wrc-2006-")
    {
        "ford-focus-rs-wrc-2006".into()
    } else if out.starts_with("rsf-") {
        out
    } else {
        format!("rsf-{out}")
    }
}

fn check_ngp_74_runtime(rbr_root: &Path) -> Result<(), String> {
    let dll = cars::resolve(rbr_root, "Plugins/NGP.dll")
        .ok_or_else(|| "Plugins/NGP.dll no encontrado".to_string())?;
    let rbz = cars::resolve(rbr_root, "physics.rbz");
    if rbz.is_none() {
        return Err("physics.rbz no encontrado".into());
    }
    let bytes = std::fs::read(dll).map_err(|e| format!("NGP.dll: {e}"))?;
    rerbr_sim::ngp_candidate::validate_runtime(&bytes)?;
    Ok(())
}

fn descriptor_from_entry(
    id: String,
    name: String,
    category: Option<String>,
    pack_name: String,
    forward_gears: u32,
    entry: &CarEntry,
    audio_index: String,
) -> VehicleDescriptor {
    let shader_settings = if entry.shader_settings.to_ascii_lowercase().ends_with(".ini") {
        entry.shader_settings.replace('\\', "/")
    } else {
        format!("{}_clear.ini", entry.shader_settings.replace('\\', "/"))
    };
    let tex_dir = entry.texture_path.replace('\\', "/");
    let tex_dir = tex_dir.trim_end_matches('/');
    let mut required_textures = Vec::new();
    if let Some(stem) = entry
        .file_name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .and_then(|s| s.strip_suffix(".sgc").or_else(|| s.strip_suffix(".SGC")))
    {
        for suffix in ["external", "external_dynamic", "external_transparent"] {
            let candidate = format!("{tex_dir}/{stem}_{suffix}.dds");
            if rerbr_content::validate_relative_path(&candidate).is_ok() {
                required_textures.push(candidate);
            }
        }
    }
    VehicleDescriptor {
        id,
        name,
        original_slot: car_pack::SLOT,
        tyre_type: 4,
        forward_gears: forward_gears.clamp(1, 8),
        index: "Cars/Cars.ini".into(),
        model: entry.file_name.replace('\\', "/"),
        parts: entry.ini_file.replace('\\', "/"),
        shaders: entry.shader_file.replace('\\', "/"),
        shader_settings,
        required_textures,
        optional_textures: Vec::new(),
        audio_index,
        category,
        rsf_pack: Some(pack_name),
    }
}

/// Discovers installed RSF NGP vehicles under `rbr_root` (`Documents/RBR`) dynamically.
/// Safe and read-only: returns empty `Vec` cleanly when RSF folders are absent.
pub fn discover(rbr_root: &Path) -> Vec<DiscoveredRsfVehicle> {
    let rsf_cars_dir = cars::resolve(rbr_root, "rsfdata/cars")
        .or_else(|| cars::resolve(rbr_root, "cars"))
        .filter(|p| p.is_dir());
    let Some(packs_root) = rsf_cars_dir else {
        return Vec::new();
    };
    if !rbr_root.join("rsfdata").exists() && packs_root.ends_with("Cars") {
        // Skip retail `Cars/` directory if `rsfdata/` doesn't exist.
        return Vec::new();
    }

    let ngp_runtime = check_ngp_74_runtime(rbr_root);
    let retail = cars::resolve(rbr_root, "Cars/Cars.ini")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| cars::parse(&t).ok())
        .unwrap_or_default();

    let mut out = Vec::new();
    let mut seen_packs: BTreeSet<String> = BTreeSet::new();
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();

    if let Ok(install) = rsf::Install::open(rbr_root) {
        for car in &install.cars {
            let Some(pack_path) = resolve_pack_in(&packs_root, &car.pack) else {
                continue;
            };
            seen_packs.insert(car.pack.to_ascii_lowercase());
            let mut item = build_rsf_entry(
                rbr_root,
                Some(&install),
                car,
                &pack_path,
                &retail,
                ngp_runtime.as_ref().err().cloned(),
            );
            dedup_id(&mut item.descriptor.id, &mut seen_ids);
            if item.descriptor.validate_schema().is_ok() {
                out.push(item);
            }
        }
    }

    // Directory fallback for custom NGP packs placed directly in `rsfdata/cars/` without `cars.json`
    if let Ok(entries) = std::fs::read_dir(&packs_root) {
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let Some(raw_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let pack_name = raw_name
                .strip_suffix(".zip")
                .or_else(|| raw_name.strip_suffix(".ZIP"))
                .unwrap_or(raw_name)
                .to_string();
            if seen_packs.contains(&pack_name.to_ascii_lowercase()) {
                continue;
            }
            if !path.is_dir() && !raw_name.to_ascii_lowercase().ends_with(".zip") {
                continue;
            }
            if Pack::open(&path).is_err() {
                continue;
            }
            seen_packs.insert(pack_name.to_ascii_lowercase());
            let synthetic_car = rsf::Car {
                id: 0,
                name: pack_name.replace('_', " "),
                pack: pack_name,
                group: Some("RSF CUSTOM".into()),
                model: 0,
                revision: String::new(),
                audio: None,
                drive_train: None,
                year: None,
            };
            let mut item = build_rsf_entry(
                rbr_root,
                None,
                &synthetic_car,
                &path,
                &retail,
                ngp_runtime.as_ref().err().cloned(),
            );
            dedup_id(&mut item.descriptor.id, &mut seen_ids);
            if item.descriptor.validate_schema().is_ok() {
                out.push(item);
            }
        }
    }

    out.sort_by(|a, b| a.descriptor.name.cmp(&b.descriptor.name));
    out
}

fn dedup_id(id: &mut String, seen: &mut BTreeSet<String>) {
    if seen.insert(id.clone()) {
        return;
    }
    for n in 2..99 {
        let candidate = format!("{id}-{n}");
        if seen.insert(candidate.clone()) {
            *id = candidate;
            return;
        }
    }
}

fn resolve_pack_in(packs_root: &Path, pack: &str) -> Option<PathBuf> {
    cars::resolve(packs_root, pack).or_else(|| cars::resolve(packs_root, &format!("{pack}.zip")))
}

fn build_rsf_entry(
    rbr_root: &Path,
    install: Option<&rsf::Install>,
    car: &rsf::Car,
    pack_path: &Path,
    retail: &[CarEntry],
    ngp_err: Option<String>,
) -> DiscoveredRsfVehicle {
    let mut notes = Vec::new();
    let tech_sheet = match inspect_pack_path_result(pack_path) {
        Ok(sheet) => Some(sheet),
        Err(err) => {
            notes.push(format!("TECH SHEET: {err}"));
            None
        }
    };
    let pack_readable = match Pack::open(pack_path) {
        Ok(p) => match (p.read("common.lsp"), p.read("gravel.lsp")) {
            (Ok(_), Ok(_)) => Ok(p),
            (Err(e), _) | (_, Err(e)) => Err(e),
        },
        Err(e) => Err(e),
    };

    let mut using_fallback_model = false;
    let model_entry = if let Some(inst) = install {
        match inst.model_entry(car, retail) {
            Ok(m) => {
                if let Some(ref reason) = m.fallback {
                    using_fallback_model = true;
                    notes.push(format!("MODELO FALLBACK: {reason}"));
                }
                Ok(m.entry)
            }
            Err(e) => Err(e),
        }
    } else if let Some(retail_slot1) = retail.iter().find(|c| c.index == car_pack::SLOT as usize) {
        using_fallback_model = true;
        notes.push("MODELO FALLBACK SLOT 5 (SIN carmodels.json)".into());
        Ok(retail_slot1.clone())
    } else {
        Err("SIN MODELO 3D".into())
    };

    let fmod_bank = car
        .audio
        .as_deref()
        .map(|a| format!("AudioFMOD/{a}.bank"))
        .filter(|p| rerbr_content::validate_relative_path(p).is_ok());
    let fmod_present = fmod_bank
        .as_deref()
        .and_then(|p| cars::resolve(rbr_root, p))
        .is_some();
    if fmod_bank.is_none() || !fmod_present {
        notes.push("SIN BANCO FMOD".into());
    }

    let is_ngp7_pack = car.pack.to_ascii_lowercase().ends_with("_ngp7");
    if is_ngp7_pack {
        notes.push("PACK NGP7 EJECUTADO SOBRE BACKEND NGP 7.4".into());
    }
    if let Some(ref e) = ngp_err {
        notes.push(e.clone());
    }
    if let Err(ref e) = pack_readable {
        notes.push(e.clone());
    }
    if let Err(ref e) = model_entry {
        notes.push(e.clone());
    }

    let classification = match (&pack_readable, &ngp_err, &model_entry) {
        (Err(_), _, Ok(_)) | (Ok(_), Some(_), Ok(_)) => VehicleClassification::VisualInspectionOnly,
        (Ok(_), None, Ok(_)) if !using_fallback_model && !is_ngp7_pack => {
            VehicleClassification::RsfNgpVerified
        }
        (Ok(_), None, Ok(_)) => VehicleClassification::RsfNgpPartial,
        _ => VehicleClassification::VisualInspectionOnly,
    };

    let forward_gears = tech_sheet.as_ref().map(|t| t.forward_gears).unwrap_or(5);
    let display_name = pack_readable
        .as_ref()
        .ok()
        .and_then(|p| p.title())
        .filter(|t| !t.is_empty() && !car.name.is_empty())
        .map(|_| car.name.clone())
        .unwrap_or_else(|| car.name.clone());
    let id = slugify_id(&display_name);

    let entry = model_entry.unwrap_or_else(|_| {
        let f = VehicleDescriptor::ford_focus_2006();
        CarEntry {
            index: car_pack::SLOT as usize,
            file_name: f.model,
            ini_file: f.parts,
            shader_file: f.shaders,
            shader_settings: f.shader_settings,
            texture_path: "Cars/FORD_FOCUS_2006/textures".into(),
        }
    });

    let descriptor = descriptor_from_entry(
        id,
        display_name,
        car.group.clone(),
        car.pack.clone(),
        forward_gears,
        &entry,
        fmod_bank
            .clone()
            .unwrap_or_else(|| "Audio/Cars/Cars.ini".into()),
    );

    let drivetrain = car.drive_train.clone().or_else(|| {
        tech_sheet
            .as_ref()
            .map(|t| t.drivetrain.label().to_string())
    });

    DiscoveredRsfVehicle {
        descriptor,
        classification,
        tech_sheet,
        drivetrain,
        year: car.year,
        fmod_bank,
        fmod_present,
        using_fallback_model,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_keeps_focus_preset_id_and_sanitizes_names() {
        assert_eq!(
            slugify_id("Ford Focus Mk II RS WRC 2006"),
            "ford-focus-rs-wrc-2006"
        );
        assert_eq!(slugify_id("Citroen DS3 WRC"), "rsf-citroen-ds3-wrc");
        assert_eq!(
            slugify_id("Toyota GR Yaris Rally2"),
            "rsf-toyota-gr-yaris-rally2"
        );
    }

    #[test]
    fn private_rsf_install_discovery_if_present() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        if !rbr_root.join("rsfdata/cache/cars.json").is_file() {
            return;
        }
        let discovered = discover(rbr_root);
        assert!(
            !discovered.is_empty(),
            "expected installed RSF cars in C:\\Richard Burns Rally"
        );
        for d in &discovered {
            d.descriptor.validate_schema().unwrap();
            assert!(d.descriptor.validate_experimental_ngp().is_ok());
            assert!(d.descriptor.validate_current_backend().is_err());
            eprintln!(
                "RSF DISCOVERED: id={} name={:?} cat={:?} gears={} dt={:?} class={:?} fmod={}({}) notes={:?}",
                d.descriptor.id,
                d.descriptor.name,
                d.descriptor.category,
                d.descriptor.forward_gears,
                d.drivetrain,
                d.classification,
                d.fmod_bank.as_deref().unwrap_or("-"),
                d.fmod_present,
                d.notes
            );
        }
    }
}
