//! Authentic Richard Burns Rally School (17 lessons, maps 73..=89) catalog and grade tracker.
//!
//! Uses `rerbr_sim::rbr_school::lesson` for authentic lesson identifiers, setup scripts
//! (`Physics/school/*.lsp`), tyre selection, and profile grade propagation without inventing
//! synthetic voiceovers or fake coaching text.

use rerbr_sim::rbr_formats::cars;
use rerbr_sim::rbr_school::lesson::{award, Lesson};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SchoolLessonEntry {
    pub lesson: Lesson,
    pub key: &'static str,
    pub map: u32,
    pub setup_file: &'static str,
    pub grade: u32,
    pub stem: String,
    pub assets_present: bool,
    pub missing_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SchoolCatalog {
    pub results: [u32; Lesson::COUNT as usize],
    pub entries: Vec<SchoolLessonEntry>,
    pub selected: usize,
    pub status: String,
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
                SchoolLessonEntry {
                    lesson,
                    key: lesson.title_key(),
                    map,
                    setup_file: lesson.setup_file(),
                    grade: results[i],
                    stem,
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
        }
    }

    pub fn record_verdict(&mut self, index: usize, verdict: u32) {
        let Some(lesson) = Lesson::new(index as u32) else {
            return;
        };
        self.results = award(self.results, lesson, verdict);
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.grade = self.results[i];
        }
        self.save_results();
    }
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
        }
    }
}
