//! Original Corolla visual loader. No game resources, physics or DirectX shaders.
pub mod cockpit;
pub(crate) mod gpu;
pub mod materials;
pub mod mesh;
pub mod placement;
pub(crate) mod runtime;
pub mod selection;

use crate::harwood::texture::upload_plan;
use rerbr_asset_formats::{cars, ini, sgc};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};

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
    selection.validate_current_backend()?;
    #[cfg(not(feature = "physics-stage"))]
    if selection.stage.id != "harwood-forest" {
        return Err("multiple stages require physics-stage mode".into());
    }
    #[cfg(feature = "physics-stage")]
    match rerbr_sim::audio_audit::inspect_vehicle(install, &selection.vehicle) {
        Ok(report) => eprintln!("physics-stage AUDIO BANK CPU PASS: {report:?}; output not implemented; silent milestone"),
        Err(error) => eprintln!("physics-stage AUDIO DISABLED: {error}; silent milestone; geometry can still load"),
    }
    let car = load_vehicle(install, &selection.vehicle)?;
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
    let stage_cameras =
        crate::driving::replay::StageCameras::load_for_stage(maps, &selection.stage.stem);
    Ok(Prepared {
        selection: selection.clone(),
        terrain: render,
        path,
        car,
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
    vehicle.validate_current_backend()?;
    let index = read(root, vehicle, &vehicle.index, 1024 * 1024)?;
    let entries = cars::parse(&String::from_utf8_lossy(&index)).map_err(|e| e.to_string())?;
    let entry = entries
        .iter()
        .find(|e| e.index == vehicle.original_slot as usize)
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
    let bytes = read(root, vehicle, &entry.file_name, 32 * 1024 * 1024)?;
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
    let geometry = mesh::convert(&elements, &parts, &shaders, &settings)?;
    let mut files = vec![
        vehicle.index.clone(),
        entry.file_name.clone(),
        entry.ini_file.clone(),
        entry.shader_file.clone(),
        clear,
    ];
    let mut textures = BTreeMap::new();
    for path in geometry
        .materials
        .iter()
        .map(|m| m.path.clone())
        .collect::<BTreeSet<_>>()
    {
        let bytes = read(root, vehicle, &path, 16 * 1024 * 1024)?;
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

#[cfg(test)]
mod tests;
