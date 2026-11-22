//! Read-only diagnostics against a Documents directory with an RBR installation.
use super::{load_showroom_model, load_vehicle_for_mode, Car};
use crate::driving::{
    career::vehicle_market_spec,
    content_manager::{Catalog, DriveCapability},
};
use rerbr_asset_formats::{car_parts, cars, ini, sgc};
use rerbr_content::{Selection, VehicleDescriptor};
use std::{collections::BTreeSet, fs, path::Path};

fn record(kind: &str, fields: &[&str]) {
    print!("AUDIT\t{kind}");
    for field in fields {
        print!("\t{}", field.replace('\t', " ").replace(['\r', '\n'], " "));
    }
    println!();
}

fn report_load(root: &Path, mode: &str, id: &str, loaded: Result<Car, String>) {
    let car = match loaded {
        Ok(car) => car,
        Err(error) => {
            record("error", &[mode, id, &error]);
            return;
        }
    };
    let g = &car.geometry;
    record(
        "loaded",
        &[
            mode,
            id,
            &g.draws.len().to_string(),
            &g.stats(None).triangles.to_string(),
            &car.textures.len().to_string(),
            &g.unresolved_faces.len().to_string(),
            &format!("{:?}", car.cockpit_error),
            &format!("{:?}", g.pivots),
            &format!("{:?}", g.wheel_visual_offsets),
        ],
    );
    for file in &car.files {
        record("file", &[mode, id, file]);
    }
    if mode == "showroom" {
        report_panels(root, &car);
    }
    for face in &g.unresolved_faces {
        record("unresolved", &[mode, id, face]);
    }
    for part in &g.drawn {
        record("drawn", &[mode, id, part]);
    }
    for (part, reason) in &g.skipped {
        record("skipped", &[mode, id, part, reason]);
    }
    let draws: BTreeSet<_> = g
        .draws
        .iter()
        .map(|draw| (draw.slot, draw.interior, draw.material))
        .collect();
    for (slot, interior, index) in draws {
        let m = &g.materials[index];
        let texture = m.path.as_ref().and_then(|p| car.textures.get(p));
        record(
            "material",
            &[
                mode,
                id,
                &slot.to_string(),
                if interior { "interior" } else { "exterior" },
                &m.shader,
                &m.name,
                &format!("{:?}", m.pass),
                m.path.as_deref().unwrap_or(""),
                texture.map_or("", |t| t.info.resolved.as_str()),
                &texture.map_or(String::new(), |t| format!("{:?}", t.info.format)),
            ],
        );
    }
}

fn report_panels(root: &Path, car: &Car) {
    let d = &car.vehicle;
    let model = cars::resolve(root, &d.model).unwrap();
    let shader_file = cars::resolve(root, &d.shaders).unwrap();
    let elements = sgc::parse(&fs::read(&model).unwrap()).unwrap();
    let shaders = ini::parse(&String::from_utf8_lossy(&fs::read(shader_file).unwrap())).unwrap();
    for draw in &car.geometry.draws {
        let (index, face_index) = draw.source;
        let sgc::Element::Mesh(mesh) = &elements[index] else {
            unreachable!()
        };
        let name = mesh.node.name.as_str();
        if !matches!(
            name,
            "e_bonnet" | "e_door_l" | "e_door_r" | "e_door_b" | "e_bumper_f" | "e_bumper_b"
        ) {
            continue;
        }
        let face = &mesh.faces[face_index];
        let original = car_parts::diffuse_path(&shaders, &face.name).unwrap_or("");
        let material = &car.geometry.materials[draw.material];
        let resolved = material.path.as_ref().and_then(|p| car.textures.get(p));
        record(
            "panel",
            &[
                &d.id,
                &d.model,
                name,
                &index.to_string(),
                &mesh.node.id.to_string(),
                &face_index.to_string(),
                &face.prim_count.to_string(),
                &face.name,
                original,
                &material.name,
                resolved.map_or("", |texture| texture.info.resolved.as_str()),
            ],
        );
    }
}

fn audit_models(root: &Path, folder: &Path, listed: &BTreeSet<String>) {
    let mut entries: Vec<_> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            audit_models(root, &path, listed);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("sgc"))
        {
            let model = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(path).unwrap();
            match sgc::parse(&bytes) {
                Ok(parsed) => record("sgc", &[&model, &parsed.len().to_string()]),
                Err(error) => record("sgc-error", &[&model, &error.to_string()]),
            }
            if model.matches('/').count() == 2 && !listed.contains(&model.to_ascii_lowercase()) {
                let stem = model.rsplit_once('.').unwrap().0;
                let mut descriptor = VehicleDescriptor::ford_focus_2006();
                descriptor.model = model.clone();
                descriptor.parts = format!("{stem}.ini");
                descriptor.shaders = format!("{stem}_shaders.ini");
                descriptor.shader_settings = format!("{stem}_shader_settings_clear.ini");
                report_load(
                    root,
                    "unlisted",
                    &model,
                    load_showroom_model(root, &descriptor),
                );
            }
        }
    }
}

fn inspect_interior(root: &Path, d: &VehicleDescriptor) -> Result<super::mesh::Geometry, String> {
    let model = d.model.replace('\\', "/");
    let (dir, _) = model.rsplit_once('/').ok_or("missing model directory")?;
    let read_ini = |path: &str| {
        let bytes = super::read_showroom_file(root, dir, path, 2 * 1024 * 1024)?;
        ini::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("{path}: {e}"))
    };
    let bytes = super::read_showroom_file(root, dir, &model, 128 * 1024 * 1024)?;
    let elements = sgc::parse(&bytes).map_err(|e| e.to_string())?;
    let settings = read_ini(&d.shader_settings).or_else(|err| {
        d.shader_settings
            .strip_suffix("_clear.ini")
            .map_or(Err(err), |s| read_ini(&format!("{s}_overcast.ini")))
    })?;
    let parts = read_ini(&d.parts)?;
    let shader_bytes = super::read_showroom_file(root, dir, &d.shaders, 2 * 1024 * 1024)?;
    let shaders = ini::parse(&String::from_utf8_lossy(&shader_bytes)).map_err(|e| e.to_string())?;
    let g = super::mesh::convert_interior_for_mode(&elements, &parts, &shaders, &settings, true)?;
    if !g.draws.iter().any(|d| d.slot == 5) {
        return Err("COCKPIT: no steering wheel draw".into());
    }
    let mut texture_source = super::ShowroomTextures {
        root,
        model_dir: dir,
        model: (&d.model, &bytes),
        shaders: (&d.shaders, &shader_bytes),
        aliases: Default::default(),
    };
    for path in g
        .materials
        .iter()
        .filter_map(|m| m.path.as_deref())
        .collect::<BTreeSet<_>>()
    {
        let (resolved, bytes) = texture_source.read(path)?;
        let tex = super::prepare_texture(path, &resolved, bytes)?;
        record(
            "interior-dds",
            &[&d.id, path, &resolved, &format!("{:?}", tex.info.format)],
        );
    }
    Ok(g)
}

#[test]
#[ignore = "requires a private RBR installation"]
fn audit_vehicle_interiors() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    for entry in &catalog.vehicles {
        if !entry.has_3d_model || entry.using_fallback_model || entry.model_missing {
            continue;
        }
        let id = &entry.descriptor.id;
        match inspect_interior(&root, &entry.descriptor) {
            Ok(g) => {
                record(
                    "interior",
                    &[
                        id,
                        &g.draws.len().to_string(),
                        &g.unresolved_faces.len().to_string(),
                    ],
                );
                for part in &g.drawn {
                    record("drawn", &["interior", id, part]);
                }
                for face in &g.unresolved_faces {
                    record("unresolved", &["interior", id, face]);
                }
            }
            Err(error) => record("error", &["interior", id, &error]),
        }
    }
}

#[test]
#[ignore = "requires a private RBR installation"]
fn audit_vehicle_resources() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let car_dir = cars::resolve(&root, "Cars").expect("missing Documents/RBR/Cars");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let mut listed = BTreeSet::new();
    for entry in &catalog.vehicles {
        let d = &entry.descriptor;
        let model = d.model.replace('\\', "/");
        listed.insert(model.to_ascii_lowercase());
        record(
            "catalog",
            &[
                &d.id,
                &d.name,
                &model,
                &d.parts,
                &d.shaders,
                &d.shader_settings,
                d.rsf_pack.as_deref().unwrap_or(""),
                &format!("{:?}", entry.capability(true)),
                &format!("{:?}", vehicle_market_spec(entry).concesionario),
                &entry.using_fallback_model.to_string(),
                &entry.model_missing.to_string(),
                &entry.files.missing.join("; "),
            ],
        );
        if entry.has_3d_model && !entry.model_missing && !entry.using_fallback_model {
            report_load(&root, "showroom", &d.id, load_showroom_model(&root, d));
        }
        if entry.capability(true) == DriveCapability::Drivable {
            report_load(
                &root,
                "driving",
                &d.id,
                load_vehicle_for_mode(&root, d, true),
            );
        }
    }
    audit_models(&root, &car_dir, &listed);
}

#[test]
#[ignore = "requires the real BMW M3 E30 and Citroen DS3 WRC resources"]
fn bmw_and_ds3_original_face_block_regressions() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let descriptor = |id| &catalog.vehicle(id).expect("vehicle in catalog").descriptor;
    let bmw = descriptor("rsf-bmw-m3-e30-grpa");
    let ds3 = descriptor("rsf-citroen-ds3-wrc");
    let load_bmw = || load_showroom_model(&root, bmw).expect("original BMW assets");
    let first_bmw = load_bmw();
    let panels = ["e_body", "e_wing_evo1"];
    for panel in panels {
        assert!(
            first_bmw.geometry.drawn.iter().any(|n| n == panel),
            "{panel}"
        );
        assert!(
            first_bmw
                .geometry
                .unresolved_faces
                .iter()
                .all(|f| !f.starts_with(&format!("{panel} "))),
            "{panel}"
        );
    }
    let paint = "cars\\bmw_m3_e30\\textures\\bmw_m3_e30_external.dds";
    let source = cars::resolve(&root, &bmw.model).unwrap();
    let sgc = sgc::parse(&fs::read(source).unwrap()).unwrap();
    for panel in panels {
        let (record, owner) = sgc
            .iter()
            .enumerate()
            .find_map(|(record, element)| match element {
                sgc::Element::Mesh(owner) if owner.node.name == panel => Some((record, owner)),
                _ => None,
            })
            .unwrap();
        let draw = first_bmw
            .geometry
            .draws
            .iter()
            .find(|draw| draw.source == (record, 0))
            .unwrap();
        assert_eq!(draw.indices.len(), owner.triangles.len() * 3, "{panel}");
        let material = &first_bmw.geometry.materials[draw.material];
        assert_eq!(material.path.as_deref(), Some(paint), "{panel}");
    }
    assert!(first_bmw.textures.contains_key(paint));
    let original = cars::resolve(&root, paint).unwrap();
    assert_eq!(first_bmw.textures[paint].bytes, fs::read(original).unwrap());

    let wheels = "cars\\ds3_wrc\\textures\\ds3_wrc_external_dynamic.dds";
    let ds3 = load_showroom_model(&root, ds3).expect("original DS3 assets");
    assert!(ds3.textures.contains_key(wheels));
    for slot in 1..=4 {
        assert!(
            ds3.geometry.draws.iter().any(|d| {
                let m = &ds3.geometry.materials[d.material];
                d.slot == slot && m.shader == "CarTire" && m.path.as_deref() == Some(wheels)
            }),
            "DS3 wheel slot {slot}"
        );
    }
    assert!(ds3
        .geometry
        .unresolved_faces
        .iter()
        .all(|f| !f.starts_with("e_wheel_gravel_")));

    let next_bmw = load_bmw();
    assert_eq!(first_bmw.files, next_bmw.files);
    assert_eq!(first_bmw.geometry.indices, next_bmw.geometry.indices);
    assert_eq!(
        first_bmw.geometry.unresolved_faces,
        next_bmw.geometry.unresolved_faces
    );
    assert!(!next_bmw.textures.contains_key(wheels));
}

#[test]
#[ignore = "requires both original Lancer Evo X V1.21 directories"]
fn lancer_evo_x_showroom_uses_the_declared_atlas() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let load = |id| {
        let vehicle = catalog.vehicle(id).expect("Lancer Evo X in catalog");
        load_showroom_model(&root, &vehicle.descriptor).expect("original Lancer Evo X assets")
    };
    let alias = load("rsf-lancer-evo-x");
    let declared = load("rsf-mitsubishi-lancer-evo-x-r4");
    assert_eq!(alias.textures.len(), 6);
    assert_eq!(alias.textures.len(), declared.textures.len());
    for (path, texture) in &alias.textures {
        assert!(path.starts_with("cars\\lancer_evo_x_v1_21\\textures\\"));
        assert!(rerbr_content::same_path(path, &texture.info.resolved));
        assert_eq!(texture.bytes, declared.textures[path].bytes);
    }
    assert_eq!(alias.geometry.indices, declared.geometry.indices);
    assert_ne!(alias.vehicle.id, declared.vehicle.id);
}

#[test]
#[ignore = "requires original RSF resources"]
fn bmw_and_lancia_use_original_shell_and_rear_lens_geometry() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let load = |id| {
        let descriptor = &catalog.vehicle(id).unwrap().descriptor;
        let path = cars::resolve(&root, &descriptor.model).unwrap();
        let elements = sgc::parse(&fs::read(path).unwrap()).unwrap();
        (load_showroom_model(&root, descriptor).unwrap(), elements)
    };
    let (bmw, bmw_model) = load("rsf-bmw-m3-e30-grpa");
    for name in [
        "e_door_l",
        "e_door_r",
        "e_door_b",
        "e_bumper_f",
        "e_bumper_b",
        "e_bonnet",
        "e_bonnetvents",
        "e_bumpervents",
    ] {
        assert!(
            bmw.geometry
                .skipped
                .iter()
                .any(|(part, why)| part == name && why.contains("alternate shell")),
            "{name}"
        );
    }
    for (name, triangles) in [
        ("e_body", 11402),
        ("e_wing_evo1", 424),
        ("e_bumperstrip_black", 82),
    ] {
        let render: usize = bmw
            .geometry
            .draws
            .iter()
            .filter(|draw| match &bmw_model[draw.source.0] {
                sgc::Element::Mesh(mesh) => mesh.node.name == name,
                _ => false,
            })
            .map(|draw| draw.indices.len() / 3)
            .sum();
        assert_eq!(render, triangles, "{name}");
    }
    let driving = load_vehicle_for_mode(&root, &bmw.vehicle, true).unwrap();
    assert!(driving.cockpit.is_some(), "{:?}", driving.cockpit_error);
    for (record, item) in bmw_model.iter().enumerate() {
        let sgc::Element::Mesh(mesh) = item else {
            continue;
        };
        if matches!(mesh.node.id, 2878 | 2882 | 2967 | 2974) {
            let drawn = driving
                .geometry
                .draws
                .iter()
                .any(|draw| draw.interior && draw.source.0 == record);
            assert_eq!(
                drawn,
                matches!(mesh.node.id, 2878 | 2882),
                "{}",
                mesh.node.id
            );
        }
    }
    let (lancia, lancia_model) = load("rsf-lancia-delta-hf-4wd-grpa");
    for side in ["lb", "rb"] {
        for (name, pass, atlas) in [
            (
                format!("e_lightglass_{side}"),
                super::materials::Pass::CoplanarGlass,
                "delta_4wd_external_transparent.dds",
            ),
            (
                format!("e_light_brake_{side}"),
                super::materials::Pass::Opaque,
                "delta_4wd_external_dynamic.dds",
            ),
        ] {
            let (record, mesh) = lancia_model
                .iter()
                .enumerate()
                .find_map(|(index, item)| match item {
                    sgc::Element::Mesh(mesh) if mesh.node.name == name => Some((index, mesh)),
                    _ => None,
                })
                .unwrap();
            let draw = lancia
                .geometry
                .draws
                .iter()
                .find(|d| d.source == (record, 0))
                .unwrap();
            let material = &lancia.geometry.materials[draw.material];
            assert_eq!(material.pass, pass, "{name}");
            assert!(material.path.as_deref().unwrap().ends_with(atlas), "{name}");
            assert_eq!(draw.indices.len(), mesh.triangles.len() * 3);
            for (&index, &original) in lancia.geometry.indices[draw.indices.clone()]
                .iter()
                .zip(mesh.triangles.iter().flatten())
            {
                assert_eq!(
                    lancia.geometry.vertices[index as usize].uv,
                    mesh.uvs[original as usize]
                );
            }
        }
    }
}

#[test]
#[ignore = "requires original RSF resources"]
fn beta_rsfs_authored_variants_and_wheel_atlases() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let load = |id| load_showroom_model(&root, &catalog.vehicle(id).unwrap().descriptor).unwrap();
    let bmw = load("rsf-bmw-m3-e30-grpa");
    assert!(bmw.geometry.drawn.iter().any(|p| p == "e_wing_evo1"));
    assert!(!bmw
        .geometry
        .drawn
        .iter()
        .any(|p| p == "e_wing" || p == "e_mirror_l" || p == "e_mirror_r"));
    let c4 = load("rsf-citroen-c4-wrc-2008");
    for side in ["l", "r"] {
        assert!(c4
            .geometry
            .drawn
            .iter()
            .any(|p| p == &format!("e_mirror_{side}")));
        assert!(c4
            .geometry
            .drawn
            .iter()
            .any(|p| p == &format!("e_mirror_{side}_glass")));
    }
    assert!(!c4
        .geometry
        .drawn
        .iter()
        .any(|p| p.starts_with("e_mirror_1_")));
    let yaris = load("rsf-toyota-yaris-wrc-2018");
    for (preset, lamps) in [
        (
            "light_on_bonnet",
            ["e_bonnet_ramp", "e_extralights_lit", "e_extralights_glass"],
        ),
        (
            "light_on_all",
            [
                "e_bumper_ramp_l",
                "e_extralights2_l_lit",
                "e_extralights_glass2_l",
            ],
        ),
    ] {
        let preset = fs::read_to_string(
            root.join(format!("Cars/YARIS_WRC18/carparts/{preset}/yaris_wrc.ini")),
        )
        .unwrap();
        let preset = ini::parse(&preset).unwrap();
        for name in lamps {
            assert!(!yaris.geometry.drawn.iter().any(|p| p == name), "{name}");
            assert!(
                super::selection::exterior_with_lamps(name, &preset, |_| true).is_ok(),
                "{name}"
            );
        }
    }
    assert!(!yaris.geometry.drawn.iter().any(|p| p == "e_bumper_ramp_r"));
    for (id, part) in [
        ("rsf-ford-fiesta-wrc-2019", "e_tapas"),
        ("rsf-hyundai-i20-coupe-wrc-2020", "e_diveplates"),
    ] {
        let car = load(id);
        assert!(car.geometry.drawn.iter().any(|p| p == part), "{id}: {part}");
        assert!(
            !car.geometry
                .unresolved_faces
                .iter()
                .any(|p| p.starts_with(&format!("{part} "))),
            "{id}: {part}"
        );
        if part == "e_diveplates" {
            assert!(!car.geometry.drawn.iter().any(|p| p == "e_bonnet_ramp"));
        }
    }
    for (id, atlas) in [
        (
            "rsf-skoda-fabia-rs-rally2",
            "fabia_rs_rally2_external_dynamic.dds",
        ),
        (
            "rsf-toyota-gr-yaris-rally2",
            "yaris_rally2_external_dynamic.dds",
        ),
    ] {
        let car = load(id);
        assert!(
            !car.geometry
                .unresolved_faces
                .iter()
                .any(|p| p.starts_with("e_wheel_gravel_")),
            "{id}"
        );
        for slot in 1..=4 {
            for shader in ["CarTire", "CarRim"] {
                assert!(
                    car.geometry.draws.iter().any(|d| {
                        let material = &car.geometry.materials[d.material];
                        d.slot == slot
                            && material.shader == shader
                            && material.path.as_deref().is_some_and(|p| p.ends_with(atlas))
                    }),
                    "{id} slot {slot} {shader}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires original Fiesta Rally2 metadata, pack and complete numbered visual copy"]
fn beta_fiesta_relocated_atlas_loads_in_driving_without_retail_index() {
    let docs = std::env::var_os("VEHICLE_AUDIT_DOCUMENTS").expect("set VEHICLE_AUDIT_DOCUMENTS");
    let docs = Path::new(&docs);
    let root = docs.join("RBR");
    let catalog = Catalog::scan_root(docs, &Selection::default());
    let entry = catalog.vehicle("rsf-ford-fiesta-rally2").unwrap();
    assert!(!root.join("Cars/Cars.ini").exists());
    assert_eq!(
        entry.capability(true),
        DriveCapability::Drivable,
        "{:?}",
        entry.files.missing
    );
    let vehicle = &entry.descriptor;
    let pack = crate::driving::ngp_experimental::inspect_pack_state(&root, vehicle);
    assert!(matches!(
        pack,
        crate::driving::ngp_experimental::PackAuditState::Ready { .. }
    ));
    let car = load_vehicle_for_mode(&root, vehicle, true).expect("original driving assets");
    assert!(car.cockpit.is_some(), "{:?}", car.cockpit_error);
    assert!(car.textures.values().all(|t| t
        .info
        .resolved
        .to_ascii_lowercase()
        .starts_with("cars/fiesta_rally2_23 2/textures/")));
}

#[test]
#[ignore = "requires the original model and parts file"]
fn dump_face_blocks() {
    let sgc_path = std::env::var_os("VEHICLE_AUDIT_SGC").expect("set VEHICLE_AUDIT_SGC");
    let sgc_path = Path::new(&sgc_path);
    let parts = fs::read(sgc_path.with_extension("ini")).unwrap();
    let parts = ini::parse(&String::from_utf8_lossy(&parts)).unwrap();
    let elements = sgc::parse(&fs::read(sgc_path).unwrap()).unwrap();
    for element in elements {
        if let sgc::Element::Mesh(mesh) = element {
            if let Some(part) = car_parts::section(&parts, &mesh.node.name) {
                record(
                    "faces",
                    &[
                        &mesh.node.name,
                        &format!("{:?}", mesh.faces),
                        &format!("{:?}", car_parts::face_blocks(part)),
                    ],
                );
            }
        }
    }
}
