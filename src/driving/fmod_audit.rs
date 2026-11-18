//! Diagnostic inspection and resolution of RSF / FMOD Studio `.bank` files.
//!
//! Strictly honest status reporting: finding or parsing a `.bank` never claims
//! active audio output until Vorbis decoding and live sample emission succeed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rerbr_sim::rbr_formats::{cars, fmod};

use super::fsb5_vorbis_setup;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmodBankSupport {
    MissingBank {
        expected_paths: Vec<String>,
        available_audiofmod: Vec<String>,
    },
    MissingStringsBank {
        bank_found_at: PathBuf,
        expected_strings_rel: String,
        available_audiofmod: Vec<String>,
    },
    ReadError(String),
    FormatUnrecognized(String),
    UnsupportedVorbisCrc {
        vorbis_count: usize,
        pcm_count: usize,
        missing_crcs: Vec<u32>,
        params: usize,
    },
    PlayableReady {
        vorbis_count: usize,
        pcm_count: usize,
        unknown_codecs: usize,
        params: usize,
    },
}

#[derive(Debug, Clone)]
pub struct FmodBankReport {
    pub bank_path: String,
    pub resolved_bank_path: Option<PathBuf>,
    pub resolved_strings_path: Option<PathBuf>,
    pub support: FmodBankSupport,
    pub sample_names: Vec<String>,
    pub event_names: Vec<String>,
    pub param_names: Vec<String>,
}

impl FmodBankReport {
    pub fn is_playable(&self) -> bool {
        matches!(self.support, FmodBankSupport::PlayableReady { .. })
    }

    pub fn summary(&self) -> String {
        match &self.support {
            FmodBankSupport::MissingBank {
                expected_paths,
                available_audiofmod,
            } => {
                let expected = expected_paths.join(" o ");
                if available_audiofmod.is_empty() {
                    format!(
                        "FMOD NO DISPONIBLE - ARCHIVO .BANK NO ENCONTRADO (SE ESPERA {expected}; CARPETA AudioFMOD VACIA O AUSENTE)"
                    )
                } else {
                    format!(
                        "FMOD NO DISPONIBLE - FALTA {expected} | AudioFMOD TIENE: {}",
                        available_audiofmod.join(", ")
                    )
                }
            }
            FmodBankSupport::MissingStringsBank {
                expected_strings_rel,
                available_audiofmod,
                ..
            } => {
                if available_audiofmod.is_empty() {
                    format!(
                        "FMOD NO DISPONIBLE - .BANK ENCONTRADO PERO FALTA {expected_strings_rel}"
                    )
                } else {
                    format!(
                        "FMOD NO DISPONIBLE - FALTA {expected_strings_rel} | AudioFMOD TIENE: {}",
                        available_audiofmod.join(", ")
                    )
                }
            }
            FmodBankSupport::ReadError(reason) => {
                format!("FMOD NO DISPONIBLE - ERROR DE LECTURA ({reason})")
            }
            FmodBankSupport::FormatUnrecognized(reason) => {
                format!("FMOD NO DISPONIBLE - FORMATO NO RECONOCIDO ({reason})")
            }
            FmodBankSupport::UnsupportedVorbisCrc {
                vorbis_count,
                pcm_count,
                missing_crcs,
                params,
            } => {
                let crc_list: Vec<String> =
                    missing_crcs.iter().map(|c| format!("{c:#010x}")).collect();
                format!(
                    "FMOD FSB5 VORBIS ({vorbis_count} VORBIS / {pcm_count} PCM / {params} PARAMS - CRC SETUP DESCONOCIDO: {})",
                    crc_list.join(", ")
                )
            }
            FmodBankSupport::PlayableReady {
                vorbis_count,
                pcm_count,
                unknown_codecs,
                params,
            } => {
                if *unknown_codecs > 0 {
                    format!(
                        "FMOD BANK LISTO ({vorbis_count} VORBIS + {pcm_count} PCM / {unknown_codecs} CODEC DESCONOCIDO / {params} PARAMS) - PENDIENTE DE MOTOR AUDIO"
                    )
                } else {
                    format!(
                        "FMOD BANK LISTO ({vorbis_count} VORBIS + {pcm_count} PCM / {params} PARAMS) - PENDIENTE DE MOTOR AUDIO"
                    )
                }
            }
        }
    }
}

pub fn expected_strings_rel(bank_rel: &str) -> Option<String> {
    let stripped = bank_rel
        .strip_suffix(".bank")
        .or_else(|| bank_rel.strip_suffix(".BANK"))?;
    Some(format!("{stripped}.strings.bank"))
}

pub fn inspect_bank_in_documents(documents: &Path, bank_rel: &str) -> FmodBankReport {
    let rbr_root = documents.join("RBR");
    inspect_bank_with_roots(&[&rbr_root, documents], bank_rel)
}

pub fn inspect_bank(rbr_root: &Path, bank_rel: &str) -> FmodBankReport {
    inspect_bank_with_roots(&[rbr_root], bank_rel)
}

/// Resolves the `.bank` and companion `.strings.bank` across ordered search roots
/// (`Documents/RBR` then `Documents`), returning resolved paths + AudioFMOD file listing.
pub fn resolve_bank_pair(
    roots: &[&Path],
    bank_rel: &str,
) -> (Option<PathBuf>, Option<PathBuf>, Vec<String>) {
    let bank_path = roots.iter().find_map(|root| cars::resolve(root, bank_rel));
    let strings_rel = expected_strings_rel(bank_rel);
    let mut strings_candidates = Vec::new();
    if let Some(s) = &strings_rel {
        strings_candidates.push(s.as_str());
    }
    strings_candidates.push("AudioFMOD/Master.strings.bank");
    strings_candidates.push("AudioFMOD/Master Bank.strings.bank");

    let strings_path = bank_path
        .as_deref()
        .and_then(|bp| {
            let parent = bp.parent()?;
            let sname = strings_rel
                .as_deref()
                .and_then(|r| Path::new(r).file_name())
                .and_then(|n| n.to_str());
            if let Some(file_name) = sname {
                if let Some(hit) = cars::resolve(parent, file_name) {
                    return Some(hit);
                }
            }
            None
        })
        .or_else(|| {
            roots.iter().find_map(|root| {
                strings_candidates
                    .iter()
                    .find_map(|rel| cars::resolve(root, rel))
            })
        });

    let available_audiofmod = list_audiofmod_entries(roots, bank_rel);
    (bank_path, strings_path, available_audiofmod)
}

pub fn find_external_vorbis_setup(roots: &[&Path], crc: u32) -> Option<Vec<u8>> {
    let rels = [
        format!("AudioFMOD/{crc:08x}.fvs"),
        format!("AudioFMOD/.fvs_{crc:08x}"),
        format!("AudioFMOD/vorbis_{crc:08x}.setup"),
    ];
    for root in roots {
        for rel in &rels {
            if let Some(path) = cars::resolve(root, rel) {
                if let Ok(bytes) = std::fs::read(path) {
                    if !bytes.is_empty() {
                        return Some(bytes);
                    }
                }
            }
        }
    }
    None
}

fn list_audiofmod_entries(roots: &[&Path], bank_rel: &str) -> Vec<String> {
    let dir_rel = Path::new(bank_rel)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("AudioFMOD");
    let mut seen = BTreeSet::new();
    for root in roots {
        if let Some(dir) = cars::resolve(root, dir_rel) {
            if let Ok(read_dir) = std::fs::read_dir(dir) {
                for entry in read_dir.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        seen.insert(name.to_string());
                    }
                }
            }
        }
    }
    seen.into_iter().take(12).collect()
}

pub fn inspect_bank_with_roots(roots: &[&Path], bank_rel: &str) -> FmodBankReport {
    let normalized_rel = bank_rel.replace('\\', "/");
    let expected_paths = if roots.len() > 1 {
        vec![
            format!("Documents/RBR/{normalized_rel}"),
            format!("Documents/{normalized_rel}"),
        ]
    } else {
        vec![format!("Documents/RBR/{normalized_rel}")]
    };
    let expected_str_rel = expected_strings_rel(&normalized_rel)
        .unwrap_or_else(|| format!("{normalized_rel}.strings.bank"));

    let (resolved_bank_path, resolved_strings_path, available_audiofmod) =
        resolve_bank_pair(roots, &normalized_rel);

    let Some(bank_file) = resolved_bank_path.clone() else {
        return FmodBankReport {
            bank_path: normalized_rel,
            resolved_bank_path: None,
            resolved_strings_path: None,
            support: FmodBankSupport::MissingBank {
                expected_paths,
                available_audiofmod,
            },
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        };
    };

    let bank_bytes = match std::fs::read(&bank_file) {
        Ok(b) => b,
        Err(e) => {
            return FmodBankReport {
                bank_path: normalized_rel,
                resolved_bank_path: Some(bank_file),
                resolved_strings_path,
                support: FmodBankSupport::ReadError(e.to_string()),
                sample_names: Vec::new(),
                event_names: Vec::new(),
                param_names: Vec::new(),
            };
        }
    };

    let Some(str_file) = resolved_strings_path.clone() else {
        return FmodBankReport {
            bank_path: normalized_rel,
            resolved_bank_path: Some(bank_file.clone()),
            resolved_strings_path: None,
            support: FmodBankSupport::MissingStringsBank {
                bank_found_at: bank_file,
                expected_strings_rel: format!("Documents/RBR/{expected_str_rel}"),
                available_audiofmod,
            },
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        };
    };

    let strings_bytes = match std::fs::read(&str_file) {
        Ok(b) => b,
        Err(e) => {
            return FmodBankReport {
                bank_path: normalized_rel,
                resolved_bank_path: Some(bank_file),
                resolved_strings_path: Some(str_file),
                support: FmodBankSupport::ReadError(e.to_string()),
                sample_names: Vec::new(),
                event_names: Vec::new(),
                param_names: Vec::new(),
            };
        }
    };

    match fmod::parse(bank_bytes, &strings_bytes) {
        Ok(bank) => {
            let mut vorbis_count = 0usize;
            let mut pcm_count = 0usize;
            let mut unknown_codecs = 0usize;
            let mut missing_crcs = BTreeSet::new();
            for s in &bank.samples {
                match s.codec {
                    fmod::VORBIS => {
                        vorbis_count += 1;
                        match s.vorbis_crc {
                            Some(crc) => {
                                if fsb5_vorbis_setup::setup_header_for_crc(crc).is_none()
                                    && find_external_vorbis_setup(roots, crc).is_none()
                                {
                                    missing_crcs.insert(crc);
                                }
                            }
                            None => {
                                missing_crcs.insert(0);
                            }
                        }
                    }
                    fmod::PCM16 => pcm_count += 1,
                    _ => unknown_codecs += 1,
                }
            }
            let support = if !missing_crcs.is_empty() {
                FmodBankSupport::UnsupportedVorbisCrc {
                    vorbis_count,
                    pcm_count,
                    missing_crcs: missing_crcs.into_iter().collect(),
                    params: bank.params.len(),
                }
            } else {
                FmodBankSupport::PlayableReady {
                    vorbis_count,
                    pcm_count,
                    unknown_codecs,
                    params: bank.params.len(),
                }
            };
            FmodBankReport {
                bank_path: normalized_rel,
                resolved_bank_path: Some(bank_file),
                resolved_strings_path: Some(str_file),
                support,
                sample_names: bank.samples.iter().map(|s| s.name.clone()).collect(),
                event_names: bank.events.iter().map(|e| e.name.clone()).collect(),
                param_names: bank
                    .params
                    .iter()
                    .map(|p| format!("{}[{:.0}..{:.0}]", p.name, p.min, p.max))
                    .collect(),
            }
        }
        Err(e) => FmodBankReport {
            bank_path: normalized_rel,
            resolved_bank_path: Some(bank_file),
            resolved_strings_path: Some(str_file),
            support: FmodBankSupport::FormatUnrecognized(e.to_string()),
            sample_names: Vec::new(),
            event_names: Vec::new(),
            param_names: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rerbr_fmod_audit_{tag}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reports_exact_expected_path_when_bank_missing() {
        let root = temp_dir("missing_bank");
        std::fs::create_dir_all(root.join("RBR/AudioFMOD")).unwrap();
        std::fs::write(root.join("RBR/AudioFMOD/OtherCar.bank"), b"dummy").unwrap();

        let report = inspect_bank_in_documents(&root, "AudioFMOD/Focus_Mk2_WRC.bank");
        assert!(!report.is_playable());
        assert!(matches!(
            report.support,
            FmodBankSupport::MissingBank { .. }
        ));
        let summary = report.summary();
        assert!(
            summary.contains("Documents/RBR/AudioFMOD/Focus_Mk2_WRC.bank"),
            "{summary}"
        );
        assert!(summary.contains("OtherCar.bank"), "{summary}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reports_missing_strings_bank_separately() {
        let root = temp_dir("missing_strings");
        std::fs::create_dir_all(root.join("RBR/AudioFMOD")).unwrap();
        std::fs::write(root.join("RBR/AudioFMOD/Focus_Mk2_WRC.bank"), b"RIFF").unwrap();

        let report = inspect_bank_in_documents(&root, "AudioFMOD/Focus_Mk2_WRC.bank");
        assert!(!report.is_playable());
        assert!(matches!(
            report.support,
            FmodBankSupport::MissingStringsBank { .. }
        ));
        let summary = report.summary();
        assert!(summary.contains("Focus_Mk2_WRC.strings.bank"), "{summary}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_bank_in_documents_fallback_root() {
        let root = temp_dir("fallback_root");
        std::fs::create_dir_all(root.join("AudioFMOD")).unwrap();
        std::fs::write(root.join("AudioFMOD/focus_mk2_wrc.bank"), b"NOT_RIFF").unwrap();
        std::fs::write(
            root.join("AudioFMOD/Focus_Mk2_WRC.strings.bank"),
            b"NOT_RIFF",
        )
        .unwrap();

        let report = inspect_bank_in_documents(&root, "AudioFMOD/Focus_Mk2_WRC.bank");
        assert!(report.resolved_bank_path.is_some());
        assert!(report.resolved_strings_path.is_some());
        assert!(matches!(
            report.support,
            FmodBankSupport::FormatUnrecognized(_)
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn inspects_real_rsf_fmod_banks_on_pc_if_present() {
        let rbr = Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("AudioFMOD/Focus_Mk2_WRC.bank").is_file() {
            return;
        }
        let report = inspect_bank(rbr, "AudioFMOD/Focus_Mk2_WRC.bank");
        assert!(report.is_playable(), "{}", report.summary());
        assert!(matches!(
            report.support,
            FmodBankSupport::PlayableReady {
                vorbis_count: 44,
                pcm_count: 0,
                unknown_codecs: 0,
                ..
            }
        ));
    }

    #[test]
    fn census_all_rsf_audiofmod_banks_on_pc() {
        let dir = Path::new(r"C:\Richard Burns Rally\AudioFMOD");
        if !dir.is_dir() {
            return;
        }
        let rbr = Path::new(r"C:\Richard Burns Rally");
        let mut playable = 0usize;
        let mut unsupported_crc = 0usize;
        let mut other = 0usize;
        for ent in std::fs::read_dir(dir).unwrap().flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.ends_with(".bank") || name.ends_with(".strings.bank") {
                continue;
            }
            let rep = inspect_bank(rbr, &format!("AudioFMOD/{name}"));
            match rep.support {
                FmodBankSupport::PlayableReady { .. } => playable += 1,
                FmodBankSupport::UnsupportedVorbisCrc { .. } => {
                    eprintln!("UNSUPPORTED CRC BANK: {name}: {}", rep.summary());
                    unsupported_crc += 1;
                }
                _ => {
                    eprintln!("OTHER BANK: {name}: {}", rep.summary());
                    other += 1;
                }
            }
        }
        assert!(
            playable >= 50,
            "expected RSF banks playable, got {playable}"
        );
        assert_eq!(unsupported_crc, 0);
        assert_eq!(other, 0);
    }
}
