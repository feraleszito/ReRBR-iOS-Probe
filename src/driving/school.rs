//! Authentic Richard Burns Rally School (17 lessons, maps 73..=89) catalog, localized texts,
//! `.dls` evaluation rules, instructor audio discovery, and grade progression tracker.
//!
//! Uses `rerbr_sim::rbr_school::lesson`, `rerbr_sim::rbr_school::bank`, `rerbr_sim::rbr_school::speech`,
//! and `rerbr_sim::rbr_formats::{rbz, strings, dls, markers}` without inventing synthetic voiceovers
//! or fabricated coaching instructions.

use rerbr_sim::rbr_formats::{cars, dls, markers, rbz, strings::Strings};
use rerbr_sim::rbr_school::{
    bank::Bank,
    lesson::{award, Lesson},
    speech,
};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const LESSON_COUNT: usize = Lesson::COUNT as usize;
pub const RECENT_ATTEMPTS: usize = 16;

pub fn medal_label(grade: u32) -> &'static str {
    match grade {
        1 | 2 => "BRONCE",
        3 => "PLATA",
        4 => "ORO",
        _ => "SIN MEDALLA",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchoolAttempt {
    pub lesson: usize,
    pub grade: u32,
    pub elapsed_ms: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchoolProgress {
    pub grades: [u32; LESSON_COUNT],
    pub recent: Vec<SchoolAttempt>,
}

impl SchoolProgress {
    fn record(&mut self, st: &rerbr_sim::drive::SchoolSessionSnapshot, grade: u32) {
        let lesson = st.lesson_index as usize;
        let Some(best) = self.grades.get_mut(lesson) else {
            return;
        };
        *best = (*best).max(grade);
        let elapsed_ms =
            (!st.untimed && st.elapsed_seconds.is_finite() && st.elapsed_seconds >= 0.0)
                .then(|| (st.elapsed_seconds * 1000.0).round() as u32);
        if self.recent.len() == RECENT_ATTEMPTS {
            self.recent.remove(0);
        }
        self.recent.push(SchoolAttempt {
            lesson,
            grade,
            elapsed_ms,
        });
    }

    pub fn medals(&self) -> [usize; 3] {
        let mut counts = [0; 3];
        for &grade in &self.grades {
            match grade {
                1 | 2 => counts[0] += 1,
                3 => counts[1] += 1,
                4 => counts[2] += 1,
                _ => {}
            }
        }
        counts
    }

    pub fn achievement(&self) -> Option<&'static str> {
        let [bronze, silver, gold] = self.medals();
        if gold == LESSON_COUNT {
            Some("ACADEMIA · TODAS EN ORO")
        } else if bronze + silver + gold == LESSON_COUNT {
            Some("DIPLOMA DE RALLY SCHOOL")
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SchoolAudioStatus {
    Ready {
        wav_count: usize,
        lang: String,
    },
    RequiresWavConversion {
        ogg_count: usize,
        lang: String,
    },
    #[default]
    Missing,
}

impl SchoolAudioStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Ready { wav_count, lang } => {
                format!("INSTRUCTOR {lang}: {wav_count} CLIPS WAV LISTOS")
            }
            Self::RequiresWavConversion { ogg_count, lang } => {
                format!("INSTRUCTOR {lang}: {ogg_count} .OGG (CONVIERTE A .WAV PCM)")
            }
            Self::Missing => "INSTRUCTOR: SIN AUDIO/RS (OPCIONAL)".into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct SchoolAudioCatalog {
    pub status: SchoolAudioStatus,
    pub clips: HashMap<u32, PathBuf>,
}

impl std::fmt::Debug for SchoolAudioCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchoolAudioCatalog")
            .field("status", &self.status)
            .field("clip_count", &self.clips.len())
            .finish()
    }
}

impl SchoolAudioCatalog {
    pub fn inspect(rbr_root: Option<&Path>) -> Self {
        let Some(root) = rbr_root else {
            return Self::default();
        };
        let mut best_ogg: Option<(String, usize)> = None;
        for (rel, lang) in [("Audio/RS/Spa", "SPA"), ("Audio/RS/Eng", "ENG")] {
            let Some(dir) = cars::resolve(root, rel) else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut clips = HashMap::new();
            let mut ogg_count = 0usize;
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                if ext == "wav" {
                    if let Some(id) = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        clips.insert(id, path);
                    }
                } else if ext == "ogg" {
                    ogg_count += 1;
                }
            }
            if !clips.is_empty() {
                return Self {
                    status: SchoolAudioStatus::Ready {
                        wav_count: clips.len(),
                        lang: lang.to_string(),
                    },
                    clips,
                };
            }
            if ogg_count > 0 && best_ogg.is_none() {
                best_ogg = Some((lang.to_string(), ogg_count));
            }
        }
        if let Some((lang, ogg_count)) = best_ogg {
            return Self {
                status: SchoolAudioStatus::RequiresWavConversion { ogg_count, lang },
                clips: HashMap::new(),
            };
        }
        Self::default()
    }

    pub fn play_ids(&self, ids: &[u32]) -> usize {
        if self.clips.is_empty() || ids.is_empty() {
            return 0;
        }
        let mut batch = Vec::new();
        for &id in ids {
            if let Some(path) = self.clips.get(&id) {
                if let Ok(samples) = super::audio::decode_wav_file(path) {
                    if !samples.is_empty() {
                        batch.push(Arc::new(samples));
                    }
                }
            }
        }
        let count = batch.len();
        if count > 0 {
            super::audio::enqueue_speech_batch(batch);
        }
        count
    }
}

#[derive(Clone, Debug)]
pub struct SchoolLessonEntry {
    pub lesson: Lesson,
    pub key: &'static str,
    pub title: Option<String>,
    pub instruction: Option<String>,
    pub cones_guide: Option<String>,
    pub map: u32,
    pub setup_file: &'static str,
    pub grade: u32,
    pub stem: String,
    pub untimed: bool,
    pub ride: bool,
    pub perfect_time: f32,
    pub pass_time: f32,
    pub time_limit: f32,
    pub target_distance: Option<f32>,
    pub has_filters: bool,
    pub briefing_audio_id: Option<u32>,
    pub assets_present: bool,
    pub missing_files: Vec<String>,
}

impl SchoolLessonEntry {
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(self.key)
    }

    pub fn grade_label(&self) -> String {
        match self.grade {
            0 => "SIN COMPLETAR (0/4)".into(),
            1..=3 => format!("APROBADO ({}/4)", self.grade),
            _ => "PERFECTO (4/4)".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SchoolRunResult {
    pub lesson_index: u32,
    pub title: String,
    pub key: &'static str,
    pub verdict: u32,
    pub elapsed_seconds: f32,
    pub perfect_time: f32,
    pub pass_time: f32,
    pub time_limit: f32,
    pub untimed: bool,
    pub ride: bool,
    pub retired: bool,
    pub headline: String,
    pub advice: Option<String>,
    pub failure_reasons: Vec<String>,
    pub previous_best: u32,
    pub new_best: u32,
    pub unlocked_count: usize,
}

#[derive(Clone, Debug)]
pub struct SchoolCatalog {
    pub results: [u32; Lesson::COUNT as usize],
    pub entries: Vec<SchoolLessonEntry>,
    pub selected: usize,
    pub status: String,
    pub strings_status: String,
    pub strings_available: bool,
    strings: Strings,
    pub audio: SchoolAudioCatalog,
}

impl Default for SchoolCatalog {
    fn default() -> Self {
        Self::load_default()
    }
}

impl SchoolCatalog {
    pub fn profile_path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(root.join("RBR").join("school_profile.txt"))
    }

    pub fn load_results() -> [u32; Lesson::COUNT as usize] {
        let Some(path) = Self::profile_path() else {
            return [0; Lesson::COUNT as usize];
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return [0; Lesson::COUNT as usize];
        };
        parse_results(&text)
    }

    pub fn save_results(&self) {
        let Some(path) = Self::profile_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, encode_results(&self.results));
    }

    pub fn load_default() -> Self {
        let results = Self::load_results();
        let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from);
        let rbr_root = doc.as_deref().map(|d| d.join("RBR"));
        Self::build(rbr_root.as_deref(), results)
    }

    pub fn build(rbr_root: Option<&Path>, results: [u32; Lesson::COUNT as usize]) -> Self {
        let (strings, strings_status, strings_available) = load_school_strings(rbr_root);
        let audio = SchoolAudioCatalog::inspect(rbr_root);
        let mut present_count = 0usize;
        let entries: Vec<SchoolLessonEntry> = Lesson::all()
            .enumerate()
            .map(|(i, lesson)| {
                let map = lesson.map();
                let (stem, missing_files) = inspect_lesson_assets(rbr_root, map);
                let assets_present = missing_files.is_empty();
                if assets_present {
                    present_count += 1;
                }
                let (si_key, sa_key) = lesson.brief_keys();
                let title = strings.get(lesson.title_key()).map(str::to_string);
                let instruction = strings.get(&si_key).map(str::to_string);
                let cones_guide = strings.get(&sa_key).map(str::to_string);
                let meta = inspect_dls_metadata(rbr_root, &stem);
                SchoolLessonEntry {
                    lesson,
                    key: lesson.title_key(),
                    title,
                    instruction,
                    cones_guide,
                    map,
                    setup_file: lesson.setup_file(),
                    grade: results[i],
                    stem,
                    untimed: lesson.untimed(),
                    ride: lesson.ride(),
                    perfect_time: meta.perfect_time,
                    pass_time: meta.pass_time,
                    time_limit: meta.time_limit,
                    target_distance: meta.target_distance,
                    has_filters: meta.has_filters,
                    briefing_audio_id: speech::briefing(lesson.index()),
                    assets_present,
                    missing_files,
                }
            })
            .collect();
        let status = if rbr_root.is_none() {
            "DOCUMENTS NO DISPONIBLE".into()
        } else if present_count == 0 {
            "FALTAN MAPS/TRACK-73..89_M.* O PHYSICS.RBZ".into()
        } else {
            format!("{present_count}/17 LECCIONES DISPONIBLES")
        };
        Self {
            results,
            entries,
            selected: 0,
            status,
            strings_status,
            strings_available,
            strings,
            audio,
        }
    }

    pub fn record_verdict(&mut self, index: usize, verdict: u32) {
        let Some(lesson) = Lesson::new(index as u32) else {
            return;
        };
        self.set_results(award(self.results, lesson, verdict));
        self.save_results();
    }

    pub fn set_results(&mut self, results: [u32; LESSON_COUNT]) {
        self.results = results;
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.grade = self.results[i];
        }
    }

    pub fn record_verdict_result(
        &mut self,
        st: &rerbr_sim::drive::SchoolSessionSnapshot,
        career: Option<&mut SchoolProgress>,
    ) -> SchoolRunResult {
        let index = st.lesson_index as usize;
        let entry = self.entries.get(index);
        let untimed = entry.is_some_and(|e| e.untimed);
        let ride = entry.is_some_and(|e| e.ride);
        let retired = matches!(st.ended, Some(rerbr_sim::drive::SchoolOutcome::Retired));
        let final_verdict = match st.ended {
            Some(rerbr_sim::drive::SchoolOutcome::Finished) if ride => 4,
            Some(rerbr_sim::drive::SchoolOutcome::Finished) => st.verdict.min(4),
            _ => 0,
        };
        let previous_best = career
            .as_ref()
            .map_or(&self.results, |p| &p.grades)
            .get(index)
            .copied()
            .unwrap_or(0);
        if let Some(progress) = career {
            progress.record(st, final_verdict);
            self.set_results(progress.grades);
        } else if final_verdict > 0 {
            self.record_verdict(index, final_verdict);
        }
        let new_best = self.results.get(index).copied().unwrap_or(0);
        let unlocked_count = self.results.iter().filter(|&&g| g > 0).count();
        let entry = self.entries.get(index);
        let title = entry
            .map(|e| e.display_title().to_string())
            .unwrap_or_else(|| format!("LECCIÓN {}", index + 1));
        let key = entry.map_or("SCHOOL", |e| e.key);
        let failure_reasons =
            self.resolve_failure_reasons(&st.failure_reasons, retired, final_verdict);
        let headline = if retired {
            self.strings
                .get("FAILURE1")
                .map(str::to_string)
                .unwrap_or_else(|| "COCHE RETIRADO / EJERCICIO FALLIDO".into())
        } else if final_verdict == 0 {
            self.strings
                .get("FAILURE1")
                .map(str::to_string)
                .unwrap_or_else(|| "EJERCICIO NO SUPERADO (0/4)".into())
        } else if final_verdict >= 4 {
            format!("EXCELENTE · VEREDICTO {final_verdict}/4")
        } else {
            format!("SUPERADO · VEREDICTO {final_verdict}/4")
        };
        let advice = if final_verdict == 0 {
            let adv_key = format!("SCOM_FAIL{}", (index % 4) + 1);
            self.strings.get(&adv_key).map(str::to_string)
        } else {
            None
        };
        let mut result_audio = Vec::new();
        if !ride {
            result_audio.push(speech::end_page(final_verdict, st.lesson_index));
        }
        self.audio.play_ids(&result_audio);
        SchoolRunResult {
            lesson_index: st.lesson_index,
            title,
            key,
            verdict: final_verdict,
            elapsed_seconds: st.elapsed_seconds,
            perfect_time: st.perfect_time,
            pass_time: st.pass_time,
            time_limit: st.time_limit,
            untimed,
            ride,
            retired,
            headline,
            advice,
            failure_reasons,
            previous_best,
            new_best,
            unlocked_count,
        }
    }

    pub fn resolve_failure_reasons(
        &self,
        raw: &[(String, bool)],
        retired: bool,
        verdict: u32,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for (key, _from_lesson) in raw {
            let resolved = self
                .strings
                .get(key)
                .map(str::to_string)
                .unwrap_or_else(|| key.clone());
            if !resolved.is_empty() && !out.contains(&resolved) {
                out.push(resolved);
            }
        }
        if out.is_empty() && retired {
            out.push("ABANDONO O DAÑO CRÍTICO DEL VEHÍCULO".into());
        } else if out.is_empty() && verdict == 0 {
            out.push("CRITERIO DE EVALUACIÓN NO CUMPLIDO".into());
        }
        out
    }

    pub fn play_briefing_audio(&self, index: usize) -> usize {
        let Some(entry) = self.entries.get(index) else {
            return 0;
        };
        let Some(id) = entry.briefing_audio_id else {
            return 0;
        };
        self.audio.play_ids(&[id])
    }

    pub fn play_event_speech(&self, ids: &[u32]) -> usize {
        self.audio.play_ids(ids)
    }

    pub fn explain_incident(&self, code: i32) -> String {
        match code {
            0 => "AVISO: CONTROL DE EJERCICIO".into(),
            1 => "PUNTO DE CONTROL / ZONA DE FRENADO".into(),
            2 => self
                .strings
                .get("OOBFAIL")
                .map(str::to_string)
                .unwrap_or_else(|| "FUERA DE PISTA".into()),
            3 => self
                .strings
                .get("CRASHFAIL")
                .map(str::to_string)
                .unwrap_or_else(|| "COLISIÓN / SALIDA FUERTE".into()),
            other => format!("INCIDENTE ESCUELA ({other})"),
        }
    }
}

#[derive(Default)]
struct DlsMeta {
    perfect_time: f32,
    pass_time: f32,
    time_limit: f32,
    target_distance: Option<f32>,
    has_filters: bool,
}

fn inspect_dls_metadata(rbr_root: Option<&Path>, stem: &str) -> DlsMeta {
    let Some(root) = rbr_root else {
        return DlsMeta::default();
    };
    let Some(dls_path) = cars::resolve(root, &format!("Maps/{stem}.dls")) else {
        return DlsMeta::default();
    };
    let Ok(bytes) = std::fs::read(dls_path) else {
        return DlsMeta::default();
    };
    let Ok(parsed) = dls::parse(&bytes) else {
        return DlsMeta::default();
    };
    let Ok(entry) = markers::driveline_entry(&parsed) else {
        return DlsMeta::default();
    };
    let bank = Bank::from_stream(&Bank::fresh(), &entry.replay, 0);
    let target_distance = bank
        .subs
        .iter()
        .find(|s| s.tag == 4 || s.tag == 5)
        .map(|s| s.center)
        .or_else(|| bank.checks.first().map(|c| c.pos));
    DlsMeta {
        perfect_time: bank.perfect_time,
        pass_time: bank.pass_time,
        time_limit: bank.time_limit,
        target_distance,
        has_filters: !bank.subs.is_empty() || !bank.checks.is_empty(),
    }
}

fn load_school_strings(rbr_root: Option<&Path>) -> (Strings, String, bool) {
    let Some(root) = rbr_root else {
        return (
            Strings::default(),
            "TEXTOS: DOCUMENTS NO DISPONIBLE".into(),
            false,
        );
    };
    let Some(archive) = cars::resolve(root, "strings.rbz") else {
        return (
            Strings::default(),
            "SIN STRINGS.RBZ (USA CLAVES ORIGINALES)".into(),
            false,
        );
    };
    let Ok(mut zip) = rbz::open(&archive) else {
        return (
            Strings::default(),
            "ERROR LEYENDO STRINGS.RBZ".into(),
            false,
        );
    };
    for (entry_name, lang) in [
        ("Strings/bstrings.spa", "SPA"),
        ("Strings/bstrings.eng", "ENG"),
    ] {
        if let Ok(mut file) = zip.by_name(entry_name) {
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_ok() {
                if let Ok(table) = rerbr_sim::rbr_formats::strings::parse(&bytes) {
                    return (
                        table,
                        format!("TEXTOS ORIGINALES OK ({lang} · {entry_name})"),
                        true,
                    );
                }
            }
        }
    }
    (
        Strings::default(),
        "STRINGS.RBZ SIN BSTRINGS.SPA/.ENG".into(),
        false,
    )
}

pub fn encode_results(results: &[u32; Lesson::COUNT as usize]) -> String {
    results
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub fn parse_results(text: &str) -> [u32; Lesson::COUNT as usize] {
    let mut out = [0; Lesson::COUNT as usize];
    for (i, part) in text.split(',').take(Lesson::COUNT as usize).enumerate() {
        if let Ok(v) = part.trim().parse::<u32>() {
            out[i] = v.min(4);
        }
    }
    out
}

fn inspect_lesson_assets(rbr_root: Option<&Path>, map: u32) -> (String, Vec<String>) {
    let Some(root) = rbr_root else {
        return (
            format!("track-{map}_M"),
            vec!["Documents/RBR (carpeta no disponible)".into()],
        );
    };
    let stem = ["M", "O", "N"]
        .into_iter()
        .map(|variant| format!("track-{map}_{variant}"))
        .find(|candidate| cars::resolve(root, &format!("Maps/{candidate}.col")).is_some())
        .unwrap_or_else(|| format!("track-{map}_M"));
    let mut missing = Vec::new();
    for ext in rerbr_content::PHYSICS_STAGE_EXTENSIONS {
        let rel = format!("Maps/{stem}.{ext}");
        if cars::resolve(root, &rel).is_none() {
            missing.push(rel);
        }
    }
    let tex_rel = format!("Maps/{stem}_textures.rbz");
    if cars::resolve(root, &tex_rel).is_none() {
        missing.push(tex_rel);
    }
    for common in ["physics.rbz", "misc.rbz", "Scripts.rbz", "Cars/Cars.ini"] {
        if cars::resolve(root, common).is_none() {
            missing.push(common.into());
        }
    }
    if cars::resolve(root, "Cars/impreza03/impreza03.sgc").is_none() {
        missing.push("Cars/impreza03/impreza03.sgc".into());
    }
    (stem, missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn school_catalog_exposes_all_17_lessons_and_awards_persisted_grades() {
        let mut cat = SchoolCatalog::build(None, [0; 17]);
        assert_eq!(cat.entries.len(), 17);
        assert!((73..=89).contains(&cat.entries[0].map));
        assert!((73..=89).contains(&cat.entries[16].map));
        cat.record_verdict(2, 3);
        assert_eq!(cat.results[0..3], [3, 3, 3]);
        assert_eq!(parse_results(&encode_results(&cat.results)), cat.results);
    }

    #[test]
    fn school_catalog_detects_real_track_m_and_lists_exact_missing_files() {
        let dir = std::env::temp_dir().join("rerbr_school_missing_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cat = SchoolCatalog::build(Some(&dir), [0; 17]);
        assert!(!cat.entries[0].assets_present);
        assert!(cat.entries[0]
            .missing_files
            .iter()
            .any(|p| p.contains("track-") && p.ends_with("_M.col")));
        assert!(!cat.entries[0]
            .missing_files
            .iter()
            .any(|p| p.contains("Maps/School")));
        let _ = std::fs::remove_dir_all(&dir);

        let pc_rbr = std::path::Path::new(r"C:\Richard Burns Rally");
        if pc_rbr.join("Maps/track-73_M.col").is_file() {
            let cat = SchoolCatalog::build(Some(pc_rbr), [0; 17]);
            let ready = cat.entries.iter().filter(|e| e.assets_present).count();
            assert_eq!(
                ready, 17,
                "all 17 school lessons on track-73..89_M should be detected: {:?}",
                cat.entries[0].missing_files
            );
            assert!(
                cat.strings_available,
                "PC RBR install should decode strings.rbz: {}",
                cat.strings_status
            );
            assert!(cat.entries[0].title.is_some());
            assert!(cat.entries[0].instruction.is_some());
            assert!(cat.entries[2].pass_time > 0.0);
            assert!(matches!(
                cat.audio.status,
                SchoolAudioStatus::Ready { .. } | SchoolAudioStatus::RequiresWavConversion { .. }
            ));
        }
    }

    #[test]
    fn school_audio_detects_wav_vs_ogg_honestly() {
        let root = std::env::temp_dir().join("rerbr_school_audio_test");
        let _ = std::fs::remove_dir_all(&root);
        let rs_spa = root.join("Audio/RS/Spa");
        std::fs::create_dir_all(&rs_spa).unwrap();
        std::fs::write(rs_spa.join("294.ogg"), b"fakeogg").unwrap();
        let cat_ogg = SchoolAudioCatalog::inspect(Some(&root));
        assert!(matches!(
            cat_ogg.status,
            SchoolAudioStatus::RequiresWavConversion { ogg_count: 1, .. }
        ));
        std::fs::write(rs_spa.join("294.wav"), b"fakewav").unwrap();
        let cat_wav = SchoolAudioCatalog::inspect(Some(&root));
        assert!(matches!(
            cat_wav.status,
            SchoolAudioStatus::Ready { wav_count: 1, .. }
        ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
#[test]
fn career_medals_use_completed_lesson_only_and_record_failed_attempts() {
    use rerbr_sim::drive::{SchoolOutcome, SchoolSessionSnapshot};

    let mut catalog = SchoolCatalog::build(None, [4; LESSON_COUNT]);
    let mut career = SchoolProgress::default();
    let mut state = SchoolSessionSnapshot {
        lesson_index: 4,
        ended: Some(SchoolOutcome::Finished),
        verdict: 3,
        elapsed_seconds: 53.25,
        ..SchoolSessionSnapshot::default()
    };
    let result = catalog.record_verdict_result(&state, Some(&mut career));
    assert_eq!((result.previous_best, result.new_best), (0, 3));
    assert_eq!(career.medals(), [0, 1, 0]);
    assert_eq!(career.achievement(), None);
    assert_eq!(career.grades[..4], [0; 4]);
    assert_eq!(career.recent[0].elapsed_ms, Some(53_250));
    state.ended = Some(SchoolOutcome::Retired);
    state.verdict = 4;
    let result = catalog.record_verdict_result(&state, Some(&mut career));
    assert_eq!((result.verdict, result.new_best), (0, 3));
    assert_eq!(career.recent.last().unwrap().grade, 0);
    career.grades.fill(2);
    assert_eq!(career.achievement(), Some("DIPLOMA DE RALLY SCHOOL"));
    career.grades.fill(4);
    assert_eq!(career.achievement(), Some("ACADEMIA · TODAS EN ORO"));
}
