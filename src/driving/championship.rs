//! Original Richard Burns Rally Championship mode (`season.dat`) manager & persistence.
//!
//! Reads the authentic rally calendar from `Documents/RBR/season.dat` via `rbr_formats::season_dat`,
//! tracks active rally, skill level, cumulative times, penalties, and stage progression cleanly.

use rerbr_content::StageDescriptor;
use rerbr_sim::rbr_formats::{cars, season_dat};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SkillLevel {
    #[default]
    Rookie,
    Professional,
    Champion,
}

impl SkillLevel {
    pub const ALL: [Self; 3] = [Self::Rookie, Self::Professional, Self::Champion];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rookie => "NOVATO",
            Self::Professional => "PROFESIONAL",
            Self::Champion => "CAMPEÓN",
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn from_word(s: &str) -> Self {
        match s.trim() {
            "1" | "Professional" => Self::Professional,
            "2" | "Champion" => Self::Champion,
            _ => Self::Rookie,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChampionshipProgress {
    pub active: bool,
    pub rally_index: usize,
    pub skill: SkillLevel,
    pub stage_index: usize,
    pub total_driving_seconds: f64,
    pub total_penalty_seconds: f64,
    pub false_starts: u8,
    pub completed_stages: usize,
}

impl Default for ChampionshipProgress {
    fn default() -> Self {
        Self {
            active: false,
            rally_index: 0,
            skill: SkillLevel::Rookie,
            stage_index: 0,
            total_driving_seconds: 0.0,
            total_penalty_seconds: 0.0,
            false_starts: 0,
            completed_stages: 0,
        }
    }
}

impl ChampionshipProgress {
    pub fn total_time_seconds(&self) -> f64 {
        self.total_driving_seconds + self.total_penalty_seconds
    }

    pub fn path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(root.join("RBR").join("championship_save.txt"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |t| Self::parse(&t))
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.serialize());
    }

    pub fn serialize(&self) -> String {
        format!(
            "active={}\nrally_index={}\nskill={}\nstage_index={}\ntotal_driving={:.3}\ntotal_penalty={:.3}\nfalse_starts={}\ncompleted_stages={}\n",
            self.active,
            self.rally_index,
            self.skill.index(),
            self.stage_index,
            self.total_driving_seconds,
            self.total_penalty_seconds,
            self.false_starts,
            self.completed_stages
        )
    }

    pub fn parse(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "active" => p.active = v == "true",
                "rally_index" => {
                    if let Ok(n) = v.parse::<usize>() {
                        p.rally_index = n.min(5);
                    }
                }
                "skill" => p.skill = SkillLevel::from_word(v),
                "stage_index" => {
                    if let Ok(n) = v.parse::<usize>() {
                        p.stage_index = n.min(6);
                    }
                }
                "total_driving" => {
                    if let Ok(n) = v.parse::<f64>() {
                        if n.is_finite() && n >= 0.0 {
                            p.total_driving_seconds = n;
                        }
                    }
                }
                "total_penalty" => {
                    if let Ok(n) = v.parse::<f64>() {
                        if n.is_finite() && n >= 0.0 {
                            p.total_penalty_seconds = n;
                        }
                    }
                }
                "false_starts" => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.false_starts = n;
                    }
                }
                "completed_stages" => {
                    if let Ok(n) = v.parse::<usize>() {
                        p.completed_stages = n;
                    }
                }
                _ => {}
            }
        }
        p
    }
}

#[derive(Clone, Debug)]
pub struct ChampionshipCalendar {
    pub rallies: Vec<season_dat::Rally>,
    pub status: String,
    pub progress: ChampionshipProgress,
}

impl ChampionshipCalendar {
    pub fn load_default() -> Self {
        let progress = ChampionshipProgress::load();
        let Some(doc) = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from) else {
            return Self {
                rallies: Vec::new(),
                status: "DOCUMENTS NO DISPONIBLE".into(),
                progress,
            };
        };
        Self::load_from_rbr_root(&doc.join("RBR"), progress)
    }

    pub fn load_from_rbr_root(rbr_root: &Path, progress: ChampionshipProgress) -> Self {
        let Some(path) = cars::resolve(rbr_root, "season.dat") else {
            return Self {
                rallies: Vec::new(),
                status: "FALTA Documents/RBR/season.dat".into(),
                progress,
            };
        };
        match std::fs::read(&path).and_then(|b| {
            season_dat::parse(&b)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }) {
            Ok(rallies) => Self {
                status: format!("{} RALLIES CARGADOS (season.dat)", rallies.len()),
                rallies,
                progress,
            },
            Err(e) => Self {
                rallies: Vec::new(),
                status: format!("ERROR season.dat: {e}"),
                progress,
            },
        }
    }

    pub fn current_stage_info(&self) -> Option<&season_dat::StageInfo> {
        let rally = self.rallies.get(self.progress.rally_index)?;
        let level = rally.levels.get(self.progress.skill.index())?;
        level.get(self.progress.stage_index)
    }

    pub fn current_stage_descriptor(&self) -> Option<StageDescriptor> {
        let info = self.current_stage_info()?;
        StageDescriptor::presets()
            .into_iter()
            .find(|s| s.original_map == info.map)
    }

    pub fn record_stage_finish(
        &mut self,
        driving_seconds: f64,
        penalty_seconds: f64,
        false_start: bool,
    ) {
        if !self.progress.active {
            return;
        }
        self.progress.total_driving_seconds += driving_seconds.max(0.0);
        self.progress.total_penalty_seconds += penalty_seconds.max(0.0);
        if false_start {
            self.progress.false_starts = self.progress.false_starts.saturating_add(1);
        }
        self.progress.completed_stages += 1;
        let max_stages = self
            .rallies
            .get(self.progress.rally_index)
            .and_then(|r| r.levels.get(self.progress.skill.index()))
            .map_or(6, |l| l.len());
        if self.progress.stage_index + 1 < max_stages {
            self.progress.stage_index += 1;
        } else if self.progress.rally_index + 1 < self.rallies.len() {
            self.progress.rally_index += 1;
            self.progress.stage_index = 0;
        } else {
            self.progress.active = false;
        }
        self.progress.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_persistence_and_advancement() {
        let p = ChampionshipProgress {
            active: true,
            rally_index: 2,
            skill: SkillLevel::Professional,
            stage_index: 3,
            total_driving_seconds: 245.5,
            total_penalty_seconds: 10.0,
            false_starts: 1,
            completed_stages: 12,
        };
        let parsed = ChampionshipProgress::parse(&p.serialize());
        assert_eq!(parsed, p);
        assert_eq!(parsed.total_time_seconds(), 255.5);
    }

    #[test]
    fn reads_private_season_dat_if_pc_assets_present() {
        let rbr = Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("season.dat").is_file() {
            return;
        }
        let mut cal =
            ChampionshipCalendar::load_from_rbr_root(rbr, ChampionshipProgress::default());
        assert_eq!(cal.rallies.len(), 6);
        assert_eq!(cal.rallies[0].name, "USA");
        cal.progress.active = true;
        cal.progress.rally_index = cal
            .rallies
            .iter()
            .position(|r| r.levels[0][0].map == 20 || r.levels[0][0].map == 22)
            .unwrap_or(1);
        let desc = cal.current_stage_descriptor();
        assert!(
            desc.is_some(),
            "UK stage 0 should match preset stage descriptor"
        );
        let before_stage = cal.progress.stage_index;
        cal.record_stage_finish(180.0, 10.0, true);
        assert_eq!(cal.progress.stage_index, before_stage + 1);
        assert_eq!(cal.progress.false_starts, 1);
        assert_eq!(cal.progress.total_time_seconds(), 190.0);
    }
}
