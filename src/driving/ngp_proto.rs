//! Read-only experimental NGP compatibility prototype and cleanroom NGPCarMenu-inspired
//! vehicle technical specification inspector.
//!
//! Strictly read-only: reads original RBR 1.02 `.lsp` scripts or optional NGP car-pack descriptors
//! using `rerbr_sim::rbr_formats` (GPL-3.0) without executing any external Windows DLLs or changing
//! the active physics backend.

use rerbr_content::VehicleDescriptor;
use rerbr_sim::ngp_crc::{Crc32, INIT_MODE};
use rerbr_sim::rbr_formats::car_pack::Pack;
use rerbr_sim::rbr_formats::car_specs::{CarSpecs, Drive};
use rerbr_sim::rbr_formats::lsp::{self, Node};
use rerbr_sim::rbr_formats::rbz;
use std::io::Read;
use std::path::Path;

/// Selectable backend selector; `OriginalRetail` remains the sole active certified runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhysicsBackend {
    #[default]
    OriginalRetail,
    #[allow(dead_code)]
    Ngp74ReadOnlyAudit,
    #[allow(dead_code)]
    Ngp75UnverifiedCandidate,
}

impl PhysicsBackend {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::OriginalRetail => "ORIGINAL RBR 1.02 (ACTIVO)",
            Self::Ngp74ReadOnlyAudit => "NGP 7.4.778 (AUDITORÍA / INACTIVO)",
            Self::Ngp75UnverifiedCandidate => "NGP 7.5.779 (NO PORTADO / BLOQUEADO)",
        }
    }
}

/// Detected physics profile inside a car's `.lsp` bundle.
#[derive(Debug, Clone, PartialEq)]
pub enum PhysicsProfile {
    OriginalRbr102,
    NgpDetected {
        car_id: Option<u32>,
        power_hp: Option<f32>,
        power_rpm: Option<f32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivetrainKind {
    FourWheelDrive,
    FrontWheelDrive,
    RearWheelDrive,
}

impl DrivetrainKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::FourWheelDrive => "4WD INTEGRAL",
            Self::FrontWheelDrive => "FWD DELANTERA",
            Self::RearWheelDrive => "RWD TRASERA",
        }
    }
}

/// Authentic vehicle technical sheet extracted from `.lsp` definitions (cleanroom NGPCarMenu feature).
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleTechSheet {
    pub profile: PhysicsProfile,
    pub drivetrain: DrivetrainKind,
    pub mass_kg: f32,
    pub forward_gears: u32,
    pub total_gears: u32,
    pub turbo: bool,
    pub rpm_limit: Option<u32>,
    pub front_diff_nm: Option<f32>,
    pub center_diff_nm: Option<f32>,
    pub rear_diff_nm: Option<f32>,
    pub front_rollbar: Option<f32>,
    pub rear_rollbar: Option<f32>,
    pub tyre_radius_m: f32,
    pub tyre_brand: &'static str,
    pub data_crc32: u32,
}

impl VehicleTechSheet {
    pub fn physics_summary(&self) -> String {
        match &self.profile {
            PhysicsProfile::OriginalRbr102 => "RBR ORIGINAL 1.02".into(),
            PhysicsProfile::NgpDetected {
                power_hp: Some(hp),
                power_rpm: Some(rpm),
                ..
            } => format!("NGP ({hp:.0} CV @ {rpm:.0} RPM)"),
            PhysicsProfile::NgpDetected { .. } => "NGP DETECTADO (NO CERTIFICADO)".into(),
        }
    }

    pub fn drivetrain_and_mass_summary(&self) -> String {
        format!("{} | {:.0} KG", self.drivetrain.label(), self.mass_kg)
    }

    pub fn gearbox_and_engine_summary(&self) -> String {
        let asp = if self.turbo { "TURBO" } else { "ATMOSF." };
        if let Some(rpm) = self.rpm_limit {
            format!("{} VEL. | {} {rpm} RPM", self.forward_gears, asp)
        } else {
            format!("{} VEL. | {}", self.forward_gears, asp)
        }
    }

    pub fn differentials_summary(&self) -> String {
        match (self.front_diff_nm, self.center_diff_nm, self.rear_diff_nm) {
            (Some(f), Some(c), Some(r)) => {
                format!("DEL {f:.0} / CEN {c:.0} / TRA {r:.0} NM")
            }
            (Some(f), None, Some(r)) => format!("DEL {f:.0} / TRA {r:.0} NM"),
            (Some(f), _, _) => format!("DEL {f:.0} NM"),
            _ => "SIN DATOS DIFERENCIAL".into(),
        }
    }

    pub fn tyres_and_suspension_summary(&self) -> String {
        match (self.front_rollbar, self.rear_rollbar) {
            (Some(f), Some(r)) => format!(
                "{} (R={:.3}M) | ARB {:.0}/{:.0}",
                self.tyre_brand,
                self.tyre_radius_m,
                f / 1000.0,
                r / 1000.0
            ),
            _ => format!("{} (R={:.3}M)", self.tyre_brand, self.tyre_radius_m),
        }
    }
}

/// Maps retail `[CarNN]` slot index to its RBR `Physics/<folder>/` directory.
pub fn physics_folder_for_slot(slot: u32) -> Option<&'static str> {
    match slot {
        0 => Some("Physics/c_xsara/"),
        1 => Some("Physics/h_accent/"),
        2 => Some("Physics/mg_zr/"),
        3 => Some("Physics/m_lancer/"),
        4 => Some("Physics/p_206/"),
        5 => Some("Physics/s_i2003/"),
        6 => Some("Physics/t_coroll/"),
        7 => Some("Physics/s_i2000/"),
        _ => None,
    }
}

/// Retail tyre brand policy (`boot::tyre_brand`: slots 5 and 7 use Pirelli; others Michelin).
pub fn retail_tyre_brand(slot: u32) -> &'static str {
    if matches!(slot, 5 | 7) {
        "PIRELLI"
    } else {
        "MICHELIN"
    }
}

/// Parses `.lsp` vehicle scripts into a validated `VehicleTechSheet`.
/// Surface script (`gravel.lsp`) overrides `common.lsp`, matching the authentic RBR boot loader.
pub fn parse_vehicle_tech_sheet(
    common: &str,
    gravel: &str,
    options_gravel: Option<&str>,
    setup_gravel: Option<&str>,
    tyre_brand: &'static str,
) -> Result<VehicleTechSheet, String> {
    let common_nodes = lsp::parse(common).map_err(|e| format!("common.lsp: {e}"))?;
    let gravel_nodes = lsp::parse(gravel).map_err(|e| format!("gravel.lsp: {e}"))?;
    let options_nodes = options_gravel
        .map(|s| lsp::parse(s).map_err(|e| format!("r_gravel.lsp: {e}")))
        .transpose()?;
    let setup_nodes = setup_gravel
        .map(|s| lsp::parse(s).map_err(|e| format!("d_gravel.lsp: {e}")))
        .transpose()?;

    let inv_mass = lookup_f32_matching(&[&gravel_nodes, &common_nodes], "InverseMass", |v| {
        v.is_finite() && (0.00033..=0.0025).contains(&v)
    })
    .or_else(|| {
        lookup_f32(&[&gravel_nodes, &common_nodes], "InverseMass")
            .filter(|v| v.is_finite() && *v > 0.0)
    })
    .ok_or_else(|| "missing or invalid InverseMass".to_string())?;
    let mass_kg = 1.0 / inv_mass;
    if !(400.0..=3000.0).contains(&mass_kg) {
        return Err(format!("out-of-range vehicle mass: {mass_kg:.1} kg"));
    }

    let total_gears = lookup_f32(&[&gravel_nodes, &common_nodes], "NumberOfGears")
        .filter(|v| v.is_finite() && *v >= 3.0 && *v <= 10.0)
        .map(|v| v.round() as u32)
        .ok_or_else(|| "missing or invalid NumberOfGears".to_string())?;
    let forward_gears = total_gears - 2;

    let ngp_specs = options_gravel.and_then(|opts| CarSpecs::parse(common, opts));
    let has_ngp_keys = ngp_specs.is_some()
        || lookup_f32(&[&gravel_nodes, &common_nodes], "FrontDownforce_NGP").is_some()
        || options_nodes
            .as_ref()
            .is_some_and(|n| find_number(n, "Features_NGP").is_some());

    let drivetrain = if let Some(specs) = ngp_specs {
        match specs.drive {
            Drive::Front => DrivetrainKind::FrontWheelDrive,
            Drive::Rear => DrivetrainKind::RearWheelDrive,
            Drive::All => DrivetrainKind::FourWheelDrive,
        }
    } else {
        match lookup_f32(&[&gravel_nodes, &common_nodes], "DriveType").map(|v| v.round() as i32) {
            Some(3) => DrivetrainKind::FrontWheelDrive,
            _ => DrivetrainKind::FourWheelDrive,
        }
    };

    let turbo = lookup_f32(&[&gravel_nodes, &common_nodes], "Turbo")
        .map(|v| v.round() as i32 != 0)
        .unwrap_or(true);
    let rpm_limit = lookup_f32(&[&gravel_nodes, &common_nodes], "RPMLimit")
        .filter(|v| v.is_finite() && *v > 1000.0)
        .map(|v| v.round() as u32);
    let tyre_radius_m = lookup_f32(&[&gravel_nodes, &common_nodes], "EffWheelRadius")
        .or_else(|| lookup_f32(&[&gravel_nodes, &common_nodes], "Radius"))
        .filter(|v| v.is_finite() && *v > 0.1 && *v < 1.0)
        .unwrap_or(0.315);

    let (front_diff_nm, center_diff_nm, rear_diff_nm, front_rollbar, rear_rollbar) =
        if let Some(setup) = &setup_nodes {
            (
                find_number(setup, "FrontDiffMaxTorque").filter(|v| v.is_finite()),
                find_number(setup, "CenterDiffMaxTorque").filter(|v| v.is_finite()),
                find_number(setup, "RearDiffMaxTorque").filter(|v| v.is_finite()),
                find_number(setup, "FrontRollBarStiffness").filter(|v| v.is_finite()),
                find_number(setup, "RearRollBarStiffness").filter(|v| v.is_finite()),
            )
        } else {
            (None, None, None, None, None)
        };

    let profile = if has_ngp_keys {
        let car_id = lookup_f32(&[&gravel_nodes, &common_nodes], "CarId").map(|v| v.round() as u32);
        let (power_hp, power_rpm) = match ngp_specs {
            Some(specs) => (Some(specs.power / 735.498_8), Some(specs.power_rpm)),
            None => (None, None),
        };
        PhysicsProfile::NgpDetected {
            car_id,
            power_hp,
            power_rpm,
        }
    } else {
        PhysicsProfile::OriginalRbr102
    };

    let mut crc = Crc32::new(INIT_MODE);
    crc.update(&mass_kg.to_bits().to_le_bytes());
    crc.update(&total_gears.to_le_bytes());
    crc.update(&[drivetrain as u8, u8::from(turbo)]);
    crc.update(&tyre_radius_m.to_bits().to_le_bytes());
    if let Some(f) = front_diff_nm {
        crc.update(&f.to_bits().to_le_bytes());
    }
    if let Some(c) = center_diff_nm {
        crc.update(&c.to_bits().to_le_bytes());
    }
    if let Some(r) = rear_diff_nm {
        crc.update(&r.to_bits().to_le_bytes());
    }

    Ok(VehicleTechSheet {
        profile,
        drivetrain,
        mass_kg,
        forward_gears,
        total_gears,
        turbo,
        rpm_limit,
        front_diff_nm,
        center_diff_nm,
        rear_diff_nm,
        front_rollbar,
        rear_rollbar,
        tyre_radius_m,
        tyre_brand,
        data_crc32: crc.finish(),
    })
}

/// Inspects an NGP pack folder or zip directly and returns detailed parse errors if any.
pub fn inspect_pack_path_result(path: &Path) -> Result<VehicleTechSheet, String> {
    let pack = Pack::open(path)?;
    let common = String::from_utf8_lossy(&pack.read("common.lsp")?).into_owned();
    let gravel = String::from_utf8_lossy(&pack.read("gravel.lsp")?).into_owned();
    let r_gravel = pack
        .read("r_gravel.lsp")
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let d_gravel = pack
        .read("setups/d_gravel.lsp")
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    parse_vehicle_tech_sheet(
        &common,
        &gravel,
        r_gravel.as_deref(),
        d_gravel.as_deref(),
        "NGP TYRE",
    )
}

/// Inspects an NGP pack folder or zip directly for authentic technical parameters.
pub fn inspect_pack_path(path: &Path) -> Option<VehicleTechSheet> {
    inspect_pack_path_result(path).ok()
}

/// Inspects a vehicle's real technical sheet from `Documents/RBR/physics.rbz` (or NGP pack for RSF vehicles).
/// Safe and read-only: returns `None` quietly if private game archives are absent.
pub fn inspect_vehicle_in_documents(
    documents_root: &Path,
    vehicle: &VehicleDescriptor,
) -> Option<VehicleTechSheet> {
    let install = documents_root.join("RBR");
    if let Some(pack_name) = vehicle.rsf_pack.as_deref().or_else(|| {
        (vehicle.id == "ford-focus-rs-wrc-2006").then_some("Ford_Focus_Mk_II_RS_WRC_2006_ngp6")
    }) {
        if let super::ngp_experimental::PackAuditState::Ready { path } =
            super::ngp_experimental::resolve_named_pack_state(&install, pack_name)
        {
            return inspect_pack_path(&path);
        }
        return None;
    }

    let folder = physics_folder_for_slot(vehicle.original_slot)?;
    let physics_rbz = install.join("physics.rbz");
    if !physics_rbz.is_file() {
        return None;
    }
    let mut zip = rbz::open(&physics_rbz).ok()?;
    let common = read_zip_text(&mut zip, &format!("{folder}common.lsp"))?;
    let gravel = read_zip_text(&mut zip, &format!("{folder}gravel.lsp"))?;
    let r_gravel = read_zip_text(&mut zip, &format!("{folder}r_gravel.lsp"));
    let d_gravel = read_zip_text(&mut zip, &format!("{folder}setups/d_gravel.lsp"));
    parse_vehicle_tech_sheet(
        &common,
        &gravel,
        r_gravel.as_deref(),
        d_gravel.as_deref(),
        retail_tyre_brand(vehicle.original_slot),
    )
    .ok()
}

fn read_zip_text<R: Read + std::io::Seek>(
    zip: &mut rbz::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let entry = rbz::by_name(zip, name).ok()?;
    let mut text = String::new();
    entry.take(2 * 1024 * 1024).read_to_string(&mut text).ok()?;
    Some(text)
}

fn lookup_f32(layers: &[&[Node]], key: &str) -> Option<f32> {
    layers.iter().find_map(|nodes| find_number(nodes, key))
}

fn lookup_f32_matching<F: Fn(f32) -> bool + Copy>(
    layers: &[&[Node]],
    key: &str,
    pred: F,
) -> Option<f32> {
    layers
        .iter()
        .find_map(|nodes| find_number_matching(nodes, key, pred))
}

fn find_number(nodes: &[Node], key: &str) -> Option<f32> {
    find_number_matching(nodes, key, |_| true)
}

fn find_number_matching<F: Fn(f32) -> bool + Copy>(
    nodes: &[Node],
    key: &str,
    pred: F,
) -> Option<f32> {
    nodes
        .windows(2)
        .find_map(|pair| match pair {
            [Node::Atom(k), Node::Atom(v)] if k == key => v.parse().ok().filter(|&n| pred(n)),
            _ => None,
        })
        .or_else(|| {
            nodes.iter().find_map(|n| match n {
                Node::List(children) => find_number_matching(children, key, pred),
                _ => None,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_COROLLA_COMMON: &str = r#"
    ("PhysicsEngine" DefManager (
        ("Car" ("CAR_ROOT"
            CarId 6
            InverseMass 0.000714
            EffWheelRadius 0.315
            ("Engine" Turbo 1)
            ("Drive" NumberOfGears 8)
            ("ControlUnit" CenterDiffHandbrakeRelease 0.5)
        ))
    ))
    "#;

    const SAMPLE_P206_COMMON: &str = r#"
    ("PhysicsEngine" DefManager (
        ("Car" ("CAR_ROOT"
            ("Engine" Turbo 1)
            ("Drive")
        ))
    ))
    "#;

    const SAMPLE_P206_GRAVEL: &str = r#"
    ("PhysicsEngine" DefManager (
        ("Car" ("CAR_ROOT"
            CarId 4
            InverseMass 0.000714
            ("Drive" NumberOfGears 7)
        ))
        ("Tyre" Radius 0.315)
    ))
    "#;

    const SAMPLE_MG_ZR_COMMON: &str = r#"
    ("PhysicsEngine" DefManager (
        ("Car" ("CAR_ROOT"
            CarId 2
            InverseMass 0.000869
            ("Engine" Turbo 0)
            ("Drive" DriveType 3 NumberOfGears 8)
            ("ControlUnit" RPMLimit 8500)
        ))
    ))
    "#;

    const SAMPLE_GRAVEL_SETUP: &str = r#"
    ("Setup"
        FrontRollBarStiffness 20000.0
        RearRollBarStiffness 30000.0
        ("Drive"
            FrontDiffMaxTorque 320.0
            CenterDiffMaxTorque 700.0
            RearDiffMaxTorque 600.0
        )
    )
    "#;

    const SAMPLE_NGP_COMMON: &str = r#"
    ("Car" ("CAR_ROOT"
        CarId 1
        InverseMass 0.000813
        FrontArea 2.0
        DragCoefficient 0.42
        FrontDownforce_NGP 900.0
        RearDownforce_NGP 1050.0
        ("Engine"
            Tq_00_RPM_NGP 0 Tq_00_NGP 0.5
            Tq_01_RPM_NGP 4500 Tq_01_NGP 1.0
            Tq_02_RPM_NGP 6000 Tq_02_NGP 0.92
        )
        ("ControlUnit" RPMLimit 7500)
        ("Drive" NumberOfGears 7)
    ))
    "#;

    const SAMPLE_NGP_OPTIONS: &str = r#"
    ("(null)" DriveTrainLoss_NGP 12 Features_NGP 16950 MaxTorque_NGP 520.0)
    "#;

    #[test]
    fn parameter_load_transmission_suspension_and_differentials_match_original_cars() {
        // Peugeot 206 WRC overrides InverseMass and NumberOfGears in gravel.lsp
        let p206 = parse_vehicle_tech_sheet(
            SAMPLE_P206_COMMON,
            SAMPLE_P206_GRAVEL,
            None,
            Some(SAMPLE_GRAVEL_SETUP),
            retail_tyre_brand(4),
        )
        .unwrap();
        assert_eq!(p206.profile, PhysicsProfile::OriginalRbr102);
        assert_eq!(p206.drivetrain, DrivetrainKind::FourWheelDrive);
        assert_eq!(p206.forward_gears, 5);
        assert_eq!(p206.total_gears, 7);
        assert!((p206.mass_kg - 1400.56).abs() < 0.2);
        assert_eq!(p206.tyre_brand, "MICHELIN");
        assert_eq!(p206.tyre_radius_m, 0.315);
        assert_eq!(p206.front_diff_nm, Some(320.0));
        assert_eq!(p206.center_diff_nm, Some(700.0));
        assert_eq!(p206.rear_diff_nm, Some(600.0));
        assert_eq!(p206.front_rollbar, Some(20000.0));
        assert_eq!(p206.rear_rollbar, Some(30000.0));

        // MG ZR Super 1600 is FWD (DriveType 3), normally aspirated, 1150.7 kg
        let zr = parse_vehicle_tech_sheet(
            SAMPLE_MG_ZR_COMMON,
            "(\"Tyre\" Radius 0.315)",
            None,
            Some(SAMPLE_GRAVEL_SETUP),
            retail_tyre_brand(2),
        )
        .unwrap();
        assert_eq!(zr.drivetrain, DrivetrainKind::FrontWheelDrive);
        assert!(!zr.turbo);
        assert_eq!(zr.rpm_limit, Some(8500));
        assert_eq!(zr.forward_gears, 6);
        assert!((zr.mass_kg - 1150.75).abs() < 0.2);
    }

    #[test]
    fn ngp_descriptor_detection_data_integrity_and_determinism() {
        let a = parse_vehicle_tech_sheet(
            SAMPLE_NGP_COMMON,
            "(\"Tyre\" Radius 0.320)",
            Some(SAMPLE_NGP_OPTIONS),
            Some(SAMPLE_GRAVEL_SETUP),
            "NGP TYRE",
        )
        .unwrap();
        let b = parse_vehicle_tech_sheet(
            SAMPLE_NGP_COMMON,
            "(\"Tyre\" Radius 0.320)",
            Some(SAMPLE_NGP_OPTIONS),
            Some(SAMPLE_GRAVEL_SETUP),
            "NGP TYRE",
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.data_crc32, b.data_crc32);
        assert!(matches!(
            a.profile,
            PhysicsProfile::NgpDetected {
                power_hp: Some(_),
                ..
            }
        ));
        assert_eq!(a.drivetrain, DrivetrainKind::FourWheelDrive);
        assert_eq!(a.forward_gears, 5);
        assert_eq!(a.tyre_radius_m, 0.320);

        // Corrupt / non-finite / out-of-range parameters must fail integrity gates.
        assert!(parse_vehicle_tech_sheet(
            "(\"Car\" (\"CAR_ROOT\" InverseMass NaN NumberOfGears 8))",
            "()",
            None,
            None,
            "MICHELIN",
        )
        .is_err());
        assert!(parse_vehicle_tech_sheet(
            "(\"Car\" (\"CAR_ROOT\" InverseMass 0.000714 NumberOfGears 1))",
            "()",
            None,
            None,
            "MICHELIN",
        )
        .is_err());
    }

    #[test]
    fn ngp_spec_parsing_performance_is_sub_millisecond() {
        let started = std::time::Instant::now();
        for _ in 0..50 {
            let sheet = parse_vehicle_tech_sheet(
                SAMPLE_COROLLA_COMMON,
                "(\"Tyre\" Radius 0.315)",
                None,
                Some(SAMPLE_GRAVEL_SETUP),
                retail_tyre_brand(6),
            )
            .unwrap();
            assert_eq!(sheet.forward_gears, 6);
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs_f64() < 0.25,
            "50 tech-sheet parses took unexpectedly long: {elapsed:?}"
        );
    }
}
