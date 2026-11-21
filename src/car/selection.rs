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

pub fn exterior_with_lamps<'a>(
    name: &str,
    parts: &'a [Section],
    has_mesh: impl Fn(&str) -> bool,
) -> Result<&'a Section, &'static str> {
    let selected = reject_inactive_variant(name, parts, exterior(name, parts), &has_mesh);
    if !name.starts_with("e_light_") {
        return selected;
    }
    if let Some(base) = name.strip_suffix("_lit") {
        if selected.is_ok() && has_mesh(base) && exterior(base, parts).is_ok() {
            return Err("unlit lamp already selected");
        }
    } else if matches!(
        name,
        "e_light_lf" | "e_light_rf" | "e_light_lb" | "e_light_rb"
    ) && selected == Err("optional variant deselected by Switch=true")
    {
        let lit = format!("{name}_lit");
        if has_mesh(&lit) && exterior(&lit, parts) == selected {
            // Some packs disable both states of a standard lamp. Keep its unlit mesh.
            return car_parts::section(parts, name)
                .filter(|part| !car_parts::face_blocks(part).is_empty())
                .ok_or("no headlamp face blocks");
        }
    }
    selected
}
pub fn interior<'a>(name: &str, parts: &'a [Section]) -> Result<&'a Section, &'static str> {
    part_set(name, parts, true)
}

pub fn interior_with_variants<'a>(
    name: &str,
    parts: &'a [Section],
    has_mesh: impl Fn(&str) -> bool,
) -> Result<&'a Section, &'static str> {
    reject_inactive_variant(name, parts, interior(name, parts), has_mesh)
}

fn mirror_family(name: &str) -> Option<(&str, &str, bool)> {
    let (view, rest) = name.split_at_checked(2)?;
    let rest = rest.strip_prefix("mirror_")?;
    let glass = rest.strip_suffix("_glass");
    let side = glass.unwrap_or(rest).rsplit('_').next()?;
    matches!(side, "l" | "r").then_some((view, side, glass.is_some()))
}

fn reject_inactive_variant<'a>(
    name: &str,
    parts: &'a [Section],
    selected: Result<&'a Section, &'static str>,
    has_mesh: impl Fn(&str) -> bool,
) -> Result<&'a Section, &'static str> {
    let part = selected?;
    if !car_parts::is_switched(part)
        || !part
            .get("Predicate")
            .is_some_and(|p| p.eq_ignore_ascii_case("NotUsed"))
    {
        return Ok(part);
    }
    let mirror = mirror_family(name);
    if name != "e_wing" && mirror.is_none() {
        return Ok(part);
    }
    let alternative = |candidate: &str| {
        if name == "e_wing" {
            candidate.starts_with("e_wing_evo")
        } else {
            mirror_family(candidate) == mirror
                || (matches!(name, "e_mirror_l" | "e_mirror_r")
                    && matches!(candidate, "e_racingmirror" | "e_stockmirror"))
        }
    };
    if parts.iter().any(|other| {
        other.name != name
            && alternative(&other.name)
            && has_mesh(&other.name)
            && other
                .get("Switch")
                .is_some_and(|s| s.eq_ignore_ascii_case("false"))
            && other
                .get("Predicate")
                .is_some_and(|p| p.eq_ignore_ascii_case("NotUsed"))
            && !car_parts::face_blocks(other).is_empty()
    }) {
        return Err("alternate part selected by original INI");
    }
    Ok(part)
}

/// Body panels and fixtures that RBR itself treats as part of the car (damage panels, glass,
/// mirrors, exhaust, roof intake). Community tools write `Switch = true` on them in some cars
/// (BMW M3 E30) without meaning "disabled", so their flag never hides them.
pub fn is_structural_panel(name: &str) -> bool {
    name.starts_with("e_door_")
        || (name.starts_with("e_bonnet") && !name.starts_with("e_bonnet_ramp"))
        || (name.starts_with("e_bumper_") && !name.starts_with("e_bumper_ramp_"))
        || name == "e_bumpervents"
        || name == "e_wing"
        || name.starts_with("e_mirror_")
        || name.starts_with("e_window_")
        || name == "e_exhaust"
        || name == "e_intakeroof"
        || name == "e_body"
}

/// Optional exterior variants (auxiliary lamps and their lit twins, alternative mirrors, wings,
/// grilles, plates, mud flaps, ...) are selected by the car's own `.ini` through
/// `Switch = true/false` with `Predicate = NotUsed`; RallySimFans' parts tool writes the preset
/// files (`carparts/*/<car>.ini`) with exactly that flag: `Switch = false` selects the variant
/// (`Lights_On` => `*_lit` false / unlit true, `wing_evo` => evo2a+evo2b false, evo1 true) and
/// `Switch = true` deselects it. Drawing both members of every pair put lit lamps over unlit
/// lamps, two mirrors, four wing pieces and two bumper strips in the same place.
pub fn variant_deselected(name: &str, part: &Section) -> bool {
    if name.starts_with("i_") || is_structural_panel(name) || !car_parts::is_switched(part) {
        return false;
    }
    let value = |key| part.get(key).filter(|v| !v.is_empty());
    match (value("SwitchBox"), value("Predicate")) {
        (_, None) => false,
        (_, Some(p)) => p.eq_ignore_ascii_case("NotUsed"),
    }
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
    if variant_deselected(name, part) {
        return Err("optional variant deselected by Switch=true");
    }
    if (name.contains("_wheel_") || name.contains("sparewheel")) && !name.contains("gravel") {
        return Err("unused surface variant");
    }
    if car_parts::face_blocks(part).is_empty() {
        return Err("no face blocks");
    }
    Ok(part)
}
