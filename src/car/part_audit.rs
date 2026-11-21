//! Read-only audit of exterior part selection against installed RSF/RBR cars (skipped without a PC install).
use super::selection;
use rerbr_asset_formats::{car_parts, ini, sgc};
use std::path::{Path, PathBuf};

fn rbr_root() -> Option<PathBuf> {
    let root = PathBuf::from(r"C:\Richard Burns Rally");
    root.join("Cars").is_dir().then_some(root)
}

fn car_files(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let sgc = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sgc")))?;
    let ini = sgc.with_extension("ini");
    ini.is_file().then_some((sgc, ini))
}

/// Dump mesh table of one car (set PART_AUDIT_CAR=folder). Ignored by default.
fn car_space_vertices(
    m: &sgc::Mesh,
    elements: &[sgc::Element],
    worlds: &std::collections::HashMap<i32, [[f32; 4]; 3]>,
) -> Vec<rerbr_render_core::Vec3> {
    let matrix = super::mesh::part_transform(m, elements, worlds);
    m.vertices
        .iter()
        .map(|v| matrix.transform_point3(rerbr_render_core::Vec3::from(v.position)))
        .collect()
}

fn coincidence(a: &[rerbr_render_core::Vec3], b: &[rerbr_render_core::Vec3]) -> f32 {
    use std::collections::HashMap;
    let cell = 0.05f32;
    let key = |p: &rerbr_render_core::Vec3| {
        (
            (p.x / cell).floor() as i32,
            (p.y / cell).floor() as i32,
            (p.z / cell).floor() as i32,
        )
    };
    let mut grid: HashMap<(i32, i32, i32), Vec<rerbr_render_core::Vec3>> = HashMap::new();
    for p in b {
        grid.entry(key(p)).or_default().push(*p);
    }
    if a.is_empty() {
        return 0.0;
    }
    let mut hit = 0usize;
    for p in a {
        let k = key(p);
        let mut found = false;
        'n: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        if v.iter().any(|q| q.distance(*p) < 0.02) {
                            found = true;
                            break 'n;
                        }
                    }
                }
            }
        }
        hit += usize::from(found);
    }
    hit as f32 / a.len() as f32
}

#[test]
#[ignore]
fn coincidence_of_switch_true_parts_with_body() {
    let Some(root) = rbr_root() else { return };
    let folder = std::env::var("PART_AUDIT_CAR").unwrap_or_else(|_| "bmw_m3_e30".into());
    let (sgc_path, ini_path) = car_files(&root.join("Cars").join(&folder)).expect("car files");
    let elements = sgc::parse(&std::fs::read(sgc_path).unwrap()).unwrap();
    let parts = ini::parse(&String::from_utf8_lossy(&std::fs::read(ini_path).unwrap())).unwrap();
    let worlds = sgc::chained_worlds(&elements);
    let body = elements
        .iter()
        .find_map(|e| match e {
            sgc::Element::Mesh(m) if m.node.name == "e_body" => Some(m),
            _ => None,
        })
        .unwrap();
    let body_v = car_space_vertices(body, &elements, &worlds);
    for e in &elements {
        let sgc::Element::Mesh(m) = e else { continue };
        if !m.node.name.starts_with("e_") || m.node.name == "e_body" {
            continue;
        }
        let Some(sec) = car_parts::section(&parts, &m.node.name) else {
            continue;
        };
        let v = car_space_vertices(m, &elements, &worlds);
        let lit_pair = m
            .node
            .name
            .strip_suffix("_lit")
            .and_then(|b| {
                elements.iter().find_map(|e| match e {
                    sgc::Element::Mesh(o) if o.node.name == b => Some(o),
                    _ => None,
                })
            })
            .map(|o| coincidence(&v, &car_space_vertices(o, &elements, &worlds)));
        println!(
            "{:<26} sw={:<6} tris={:<5} body_overlap={:>4.0}% lit_pair_overlap={:?}",
            m.node.name,
            sec.get("Switch").unwrap_or("-"),
            m.triangles.len(),
            100.0 * coincidence(&v, &body_v),
            lit_pair.map(|x| (x * 100.0).round())
        );
    }
}

#[test]
fn audit_variant_selection_across_installed_cars_if_pc_present() {
    let Some(root) = rbr_root() else { return };
    let mut dirs: Vec<_> = std::fs::read_dir(root.join("Cars"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut hidden_by_name: std::collections::BTreeMap<String, usize> = Default::default();
    let mut lamp_overlaps: Vec<String> = Vec::new();
    let mut missing_lamps: Vec<String> = Vec::new();
    let mut cars = 0usize;
    for dir in dirs {
        let Some((sgc_path, ini_path)) = car_files(&dir) else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&sgc_path) else {
            continue;
        };
        let Ok(elements) = sgc::parse(&bytes) else {
            continue;
        };
        let Ok(text) = std::fs::read(&ini_path) else {
            continue;
        };
        let Ok(parts) = ini::parse(&String::from_utf8_lossy(&text)) else {
            continue;
        };
        cars += 1;
        let folder = dir.file_name().unwrap().to_string_lossy().to_string();
        let meshes: Vec<&str> = elements
            .iter()
            .filter_map(|e| match e {
                sgc::Element::Mesh(m) => Some(m.node.name.as_str()),
                _ => None,
            })
            .collect();
        let mesh_names: std::collections::BTreeSet<_> = meshes.iter().copied().collect();
        let drawn = |n: &str| {
            selection::exterior_with_lamps(n, &parts, |name| mesh_names.contains(name)).is_ok()
        };
        for n in &meshes {
            if n.starts_with("e_")
                && selection::exterior(n, &parts)
                    == Err("optional variant deselected by Switch=true")
            {
                *hidden_by_name.entry((*n).to_string()).or_default() += 1;
            }
            // A lamp and its lit twin must never both be drawn at once when the car uses NotUsed.
            if let Some(base) = n.strip_suffix("_lit") {
                if n.starts_with("e_light_") && meshes.contains(&base) && drawn(n) && drawn(base) {
                    lamp_overlaps.push(format!("{folder}: {base}"));
                }
            }
        }
        for base in ["e_light_lf", "e_light_rf", "e_light_lb", "e_light_rb"] {
            let lit = format!("{base}_lit");
            if meshes.contains(&base)
                && meshes.contains(&lit.as_str())
                && !drawn(base)
                && !drawn(&lit)
            {
                missing_lamps.push(format!("{folder}: {base}"));
            }
        }
    }
    println!("VARIANT AUDIT: {cars} cars");
    for (n, c) in &hidden_by_name {
        println!("  hidden {n}: {c}");
    }
    println!("  lamp overlaps after fix: {lamp_overlaps:?}");
    println!("  missing lamp pairs after fix: {missing_lamps:?}");
    assert!(cars >= 80);
    assert!(
        lamp_overlaps.is_empty(),
        "lamp+lit twin still drawn together: {lamp_overlaps:?}"
    );
    assert!(
        missing_lamps.is_empty(),
        "headlamp/taillamp pair vanished entirely: {missing_lamps:?}"
    );
}

#[test]
fn bmw_m3_e30_draws_one_variant_per_group_if_pc_present() {
    let Some(root) = rbr_root() else { return };
    let Some((_, ini_path)) = car_files(&root.join("Cars").join("bmw_m3_e30")) else {
        return;
    };
    let parts = ini::parse(&String::from_utf8_lossy(&std::fs::read(ini_path).unwrap())).unwrap();
    let drawn = |n: &str| selection::exterior(n, &parts).is_ok();
    // Structural panels remain.
    for n in [
        "e_body",
        "e_door_l",
        "e_door_r",
        "e_bonnet",
        "e_bumper_f",
        "e_bumper_b",
        "e_wing",
        "e_exhaust",
    ] {
        assert!(drawn(n), "{n} must stay drawn");
    }
    // Optional variants: exactly the Switch=false member.
    assert!(
        drawn("e_wing_evo1")
            && !drawn("e_wing_evo2a")
            && !drawn("e_wing_evo2b")
            && !drawn("e_wing_evo2abig")
    );
    assert!(drawn("e_racingmirror") && !drawn("e_stockmirror"));
    assert!(drawn("e_bumperstrip_black") && !drawn("e_bumperstrip_mapped"));
    assert!(drawn("e_light_lf") && !drawn("e_light_lf_lit"));
    assert!(drawn("e_light_lb") && !drawn("e_light_lb_lit"));
    assert!(!drawn("e_bumperlight") && !drawn("e_bumperlight_lit"));
    assert!(!drawn("e_bumperlight_glass") && !drawn("e_bumperlight_glass2"));
    assert!(!drawn("e_mudflap_lf") && !drawn("e_roof") && !drawn("e_frontplate"));
    // Stop/reverse lamps keep the original off/on switch state.
    assert!(drawn("e_light_brake_lb") && !drawn("e_light_brake_lb_lit"));
}

#[test]
#[ignore]
fn bbox_of_exterior_parts() {
    let Some(root) = rbr_root() else { return };
    let folder = std::env::var("PART_AUDIT_CAR").unwrap_or_else(|_| "bmw_m3_e30".into());
    let (sgc_path, ini_path) = car_files(&root.join("Cars").join(&folder)).expect("car files");
    let elements = sgc::parse(&std::fs::read(sgc_path).unwrap()).unwrap();
    let parts = ini::parse(&String::from_utf8_lossy(&std::fs::read(ini_path).unwrap())).unwrap();
    let worlds = sgc::chained_worlds(&elements);
    for e in &elements {
        let sgc::Element::Mesh(m) = e else { continue };
        if !m.node.name.starts_with("e_")
            || m.node.name.contains("wheel")
            || m.node.name.contains("brakedisc")
        {
            continue;
        }
        let Some(sec) = car_parts::section(&parts, &m.node.name) else {
            continue;
        };
        let v = car_space_vertices(m, &elements, &worlds);
        if v.is_empty() {
            continue;
        }
        let min = v
            .iter()
            .fold(rerbr_render_core::Vec3::splat(1e9), |a, b| a.min(*b));
        let max = v
            .iter()
            .fold(rerbr_render_core::Vec3::splat(-1e9), |a, b| a.max(*b));
        println!(
            "BBOX {:<24} sw={:<5} min=({:>5.2},{:>5.2},{:>5.2}) max=({:>5.2},{:>5.2},{:>5.2})",
            m.node.name,
            sec.get("Switch").unwrap_or("-"),
            min.x,
            min.y,
            min.z,
            max.x,
            max.y,
            max.z
        );
    }
}

#[test]
#[ignore]
fn dump_parts_of_car() {
    let Some(root) = rbr_root() else { return };
    let folder = std::env::var("PART_AUDIT_CAR").unwrap_or_else(|_| "bmw_m3_e30".into());
    let (sgc_path, ini_path) = car_files(&root.join("Cars").join(&folder)).expect("car files");
    let elements = sgc::parse(&std::fs::read(sgc_path).unwrap()).unwrap();
    let parts = ini::parse(&String::from_utf8_lossy(&std::fs::read(ini_path).unwrap())).unwrap();
    let worlds = sgc::chained_worlds(&elements);
    for e in &elements {
        let sgc::Element::Mesh(m) = e else { continue };
        let sec = car_parts::section(&parts, &m.node.name);
        let w = worlds[&m.node.id];
        println!(
            "{:<28} tris={:<6} dmg={:<5} pos=({:>6.2},{:>6.2},{:>6.2}) half=({:>5.2},{:>5.2},{:>5.2}) sw={:?} pred={:?} shaders={:?} sel={:?}",
            m.node.name,
            m.triangles.len(),
            m.damageable,
            w[0][3], w[1][3], w[2][3],
            m.half_extent[0], m.half_extent[1], m.half_extent[2],
            sec.and_then(|s| s.get("Switch")),
            sec.and_then(|s| s.get("Predicate")),
            sec.map(|s| car_parts::face_blocks(s).iter().map(|b| b.shader.clone()).collect::<Vec<_>>()),
            selection::exterior(&m.node.name, &parts).map(|_| "draw").unwrap_or_else(|e| e),
        );
    }
}
