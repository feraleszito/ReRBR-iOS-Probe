use rbr_formats::{cars, data::Data};
use std::path::{Path, PathBuf};

pub const FILES: [&str; 9] = [
    "Plugins/NGP.dll",
    "physics.rbz",
    "misc.rbz",
    "Scripts.rbz",
    "Cars/Cars.ini",
    "Sgs/br_spectator_00.sgs",
    "Sgs/br_spectator_01.sgs",
    "Sgs/br_spectator_02.sgs",
    "Sgs/br_spectator_03.sgs",
];
#[derive(Debug, Clone)]
pub struct Paths {
    pub install: PathBuf,
    pub overlay: Option<PathBuf>,
    pub maps: PathBuf,
}
impl Paths {
    pub fn documents(documents: &Path) -> Self {
        Self::for_selection(documents, &rerbr_content::Selection::default()).expect("validated default content profile")
    }
    pub fn for_selection(documents: &Path, selection: &rerbr_content::Selection) -> Result<Self, String> {
        selection.validate_current_backend()?;
        Ok(Self {
            install: documents.join("RBR"),
            overlay: None,
            maps: documents.join(&selection.stage.documents_maps),
        })
    }
    pub fn data(&self) -> Data {
        Data::new(self.install.clone(), self.overlay.clone())
    }
}
pub struct Checked {
    pub physics: PathBuf,
}
pub fn check(paths: &Paths) -> Result<Checked, String> {
    check_selection(paths, &rerbr_content::Selection::default())
}
pub fn check_selection(paths: &Paths, selection: &rerbr_content::Selection) -> Result<Checked, String> {
    selection.validate_current_backend()?;
    let data = paths.data();
    let mut total = 0;
    for file in FILES.into_iter().chain(std::iter::once(selection.vehicle.model.as_str())) {
        let path = data.resolve(file).ok_or_else(|| format!("missing private RBR/{file}"))?;
        let length = path.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 || length > 64 * 1024 * 1024 {
            return Err(format!("invalid private file size: {file}"));
        }
        total += length;
    }
    for extension in rerbr_content::PHYSICS_STAGE_EXTENSIONS {
        let file = selection.stage.file(extension)?;
        let path = cars::resolve(&paths.maps, &file)
            .ok_or_else(|| format!("missing private {}/{file}", selection.stage.documents_maps))?;
        let length = path.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 || length > 64 * 1024 * 1024 {
            return Err(format!("invalid stage file size: {file}"));
        }
        total += length;
    }
    let dll = data.resolve("Plugins/NGP.dll").unwrap();
    crate::ngp_candidate::validate_runtime(&std::fs::read(&dll).map_err(|e| e.to_string())?)?;
    let index = data.resolve("Cars/Cars.ini").unwrap();
    let bytes = std::fs::read(index).map_err(|e| e.to_string())?;
    let cars = cars::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?;
    if !cars.iter().any(|c| {
        c.index == selection.vehicle.original_slot as usize
            && rerbr_content::same_path(&c.file_name, &selection.vehicle.model)
    }) {
        return Err(format!(
            "[Car{:02}] must name the selected model {}",
            selection.vehicle.original_slot, selection.vehicle.model
        ));
    }
    eprintln!("physics-probe INPUT PASS: 16 private files, {total} bytes; NGP 7.4.778 DLL verified (vehicle compatibility not certified); no files extracted or written");
    Ok(Checked { physics: data.resolve("physics.rbz").unwrap() })
}
