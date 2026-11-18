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
    car_parts,
    cars::{self, CarEntry},
    ini, rsf,
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
    pub model_missing: bool,
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

fn collect_shader_textures(rbr_root: &Path, shader_file: &str) -> Vec<String> {
    let Some(resolved) = cars::resolve(rbr_root, shader_file) else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(resolved) else {
        return Vec::new();
    };
    let Ok(sections) = ini::parse(&String::from_utf8_lossy(&bytes)) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for section in &sections {
        if let Some(path) = car_parts::diffuse_path(&sections, &section.name) {
            let norm = path.replace('\\', "/");
            if rerbr_content::validate_relative_path(&norm).is_ok()
                && seen.insert(norm.to_ascii_lowercase())
            {
                out.push(norm);
            }
        }
    }
    out
}

fn is_focus_2006_rsf_car(id: &str, car: &rsf::Car) -> bool {
    if id == "ford-focus-rs-wrc-2006" {
        return true;
    }
    let hay = format!("{} {}", car.name, car.pack).to_ascii_lowercase();
    hay.contains("ford") && hay.contains("focus") && hay.contains("2006")
}

fn focus_2006_preset_entry() -> (CarEntry, VehicleDescriptor) {
    let f = VehicleDescriptor::ford_focus_2006();
    let entry = CarEntry {
        index: car_pack::SLOT as usize,
        file_name: f.model.clone(),
        ini_file: f.parts.clone(),
        shader_file: f.shaders.clone(),
        shader_settings: f
            .shader_settings
            .strip_suffix("_clear.ini")
            .unwrap_or(&f.shader_settings)
            .to_string(),
        texture_path: "Cars/FORD_FOCUS_2006/textures".into(),
    };
    (entry, f)
}

fn try_preset_or_folder_entry(
    rbr_root: &Path,
    car: &rsf::Car,
) -> Option<(CarEntry, Option<String>)> {
    let id = slugify_id(&car.name);
    if is_focus_2006_rsf_car(&id, car) {
        let (entry, _) = focus_2006_preset_entry();
        if cars::resolve(rbr_root, &entry.file_name).is_some() {
            return Some((
                entry,
                cars::resolve(rbr_root, "AudioFMOD/Focus_Mk2_WRC.bank")
                    .map(|_| "AudioFMOD/Focus_Mk2_WRC.bank".into()),
            ));
        }
    }
    // Look for a matching folder directly under `Cars/` when `carmodels.json` is absent.
    let cars_dir = cars::resolve(rbr_root, "Cars").filter(|p| p.is_dir())?;
    let pack_lower = car.pack.to_ascii_lowercase();
    let tokens: Vec<String> = car
        .name
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect();
    if let Ok(entries) = std::fs::read_dir(&cars_dir) {
        for dir_entry in entries.flatten() {
            let folder_path = dir_entry.path();
            if !folder_path.is_dir() {
                continue;
            }
            let Some(folder_name) = folder_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let folder_lower = folder_name.to_ascii_lowercase();
            let folder_compact: String = folder_lower
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect();
            let matched = pack_lower.contains(&folder_compact)
                || (!tokens.is_empty()
                    && tokens
                        .iter()
                        .filter(|t| folder_lower.contains(t.as_str()))
                        .count()
                        >= 2);
            if !matched {
                continue;
            }
            if let Ok(files) = std::fs::read_dir(&folder_path) {
                for f in files.flatten() {
                    let p = f.path();
                    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sgc")) {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            let entry = car_pack::model_entry(folder_name, stem);
                            return Some((entry, None));
                        }
                    }
                }
            }
        }
    }
    None
}

struct DescriptorMeta<'a> {
    id: String,
    name: String,
    category: Option<String>,
    pack_name: String,
    forward_gears: u32,
    audio_index: &'a str,
}

fn descriptor_from_entry(
    rbr_root: &Path,
    meta: DescriptorMeta<'_>,
    entry: &CarEntry,
) -> VehicleDescriptor {
    let shader_settings = if entry.shader_settings.to_ascii_lowercase().ends_with(".ini") {
        entry.shader_settings.replace('\\', "/")
    } else {
        format!("{}_clear.ini", entry.shader_settings.replace('\\', "/"))
    };
    let focus_preset = VehicleDescriptor::ford_focus_2006();
    let is_focus_own_model = meta.id == focus_preset.id
        && rerbr_content::same_path(&entry.file_name, &focus_preset.model);
    let (required_textures, optional_textures, audio_index) = if is_focus_own_model {
        let audio = if meta.audio_index == "Audio/Cars/Cars.ini" {
            focus_preset.audio_index.clone()
        } else {
            meta.audio_index.to_string()
        };
        (
            focus_preset.required_textures.clone(),
            focus_preset.optional_textures.clone(),
            audio,
        )
    } else {
        let mut req = collect_shader_textures(rbr_root, &entry.shader_file);
        if req.is_empty() {
            let tex_dir = entry.texture_path.replace('\\', "/");
            let tex_dir = tex_dir.trim_end_matches('/');
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
                        req.push(candidate);
                    }
                }
            }
        }
        (req, Vec::new(), meta.audio_index.to_string())
    };
    VehicleDescriptor {
        id: meta.id,
        name: meta.name,
        original_slot: car_pack::SLOT,
        tyre_type: 4,
        forward_gears: meta.forward_gears.clamp(1, 8),
        index: "Cars/Cars.ini".into(),
        model: entry.file_name.replace('\\', "/"),
        parts: entry.ini_file.replace('\\', "/"),
        shaders: entry.shader_file.replace('\\', "/"),
        shader_settings,
        required_textures,
        optional_textures,
        audio_index,
        category: meta.category,
        rsf_pack: Some(meta.pack_name),
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

fn expected_own_model_entry(install: Option<&rsf::Install>, car: &rsf::Car) -> CarEntry {
    if let Some(inst) = install {
        if let Some(cm) = inst.models.iter().find(|m| m.id == car.model) {
            return car_pack::model_entry(&cm.path, &cm.filename);
        }
    }
    let slug = car.pack.trim_end_matches("_ngp6").trim_end_matches("_ngp7");
    car_pack::model_entry(slug, slug)
}

struct ResolvedModel {
    entry: CarEntry,
    model_missing: bool,
    audio_hint: Option<String>,
}

fn resolve_model_entry(
    rbr_root: &Path,
    install: Option<&rsf::Install>,
    car: &rsf::Car,
    retail: &[CarEntry],
    notes: &mut Vec<String>,
) -> ResolvedModel {
    if let Some(inst) = install {
        match inst.model_entry(car, retail) {
            Ok(m) if m.fallback.is_none() => {
                return ResolvedModel {
                    entry: m.entry,
                    model_missing: false,
                    audio_hint: None,
                };
            }
            Ok(m) => {
                if let Some((direct_entry, audio_hint)) = try_preset_or_folder_entry(rbr_root, car)
                {
                    return ResolvedModel {
                        entry: direct_entry,
                        model_missing: false,
                        audio_hint,
                    };
                }
                if let Some(reason) = m.fallback {
                    notes.push(format!(
                        "MODELO 3D PROPIO AUSENTE (BLOQUEADO SIN FALLBACK): {reason}"
                    ));
                }
                return ResolvedModel {
                    entry: expected_own_model_entry(Some(inst), car),
                    model_missing: true,
                    audio_hint: None,
                };
            }
            Err(e) => {
                if let Some((direct_entry, audio_hint)) = try_preset_or_folder_entry(rbr_root, car)
                {
                    return ResolvedModel {
                        entry: direct_entry,
                        model_missing: false,
                        audio_hint,
                    };
                }
                notes.push(format!("MODELO 3D PROPIO AUSENTE (BLOQUEADO): {e}"));
                return ResolvedModel {
                    entry: expected_own_model_entry(Some(inst), car),
                    model_missing: true,
                    audio_hint: None,
                };
            }
        }
    }
    if let Some((direct_entry, audio_hint)) = try_preset_or_folder_entry(rbr_root, car) {
        return ResolvedModel {
            entry: direct_entry,
            model_missing: false,
            audio_hint,
        };
    }
    notes.push("SIN MODELO 3D PROPIO EN Cars/ (BLOQUEADO SIN FALLBACK)".into());
    ResolvedModel {
        entry: expected_own_model_entry(None, car),
        model_missing: true,
        audio_hint: None,
    }
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

    let using_fallback_model = false;
    let ResolvedModel {
        entry,
        model_missing,
        audio_hint: fallback_audio_hint,
    } = resolve_model_entry(rbr_root, install, car, retail, &mut notes);

    let fmod_bank = car
        .audio
        .as_deref()
        .map(|a| format!("AudioFMOD/{a}.bank"))
        .filter(|p| rerbr_content::validate_relative_path(p).is_ok())
        .or(fallback_audio_hint);
    let fmod_present = fmod_bank
        .as_deref()
        .and_then(|p| cars::resolve(rbr_root, p))
        .is_some();
    if fmod_bank.is_none() {
        notes.push("SIN BANCO FMOD ASIGNADO EN cars.json (NO USA AUDIO RETAIL)".into());
    } else if !fmod_present {
        notes.push(format!(
            "FALTA BANCO FMOD {}",
            fmod_bank.as_deref().unwrap_or("")
        ));
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

    let own_model_ok = !model_missing && !using_fallback_model;
    let classification = match (&pack_readable, own_model_ok) {
        (Ok(_), true) if !is_ngp7_pack && fmod_present && ngp_err.is_none() => {
            VehicleClassification::RsfNgpVerified
        }
        (Ok(_), true) => VehicleClassification::RsfNgpPartial,
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

    let audio_index = fmod_bank
        .clone()
        .unwrap_or_else(|| "AudioFMOD/SIN_BANCO_FMOD.bank".into());
    let descriptor = descriptor_from_entry(
        rbr_root,
        DescriptorMeta {
            id,
            name: display_name,
            category: car.group.clone(),
            pack_name: car.pack.clone(),
            forward_gears,
            audio_index: &audio_index,
        },
        &entry,
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
        model_missing,
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

    fn write_synthetic_focus_install(root: &Path) {
        let rbr = root.join("RBR");
        let focus = VehicleDescriptor::ford_focus_2006();
        std::fs::create_dir_all(rbr.join("Cars/impreza03")).unwrap();
        std::fs::write(
            rbr.join("Cars/Cars.ini"),
            "[Car05]\r\nFileName = \"Cars\\impreza03\\impreza03.sgc\"\r\nIniFile = \"Cars\\impreza03\\impreza03.ini\"\r\nShaderFile = \"Cars\\impreza03\\impreza03_shaders.ini\"\r\nShaderSettings = \"Cars\\impreza03\\impreza03_shader_settings\"\r\nTexturePath = \"Cars\\impreza03\\Textures\\\"\r\n",
        )
        .unwrap();
        std::fs::write(rbr.join("Cars/impreza03/impreza03.sgc"), b"impreza").unwrap();

        let focus_dir = rbr.join("Cars/FORD_FOCUS_2006/textures");
        std::fs::create_dir_all(&focus_dir).unwrap();
        std::fs::write(rbr.join(&focus.model), b"focus").unwrap();
        std::fs::write(rbr.join(&focus.parts), b"[Cam_external]\nFOV=1.2\n").unwrap();
        std::fs::write(
            rbr.join(&focus.shaders),
            "[TSDefault]\r\nfocus_wrc06_external.dds = Cars\\FORD_FOCUS_2006\\textures\\focus_wrc06_external.dds\r\n",
        )
        .unwrap();
        std::fs::write(rbr.join(&focus.shader_settings), b"[CarPaint]\r\n").unwrap();
        for tex in &focus.required_textures {
            std::fs::write(rbr.join(tex), b"DDS ").unwrap();
        }

        let pack = rbr.join("rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(pack.join("common.lsp"), b"CarName = Ford Focus\n").unwrap();
        std::fs::write(pack.join("gravel.lsp"), b"NumberOfGears = 5\n").unwrap();
    }

    #[test]
    fn focus_2006_never_falls_back_to_impreza_without_carmodels_json() {
        let root =
            std::env::temp_dir().join(format!("rerbr-focus-no-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write_synthetic_focus_install(&root);

        let discovered = discover(&root.join("RBR"));
        let focus = discovered
            .iter()
            .find(|d| d.descriptor.id == "ford-focus-rs-wrc-2006")
            .expect("Focus pack should be discovered even without rsfdata/cache JSONs");
        assert_eq!(focus.descriptor.model, "Cars/FORD_FOCUS_2006/focus06.sgc");
        assert_eq!(focus.descriptor.parts, "Cars/FORD_FOCUS_2006/focus06.ini");
        assert_eq!(
            focus.descriptor.shaders,
            "Cars/FORD_FOCUS_2006/focus06_shaders.ini"
        );
        assert_eq!(
            focus.descriptor.shader_settings,
            "Cars/FORD_FOCUS_2006/focus06_shader_settings_clear.ini"
        );
        assert!(!focus.using_fallback_model);

        let catalog = super::super::content_manager::Catalog::scan_root(
            &root,
            &rerbr_content::Selection::default(),
        );
        let entry = catalog.vehicle("ford-focus-rs-wrc-2006").unwrap();
        assert_eq!(entry.descriptor.model, "Cars/FORD_FOCUS_2006/focus06.sgc");
        assert!(!entry.using_fallback_model);

        let report = super::super::ngp_experimental::FocusDiagnosticsReport::inspect(
            Some(&root),
            &entry.descriptor,
        );
        let car_files = report
            .steps
            .iter()
            .find(|s| s.stage == "CAR FILES")
            .unwrap();
        assert_eq!(
            car_files.status,
            super::super::ngp_experimental::DiagnosticStatus::Ok
        );
        let car_pack = report.steps.iter().find(|s| s.stage == "CAR PACK").unwrap();
        assert_eq!(
            car_pack.status,
            super::super::ngp_experimental::DiagnosticStatus::Ok
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn focus_2006_prefers_installed_focus_folder_over_carmodels_fallback_chain() {
        let root =
            std::env::temp_dir().join(format!("rerbr-focus-cache-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write_synthetic_focus_install(&root);

        let cache = root.join("RBR/rsfdata/cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("cars.json"),
            r#"[{"id":"55","name":"Ford Focus Mk II RS WRC 2006","path":"Ford_Focus_Mk_II_RS_WRC_2006_ngp6","carmodel_id":"17","audio":"Focus_Mk2_WRC"}]"#,
        )
        .unwrap();
        std::fs::write(
            cache.join("carmodels.json"),
            r#"[{"id":"17","name":"Ford Focus Missing Chain","path":"MISSING_FOCUS_FOLDER","filename":"missing","fallback":"5"}]"#,
        )
        .unwrap();

        let discovered = discover(&root.join("RBR"));
        let focus = discovered
            .iter()
            .find(|d| d.descriptor.id == "ford-focus-rs-wrc-2006")
            .expect("Focus pack should be discovered");
        assert_eq!(focus.descriptor.model, "Cars/FORD_FOCUS_2006/focus06.sgc");
        assert!(!focus.using_fallback_model);

        let catalog = super::super::content_manager::Catalog::scan_root(
            &root,
            &rerbr_content::Selection::default(),
        );
        let entry = catalog.vehicle("ford-focus-rs-wrc-2006").unwrap();
        assert_eq!(entry.descriptor.model, "Cars/FORD_FOCUS_2006/focus06.sgc");
        assert!(!entry.using_fallback_model);
        let _ = std::fs::remove_dir_all(&root);
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

    #[test]
    fn audits_and_boots_all_installed_rsf_cars_if_pc_present() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        if !rbr_root.join("rsfdata/cache/cars.json").is_file() {
            return;
        }
        let discovered = discover(rbr_root);
        let paths = rerbr_sim::inputs::Paths {
            install: rbr_root.to_path_buf(),
            overlay: None,
            maps: rbr_root.join("Maps"),
        };
        let pack_root = rbr_root.join("rsfdata/cars");

        for d in &discovered {
            let model_res = crate::car::load_vehicle_for_mode(rbr_root, &d.descriptor, true);
            let fmod_res = d.fmod_bank.as_deref().map(|bank_rel| {
                super::super::fmod_synth::FmodVehicleAudio::load_from_documents(
                    rbr_root, bank_rel, 44100,
                )
            });
            let pack_name = d.descriptor.rsf_pack.as_deref().unwrap_or("");
            let pack_dir = pack_root.join(pack_name);
            let sel = rerbr_content::Selection {
                vehicle: d.descriptor.clone(),
                stage: rerbr_content::StageDescriptor::harwood_forest(),
            };
            let sim_res = if pack_dir.is_dir() {
                Some(rerbr_sim::drive::Session::boot_experimental_pack(
                    &paths, &sel, &pack_dir, false,
                ))
            } else {
                None
            };
            let (model_str, tex_count) = match &model_res {
                Ok(car) => (
                    format!("OK({} draws)", car.geometry.drawn.len()),
                    car.textures.len(),
                ),
                Err(e) => (format!("ERR({e})"), 0),
            };
            let fmod_str = match &fmod_res {
                Some(Ok(synth)) => {
                    let st = synth.stats();
                    format!("OK({}/{} decoded)", st.decoded_samples, st.total_samples)
                }
                Some(Err(e)) => format!("ERR({e})"),
                None => "NONE".into(),
            };
            let sim_str = match &sim_res {
                Some(Ok(_)) => "BOOT OK".into(),
                Some(Err(e)) => format!("BOOT ERR({e})"),
                None => "NO PACK DIR".into(),
            };
            eprintln!(
                "RSF FULL AUDIT [{}]: model={}({}) tex={} fmod={} sim={} fallback={}",
                d.descriptor.id,
                d.descriptor.model,
                model_str,
                tex_count,
                fmod_str,
                sim_str,
                d.using_fallback_model
            );
        }
    }

    #[test]
    fn imported_rsf_car_never_silently_substitutes_model_or_retail_audio_when_incomplete() {
        let root =
            std::env::temp_dir().join(format!("rerbr-rsf-no-silent-fb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write_synthetic_focus_install(&root);

        let cache = root.join("RBR/rsfdata/cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("cars.json"),
            r#"[
                {"id":"55","name":"Ford Focus Mk II RS WRC 2006","path":"Ford_Focus_Mk_II_RS_WRC_2006_ngp6","carmodel_id":"17","audio":"Focus_Mk2_WRC"},
                {"id":"99","name":"Peugeot 306 Maxi Kit Car","path":"Peugeot_306_Maxi_Kit_Car_ngp6","carmodel_id":"48","audio":"306_maxi_B"},
                {"id":"100","name":"Citroen DS3 WRC","path":"Citroen_DS3_WRC_ngp6","carmodel_id":"78","audio":null}
            ]"#,
        )
        .unwrap();
        std::fs::write(
            cache.join("carmodels.json"),
            r#"[
                {"id":"17","name":"Ford Focus","path":"FORD_FOCUS_2006","filename":"focus06","fallback":"5"},
                {"id":"48","name":"Peugeot 306 Maxi","path":"p306maxi","filename":"p306maxi","fallback":"5"},
                {"id":"78","name":"Citroen DS3 WRC","path":"DS3_WRC","filename":"ds3_wrc","fallback":"5"}
            ]"#,
        )
        .unwrap();

        let maxi_pack = root.join("RBR/rsfdata/cars/Peugeot_306_Maxi_Kit_Car_ngp6");
        std::fs::create_dir_all(&maxi_pack).unwrap();
        std::fs::write(
            maxi_pack.join("common.lsp"),
            b"CarName = Peugeot 306 Maxi\n",
        )
        .unwrap();
        std::fs::write(maxi_pack.join("gravel.lsp"), b"NumberOfGears = 6\n").unwrap();
        // Deliberately omit Cars/p306maxi -> must NOT substitute impreza03 or focus06!

        let ds3_pack = root.join("RBR/rsfdata/cars/Citroen_DS3_WRC_ngp6");
        std::fs::create_dir_all(&ds3_pack).unwrap();
        std::fs::write(ds3_pack.join("common.lsp"), b"CarName = DS3\n").unwrap();
        std::fs::write(ds3_pack.join("gravel.lsp"), b"NumberOfGears = 6\n").unwrap();
        std::fs::create_dir_all(root.join("RBR/Cars/DS3_WRC")).unwrap();
        std::fs::write(root.join("RBR/Cars/DS3_WRC/ds3_wrc.sgc"), b"ds3").unwrap();

        let catalog = super::super::content_manager::Catalog::scan_root(
            &root,
            &rerbr_content::Selection::default(),
        );
        let maxi = catalog.vehicle("rsf-peugeot-306-maxi-kit-car").unwrap();
        assert_eq!(maxi.descriptor.model, "Cars/p306maxi/p306maxi.sgc");
        assert!(maxi.model_missing);
        assert!(!maxi.using_fallback_model);
        assert!(!maxi.files.missing.is_empty());
        assert_eq!(
            maxi.state,
            super::super::content_manager::EntryState::Unsupported
        );

        let ds3 = catalog.vehicle("rsf-citroen-ds3-wrc").unwrap();
        assert_eq!(ds3.descriptor.model, "Cars/DS3_WRC/ds3_wrc.sgc");
        assert!(!ds3.model_missing);
        assert!(
            !ds3.descriptor.audio_index.contains("Cars.ini"),
            "RSF car without audio bank must never fall back to retail Cars.ini: {}",
            ds3.descriptor.audio_index
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
