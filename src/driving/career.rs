//! RBR Mobile Career Mode: pilot profile, USD economy, provisional market catalog,
//! used car dealership, personal career garage, and first playable Regional Championship.
//!
//! Persists independently in `Documents/RBR/career_profile.json` using atomic writes.
//! Never touches `personal_bests.json`, `championship_save.txt`, or `school_profile.txt`.

use super::content_manager::{DriveCapability, VehicleEntry};
use rerbr_content::StageDescriptor;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CAREER_SCHEMA_VERSION: u32 = 1;
pub const INITIAL_CASH_USD: u64 = 15_000;

/// Future progression economic targets (documented for Phase 3+ seasons; not active yet).
pub const WRC_SEASON_WIN_BASE_USD: u64 = 1_000_000;
pub const WRC_RALLY_WIN_MIN_USD: u64 = 75_000;
pub const WRC_RALLY_WIN_MAX_USD: u64 = 165_000;

/// Configurable provisional base prizes for the Regional Initiation Championship (USD).
/// Multiplied by `CareerDifficulty::prize_multiplier_pct()`.
pub const REGIONAL_BASE_PRIZES_USD: [u64; 6] = [4_500, 3_000, 2_000, 1_200, 750, 400];

/// Touch-friendly preset pilot names for quick profile creation (customizable via tag/letters).
pub const PILOT_NAME_PRESETS: [&str; 8] = [
    "PILOTO RBR",
    "ADRIAN",
    "CARLOS",
    "MARCOS",
    "LUCAS",
    "SOFIA",
    "RALLY PRO",
    "BURNS FAN",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerTier {
    /// Entry/Initiation FWD modern/Super 1600 tier eligible for Copa Regional de Iniciación.
    Regional,
    Rally3,
    KitCarGroupA,
    Rally2,
    Wrc,
    Historic,
}

impl CareerTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Regional => "REGIONAL / INICIACIÓN",
            Self::Rally3 => "RALLY 3",
            Self::KitCarGroupA => "KIT CAR / GRUPO A",
            Self::Rally2 => "RALLY 2",
            Self::Wrc => "WRC",
            Self::Historic => "HISTÓRICOS / GRUPO B",
        }
    }

    pub fn eligible_for_regional(self) -> bool {
        self == Self::Regional
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DealershipKind {
    #[default]
    UsedCars,
    NewSalon,
}

impl DealershipKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::UsedCars => "CONCESIONARIO 2ª MANO",
            Self::NewSalon => "SALÓN AUTOMOTOR — 0 KM",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::UsedCars => "2ª MANO",
            Self::NewSalon => "SALÓN 0KM",
        }
    }

    pub fn condition_label(self) -> &'static str {
        match self {
            Self::UsedCars => "USADO",
            Self::NewSalon => "0 KM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealershipChannel {
    /// Offered exclusively in `SALÓN AUTOMOTOR` as a brand-new `0 KM` vehicle.
    NewSalonOnly,
    /// Offered exclusively in `2ª MANO` as a `USADO` vehicle.
    UsedOnly,
    /// Offered in BOTH `SALÓN AUTOMOTOR` (`0 KM`) and `2ª MANO` (`USADO` with discount).
    Both,
    /// Validated for Time Trial (`CONTRARRELOJ`) & General `GARAJE`, but excluded from both
    /// Career dealerships until its commercial year rule (e.g. WRC 2013/2016/2017) is defined.
    PendingCommercialRule,
}

impl DealershipChannel {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewSalonOnly => "SALÓN AUTOMOTOR (0 KM)",
            Self::UsedOnly => "2ª MANO (USADO)",
            Self::Both => "SALÓN AUTOMOTOR (0 KM) Y 2ª MANO (USADO)",
            Self::PendingCommercialRule => "PENDIENTE REGLA COMERCIAL",
        }
    }

    pub fn includes(self, kind: DealershipKind) -> bool {
        matches!(
            (self, kind),
            (Self::NewSalonOnly | Self::Both, DealershipKind::NewSalon)
                | (Self::UsedOnly | Self::Both, DealershipKind::UsedCars)
        )
    }
}

/// Configurable table of WRC vehicles from years not yet assigned to a commercial rule (2013, 2016, 2017, 2021).
/// Keeping them here lets future updates assign them to `NewSalonOnly`, `UsedOnly`, or `Both` without
/// arbitrary placement today. They remain 100% drivable in `CONTRARRELOJ` and viewable in `GARAJE`.
pub const PENDING_COMMERCIAL_RULE_WRC_IDS: &[&str] = &[
    "rsf-mini-jcw-wrc",
    "rsf-vw-polo-r-wrc-2016",
    "rsf-citroen-c3-wrc-2017",
    "rsf-hyundai-i20-coupe-wrc-2017",
    "rsf-hyundai-i20-coupe-wrc-2021",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleMarketSpec {
    pub precio_0km: Option<u64>,
    pub precio_usado: Option<u64>,
    pub descuento_usado_pct: u8,
    pub concesionario: DealershipChannel,
    pub categoria: String,
    pub anio: Option<u32>,
    pub elegibilidad_carrera: CareerTier,
    /// Legacy convenience price (`precio_usado.or(precio_0km).unwrap_or(0)`).
    pub price_usd: u64,
    /// Legacy alias for `elegibilidad_carrera`.
    pub tier: CareerTier,
    pub is_provisional: bool,
}

impl VehicleMarketSpec {
    fn build(
        precio_0km: Option<u64>,
        precio_usado: Option<u64>,
        concesionario: DealershipChannel,
        categoria: impl Into<String>,
        anio: Option<u32>,
        elegibilidad_carrera: CareerTier,
    ) -> Self {
        let descuento_usado_pct = match (precio_0km, precio_usado) {
            (Some(new_p), Some(used_p)) if new_p > 0 && used_p < new_p => {
                (((new_p - used_p) * 100) / new_p) as u8
            }
            _ => 0,
        };
        let price_usd = precio_usado.or(precio_0km).unwrap_or(0);
        Self {
            precio_0km,
            precio_usado,
            descuento_usado_pct,
            concesionario,
            categoria: categoria.into(),
            anio,
            elegibilidad_carrera,
            price_usd,
            tier: elegibilidad_carrera,
            is_provisional: true,
        }
    }

    pub fn offered_in(&self, kind: DealershipKind) -> bool {
        self.concesionario.includes(kind) && self.price_for(kind).is_some_and(|p| p > 0)
    }

    pub fn price_for(&self, kind: DealershipKind) -> Option<u64> {
        if !self.concesionario.includes(kind) {
            return None;
        }
        match kind {
            DealershipKind::NewSalon => self.precio_0km,
            DealershipKind::UsedCars => self.precio_usado,
        }
    }

    pub fn condition_badge(&self, kind: DealershipKind) -> String {
        match kind {
            DealershipKind::NewSalon => "0 KM".into(),
            DealershipKind::UsedCars => {
                if self.descuento_usado_pct > 0 && self.precio_0km.is_some() {
                    format!("USADO (-{}%)", self.descuento_usado_pct)
                } else {
                    "USADO".into()
                }
            }
        }
    }
}

fn infer_vehicle_year(entry: &VehicleEntry) -> Option<u32> {
    if let Some(y) = entry
        .year
        .or_else(|| super::content_manager::preset_year_for_vehicle_id(&entry.descriptor.id))
    {
        return Some(y);
    }
    match entry.descriptor.id.as_str() {
        "rsf-renault-clio-rally5"
        | "rsf-renault-clio-rally4"
        | "rsf-ford-fiesta-rally4"
        | "rsf-peugeot-208-rally4"
        | "rsf-ford-fiesta-rally3" => Some(2021),
        "rsf-renault-clio-rally3"
        | "rsf-ford-fiesta-rally3-evo"
        | "rsf-ford-fiesta-rally2"
        | "rsf-hyundai-i20-n-rally2" => Some(2023),
        "rsf-toyota-gr-yaris-rally2" | "rsf-skoda-fabia-rs-rally2" => Some(2024),
        "rsf-ford-fiesta-mk-viii-r2" | "rsf-ford-fiesta-wrc-2019" => Some(2019),
        "rsf-opel-adam-r2" | "rsf-mini-jcw-wrc" => Some(2013),
        "rsf-peugeot-208-r2" => Some(2012),
        "rsf-citroen-c2-r2-max" => Some(2011),
        "rsf-citroen-ds3-wrc" | "rsf-ford-fiesta-rs-wrc-2014" => Some(2014),
        "rsf-vw-polo-r-wrc-2016" => Some(2016),
        "rsf-citroen-c3-wrc-2017" | "rsf-hyundai-i20-coupe-wrc-2017" => Some(2017),
        "rsf-toyota-yaris-wrc-2018" => Some(2018),
        "rsf-hyundai-i20-coupe-wrc-2020" => Some(2020),
        "rsf-hyundai-i20-coupe-wrc-2021" => Some(2021),
        "ford-focus-rs-wrc-2006"
        | "rsf-citroen-xsara-wrc-2006"
        | "rsf-subaru-impreza-gda-wrc2003-s9"
        | "rsf-skoda-fabia-wrc-2006" => Some(2006),
        "rsf-citroen-c4-wrc-2008" => Some(2008),
        "rsf-peugeot-306-maxi-kit-car" => Some(1998),
        "rsf-audi-200-quattro-grpa" => Some(1988),
        "rsf-bmw-m3-e30-grpa" => Some(1990),
        "rsf-audi-quattro-a1-grpb" => Some(1983),
        "rsf-audi-quattro-a2-grpb" | "rsf-bmw-m1-grpb" => Some(1984),
        "rsf-audi-sport-quattro-grpb"
        | "rsf-peugeot-205-t16-grpb"
        | "rsf-porsche-911-sc-rs-grpb"
        | "rsf-toyota-celica-tct-ta64-grpb" => Some(1985),
        _ => None,
    }
}

fn infer_real_category(entry: &VehicleEntry) -> String {
    if let Some(cat) = entry
        .descriptor
        .category
        .as_deref()
        .filter(|c| !c.is_empty() && *c != "RBR Original")
    {
        return cat.to_string();
    }
    match entry.descriptor.id.as_str() {
        "mg-zr-s1600" => "Super 1600".into(),
        id if entry.descriptor.is_original_rbr() || id == "ford-focus-rs-wrc-2006" => {
            "WRC 2.0".into()
        }
        _ => entry
            .descriptor
            .category
            .clone()
            .unwrap_or_else(|| "Rally".into()),
    }
}

/// Configurable provisional market specification for any installed vehicle,
/// enforcing strict commercial placement across `SALÓN AUTOMOTOR (0 KM)` and `2ª MANO (USADO)`.
/// Changing prices in future versions never invalidates already-owned cars.
pub fn vehicle_market_spec(entry: &VehicleEntry) -> VehicleMarketSpec {
    let id = entry.descriptor.id.as_str();
    let real_cat = infer_real_category(entry);
    let cat_lower = real_cat.to_ascii_lowercase();
    let name_lower = entry.descriptor.name.to_ascii_lowercase();
    let year = infer_vehicle_year(entry);

    // 1. Explicit Pending Commercial Rule table (WRC 2013, 2016, 2017, 2021)
    if PENDING_COMMERCIAL_RULE_WRC_IDS.contains(&id) {
        return VehicleMarketSpec::build(
            None,
            None,
            DealershipChannel::PendingCommercialRule,
            real_cat,
            year,
            CareerTier::Wrc,
        );
    }

    // 2. Explicit vehicle-level specifications
    match id {
        // 2.1 Rally5: Exclusively in SALÓN AUTOMOTOR (0 KM), affordable with initial US$15,000
        "rsf-renault-clio-rally5" => {
            return VehicleMarketSpec::build(
                Some(13_800),
                None,
                DealershipChannel::NewSalonOnly,
                "Rally 5",
                year.or(Some(2021)),
                CareerTier::Regional,
            );
        }
        // 2.2 Rally4: Offered in BOTH SALÓN AUTOMOTOR (0 KM) and 2ª MANO (USADO with discount < 0 KM)
        "rsf-renault-clio-rally4" | "rsf-ford-fiesta-rally4" | "rsf-peugeot-208-rally4" => {
            return VehicleMarketSpec::build(
                Some(24_000),
                Some(18_500),
                DealershipChannel::Both,
                "Rally 4",
                year.or(Some(2021)),
                CareerTier::Regional,
            );
        }
        // 2.3 R2 and Super 1600: Exclusively in 2ª MANO (USADO), keeping their real category
        "mg-zr-s1600" => {
            return VehicleMarketSpec::build(
                None,
                Some(12_500),
                DealershipChannel::UsedOnly,
                "Super 1600",
                year.or(Some(2004)),
                CareerTier::Regional,
            );
        }
        "rsf-citroen-c2-r2-max" => {
            return VehicleMarketSpec::build(
                None,
                Some(13_200),
                DealershipChannel::UsedOnly,
                "Group R2",
                year.or(Some(2011)),
                CareerTier::Regional,
            );
        }
        "rsf-peugeot-208-r2" => {
            return VehicleMarketSpec::build(
                None,
                Some(13_900),
                DealershipChannel::UsedOnly,
                "Group R2",
                year.or(Some(2012)),
                CareerTier::Regional,
            );
        }
        "rsf-opel-adam-r2" => {
            return VehicleMarketSpec::build(
                None,
                Some(14_400),
                DealershipChannel::UsedOnly,
                "Group R2",
                year.or(Some(2013)),
                CareerTier::Regional,
            );
        }
        "rsf-ford-fiesta-mk-viii-r2" => {
            return VehicleMarketSpec::build(
                None,
                Some(14_800),
                DealershipChannel::UsedOnly,
                "Group R2",
                year.or(Some(2019)),
                CareerTier::Regional,
            );
        }
        // 2.4 Rally3:
        // - Ford Fiesta Rally3 Evo: Exclusively in SALÓN AUTOMOTOR (0 KM)
        // - Ford Fiesta Rally3 standard & Renault Clio Rally3: Exclusively in 2ª MANO (USADO)
        "rsf-ford-fiesta-rally3-evo" => {
            return VehicleMarketSpec::build(
                Some(46_000),
                None,
                DealershipChannel::NewSalonOnly,
                "Rally 3",
                year.or(Some(2023)),
                CareerTier::Rally3,
            );
        }
        "rsf-ford-fiesta-rally3" | "rsf-renault-clio-rally3" => {
            return VehicleMarketSpec::build(
                None,
                Some(36_500),
                DealershipChannel::UsedOnly,
                "Rally 3",
                year,
                CareerTier::Rally3,
            );
        }
        // 2.5 Rally2 & R5:
        // - Modern Rally2 (GR Yaris Rally2, Fabia RS Rally2, Fiesta Rally2, i20 N Rally2): BOTH (0 KM and USADO)
        "rsf-toyota-gr-yaris-rally2"
        | "rsf-skoda-fabia-rs-rally2"
        | "rsf-ford-fiesta-rally2"
        | "rsf-hyundai-i20-n-rally2" => {
            return VehicleMarketSpec::build(
                Some(115_000),
                Some(89_000),
                DealershipChannel::Both,
                "Rally 2",
                year,
                CareerTier::Rally2,
            );
        }
        // 2.6 WRC 2018–2020: Exclusively in SALÓN AUTOMOTOR (0 KM), NEVER in 2ª MANO
        "rsf-toyota-yaris-wrc-2018"
        | "rsf-ford-fiesta-wrc-2019"
        | "rsf-hyundai-i20-coupe-wrc-2020" => {
            return VehicleMarketSpec::build(
                Some(245_000),
                None,
                DealershipChannel::NewSalonOnly,
                "WRC 1.6",
                year,
                CareerTier::Wrc,
            );
        }
        // 2.7 WRC 2014–2015: Exclusively in 2ª MANO (USADO), NEVER as 0 KM
        "rsf-citroen-ds3-wrc" | "rsf-ford-fiesta-rs-wrc-2014" => {
            return VehicleMarketSpec::build(
                None,
                Some(155_000),
                DealershipChannel::UsedOnly,
                "WRC 1.6",
                year.or(Some(2014)),
                CareerTier::Wrc,
            );
        }
        // 2.8 WRC 2.0 2000–2009: Exclusively in 2ª MANO (USADO), NEVER as 0 KM
        "ford-focus-rs-wrc-2006"
        | "rsf-citroen-xsara-wrc-2006"
        | "rsf-citroen-c4-wrc-2008"
        | "rsf-subaru-impreza-gda-wrc2003-s9"
        | "rsf-skoda-fabia-wrc-2006" => {
            return VehicleMarketSpec::build(
                None,
                Some(138_000),
                DealershipChannel::UsedOnly,
                "WRC 2.0",
                year,
                CareerTier::Wrc,
            );
        }
        // 2.9 Historic Group A / Kit Car: Exclusively in 2ª MANO (USADO)
        "rsf-peugeot-306-maxi-kit-car" => {
            return VehicleMarketSpec::build(
                None,
                Some(45_000),
                DealershipChannel::UsedOnly,
                "Group A7",
                year.or(Some(1998)),
                CareerTier::KitCarGroupA,
            );
        }
        "rsf-audi-200-quattro-grpa" | "rsf-bmw-m3-e30-grpa" => {
            return VehicleMarketSpec::build(
                None,
                Some(58_000),
                DealershipChannel::UsedOnly,
                "Group A8",
                year,
                CareerTier::KitCarGroupA,
            );
        }
        _ => {}
    }

    // 3. Category & year-driven fallback rules for any other installed RSF/retail models
    if cat_lower.contains("rally 5") || name_lower.contains("rally5") {
        VehicleMarketSpec::build(
            Some(13_800),
            None,
            DealershipChannel::NewSalonOnly,
            real_cat,
            year,
            CareerTier::Regional,
        )
    } else if cat_lower.contains("rally 4") || name_lower.contains("rally4") {
        VehicleMarketSpec::build(
            Some(24_000),
            Some(18_500),
            DealershipChannel::Both,
            real_cat,
            year,
            CareerTier::Regional,
        )
    } else if cat_lower.contains("group r2")
        || name_lower.contains(" r2")
        || cat_lower.contains("super 1600")
        || name_lower.contains("s1600")
        || name_lower.contains("super 1600")
    {
        VehicleMarketSpec::build(
            None,
            Some(14_200),
            DealershipChannel::UsedOnly,
            real_cat,
            year,
            CareerTier::Regional,
        )
    } else if cat_lower.contains("rally 3") || name_lower.contains("rally3") {
        if name_lower.contains("evo") {
            VehicleMarketSpec::build(
                Some(46_000),
                None,
                DealershipChannel::NewSalonOnly,
                real_cat,
                year,
                CareerTier::Rally3,
            )
        } else {
            VehicleMarketSpec::build(
                None,
                Some(36_500),
                DealershipChannel::UsedOnly,
                real_cat,
                year,
                CareerTier::Rally3,
            )
        }
    } else if cat_lower.contains("rally 2") || name_lower.contains("rally2") {
        VehicleMarketSpec::build(
            Some(115_000),
            Some(89_000),
            DealershipChannel::Both,
            real_cat,
            year,
            CareerTier::Rally2,
        )
    } else if cat_lower.contains("r5") || name_lower.contains(" r5") {
        VehicleMarketSpec::build(
            None,
            Some(74_000),
            DealershipChannel::UsedOnly,
            real_cat,
            year,
            CareerTier::Rally2,
        )
    } else if cat_lower.contains("group b") || name_lower.contains("grpb") {
        VehicleMarketSpec::build(
            None,
            Some(160_000),
            DealershipChannel::UsedOnly,
            "Group B",
            year,
            CareerTier::Historic,
        )
    } else if cat_lower.contains("group a") || cat_lower.contains("kit car") {
        VehicleMarketSpec::build(
            None,
            Some(52_000),
            DealershipChannel::UsedOnly,
            real_cat,
            year,
            CareerTier::KitCarGroupA,
        )
    } else if cat_lower.contains("wrc")
        || entry.descriptor.is_original_rbr()
        || name_lower.contains("wrc")
    {
        match year {
            Some(2018..=2020) => VehicleMarketSpec::build(
                Some(245_000),
                None,
                DealershipChannel::NewSalonOnly,
                real_cat,
                year,
                CareerTier::Wrc,
            ),
            Some(2014..=2015) => VehicleMarketSpec::build(
                None,
                Some(155_000),
                DealershipChannel::UsedOnly,
                real_cat,
                year,
                CareerTier::Wrc,
            ),
            Some(1997..=2009) => VehicleMarketSpec::build(
                None,
                Some(138_000),
                DealershipChannel::UsedOnly,
                "WRC 2.0",
                year,
                CareerTier::Wrc,
            ),
            _ if entry.descriptor.is_original_rbr() || cat_lower.contains("wrc 2.0") => {
                VehicleMarketSpec::build(
                    None,
                    Some(138_000),
                    DealershipChannel::UsedOnly,
                    "WRC 2.0",
                    year,
                    CareerTier::Wrc,
                )
            }
            _ => VehicleMarketSpec::build(
                None,
                None,
                DealershipChannel::PendingCommercialRule,
                real_cat,
                year,
                CareerTier::Wrc,
            ),
        }
    } else {
        VehicleMarketSpec::build(
            None,
            Some(65_000),
            DealershipChannel::UsedOnly,
            real_cat,
            year,
            CareerTier::KitCarGroupA,
        )
    }
}

pub fn format_usd(amount: u64) -> String {
    let s = amount.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 4);
    out.push_str("US$");
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(char::from(b));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CareerDifficulty {
    #[default]
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

impl CareerDifficulty {
    pub const ALL: [Self; 4] = [
        Self::Beginner,
        Self::Intermediate,
        Self::Advanced,
        Self::Expert,
    ];

    pub fn next(self) -> Self {
        match self {
            Self::Beginner => Self::Intermediate,
            Self::Intermediate => Self::Advanced,
            Self::Advanced => Self::Expert,
            Self::Expert => Self::Beginner,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Beginner => Self::Expert,
            Self::Intermediate => Self::Beginner,
            Self::Advanced => Self::Intermediate,
            Self::Expert => Self::Advanced,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
            Self::Expert => "expert",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "intermediate" => Self::Intermediate,
            "advanced" => Self::Advanced,
            "expert" => Self::Expert,
            _ => Self::Beginner,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Beginner => "PRINCIPIANTE",
            Self::Intermediate => "INTERMEDIO",
            Self::Advanced => "AVANZADO",
            Self::Expert => "EXPERTO",
        }
    }

    /// Prize multiplier in percent (`100` = 1.00x, `190` = 1.90x).
    pub fn prize_multiplier_pct(self) -> u64 {
        match self {
            Self::Beginner => 100,
            Self::Intermediate => 125,
            Self::Advanced => 155,
            Self::Expert => 190,
        }
    }

    /// Rival pace factor relative to stage reference time (higher = slower rivals).
    pub fn rival_pace_factor(self) -> f64 {
        match self {
            Self::Beginner => 1.18,
            Self::Intermediate => 1.08,
            Self::Advanced => 1.00,
            Self::Expert => 0.94,
        }
    }

    pub fn regional_prize_for_position(self, position_1_based: usize) -> u64 {
        let base = REGIONAL_BASE_PRIZES_USD
            .get(position_1_based.saturating_sub(1))
            .copied()
            .unwrap_or(*REGIONAL_BASE_PRIZES_USD.last().unwrap());
        (base * self.prize_multiplier_pct()) / 100
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegionalStageSpec {
    pub stage_id: &'static str,
    pub display_name: &'static str,
    pub surface: &'static str,
    pub length_km: f32,
    /// Calibrated reference time in ms for an Initiation/S1600 car on Advanced difficulty.
    pub reference_ms: u64,
}

/// First playable Regional Championship: 3 short verified stages whose maps AND country
/// spectator `.sgs` files (`us_`, `mo_`, `br_`) are all already present on the iPad.
pub const REGIONAL_STAGES: [RegionalStageSpec; 3] = [
    RegionalStageSpec {
        stage_id: "diamond-creek",
        display_name: "Diamond Creek",
        surface: "Gravel",
        length_km: 4.98,
        reference_ms: 210_000,
    },
    RegionalStageSpec {
        stage_id: "cote-darbroz",
        display_name: "Cote d'Arbroz",
        surface: "Tarmac",
        length_km: 4.65,
        reference_ms: 186_000,
    },
    RegionalStageSpec {
        stage_id: "shepherds-shield",
        display_name: "Shepherds Shield",
        surface: "Gravel",
        length_km: 4.91,
        reference_ms: 212_000,
    },
];

pub fn regional_stage_descriptor(stage_index: usize) -> Option<StageDescriptor> {
    let spec = REGIONAL_STAGES.get(stage_index)?;
    StageDescriptor::presets()
        .into_iter()
        .find(|s| s.id == spec.stage_id)
}

#[derive(Debug, Clone, Copy)]
struct RivalBlueprint {
    name: &'static str,
    car_label: &'static str,
    /// Base pace offset relative to `reference_ms * difficulty.rival_pace_factor()`.
    skill_ratio: f64,
}

const REGIONAL_RIVALS: [RivalBlueprint; 6] = [
    RivalBlueprint {
        name: "M. KALLIO",
        car_label: "MG ZR S1600",
        skill_ratio: 0.985,
    },
    RivalBlueprint {
        name: "L. MOREAU",
        car_label: "Clio Rally5",
        skill_ratio: 1.005,
    },
    RivalBlueprint {
        name: "T. EVANS",
        car_label: "Fiesta R2",
        skill_ratio: 1.022,
    },
    RivalBlueprint {
        name: "R. SALAZAR",
        car_label: "208 Rally4",
        skill_ratio: 1.040,
    },
    RivalBlueprint {
        name: "K. SATO",
        car_label: "MG ZR S1600",
        skill_ratio: 1.062,
    },
    RivalBlueprint {
        name: "D. NOVAK",
        car_label: "Clio Rally5",
        skill_ratio: 1.088,
    },
];

fn deterministic_jitter(seed: u64, rival_idx: usize, stage_idx: usize) -> f64 {
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add((rival_idx as u64 + 1).wrapping_mul(1442695040888963407))
        .wrapping_add((stage_idx as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    // Map to [-0.014, +0.014] (~ +/-2.5s natural stage-to-stage variation)
    let unit = (x & 0xFFFF) as f64 / 65535.0;
    (unit - 0.5) * 0.028
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RivalEntry {
    pub name: String,
    pub car_label: String,
    pub stage_times_ms: Vec<u64>,
}

impl RivalEntry {
    pub fn total_after_stages(&self, completed_stages: usize) -> u64 {
        self.stage_times_ms
            .iter()
            .take(completed_stages)
            .copied()
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandingRow {
    pub position: usize,
    pub name: String,
    pub car_label: String,
    pub total_ms: u64,
    pub gap_to_leader_ms: u64,
    pub is_player: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    InitialGrant,
    VehiclePurchase,
    ChampionshipPrize,
}

impl TransactionKind {
    fn code(&self) -> &'static str {
        match self {
            Self::InitialGrant => "initial_grant",
            Self::VehiclePurchase => "vehicle_purchase",
            Self::ChampionshipPrize => "championship_prize",
        }
    }

    fn from_code(s: &str) -> Self {
        match s {
            "vehicle_purchase" => Self::VehiclePurchase,
            "championship_prize" => Self::ChampionshipPrize,
            _ => Self::InitialGrant,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub kind: TransactionKind,
    /// Positive for credit, negative for debit.
    pub delta_usd: i64,
    pub balance_after_usd: u64,
    pub description: String,
    pub timestamp_unix_s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedChampionshipRecord {
    pub championship_id: String,
    pub title: String,
    pub difficulty: CareerDifficulty,
    pub vehicle_id: String,
    pub vehicle_name: String,
    pub final_position: usize,
    pub total_ms: u64,
    pub prize_usd: u64,
    pub abandoned: bool,
    pub completed_unix_s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveChampionship {
    pub id: String,
    pub title: String,
    pub seed: u64,
    pub difficulty: CareerDifficulty,
    pub locked_vehicle_id: String,
    pub locked_vehicle_name: String,
    pub stage_ids: Vec<String>,
    pub player_stage_times_ms: Vec<u64>,
    pub rivals: Vec<RivalEntry>,
    pub finished: bool,
    pub reward_claimed: bool,
    pub final_position: Option<usize>,
    pub prize_awarded_usd: u64,
}

impl ActiveChampionship {
    pub fn completed_stages(&self) -> usize {
        self.player_stage_times_ms.len()
    }

    pub fn total_stages(&self) -> usize {
        self.stage_ids.len()
    }

    pub fn next_stage_index(&self) -> Option<usize> {
        if self.finished || self.player_stage_times_ms.len() >= self.stage_ids.len() {
            None
        } else {
            Some(self.player_stage_times_ms.len())
        }
    }

    pub fn next_stage_id(&self) -> Option<&str> {
        let idx = self.next_stage_index()?;
        self.stage_ids.get(idx).map(String::as_str)
    }

    pub fn player_total_ms(&self) -> u64 {
        self.player_stage_times_ms.iter().copied().sum()
    }

    pub fn standings(&self, pilot_name: &str) -> Vec<StandingRow> {
        let done = self.completed_stages();
        let mut rows = Vec::with_capacity(self.rivals.len() + 1);
        rows.push(StandingRow {
            position: 0,
            name: pilot_name.to_string(),
            car_label: self.locked_vehicle_name.clone(),
            total_ms: self.player_total_ms(),
            gap_to_leader_ms: 0,
            is_player: true,
        });
        for rival in &self.rivals {
            rows.push(StandingRow {
                position: 0,
                name: rival.name.clone(),
                car_label: rival.car_label.clone(),
                total_ms: rival.total_after_stages(done),
                gap_to_leader_ms: 0,
                is_player: false,
            });
        }
        rows.sort_by(|a, b| {
            a.total_ms
                .cmp(&b.total_ms)
                .then_with(|| b.is_player.cmp(&a.is_player))
                .then_with(|| a.name.cmp(&b.name))
        });
        let leader_ms = rows.first().map_or(0, |r| r.total_ms);
        for (i, row) in rows.iter_mut().enumerate() {
            row.position = i + 1;
            row.gap_to_leader_ms = row.total_ms.saturating_sub(leader_ms);
        }
        rows
    }

    pub fn player_position(&self, pilot_name: &str) -> usize {
        self.standings(pilot_name)
            .iter()
            .find(|r| r.is_player)
            .map_or(self.rivals.len() + 1, |r| r.position)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageRecordOutcome {
    pub stage_index: usize,
    pub stage_time_ms: u64,
    pub championship_finished: bool,
    pub current_position: usize,
    pub prize_awarded_usd: u64,
    pub newly_unlocked_historics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CareerProfile {
    pub schema_version: u32,
    pub pilot_name: String,
    pub cash_usd: u64,
    pub owned_vehicle_ids: Vec<String>,
    pub active_career_vehicle_id: Option<String>,
    pub podiums: u32,
    pub wins: u32,
    pub historics_unlocked: bool,
    pub next_championship_seq: u64,
    pub transactions: Vec<Transaction>,
    pub completed_championships: Vec<CompletedChampionshipRecord>,
    pub active_championship: Option<ActiveChampionship>,
}

impl CareerProfile {
    pub fn create(pilot_name: &str) -> Result<Self, String> {
        let cleaned = sanitize_pilot_name(pilot_name);
        if cleaned.is_empty() {
            return Err("EL NOMBRE DEL PILOTO NO PUEDE ESTAR VACÍO".into());
        }
        let now = unix_now_s();
        Ok(Self {
            schema_version: CAREER_SCHEMA_VERSION,
            pilot_name: cleaned,
            cash_usd: INITIAL_CASH_USD,
            owned_vehicle_ids: Vec::new(),
            active_career_vehicle_id: None,
            podiums: 0,
            wins: 0,
            historics_unlocked: false,
            next_championship_seq: 1,
            transactions: vec![Transaction {
                kind: TransactionKind::InitialGrant,
                delta_usd: INITIAL_CASH_USD as i64,
                balance_after_usd: INITIAL_CASH_USD,
                description: "Presupuesto inicial de carrera".into(),
                timestamp_unix_s: now,
            }],
            completed_championships: Vec::new(),
            active_championship: None,
        })
    }

    pub fn owns_vehicle(&self, vehicle_id: &str) -> bool {
        self.owned_vehicle_ids.iter().any(|id| id == vehicle_id)
    }

    pub fn can_afford(&self, price_usd: u64) -> bool {
        price_usd > 0 && self.cash_usd >= price_usd
    }

    pub fn has_locked_championship(&self) -> bool {
        self.active_championship
            .as_ref()
            .is_some_and(|c| !c.finished)
    }

    /// Strict dealership purchase validation for a specific dealership (`2ª MANO` or `SALÓN AUTOMOTOR 0 KM`):
    /// - Vehicle must be genuinely `Drivable` (never visual-only or unavailable).
    /// - Vehicle must be commercially offered in `kind` (`spec.offered_in(kind)`).
    /// - Uses the exact dealership price (`spec.price_for(kind)`).
    /// - Prevents duplicate ownership across both dealerships and overdrafts.
    pub fn buy_vehicle_in_dealership(
        &mut self,
        entry: &VehicleEntry,
        kind: DealershipKind,
        ngp_experimental: bool,
    ) -> Result<u64, String> {
        let cap = entry.capability(ngp_experimental);
        if cap != DriveCapability::Drivable {
            return Err(match cap {
                DriveCapability::VisualOnly => {
                    "SOLO VISUALIZACIÓN 3D — NO SE PUEDE COMPRAR SIN FÍSICA".into()
                }
                DriveCapability::Unavailable => "ARCHIVOS FALTANTES — VEHÍCULO NO COMPRABLE".into(),
                DriveCapability::Drivable => unreachable!(),
            });
        }
        let spec = vehicle_market_spec(entry);
        let Some(price) = spec.price_for(kind).filter(|&p| p > 0) else {
            return Err(format!(
                "ESTE VEHÍCULO NO SE VENDE EN {}",
                kind.short_name()
            ));
        };
        if self.owns_vehicle(&entry.descriptor.id) {
            return Err("YA TIENES ESTE VEHÍCULO EN TU GARAJE DE CARRERA".into());
        }
        if self.cash_usd < price {
            return Err(format!(
                "SALDO INSUFICIENTE (NECESITAS {}, TIENES {})",
                format_usd(price),
                format_usd(self.cash_usd)
            ));
        }

        self.cash_usd -= price;
        self.owned_vehicle_ids.push(entry.descriptor.id.clone());
        if self.active_career_vehicle_id.is_none() {
            self.active_career_vehicle_id = Some(entry.descriptor.id.clone());
        }
        self.transactions.push(Transaction {
            kind: TransactionKind::VehiclePurchase,
            delta_usd: -(price as i64),
            balance_after_usd: self.cash_usd,
            description: format!(
                "Compra {} {}",
                kind.condition_label(),
                entry.descriptor.name
            ),
            timestamp_unix_s: unix_now_s(),
        });
        Ok(price)
    }

    /// Convenience purchase helper that buys from `2ª MANO` when offered there, or `SALÓN AUTOMOTOR` otherwise.
    pub fn buy_vehicle(
        &mut self,
        entry: &VehicleEntry,
        ngp_experimental: bool,
    ) -> Result<u64, String> {
        let spec = vehicle_market_spec(entry);
        let kind = if spec.offered_in(DealershipKind::UsedCars) {
            DealershipKind::UsedCars
        } else {
            DealershipKind::NewSalon
        };
        self.buy_vehicle_in_dealership(entry, kind, ngp_experimental)
    }

    pub fn select_career_vehicle(&mut self, vehicle_id: &str) -> Result<(), String> {
        if self.has_locked_championship() {
            return Err("COCHE BLOQUEADO DURANTE EL CAMPEONATO ACTIVO".into());
        }
        if !self.owns_vehicle(vehicle_id) {
            return Err("ESTE VEHÍCULO NO PERTENECE A TU GARAJE DE CARRERA".into());
        }
        self.active_career_vehicle_id = Some(vehicle_id.to_string());
        Ok(())
    }

    /// Starts a new 3-stage Regional Initiation Championship, locking car and difficulty.
    pub fn start_regional_championship(
        &mut self,
        vehicle_entry: &VehicleEntry,
        ngp_experimental: bool,
        difficulty: CareerDifficulty,
    ) -> Result<(), String> {
        if self.has_locked_championship() {
            return Err("YA TIENES UN CAMPEONATO REGIONAL EN CURSO".into());
        }
        if !self.owns_vehicle(&vehicle_entry.descriptor.id) {
            return Err("DEBES COMPRAR UN COCHE DE INICIACIÓN EN 2ª MANO PRIMERO".into());
        }
        if vehicle_entry.capability(ngp_experimental) != DriveCapability::Drivable {
            return Err("EL COCHE SELECCIONADO NO ES CONDUCIBLE CON FÍSICA".into());
        }
        let spec = vehicle_market_spec(vehicle_entry);
        if !spec.tier.eligible_for_regional() {
            return Err(format!(
                "CATEGORÍA {} NO ELEGIBLE PARA COPA REGIONAL (REQUIERE INICIACIÓN)",
                spec.tier.label()
            ));
        }

        let seq = self.next_championship_seq;
        self.next_championship_seq = self.next_championship_seq.saturating_add(1);
        let seed = seq
            .wrapping_mul(0x517C_C1B7_2722_0A95)
            .wrapping_add(difficulty as u64 + 17);

        let rivals: Vec<RivalEntry> = REGIONAL_RIVALS
            .iter()
            .enumerate()
            .map(|(r_idx, bp)| {
                let stage_times_ms = REGIONAL_STAGES
                    .iter()
                    .enumerate()
                    .map(|(s_idx, st)| {
                        let jitter = deterministic_jitter(seed, r_idx, s_idx);
                        let factor =
                            difficulty.rival_pace_factor() * bp.skill_ratio * (1.0 + jitter);
                        ((st.reference_ms as f64) * factor).round().max(30_000.0) as u64
                    })
                    .collect();
                RivalEntry {
                    name: bp.name.to_string(),
                    car_label: bp.car_label.to_string(),
                    stage_times_ms,
                }
            })
            .collect();

        self.active_career_vehicle_id = Some(vehicle_entry.descriptor.id.clone());
        self.active_championship = Some(ActiveChampionship {
            id: format!("regional-{seq}"),
            title: "COPA REGIONAL DE INICIACIÓN".into(),
            seed,
            difficulty,
            locked_vehicle_id: vehicle_entry.descriptor.id.clone(),
            locked_vehicle_name: vehicle_entry.descriptor.name.clone(),
            stage_ids: REGIONAL_STAGES
                .iter()
                .map(|s| s.stage_id.to_string())
                .collect(),
            player_stage_times_ms: Vec::new(),
            rivals,
            finished: false,
            reward_claimed: false,
            final_position: None,
            prize_awarded_usd: 0,
        });
        Ok(())
    }

    /// Records a validated stage finish strictly in order.
    /// Rejects out-of-order stages, wrong vehicle, invalid runs, or already-finished championships.
    /// Pays the championship prize ONCE when Stage 3 completes.
    pub fn record_championship_stage_result(
        &mut self,
        stage_id: &str,
        vehicle_id: &str,
        total_ms: u64,
        valid_run: bool,
    ) -> Result<StageRecordOutcome, String> {
        if !valid_run || total_ms == 0 {
            return Err("CARRERA NO VÁLIDA — NO SE REGISTRA EN EL CAMPEONATO".into());
        }
        let pilot_name = self.pilot_name.clone();
        let Some(champ) = self.active_championship.as_mut() else {
            return Err("NO HAY CAMPEONATO ACTIVO".into());
        };
        if champ.finished {
            return Err("EL CAMPEONATO YA FINALIZÓ (PREMIO YA PROCESADO)".into());
        }
        if champ.locked_vehicle_id != vehicle_id {
            return Err("EL VEHÍCULO NO COINCIDE CON EL COCHE BLOQUEADO DEL CAMPEONATO".into());
        }
        let Some(stage_index) = champ.next_stage_index() else {
            return Err("NO QUEDAN ETAPAS PENDIENTES EN ESTE CAMPEONATO".into());
        };
        let expected_stage = champ
            .stage_ids
            .get(stage_index)
            .map(String::as_str)
            .unwrap_or("");
        if expected_stage != stage_id {
            return Err(format!(
                "ETAPA FUERA DE ORDEN (ESPERADA {}, RECIBIDA {})",
                expected_stage, stage_id
            ));
        }

        champ.player_stage_times_ms.push(total_ms);
        let current_position = champ.player_position(&pilot_name);
        let all_done = champ.player_stage_times_ms.len() >= champ.stage_ids.len();

        let mut prize_awarded_usd = 0;
        let mut newly_unlocked_historics = false;

        if all_done {
            champ.finished = true;
            champ.final_position = Some(current_position);
            if !champ.reward_claimed {
                champ.reward_claimed = true;
                prize_awarded_usd = champ
                    .difficulty
                    .regional_prize_for_position(current_position);
                champ.prize_awarded_usd = prize_awarded_usd;

                let champ_id = champ.id.clone();
                let champ_title = champ.title.clone();
                let difficulty = champ.difficulty;
                let v_id = champ.locked_vehicle_id.clone();
                let v_name = champ.locked_vehicle_name.clone();
                let total_champ_ms = champ.player_total_ms();

                self.cash_usd = self.cash_usd.saturating_add(prize_awarded_usd);
                if current_position == 1 {
                    self.wins = self.wins.saturating_add(1);
                }
                if current_position <= 3 {
                    self.podiums = self.podiums.saturating_add(1);
                    if !self.historics_unlocked {
                        self.historics_unlocked = true;
                        newly_unlocked_historics = true;
                    }
                }
                let now = unix_now_s();
                self.transactions.push(Transaction {
                    kind: TransactionKind::ChampionshipPrize,
                    delta_usd: prize_awarded_usd as i64,
                    balance_after_usd: self.cash_usd,
                    description: format!(
                        "Premio P{} {} ({})",
                        current_position,
                        champ_title,
                        difficulty.label()
                    ),
                    timestamp_unix_s: now,
                });
                self.completed_championships
                    .push(CompletedChampionshipRecord {
                        championship_id: champ_id,
                        title: champ_title,
                        difficulty,
                        vehicle_id: v_id,
                        vehicle_name: v_name,
                        final_position: current_position,
                        total_ms: total_champ_ms,
                        prize_usd: prize_awarded_usd,
                        abandoned: false,
                        completed_unix_s: now,
                    });
            }
        }

        Ok(StageRecordOutcome {
            stage_index,
            stage_time_ms: total_ms,
            championship_finished: all_done,
            current_position,
            prize_awarded_usd,
            newly_unlocked_historics,
        })
    }

    /// Retires/abandons the active championship with zero prize and unlocks car/difficulty.
    pub fn abandon_championship(&mut self) -> Result<(), String> {
        let Some(champ) = self.active_championship.take() else {
            return Err("NO HAY CAMPEONATO ACTIVO PARA ABANDONAR".into());
        };
        if !champ.finished {
            let final_position = champ.rivals.len() + 1;
            let total_ms = champ.player_total_ms();
            self.completed_championships
                .push(CompletedChampionshipRecord {
                    championship_id: champ.id,
                    title: champ.title,
                    difficulty: champ.difficulty,
                    vehicle_id: champ.locked_vehicle_id,
                    vehicle_name: champ.locked_vehicle_name,
                    final_position,
                    total_ms,
                    prize_usd: 0,
                    abandoned: true,
                    completed_unix_s: unix_now_s(),
                });
        }
        Ok(())
    }

    /// Dismisses a finished championship screen cleanly without double-paying.
    pub fn dismiss_finished_championship(&mut self) -> Result<(), String> {
        let Some(champ) = self.active_championship.as_ref() else {
            return Ok(());
        };
        if !champ.finished {
            return Err("EL CAMPEONATO TODAVÍA ESTÁ EN CURSO".into());
        }
        self.active_championship = None;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CareerStore {
    pub profile: Option<CareerProfile>,
    pub load_error: Option<String>,
}

impl CareerStore {
    pub fn path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(Self::path_in(&root))
    }

    pub fn path_in(documents_root: &Path) -> PathBuf {
        documents_root.join("RBR").join("career_profile.json")
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(path) {
            Ok(text) => match parse_profile_json(&text) {
                Ok(profile) => Self {
                    profile: Some(profile),
                    load_error: None,
                },
                Err(err) => Self {
                    profile: None,
                    load_error: Some(format!("PERFIL CARRERA CORRUPTO: {err}")),
                },
            },
            Err(err) => Self {
                profile: None,
                load_error: Some(format!("ERROR LEYENDO CARRERA: {err}")),
            },
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        let Some(profile) = self.profile.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp_name = format!(
            ".{}.tmp.{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("career_profile.json"),
            std::process::id()
        );
        let tmp_path = path.with_file_name(tmp_name);
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
            file.write_all(encode_profile_json(profile).as_bytes())
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        match std::fs::rename(&tmp_path, path) {
            Ok(()) => Ok(()),
            Err(_) => {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
                std::fs::rename(&tmp_path, path).map_err(|e| {
                    let _ = std::fs::remove_file(&tmp_path);
                    e.to_string()
                })
            }
        }
    }

    /// Creates a brand-new profile ONLY if none exists (never resets or duplicates initial cash).
    pub fn create_profile(&mut self, pilot_name: &str) -> Result<(), String> {
        if self.profile.is_some() {
            return Err("YA EXISTE UN PERFIL DE CARRERA ACTIVO".into());
        }
        let profile = CareerProfile::create(pilot_name)?;
        self.profile = Some(profile);
        self.load_error = None;
        self.save()
    }
}

pub fn sanitize_pilot_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.') {
            out.push(ch.to_ascii_uppercase());
            if out.len() >= 18 {
                break;
            }
        }
    }
    out.trim().to_string()
}

fn unix_now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------
// Zero-dependency deterministic JSON encoder & parser for `career_profile.json`
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn encode_profile_json(p: &CareerProfile) -> String {
    let mut out = String::with_capacity(2048);
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"schema_version\": {},", p.schema_version);
    let _ = writeln!(out, "  \"pilot_name\": {},", json_escape(&p.pilot_name));
    let _ = writeln!(out, "  \"cash_usd\": {},", p.cash_usd);
    out.push_str("  \"owned_vehicle_ids\": [");
    for (i, id) in p.owned_vehicle_ids.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&json_escape(id));
    }
    out.push_str("],\n");
    match &p.active_career_vehicle_id {
        Some(id) => {
            let _ = writeln!(out, "  \"active_career_vehicle_id\": {},", json_escape(id));
        }
        None => {
            out.push_str("  \"active_career_vehicle_id\": null,\n");
        }
    }
    let _ = writeln!(out, "  \"podiums\": {},", p.podiums);
    let _ = writeln!(out, "  \"wins\": {},", p.wins);
    let _ = writeln!(out, "  \"historics_unlocked\": {},", p.historics_unlocked);
    let _ = writeln!(
        out,
        "  \"next_championship_seq\": {},",
        p.next_championship_seq
    );

    out.push_str("  \"transactions\": [\n");
    for (i, tx) in p.transactions.iter().enumerate() {
        let comma = if i + 1 < p.transactions.len() {
            ","
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "    {{\"kind\": {}, \"delta_usd\": {}, \"balance_after_usd\": {}, \"description\": {}, \"timestamp_unix_s\": {}}}{comma}",
            json_escape(tx.kind.code()),
            tx.delta_usd,
            tx.balance_after_usd,
            json_escape(&tx.description),
            tx.timestamp_unix_s
        );
    }
    out.push_str("  ],\n");

    out.push_str("  \"completed_championships\": [\n");
    for (i, rec) in p.completed_championships.iter().enumerate() {
        let comma = if i + 1 < p.completed_championships.len() {
            ","
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "    {{\"championship_id\": {}, \"title\": {}, \"difficulty\": {}, \"vehicle_id\": {}, \"vehicle_name\": {}, \"final_position\": {}, \"total_ms\": {}, \"prize_usd\": {}, \"abandoned\": {}, \"completed_unix_s\": {}}}{comma}",
            json_escape(&rec.championship_id),
            json_escape(&rec.title),
            json_escape(rec.difficulty.code()),
            json_escape(&rec.vehicle_id),
            json_escape(&rec.vehicle_name),
            rec.final_position,
            rec.total_ms,
            rec.prize_usd,
            rec.abandoned,
            rec.completed_unix_s
        );
    }
    out.push_str("  ],\n");

    match &p.active_championship {
        None => {
            out.push_str("  \"active_championship\": null\n");
        }
        Some(c) => {
            out.push_str("  \"active_championship\": {\n");
            let _ = writeln!(out, "    \"id\": {},", json_escape(&c.id));
            let _ = writeln!(out, "    \"title\": {},", json_escape(&c.title));
            let _ = writeln!(out, "    \"seed\": {},", c.seed);
            let _ = writeln!(
                out,
                "    \"difficulty\": {},",
                json_escape(c.difficulty.code())
            );
            let _ = writeln!(
                out,
                "    \"locked_vehicle_id\": {},",
                json_escape(&c.locked_vehicle_id)
            );
            let _ = writeln!(
                out,
                "    \"locked_vehicle_name\": {},",
                json_escape(&c.locked_vehicle_name)
            );
            out.push_str("    \"stage_ids\": [");
            for (i, s) in c.stage_ids.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(s));
            }
            out.push_str("],\n");
            out.push_str("    \"player_stage_times_ms\": [");
            for (i, ms) in c.player_stage_times_ms.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "{ms}");
            }
            out.push_str("],\n");
            out.push_str("    \"rivals\": [\n");
            for (i, rv) in c.rivals.iter().enumerate() {
                let comma = if i + 1 < c.rivals.len() { "," } else { "" };
                out.push_str("      {\"name\": ");
                out.push_str(&json_escape(&rv.name));
                out.push_str(", \"car_label\": ");
                out.push_str(&json_escape(&rv.car_label));
                out.push_str(", \"stage_times_ms\": [");
                for (j, ms) in rv.stage_times_ms.iter().enumerate() {
                    if j > 0 {
                        out.push_str(", ");
                    }
                    let _ = write!(out, "{ms}");
                }
                let _ = writeln!(out, "]}}{comma}");
            }
            out.push_str("    ],\n");
            let _ = writeln!(out, "    \"finished\": {},", c.finished);
            let _ = writeln!(out, "    \"reward_claimed\": {},", c.reward_claimed);
            match c.final_position {
                Some(pos) => {
                    let _ = writeln!(out, "    \"final_position\": {pos},");
                }
                None => {
                    out.push_str("    \"final_position\": null,\n");
                }
            }
            let _ = writeln!(out, "    \"prize_awarded_usd\": {}", c.prize_awarded_usd);
            out.push_str("  }\n");
        }
    }
    out.push_str("}\n");
    out
}

#[derive(Debug, Clone)]
enum JsonVal {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Str(String),
    Array(Vec<JsonVal>),
    Object(Vec<(String, JsonVal)>),
}

impl JsonVal {
    fn as_obj(&self) -> Option<&[(String, JsonVal)]> {
        match self {
            Self::Object(v) => Some(v),
            _ => None,
        }
    }
    fn field(&self, key: &str) -> Option<&JsonVal> {
        self.as_obj()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Uint(v) => Some(*v),
            Self::Int(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Uint(v) => i64::try_from(*v).ok(),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&[JsonVal]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", char::from(b), self.pos))
        }
    }

    fn parse_val(&mut self) -> Result<JsonVal, String> {
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonVal::Str),
            Some(b't') => self.parse_lit(b"true", JsonVal::Bool(true)),
            Some(b'f') => self.parse_lit(b"false", JsonVal::Bool(false)),
            Some(b'n') => self.parse_lit(b"null", JsonVal::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            other => Err(format!("unexpected token {other:?} at byte {}", self.pos)),
        }
    }

    fn parse_lit(&mut self, lit: &[u8], val: JsonVal) -> Result<JsonVal, String> {
        self.skip_ws();
        if self.bytes[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(val)
        } else {
            Err(format!("invalid literal at byte {}", self.pos))
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    if self.pos >= self.bytes.len() {
                        return Err("unterminated escape".into());
                    }
                    let esc = self.bytes[self.pos];
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            if self.pos + 4 > self.bytes.len() {
                                return Err("truncated unicode escape".into());
                            }
                            let hex = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
                                .map_err(|_| "invalid utf8 escape".to_string())?;
                            let code = u32::from_str_radix(hex, 16)
                                .map_err(|_| "invalid hex escape".to_string())?;
                            self.pos += 4;
                            if let Some(ch) = char::from_u32(code) {
                                out.push(ch);
                            }
                        }
                        _ => return Err("unknown escape sequence".into()),
                    }
                }
                _ => {
                    let start = self.pos - 1;
                    let s = std::str::from_utf8(&self.bytes[start..])
                        .map_err(|_| "invalid utf8 in string".to_string())?;
                    let ch = s.chars().next().ok_or("empty utf8 char")?;
                    out.push(ch);
                    self.pos = start + ch.len_utf8();
                }
            }
        }
        Err("unterminated string".into())
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        self.skip_ws();
        let start = self.pos;
        let neg = if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
            true
        } else {
            false
        };
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let slice = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid number".to_string())?;
        if neg {
            slice
                .parse::<i64>()
                .map(JsonVal::Int)
                .map_err(|e| format!("bad int {slice:?}: {e}"))
        } else {
            slice
                .parse::<u64>()
                .map(JsonVal::Uint)
                .map_err(|e| format!("bad uint {slice:?}: {e}"))
        }
    }

    fn parse_array(&mut self) -> Result<JsonVal, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Array(items));
        }
        loop {
            items.push(self.parse_val()?);
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonVal::Array(items));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonVal, String> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Object(fields));
        }
        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            let val = self.parse_val()?;
            fields.push((key, val));
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonVal::Object(fields));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
    }
}

pub fn parse_profile_json(text: &str) -> Result<CareerProfile, String> {
    let mut parser = JsonParser::new(text);
    let root = parser.parse_val()?;
    let schema_version = root
        .field("schema_version")
        .and_then(JsonVal::as_u64)
        .ok_or("missing schema_version")? as u32;
    if schema_version != CAREER_SCHEMA_VERSION {
        return Err(format!("unsupported schema_version {schema_version}"));
    }
    let pilot_name = root
        .field("pilot_name")
        .and_then(JsonVal::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .ok_or("missing pilot_name")?;
    let cash_usd = root
        .field("cash_usd")
        .and_then(JsonVal::as_u64)
        .ok_or("missing cash_usd")?;
    let owned_vehicle_ids = root
        .field("owned_vehicle_ids")
        .and_then(JsonVal::as_array)
        .ok_or("missing owned_vehicle_ids")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let active_career_vehicle_id = match root.field("active_career_vehicle_id") {
        Some(JsonVal::Str(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    let podiums = root.field("podiums").and_then(JsonVal::as_u64).unwrap_or(0) as u32;
    let wins = root.field("wins").and_then(JsonVal::as_u64).unwrap_or(0) as u32;
    let historics_unlocked = root
        .field("historics_unlocked")
        .and_then(JsonVal::as_bool)
        .unwrap_or(false);
    let next_championship_seq = root
        .field("next_championship_seq")
        .and_then(JsonVal::as_u64)
        .unwrap_or(1)
        .max(1);

    let mut transactions = Vec::new();
    if let Some(arr) = root.field("transactions").and_then(JsonVal::as_array) {
        for item in arr {
            let kind = item
                .field("kind")
                .and_then(JsonVal::as_str)
                .map(TransactionKind::from_code)
                .unwrap_or(TransactionKind::InitialGrant);
            let delta_usd = item
                .field("delta_usd")
                .and_then(JsonVal::as_i64)
                .unwrap_or(0);
            let balance_after_usd = item
                .field("balance_after_usd")
                .and_then(JsonVal::as_u64)
                .unwrap_or(0);
            let description = item
                .field("description")
                .and_then(JsonVal::as_str)
                .unwrap_or("")
                .to_string();
            let timestamp_unix_s = item
                .field("timestamp_unix_s")
                .and_then(JsonVal::as_u64)
                .unwrap_or(0);
            transactions.push(Transaction {
                kind,
                delta_usd,
                balance_after_usd,
                description,
                timestamp_unix_s,
            });
        }
    }

    let mut completed_championships = Vec::new();
    if let Some(arr) = root
        .field("completed_championships")
        .and_then(JsonVal::as_array)
    {
        for item in arr {
            completed_championships.push(CompletedChampionshipRecord {
                championship_id: item
                    .field("championship_id")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string(),
                title: item
                    .field("title")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string(),
                difficulty: item
                    .field("difficulty")
                    .and_then(JsonVal::as_str)
                    .map(CareerDifficulty::from_code)
                    .unwrap_or_default(),
                vehicle_id: item
                    .field("vehicle_id")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string(),
                vehicle_name: item
                    .field("vehicle_name")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string(),
                final_position: item
                    .field("final_position")
                    .and_then(JsonVal::as_u64)
                    .unwrap_or(0) as usize,
                total_ms: item
                    .field("total_ms")
                    .and_then(JsonVal::as_u64)
                    .unwrap_or(0),
                prize_usd: item
                    .field("prize_usd")
                    .and_then(JsonVal::as_u64)
                    .unwrap_or(0),
                abandoned: item
                    .field("abandoned")
                    .and_then(JsonVal::as_bool)
                    .unwrap_or(false),
                completed_unix_s: item
                    .field("completed_unix_s")
                    .and_then(JsonVal::as_u64)
                    .unwrap_or(0),
            });
        }
    }

    let active_championship = match root.field("active_championship") {
        Some(obj @ JsonVal::Object(_)) => {
            let id = obj
                .field("id")
                .and_then(JsonVal::as_str)
                .ok_or("active_championship missing id")?
                .to_string();
            let title = obj
                .field("title")
                .and_then(JsonVal::as_str)
                .unwrap_or("COPA REGIONAL DE INICIACIÓN")
                .to_string();
            let seed = obj.field("seed").and_then(JsonVal::as_u64).unwrap_or(1);
            let difficulty = obj
                .field("difficulty")
                .and_then(JsonVal::as_str)
                .map(CareerDifficulty::from_code)
                .unwrap_or_default();
            let locked_vehicle_id = obj
                .field("locked_vehicle_id")
                .and_then(JsonVal::as_str)
                .ok_or("active_championship missing locked_vehicle_id")?
                .to_string();
            let locked_vehicle_name = obj
                .field("locked_vehicle_name")
                .and_then(JsonVal::as_str)
                .unwrap_or(&locked_vehicle_id)
                .to_string();
            let stage_ids = obj
                .field("stage_ids")
                .and_then(JsonVal::as_array)
                .ok_or("active_championship missing stage_ids")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            let player_stage_times_ms = obj
                .field("player_stage_times_ms")
                .and_then(JsonVal::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(JsonVal::as_u64)
                .collect();
            let mut rivals = Vec::new();
            if let Some(arr) = obj.field("rivals").and_then(JsonVal::as_array) {
                for r in arr {
                    let name = r
                        .field("name")
                        .and_then(JsonVal::as_str)
                        .unwrap_or("RIVAL")
                        .to_string();
                    let car_label = r
                        .field("car_label")
                        .and_then(JsonVal::as_str)
                        .unwrap_or("S1600")
                        .to_string();
                    let stage_times_ms = r
                        .field("stage_times_ms")
                        .and_then(JsonVal::as_array)
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(JsonVal::as_u64)
                        .collect();
                    rivals.push(RivalEntry {
                        name,
                        car_label,
                        stage_times_ms,
                    });
                }
            }
            let finished = obj
                .field("finished")
                .and_then(JsonVal::as_bool)
                .unwrap_or(false);
            let reward_claimed = obj
                .field("reward_claimed")
                .and_then(JsonVal::as_bool)
                .unwrap_or(false);
            let final_position = obj
                .field("final_position")
                .and_then(JsonVal::as_u64)
                .map(|v| v as usize);
            let prize_awarded_usd = obj
                .field("prize_awarded_usd")
                .and_then(JsonVal::as_u64)
                .unwrap_or(0);
            Some(ActiveChampionship {
                id,
                title,
                seed,
                difficulty,
                locked_vehicle_id,
                locked_vehicle_name,
                stage_ids,
                player_stage_times_ms,
                rivals,
                finished,
                reward_claimed,
                final_position,
                prize_awarded_usd,
            })
        }
        _ => None,
    };

    Ok(CareerProfile {
        schema_version,
        pilot_name,
        cash_usd,
        owned_vehicle_ids,
        active_career_vehicle_id,
        podiums,
        wins,
        historics_unlocked,
        next_championship_seq,
        transactions,
        completed_championships,
        active_championship,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driving::content_manager::{Catalog, EntryState, FileAudit};
    use rerbr_content::{Selection, VehicleClassification, VehicleDescriptor};

    fn make_entry(desc: VehicleDescriptor, visual_only: bool) -> VehicleEntry {
        let year = super::super::content_manager::preset_year_for_vehicle_id(&desc.id);
        VehicleEntry {
            classification: if visual_only {
                VehicleClassification::VisualInspectionOnly
            } else {
                desc.base_classification()
            },
            year,
            descriptor: desc,
            state: EntryState::Installed,
            files: FileAudit::unchecked(),
            tech_sheet: None,
            drivetrain: None,
            fmod_bank: None,
            using_fallback_model: false,
            model_missing: false,
            has_3d_model: true,
            notes: Vec::new(),
        }
    }

    fn mg_zr_entry() -> VehicleEntry {
        let desc = VehicleDescriptor::original_rbr_presets()
            .into_iter()
            .find(|v| v.id == "mg-zr-s1600")
            .unwrap();
        make_entry(desc, false)
    }

    fn corolla_wrc_entry() -> VehicleEntry {
        make_entry(VehicleDescriptor::toyota_corolla_wrc(), false)
    }

    fn clio_rally5_visual_only() -> VehicleEntry {
        let mut desc = VehicleDescriptor::ford_focus_2006();
        desc.id = "rsf-renault-clio-rally5".into();
        desc.name = "Renault Clio Rally5".into();
        desc.category = Some("Rally 5".into());
        desc.rsf_pack = Some("Renault_Clio_Rally5_ngp6".into());
        make_entry(desc, true)
    }

    #[test]
    fn profile_starts_with_15000_usd_zero_cars_and_never_duplicates_initial_cash() {
        let dir = std::env::temp_dir().join(format!("rerbr-career-init-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = CareerStore::path_in(&dir);

        let mut store = CareerStore::load_from_path(&path);
        assert!(store.profile.is_none());
        let p = CareerProfile::create("Adrian").unwrap();
        assert_eq!(p.pilot_name, "ADRIAN");
        assert_eq!(p.cash_usd, 15_000);
        assert!(p.owned_vehicle_ids.is_empty());
        assert_eq!(p.transactions.len(), 1);

        store.profile = Some(p);
        store.save_to_path(&path).unwrap();

        // Reloading must restore exact balance and refuse duplicate profile creation.
        let mut reloaded = CareerStore::load_from_path(&path);
        assert_eq!(reloaded.profile.as_ref().unwrap().cash_usd, 15_000);
        assert!(reloaded.create_profile("Otro").is_err());
        assert_eq!(reloaded.profile.as_ref().unwrap().cash_usd, 15_000);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dealership_allows_initiation_purchase_blocks_visual_only_duplicates_and_overdraft() {
        let mut p = CareerProfile::create("Piloto").unwrap();
        let zr = mg_zr_entry();
        let wrc = corolla_wrc_entry();
        let clio_vis = clio_rally5_visual_only();

        // Visual-only model MUST NOT be purchasable.
        assert!(p.buy_vehicle(&clio_vis, true).is_err());
        assert_eq!(p.cash_usd, 15_000);

        // Expensive WRC exceeds initial balance -> overdraft blocked.
        assert!(p.buy_vehicle(&wrc, false).is_err());
        assert_eq!(p.cash_usd, 15_000);

        // Initiation MG ZR S1600 (US$12,500) succeeds and sets active career car.
        let paid = p.buy_vehicle(&zr, false).unwrap();
        assert_eq!(paid, 12_500);
        assert_eq!(p.cash_usd, 2_500);
        assert!(p.owns_vehicle("mg-zr-s1600"));
        assert_eq!(p.active_career_vehicle_id.as_deref(), Some("mg-zr-s1600"));

        // Duplicate purchase blocked.
        assert!(p.buy_vehicle(&zr, false).is_err());
        assert_eq!(p.cash_usd, 2_500);
    }

    #[test]
    fn regional_championship_full_flow_locks_car_and_difficulty_and_pays_once() {
        let dir = std::env::temp_dir().join(format!("rerbr-career-champ-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = CareerStore::path_in(&dir);

        let mut p = CareerProfile::create("Adrian").unwrap();
        let zr = mg_zr_entry();
        let wrc = corolla_wrc_entry();

        // Cannot start championship without owning an eligible car.
        assert!(p
            .start_regional_championship(&zr, false, CareerDifficulty::Intermediate)
            .is_err());

        // Buy MG ZR S1600 -> US$2,500 remaining.
        p.buy_vehicle(&zr, false).unwrap();

        // Even if player somehow owned WRC, Regional championship rejects WRC category.
        p.owned_vehicle_ids.push(wrc.descriptor.id.clone());
        assert!(p
            .start_regional_championship(&wrc, false, CareerDifficulty::Intermediate)
            .is_err());

        // Start Regional Championship with MG ZR S1600 on Intermediate.
        p.start_regional_championship(&zr, false, CareerDifficulty::Intermediate)
            .unwrap();
        assert!(p.has_locked_championship());

        // Car switching is blocked while championship is active!
        assert!(p.select_career_vehicle("toyota-corolla-wrc").is_err());

        // Cannot skip Stage 1 to submit Stage 2.
        assert!(p
            .record_championship_stage_result("cote-darbroz", "mg-zr-s1600", 170_000, true)
            .is_err());
        // Invalid run rejected.
        assert!(p
            .record_championship_stage_result("diamond-creek", "mg-zr-s1600", 150_000, false)
            .is_err());
        // Wrong car rejected.
        assert!(p
            .record_championship_stage_result("diamond-creek", "toyota-corolla-wrc", 150_000, true)
            .is_err());

        // Complete Stage 1 (fast winning pace).
        let s1 = p
            .record_championship_stage_result("diamond-creek", "mg-zr-s1600", 160_000, true)
            .unwrap();
        assert!(!s1.championship_finished);
        assert_eq!(s1.prize_awarded_usd, 0);
        assert_eq!(s1.current_position, 1);

        // Persist mid-championship and reload -> exact progress recovered!
        let store = CareerStore {
            profile: Some(p),
            load_error: None,
        };
        store.save_to_path(&path).unwrap();
        let mut p = CareerStore::load_from_path(&path).profile.unwrap();
        assert_eq!(
            p.active_championship.as_ref().unwrap().completed_stages(),
            1
        );
        assert_eq!(
            p.active_championship.as_ref().unwrap().next_stage_id(),
            Some("cote-darbroz")
        );

        // Complete Stage 2 & Stage 3 (winning 1st place!).
        let s2 = p
            .record_championship_stage_result("cote-darbroz", "mg-zr-s1600", 165_000, true)
            .unwrap();
        assert!(!s2.championship_finished);

        let cash_before_finish = p.cash_usd;
        let s3 = p
            .record_championship_stage_result("shepherds-shield", "mg-zr-s1600", 190_000, true)
            .unwrap();
        assert!(s3.championship_finished);
        assert_eq!(s3.current_position, 1);
        let expected_prize = CareerDifficulty::Intermediate.regional_prize_for_position(1);
        assert_eq!(s3.prize_awarded_usd, expected_prize);
        assert_eq!(p.cash_usd, cash_before_finish + expected_prize);
        assert_eq!(p.wins, 1);
        assert_eq!(p.podiums, 1);
        assert!(p.historics_unlocked);
        assert!(s3.newly_unlocked_historics);

        // Anti-exploit: trying to record again NEVER pays a second time!
        assert!(p
            .record_championship_stage_result("shepherds-shield", "mg-zr-s1600", 180_000, true)
            .is_err());
        assert_eq!(p.cash_usd, cash_before_finish + expected_prize);

        // Dismiss finished championship cleanly -> unlocks car selection.
        p.dismiss_finished_championship().unwrap();
        assert!(p.active_championship.is_none());
        assert!(p.select_career_vehicle("toyota-corolla-wrc").is_ok());

        // Verify persistence after completion.
        CareerStore {
            profile: Some(p.clone()),
            load_error: None,
        }
        .save_to_path(&path)
        .unwrap();
        let final_reloaded = CareerStore::load_from_path(&path).profile.unwrap();
        assert_eq!(final_reloaded.cash_usd, p.cash_usd);
        assert_eq!(final_reloaded.wins, 1);
        assert_eq!(final_reloaded.podiums, 1);
        assert!(final_reloaded.historics_unlocked);
        assert_eq!(final_reloaded.completed_championships.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn abandon_championship_awards_zero_and_unlocks_new_attempt() {
        let mut p = CareerProfile::create("Adrian").unwrap();
        let zr = mg_zr_entry();
        p.buy_vehicle(&zr, false).unwrap();
        p.start_regional_championship(&zr, false, CareerDifficulty::Expert)
            .unwrap();
        let cash = p.cash_usd;
        p.abandon_championship().unwrap();
        assert_eq!(p.cash_usd, cash);
        assert!(!p.has_locked_championship());
        assert_eq!(p.completed_championships.len(), 1);
        assert!(p.completed_championships[0].abandoned);
        assert_eq!(p.completed_championships[0].prize_usd, 0);
        assert!(p
            .start_regional_championship(&zr, false, CareerDifficulty::Beginner)
            .is_ok());
    }

    #[test]
    fn regional_stages_all_exist_in_preset_catalog_and_format_usd_works() {
        for idx in 0..REGIONAL_STAGES.len() {
            let desc = regional_stage_descriptor(idx).expect("regional stage preset must exist");
            assert!(desc.validate_current_backend().is_ok());
        }
        let cat = Catalog::descriptors(&Selection::default());
        let zr = cat.vehicle("mg-zr-s1600").expect("MG ZR S1600 in catalog");
        let spec = vehicle_market_spec(zr);
        assert!(spec.tier.eligible_for_regional());
        assert!(spec.price_usd <= INITIAL_CASH_USD);
        assert_eq!(format_usd(15_000), "US$15,000");
        assert_eq!(format_usd(1_000_000), "US$1,000,000");
    }

    fn make_rsf_entry(id: &str, name: &str, category: &str, year: u32) -> VehicleEntry {
        let mut desc = VehicleDescriptor::ford_focus_2006();
        desc.id = id.into();
        desc.name = name.into();
        desc.category = Some(category.into());
        desc.rsf_pack = Some(format!("{id}_ngp6"));
        let mut e = make_entry(desc, false);
        e.year = Some(year);
        e
    }

    #[test]
    fn commercial_dealership_rules_enforce_new_salon_0km_vs_used_2a_mano_and_unified_garage() {
        // 1. Rally5: exclusively in SALÓN AUTOMOTOR (0 KM), affordable with initial US$15,000, never in 2ª MANO.
        let clio_r5 = make_rsf_entry(
            "rsf-renault-clio-rally5",
            "Renault Clio Rally5",
            "Rally 5",
            2021,
        );
        let s_r5 = vehicle_market_spec(&clio_r5);
        assert_eq!(s_r5.concesionario, DealershipChannel::NewSalonOnly);
        assert!(s_r5.offered_in(DealershipKind::NewSalon));
        assert!(!s_r5.offered_in(DealershipKind::UsedCars));
        assert!(s_r5.precio_0km.unwrap() <= INITIAL_CASH_USD);
        assert!(s_r5.precio_usado.is_none());

        // 2. Rally4: offered in BOTH SALÓN AUTOMOTOR (0 KM) and 2ª MANO (USADO with discount < 0 KM).
        for (id, name) in [
            ("rsf-renault-clio-rally4", "Renault Clio Rally4"),
            ("rsf-ford-fiesta-rally4", "Ford Fiesta Rally4"),
            ("rsf-peugeot-208-rally4", "Peugeot 208 Rally4"),
        ] {
            let car = make_rsf_entry(id, name, "Rally 4", 2021);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::Both);
            assert!(sp.offered_in(DealershipKind::NewSalon));
            assert!(sp.offered_in(DealershipKind::UsedCars));
            assert!(sp.precio_usado.unwrap() < sp.precio_0km.unwrap());
            assert!(sp.descuento_usado_pct > 0);
        }

        // 3. R2 and Super 1600: exclusively in 2ª MANO (USADO), keeping real category.
        for (id, name, yr) in [
            ("rsf-opel-adam-r2", "Opel ADAM R2", 2013),
            ("rsf-ford-fiesta-mk-viii-r2", "Ford Fiesta Mk VIII R2", 2019),
            ("rsf-peugeot-208-r2", "Peugeot 208 R2", 2012),
            ("rsf-citroen-c2-r2-max", "Citroen C2 R2 Max", 2011),
        ] {
            let car = make_rsf_entry(id, name, "Group R2", yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::UsedOnly);
            assert!(!sp.offered_in(DealershipKind::NewSalon));
            assert!(sp.offered_in(DealershipKind::UsedCars));
            assert_eq!(sp.categoria, "Group R2");
        }
        let zr = mg_zr_entry();
        let sp_zr = vehicle_market_spec(&zr);
        assert_eq!(sp_zr.concesionario, DealershipChannel::UsedOnly);
        assert_eq!(sp_zr.categoria, "Super 1600");

        // 4. Rally3: Fiesta Rally3 Evo in SALÓN AUTOMOTOR (0 KM); standard Fiesta Rally3 & Clio Rally3 in 2ª MANO.
        let evo = make_rsf_entry(
            "rsf-ford-fiesta-rally3-evo",
            "Ford Fiesta rally3 evo",
            "Rally 3",
            2023,
        );
        let sp_evo = vehicle_market_spec(&evo);
        assert_eq!(sp_evo.concesionario, DealershipChannel::NewSalonOnly);
        assert!(sp_evo.offered_in(DealershipKind::NewSalon));
        assert!(!sp_evo.offered_in(DealershipKind::UsedCars));
        for (id, name, yr) in [
            ("rsf-ford-fiesta-rally3", "Ford Fiesta Rally3", 2021),
            ("rsf-renault-clio-rally3", "Renault Clio Rally3", 2023),
        ] {
            let car = make_rsf_entry(id, name, "Rally 3", yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::UsedOnly);
            assert!(!sp.offered_in(DealershipKind::NewSalon));
            assert!(sp.offered_in(DealershipKind::UsedCars));
        }

        // 5. Rally2 / R5: Modern Rally2 in BOTH; older R5 exclusively in 2ª MANO.
        for (id, name, yr) in [
            ("rsf-toyota-gr-yaris-rally2", "Toyota GR Yaris Rally2", 2024),
            ("rsf-skoda-fabia-rs-rally2", "Skoda Fabia RS Rally2", 2024),
            ("rsf-ford-fiesta-rally2", "Ford Fiesta Rally2", 2023),
        ] {
            let car = make_rsf_entry(id, name, "Rally 2", yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::Both);
            assert!(sp.precio_usado.unwrap() < sp.precio_0km.unwrap());
        }
        let r5 = make_rsf_entry("rsf-citroen-c3-r5", "Citroen C3 R5", "Group R5", 2020);
        let sp_r5 = vehicle_market_spec(&r5);
        assert_eq!(sp_r5.concesionario, DealershipChannel::UsedOnly);

        // 6. WRC 2018–2020: exclusively in SALÓN AUTOMOTOR (0 KM), NEVER in 2ª MANO.
        for (id, name, yr) in [
            ("rsf-toyota-yaris-wrc-2018", "Toyota Yaris WRC 2018", 2018),
            ("rsf-ford-fiesta-wrc-2019", "Ford Fiesta WRC 2019", 2019),
            (
                "rsf-hyundai-i20-coupe-wrc-2020",
                "Hyundai i20 Coupe WRC 2020",
                2020,
            ),
        ] {
            let car = make_rsf_entry(id, name, "WRC 1.6", yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::NewSalonOnly);
            assert!(sp.offered_in(DealershipKind::NewSalon));
            assert!(!sp.offered_in(DealershipKind::UsedCars));
        }

        // 7. WRC 2014–2015: exclusively in 2ª MANO (USADO), NEVER as 0 KM.
        for (id, name) in [
            ("rsf-citroen-ds3-wrc", "Citroen DS3 WRC"),
            ("rsf-ford-fiesta-rs-wrc-2014", "Ford Fiesta RS WRC 2014"),
        ] {
            let car = make_rsf_entry(id, name, "WRC 1.6", 2014);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::UsedOnly);
            assert!(!sp.offered_in(DealershipKind::NewSalon));
            assert!(sp.offered_in(DealershipKind::UsedCars));
        }

        // 8. WRC 2.0 2000–2009 & Group B & Group A: exclusively in 2ª MANO (USADO), NEVER as 0 KM.
        for (id, name, cat, yr) in [
            (
                "ford-focus-rs-wrc-2006",
                "Ford Focus Mk II RS WRC 2006",
                "WRC 2.0",
                2006,
            ),
            (
                "rsf-citroen-xsara-wrc-2006",
                "Citroen Xsara WRC 2006",
                "WRC 2.0",
                2006,
            ),
            (
                "rsf-citroen-c4-wrc-2008",
                "Citroen C4 WRC 2008",
                "WRC 2.0",
                2008,
            ),
            (
                "rsf-audi-sport-quattro-grpb",
                "Audi Sport quattro GrpB",
                "Group B",
                1985,
            ),
            (
                "rsf-peugeot-205-t16-grpb",
                "Peugeot 205 T16 GrpB",
                "Group B",
                1985,
            ),
            (
                "rsf-peugeot-306-maxi-kit-car",
                "Peugeot 306 Maxi Kit Car",
                "Group A7",
                1998,
            ),
        ] {
            let car = make_rsf_entry(id, name, cat, yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::UsedOnly);
            assert!(!sp.offered_in(DealershipKind::NewSalon));
            assert!(sp.offered_in(DealershipKind::UsedCars));
        }

        // 9. WRC of undefined commercial years (2016, 2017): PendingCommercialRule (neither dealership).
        for (id, name, yr) in [
            ("rsf-vw-polo-r-wrc-2016", "VW Polo R WRC 2016", 2016),
            ("rsf-citroen-c3-wrc-2017", "Citroen C3 WRC 2017", 2017),
        ] {
            let car = make_rsf_entry(id, name, "WRC 1.6", yr);
            let sp = vehicle_market_spec(&car);
            assert_eq!(sp.concesionario, DealershipChannel::PendingCommercialRule);
            assert!(!sp.offered_in(DealershipKind::NewSalon));
            assert!(!sp.offered_in(DealershipKind::UsedCars));
        }

        // 10. Unified personal Career Garage & duplicate protection across both dealerships:
        let mut profile = CareerProfile::create("Adrian").unwrap();
        // Cannot buy Clio Rally5 in 2ª MANO:
        assert!(profile
            .buy_vehicle_in_dealership(&clio_r5, DealershipKind::UsedCars, true)
            .is_err());
        // Buy Clio Rally5 0 KM in SALÓN AUTOMOTOR with initial US$15,000:
        let paid_r5 = profile
            .buy_vehicle_in_dealership(&clio_r5, DealershipKind::NewSalon, true)
            .unwrap();
        assert_eq!(paid_r5, 13_800);
        assert!(profile.owns_vehicle("rsf-renault-clio-rally5"));

        // Grant cash and buy a Rally4 (offered in BOTH) from 2ª MANO:
        profile.cash_usd = 100_000;
        let clio_r4 = make_rsf_entry(
            "rsf-renault-clio-rally4",
            "Renault Clio Rally4",
            "Rally 4",
            2021,
        );
        let paid_used_r4 = profile
            .buy_vehicle_in_dealership(&clio_r4, DealershipKind::UsedCars, true)
            .unwrap();
        assert_eq!(paid_used_r4, 18_500);
        // Attempting to buy the same Rally4 again in SALÓN AUTOMOTOR (0 KM) is blocked!
        assert!(profile
            .buy_vehicle_in_dealership(&clio_r4, DealershipKind::NewSalon, true)
            .is_err());
        assert_eq!(profile.owned_vehicle_ids.len(), 2);
    }
}
