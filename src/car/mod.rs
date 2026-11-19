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

use crate::harwood::texture::upload_plan;
use rerbr_asset_formats::{cars, ini, sgc};
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
    let shaders = sections(root, vehicle, &entry.shader_file)?;
    let settings = sections(root, vehicle, &clear)?;
    let geometry = mesh::convert_for_mode(
        &elements,
        &parts,
        &shaders,
        &settings,
        allow_experimental_ngp && vehicle.is_ngp_experimental_car(),
    )?;
    let _ = &mut files;
    let mut textures = BTreeMap::new();
    for path in geometry
        .materials
        .iter()
        .map(|m| m.path.clone())
        .collect::<BTreeSet<_>>()
    {
        let bytes = read(root, vehicle, &path, 64 * 1024 * 1024)?;
        let plan = upload_plan(&bytes, u32::MAX)?;
        eprintln!(
            "car-chase DDS PASS: {path} {:?}, {} mips",
            plan.format,
            plan.mips.len()
        );
        files.push(path.clone());
        let first = plan.mips.first().ok_or("CAR DDS: missing mip chain")?;
        let info = crate::asset_probe::TextureInfo {
            request: path.clone(),
            resolved: path.clone(),
            width: first.width,
            height: first.height,
            mip_count: plan.mips.len() as u32,
            format: plan.format,
        };
        textures.insert(path, crate::asset_probe::TextureData { info, bytes });
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
            "car-chase MATERIAL: {} / {} -> {} {:?}",
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
    let car = cockpit::install(car, root, &elements, &parts, &shaders, &settings);
    Ok(car)
}
pub fn private_root(documents: &Path) -> PathBuf {
    documents.join("RBR")
}

/// Safe read of a relative file under `root` (`Documents/RBR`) for 3D showroom inspection.
/// Never allows path traversal or reading outside `root`.
#[cfg(any(feature = "physics-drive", test))]
fn read_showroom_file(root: &Path, rel: &str, limit: u64) -> Result<Vec<u8>, String> {
    let norm = rel.replace('\\', "/");
    if norm
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(':'))
    {
        return Err(format!("SHOWROOM PATH: invalid {rel}"));
    }
    let resolved =
        cars::resolve(root, &norm).ok_or_else(|| format!("SHOWROOM READ: missing {norm}"))?;
    let canonical = resolved.canonicalize().map_err(|e| e.to_string())?;
    let root_canon = root.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root_canon) {
        return Err("SHOWROOM PATH: escapes private root".into());
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

/// Loads any installed RBR or RallySimFans 3D exterior car model (`.sgc`, `.ini`, `_shaders.ini`,
/// `_shader_settings_{clear,overcast}.ini`, and `.dds` textures) purely for 3D showroom/garage
/// visualization, without requiring physics NGP packs or `Cars/Cars.ini` slot registration.
#[cfg(any(feature = "physics-drive", test))]
pub fn load_showroom_model(
    root: &Path,
    vehicle: &rerbr_content::VehicleDescriptor,
) -> Result<Car, String> {
    let sgc_bytes = read_showroom_file(root, &vehicle.model, 128 * 1024 * 1024)?;
    let elements = sgc::parse(&sgc_bytes).map_err(|e| format!("SHOWROOM SGC: {e}"))?;
    let parts_bytes = read_showroom_file(root, &vehicle.parts, 2 * 1024 * 1024)?;
    let parts = ini::parse(&String::from_utf8_lossy(&parts_bytes))
        .map_err(|e| format!("SHOWROOM PARTS INI: {e}"))?;
    let shaders_bytes = read_showroom_file(root, &vehicle.shaders, 2 * 1024 * 1024)?;
    let shaders = ini::parse(&String::from_utf8_lossy(&shaders_bytes))
        .map_err(|e| format!("SHOWROOM SHADERS INI: {e}"))?;
    let settings_bytes = read_showroom_file(root, &vehicle.shader_settings, 2 * 1024 * 1024)
        .or_else(|_| {
            let alt = vehicle
                .shader_settings
                .replace("_clear.ini", "_overcast.ini");
            read_showroom_file(root, &alt, 2 * 1024 * 1024)
        })?;
    let settings = ini::parse(&String::from_utf8_lossy(&settings_bytes))
        .map_err(|e| format!("SHOWROOM SETTINGS INI: {e}"))?;

    let geometry = mesh::convert_for_mode(&elements, &parts, &shaders, &settings, true)?;
    let model_dir = vehicle
        .model
        .replace('\\', "/")
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default();

    let mut textures = BTreeMap::new();
    let mut files = vec![
        vehicle.model.clone(),
        vehicle.parts.clone(),
        vehicle.shaders.clone(),
        vehicle.shader_settings.clone(),
    ];
    for path in geometry
        .materials
        .iter()
        .map(|m| m.path.clone())
        .collect::<BTreeSet<_>>()
    {
        let bytes = read_showroom_file(root, &path, 64 * 1024 * 1024).or_else(|first_err| {
            // Some community RSF car packs rename the outer folder while keeping the original
            // filename inside `<model_dir>/textures/<basename>`. Resolve strictly inside the same car folder.
            let basename = path.replace('\\', "/");
            let basename = basename.rsplit('/').next().unwrap_or("");
            if !model_dir.is_empty() && !basename.is_empty() {
                let fallback = format!("{model_dir}/textures/{basename}");
                read_showroom_file(root, &fallback, 64 * 1024 * 1024)
            } else {
                Err(first_err)
            }
        })?;
        let plan = upload_plan(&bytes, u32::MAX)?;
        let first = plan.mips.first().ok_or("SHOWROOM DDS: missing mip chain")?;
        let info = crate::asset_probe::TextureInfo {
            request: path.clone(),
            resolved: path.clone(),
            width: first.width,
            height: first.height,
            mip_count: plan.mips.len() as u32,
            format: plan.format,
        };
        files.push(path.clone());
        textures.insert(path, crate::asset_probe::TextureData { info, bytes });
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
mod tests;
