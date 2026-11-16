use rbr_formats::{cars, data::Data, ngp};
use std::path::{Path, PathBuf};

pub const FILES: [&str; 10] = [
    "Plugins/NGP.dll",
    "physics.rbz",
    "misc.rbz",
    "Scripts.rbz",
    "Cars/Cars.ini",
    "Cars/COROLLA/corolla.sgc",
    "Sgs/br_spectator_00.sgs",
    "Sgs/br_spectator_01.sgs",
    "Sgs/br_spectator_02.sgs",
    "Sgs/br_spectator_03.sgs",
];
pub const MAPS: [&str; 6] =
    ["track-20_O.lbs", "track-20_O.col", "track-20_O.trk", "track-20_O.mat", "track-20_O.dls", "track-20_O.fnc"];
#[derive(Debug, Clone)]
pub struct Paths {
    pub install: PathBuf,
    pub overlay: Option<PathBuf>,
    pub maps: PathBuf,
}
impl Paths {
    pub fn documents(documents: &Path) -> Self {
        Self { install: documents.join("RBR"), overlay: None, maps: documents.join("Harwood/Maps") }
    }
    pub fn data(&self) -> Data {
        Data::new(self.install.clone(), self.overlay.clone())
    }
}
pub struct Checked {
    pub physics: PathBuf,
}
pub fn check(paths: &Paths) -> Result<Checked, String> {
    let data = paths.data();
    let mut total = 0;
    for file in FILES {
        let path = data.resolve(file).ok_or_else(|| format!("missing private RBR/{file}"))?;
        let length = path.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 || length > 64 * 1024 * 1024 {
            return Err(format!("invalid private file size: {file}"));
        }
        total += length;
    }
    for file in MAPS {
        let path = cars::resolve(&paths.maps, file).ok_or_else(|| format!("missing private Harwood/Maps/{file}"))?;
        let length = path.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 || length > 64 * 1024 * 1024 {
            return Err(format!("invalid stage file size: {file}"));
        }
        total += length;
    }
    let dll = data.resolve("Plugins/NGP.dll").unwrap();
    ngp::validate(&std::fs::read(&dll).map_err(|e| e.to_string())?)?;
    let index = data.resolve("Cars/Cars.ini").unwrap();
    let bytes = std::fs::read(index).map_err(|e| e.to_string())?;
    let cars = cars::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?;
    if !cars
        .iter()
        .any(|c| c.index == 6 && c.file_name.replace('\\', "/").eq_ignore_ascii_case("cars/corolla/corolla.sgc"))
    {
        return Err("[Car06] must name the original Corolla".into());
    }
    eprintln!("physics-probe INPUT PASS: 16 private files, {total} bytes; NGP 7.4.778 verified; no files extracted or written");
    Ok(Checked { physics: data.resolve("physics.rbz").unwrap() })
}
