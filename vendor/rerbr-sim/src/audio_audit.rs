//! Read-only preparation for an audio port. No output device or fabricated sound.
use rbr_formats::{audio, cars, eng};
use std::{io::Read, path::Path};

#[derive(Debug)]
pub struct Report {
    pub engine_path: String,
    pub banks: usize,
    pub parts: usize,
    pub samples: usize,
    pub cycle_markers: usize,
    pub rates: Vec<u32>,
}
fn read(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>, String> {
    let file = cars::resolve(root, path).ok_or_else(|| format!("AUDIO MISSING: Documents/RBR/{path}"))?;
    let canonical = file.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("AUDIO: private root escape".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(canonical)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("AUDIO: file too large".into());
    }
    Ok(bytes)
}
pub fn inspect(root: &Path) -> Result<Report, String> {
    inspect_vehicle(root, &rerbr_content::VehicleDescriptor::default())
}
pub fn inspect_vehicle(root: &Path, vehicle: &rerbr_content::VehicleDescriptor) -> Result<Report, String> {
    vehicle.validate_current_backend()?;
    let bytes = read(root, &vehicle.audio_index, 1024 * 1024)?;
    let sections = audio::parse_ini(&bytes).map_err(|e| e.to_string())?;
    let car = audio::CarSound::read(&sections, vehicle.original_slot)
        .ok_or_else(|| format!("AUDIO: missing Car{} sound mapping", vehicle.original_slot))?;
    if car.engine.is_empty() || !car.engine.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err("AUDIO: unsafe engine bank name".into());
    }
    let engine_path = format!("Audio/Cars/{}.eng", car.engine);
    let bank = eng::parse(&read(root, &engine_path, 16 * 1024 * 1024)?).map_err(|e| e.to_string())?;
    let mut report =
        Report { engine_path, banks: bank.banks.len(), parts: 0, samples: 0, cycle_markers: 0, rates: Vec::new() };
    for part in bank.banks.iter().flat_map(|b| &b.parts) {
        report.parts += 1;
        report.samples += part.samples.len();
        report.cycle_markers += part.cycles.len();
        report.rates.push(part.rate);
        if part.rate == 0
            || part.samples.is_empty()
            || part.cycles.len() < 2
            || !part.cycles.windows(2).all(|w| w[0].is_finite() && w[1] > w[0])
        {
            return Err("AUDIO: unsupported engine part data".into());
        }
    }
    report.rates.sort_unstable();
    report.rates.dedup();
    if report.banks == 0 {
        return Err("AUDIO: empty bank".into());
    }
    Ok(report)
}
