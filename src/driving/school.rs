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
    pub assets_present: bool,
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
                let assets_present = rbr_root.is_some_and(|root| {
                    cars::resolve(root, &format!("Maps/track-{map}_O.col")).is_some()
                        || cars::resolve(root, &format!("Maps/track-{map}_N.col")).is_some()
                });
                if assets_present {
                    present_count += 1;
                }
                SchoolLessonEntry {
                    lesson,
                    key: lesson.title_key(),
                    map,
                    setup_file: lesson.setup_file(),
                    grade: results[i],
                    assets_present,
                }
            })
            .collect();
        let status = if rbr_root.is_none() {
            "DOCUMENTS NO DISPONIBLE".into()
        } else if present_count == 0 {
            "FALTA MAPS/TRACK-73..89 PARA ESCUELA DE RALLY".into()
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
}
