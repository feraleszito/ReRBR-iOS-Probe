//! Exterior selection adapted from reRBR car/parts.rs and lights::shown_at_start.
use rerbr_asset_formats::{car_parts, ini::Section};

pub const SIDES: [&str; 4] = ["lf", "rf", "lb", "rb"];
pub fn wheel(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("e_wheel_gravel_")?;
    SIDES
        .iter()
        .position(|&s| s == rest.strip_prefix("tyre_").unwrap_or(rest))
}
pub fn slot(name: &str) -> u32 {
    wheel(name)
        .or_else(|| {
            name.strip_prefix("e_brakedisc_").and_then(|s| {
                SIDES
                    .iter()
                    .position(|&side| side == s.strip_suffix("_lit").unwrap_or(s))
            })
        })
        .map_or(0, |k| k as u32 + 1)
}
pub fn shown_at_start(part: &Section) -> bool {
    let value = |key| part.get(key).filter(|v| !v.is_empty());
    match (
        car_parts::is_switched(part),
        value("SwitchBox"),
        value("Predicate"),
    ) {
        (true, Some(b), Some(p)) => {
            if p.eq_ignore_ascii_case("NotUsed") {
                return true;
            }
            ["light", "brake", "reverse"].iter().any(|n| b.contains(n)) && p == "off"
        }
        _ => true,
    }
}
pub fn exterior<'a>(name: &str, parts: &'a [Section]) -> Result<&'a Section, &'static str> {
    part_set(name, parts, false)
}
pub fn interior<'a>(name: &str, parts: &'a [Section]) -> Result<&'a Section, &'static str> {
    part_set(name, parts, true)
}
fn part_set<'a>(
    name: &str,
    parts: &'a [Section],
    inside: bool,
) -> Result<&'a Section, &'static str> {
    if name.starts_with("ghost_") {
        return Err("ghost");
    }
    if name.starts_with("texture_") {
        return Err("overlay");
    }
    if name.starts_with("Plane_") {
        return Err("shadow plane");
    }
    if name.starts_with("i_") != inside {
        return Err(if inside { "exterior" } else { "interior" });
    }
    if inside
        && (["i_wing", "i_bumper", "i_window_lf", "i_window_rf"]
            .iter()
            .any(|p| name.contains(p))
            || name == "i_drivers")
    {
        return Err("not built by original interior loader");
    }
    if name == "pc_e_premove" || name.starts_with("k_") || name.starts_with("d_") {
        return Err("not built by original loader");
    }
    let part = car_parts::section(parts, name).ok_or("no part section")?;
    if !shown_at_start(part) {
        return Err("hidden initial switch state (lights off)");
    }
    if (name.contains("_wheel_") || name.contains("sparewheel")) && !name.contains("gravel") {
        return Err("unused surface variant");
    }
    if car_parts::face_blocks(part).is_empty() {
        return Err("no face blocks");
    }
    Ok(part)
}
