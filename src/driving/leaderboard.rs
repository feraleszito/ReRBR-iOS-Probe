//! Sistema de Clasificación de Tiempos (Leaderboards) para RBR Mobile (Android e iOS/iPadOS).
//!
//! - Reutiliza `RecordKey`, `RunSubmission` y `RecordStore` (`src/driving/records.rs`).
//! - Persiste todos los tiempos válidos completados en `<Documents>/RBR/leaderboards.json`
//!   y migra automáticamente récords previos desde `personal_bests.json`.
//! - Separa explícitamente tiempos locales (`VerificationStatus::LocalOnly`) de futuros tiempos
//!   verificados por servidor (`VerificationStatus::ServerVerified`), sin fingir un backend activo.
//! - Incluye sellado de integridad SHA-256 y comprobación física básica contra resultados manipulados.

use super::records::{self, PersonalBest, RecordKey, RecordStore, RunSubmission};
use rerbr_sim::sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LEADERBOARD_SCHEMA_VERSION: u32 = 1;
pub const ONLINE_PAYLOAD_SCHEMA_VERSION: u32 = 1;
pub const MAX_ENTRIES_PER_STAGE: usize = 100;
/// Velocidad media máxima físicamente plausible en un tramo completo de rally (95 m/s = 342 km/h).
const MAX_PLAUSIBLE_AVG_SPEED_MPS: f64 = 95.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus {
    /// Tiempo registrado localmente en el dispositivo (nunca presentado como récord mundial).
    LocalOnly,
    /// Reservado para futura verificación en servidor global Android/iOS.
    ServerVerified {
        server_record_id: String,
        verified_unix_s: u64,
    },
}

impl VerificationStatus {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LocalOnly => "local",
            Self::ServerVerified { .. } => "server_verified",
        }
    }

    pub fn ui_badge(&self) -> &'static str {
        match self {
            Self::LocalOnly => "LOCAL",
            Self::ServerVerified { .. } => "VERIFICADO",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeaderboardFilter {
    #[default]
    StageAllCars,
    StageCurrentCar,
    PersonalBestsByStage,
}

impl LeaderboardFilter {
    pub fn next(self) -> Self {
        match self {
            Self::StageAllCars => Self::StageCurrentCar,
            Self::StageCurrentCar => Self::PersonalBestsByStage,
            Self::PersonalBestsByStage => Self::StageAllCars,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            Self::StageAllCars => "TRAMO · TODOS LOS COCHES",
            Self::StageCurrentCar => "TRAMO · COCHE ACTUAL",
            Self::PersonalBestsByStage => "MIS MEJORES POR TRAMO",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderboardEntry {
    pub entry_id: String,
    pub pilot_name: String,
    pub key: RecordKey,
    pub car_name: String,
    pub car_category: String,
    pub stage_name: String,
    pub total_ms: u64,
    pub driving_ms: u64,
    pub penalty_ms: u64,
    pub false_start: bool,
    pub splits_ms: Vec<u64>,
    pub splits_passed: usize,
    pub splits_total: usize,
    pub sim_steps: u64,
    pub stage_length_m: u32,
    pub platform: String,
    pub recorded_unix_s: u64,
    pub verification: VerificationStatus,
    pub integrity_sha256: String,
}

impl LeaderboardEntry {
    pub fn canonical_vehicle_id(&self) -> String {
        format!("{}@{}", self.key.car_id, self.key.physics_version)
    }

    pub fn canonical_stage_id(&self) -> String {
        format!(
            "{}|{}|{}",
            self.key.stage_id, self.key.stage_variant, self.key.surface
        )
    }

    pub fn physics_badge(&self) -> &'static str {
        if self.key.physics_version.starts_with("rsf-ngp-7.4") {
            "NGP 7.4"
        } else if self.key.physics_version.starts_with("rsf-ngp") {
            "NGP"
        } else {
            "RBR 1.02"
        }
    }

    pub fn compute_expected_digest(&self) -> String {
        compute_entry_digest(
            &self.pilot_name,
            &self.key,
            self.total_ms,
            self.driving_ms,
            self.penalty_ms,
            self.false_start,
            &self.splits_ms,
            self.sim_steps,
            self.stage_length_m,
            &self.platform,
            self.recorded_unix_s,
        )
    }

    pub fn verify_integrity(&self) -> bool {
        if self.driving_ms == 0 || self.total_ms != self.driving_ms.saturating_add(self.penalty_ms)
        {
            return false;
        }
        for pair in self.splits_ms.windows(2) {
            if pair[1] <= pair[0] {
                return false;
            }
        }
        if self.stage_length_m >= 500 {
            let secs = self.driving_ms as f64 / 1000.0;
            if secs <= 0.0 || (self.stage_length_m as f64 / secs) > MAX_PLAUSIBLE_AVG_SPEED_MPS {
                return false;
            }
        }
        self.integrity_sha256 == self.compute_expected_digest()
    }
}

#[derive(Debug, Clone)]
pub struct LeaderboardSubmission {
    pub run: RunSubmission,
    pub pilot_name: String,
    pub car_category: String,
    pub sim_steps: u64,
    pub stage_length_m: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderboardSubmitResult {
    pub accepted: bool,
    pub rejection_reason: Option<&'static str>,
    pub stage_rank: Option<usize>,
    pub stage_total_entries: usize,
    pub car_rank: Option<usize>,
    pub personal_best_ms: Option<u64>,
    pub is_stage_record: bool,
}

/// Sobre versionado listo para sincronización online futura entre jugadores de Android e iOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineSubmissionPayload {
    pub payload_version: u32,
    pub entry_id: String,
    pub canonical_stage_id: String,
    pub canonical_vehicle_id: String,
    pub rules_version: String,
    pub pilot_name: String,
    pub platform: String,
    pub total_ms: u64,
    pub driving_ms: u64,
    pub penalty_ms: u64,
    pub false_start: bool,
    pub splits_ms: Vec<u64>,
    pub sim_steps: u64,
    pub stage_length_m: u32,
    pub recorded_unix_s: u64,
    pub client_integrity_sha256: String,
}

#[derive(Debug, Clone, Default)]
pub struct LeaderboardStore {
    entries: Vec<LeaderboardEntry>,
    quarantined_tampered: usize,
}

impl LeaderboardStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path_in(documents_dir: &Path) -> PathBuf {
        documents_dir.join("RBR").join("leaderboards.json")
    }

    pub fn default_path() -> Option<PathBuf> {
        std::env::var_os("RERBR_PROBE_DOCUMENTS").map(|d| Self::path_in(Path::new(&d)))
    }

    pub fn load_with_migration(pb_store: &RecordStore) -> Self {
        let mut store = match Self::default_path() {
            Some(p) => Self::load_from_path(&p).unwrap_or_default(),
            None => Self::default(),
        };
        if store.migrate_from_personal_bests(pb_store) > 0 {
            let _ = store.save();
        }
        store
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("leaderboards read {}: {e}", path.display()))?;
        Self::from_json(&text)
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::default_path() else {
            return Ok(());
        };
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("leaderboards mkdir {}: {e}", parent.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        let payload = self.to_json();
        std::fs::write(&tmp, payload.as_bytes())
            .map_err(|e| format!("leaderboards write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("leaderboards rename {}: {e}", path.display())
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn quarantined_tampered_count(&self) -> usize {
        self.quarantined_tampered
    }

    pub fn entries(&self) -> &[LeaderboardEntry] {
        &self.entries
    }

    /// Importa récords existentes de `personal_bests.json` que aún no estén representados en la tabla.
    pub fn migrate_from_personal_bests(&mut self, pb_store: &RecordStore) -> usize {
        let mut added = 0usize;
        for pb in pb_store.all() {
            let already = self.entries.iter().any(|e| {
                e.key == pb.key
                    && e.total_ms == pb.best_total_ms
                    && e.recorded_unix_s == pb.recorded_unix_s
            });
            if already || pb.best_total_ms == 0 || pb.driving_ms == 0 {
                continue;
            }
            let entry = build_migrated_entry(pb);
            if entry.verify_integrity() {
                self.entries.push(entry);
                added += 1;
            }
        }
        if added > 0 {
            self.sort_and_trim();
        }
        added
    }

    /// Valida e inserta un nuevo resultado completado en la clasificación local.
    pub fn submit(&mut self, sub: &LeaderboardSubmission) -> LeaderboardSubmitResult {
        if let Err(reason) = validate_leaderboard_submission(sub) {
            return LeaderboardSubmitResult {
                accepted: false,
                rejection_reason: Some(reason),
                stage_rank: None,
                stage_total_entries: self.stage_entries(&sub.run.key.stage_id, None).len(),
                car_rank: None,
                personal_best_ms: self
                    .best_for_stage_and_car(&sub.run.key.stage_id, Some(&sub.run.key.car_id))
                    .map(|e| e.total_ms),
                is_stage_record: false,
            };
        }

        let now_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let platform = normalize_platform_tag(current_platform_tag());
        let pilot = sanitize_leaderboard_pilot(&sub.pilot_name);
        let category = if sub.car_category.trim().is_empty() {
            "Rally".to_string()
        } else {
            sub.car_category.trim().to_string()
        };
        let total_ms = sub.run.total_ms();
        let digest = compute_entry_digest(
            &pilot,
            &sub.run.key,
            total_ms,
            sub.run.driving_ms,
            sub.run.penalty_ms,
            sub.run.false_start,
            &sub.run.splits_ms,
            sub.sim_steps,
            sub.stage_length_m,
            platform,
            now_s,
        );
        let entry_id = format!("lb1-{}", &digest[..20]);

        let entry = LeaderboardEntry {
            entry_id: entry_id.clone(),
            pilot_name: pilot,
            key: sub.run.key.clone(),
            car_name: sub.run.car_name.clone(),
            car_category: category,
            stage_name: sub.run.stage_name.clone(),
            total_ms,
            driving_ms: sub.run.driving_ms,
            penalty_ms: sub.run.penalty_ms,
            false_start: sub.run.false_start,
            splits_ms: sub.run.splits_ms.clone(),
            splits_passed: sub.run.splits_passed,
            splits_total: sub.run.splits_total,
            sim_steps: sub.sim_steps,
            stage_length_m: sub.stage_length_m,
            platform: platform.into(),
            recorded_unix_s: now_s,
            verification: VerificationStatus::LocalOnly,
            integrity_sha256: digest,
        };

        if !self.entries.iter().any(|e| e.entry_id == entry_id) {
            self.entries.push(entry);
            self.sort_and_trim();
        }

        let all_stage = self.stage_entries(&sub.run.key.stage_id, None);
        let stage_rank = all_stage
            .iter()
            .position(|e| e.entry_id == entry_id)
            .map(|idx| idx + 1);
        let car_stage = self.stage_entries(&sub.run.key.stage_id, Some(&sub.run.key.car_id));
        let car_rank = car_stage
            .iter()
            .position(|e| e.entry_id == entry_id)
            .map(|idx| idx + 1);
        let pb_ms = car_stage.first().map(|e| e.total_ms);

        LeaderboardSubmitResult {
            accepted: true,
            rejection_reason: None,
            stage_rank,
            stage_total_entries: all_stage.len(),
            car_rank,
            personal_best_ms: pb_ms,
            is_stage_record: stage_rank == Some(1),
        }
    }

    /// Devuelve la clasificación ordenada de menor a mayor tiempo para un tramo (opcionalmente filtrada por coche).
    pub fn stage_entries(&self, stage_id: &str, car_id: Option<&str>) -> Vec<&LeaderboardEntry> {
        let mut list: Vec<&LeaderboardEntry> = self
            .entries
            .iter()
            .filter(|e| e.key.stage_id == stage_id && car_id.is_none_or(|cid| e.key.car_id == cid))
            .collect();
        list.sort_by_key(|e| (e.total_ms, e.driving_ms, e.recorded_unix_s));
        list
    }

    pub fn best_for_stage_and_car(
        &self,
        stage_id: &str,
        car_id: Option<&str>,
    ) -> Option<&LeaderboardEntry> {
        self.stage_entries(stage_id, car_id).into_iter().next()
    }

    /// Devuelve el mejor tiempo personal en cada tramo distinto que tenga tiempos registrados.
    pub fn personal_bests_by_stage(&self) -> Vec<&LeaderboardEntry> {
        let mut best_per_stage: std::collections::BTreeMap<&str, &LeaderboardEntry> =
            std::collections::BTreeMap::new();
        for entry in &self.entries {
            best_per_stage
                .entry(entry.key.stage_id.as_str())
                .and_modify(|cur| {
                    if (entry.total_ms, entry.driving_ms) < (cur.total_ms, cur.driving_ms) {
                        *cur = entry;
                    }
                })
                .or_insert(entry);
        }
        let mut out: Vec<&LeaderboardEntry> = best_per_stage.into_values().collect();
        out.sort_by(|a, b| {
            a.stage_name
                .cmp(&b.stage_name)
                .then_with(|| a.total_ms.cmp(&b.total_ms))
        });
        out
    }

    /// Construye los sobres versionados pendientes de sincronización futura con un servidor global.
    pub fn pending_online_submissions(&self) -> Vec<OnlineSubmissionPayload> {
        self.entries
            .iter()
            .filter(|e| {
                matches!(e.verification, VerificationStatus::LocalOnly) && e.verify_integrity()
            })
            .map(|e| OnlineSubmissionPayload {
                payload_version: ONLINE_PAYLOAD_SCHEMA_VERSION,
                entry_id: e.entry_id.clone(),
                canonical_stage_id: e.canonical_stage_id(),
                canonical_vehicle_id: e.canonical_vehicle_id(),
                rules_version: e.key.rules_version.clone(),
                pilot_name: e.pilot_name.clone(),
                platform: e.platform.clone(),
                total_ms: e.total_ms,
                driving_ms: e.driving_ms,
                penalty_ms: e.penalty_ms,
                false_start: e.false_start,
                splits_ms: e.splits_ms.clone(),
                sim_steps: e.sim_steps,
                stage_length_m: e.stage_length_m,
                recorded_unix_s: e.recorded_unix_s,
                client_integrity_sha256: e.integrity_sha256.clone(),
            })
            .collect()
    }

    fn sort_and_trim(&mut self) {
        self.entries.sort_by(|a, b| {
            a.key
                .stage_id
                .cmp(&b.key.stage_id)
                .then_with(|| a.total_ms.cmp(&b.total_ms))
                .then_with(|| a.driving_ms.cmp(&b.driving_ms))
                .then_with(|| a.recorded_unix_s.cmp(&b.recorded_unix_s))
        });

        let mut kept = Vec::with_capacity(self.entries.len());
        let mut count_in_stage = 0usize;
        let mut current_stage = String::new();
        let mut best_car_seen: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();

        for entry in self.entries.drain(..) {
            if entry.key.stage_id != current_stage {
                current_stage.clone_from(&entry.key.stage_id);
                count_in_stage = 0;
            }
            let car_combo = (
                entry.key.stage_id.clone(),
                entry.key.car_id.clone(),
                entry.key.physics_version.clone(),
            );
            let is_first_for_car = best_car_seen.insert(car_combo);
            if count_in_stage < MAX_ENTRIES_PER_STAGE || is_first_for_car {
                count_in_stage += 1;
                kept.push(entry);
            }
        }
        self.entries = kept;
    }

    pub fn to_json(&self) -> String {
        let mut out = format!(
            "{{\n  \"schema_version\": {},\n  \"entries\": [\n",
            LEADERBOARD_SCHEMA_VERSION
        );
        for (idx, e) in self.entries.iter().enumerate() {
            let comma = if idx + 1 == self.entries.len() {
                ""
            } else {
                ","
            };
            let splits = e
                .splits_ms
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let (ver_code, srv_id, srv_ts) = match &e.verification {
                VerificationStatus::LocalOnly => ("local", "", 0u64),
                VerificationStatus::ServerVerified {
                    server_record_id,
                    verified_unix_s,
                } => (
                    "server_verified",
                    server_record_id.as_str(),
                    *verified_unix_s,
                ),
            };
            out.push_str(&format!(
                "    {{\"entry_id\":\"{}\",\"pilot_name\":\"{}\",\"car_id\":\"{}\",\"car_name\":\"{}\",\"car_category\":\"{}\",\"stage_id\":\"{}\",\"stage_name\":\"{}\",\"stage_variant\":\"{}\",\"surface\":\"{}\",\"physics_version\":\"{}\",\"rules_version\":\"{}\",\"total_ms\":{},\"driving_ms\":{},\"penalty_ms\":{},\"false_start\":{},\"splits_ms\":[{}],\"splits_passed\":{},\"splits_total\":{},\"sim_steps\":{},\"stage_length_m\":{},\"platform\":\"{}\",\"recorded_unix_s\":{},\"verification\":\"{}\",\"server_record_id\":\"{}\",\"verified_unix_s\":{},\"integrity_sha256\":\"{}\"}}{}\n",
                json_escape(&e.entry_id),
                json_escape(&e.pilot_name),
                json_escape(&e.key.car_id),
                json_escape(&e.car_name),
                json_escape(&e.car_category),
                json_escape(&e.key.stage_id),
                json_escape(&e.stage_name),
                json_escape(&e.key.stage_variant),
                json_escape(&e.key.surface),
                json_escape(&e.key.physics_version),
                json_escape(&e.key.rules_version),
                e.total_ms,
                e.driving_ms,
                e.penalty_ms,
                e.false_start,
                splits,
                e.splits_passed,
                e.splits_total,
                e.sim_steps,
                e.stage_length_m,
                json_escape(&e.platform),
                e.recorded_unix_s,
                ver_code,
                json_escape(srv_id),
                srv_ts,
                json_escape(&e.integrity_sha256),
                comma
            ));
        }
        out.push_str("  ]\n}\n");
        out
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let ver = extract_u64(text, "schema_version").unwrap_or(1) as u32;
        if ver == 0 || ver > LEADERBOARD_SCHEMA_VERSION {
            return Err(format!("unsupported leaderboard schema_version {ver}"));
        }
        let mut entries = Vec::new();
        let mut quarantined = 0usize;
        if let Some(arr_start) = text.find("\"entries\"") {
            let rest = &text[arr_start..];
            if let Some(open_br) = rest.find('[') {
                let mut slice = &rest[open_br + 1..];
                while let Some(obj_start) = slice.find('{') {
                    let after_open = &slice[obj_start + 1..];
                    let Some(obj_end) = find_object_end(after_open) else {
                        break;
                    };
                    let obj = &after_open[..obj_end];
                    if let Some(entry) = parse_entry_obj(obj) {
                        if entry.verify_integrity() {
                            if !entries
                                .iter()
                                .any(|x: &LeaderboardEntry| x.entry_id == entry.entry_id)
                            {
                                entries.push(entry);
                            }
                        } else {
                            quarantined += 1;
                        }
                    }
                    slice = &after_open[obj_end + 1..];
                }
            }
        }
        let mut store = Self {
            entries,
            quarantined_tampered: quarantined,
        };
        store.sort_and_trim();
        Ok(store)
    }
}

pub fn validate_leaderboard_submission(sub: &LeaderboardSubmission) -> Result<(), &'static str> {
    sub.run.validate()?;
    if sub.stage_length_m >= 500 {
        let secs = sub.run.driving_ms as f64 / 1000.0;
        if secs <= 0.0 || (sub.stage_length_m as f64 / secs) > MAX_PLAUSIBLE_AVG_SPEED_MPS {
            return Err("IMPLAUSIBLE STAGE SPEED");
        }
    }
    if sub.sim_steps > 0 {
        let sim_ms = sub.sim_steps.saturating_mul(1000) / 720;
        if sim_ms.saturating_mul(2) < sub.run.driving_ms {
            return Err("SIMULATION STEP COUNT MISMATCH");
        }
    }
    Ok(())
}

pub fn sanitize_leaderboard_pilot(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .take(18)
        .collect();
    if cleaned.is_empty() {
        "PILOTO RBR".into()
    } else {
        cleaned.to_uppercase()
    }
}

pub fn current_platform_tag() -> &'static str {
    if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn normalize_platform_tag(raw: &str) -> &'static str {
    if raw.eq_ignore_ascii_case("ios") || raw.eq_ignore_ascii_case("ipados") {
        "ios"
    } else if raw.eq_ignore_ascii_case("android") {
        "android"
    } else if raw.eq_ignore_ascii_case("macos") {
        "macos"
    } else if raw.eq_ignore_ascii_case("windows") {
        "windows"
    } else {
        "linux"
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_entry_digest(
    pilot_name: &str,
    key: &RecordKey,
    total_ms: u64,
    driving_ms: u64,
    penalty_ms: u64,
    false_start: bool,
    splits_ms: &[u64],
    sim_steps: u64,
    stage_length_m: u32,
    platform: &str,
    recorded_unix_s: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rerbr-leaderboard-v1|");
    hasher.update(pilot_name.as_bytes());
    hasher.update(b"|");
    hasher.update(key.canonical_id().as_bytes());
    hasher.update(b"|");
    hasher.update(total_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(driving_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(penalty_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(if false_start { b"1" } else { b"0" });
    hasher.update(b"|");
    for (idx, &sp) in splits_ms.iter().enumerate() {
        if idx > 0 {
            hasher.update(b",");
        }
        hasher.update(sp.to_string().as_bytes());
    }
    hasher.update(b"|");
    hasher.update(sim_steps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(stage_length_m.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(platform.as_bytes());
    hasher.update(b"|");
    hasher.update(recorded_unix_s.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn build_migrated_entry(pb: &PersonalBest) -> LeaderboardEntry {
    let platform = normalize_platform_tag(&pb.platform);
    let pilot = "PILOTO RBR".to_string();
    let digest = compute_entry_digest(
        &pilot,
        &pb.key,
        pb.best_total_ms,
        pb.driving_ms,
        pb.penalty_ms,
        pb.false_start,
        &pb.splits_ms,
        0,
        0,
        platform,
        pb.recorded_unix_s,
    );
    LeaderboardEntry {
        entry_id: format!("lb1-{}", &digest[..20]),
        pilot_name: pilot,
        key: pb.key.clone(),
        car_name: pb.car_name.clone(),
        car_category: "Rally".into(),
        stage_name: pb.stage_name.clone(),
        total_ms: pb.best_total_ms,
        driving_ms: pb.driving_ms,
        penalty_ms: pb.penalty_ms,
        false_start: pb.false_start,
        splits_ms: pb.splits_ms.clone(),
        splits_passed: pb.splits_passed,
        splits_total: pb.splits_total,
        sim_steps: 0,
        stage_length_m: 0,
        platform: platform.into(),
        recorded_unix_s: pb.recorded_unix_s,
        verification: VerificationStatus::LocalOnly,
        integrity_sha256: digest,
    }
}

pub fn format_leaderboard_row(rank: usize, entry: &LeaderboardEntry, leader_ms: u64) -> String {
    let time_str = records::format_time_ms(entry.total_ms);
    let delta_str = if rank == 1 || entry.total_ms <= leader_ms {
        "LÍDER".to_string()
    } else {
        format!(
            "+{}",
            records::format_delta_ms((entry.total_ms - leader_ms) as i64).trim_start_matches('+')
        )
    };
    let pen = if entry.penalty_ms > 0 {
        format!(" [+{}s]", entry.penalty_ms / 1000)
    } else {
        String::new()
    };
    format!(
        "#{rank} {time_str} ({delta_str}){pen} · {} · {} [{}·{}]",
        entry.car_name,
        entry.pilot_name,
        entry.physics_badge(),
        entry.verification.ui_badge()
    )
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

fn find_object_end(s: &str) -> Option<usize> {
    let mut in_str = false;
    let mut esc = false;
    for (idx, b) in s.bytes().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
        } else if b == b'"' {
            in_str = true;
        } else if b == b'}' {
            return Some(idx);
        }
    }
    None
}

fn parse_entry_obj(obj: &str) -> Option<LeaderboardEntry> {
    let entry_id = extract_str(obj, "entry_id")?;
    let pilot_name = extract_str(obj, "pilot_name").unwrap_or_else(|| "PILOTO RBR".into());
    let car_id = extract_str(obj, "car_id")?;
    let car_name = extract_str(obj, "car_name")?;
    let car_category = extract_str(obj, "car_category").unwrap_or_else(|| "Rally".into());
    let stage_id = extract_str(obj, "stage_id")?;
    let stage_name = extract_str(obj, "stage_name")?;
    let stage_variant = extract_str(obj, "stage_variant")?;
    let surface = extract_str(obj, "surface")?;
    let physics_version = extract_str(obj, "physics_version")?;
    let rules_version = extract_str(obj, "rules_version")?;
    let total_ms = extract_u64(obj, "total_ms")?;
    let driving_ms = extract_u64(obj, "driving_ms")?;
    let penalty_ms = extract_u64(obj, "penalty_ms").unwrap_or(0);
    let false_start = extract_bool(obj, "false_start").unwrap_or(false);
    let splits_ms = extract_u64_array(obj, "splits_ms");
    let splits_passed =
        extract_u64(obj, "splits_passed").unwrap_or(splits_ms.len() as u64) as usize;
    let splits_total = extract_u64(obj, "splits_total").unwrap_or(splits_ms.len() as u64) as usize;
    let sim_steps = extract_u64(obj, "sim_steps").unwrap_or(0);
    let stage_length_m = extract_u64(obj, "stage_length_m").unwrap_or(0) as u32;
    let platform = extract_str(obj, "platform").unwrap_or_else(|| "unknown".into());
    let recorded_unix_s = extract_u64(obj, "recorded_unix_s").unwrap_or(0);
    let ver_str = extract_str(obj, "verification").unwrap_or_else(|| "local".into());
    let verification = if ver_str == "server_verified" {
        let sid = extract_str(obj, "server_record_id").unwrap_or_default();
        let vts = extract_u64(obj, "verified_unix_s").unwrap_or(0);
        if sid.is_empty() {
            VerificationStatus::LocalOnly
        } else {
            VerificationStatus::ServerVerified {
                server_record_id: sid,
                verified_unix_s: vts,
            }
        }
    } else {
        VerificationStatus::LocalOnly
    };
    let integrity_sha256 = extract_str(obj, "integrity_sha256")?;

    Some(LeaderboardEntry {
        entry_id,
        pilot_name,
        key: RecordKey {
            car_id,
            stage_id,
            stage_variant,
            surface,
            physics_version,
            rules_version,
        },
        car_name,
        car_category,
        stage_name,
        total_ms,
        driving_ms,
        penalty_ms,
        false_start,
        splits_ms,
        splits_passed,
        splits_total,
        sim_steps,
        stage_length_m,
        platform,
        recorded_unix_s,
        verification,
        integrity_sha256,
    })
}

fn find_key_value<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let pos = s.find(&needle)?;
    let after = &s[pos + needle.len()..];
    let colon = after.find(':')?;
    Some(after[colon + 1..].trim_start())
}

fn extract_str(s: &str, key: &str) -> Option<String> {
    let rest = find_key_value(s, key)?;
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut esc = false;
    for ch in rest[1..].chars() {
        if esc {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

fn extract_u64(s: &str, key: &str) -> Option<u64> {
    let rest = find_key_value(s, key)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
}

fn extract_bool(s: &str, key: &str) -> Option<bool> {
    let rest = find_key_value(s, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_u64_array(s: &str, key: &str) -> Vec<u64> {
    let Some(rest) = find_key_value(s, key) else {
        return Vec::new();
    };
    let Some(stripped) = rest.strip_prefix('[') else {
        return Vec::new();
    };
    let Some(end) = stripped.find(']') else {
        return Vec::new();
    };
    stripped[..end]
        .split(',')
        .filter_map(|part| part.trim().parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rerbr_content::{Selection, StageDescriptor, VehicleDescriptor};

    fn make_sub(
        car: VehicleDescriptor,
        stage: StageDescriptor,
        driving_ms: u64,
        penalty_ms: u64,
        pilot: &str,
    ) -> LeaderboardSubmission {
        let sel = Selection {
            vehicle: car,
            stage,
        };
        let sim_steps = (driving_ms * 720) / 1000;
        LeaderboardSubmission {
            run: RunSubmission {
                key: RecordKey::from_selection(
                    &sel,
                    "rsf-ngp-7.4.778:Ford_Focus_Mk_II_RS_WRC_2006_ngp6",
                    true,
                ),
                car_name: sel.vehicle.name.clone(),
                stage_name: sel.stage.name.clone(),
                driving_ms,
                penalty_ms,
                false_start: penalty_ms > 0,
                splits_ms: vec![driving_ms / 3, (driving_ms * 2) / 3],
                splits_passed: 2,
                splits_total: 2,
                finished: true,
                finish_crossed_forward: true,
                visual_only: false,
                is_school: false,
            },
            pilot_name: pilot.into(),
            car_category: "WRC 2.0L".into(),
            sim_steps,
            stage_length_m: 4800,
        }
    }

    #[test]
    fn leaderboard_sorts_ascending_handles_penalties_filters_and_persists_across_sessions() {
        let dir = std::env::temp_dir().join(format!("rerbr-lb-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = LeaderboardStore::path_in(&dir);

        let mut store = LeaderboardStore::new();
        let focus = VehicleDescriptor::ford_focus_2006();
        let corolla = VehicleDescriptor::toyota_corolla_wrc();
        let harwood = StageDescriptor::harwood_forest();

        // 1ª tanda: 03:30.000
        let r1 = store.submit(&make_sub(
            focus.clone(),
            harwood.clone(),
            210_000,
            0,
            "piloto a",
        ));
        assert!(r1.accepted);
        assert_eq!(r1.stage_rank, Some(1));
        assert!(r1.is_stage_record);

        // 2ª tanda con Corolla: más rápida en conducción (204s) pero +10s salida en falso = 214s -> puesto #2
        let r2 = store.submit(&make_sub(
            corolla.clone(),
            harwood.clone(),
            204_000,
            10_000,
            "piloto b",
        ));
        assert!(r2.accepted);
        assert_eq!(r2.stage_rank, Some(2));
        assert_eq!(r2.car_rank, Some(1));

        // 3ª tanda con Focus: 03:18.500 -> nuevo #1 absoluto del tramo
        let r3 = store.submit(&make_sub(
            focus.clone(),
            harwood.clone(),
            198_500,
            0,
            "piloto a",
        ));
        assert!(r3.accepted);
        assert_eq!(r3.stage_rank, Some(1));
        assert_eq!(r3.stage_total_entries, 3);
        assert!(r3.is_stage_record);

        // Verificar orden menor a mayor tiempo total
        let ranked = store.stage_entries(&harwood.id, None);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].total_ms, 198_500);
        assert_eq!(ranked[1].total_ms, 210_000);
        assert_eq!(ranked[2].total_ms, 214_000);
        assert!(ranked[2].false_start);
        assert_eq!(ranked[0].verification, VerificationStatus::LocalOnly);

        // Filtrado por coche actual
        let corolla_only = store.stage_entries(&harwood.id, Some(&corolla.id));
        assert_eq!(corolla_only.len(), 1);
        assert_eq!(corolla_only[0].total_ms, 214_000);

        // Persistencia atómica y recarga íntegra
        store.save_to_path(&path).unwrap();
        let reloaded = LeaderboardStore::load_from_path(&path).unwrap();
        assert_eq!(reloaded.len(), 3);
        assert_eq!(reloaded.quarantined_tampered_count(), 0);
        assert_eq!(
            reloaded.stage_entries(&harwood.id, None)[0].total_ms,
            198_500
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn incomplete_invalid_and_tampered_entries_are_strictly_rejected_or_quarantined() {
        let mut store = LeaderboardStore::new();
        let focus = VehicleDescriptor::ford_focus_2006();
        let harwood = StageDescriptor::harwood_forest();

        // Sesión incompleta -> rechazada
        let mut incomplete = make_sub(focus.clone(), harwood.clone(), 200_000, 0, "test");
        incomplete.run.finished = false;
        assert!(!store.submit(&incomplete).accepted);

        //Velocidad media imposible (4.8 km en 10 segundos = 1728 km/h) -> rechazada
        let impossible_speed = make_sub(focus.clone(), harwood.clone(), 10_000, 0, "cheat");
        let rej = store.submit(&impossible_speed);
        assert!(!rej.accepted);
        assert_eq!(rej.rejection_reason, Some("IMPLAUSIBLE STAGE SPEED"));

        // Desajuste entre pasos de simulación y reloj -> rechazado
        let mut bad_steps = make_sub(focus.clone(), harwood.clone(), 200_000, 0, "cheat");
        bad_steps.sim_steps = 1000; // ~1.3s simulados para un tiempo de 200s
        let rej2 = store.submit(&bad_steps);
        assert!(!rej2.accepted);
        assert_eq!(
            rej2.rejection_reason,
            Some("SIMULATION STEP COUNT MISMATCH")
        );

        // Insertar un tiempo legítimo y luego manipular manualmente el JSON en disco
        assert!(
            store
                .submit(&make_sub(focus, harwood, 205_000, 0, "honesto"))
                .accepted
        );
        let legit_json = store.to_json();
        let tampered_json = legit_json.replace("\"total_ms\":205000", "\"total_ms\":120000");
        let after_tamper = LeaderboardStore::from_json(&tampered_json).unwrap();
        assert_eq!(
            after_tamper.len(),
            0,
            "El registro manipulado debe quedar fuera del ranking válido"
        );
        assert_eq!(after_tamper.quarantined_tampered_count(), 1);
    }

    #[test]
    fn cross_platform_android_ios_records_share_canonical_ids_and_migrate_legacy_pbs() {
        let mut pb_store = RecordStore::new();
        let sel = Selection {
            vehicle: VehicleDescriptor::toyota_corolla_wrc(),
            stage: StageDescriptor::chirdonhead(),
        };
        let run = RunSubmission {
            key: RecordKey::from_selection(&sel, "rbr-retail-1.02", true),
            car_name: sel.vehicle.name.clone(),
            stage_name: sel.stage.name.clone(),
            driving_ms: 195_320,
            penalty_ms: 0,
            false_start: false,
            splits_ms: vec![65_000, 130_000],
            splits_passed: 2,
            splits_total: 2,
            finished: true,
            finish_crossed_forward: true,
            visual_only: false,
            is_school: false,
        };
        assert!(pb_store.submit(&run).valid);

        let mut lb = LeaderboardStore::new();
        assert_eq!(lb.migrate_from_personal_bests(&pb_store), 1);
        assert_eq!(lb.migrate_from_personal_bests(&pb_store), 0); // idempotente

        let pending = lb.pending_online_submissions();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].payload_version, ONLINE_PAYLOAD_SCHEMA_VERSION);
        assert_eq!(
            pending[0].canonical_vehicle_id,
            "toyota-corolla-wrc@rbr-retail-1.02"
        );
        assert!(pending[0].canonical_stage_id.starts_with("chirdonhead|"));
    }
}
