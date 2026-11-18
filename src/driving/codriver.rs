//! Authentic Richard Burns Rally co-driver pacenote loader and audio synchronizer.
//!
//! Reads the stage's `.dls` `Driveline` marker keys with `rbr_formats::markers` and drives
//! speed-dependent lookahead synchronization through `rbr_gameplay::markers::MarkerTicker`,
//! queueing authentic PCM `.wav` speech clips sequentially without altering physics state.

use super::audio;
use rerbr_sim::{
    rbr_formats::{
        cars,
        markers::{self, StageMarkers, TAG_END, TAG_FINISH, TAG_SPLIT, TAG_START},
        pacenote::codriver as rbr_pacenote,
    },
    rbr_gameplay::markers::{MarkerEvent, MarkerSettings, MarkerTicker, PaceCallout, TickInput},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceBankStatus {
    Ready {
        voice: String,
        wav_count: usize,
        note_count: usize,
    },
    RequiresWavConversion {
        ogg_dir: String,
        ogg_count: usize,
        note_count: usize,
    },
    Missing {
        note_count: usize,
    },
}

impl VoiceBankStatus {
    pub fn summary(&self) -> String {
        match self {
            Self::Ready {
                voice,
                wav_count,
                note_count,
            } => format!("COPILOTO: {voice} ({wav_count} WAV / {note_count} NOTAS)"),
            Self::RequiresWavConversion {
                ogg_dir,
                ogg_count,
                note_count,
            } => format!(
                "COPILOTO: {ogg_count} .OGG EN {ogg_dir} -> CONVERTIR A .WAV ({note_count} NOTAS)"
            ),
            Self::Missing { note_count } => {
                format!("COPILOTO: SIN VOCES WAV ({note_count} NOTAS .DLS)")
            }
        }
    }
}

#[derive(Debug, Clone)]
struct VoiceDirectoryScan {
    voice_label: String,
    wav_files: HashMap<String, PathBuf>,
    ogg_dir: Option<String>,
    ogg_count: usize,
}

#[derive(Default)]
struct ClipBank {
    clips: HashMap<String, Vec<Arc<Vec<[i16; 2]>>>>,
    status_label: String,
    take_cursor: usize,
}

impl ClipBank {
    fn load(documents: &Path, note_count: usize) -> (Self, VoiceBankStatus) {
        let scan = scan_voice_directories(documents);
        let mut clips: HashMap<String, Vec<Arc<Vec<[i16; 2]>>>> = HashMap::new();
        let mut decoded_files = 0usize;

        if !scan.wav_files.is_empty() {
            // Decode unique referenced stems once on stage load so driving triggers perform zero disk I/O.
            let mut decoded_paths: HashMap<PathBuf, Arc<Vec<[i16; 2]>>> = HashMap::new();
            for (stem, path) in &scan.wav_files {
                let clip = match decoded_paths.get(path) {
                    Some(existing) => Arc::clone(existing),
                    None => match audio::decode_wav_file(path) {
                        Ok(pcm) if !pcm.is_empty() => {
                            let arc = Arc::new(pcm);
                            decoded_paths.insert(path.clone(), Arc::clone(&arc));
                            decoded_files += 1;
                            arc
                        }
                        _ => continue,
                    },
                };
                clips.entry(stem.clone()).or_default().push(clip);
            }
        }

        let status = if decoded_files > 0 {
            VoiceBankStatus::Ready {
                voice: scan.voice_label,
                wav_count: decoded_files,
                note_count,
            }
        } else if scan.ogg_count > 0 {
            VoiceBankStatus::RequiresWavConversion {
                ogg_dir: scan.ogg_dir.unwrap_or_else(|| "Audio/Speech".into()),
                ogg_count: scan.ogg_count,
                note_count,
            }
        } else {
            VoiceBankStatus::Missing { note_count }
        };
        let status_label = status.summary();
        (
            Self {
                clips,
                status_label,
                take_cursor: 0,
            },
            status,
        )
    }

    fn pick(&mut self, candidates: &[String]) -> Option<Arc<Vec<[i16; 2]>>> {
        let mut available: Vec<&Arc<Vec<[i16; 2]>>> = Vec::new();
        for stem in candidates {
            let key = normalize_stem(stem);
            if let Some(list) = self.clips.get(&key) {
                available.extend(list.iter());
            }
        }
        if available.is_empty() {
            return None;
        }
        let chosen = Arc::clone(available[self.take_cursor % available.len()]);
        self.take_cursor = self.take_cursor.wrapping_add(1);
        Some(chosen)
    }
}

#[derive(Debug, Clone)]
pub struct ActiveVisualCallout {
    pub tag: u32,
    pub slot_word_id: Option<u32>,
    pub flag_word_id: Option<u32>,
    pub label: String,
    pub at: Instant,
}

/// Active per-stage RBR co-driver runtime.
pub struct CoDriverSession {
    markers: StageMarkers,
    ticker: MarkerTicker,
    settings: MarkerSettings,
    bank: ClipBank,
    pub status: VoiceBankStatus,
    pub last_callout: Option<(String, Instant)>,
    pub last_visual: Option<ActiveVisualCallout>,
}

impl CoDriverSession {
    /// Load `.dls` driveline pacenotes for `stage_stem` and index available PCM voice files.
    pub fn load_for_stage(documents: &Path, maps: &Path, stage_stem: &str) -> Self {
        let dls_name = format!("{stage_stem}.dls");
        let markers = cars::resolve(maps, &dls_name)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| markers::parse(&bytes).ok())
            .unwrap_or(StageMarkers { keys: Vec::new() });
        Self::from_markers(documents, markers)
    }

    pub fn from_markers(documents: &Path, markers: StageMarkers) -> Self {
        let note_count = markers.pace_notes().count();
        let (bank, status) = ClipBank::load(documents, note_count);
        audio::set_codriver_status(bank.status_label.clone());
        let ticker = MarkerTicker::load(&markers);
        Self {
            markers,
            ticker,
            settings: MarkerSettings::default(),
            bank,
            status,
            last_callout: None,
            last_visual: None,
        }
    }

    #[cfg(test)]
    fn from_markers_and_settings(
        documents: &Path,
        markers: StageMarkers,
        settings: MarkerSettings,
    ) -> Self {
        let mut session = Self::from_markers(documents, markers);
        session.settings = settings;
        session
    }

    /// Clear `FIRED` runtime bits and speech queue cleanly on stage restart.
    pub fn reset(&mut self) {
        self.ticker = MarkerTicker::load(&self.markers);
        self.last_callout = None;
        self.last_visual = None;
        audio::clear_speech_queue();
    }

    /// Advance pacenotes using authentic RBR 1.02 speed-dependent lookahead.
    /// Returns any spoken pacenote descriptions emitted on this tick.
    pub fn tick(&mut self, driveline_distance_m: f32, speed_mps: f32) -> Vec<String> {
        if self.ticker.keys().is_empty()
            || !driveline_distance_m.is_finite()
            || !speed_mps.is_finite()
        {
            return Vec::new();
        }
        let input = TickInput {
            x: driveline_distance_m,
            dot150: speed_mps.max(0.0),
            seek_d: self.ticker.seek_d(driveline_distance_m),
            lookahead_scale: self.settings.callout_distance,
            pace_speed_threshold: self.settings.marker_threshold,
            ..TickInput::default()
        };
        let (next, events) =
            std::mem::replace(&mut self.ticker, MarkerTicker::from_keys(Vec::new())).tick(input);
        self.ticker = next;

        let mut spoken_labels = Vec::new();
        for event in events {
            let MarkerEvent::PaceCallout(callout) = event else {
                continue;
            };
            if let Some(label) = self.dispatch_callout(&callout) {
                self.last_visual = Some(ActiveVisualCallout {
                    tag: callout.key.tag,
                    slot_word_id: u32::try_from(callout.slot_word_id).ok(),
                    flag_word_id: u32::try_from(callout.flag_word_id).ok(),
                    label: label.clone(),
                    at: Instant::now(),
                });
                spoken_labels.push(label);
            }
        }
        if let Some(last) = spoken_labels.last() {
            self.last_callout = Some((last.clone(), Instant::now()));
        }
        spoken_labels
    }

    pub fn current_text(&self, now: Instant) -> Option<&str> {
        let (text, at) = self.last_callout.as_ref()?;
        (now.saturating_duration_since(*at).as_secs_f32() <= 3.5).then_some(text.as_str())
    }

    pub fn current_visual(
        &self,
        now: Instant,
        anticipation_s: f32,
    ) -> Option<&ActiveVisualCallout> {
        let v = self.last_visual.as_ref()?;
        (now.saturating_duration_since(v.at).as_secs_f32() <= anticipation_s.max(0.5)).then_some(v)
    }

    #[cfg(test)]
    pub fn fired_count(&self) -> usize {
        self.ticker
            .keys()
            .iter()
            .filter(|k| k.word1 & rerbr_sim::rbr_gameplay::markers::FIRED != 0)
            .count()
    }

    #[cfg(test)]
    pub fn marker_keys(&self) -> &[rerbr_sim::rbr_formats::markers::MarkerKey] {
        self.ticker.keys()
    }

    fn dispatch_callout(&mut self, callout: &PaceCallout) -> Option<String> {
        let spec = callout_spec(callout.key.tag, callout.slot_word_id)?;
        let mut clips = Vec::new();
        let mut parts = vec![spec.label.to_string()];

        if let Some(clip) = self.bank.pick(&spec.stems) {
            clips.push(clip);
        }

        if let Some(mod_spec) = modifier_spec(callout.flag_word_id) {
            parts.push(mod_spec.label.to_string());
            if let Some(clip) = self.bank.pick(&mod_spec.stems) {
                clips.push(clip);
            }
        }

        let is_control_note = matches!(
            callout.key.tag,
            TAG_START | TAG_FINISH | TAG_SPLIT | TAG_END
        );
        if !is_control_note {
            if let Some(idx) = self.ticker.index_of(callout.key) {
                let gap = self.ticker.key_gap(idx);
                if (0.0..15.0).contains(&gap) {
                    let link_word_id = link_for_next(&self.ticker, idx);
                    if let Some(link) = word_spec(link_word_id) {
                        parts.push(link.label.to_string());
                        if let Some(clip) = self.bank.pick(&link.stems) {
                            clips.push(clip);
                        }
                    }
                } else if gap >= 25.0 {
                    let bucket = distance_bucket(gap);
                    parts.push(format!("{bucket}"));
                    let dist_stems = distance_stems(bucket);
                    if let Some(clip) = self.bank.pick(&dist_stems) {
                        clips.push(clip);
                    }
                }
            }
        }

        if !clips.is_empty() {
            audio::enqueue_speech_batch(clips);
        }
        Some(parts.join(" "))
    }
}

struct WordSpec {
    label: &'static str,
    stems: Vec<String>,
}

fn spec(label: &'static str, stems: &[&str]) -> WordSpec {
    WordSpec {
        label,
        stems: stems.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn callout_spec(tag: u32, slot_word_id: i32) -> Option<WordSpec> {
    if tag == 32 {
        return Some(spec("PRECAUCIÓN", &["caution", "care"]));
    }
    word_spec(slot_word_id)
}

fn modifier_spec(flag_word_id: i32) -> Option<WordSpec> {
    if flag_word_id < 0 {
        return None;
    }
    word_spec(flag_word_id)
}

fn link_for_next(ticker: &MarkerTicker, idx: usize) -> i32 {
    let mut next = ticker.next_key(idx);
    while let Some(k) = next {
        let tag = ticker.keys()[k].tag;
        if rerbr_sim::rbr_gameplay::markers::spoken_tag(tag) {
            return if tag == 27 { 0x30 } else { 0x1f };
        }
        next = ticker.next_key(k);
    }
    0x1f
}

/// Complete E25 bank word ID table from RBR 1.02 `Audio/CoDriver.ini` & `Audio/CoOther.ini`.
fn word_spec(word_id: i32) -> Option<WordSpec> {
    match word_id {
        0x0e => Some(spec(
            "90 IZQ",
            &["90_left1", "90_left2", "90_left3", "90 left", "90_left"],
        )),
        0x0f => Some(spec(
            "90 DER",
            &[
                "90_right1",
                "90_right2",
                "90_right3",
                "90 right",
                "90_right",
            ],
        )),
        0x10 => Some(spec("Y", &["and1", "and2", "and"])),
        0x11 => Some(spec("PUENTE", &["bridge"])),
        0x12 => Some(spec("BACHE", &["bump1", "bump2", "bump"])),
        0x13 => Some(spec("CUIDADO", &["care", "caution"])),
        0x14 => Some(spec("PRECAUCIÓN", &["caution", "care"])),
        0x15 => Some(spec("CORTAR", &["cut1", "cut2", "cut"])),
        0x16 => Some(spec("NO CORTAR", &["dontcut1", "dontcut2", "dontcut"])),
        0x17 => Some(spec(
            "IZQ SUAVE",
            &[
                "easy_left",
                "easy_left1",
                "easy_left2",
                "easy_left3",
                "easy left",
            ],
        )),
        0x18 => Some(spec(
            "DER SUAVE",
            &[
                "easy_right",
                "easy_right1",
                "easy_right2",
                "easy_right3",
                "easy_right4",
                "easy right",
            ],
        )),
        0x19 => Some(spec(
            "IZQ RÁPIDA",
            &[
                "fast_left",
                "fast_left1",
                "fast_left2",
                "fast_left3",
                "fast left",
            ],
        )),
        0x1a => Some(spec(
            "DER RÁPIDA",
            &[
                "fast_right",
                "fast_right1",
                "fast_right2",
                "fast_right3",
                "fast right",
            ],
        )),
        0x1b => Some(spec("META", &["finish"])),
        0x1c => Some(spec("VADO", &["ford"])),
        0x1d => Some(spec(
            "HORQUILLA IZQ",
            &[
                "hairpin_left1",
                "hairpin_left2",
                "hairpin_left3",
                "hp left",
                "hairpin_left",
            ],
        )),
        0x1e => Some(spec(
            "HORQUILLA DER",
            &[
                "hairpin_right1",
                "hairpin_right2",
                "hairpin_right3",
                "hp right",
                "hairpin_right",
            ],
        )),
        0x1f => Some(spec("PARA", &["into1", "into2", "into"])),
        0x20 => Some(spec("SALTO", &["jump1", "jump2", "jump"])),
        0x21 => Some(spec(
            "IZQ K",
            &["k_left1", "k_left2", "k_left3", "k left", "k_left"],
        )),
        0x22 => Some(spec(
            "DER K",
            &["k_right1", "k_right2", "k_right3", "k right", "k_right"],
        )),
        0x23 => Some(spec("LARGA", &["long1", "long2", "long"])),
        0x24 => Some(spec("QUIZÁ", &["maybe1", "maybe2", "maybe"])),
        0x25 => Some(spec(
            "IZQ MEDIA",
            &[
                "medium_left1",
                "medium_left2",
                "medium_left3",
                "medium_left",
                "medium left",
            ],
        )),
        0x26 => Some(spec(
            "DER MEDIA",
            &[
                "medium_right",
                "medium_right1",
                "medium_right2",
                "medium_right3",
                "medium right",
            ],
        )),
        0x27 => Some(spec("ESTRECHA", &["narrows1", "narrows2", "narrows"])),
        0x28 => Some(spec("CRESTA", &["overcrest1", "overcrest2", "overcrest"])),
        0x29 => Some(spec(
            "SE CIERRA MUCHO",
            &[
                "tightens_bad1",
                "tightens_bad2",
                "tightens bad",
                "tightens_bad",
            ],
        )),
        0x2a => Some(spec("SE CIERRA", &["tightens1", "tightens2", "tightens"])),
        0x2c => Some(spec("SINUOSO", &["twisty"])),
        0x2d => Some(spec("SE ABRE", &["wideout1", "wideout2", "wideout"])),
        0x30 => Some(spec("SOBRE", &["onto"])),
        0x31 => Some(spec("DER FONDO", &["flat_right", "flat right"])),
        0x32 => Some(spec("IZQ FONDO", &["flat_left", "flat left"])),
        0x33 => Some(spec("RECTO", &["go_straight", "go straight"])),
        0x34 => Some(spec("MANTÉN DER", &["keep_right", "keep right"])),
        0x35 => Some(spec("MANTÉN IZQ", &["keep_left", "keep left"])),
        0x36 => Some(spec("MANTÉN CENTRO", &["keep_middle", "keep middle"])),
        _ => None,
    }
}

fn distance_bucket(metres: f32) -> u32 {
    const BUCKETS: [u32; 14] = [
        30, 50, 70, 100, 150, 200, 300, 400, 500, 600, 700, 800, 900, 1000,
    ];
    *BUCKETS
        .iter()
        .min_by(|&&a, &&b| {
            let da = (a as f32 - metres).abs();
            let db = (b as f32 - metres).abs();
            da.total_cmp(&db)
        })
        .unwrap_or(&100)
}

fn distance_stems(bucket: u32) -> Vec<String> {
    vec![
        format!("p{bucket}"),
        format!("p{bucket}_1"),
        format!("p{bucket}_2"),
        format!("{bucket}"),
    ]
}

fn normalize_stem(stem: &str) -> String {
    stem.trim()
        .to_ascii_lowercase()
        .trim_end_matches(".wav")
        .trim_end_matches(".ogg")
        .to_string()
}

fn collect_audio_in_dir(dir: &Path, wavs: &mut HashMap<String, PathBuf>, ogg_count: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("wav") {
            wavs.entry(normalize_stem(stem)).or_insert(path);
        } else if ext.eq_ignore_ascii_case("ogg") {
            *ogg_count += 1;
        }
    }
}

fn scan_voice_directories(documents: &Path) -> VoiceDirectoryScan {
    let roots = [documents.join("RBR"), documents.to_path_buf()];
    let mut best_ogg_dir: Option<String> = None;
    let mut best_ogg_count = 0usize;

    for root in &roots {
        if !root.is_dir() {
            continue;
        }
        // Check Pacenote plugin configured voice if present and converted to WAV.
        if let Some(plugin_dir) = rbr_pacenote::plugin_dir(root) {
            let settings = rbr_pacenote::settings(&plugin_dir);
            if let Some(voice) = settings.voice {
                let rel = format!("sounds/{voice}");
                if let Some(dir) = cars::resolve(plugin_dir.as_path(), &rel) {
                    let mut wavs = HashMap::new();
                    let mut oggs = 0usize;
                    collect_audio_in_dir(&dir, &mut wavs, &mut oggs);
                    if !wavs.is_empty() {
                        return VoiceDirectoryScan {
                            voice_label: format!("Pacenote/{voice}"),
                            wav_files: wavs,
                            ogg_dir: None,
                            ogg_count: 0,
                        };
                    }
                    if oggs > best_ogg_count {
                        best_ogg_count = oggs;
                        best_ogg_dir = Some(format!("Plugins/Pacenote/{rel}"));
                    }
                }
            }
        }

        // Standard retail RBR speech folders + their matching number directories.
        let candidates = [
            ("Audio/Speech/Spa", "Audio/Speech/NumSpa", "ESPAÑOL"),
            ("Audio/Speech/Eng", "Audio/Speech/Number", "ENGLISH"),
            ("Audio/Speech/PaceNote", "Audio/Speech/Number", "ENGLISH"),
            ("Audio/Speech/Fre", "Audio/Speech/NumFre", "FRANÇAIS"),
            ("Audio/Speech/Ger", "Audio/Speech/NumGer", "DEUTSCH"),
            ("Audio/Speech/Ita", "Audio/Speech/NumIta", "ITALIANO"),
            ("Audio/Speech/Czh", "Audio/Speech/NumCzh", "ČEŠTINA"),
            ("Audio/Speech/Pol", "Audio/Speech/NumPol", "POLSKI"),
        ];
        for (speech_rel, num_rel, label) in candidates {
            let Some(speech_dir) = cars::resolve(root, speech_rel) else {
                continue;
            };
            let mut wavs = HashMap::new();
            let mut oggs = 0usize;
            collect_audio_in_dir(&speech_dir, &mut wavs, &mut oggs);
            if let Some(num_dir) = cars::resolve(root, num_rel) {
                collect_audio_in_dir(&num_dir, &mut wavs, &mut oggs);
            }
            if num_rel != "Audio/Speech/Number" {
                if let Some(fallback_num) = cars::resolve(root, "Audio/Speech/Number") {
                    collect_audio_in_dir(&fallback_num, &mut wavs, &mut oggs);
                }
            }
            if !wavs.is_empty() {
                return VoiceDirectoryScan {
                    voice_label: label.to_string(),
                    wav_files: wavs,
                    ogg_dir: None,
                    ogg_count: 0,
                };
            }
            if oggs > best_ogg_count {
                best_ogg_count = oggs;
                best_ogg_dir = Some(speech_rel.to_string());
            }
        }
    }

    VoiceDirectoryScan {
        voice_label: "NO AUDIO".into(),
        wav_files: HashMap::new(),
        ogg_dir: best_ogg_dir,
        ogg_count: best_ogg_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rerbr_sim::rbr_formats::markers::MarkerKey;

    fn write_test_wav(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // Minimal valid 44.1 kHz mono 16-bit PCM WAV (4 samples)
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36u32 + 8).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&88200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        std::fs::write(path, bytes).unwrap();
    }

    fn key(tag: u32, word1: u32, distance: f32) -> MarkerKey {
        MarkerKey {
            tag,
            word1,
            distance,
        }
    }

    #[test]
    fn speed_dependent_lookahead_and_stationary_gate() {
        let root = std::env::temp_dir().join(format!("rerbr-codriver-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write_test_wav(&root.join("RBR/Audio/Speech/Spa/90 left.wav"));
        write_test_wav(&root.join("RBR/Audio/Speech/Spa/tightens.wav"));
        write_test_wav(&root.join("RBR/Audio/Speech/NumSpa/p100.wav"));

        let markers = StageMarkers {
            keys: vec![
                key(TAG_START, 0, 5.0),
                key(1, 0x4, 80.0), // 90 left + TIGHTENS
                key(10, 0, 180.0), // 90 right, gap=100m
            ],
        };
        let mut session =
            CoDriverSession::from_markers_and_settings(&root, markers, MarkerSettings::default());
        assert!(matches!(session.status, VoiceBankStatus::Ready { .. }));

        // Stationary at 6m (below 25 km/h threshold): emits nothing.
        assert!(session.tick(6.0, 0.0).is_empty());
        assert_eq!(session.fired_count(), 1); // StartLine crossed & fired, pacenote untouched

        // Slow speed (5 m/s = 18 km/h < 25 km/h) with lookahead=20m at x=20m: still doesn't reach 80m.
        assert!(session.tick(20.0, 5.0).is_empty());

        // High speed (25 m/s = 90 km/h): lookahead = 4*25 = 100m; at x=21m -> 121m > 80m -> triggers 90 left!
        let spoken = session.tick(21.0, 25.0);
        assert_eq!(spoken, vec!["90 IZQ SE CIERRA 100"]);

        // Re-ticking same position does NOT repeat due to FIRED bit.
        assert!(session.tick(21.0, 25.0).is_empty());

        // Advancing further hits 180m note; reset restores all FIRED bits cleanly.
        let spoken2 = session.tick(90.0, 25.0);
        assert_eq!(spoken2, vec!["90 DER"]);
        session.reset();
        assert_eq!(session.fired_count(), 0);
        assert!(session.current_text(Instant::now()).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn consecutive_calls_fire_in_order_and_detect_ogg_conversion_requirement() {
        let root = std::env::temp_dir().join(format!("rerbr-codriver-ogg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let spa = root.join("RBR/Audio/Speech/Spa");
        std::fs::create_dir_all(&spa).unwrap();
        std::fs::write(spa.join("90 left.ogg"), b"OggS-fake").unwrap();

        let markers = StageMarkers {
            keys: vec![key(1, 0, 50.0), key(5, 0, 60.0), key(TAG_FINISH, 0, 120.0)],
        };
        let mut session = CoDriverSession::from_markers(&root, markers);
        assert!(matches!(
            session.status,
            VoiceBankStatus::RequiresWavConversion { ogg_count: 1, .. }
        ));

        // 20 m/s -> 80m lookahead -> both 50m (gap 10m -> PARA) and 60m (gap 60m -> 50) fire cleanly in ascending order.
        let calls = session.tick(10.0, 20.0);
        assert_eq!(calls.len(), 2);
        assert!(calls[0].starts_with("90 IZQ PARA"));
        assert!(calls[1].starts_with("IZQ SUAVE"));

        // Finish line emits META once crossed (> 120.0).
        let finish_calls = session.tick(125.0, 20.0);
        assert_eq!(finish_calls, vec!["META"]);
        assert!(session.tick(130.0, 20.0).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolves_real_installed_dls_pacenotes_if_present_on_dev_pc() {
        let dls = Path::new(r"C:\Richard Burns Rally\Maps\track-22_O.dls");
        if !dls.is_file() {
            return;
        }
        let bytes = std::fs::read(dls).unwrap();
        let parsed = markers::parse(&bytes).unwrap();
        assert!(!parsed.keys.is_empty());
        let mut ticker = MarkerTicker::load(&parsed);
        let mut total_callouts = 0usize;
        let mut x = parsed.start().unwrap_or(0.0);
        let finish = parsed.finish().unwrap_or(5000.0);
        while x <= finish + 20.0 {
            let input = TickInput {
                x,
                dot150: 25.0,
                seek_d: ticker.seek_d(x),
                lookahead_scale: 4.0,
                pace_speed_threshold: 25.0,
                ..TickInput::default()
            };
            let (next, events) = ticker.tick(input);
            ticker = next;
            for ev in events {
                if let MarkerEvent::PaceCallout(c) = ev {
                    if rerbr_sim::rbr_gameplay::markers::spoken_tag(c.key.tag)
                        || c.key.tag == TAG_FINISH
                    {
                        total_callouts += 1;
                        assert!(
                            callout_spec(c.key.tag, c.slot_word_id).is_some(),
                            "unmapped tag {} (slot_word_id={:#x})",
                            c.key.tag,
                            c.slot_word_id
                        );
                    }
                }
            }
            x += 5.0;
        }
        assert!(
            total_callouts > 10,
            "Chirdonhead should produce >10 pace callouts"
        );
    }
}
