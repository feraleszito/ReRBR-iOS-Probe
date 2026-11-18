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

impl FocusDiagnosticsReport {
    pub fn inspect(documents: Option<&Path>, vehicle: &VehicleDescriptor) -> Self {
        let mut steps = Vec::with_capacity(9);
        let Some(doc_root) = documents else {
            return Self::with_missing_documents(vehicle);
        };
        let rbr_root = doc_root.join("RBR");

        // 1. CAR FILES
        let required_ok = vehicle
            .required_paths()
            .filter(|p| !rerbr_content::same_path(p, &vehicle.index))
            .filter(|p| cars::resolve(&rbr_root, p).is_none())
            .collect::<Vec<_>>();
        let car_files_ok = required_ok.is_empty();
        steps.push(DiagnosticStep {
            stage: "CAR FILES",
            status: if car_files_ok {
                DiagnosticStatus::Ok
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if car_files_ok {
                format!("{} OK", vehicle.model)
            } else {
                format!("FALTA {}", required_ok[0])
            },
        });

        // 2. CAR MODEL (3D SGC + shaders + DDS textures)
        let car_model_ok = if car_files_ok {
            match crate::car::load_vehicle_for_mode(&rbr_root, vehicle, true) {
                Ok(car) => {
                    steps.push(DiagnosticStep {
                        stage: "CAR MODEL",
                        status: DiagnosticStatus::Ok,
                        detail: format!(
                            "{} DIBUJOS / {} TEXTURAS DDS",
                            car.geometry.drawn.len(),
                            car.textures.len()
                        ),
                    });
                    true
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

        // 3. CAR PACK (rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6)
        let pack_path = find_focus_pack(&rbr_root);
        let mut car_pack_ok = false;
        if let Some(ref p) = pack_path {
            match Pack::open(p).and_then(|pack| {
                pack.read("common.lsp")?;
                pack.read("gravel.lsp")?;
                Ok(())
            }) {
                Ok(()) => {
                    car_pack_ok = true;
                    let summary = ngp_proto::inspect_vehicle_in_documents(doc_root, vehicle)
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
            steps.push(DiagnosticStep {
                stage: "CAR PACK",
                status: DiagnosticStatus::Blocked,
                detail: "NO ENCONTRADO rsfdata/cars/Ford_Focus_*_ngp6".into(),
            });
        }

        // 4. NGP DATA (Plugins/NGP.dll + physics.rbz)
        let dll = cars::resolve(&rbr_root, "Plugins/NGP.dll");
        let physics_rbz = cars::resolve(&rbr_root, "physics.rbz");
        let ngp_74_verified = match (&dll, &physics_rbz) {
            (Some(dll_path), Some(_rbz_path)) => match std::fs::read(dll_path) {
                Ok(bytes) => match rerbr_sim::ngp_candidate::validate_runtime(&bytes) {
                    Ok(_) => {
                        steps.push(DiagnosticStep {
                            stage: "NGP DATA",
                            status: DiagnosticStatus::Ok,
                            detail: "NGP 7.4.778 + physics.rbz".into(),
                        });
                        true
                    }
                    Err(err) => {
                        steps.push(DiagnosticStep {
                            stage: "NGP DATA",
                            status: DiagnosticStatus::Blocked,
                            detail: format!("NGP NO PORTADO ({err})"),
                        });
                        false
                    }
                },
                Err(err) => {
                    steps.push(DiagnosticStep {
                        stage: "NGP DATA",
                        status: DiagnosticStatus::Blocked,
                        detail: format!("NGP.DLL LEGIBLE ERROR: {err}"),
                    });
                    false
                }
            },
            (None, _) => {
                steps.push(DiagnosticStep {
                    stage: "NGP DATA",
                    status: DiagnosticStatus::Blocked,
                    detail: "FALTA Plugins/NGP.dll".into(),
                });
                false
            }
            (_, None) => {
                steps.push(DiagnosticStep {
                    stage: "NGP DATA",
                    status: DiagnosticStatus::Blocked,
                    detail: "FALTA physics.rbz".into(),
                });
                false
            }
        };

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
                "SIN FÍSICA NGP -> MODO VISUAL EXPERIMENTAL".into()
            } else {
                "BLOQUEADO (SIN DATOS NGP/MODELO)".into()
            },
        });

        // 6. PHYSICS INITIALIZATION
        steps.push(DiagnosticStep {
            stage: "PHYSICS INITIALIZATION",
            status: if backend_ok {
                DiagnosticStatus::Pending
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok {
                "LISTO PARA BOOT EXPERIMENTAL".into()
            } else {
                "SIN FÍSICA NGP ACTIVA".into()
            },
        });

        // 7. VEHICLE SPAWN
        steps.push(DiagnosticStep {
            stage: "VEHICLE SPAWN",
            status: if backend_ok {
                DiagnosticStatus::Pending
            } else if car_model_ok {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok {
                "ESPERANDO CARGA DE TRAMO".into()
            } else if car_model_ok {
                "SPAWN VISUAL ESTÁTICO EN SALIDA".into()
            } else {
                "SIN MODELO PARA SPAWN".into()
            },
        });

        // 8. AUDIO (FMOD bank status)
        let bank_present = cars::resolve(&rbr_root, &vehicle.audio_index).is_some();
        steps.push(DiagnosticStep {
            stage: "AUDIO",
            status: DiagnosticStatus::Unsupported,
            detail: if bank_present {
                "AUDIO: UNSUPPORTED - FMOD BANK (SILENCIO)".into()
            } else {
                "AUDIO: UNSUPPORTED - SIN FMOD BANK (SILENCIO)".into()
            },
        });

        // 9. DRIVING
        steps.push(DiagnosticStep {
            stage: "DRIVING",
            status: if backend_ok {
                DiagnosticStatus::Pending
            } else if car_model_ok {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Blocked
            },
            detail: if backend_ok {
                "NGP EXPERIMENTAL LISTO PARA PRUEBA".into()
            } else if car_model_ok {
                "SOLO INSPECCIÓN VISUAL (SIN FÍSICA NGP)".into()
            } else {
                "BLOQUEADO".into()
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
            "DRIVING",
            DiagnosticStatus::Warning,
            "MODO VISUAL - SIN FÍSICA NGP",
        );
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
                "AUDIO",
                "DRIVING",
            ]
        );
        assert_eq!(report.steps[7].status, DiagnosticStatus::Unsupported);
        assert!(report.steps[7].detail.contains("FMOD BANK"));
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
}
