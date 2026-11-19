//! Persistent personal best (PB) stage records with atomic storage.
//!
//! Records are strictly keyed by:
//! `(car_id, stage_id, stage_variant, surface, physics_version, rules_version)`
//! so times from different physics backends, stage variants, surfaces, or assist
//! rules are never mixed. All timings are stored in integer milliseconds derived
//! from deterministic 720 Hz simulation time (never render FPS) and designed for
//! direct compatibility with future Android builds and online leaderboards.

use rerbr_content::{Selection, StageDescriptor, VehicleDescriptor};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const RECORDS_SCHEMA_VERSION: u32 = 1;
pub const RULES_VERSION_PREFIX: &str = "rules-v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordKey {
    pub car_id: String,
    pub stage_id: String,
    pub stage_variant: String,
    pub surface: String,
    pub physics_version: String,
    pub rules_version: String,
}

impl RecordKey {
    pub fn from_selection(
        selection: &Selection,
        physics_version: impl Into<String>,
        clutch_help: bool,
    ) -> Self {
        Self::from_parts(
            &selection.vehicle,
            &selection.stage,
            physics_version,
            clutch_help,
        )
    }

    pub fn from_parts(
        vehicle: &VehicleDescriptor,
        stage: &StageDescriptor,
        physics_version: impl Into<String>,
        clutch_help: bool,
    ) -> Self {
        let stage_variant = format!(
            "{}:w{}:wet{}:wear{}",
            stage.stem, stage.weather_letter, stage.wetness, stage.wear
        );
        let rules_version = format!(
            "{RULES_VERSION_PREFIX}:manual:clutch_help={}",
            u8::from(clutch_help)
        );
        Self {
            car_id: vehicle.id.clone(),
            stage_id: stage.id.clone(),
            stage_variant,
            surface: stage.surface().to_string(),
            physics_version: physics_version.into(),
            rules_version,
        }
    }

    pub fn canonical_id(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.car_id,
            self.stage_id,
            self.stage_variant,
            self.surface,
            self.physics_version,
            self.rules_version
        )
    }
}

/// Computes the honest physics backend version identifier for record separation.
pub fn physics_version_for(vehicle: &VehicleDescriptor, ngp_runtime_label: Option<&str>) -> String {
    if vehicle.is_ngp_experimental_car() {
        let rt = ngp_runtime_label
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("ngp-7.4.778");
        let pack = vehicle.rsf_pack.as_deref().unwrap_or("pack");
        format!("rsf-{rt}:{pack}")
    } else {
        "rbr-retail-1.02".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalBest {
    pub key: RecordKey,
    pub car_name: String,
    pub stage_name: String,
    pub best_total_ms: u64,
    pub driving_ms: u64,
    pub penalty_ms: u64,
    pub false_start: bool,
    pub splits_ms: Vec<u64>,
    pub splits_passed: usize,
    pub splits_total: usize,
    pub attempts: u32,
    pub platform: String,
    pub recorded_unix_s: u64,
}

/// Candidate finished run submitted for validation and PB update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSubmission {
    pub key: RecordKey,
    pub car_name: String,
    pub stage_name: String,
    pub driving_ms: u64,
    pub penalty_ms: u64,
    pub false_start: bool,
    pub splits_ms: Vec<u64>,
    pub splits_passed: usize,
    pub splits_total: usize,
    pub finished: bool,
    pub finish_crossed_forward: bool,
    pub visual_only: bool,
    pub is_school: bool,
}

impl RunSubmission {
    pub fn total_ms(&self) -> u64 {
        self.driving_ms.saturating_add(self.penalty_ms)
    }

    /// Strict validity gate: abandoned, incomplete, visual-only, school, or
    /// checkpoint-omitted runs are never accepted as personal bests.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.is_school {
            return Err("RALLY SCHOOL RUN EXCLUDED FROM STAGE RECORDS");
        }
        if self.visual_only {
            return Err("VISUAL-ONLY RUN WITHOUT PHYSICS");
        }
        if !self.finished || !self.finish_crossed_forward {
            return Err("INCOMPLETE OR ABANDONED RUN");
        }
        if self.splits_passed < self.splits_total {
            return Err("CHECKPOINTS OMITTED");
        }
        if self.driving_ms == 0 || self.total_ms() == 0 {
            return Err("ZERO SIMULATION TIME");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOutcome {
    pub valid: bool,
    pub rejection_reason: Option<&'static str>,
    pub is_new_best: bool,
    pub previous_best_ms: Option<u64>,
    pub current_best_ms: Option<u64>,
    /// Signed delta (`run_total_ms - previous_best_ms`). Negative means faster than previous PB.
    pub delta_ms: Option<i64>,
}

impl RecordOutcome {
    pub fn summary_line(&self) -> String {
        if !self.valid {
            return format!(
                "SIN RÉCORD ({})",
                self.rejection_reason.unwrap_or("INVÁLIDO")
            );
        }
        if self.is_new_best {
            match self.delta_ms {
                Some(delta) => format!("NUEVO RÉCORD PERSONAL ({})", format_delta_ms(delta)),
                None => "PRIMER RÉCORD PERSONAL GUARDADO".into(),
            }
        } else {
            match (self.current_best_ms, self.delta_ms) {
                (Some(pb), Some(delta)) => {
                    format!(
                        "RÉCORD: {} ({})",
                        format_time_ms(pb),
                        format_delta_ms(delta)
                    )
                }
                (Some(pb), None) => format!("RÉCORD: {}", format_time_ms(pb)),
                _ => "CARRERA VÁLIDA".into(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordStore {
    pub schema_version: u32,
    records: BTreeMap<RecordKey, PersonalBest>,
}

impl Default for RecordStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordStore {
    pub fn new() -> Self {
        Self {
            schema_version: RECORDS_SCHEMA_VERSION,
            records: BTreeMap::new(),
        }
    }

    pub fn path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(Self::path_in(&root))
    }

    pub fn path_in(documents_root: &Path) -> PathBuf {
        documents_root.join("RBR").join("personal_bests.json")
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| Self::load_from_path(&p).ok())
            .unwrap_or_default()
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json(&text)
    }

    /// Atomic save: write to temp sibling, fsync, then rename over target so crashes
    /// or app updates never leave a truncated or corrupted records file.
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp_name = format!(
            ".{}.tmp.{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("personal_bests.json"),
            std::process::id()
        );
        let tmp_path = path.with_file_name(tmp_name);
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
            file.write_all(self.to_json().as_bytes())
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        match std::fs::rename(&tmp_path, path) {
            Ok(()) => Ok(()),
            Err(_) => {
                // On Windows, rename fails if target already exists; remove + rename atomically.
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

    pub fn get(&self, key: &RecordKey) -> Option<&PersonalBest> {
        self.records.get(key)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn all(&self) -> impl Iterator<Item = &PersonalBest> {
        self.records.values()
    }

    /// Validates and records a finished run. Only replaces the stored PB if the run is
    /// strictly valid and faster (`total_ms < existing.best_total_ms`).
    pub fn submit(&mut self, run: &RunSubmission) -> RecordOutcome {
        if let Err(reason) = run.validate() {
            let existing = self.records.get(&run.key).map(|pb| pb.best_total_ms);
            return RecordOutcome {
                valid: false,
                rejection_reason: Some(reason),
                is_new_best: false,
                previous_best_ms: existing,
                current_best_ms: existing,
                delta_ms: None,
            };
        }

        let total_ms = run.total_ms();
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let platform = current_platform().to_string();

        if let Some(existing) = self.records.get_mut(&run.key) {
            let prev = existing.best_total_ms;
            let delta = (total_ms as i64) - (prev as i64);
            existing.attempts = existing.attempts.saturating_add(1);
            if total_ms < prev {
                existing.car_name = run.car_name.clone();
                existing.stage_name = run.stage_name.clone();
                existing.best_total_ms = total_ms;
                existing.driving_ms = run.driving_ms;
                existing.penalty_ms = run.penalty_ms;
                existing.false_start = run.false_start;
                existing.splits_ms = run.splits_ms.clone();
                existing.splits_passed = run.splits_passed;
                existing.splits_total = run.splits_total;
                existing.platform = platform;
                existing.recorded_unix_s = now_unix;
                RecordOutcome {
                    valid: true,
                    rejection_reason: None,
                    is_new_best: true,
                    previous_best_ms: Some(prev),
                    current_best_ms: Some(total_ms),
                    delta_ms: Some(delta),
                }
            } else {
                RecordOutcome {
                    valid: true,
                    rejection_reason: None,
                    is_new_best: false,
                    previous_best_ms: Some(prev),
                    current_best_ms: Some(prev),
                    delta_ms: Some(delta),
                }
            }
        } else {
            self.records.insert(
                run.key.clone(),
                PersonalBest {
                    key: run.key.clone(),
                    car_name: run.car_name.clone(),
                    stage_name: run.stage_name.clone(),
                    best_total_ms: total_ms,
                    driving_ms: run.driving_ms,
                    penalty_ms: run.penalty_ms,
                    false_start: run.false_start,
                    splits_ms: run.splits_ms.clone(),
                    splits_passed: run.splits_passed,
                    splits_total: run.splits_total,
                    attempts: 1,
                    platform,
                    recorded_unix_s: now_unix,
                },
            );
            RecordOutcome {
                valid: true,
                rejection_reason: None,
                is_new_best: true,
                previous_best_ms: None,
                current_best_ms: Some(total_ms),
                delta_ms: None,
            }
        }
    }

    pub fn to_json(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{{");
        let _ = writeln!(out, "  \"schema_version\": {},", RECORDS_SCHEMA_VERSION);
        let _ = writeln!(out, "  \"records\": [");
        let total = self.records.len();
        for (i, pb) in self.records.values().enumerate() {
            let splits: Vec<String> = pb.splits_ms.iter().map(|s| s.to_string()).collect();
            let _ = writeln!(
                out,
                "    {{\"car_id\":{},\"car_name\":{},\"stage_id\":{},\"stage_name\":{},\"stage_variant\":{},\"surface\":{},\"physics_version\":{},\"rules_version\":{},\"best_total_ms\":{},\"driving_ms\":{},\"penalty_ms\":{},\"false_start\":{},\"splits_ms\":[{}],\"splits_passed\":{},\"splits_total\":{},\"attempts\":{},\"platform\":{},\"recorded_unix_s\":{}}}{}",
                json_str(&pb.key.car_id),
                json_str(&pb.car_name),
                json_str(&pb.key.stage_id),
                json_str(&pb.stage_name),
                json_str(&pb.key.stage_variant),
                json_str(&pb.key.surface),
                json_str(&pb.key.physics_version),
                json_str(&pb.key.rules_version),
                pb.best_total_ms,
                pb.driving_ms,
                pb.penalty_ms,
                pb.false_start,
                splits.join(","),
                pb.splits_passed,
                pb.splits_total,
                pb.attempts,
                json_str(&pb.platform),
                pb.recorded_unix_s,
                if i + 1 < total { "," } else { "" }
            );
        }
        let _ = writeln!(out, "  ]");
        let _ = writeln!(out, "}}");
        out
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let schema_version = extract_u64(text, "schema_version").unwrap_or(0) as u32;
        if schema_version == 0 || schema_version > RECORDS_SCHEMA_VERSION {
            return Err(format!(
                "unsupported records schema_version {schema_version}"
            ));
        }
        let mut store = Self::new();
        let Some(arr_start) = text.find("\"records\"") else {
            return Ok(store);
        };
        let rest = &text[arr_start..];
        let Some(lb) = rest.find('[') else {
            return Ok(store);
        };
        let body = &rest[lb + 1..];
        for obj in split_json_objects(body) {
            let Some(car_id) = extract_str(obj, "car_id") else {
                continue;
            };
            let Some(stage_id) = extract_str(obj, "stage_id") else {
                continue;
            };
            let Some(stage_variant) = extract_str(obj, "stage_variant") else {
                continue;
            };
            let Some(surface) = extract_str(obj, "surface") else {
                continue;
            };
            let Some(physics_version) = extract_str(obj, "physics_version") else {
                continue;
            };
            let Some(rules_version) = extract_str(obj, "rules_version") else {
                continue;
            };
            let Some(best_total_ms) = extract_u64(obj, "best_total_ms") else {
                continue;
            };
            if best_total_ms == 0 {
                continue;
            }
            let key = RecordKey {
                car_id,
                stage_id,
                stage_variant,
                surface,
                physics_version,
                rules_version,
            };
            let pb = PersonalBest {
                car_name: extract_str(obj, "car_name").unwrap_or_else(|| key.car_id.clone()),
                stage_name: extract_str(obj, "stage_name").unwrap_or_else(|| key.stage_id.clone()),
                best_total_ms,
                driving_ms: extract_u64(obj, "driving_ms").unwrap_or(best_total_ms),
                penalty_ms: extract_u64(obj, "penalty_ms").unwrap_or(0),
                false_start: extract_bool(obj, "false_start").unwrap_or(false),
                splits_ms: extract_u64_array(obj, "splits_ms"),
                splits_passed: extract_u64(obj, "splits_passed").unwrap_or(0) as usize,
                splits_total: extract_u64(obj, "splits_total").unwrap_or(0) as usize,
                attempts: extract_u64(obj, "attempts").unwrap_or(1) as u32,
                platform: extract_str(obj, "platform").unwrap_or_else(|| "unknown".into()),
                recorded_unix_s: extract_u64(obj, "recorded_unix_s").unwrap_or(0),
                key: key.clone(),
            };
            store.records.insert(key, pb);
        }
        Ok(store)
    }
}

pub fn format_time_ms(ms: u64) -> String {
    let mins = ms / 60_000;
    let secs = (ms % 60_000) / 1000;
    let millis = ms % 1000;
    format!("{mins:02}:{secs:02}.{millis:03}")
}

pub fn format_delta_ms(delta_ms: i64) -> String {
    let sign = if delta_ms < 0 { "-" } else { "+" };
    let abs = delta_ms.unsigned_abs();
    let secs = abs / 1000;
    let millis = abs % 1000;
    if secs >= 60 {
        format!("{sign}{}:{:02}.{millis:03}", secs / 60, secs % 60)
    } else {
        format!("{sign}{secs}.{millis:03}s")
    }
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        "desktop"
    }
}

fn json_str(s: &str) -> String {
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

fn split_json_objects(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_str = false;
    let mut esc = false;
    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(st) = start.take() {
                            out.push(&s[st..=idx]);
                        }
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }
    out
}

fn find_key_value<'a>(obj: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let pos = obj.find(&needle)?;
    let rest = obj[pos + needle.len()..].trim_start();
    Some(rest.strip_prefix(':')?.trim_start())
}

fn extract_str(obj: &str, key: &str) -> Option<String> {
    let rest = find_key_value(obj, key)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            match ch {
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

fn extract_u64(obj: &str, key: &str) -> Option<u64> {
    let rest = find_key_value(obj, key)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn extract_bool(obj: &str, key: &str) -> Option<bool> {
    let rest = find_key_value(obj, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_u64_array(obj: &str, key: &str) -> Vec<u64> {
    let Some(rest) = find_key_value(obj, key).and_then(|r| r.strip_prefix('[')) else {
        return Vec::new();
    };
    let Some(end) = rest.find(']') else {
        return Vec::new();
    };
    rest[..end]
        .split(',')
        .filter_map(|part| part.trim().parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_submission(driving_ms: u64) -> RunSubmission {
        let sel = Selection {
            vehicle: VehicleDescriptor::toyota_corolla_wrc(),
            stage: StageDescriptor::harwood_forest(),
        };
        RunSubmission {
            key: RecordKey::from_selection(&sel, "rbr-retail-1.02", true),
            car_name: sel.vehicle.name,
            stage_name: sel.stage.name,
            driving_ms,
            penalty_ms: 0,
            false_start: false,
            splits_ms: vec![driving_ms / 3, (driving_ms * 2) / 3],
            splits_passed: 2,
            splits_total: 2,
            finished: true,
            finish_crossed_forward: true,
            visual_only: false,
            is_school: false,
        }
    }

    #[test]
    fn records_persist_atomically_across_reload() {
        let dir = std::env::temp_dir().join(format!("rerbr-pb-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = RecordStore::path_in(&dir);

        let mut store = RecordStore::new();
        let run = sample_submission(215_430);
        let res = store.submit(&run);
        assert!(res.valid);
        assert!(res.is_new_best);
        assert_eq!(res.current_best_ms, Some(215_430));
        assert_eq!(res.delta_ms, None);

        store.save_to_path(&path).unwrap();
        assert!(path.is_file());

        let reloaded = RecordStore::load_from_path(&path).unwrap();
        assert_eq!(reloaded.len(), 1);
        let pb = reloaded.get(&run.key).expect("PB reloaded");
        assert_eq!(pb.best_total_ms, 215_430);
        assert_eq!(pb.splits_ms, vec![71_810, 143_620]);
        assert_eq!(pb.attempts, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn faster_run_improves_pb_and_slower_run_keeps_existing_pb() {
        let mut store = RecordStore::new();
        let run1 = sample_submission(220_000);
        let o1 = store.submit(&run1);
        assert!(o1.is_new_best);

        // Slower run: valid, positive delta, does NOT overwrite PB, increments attempts.
        let run2 = sample_submission(223_500);
        let o2 = store.submit(&run2);
        assert!(o2.valid);
        assert!(!o2.is_new_best);
        assert_eq!(o2.previous_best_ms, Some(220_000));
        assert_eq!(o2.current_best_ms, Some(220_000));
        assert_eq!(o2.delta_ms, Some(3_500));
        assert_eq!(store.get(&run1.key).unwrap().attempts, 2);

        // Faster run: replaces PB with negative delta.
        let run3 = sample_submission(218_125);
        let o3 = store.submit(&run3);
        assert!(o3.valid);
        assert!(o3.is_new_best);
        assert_eq!(o3.previous_best_ms, Some(220_000));
        assert_eq!(o3.current_best_ms, Some(218_125));
        assert_eq!(o3.delta_ms, Some(-1_875));
        assert_eq!(store.get(&run1.key).unwrap().best_total_ms, 218_125);
        assert_eq!(store.get(&run1.key).unwrap().attempts, 3);
    }

    #[test]
    fn invalid_abandoned_omitted_checkpoint_or_visual_runs_are_rejected() {
        let mut store = RecordStore::new();

        let mut abandoned = sample_submission(180_000);
        abandoned.finished = false;
        assert!(!store.submit(&abandoned).valid);

        let mut reverse_finish = sample_submission(180_000);
        reverse_finish.finish_crossed_forward = false;
        assert!(!store.submit(&reverse_finish).valid);

        let mut missed_split = sample_submission(180_000);
        missed_split.splits_passed = 1;
        missed_split.splits_total = 2;
        assert!(!store.submit(&missed_split).valid);

        let mut visual_only = sample_submission(180_000);
        visual_only.visual_only = true;
        assert!(!store.submit(&visual_only).valid);

        let mut school = sample_submission(180_000);
        school.is_school = true;
        assert!(!store.submit(&school).valid);

        let zero = sample_submission(0);
        assert!(!store.submit(&zero).valid);

        assert!(store.is_empty());
    }

    #[test]
    fn physics_rules_surface_and_variant_versions_never_mix_pbs() {
        let mut store = RecordStore::new();
        let base = sample_submission(200_000);
        assert!(store.submit(&base).is_new_best);

        let mut ngp_run = sample_submission(190_000);
        ngp_run.key.physics_version = "rsf-ngp-7.4.778:Toyota_GR_Yaris_Rally2_ngp7".into();
        assert!(store.submit(&ngp_run).is_new_best);

        let mut rules_run = sample_submission(195_000);
        rules_run.key.rules_version = "rules-v1:manual:clutch_help=0".into();
        assert!(store.submit(&rules_run).is_new_best);

        let mut variant_run = sample_submission(192_000);
        variant_run.key.stage_variant = "track-20_N:w1:wet0:wear1".into();
        assert!(store.submit(&variant_run).is_new_best);

        assert_eq!(store.len(), 4);
        assert_eq!(store.get(&base.key).unwrap().best_total_ms, 200_000);
        assert_eq!(store.get(&ngp_run.key).unwrap().best_total_ms, 190_000);
        assert_eq!(store.get(&rules_run.key).unwrap().best_total_ms, 195_000);
        assert_eq!(store.get(&variant_run.key).unwrap().best_total_ms, 192_000);
    }

    #[test]
    fn false_start_penalty_is_included_in_total_time() {
        let mut store = RecordStore::new();
        let mut run = sample_submission(200_000);
        run.false_start = true;
        run.penalty_ms = 10_000;
        let out = store.submit(&run);
        assert!(out.valid);
        assert_eq!(out.current_best_ms, Some(210_000));
        assert_eq!(format_time_ms(210_000), "03:30.000");
        assert_eq!(format_delta_ms(-1_250), "-1.250s");
        assert_eq!(format_delta_ms(65_020), "+1:05.020");
    }
}
