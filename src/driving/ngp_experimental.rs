//! Staged experimental NGP diagnostics and isolated loader for non-retail pack vehicles
//! (Ford Focus Mk II RS WRC 2006).
//!
//! Strictly separated from certified `OriginalRetail` gameplay: never substitutes another
//! retail car's physics, cleanly isolates unsupported FMOD audio, and permits safe visual
//! inspection when NGP runtime physics are unavailable.

use super::ngp_proto;
use rerbr_content::VehicleDescriptor;
use rerbr_sim::rbr_formats::{car_pack::Pack, cars};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Unsupported,
    Blocked,
    Pending,
}

impl DiagnosticStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "VISUAL",
            Self::Unsupported => "UNSUPPORTED",
            Self::Blocked => "BLOQUEADO",
            Self::Pending => "PENDIENTE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticStep {
    pub stage: &'static str,
    pub status: DiagnosticStatus,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct FocusDiagnosticsReport {
    pub vehicle_name: String,
    pub pack_path: Option<PathBuf>,
    pub steps: Vec<DiagnosticStep>,
}

#[derive(Debug, Clone, Default)]
pub struct LiveRuntimeState<'a> {
    pub worker_error: Option<&'a str>,
    pub physics_settled: bool,
    pub physics_status: Option<&'a str>,
    pub sim_steps: Option<u32>,
    pub speed_mps: f64,
    pub rpm: f32,
    pub gear: i32,
    pub throttle_rx: f32,
    pub steering_rx: f32,
    pub clutch_rx: f32,
    pub clutch_help: bool,
    pub session_active: bool,
    pub stage_id: &'a str,
    pub model_path: &'a str,
    pub audio_status: &'a str,
}

fn detected_fallback_retail_car(vehicle: &VehicleDescriptor) -> Option<&'static str> {
    if !vehicle.is_ngp_experimental_car() {
        return None;
    }
    rerbr_content::VehicleDescriptor::presets()
        .into_iter()
        .find(|p| {
            !p.is_ngp_experimental_car() && rerbr_content::same_path(&p.model, &vehicle.model)
        })
        .map(|_| match vehicle.model.to_ascii_lowercase().as_str() {
            m if m.contains("impreza03") => "Subaru Impreza 2003 (SLOT 5)",
            m if m.contains("impreza00") => "Subaru Impreza 2000",
            m if m.contains("corolla") => "Toyota Corolla WRC",
            _ => "COCHE ORIGINAL FALLBACK",
        })
}

impl FocusDiagnosticsReport {
    pub fn inspect(documents: Option<&Path>, vehicle: &VehicleDescriptor) -> Self {
        let mut steps = Vec::with_capacity(9);
        let Some(doc_root) = documents else {
            return Self::with_missing_documents(vehicle);
        };
        let rbr_root = doc_root.join("RBR");
        let fallback_car = detected_fallback_retail_car(vehicle);

        // 1. CAR FILES
        let required_ok = vehicle
            .required_paths()
            .filter(|p| !rerbr_content::same_path(p, &vehicle.index))
            .filter(|p| cars::resolve(&rbr_root, p).is_none())
            .collect::<Vec<_>>();
        let car_files_ok = required_ok.is_empty() && fallback_car.is_none();
        steps.push(DiagnosticStep {
            stage: "CAR FILES",
            status: if car_files_ok {
                DiagnosticStatus::Ok
            } else if fallback_car.is_some() && required_ok.is_empty() {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if let Some(fb) = fallback_car {
                format!("FALLBACK -> {fb} (FALTA MODELO PROPIO EN Cars/)")
            } else if car_files_ok {
                format!("{} OK", vehicle.model)
            } else {
                format!("FALTA {}", required_ok[0])
            },
        });

        // 2. CAR MODEL (3D SGC + shaders + DDS textures)
        let car_model_ok = if required_ok.is_empty() {
            match crate::car::load_vehicle_for_mode(&rbr_root, vehicle, true) {
                Ok(car) => {
                    if let Some(fb) = fallback_car {
                        steps.push(DiagnosticStep {
                            stage: "CAR MODEL",
                            status: DiagnosticStatus::Warning,
                            detail: format!(
                                "MODELO FALLBACK {fb} ({} DIBUJOS / {} DDS)",
                                car.geometry.drawn.len(),
                                car.textures.len()
                            ),
                        });
                    } else {
                        steps.push(DiagnosticStep {
                            stage: "CAR MODEL",
                            status: DiagnosticStatus::Ok,
                            detail: format!(
                                "{} -> {} DIBUJOS / {} DDS",
                                vehicle.model,
                                car.geometry.drawn.len(),
                                car.textures.len()
                            ),
                        });
                    }
                    fallback_car.is_none()
                }
                Err(err) => {
                    steps.push(DiagnosticStep {
                        stage: "CAR MODEL",
                        status: DiagnosticStatus::Blocked,
                        detail: err,
                    });
                    false
                }
            }
        } else {
            steps.push(DiagnosticStep {
                stage: "CAR MODEL",
                status: DiagnosticStatus::Blocked,
                detail: "REQUIERE CAR FILES COMPLETOS".into(),
            });
            false
        };

        // 3. CAR PACK (rsfdata/cars/<pack> or Ford_Focus_*_ngp6)
        let pack_path = find_pack_for(&rbr_root, vehicle);
        let mut car_pack_ok = false;
        if let Some(ref p) = pack_path {
            match Pack::open(p).and_then(|pack| {
                pack.read("common.lsp")?;
                pack.read("gravel.lsp")?;
                Ok(())
            }) {
                Ok(()) => {
                    car_pack_ok = true;
                    let summary = ngp_proto::inspect_pack_path(p)
                        .or_else(|| ngp_proto::inspect_vehicle_in_documents(doc_root, vehicle))
                        .map(|s| format!("{} | {:.0}KG", s.physics_summary(), s.mass_kg))
                        .unwrap_or_else(|| "PACK NGP LEGIBLE".into());
                    steps.push(DiagnosticStep {
                        stage: "CAR PACK",
                        status: DiagnosticStatus::Ok,
                        detail: summary,
                    });
                }
                Err(err) => {
                    steps.push(DiagnosticStep {
                        stage: "CAR PACK",
                        status: DiagnosticStatus::Blocked,
                        detail: err,
                    });
                }
            }
        } else {
            let expected = vehicle.rsf_pack.as_deref().unwrap_or("Ford_Focus_*_ngp6");
            steps.push(DiagnosticStep {
                stage: "CAR PACK",
                status: DiagnosticStatus::Blocked,
                detail: format!("NO ENCONTRADO rsfdata/cars/{expected}"),
            });
        }

        // 4. NGP DATA (Plugins/NGP.dll + physics.rbz revision)
        let ngp_rt = rerbr_sim::ngp_candidate::inspect_runtime_dir(&rbr_root);
        let physics_rbz = cars::resolve(&rbr_root, "physics.rbz");
        let ngp_74_verified = physics_rbz.is_some()
            && ngp_rt.kind == rerbr_sim::ngp_candidate::NgpRuntimeKind::Verified74;
        steps.push(DiagnosticStep {
            stage: "NGP DATA",
            status: if ngp_74_verified {
                DiagnosticStatus::Ok
            } else if ngp_rt.kind == rerbr_sim::ngp_candidate::NgpRuntimeKind::Detected75Unverified
            {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if physics_rbz.is_none() {
                "FALTA physics.rbz".into()
            } else {
                ngp_rt.summary.clone()
            },
        });

        // 5. NGP BACKEND
        let backend_ok = car_pack_ok && ngp_74_verified;
        steps.push(DiagnosticStep {
            stage: "NGP BACKEND",
            status: if backend_ok {
                DiagnosticStatus::Ok
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok {
                "NGP 7.4 EXPERIMENTAL (PACK)".into()
            } else if car_model_ok {
                format!("SIN BACKEND ACTIVO ({})", ngp_rt.summary)
            } else {
                "BLOQUEADO (SIN DATOS NGP/MODELO)".into()
            },
        });

        // 6. PHYSICS INITIALIZATION (always Pending until live worker settles)
        steps.push(DiagnosticStep {
            stage: "PHYSICS INITIALIZATION",
            status: if backend_ok || !vehicle.is_ngp_experimental_car() {
                DiagnosticStatus::Pending
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok || !vehicle.is_ngp_experimental_car() {
                "PENDIENTE DE INICIO (SIN SESIÓN ACTIVA)".into()
            } else {
                "SIN FÍSICA NGP ACTIVA".into()
            },
        });

        // 7. VEHICLE SPAWN (always Pending until live 720 Hz steps run)
        steps.push(DiagnosticStep {
            stage: "VEHICLE SPAWN",
            status: if backend_ok || !vehicle.is_ngp_experimental_car() {
                DiagnosticStatus::Pending
            } else if car_model_ok {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok || !vehicle.is_ngp_experimental_car() {
                "PENDIENTE DE SPAWN EN TRAMO".into()
            } else if car_model_ok {
                "SPAWN VISUAL ESTÁTICO EN SALIDA".into()
            } else {
                "SIN MODELO PARA SPAWN".into()
            },
        });

        // 8. CONTROLS (always Pending until driving loop delivers inputs)
        steps.push(DiagnosticStep {
            stage: "CONTROLS",
            status: DiagnosticStatus::Pending,
            detail: "PENDIENTE DE SESIÓN DE CONDUCCIÓN".into(),
        });

        // 9. AUDIO (FMOD bank / engine bank status)
        let bank_present = cars::resolve(&rbr_root, &vehicle.audio_index).is_some();
        let is_fmod_bank = vehicle.audio_index.to_ascii_lowercase().ends_with(".bank");
        let audio_report = if bank_present && is_fmod_bank {
            let rep = super::fmod_audit::inspect_bank(&rbr_root, &vehicle.audio_index);
            (DiagnosticStatus::Unsupported, rep.summary())
        } else if !vehicle.is_ngp_experimental_car() && bank_present {
            (
                DiagnosticStatus::Pending,
                format!("RETAIL ({}) - PENDIENTE DE BOOT", vehicle.audio_index),
            )
        } else {
            (
                DiagnosticStatus::Unsupported,
                if bank_present {
                    "AUDIO: UNSUPPORTED - FMOD BANK (SILENCIO)".into()
                } else {
                    "AUDIO: UNSUPPORTED - SIN FMOD BANK (SILENCIO)".into()
                },
            )
        };
        steps.push(DiagnosticStep {
            stage: "AUDIO",
            status: audio_report.0,
            detail: audio_report.1,
        });

        // 10. DRIVING (live state, never pre-marked OK)
        steps.push(DiagnosticStep {
            stage: "DRIVING",
            status: if backend_ok || !vehicle.is_ngp_experimental_car() {
                DiagnosticStatus::Pending
            } else if car_model_ok {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok || !vehicle.is_ngp_experimental_car() {
                "PENDIENTE DE INICIO EN TRAMO".into()
            } else if car_model_ok {
                "SOLO INSPECCIÓN VISUAL (SIN FÍSICA NGP)".into()
            } else {
                "NO DISPONIBLE".into()
            },
        });

        Self {
            vehicle_name: vehicle.name.clone(),
            pack_path,
            steps,
        }
    }

    fn with_missing_documents(vehicle: &VehicleDescriptor) -> Self {
        let stages = [
            "CAR FILES",
            "CAR MODEL",
            "CAR PACK",
            "NGP DATA",
            "NGP BACKEND",
            "PHYSICS INITIALIZATION",
            "VEHICLE SPAWN",
            "CONTROLS",
            "AUDIO",
            "DRIVING",
        ];
        let steps = stages
            .into_iter()
            .map(|stage| DiagnosticStep {
                stage,
                status: if stage == "AUDIO" {
                    DiagnosticStatus::Unsupported
                } else {
                    DiagnosticStatus::Blocked
                },
                detail: if stage == "AUDIO" {
                    "AUDIO: UNSUPPORTED - FMOD BANK".into()
                } else {
                    "DOCUMENTS NO DISPONIBLE".into()
                },
            })
            .collect();
        Self {
            vehicle_name: vehicle.name.clone(),
            pack_path: None,
            steps,
        }
    }

    pub fn set_step(&mut self, stage: &str, status: DiagnosticStatus, detail: impl Into<String>) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.stage == stage) {
            step.status = status;
            step.detail = detail.into();
        }
    }

    pub fn record_physics_ready(&mut self) {
        self.set_step(
            "PHYSICS INITIALIZATION",
            DiagnosticStatus::Ok,
            "NGP 7.4 PACK ACTIVO",
        );
        self.set_step(
            "VEHICLE SPAWN",
            DiagnosticStatus::Ok,
            "VEHÍCULO POSICIONADO EN SALIDA",
        );
        self.set_step(
            "CONTROLS",
            DiagnosticStatus::Ok,
            "CONTROLES ENLAZADOS AL WORKER 720HZ",
        );
        self.set_step(
            "DRIVING",
            DiagnosticStatus::Ok,
            "CONDUCCIÓN EXPERIMENTAL ACTIVA",
        );
    }

    pub fn record_physics_failure(&mut self, error: &str) {
        self.set_step(
            "PHYSICS INITIALIZATION",
            DiagnosticStatus::Blocked,
            error.to_string(),
        );
        self.set_step(
            "VEHICLE SPAWN",
            DiagnosticStatus::Warning,
            "SPAWN VISUAL ESTÁTICO SIN FÍSICA NGP",
        );
        self.set_step(
            "CONTROLS",
            DiagnosticStatus::Warning,
            "CONTROLES INACTIVOS (MODO VISUAL)",
        );
        self.set_step(
            "DRIVING",
            DiagnosticStatus::Warning,
            "MODO VISUAL - SIN FÍSICA NGP",
        );
    }

    /// Updates stages 6..=10 (Physics, Spawn, Controls, Audio, Driving) strictly from live runtime state.
    pub fn apply_live_state(&mut self, live: &LiveRuntimeState<'_>) {
        if let Some(err) = live.worker_error {
            self.record_physics_failure(err);
        } else if live.physics_settled {
            self.set_step(
                "PHYSICS INITIALIZATION",
                DiagnosticStatus::Ok,
                live.physics_status.unwrap_or("FÍSICA INICIALIZADA (720HZ)"),
            );
            if let Some(step_count) = live.sim_steps.filter(|&n| n > 0) {
                self.set_step(
                    "VEHICLE SPAWN",
                    DiagnosticStatus::Ok,
                    format!(
                        "SPAWN ACTIVO (STEP {step_count} · {:.1} m/s · G{})",
                        live.speed_mps, live.gear
                    ),
                );
            } else {
                self.set_step(
                    "VEHICLE SPAWN",
                    DiagnosticStatus::Pending,
                    "ESPERANDO PRIMER STEP 720HZ",
                );
            }
            if live.session_active {
                self.set_step(
                    "CONTROLS",
                    DiagnosticStatus::Ok,
                    format!(
                        "THR {:.2} STR {:.2} CLU {:.2} AYUDA {}",
                        live.throttle_rx,
                        live.steering_rx,
                        live.clutch_rx,
                        if live.clutch_help { "ON" } else { "OFF" }
                    ),
                );
                self.set_step(
                    "DRIVING",
                    DiagnosticStatus::Ok,
                    format!(
                        "SESIÓN ACTIVA ({}) · {:.0} RPM · MODELO {}",
                        live.stage_id, live.rpm, live.model_path
                    ),
                );
            }
        }
        if !live.audio_status.is_empty() && live.audio_status != "AUDIO UNINITIALIZED" {
            let audio_st = if live.audio_status.contains("AUTHENTIC") {
                DiagnosticStatus::Ok
            } else if live.audio_status.contains("FMOD") {
                DiagnosticStatus::Unsupported
            } else {
                DiagnosticStatus::Warning
            };
            self.set_step("AUDIO", audio_st, live.audio_status);
        }
    }

    pub fn export_text(&self, extra_lines: &[String]) -> String {
        let mut lines = Vec::with_capacity(self.steps.len() + extra_lines.len() + 6);
        lines.push("=== RERBR iOS DIAGNOSTICO RUNTIME (10 ETAPAS) ===".to_string());
        lines.push(format!("VEHICULO: {}", self.vehicle_name));
        lines.push(format!(
            "PACK NGP: {}",
            self.pack_path
                .as_ref()
                .map_or_else(|| "NINGUNO / RETAIL".into(), |p| p.display().to_string())
        ));
        lines.push("--- ETAPAS ---".to_string());
        for (i, step) in self.steps.iter().enumerate() {
            lines.push(format!(
                "{:02}. {:<22} [{}] {}",
                i + 1,
                step.stage,
                step.status.label(),
                step.detail
            ));
        }
        if !extra_lines.is_empty() {
            lines.push("--- CONTEXTO RUNTIME ---".to_string());
            lines.extend(extra_lines.iter().cloned());
        }
        lines.join("\n") + "\n"
    }

    pub fn write_export_file(
        &self,
        documents: &Path,
        extra_lines: &[String],
    ) -> Result<PathBuf, String> {
        let out = documents.join("rerbr-diagnostico.txt");
        let text = self.export_text(extra_lines);
        std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
        Ok(out)
    }

    pub fn can_enter_visual_mode(&self) -> bool {
        let ok = |name: &str| {
            self.steps
                .iter()
                .find(|s| s.stage == name)
                .is_some_and(|s| s.status == DiagnosticStatus::Ok)
        };
        ok("CAR FILES") && ok("CAR MODEL")
    }

    pub fn can_boot_ngp_pack(&self) -> Option<&PathBuf> {
        let ok = |name: &str| {
            self.steps
                .iter()
                .find(|s| s.stage == name)
                .is_some_and(|s| s.status == DiagnosticStatus::Ok)
        };
        if ok("CAR FILES")
            && ok("CAR MODEL")
            && ok("CAR PACK")
            && ok("NGP DATA")
            && ok("NGP BACKEND")
        {
            self.pack_path.as_ref()
        } else {
            None
        }
    }
}

/// Discover NGP physics pack folder or zip for a specific RSF vehicle under `Documents/RBR`.
pub fn find_pack_for(rbr_root: &Path, vehicle: &VehicleDescriptor) -> Option<PathBuf> {
    if let Some(pack_name) = vehicle.rsf_pack.as_deref() {
        for rel in [
            format!("rsfdata/cars/{pack_name}"),
            format!("rsfdata/cars/{pack_name}.zip"),
            format!("cars/{pack_name}"),
            format!("cars/{pack_name}.zip"),
        ] {
            if let Some(resolved) = cars::resolve(rbr_root, &rel) {
                return Some(resolved);
            }
        }
    }
    if vehicle.id == "ford-focus-rs-wrc-2006" {
        return find_focus_pack(rbr_root);
    }
    None
}

/// Discover optional Ford Focus 2006 NGP pack directory or zip in `Documents/RBR`.
pub fn find_focus_pack(rbr_root: &Path) -> Option<PathBuf> {
    let direct = [
        "rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6",
        "rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6.zip",
        "rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006",
        "rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006.zip",
    ];
    for rel in direct {
        if let Some(resolved) = cars::resolve(rbr_root, rel) {
            return Some(resolved);
        }
    }
    for parent_rel in ["rsfdata/cars", "cars"] {
        let Some(dir) = cars::resolve(rbr_root, parent_rel) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("focus") && (name.contains("2006") || name.contains("mk_ii")) {
                return Some(entry.path());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_always_emits_all_nine_stages_and_handles_missing_documents() {
        let report = FocusDiagnosticsReport::inspect(None, &VehicleDescriptor::ford_focus_2006());
        let names: Vec<&str> = report.steps.iter().map(|s| s.stage).collect();
        assert_eq!(
            names,
            vec![
                "CAR FILES",
                "CAR MODEL",
                "CAR PACK",
                "NGP DATA",
                "NGP BACKEND",
                "PHYSICS INITIALIZATION",
                "VEHICLE SPAWN",
                "CONTROLS",
                "AUDIO",
                "DRIVING",
            ]
        );
        assert_eq!(report.steps[8].status, DiagnosticStatus::Unsupported);
        assert!(report.steps[8].detail.contains("FMOD BANK"));
        assert!(!report.can_enter_visual_mode());
        assert!(report.can_boot_ngp_pack().is_none());
    }

    #[test]
    fn inspects_real_installed_focus_assets_if_present_on_dev_pc() {
        let rbr = Path::new(r"C:\Richard Burns Rally\Cars\FORD_FOCUS_2006\focus06.sgc");
        if !rbr.is_file() {
            return;
        }
        // Create temp Documents symlink/layout or test loader directly on C:\Richard Burns Rally
        let car = crate::car::load_vehicle_for_mode(
            Path::new(r"C:\Richard Burns Rally"),
            &VehicleDescriptor::ford_focus_2006(),
            true,
        )
        .expect("Ford Focus 2006 SGC/textures should load cleanly in experimental mode");
        assert!(!car.geometry.drawn.is_empty());
        assert!(!car.textures.is_empty());
        assert!(find_focus_pack(Path::new(r"C:\Richard Burns Rally")).is_some());

        // Verify NGP 7.5 DLL is cleanly identified as unported (blocked without panic),
        // while NGP 7.4 DLL validates when present.
        let ngp75 = Path::new(r"C:\Richard Burns Rally\Plugins\NGP.dll");
        if ngp75.is_file() {
            let bytes = std::fs::read(ngp75).unwrap();
            assert!(rerbr_sim::ngp_candidate::validate_runtime(&bytes).is_err());
        }
        let ngp74 = Path::new(r"C:\Users\adria\Desktop\RBR\Plugins\NGP.dll");
        if ngp74.is_file() {
            let bytes = std::fs::read(ngp74).unwrap();
            assert!(rerbr_sim::ngp_candidate::validate_runtime(&bytes).is_ok());
        }
    }

    #[test]
    fn fallback_retail_model_never_reports_ok_for_experimental_car() {
        let mut fake_focus = VehicleDescriptor::ford_focus_2006();
        fake_focus.model = "Cars/impreza03/impreza03.sgc".into();
        let dir = std::env::temp_dir().join("rerbr_test_focus_fallback");
        let _ = std::fs::remove_dir_all(&dir);
        let imp = dir.join("RBR/Cars/impreza03");
        std::fs::create_dir_all(&imp).unwrap();
        std::fs::write(imp.join("impreza03.sgc"), b"").unwrap();
        let report = FocusDiagnosticsReport::inspect(Some(&dir), &fake_focus);
        let car_files = report
            .steps
            .iter()
            .find(|s| s.stage == "CAR FILES")
            .unwrap();
        assert_ne!(car_files.status, DiagnosticStatus::Ok);
        assert!(car_files.detail.contains("FALLBACK"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_runtime_state_and_diagnostic_export_are_accurate() {
        let mut report =
            FocusDiagnosticsReport::inspect(None, &VehicleDescriptor::ford_focus_2006());
        assert_eq!(report.steps.len(), 10);
        // Never mark physics/spawn/controls/driving OK before live session state arrives.
        for name in [
            "PHYSICS INITIALIZATION",
            "VEHICLE SPAWN",
            "CONTROLS",
            "DRIVING",
        ] {
            let st = report
                .steps
                .iter()
                .find(|s| s.stage == name)
                .unwrap()
                .status;
            assert_ne!(st, DiagnosticStatus::Ok, "{name} must not start OK");
        }

        report.apply_live_state(&LiveRuntimeState {
            physics_settled: true,
            physics_status: Some("NGP PARSED / CRC UNVERIFIED"),
            sim_steps: Some(720),
            speed_mps: 12.5,
            rpm: 6200.0,
            gear: 3,
            throttle_rx: 1.0,
            steering_rx: -0.2,
            clutch_rx: 0.0,
            clutch_help: true,
            session_active: true,
            stage_id: "harwood-forest",
            model_path: "Cars/FORD_FOCUS_2006/focus06.sgc",
            audio_status: "FMOD FSB5 VORBIS (44 VORBIS)",
            ..Default::default()
        });
        assert_eq!(
            report
                .steps
                .iter()
                .find(|s| s.stage == "VEHICLE SPAWN")
                .unwrap()
                .status,
            DiagnosticStatus::Ok
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|s| s.stage == "CONTROLS")
                .unwrap()
                .status,
            DiagnosticStatus::Ok
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|s| s.stage == "AUDIO")
                .unwrap()
                .status,
            DiagnosticStatus::Unsupported
        );
        assert_eq!(
            report
                .steps
                .iter()
                .find(|s| s.stage == "DRIVING")
                .unwrap()
                .status,
            DiagnosticStatus::Ok
        );

        let dir = std::env::temp_dir().join("rerbr_diag_export_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = report
            .write_export_file(&dir, &["ESCUELA: 17/17 DISPONIBLES".into()])
            .unwrap();
        let txt = std::fs::read_to_string(&path).unwrap();
        assert!(txt.contains("10. DRIVING"));
        assert!(txt.contains("Cars/FORD_FOCUS_2006/focus06.sgc"));
        assert!(txt.contains("ESCUELA: 17/17 DISPONIBLES"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
