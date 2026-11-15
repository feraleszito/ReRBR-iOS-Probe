//! Original Corolla visual loader. No game resources, physics or DirectX shaders.
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
    pub geometry: mesh::Geometry,
    pub(crate) textures: BTreeMap<String, crate::asset_probe::TextureData>,
    pub files: Vec<String>,
}
pub const PRIVATE_FILES: [&str; 8] = [
    "Cars/Cars.ini",
    "Cars/COROLLA/corolla.sgc",
    "Cars/COROLLA/corolla.ini",
    "Cars/COROLLA/corolla_shaders.ini",
    "Cars/COROLLA/corolla_shader_settings_clear.ini",
    "Cars/COROLLA/textures/corolla_external.dds",
    "Cars/COROLLA/textures/corolla_external_dynamic.dds",
    "Cars/COROLLA/textures/corolla_external_transparent.dds",
];
pub struct Prepared {
    pub terrain: crate::harwood::LoadedBlock,
    pub path: placement::ChasePath,
    pub car: Car,
}
pub fn prepare(install: &Path, maps: &Path) -> Result<Prepared, String> {
    let car = load(install)?;
    let terrain = crate::harwood::benchmark::load(maps)?;
    let path = placement::ChasePath::new(terrain.route, &terrain.render.mesh, &car.geometry)?;
    path.validate()?;
    Ok(Prepared {
        terrain: terrain.render,
        path,
        car,
    })
}
fn read(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>, String> {
    if path
        .split(['\\', '/'])
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(':'))
    {
        return Err(format!("CAR PATH: invalid {path}"));
    }
    if !PRIVATE_FILES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&path.replace('\\', "/")))
    {
        return Err(format!(
            "CAR READ: additional resource required outside the eight-file profile: {path}"
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
fn sections(root: &Path, path: &str) -> Result<Vec<ini::Section>, String> {
    let bytes = read(root, path, 1024 * 1024)?;
    ini::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("CAR INI {path}: {e}"))
}

pub fn load(root: &Path) -> Result<Car, String> {
    let index = read(root, "Cars/Cars.ini", 1024 * 1024)?;
    let entries = cars::parse(&String::from_utf8_lossy(&index)).map_err(|e| e.to_string())?;
    let entry = entries
        .iter()
        .find(|e| e.index == 6)
        .ok_or("CAR: [Car06] missing")?;
    if !entry
        .file_name
        .replace('\\', "/")
        .eq_ignore_ascii_case("cars/corolla/corolla.sgc")
    {
        return Err("CAR: [Car06] must select the original COROLLA".into());
    }
    let clear = format!("{}_clear.ini", entry.shader_settings);
    let bytes = read(root, &entry.file_name, 32 * 1024 * 1024)?;
    let elements = sgc::parse(&bytes).map_err(|e| format!("CAR SGC: {e}"))?;
    eprintln!(
        "car-chase READ PASS: [Car06] COROLLA, {} bytes, {} decoded records",
        bytes.len(),
        elements.len()
    );
    let parts = sections(root, &entry.ini_file)?;
    let shaders = sections(root, &entry.shader_file)?;
    let settings = sections(root, &clear)?;
    let geometry = mesh::convert(&elements, &parts, &shaders, &settings)?;
    let mut files = vec![
        "Cars/Cars.ini".into(),
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
        let bytes = read(root, &path, 16 * 1024 * 1024)?;
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
    Ok(Car {
        geometry,
        textures,
        files,
    })
}
pub fn private_root(documents: &Path) -> PathBuf {
    documents.join("RBR")
}

#[cfg(test)]
mod tests;
