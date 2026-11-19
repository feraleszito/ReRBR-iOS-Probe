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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const RECORDS_SCHEMA_VERSION: u32 = 1;
pub const RULES_VERSION_PREFIX: &str = "rules-v1";
pub const SPLIT_BANNER_DURATION_SECS: f64 = 4.0;

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
        if self.splits_total > 0 && !self.splits_ms.is_empty() {
            if self.splits_ms.len() != self.splits_total {
                return Err("INCOMPLETE SPLIT RECORD");
            }
            if self.splits_ms.windows(2).any(|w| w[1] < w[0]) {
                return Err("OUT-OF-ORDER SPLITS");
            }
        }
        if self.driving_ms == 0 || self.total_ms() == 0 {
            return Err("ZERO SIMULATION TIME");
        }
        Ok(())
    }
}

/// Temporary in-race HUD banner shown when the car crosses an original `.dls` split marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitBanner {
    /// 1-based split index (`1..=splits_total`).
    pub split_number: usize,
    pub splits_total: usize,
    /// Cumulative split time from stage start in milliseconds (720 Hz simulation time).
    pub cumulative_ms: u64,
    /// Signed delta vs the stored PB run's split at the exact same index
    /// (`cumulative_ms - pb_split_ms`). Negative means faster (green), positive means slower (red).
    pub delta_ms: Option<i64>,
}

impl SplitBanner {
    pub fn headline(&self) -> String {
        match self.delta_ms {
            Some(delta) => format!(
                "PARCIAL {} — {} — {}",
                self.split_number,
                format_time_ms(self.cumulative_ms),
                format_split_delta_ms(delta)
            ),
            None => format!(
                "PARCIAL {} — {}",
                self.split_number,
                format_time_ms(self.cumulative_ms)
            ),
        }
    }

    /// `Some(true)` if faster/equal to PB split, `Some(false)` if slower, `None` without PB split.
    pub fn is_faster(&self) -> Option<bool> {
        self.delta_ms.map(|d| d <= 0)
    }
}

/// Per-run sequential intermediate split tracker driven by authentic 720 Hz `StageTiming`.
/// Supports any split count `0..=16` dynamically from `.dls` markers without hardcoding 2.
#[derive(Debug, Clone, Default)]
pub struct SplitTracker {
    splits_total: usize,
    recorded_ms: Vec<u64>,
    active_banner: Option<(SplitBanner, Instant)>,
    order_violation: bool,
}

impl SplitTracker {
    pub fn reset(&mut self) {
        self.splits_total = 0;
        self.recorded_ms.clear();
        self.active_banner = None;
        self.order_violation = false;
    }

    pub fn recorded_ms(&self) -> &[u64] {
        &self.recorded_ms
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn splits_total(&self) -> usize {
        self.splits_total
    }

    pub fn has_order_violation(&self) -> bool {
        self.order_violation
    }

    pub fn active_banner(&self, now: Instant) -> Option<&SplitBanner> {
        self.active_banner
            .as_ref()
            .and_then(|(banner, until)| (now < *until).then_some(banner))
    }

    /// Synchronizes live simulation split state. Each split can fire at most once and strictly
    /// in ascending order; lingering on a split marker never produces duplicates.
    pub fn sync(
        &mut self,
        now: Instant,
        splits_passed: usize,
        splits_total: usize,
        split_times_s: &[Option<f64>],
        fallback_elapsed_s: Option<f64>,
        pb_splits_ms: Option<&[u64]>,
    ) -> Option<SplitBanner> {
        self.splits_total = splits_total;
        if splits_total == 0 {
            return None;
        }
        if splits_passed > splits_total {
            self.order_violation = true;
            return None;
        }
        // Detect out-of-order marker timestamps (a later split stamped while an earlier split is missing).
        for idx in self.recorded_ms.len()..splits_total {
            if split_times_s.get(idx).copied().flatten().is_none()
                && split_times_s
                    .get(idx + 1..splits_total)
                    .is_some_and(|tail| tail.iter().any(Option::is_some))
            {
                self.order_violation = true;
                return None;
            }
        }
        if splits_passed <= self.recorded_ms.len() {
            return None;
        }
        let mut latest = None;
        while self.recorded_ms.len() < splits_passed {
            let idx = self.recorded_ms.len();
            let secs = split_times_s
                .get(idx)
                .copied()
                .flatten()
                .filter(|v| v.is_finite() && *v > 0.0)
                .or_else(|| fallback_elapsed_s.filter(|v| v.is_finite() && *v > 0.0));
            let Some(secs) = secs else {
                break;
            };
            let cumulative_ms = (secs * 1000.0).round().max(1.0) as u64;
            if let Some(&prev_ms) = self.recorded_ms.last() {
                if cumulative_ms < prev_ms {
                    self.order_violation = true;
                    break;
                }
            }
            self.recorded_ms.push(cumulative_ms);
            // Strictly compare against the stored PB run's own split at the exact same index.
            let delta_ms = pb_splits_ms
                .and_then(|pb| pb.get(idx).copied())
                .filter(|&pb_ms| pb_ms > 0)
                .map(|pb_ms| (cumulative_ms as i64) - (pb_ms as i64));
            let banner = SplitBanner {
                split_number: idx + 1,
                splits_total,
                cumulative_ms,
                delta_ms,
            };
            self.active_banner = Some((
                banner.clone(),
                now + Duration::from_secs_f64(SPLIT_BANNER_DURATION_SECS),
            ));
            latest = Some(banner);
        }
        latest
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

/// Formats a split delta in standard rally `±MM:SS.mmm` format (e.g. `-00:02.310`, `+00:01.420`).
pub fn format_split_delta_ms(delta_ms: i64) -> String {
    let sign = if delta_ms < 0 { "-" } else { "+" };
    let abs = delta_ms.unsigned_abs();
    let mins = abs / 60_000;
    let secs = (abs % 60_000) / 1000;
    let millis = abs % 1000;
    format!("{sign}{mins:02}:{secs:02}.{millis:03}")
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
        assert_eq!(format_split_delta_ms(-2_310), "-00:02.310");
        assert_eq!(format_split_delta_ms(65_020), "+01:05.020");
    }

    #[test]
    fn split_tracker_records_in_order_prevents_duplicates_and_compares_pb() {
        let mut tracker = SplitTracker::default();
        let t0 = Instant::now();
        let pb_splits = [84_650u64, 171_230u64];

        // Before split 1: nothing triggers.
        assert!(tracker
            .sync(t0, 0, 2, &[None, None], Some(40.0), Some(&pb_splits))
            .is_none());
        assert!(tracker.active_banner(t0).is_none());

        // Cross split 1 (slower by +1.100s).
        let b1 = tracker
            .sync(
                t0 + Duration::from_secs(1),
                1,
                2,
                &[Some(85.750), None],
                Some(85.750),
                Some(&pb_splits),
            )
            .expect("split 1 banner");
        assert_eq!(b1.split_number, 1);
        assert_eq!(b1.cumulative_ms, 85_750);
        assert_eq!(b1.delta_ms, Some(1_100));
        assert_eq!(b1.is_faster(), Some(false));
        assert_eq!(b1.headline(), "PARCIAL 1 — 01:25.750 — +00:01.100");
        assert!(tracker.active_banner(t0 + Duration::from_secs(2)).is_some());
        assert!(tracker.active_banner(t0 + Duration::from_secs(6)).is_none());

        // Lingering on split 1 for multiple ticks must NEVER trigger a duplicate.
        assert!(tracker
            .sync(
                t0 + Duration::from_secs(2),
                1,
                2,
                &[Some(85.750), None],
                Some(86.200),
                Some(&pb_splits),
            )
            .is_none());
        assert_eq!(tracker.recorded_ms(), &[85_750]);

        // Cross split 2 (faster by -2.310s vs PB split 2).
        let b2 = tracker
            .sync(
                t0 + Duration::from_secs(10),
                2,
                2,
                &[Some(85.750), Some(168.920)],
                Some(168.920),
                Some(&pb_splits),
            )
            .expect("split 2 banner");
        assert_eq!(b2.split_number, 2);
        assert_eq!(b2.cumulative_ms, 168_920);
        assert_eq!(b2.delta_ms, Some(-2_310));
        assert_eq!(b2.is_faster(), Some(true));
        assert_eq!(b2.headline(), "PARCIAL 2 — 02:48.920 — -00:02.310");
        assert_eq!(tracker.recorded_ms(), &[85_750, 168_920]);
        assert!(!tracker.has_order_violation());
    }

    #[test]
    fn split_tracker_without_pb_and_variable_split_counts() {
        let mut tracker = SplitTracker::default();
        let t0 = Instant::now();

        // 0-split stage works cleanly.
        assert!(tracker.sync(t0, 0, 0, &[], Some(30.0), None).is_none());
        assert!(!tracker.has_order_violation());

        // 3-split stage without prior PB shows cumulative times with no delta.
        let b1 = tracker
            .sync(t0, 1, 3, &[Some(84.650), None, None], None, None)
            .expect("split 1");
        assert_eq!(b1.headline(), "PARCIAL 1 — 01:24.650");
        assert_eq!(b1.is_faster(), None);

        let b2 = tracker
            .sync(t0, 2, 3, &[Some(84.650), Some(150.0), None], None, None)
            .expect("split 2");
        assert_eq!(b2.headline(), "PARCIAL 2 — 02:30.000");

        let b3 = tracker
            .sync(
                t0,
                3,
                3,
                &[Some(84.650), Some(150.0), Some(215.5)],
                None,
                None,
            )
            .expect("split 3");
        assert_eq!(b3.headline(), "PARCIAL 3 — 03:35.500");
        assert_eq!(tracker.recorded_ms(), &[84_650, 150_000, 215_500]);
    }

    #[test]
    fn split_tracker_and_submission_reject_out_of_order_and_never_mix_independent_sectors() {
        let mut tracker = SplitTracker::default();
        let t0 = Instant::now();
        // Out-of-order: split 2 stamped while split 1 is missing -> rejected.
        assert!(tracker
            .sync(t0, 1, 2, &[None, Some(120.0)], None, None)
            .is_none());
        assert!(tracker.has_order_violation());

        // Non-monotonic splits in submission are rejected.
        let mut bad_order = sample_submission(200_000);
        bad_order.splits_ms = vec![90_000, 80_000];
        assert_eq!(bad_order.validate(), Err("OUT-OF-ORDER SPLITS"));

        // 0-split stage submission is valid.
        let mut zero_splits = sample_submission(120_000);
        zero_splits.splits_ms.clear();
        zero_splits.splits_passed = 0;
        zero_splits.splits_total = 0;
        assert!(zero_splits.validate().is_ok());

        // Slower overall run with a faster Split 1 MUST NOT overwrite PB's splits_ms.
        let mut store = RecordStore::new();
        let mut pb_run = sample_submission(210_000);
        pb_run.splits_ms = vec![75_000, 145_000];
        assert!(store.submit(&pb_run).is_new_best);

        let mut slower_with_fast_s1 = sample_submission(215_000);
        slower_with_fast_s1.splits_ms = vec![70_000, 150_000];
        let out = store.submit(&slower_with_fast_s1);
        assert!(out.valid);
        assert!(!out.is_new_best);
        assert_eq!(
            store.get(&pb_run.key).unwrap().splits_ms,
            vec![75_000, 145_000]
        );
    }

    #[test]
    fn backwards_compatible_reload_of_legacy_v1_record_without_splits() {
        let run = sample_submission(218_000);
        let legacy_json = format!(
            r#"{{
  "schema_version": 1,
  "records": [
    {{"car_id":"{}","car_name":"{}","stage_id":"{}","stage_name":"{}","stage_variant":"{}","surface":"{}","physics_version":"{}","rules_version":"{}","best_total_ms":225000,"driving_ms":225000,"penalty_ms":0,"false_start":false,"splits_ms":[],"splits_passed":2,"splits_total":2,"attempts":1,"platform":"ios","recorded_unix_s":1700000000}}
  ]
}}"#,
            run.key.car_id,
            run.car_name,
            run.key.stage_id,
            run.stage_name,
            run.key.stage_variant,
            run.key.surface,
            run.key.physics_version,
            run.key.rules_version
        );
        let mut store = RecordStore::from_json(&legacy_json).expect("legacy JSON must load");
        assert_eq!(store.len(), 1);
        let existing = store.get(&run.key).expect("legacy PB key match");
        assert!(existing.splits_ms.is_empty());

        // SplitTracker against empty legacy PB splits produces no fake delta.
        let mut tracker = SplitTracker::default();
        let b1 = tracker
            .sync(
                Instant::now(),
                1,
                2,
                &[Some(72.5)],
                None,
                Some(&existing.splits_ms),
            )
            .unwrap();
        assert_eq!(b1.delta_ms, None);

        // Faster run upgrades stored PB cleanly with real splits.
        let out = store.submit(&run);
        assert!(out.is_new_best);
        assert_eq!(store.get(&run.key).unwrap().splits_ms, run.splits_ms);
    }

    #[test]
    fn authentic_timekeeper_fires_diamond_creek_dls_splits_in_order_if_pc_present() {
        let dls_path = Path::new(r"C:\Richard Burns Rally\Maps\track-63_O.dls");
        if !dls_path.is_file() {
            return;
        }
        let bytes = std::fs::read(dls_path).unwrap();
        let mk = rerbr_sim::rbr_formats::markers::parse(&bytes).unwrap();
        let mut tk = rerbr_sim::rbr_gameplay::timing::TimeKeeper::load(&mk);
        let dt = 1.0_f32 / 720.0;
        let mut t = rerbr_sim::rbr_gameplay::timing::CLOCK_START;
        let splits: Vec<f32> = tk.card.splits().iter().map(|s| s.distance).collect();
        assert_eq!(splits.len(), 2, "Diamond Creek I has 2 authentic splits");
        assert!(splits[0] > tk.card.start && splits[1] > splits[0] && tk.card.finish > splits[1]);

        let mut tracker = SplitTracker::default();
        let now = Instant::now();
        let mut banner_events = Vec::new();
        // Drive forward at 30 m/s from start through split 1, linger at split 1, then through split 2 and finish.
        let mut dist = 0.0_f32;
        for step in 0..200_000u32 {
            t = rerbr_sim::rbr_gameplay::timing::advance_time(t, dt, true);
            // Linger near split 1 for 500 steps (~0.7s) to verify zero duplicate triggers.
            if (dist - splits[0]).abs() > 0.5 || step % 2 == 0 {
                dist += 30.0 * dt;
            }
            let (next_tk, _cp) = tk.record(dist, t, dt);
            tk = next_tk;
            let split_times: Vec<Option<f64>> = tk
                .card
                .splits()
                .iter()
                .map(|s| {
                    s.time
                        .filter(|&st| tk.card.started && st >= tk.card.origin)
                        .map(|st| st - tk.card.origin)
                })
                .collect();
            if let Some(banner) = tracker.sync(
                now,
                tk.card.passed,
                tk.card.splits().len(),
                &split_times,
                None,
                None,
            ) {
                banner_events.push(banner);
            }
            if tk.card.finish_time.is_some() {
                break;
            }
        }
        assert!(tk.card.finish_time.is_some());
        assert_eq!(tk.card.passed, 2);
        assert_eq!(
            banner_events.len(),
            2,
            "each split must fire exactly once even when lingering"
        );
        assert_eq!(banner_events[0].split_number, 1);
        assert_eq!(banner_events[1].split_number, 2);
        assert!(banner_events[1].cumulative_ms > banner_events[0].cumulative_ms);
        assert!(!tracker.has_order_violation());
    }
}
