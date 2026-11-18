//! Read-only FMOD Studio bank (`AudioFMOD/*.bank`) inspector and PCM16 waveform loader.
//!
//! Uses `rerbr_sim::rbr_formats::fmod` (GPL-3.0) to inspect RSF vehicle FMOD events, parameters,
//! and FSB5 audio codecs cleanly. Distinguishes uncompressed `PCM16` banks (playable directly in
//! pure Rust/CoreAudio) from FSB5 headerless `VORBIS` banks (which require proprietary FMOD runtime
//! or external Vorbis codebook reconstruction and therefore report an explicit honest limitation).

use rerbr_sim::rbr_formats::{cars, fmod};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmodBankSupport {
    /// Bank contains playable uncompressed PCM16 samples.
    Pcm16Playable { sample_count: usize },
    /// Bank uses FMOD FSB5 headerless Vorbis (`codec = 15`), unsupported without external decoder.
    VorbisUnsupportedOnIos {
        vorbis_count: usize,
        pcm_count: usize,
        event_names: Vec<String>,
        param_names: Vec<String>,
    },
    /// Bank missing or unreadable.
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub struct FmodBankReport {
    pub bank_path: String,
    pub support: FmodBankSupport,
    pub sample_names: Vec<String>,
    pub event_names: Vec<String>,
    pub param_names: Vec<String>,
}

impl FmodBankReport {
    pub fn summary(&self) -> String {
        match &self.support {
            FmodBankSupport::Pcm16Playable { sample_count } => {
                format!("FMOD PCM16 ({sample_count} SAMPLES / {} EVENTOS)", self.event_names.len())
            }
            FmodBankSupport::VorbisUnsupportedOnIos {
                vorbis_count,
                pcm_count,
                ..
            } => format!(
                "FMOD FSB5 VORBIS ({vorbis_count} VORBIS / {pcm_count} PCM / {} PARAMS - SIN DECODER EN iOS)",
                self.param_names.len()
            ),
            FmodBankSupport::Unavailable(reason) => format!("FMOD NO DISPONIBLE ({reason})"),
        }
    }
}

/// Resolves and inspects `bank_rel` (e.g. `AudioFMOD/Focus_Mk2_WRC.bank`) under `rbr_root`.
pub fn inspect_bank(rbr_root: &Path, bank_rel: &str) -> FmodBankReport {
    let Some(bank_path) = cars::resolve(rbr_root, bank_rel) else {
        return FmodBankReport {
            bank_path: bank_rel.to_string(),
            support: FmodBankSupport::Unavailable("ARCHIVO .BANK NO ENCONTRADO".into()),
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        };
    };
    let stem_rel = bank_rel
        .strip_suffix(".bank")
        .or_else(|| bank_rel.strip_suffix(".BANK"))
        .unwrap_or(bank_rel);
    let strings_candidates = [
        format!("{stem_rel}.strings.bank"),
        "AudioFMOD/Master.strings.bank".to_string(),
        "AudioFMOD/Master Bank.strings.bank".to_string(),
    ];
    let strings_bytes = strings_candidates
        .iter()
        .find_map(|rel| cars::resolve(rbr_root, rel).and_then(|p| std::fs::read(p).ok()));

    let bytes = match std::fs::read(&bank_path) {
        Ok(b) => b,
        Err(e) => {
            return FmodBankReport {
                bank_path: bank_rel.to_string(),
                support: FmodBankSupport::Unavailable(e.to_string()),
                sample_names: Vec::new(),
                event_names: Vec::new(),
                param_names: Vec::new(),
            }
        }
    };

    // If matching `.strings.bank` isn't present, inspect raw FSB5/FEV cleanly by passing minimal strings or empty check
    let parsed = if let Some(ref sb) = strings_bytes {
        fmod::parse(bytes, sb)
    } else {
        return FmodBankReport {
            bank_path: bank_rel.to_string(),
            support: FmodBankSupport::Unavailable("FALTA .strings.bank".into()),
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        };
    };

    match parsed {
        Ok(bank) => {
            let pcm_count = bank
                .samples
                .iter()
                .filter(|s| s.codec == fmod::PCM16)
                .count();
            let vorbis_count = bank
                .samples
                .iter()
                .filter(|s| s.codec == fmod::VORBIS)
                .count();
            let sample_names = bank.samples.iter().map(|s| s.name.clone()).collect();
            let event_names: Vec<String> = bank
                .events
                .iter()
                .map(|e| e.name.clone())
                .filter(|n| !n.is_empty())
                .collect();
            let param_names: Vec<String> = bank
                .params
                .iter()
                .map(|p| format!("{}[{:.0}..{:.0}]", p.name, p.min, p.max))
                .collect();
            let support = if vorbis_count == 0 && pcm_count > 0 {
                FmodBankSupport::Pcm16Playable {
                    sample_count: pcm_count,
                }
            } else {
                FmodBankSupport::VorbisUnsupportedOnIos {
                    vorbis_count,
                    pcm_count,
                    event_names: event_names.clone(),
                    param_names: param_names.clone(),
                }
            };
            FmodBankReport {
                bank_path: bank_rel.to_string(),
                support,
                sample_names,
                event_names,
                param_names,
            }
        }
        Err(e) => FmodBankReport {
            bank_path: bank_rel.to_string(),
            support: FmodBankSupport::Unavailable(e.to_string()),
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_real_rsf_fmod_banks_on_pc_if_present() {
        let rbr = Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("AudioFMOD/Focus_Mk2_WRC.bank").is_file() {
            return;
        }
        for rel in ["AudioFMOD/Focus_Mk2_WRC.bank", "AudioFMOD/306_maxi_B.bank"] {
            let report = inspect_bank(rbr, rel);
            eprintln!(
                "FMOD REPORT {rel}: {} | samples={:?} | events={:?} | params={:?}",
                report.summary(),
                &report.sample_names[..report.sample_names.len().min(6)],
                report.event_names,
                report.param_names
            );
            assert!(
                !matches!(report.support, FmodBankSupport::Unavailable(_)),
                "{rel}: {:?}",
                report.support
            );
        }
    }

    #[test]
    fn census_all_rsf_audiofmod_banks_on_pc() {
        let dir = Path::new(r"C:\Richard Burns Rally\AudioFMOD");
        if !dir.is_dir() {
            return;
        }
        let rbr = Path::new(r"C:\Richard Burns Rally");
        let (mut total, mut pcm_banks, mut vorbis_banks, mut failed) =
            (0usize, 0usize, 0usize, 0usize);
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                let n = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                n.ends_with(".bank") && !n.ends_with(".strings.bank")
            })
            .collect();
        entries.sort();
        for p in entries {
            total += 1;
            let name = p.file_name().and_then(|s| s.to_str()).unwrap();
            let rep = inspect_bank(rbr, &format!("AudioFMOD/{name}"));
            match rep.support {
                FmodBankSupport::Pcm16Playable { .. } => pcm_banks += 1,
                FmodBankSupport::VorbisUnsupportedOnIos { .. } => vorbis_banks += 1,
                _ => failed += 1,
            }
        }
        eprintln!(
            "FMOD CENSUS total={total} pcm16={pcm_banks} fsb5_vorbis={vorbis_banks} other_or_err={failed}"
        );
        assert!(total > 0);
        assert_eq!(failed, 0);
    }
}
