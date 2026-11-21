//! Original Corolla visual loader. No game resources, physics or DirectX shaders.
pub mod cockpit;
pub(crate) mod gpu;
pub mod materials;
pub mod mesh;
pub mod placement;
pub(crate) mod runtime;
pub mod selection;
#[cfg(feature = "physics-drive")]
pub(crate) mod showroom;

use crate::{
    asset_probe::{TextureData, TextureInfo},
    harwood::texture::upload_plan,
};
use rerbr_asset_formats::{cars, dds, ini, sgc};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone)]
pub struct LoadedCarReport {
    pub vehicle_id: String,
    pub model_path: String,
    pub draw_count: usize,
    pub textures: Vec<String>,
    pub has_cockpit_interior: bool,
}

static LAST_LOADED_CAR: OnceLock<Mutex<Option<LoadedCarReport>>> = OnceLock::new();

pub fn last_loaded_report() -> Option<LoadedCarReport> {
    LAST_LOADED_CAR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.clone())
}

pub struct Car {
    pub vehicle: rerbr_content::VehicleDescriptor,
    #[cfg(feature = "physics-stage")]
    pub cameras: crate::driving::cameras::Mounts,
    pub geometry: mesh::Geometry,
    pub(crate) textures: BTreeMap<String, crate::asset_probe::TextureData>,
    pub files: Vec<String>,
    pub cockpit: Option<rerbr_render_core::Mat4>,
    pub cockpit_error: Option<String>,
}
pub struct Prepared {
    pub selection: rerbr_content::Selection,
    pub terrain: crate::harwood::LoadedBlock,
    pub path: placement::ChasePath,
    pub car: Car,
    #[cfg(feature = "physics-stage")]
    pub stage_cameras: Option<crate::driving::replay::StageCameras>,
}
pub fn prepare(install: &Path, maps: &Path) -> Result<Prepared, String> {
    prepare_selected(install, maps, &rerbr_content::Selection::default())
}
pub fn prepare_selected(
    install: &Path,
    maps: &Path,
    selection: &rerbr_content::Selection,
) -> Result<Prepared, String> {
    prepare_selected_for_mode(install, maps, selection, false)
}
pub fn prepare_selected_for_mode(
    install: &Path,
    maps: &Path,
    selection: &rerbr_content::Selection,
    allow_experimental_ngp: bool,
) -> Result<Prepared, String> {
    selection.validate_for_mode(allow_experimental_ngp)?;
    #[cfg(not(feature = "physics-stage"))]
    if selection.stage.id != "harwood-forest" {
        return Err("multiple stages require physics-stage mode".into());
    }
    #[cfg(feature = "physics-stage")]
    if selection.vehicle.is_ngp_experimental_car() {
        eprintln!(
            "experimental-ngp AUDIO: {} configured with FMOD bank ({})",
            selection.vehicle.id, selection.vehicle.audio_index
        );
    } else {
        match rerbr_sim::audio_audit::inspect_vehicle(install, &selection.vehicle) {
            Ok(report) => eprintln!("physics-stage AUDIO BANK CPU PASS: {report:?}; output not implemented; silent milestone"),
            Err(error) => eprintln!("physics-stage AUDIO DISABLED: {error}; silent milestone; geometry can still load"),
        }
    }
    let car = load_vehicle_for_mode(install, &selection.vehicle, allow_experimental_ngp)?;
    if let Ok(mut slot) = LAST_LOADED_CAR.get_or_init(|| Mutex::new(None)).lock() {
        *slot = Some(LoadedCarReport {
            vehicle_id: selection.vehicle.id.clone(),
            model_path: selection.vehicle.model.clone(),
            draw_count: car.geometry.drawn.len(),
            textures: car.textures.keys().cloned().collect(),
            has_cockpit_interior: car.cockpit.is_some(),
        });
    }
    #[cfg(not(feature = "physics-stage"))]
    let (path, render) = {
        let terrain = crate::harwood::benchmark::load(maps)?;
        let path = placement::ChasePath::new(terrain.route, &terrain.render.mesh, &car.geometry)?;
        (path, terrain.render)
    };
    #[cfg(feature = "physics-stage")]
    let (path, render) = {
        let full = crate::harwood::stage::load_selected(maps, &selection.stage)?;
        // Retain the tested Harwood preview. Other stages use live physics;
        // sparse LBS points must never be invented into a benchmark spline.
        let path = if selection.stage.id == "harwood-forest" {
            let route = crate::harwood::benchmark::load(maps)?.route;
            let path = placement::ChasePath::new(route, &full.ground, &car.geometry)?;
            path.validate()?;
            path
        } else {
            placement::ChasePath::for_driving(&full.ground, &car.geometry)?
        };
        (path, full.render)
    };
    #[cfg(not(feature = "physics-stage"))]
    path.validate()?;
    #[cfg(feature = "physics-stage")]
    let stage_cameras =
        crate::driving::replay::StageCameras::load_for_stage(maps, &selection.stage.stem);
    Ok(Prepared {
        selection: selection.clone(),
        terrain: render,
        path,
        car,
        #[cfg(feature = "physics-stage")]
        stage_cameras,
    })
}
fn read(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
    path: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    if path
        .split(['\\', '/'])
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(':'))
    {
        return Err(format!("CAR PATH: invalid {path}"));
    }
    if !vehicle.allows(path) {
        return Err(format!(
            "CAR READ: resource outside selected vehicle profile '{}': {path}",
            vehicle.id
        ));
    }
    let resolved = cars::resolve(root, path).ok_or_else(|| format!("CAR READ: missing {path}"))?;
    let canonical = resolved.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("CAR PATH: escapes private root".into());
    }
    let mut data = Vec::new();
    std::fs::File::open(&canonical)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > limit {
        return Err(format!("CAR READ: {path} exceeds limit"));
    }
    Ok(data)
}
fn sections(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
    path: &str,
) -> Result<Vec<ini::Section>, String> {
    let bytes = read(root, vehicle, path, 1024 * 1024)?;
    ini::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("CAR INI {path}: {e}"))
}

fn prepare_texture(request: &str, resolved: &str, bytes: Vec<u8>) -> Result<TextureData, String> {
    let bytes = dds::to_rgba8(&bytes).unwrap_or(bytes);
    let plan = upload_plan(&bytes, u32::MAX).map_err(|err| format!("CAR DDS {resolved}: {err}"))?;
    let first = plan.mips.first().ok_or("CAR DDS: missing mip chain")?;
    let info = TextureInfo {
        request: request.into(),
        resolved: resolved.into(),
        width: first.width,
        height: first.height,
        mip_count: plan.mips.len() as u32,
        format: plan.format,
    };
    Ok(TextureData { info, bytes })
}

pub fn load(root: &Path) -> Result<Car, String> {
    load_vehicle(root, &rerbr_content::VehicleDescriptor::default())
}
pub fn load_vehicle(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
) -> Result<Car, String> {
    load_vehicle_for_mode(root, vehicle, false)
}
pub fn load_vehicle_for_mode(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
    allow_experimental_ngp: bool,
) -> Result<Car, String> {
    if allow_experimental_ngp {
        vehicle.validate_experimental_ngp()?;
    } else {
        vehicle.validate_current_backend()?;
    }
    let (entry, clear, mut files) = if allow_experimental_ngp && vehicle.is_ngp_experimental_car() {
        let base_settings = vehicle
            .shader_settings
            .strip_suffix("_clear.ini")
            .unwrap_or(&vehicle.shader_settings)
            .to_string();
        let folder = vehicle
            .model
            .rsplit_once('/')
            .map(|(dir, _)| format!("{dir}/textures/"))
            .unwrap_or_else(|| "Cars/FORD_FOCUS_2006/textures/".into());
        let entry = cars::CarEntry {
            index: 5,
            file_name: vehicle.model.clone(),
            ini_file: vehicle.parts.clone(),
            shader_file: vehicle.shaders.clone(),
            shader_settings: base_settings,
            texture_path: folder,
        };
        let files = vec![
            entry.file_name.clone(),
            entry.ini_file.clone(),
            entry.shader_file.clone(),
            vehicle.shader_settings.clone(),
        ];
        (entry, vehicle.shader_settings.clone(), files)
    } else {
        let index = read(root, vehicle, &vehicle.index, 1024 * 1024)?;
        let entries = cars::parse(&String::from_utf8_lossy(&index)).map_err(|e| e.to_string())?;
        let entry = entries
            .iter()
            .find(|e| e.index == vehicle.original_slot as usize)
            .cloned()
            .ok_or_else(|| format!("CAR: [Car{:02}] missing", vehicle.original_slot))?;
        if !rerbr_content::same_path(&entry.file_name, &vehicle.model) {
            return Err(format!("CAR: selected model mismatch for {}", vehicle.id));
        }
        let clear = format!("{}_clear.ini", entry.shader_settings);
        if !rerbr_content::same_path(&entry.ini_file, &vehicle.parts)
            || !rerbr_content::same_path(&entry.shader_file, &vehicle.shaders)
            || !rerbr_content::same_path(&clear, &vehicle.shader_settings)
        {
            return Err(format!(
                "CAR: configuration dependencies disagree with selected vehicle {}",
                vehicle.id
            ));
        }
        let files = vec![
            vehicle.index.clone(),
            entry.file_name.clone(),
            entry.ini_file.clone(),
            entry.shader_file.clone(),
            clear.clone(),
        ];
        (entry, clear, files)
    };
    eprintln!(
        "car-chase MODEL PATH: {} -> {}",
        vehicle.id, entry.file_name
    );
    if vehicle.is_ngp_experimental_car()
        && rerbr_content::VehicleDescriptor::presets()
            .iter()
            .filter(|v| !v.is_ngp_experimental_car())
            .any(|v| rerbr_content::same_path(&v.model, &entry.file_name))
    {
        return Err(format!(
            "CAR ISOLATION: experimental vehicle {} refused fallback retail model '{}'",
            vehicle.id, entry.file_name
        ));
    }
    let bytes = read(root, vehicle, &entry.file_name, 128 * 1024 * 1024)?;
    let elements = sgc::parse(&bytes).map_err(|e| format!("CAR SGC: {e}"))?;
    eprintln!(
        "car-chase READ PASS: {}, {} bytes, {} decoded records",
        vehicle.id,
        bytes.len(),
        elements.len()
    );
    let parts = sections(root, vehicle, &entry.ini_file)?;
    let shader_bytes = read(root, vehicle, &entry.shader_file, 1024 * 1024)?;
    let shaders = ini::parse(&String::from_utf8_lossy(&shader_bytes))
        .map_err(|e| format!("CAR INI {}: {e}", entry.shader_file))?;
    let settings = sections(root, vehicle, &clear)?;
    let geometry = mesh::convert_for_mode(
        &elements,
        &parts,
        &shaders,
        &settings,
        allow_experimental_ngp && vehicle.is_ngp_experimental_car(),
    )?;
    log_unresolved_faces(vehicle, &geometry);
    let model = entry.file_name.replace('\\', "/");
    let mut copy = model
        .rsplit_once('/')
        .map(|(model_dir, _)| ShowroomTextures {
            root,
            model_dir,
            model: (&entry.file_name, &bytes),
            shaders: (&entry.shader_file, &shader_bytes),
            aliases: BTreeMap::new(),
        });
    let mut read_texture = |path: &str| {
        let (resolved, bytes) = match read(root, vehicle, path, 64 * 1024 * 1024) {
            Ok(bytes) => (path.to_owned(), bytes),
            Err(error)
                if vehicle.is_ngp_experimental_car() && cars::resolve(root, path).is_none() =>
            {
                let copy = copy.as_mut().ok_or_else(|| error.clone())?;
                copy.read(path)
                    .map_err(|copy_error| format!("{error}; {copy_error}"))?
            }
            Err(error) => return Err(error),
        };
        prepare_texture(path, &resolved, bytes)
    };
    let mut textures = BTreeMap::new();
    for path in geometry
        .materials
        .iter()
        .filter_map(|m| m.path.clone())
        .collect::<BTreeSet<_>>()
    {
        let texture = read_texture(&path)?;
        eprintln!(
            "car-chase DDS PASS: {path} {:?}, {} mips",
            texture.info.format, texture.info.mip_count
        );
        files.push(texture.info.resolved.clone());
        textures.insert(path, texture);
    }
    let s = geometry.stats(None);
    let o = geometry.stats(Some(false));
    let t = geometry.stats(Some(true));
    eprintln!("car-chase CONVERT PASS: {} exterior meshes, {} skipped, {} vertices, {} triangles, {} draws, {} materials, {} DDS; gravel wheels=4",geometry.drawn.len(),geometry.skipped.len(),s.vertices,s.triangles,s.draws,geometry.materials.len(),textures.len());
    eprintln!(
        "car-chase OPAQUE {o:?}; TRANSPARENT {t:?}; required private files={}",
        files.len()
    );
    for m in &geometry.materials {
        eprintln!(
            "car-chase MATERIAL: {} / {} -> {:?} {:?}",
            m.shader, m.name, m.path, m.pass
        );
    }
    let car = Car {
        vehicle: vehicle.clone(),
        #[cfg(feature = "physics-stage")]
        cameras: crate::driving::cameras::Mounts::parse(&parts)?,
        geometry,
        textures,
        files,
        cockpit: None,
        cockpit_error: None,
    };
    #[cfg(feature = "physics-stage")]
    let car = cockpit::install(car, &elements, &parts, &shaders, &settings, read_texture);
    Ok(car)
}
pub fn private_root(documents: &Path) -> PathBuf {
    documents.join("RBR")
}

/// Read a descriptor dependency from the selected car's directory.
fn read_showroom_file(
    root: &Path,
    model_dir: &str,
    rel: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let norm = rel.replace('\\', "/");
    rerbr_content::validate_relative_path(&norm)?;
    if !norm
        .to_ascii_lowercase()
        .starts_with(&format!("{}/", model_dir.to_ascii_lowercase()))
    {
        return Err(format!("SHOWROOM PATH: invalid {rel}"));
    }
    let car_dir = cars::resolve(root, model_dir)
        .ok_or_else(|| format!("SHOWROOM PATH: missing model directory {model_dir}"))?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let resolved =
        cars::resolve(root, &norm).ok_or_else(|| format!("SHOWROOM READ: missing {norm}"))?;
    let canonical = resolved.canonicalize().map_err(|e| e.to_string())?;
    let root_canon = root.canonicalize().map_err(|e| e.to_string())?;
    if !car_dir.starts_with(&root_canon) || !canonical.starts_with(&car_dir) {
        return Err(format!(
            "SHOWROOM PATH: {rel} escapes selected model directory"
        ));
    }
    let mut data = Vec::new();
    std::fs::File::open(&canonical)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > limit {
        return Err(format!("SHOWROOM READ: {norm} exceeds limit"));
    }
    Ok(data)
}

fn log_unresolved_faces(vehicle: &rerbr_content::VehicleDescriptor, geometry: &mesh::Geometry) {
    for face in &geometry.unresolved_faces {
        eprintln!(
            "CAR MATERIAL: {}: {face}; using untextured surface",
            vehicle.id
        );
    }
}

struct ShowroomTextures<'a> {
    root: &'a Path,
    model_dir: &'a str,
    model: (&'a str, &'a [u8]),
    shaders: (&'a str, &'a [u8]),
    aliases: BTreeMap<String, bool>,
}

impl ShowroomTextures<'_> {
    fn read(&mut self, request: &str) -> Result<(String, Vec<u8>), String> {
        rerbr_content::validate_relative_path(request)?;
        let path = request.replace('\\', "/");
        let basename = path.rsplit('/').next().unwrap_or(&path);
        let Self {
            root,
            model_dir,
            model,
            shaders,
            aliases,
        } = self;
        const LIMIT: u64 = 64 * 1024 * 1024;
        if path
            .to_ascii_lowercase()
            .starts_with(&format!("{}/", model_dir.to_ascii_lowercase()))
        {
            let bytes = read_showroom_file(root, model_dir, &path, LIMIT)?;
            return Ok((path, bytes));
        }
        if let [cars, folder, textures, _] = path.split('/').collect::<Vec<_>>().as_slice() {
            if cars.eq_ignore_ascii_case("cars") && textures.eq_ignore_ascii_case("textures") {
                let dir = format!("{cars}/{folder}");
                if !dir.eq_ignore_ascii_case(model_dir) {
                    let verified = *aliases.entry(dir.clone()).or_insert_with(|| {
                        let matches = |(path, expected): (&str, &[u8])| {
                            let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
                            read_showroom_file(
                                root,
                                &dir,
                                &format!("{dir}/{basename}"),
                                expected.len() as u64,
                            )
                            .is_ok_and(|bytes| bytes == expected)
                        };
                        // A renamed copy of the same model may still declare
                        // its original directory in an identical shader map.
                        matches(*shaders) && matches(*model)
                    });
                    if verified {
                        if cars::resolve(root, &path).is_some() {
                            let bytes = read_showroom_file(root, &dir, &path, LIMIT)?;
                            return Ok((path, bytes));
                        }
                        let local_path = format!("{model_dir}/textures/{basename}");
                        let bytes = read_showroom_file(root, model_dir, &local_path, LIMIT)
                            .map_err(|err| {
                                format!("SHOWROOM DDS: {request} for {model_dir}: {err}")
                            })?;
                        return Ok((local_path, bytes));
                    }
                }
            }
        }
        Err(format!(
            "SHOWROOM DDS: unverified source {request} for {model_dir}"
        ))
    }
}

/// Loads any installed RBR or RallySimFans 3D exterior car model (`.sgc`, `.ini`, `_shaders.ini`,
/// `_shader_settings_{clear,overcast}.ini`, and `.dds` textures) purely for 3D showroom/garage
/// visualization, without requiring physics NGP packs or `Cars/Cars.ini` slot registration.
#[cfg(any(feature = "physics-drive", test))]
pub fn load_showroom_model(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
) -> Result<Car, String> {
    let model = vehicle.model.replace('\\', "/");
    let (model_dir, _) = model
        .rsplit_once('/')
        .ok_or("SHOWROOM PATH: no model directory")?;
    let sgc_bytes = read_showroom_file(root, model_dir, &vehicle.model, 128 * 1024 * 1024)?;
    let elements = sgc::parse(&sgc_bytes).map_err(|e| format!("SHOWROOM SGC: {e}"))?;
    let parts_bytes = read_showroom_file(root, model_dir, &vehicle.parts, 2 * 1024 * 1024)?;
    let parts = ini::parse(&String::from_utf8_lossy(&parts_bytes))
        .map_err(|e| format!("SHOWROOM PARTS INI: {e}"))?;
    let shaders_bytes = read_showroom_file(root, model_dir, &vehicle.shaders, 2 * 1024 * 1024)?;
    let shaders = ini::parse(&String::from_utf8_lossy(&shaders_bytes))
        .map_err(|e| format!("SHOWROOM SHADERS INI: {e}"))?;
    let (settings_path, settings_bytes) =
        read_showroom_file(root, model_dir, &vehicle.shader_settings, 2 * 1024 * 1024)
            .map(|bytes| (vehicle.shader_settings.clone(), bytes))
            .or_else(|err| {
                let alt = vehicle
                    .shader_settings
                    .replace("_clear.ini", "_overcast.ini");
                if alt == vehicle.shader_settings {
                    return Err(err);
                }
                let bytes = read_showroom_file(root, model_dir, &alt, 2 * 1024 * 1024)?;
                Ok((alt, bytes))
            })?;
    let settings = ini::parse(&String::from_utf8_lossy(&settings_bytes))
        .map_err(|e| format!("SHOWROOM SETTINGS INI: {e}"))?;

    let geometry = mesh::convert_for_mode(&elements, &parts, &shaders, &settings, true)?;
    log_unresolved_faces(vehicle, &geometry);

    let mut textures = BTreeMap::new();
    let mut files = vec![
        vehicle.model.clone(),
        vehicle.parts.clone(),
        vehicle.shaders.clone(),
        settings_path,
    ];
    let mut texture_source = ShowroomTextures {
        root,
        model_dir,
        model: (&vehicle.model, &sgc_bytes),
        shaders: (&vehicle.shaders, &shaders_bytes),
        aliases: BTreeMap::new(),
    };
    for path in geometry
        .materials
        .iter()
        .filter_map(|m| m.path.clone())
        .collect::<BTreeSet<_>>()
    {
        let (resolved, bytes) = texture_source.read(&path)?;
        let texture = prepare_texture(&path, &resolved, bytes)?;
        files.push(resolved);
        textures.insert(path, texture);
    }
    #[cfg(feature = "physics-stage")]
    let cameras = crate::driving::cameras::Mounts::parse(&parts)
        .unwrap_or_else(|_| crate::driving::cameras::Mounts::fallback());
    Ok(Car {
        vehicle: vehicle.clone(),
        #[cfg(feature = "physics-stage")]
        cameras,
        geometry,
        textures,
        files,
        cockpit: None,
        cockpit_error: None,
    })
}

#[cfg(test)]
mod part_audit;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "physics-stage"))]
mod visual_audit;
