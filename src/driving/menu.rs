//! Native menu model for the validated Corolla/Harwood preset.
//! Every row and hit region is shared; unsupported content is never offered as playable.

use super::content_manager::{physics_report, Catalog, EntryState, IMPORT_STEPS};
use super::layout::Rect;
use super::ui::{self, ACCENT, BAD, DIM, GOOD, LINE, PANEL_ACTIVE, PANEL_DEEP, TEXT, WARN};
use crate::harwood::benchmark::hud::Canvas;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Main,
    Loading,
    Drive,
    Garage,
    GarageDetail,
    Stages,
    StageDetail,
    Content,
    ContentImport,
    ContentCompatibility,
    Settings,
    Controls,
    CameraEditor,
    ControlsEditor,
    About,
    Paused,
    Driving,
    StageComplete,
    Replay,
    NgpExperimental,
    GamepadSettings,
    PacenoteHudSettings,
    Championship,
    RallySchool,
}

pub(crate) fn format_stage_time(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    let mins = tenths / 600;
    let secs = (tenths % 600) as f64 / 10.0;
    format!("{mins:02}:{secs:04.1}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Open(Page),
    Back,
    ResetToMain,
    StartRally,
    Start,
    Restart,
    StartReplay,
    ReplayTogglePause,
    ReplayRestart,
    ReplayCycleCamera,
    ReplayExit,
    NextStagePage,
    PrevStagePage,
    NextGaragePage,
    PrevGaragePage,
    Calibrate,
    ResetCalibration,
    SensDown,
    SensUp,
    DeadDown,
    DeadUp,
    CameraNext,
    VolumeDown,
    VolumeUp,
    MusicVolumeDown,
    MusicVolumeUp,
    CodriverVolumeDown,
    CodriverVolumeUp,
    ToggleFps,
    ToggleDiagnostics,
    ToggleMinimalHud,
    ToggleInvertPedals,
    ToggleClutchHelp,
    InspectContent,
    CheckPhysics,
    ToggleExperimentalMode,
    RunFocusDiagnostics,
    StartFocusVisualInspection,
    OpenVehicle(usize),
    SelectVehicle(usize),
    OpenStage(usize),
    SelectStage(usize),
    CamFovDown,
    CamFovUp,
    CamPosXDown,
    CamPosXUp,
    CamPosYDown,
    CamPosYUp,
    CamPosZDown,
    CamPosZUp,
    CamLookXDown,
    CamLookXUp,
    CamLookZDown,
    CamLookZUp,
    CamLookYDown,
    CamLookYUp,
    CamSave,
    CamReset,
    CtrlNext,
    CtrlSizeDown,
    CtrlSizeUp,
    CtrlOpacityDown,
    CtrlOpacityUp,
    GlobalOpacityDown,
    GlobalOpacityUp,
    CtrlSave,
    CtrlReset,
    RecoverCar,
    GpDeadDown,
    GpDeadUp,
    GpSensDown,
    GpSensUp,
    GpCurveDown,
    GpCurveUp,
    GpInvertSteer,
    GpInvertTriggers,
    GpBindGearUp,
    GpBindGearDown,
    GpBindHandbrake,
    GpBindClutch,
    GpBindCamera,
    GpBindRecover,
    GpSave,
    GpReset,
    PnToggle,
    PnScaleDown,
    PnScaleUp,
    PnPosXDown,
    PnPosXUp,
    PnPosYDown,
    PnPosYUp,
    PnAnticipationDown,
    PnAnticipationUp,
    PnToggleDebug,
    PnSave,
    PnReset,
    ChampToggleActive,
    ChampCycleSkill,
    ChampNextRally,
    ChampStartStage,
    ChampReset,
    SchoolPrevLesson,
    SchoolNextLesson,
    SchoolInspectLesson,
    SchoolStartLesson,
    ExportDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Action {
        command: Command,
        enabled: bool,
    },
    Stepper {
        down: Command,
        up: Command,
        value: String,
    },
    Info {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub kind: RowKind,
}
impl Row {
    fn action(label: impl Into<String>, command: Command) -> Self {
        Self {
            label: label.into(),
            kind: RowKind::Action {
                command,
                enabled: true,
            },
        }
    }
    fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: RowKind::Action {
                command: Command::Back,
                enabled: false,
            },
        }
    }
    fn stepper(
        label: impl Into<String>,
        down: Command,
        up: Command,
        value: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            kind: RowKind::Stepper {
                down,
                up,
                value: value.into(),
            },
        }
    }
    fn info(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: RowKind::Info {
                value: value.into(),
            },
        }
    }
}

pub struct Context<'a> {
    pub selection: &'a rerbr_content::Selection,
    pub content: &'a Catalog,
    pub camera: &'a str,
    pub steering_ready: bool,
    pub cockpit_ready: bool,
    pub scene_loaded: bool,
    pub loading_phase: f32,
    pub loading_error: Option<&'a str>,
    pub cam_override: super::cameras::CameraOverride,
    pub selected_control: super::layout::EditableControl,
    pub sensitivity: f64,
    pub dead_zone: f64,
    pub volume: f64,
    pub music_volume: f64,
    pub codriver_volume: f64,
    pub fps: bool,
    pub diagnostics: bool,
    pub minimal_hud: bool,
    pub invert_pedals: bool,
    pub clutch_help: bool,
    pub ctrl_opacity: f32,
    pub global_opacity: f32,
    pub stage_result: Option<super::race::StageResult>,
    pub has_replay: bool,
    pub ngp_experimental: bool,
    pub focus_diagnostics: Option<&'a super::ngp_experimental::FocusDiagnosticsReport>,
    pub gamepad_connected: bool,
    pub gamepad_name: Option<&'a str>,
    pub gamepad_settings: super::gamepad::GamepadSettings,
    pub pacenote_hud: super::pacenote_icons::PacenoteHudSettings,
    pub pacenote_atlas_status: Option<&'a str>,
    pub championship: Option<&'a super::championship::ChampionshipCalendar>,
    pub school: Option<&'a super::school::SchoolCatalog>,
}

#[derive(Debug, Default)]
pub struct Menu {
    pub page: Page,
    stack: Vec<Page>,
    pub inspected_vehicle: usize,
    pub inspected_stage: usize,
    pub garage_page: usize,
    pub stage_page: usize,
    pub focused_row: usize,
    pub gamepad_focus_active: bool,
}
impl Menu {
    pub fn open(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        self.stack.push(self.page);
        self.page = page;
        self.focused_row = 0;
    }
    pub fn back(&mut self) {
        self.page = self.stack.pop().unwrap_or(Page::Main);
        self.focused_row = 0;
    }
    pub fn reset_to_main(&mut self) {
        self.stack.clear();
        self.page = Page::Main;
        self.focused_row = 0;
    }
    pub fn show(&mut self, page: Page) {
        self.stack.clear();
        self.page = page;
        self.focused_row = 0;
    }
    pub fn resume(&mut self) {
        self.stack.clear();
        self.page = Page::Driving;
        self.focused_row = 0;
    }
    pub fn pause(&mut self) {
        if self.page == Page::Driving {
            self.stack.clear();
            self.page = Page::Paused;
            self.focused_row = 0;
        }
    }

    fn row_interactive(row: &Row) -> bool {
        matches!(
            row.kind,
            RowKind::Action { enabled: true, .. } | RowKind::Stepper { .. }
        )
    }

    pub fn focus_move(&mut self, rows: &[Row], delta: i32) {
        let indices: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Self::row_interactive(r).then_some(i))
            .collect();
        if indices.is_empty() {
            self.focused_row = 0;
            return;
        }
        self.gamepad_focus_active = true;
        let current_pos = indices.iter().position(|&i| i == self.focused_row);
        let next_pos = match current_pos {
            Some(pos) => {
                let len = indices.len() as isize;
                ((pos as isize + delta as isize).rem_euclid(len)) as usize
            }
            None => 0,
        };
        self.focused_row = indices[next_pos];
    }

    pub fn focus_confirm(&self, rows: &[Row]) -> Option<Command> {
        let row = rows.get(self.focused_row)?;
        match row.kind {
            RowKind::Action {
                command,
                enabled: true,
            } => Some(command),
            RowKind::Stepper { up, .. } => Some(up),
            _ => rows.iter().find_map(|r| match r.kind {
                RowKind::Action {
                    command,
                    enabled: true,
                } => Some(command),
                _ => None,
            }),
        }
    }

    pub fn focus_adjust(&self, rows: &[Row], delta: i32) -> Option<Command> {
        let row = rows.get(self.focused_row)?;
        match row.kind {
            RowKind::Stepper { down, up, .. } => Some(if delta < 0 { down } else { up }),
            _ => None,
        }
    }
    pub fn title(&self) -> &'static str {
        match self.page {
            Page::Main | Page::Loading => "RICHARD BURNS RALLY",
            Page::Drive => "INICIAR RALLY",
            Page::Garage | Page::GarageDetail => "SELECCIONAR COCHE",
            Page::Stages | Page::StageDetail => "SELECCIONAR TRAMO",
            Page::Content | Page::ContentImport | Page::ContentCompatibility => {
                "GESTOR DE CONTENIDO"
            }
            Page::Settings => "CONFIGURACIÓN",
            Page::Controls => "CONTROLES",
            Page::CameraEditor => "EDITOR DE CÁMARAS",
            Page::ControlsEditor => "EDITOR DE CONTROLES",
            Page::About => "ACERCA DE",
            Page::Paused => "PAUSA",
            Page::Driving => "CONDUCCIÓN",
            Page::StageComplete => "STAGE COMPLETE",
            Page::Replay => "REPETICIÓN",
            Page::NgpExperimental => "NGP EXPERIMENTAL",
            Page::GamepadSettings => "MANDO / GAMEPAD",
            Page::PacenoteHudSettings => "NOTAS VISUALES",
            Page::Championship => "CAMPEONATO RBR",
            Page::RallySchool => "ESCUELA DE RALLY",
        }
    }
    pub fn rows(&self, ctx: &Context) -> Vec<Row> {
        let vehicle = &ctx.selection.vehicle;
        let stage = &ctx.selection.stage;
        let vehicle_state = ctx
            .content
            .vehicle(&vehicle.id)
            .map_or(EntryState::Selected, |entry| entry.state);
        let stage_state = ctx
            .content
            .stage(&stage.id)
            .map_or(EntryState::Selected, |entry| entry.state);
        match self.page {
            Page::Main => vec![
                Row::action(
                    format!("SELECCIONAR COCHE: {}", vehicle.name),
                    Command::Open(Page::Garage),
                ),
                Row::action(
                    format!("SELECCIONAR TRAMO: {}", stage.name),
                    Command::Open(Page::Stages),
                ),
                Row::action("INICIAR RALLY", Command::StartRally),
                Row::action("CONFIGURACIÓN", Command::Open(Page::Settings)),
                Row::action("CONTROLES", Command::Open(Page::Controls)),
                Row::action("CAMPEONATO RBR", Command::Open(Page::Championship)),
                Row::action("ESCUELA DE RALLY", Command::Open(Page::RallySchool)),
                Row::action("GESTOR DE CONTENIDO", Command::Open(Page::Content)),
                Row::action("ACERCA DE", Command::Open(Page::About)),
            ],
            Page::Loading => {
                if ctx.loading_error.is_some() {
                    vec![Row::action(
                        "Volver al menú principal",
                        Command::ResetToMain,
                    )]
                } else {
                    vec![Row::action("CANCELAR", Command::ResetToMain)]
                }
            }
            Page::Drive => vec![
                Row::info("TRAMO", stage.name.clone()),
                Row::info("COCHE", vehicle.name.clone()),
                Row {
                    label: if ctx.steering_ready {
                        "INICIAR RALLY".into()
                    } else {
                        "INICIAR RALLY - SUJETA EN HORIZONTAL".into()
                    },
                    kind: RowKind::Action {
                        command: Command::StartRally,
                        enabled: ctx.steering_ready,
                    },
                },
                Row::action("VOLVER", Command::Back),
            ],
            Page::Garage => {
                const PER_PAGE: usize = 9;
                let total_vehicles = ctx.content.vehicles.len();
                let total_pages = total_vehicles.div_ceil(PER_PAGE);
                let page_idx = self.garage_page.min(total_pages.saturating_sub(1));
                let start = page_idx * PER_PAGE;
                let end = (start + PER_PAGE).min(total_vehicles);

                let mut rows: Vec<Row> = ctx.content.vehicles[start..end]
                    .iter()
                    .enumerate()
                    .map(|(offset, entry)| {
                        let index = start + offset;
                        let class_tag = entry.classification.badge();
                        let cat_tag = entry
                            .descriptor
                            .category
                            .as_deref()
                            .filter(|c| *c != "RBR Original")
                            .map(|c| format!(" · {c}"))
                            .unwrap_or_default();
                        Row::action(
                            format!(
                                "{} [{class_tag}{cat_tag}] - {}",
                                entry.descriptor.name,
                                entry.state.label()
                            ),
                            Command::OpenVehicle(index),
                        )
                    })
                    .collect();
                if total_pages > 1 {
                    rows.push(Row::action(
                        format!("SIGUIENTE PÁGINA ({}/{})", page_idx + 1, total_pages),
                        Command::NextGaragePage,
                    ));
                    rows.push(Row::action("PÁGINA ANTERIOR", Command::PrevGaragePage));
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::GarageDetail => {
                let entry = ctx.content.vehicles.get(self.inspected_vehicle);
                let inspected = entry.map(|e| &e.descriptor).unwrap_or(vehicle);
                let state = entry.map_or(vehicle_state, |e| e.state);
                let is_ngp_car = inspected.is_ngp_experimental_car();
                let supported = if is_ngp_car {
                    ctx.ngp_experimental
                } else {
                    inspected.validate_current_backend().is_ok()
                };
                let mut rows = Vec::new();
                if is_ngp_car {
                    let class_text = entry.map(|e| e.classification.badge()).unwrap_or("RSF NGP");
                    let fb_warn = if entry.is_some_and(|e| e.using_fallback_model) {
                        " [MODELO FALLBACK!]"
                    } else {
                        ""
                    };
                    rows.push(Row::info(
                        "CLASIFICACIÓN",
                        if ctx.ngp_experimental {
                            format!("{class_text}{fb_warn} — BACKEND NGP ACTIVO")
                        } else {
                            format!("{class_text}{fb_warn} — ACTIVA MODO NGP PARA CONDUCIR")
                        },
                    ));
                } else if inspected.is_validated_reference() {
                    rows.push(Row::info(
                        "CLASIFICACIÓN",
                        "ORIGINAL RBR VALIDADO (REFERENCIA)",
                    ));
                } else {
                    rows.push(Row::info(
                        "CLASIFICACIÓN",
                        "ORIGINAL RBR FUNCIONAL (BACKEND 1.02)",
                    ));
                }
                if let Some(cat) = inspected.category.as_deref() {
                    rows.push(Row::info("CATEGORÍA", cat));
                }
                if state == EntryState::Selected {
                    rows.push(Row::info("SELECCIÓN", "SELECCIONADO"));
                } else {
                    rows.push(Row {
                        label: if supported {
                            if is_ngp_car {
                                "SELECCIONAR RSF (NGP EXPERIMENTAL)".into()
                            } else {
                                "SELECCIONAR COCHE".into()
                            }
                        } else {
                            "NO DISPONIBLE / ACTIVA MODO NGP".into()
                        },
                        kind: RowKind::Action {
                            command: Command::SelectVehicle(self.inspected_vehicle),
                            enabled: supported,
                        },
                    });
                }
                if is_ngp_car {
                    rows.push(Row::action(
                        format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                        Command::ToggleExperimentalMode,
                    ));
                    rows.push(Row::action(
                        "DIAGNÓSTICO NGP (9 ETAPAS)",
                        Command::RunFocusDiagnostics,
                    ));
                }
                if let Some(sheet) = entry.and_then(|e| e.tech_sheet.as_ref()) {
                    rows.push(Row::info("FÍSICA", sheet.physics_summary()));
                    rows.push(Row::info("TRACCIÓN", sheet.drivetrain_and_mass_summary()));
                    rows.push(Row::info(
                        "MOTOR / CAJA",
                        sheet.gearbox_and_engine_summary(),
                    ));
                    rows.push(Row::info("DIFERENCIALES", sheet.differentials_summary()));
                    rows.push(Row::info(
                        "NEUMÁTICOS",
                        sheet.tyres_and_suspension_summary(),
                    ));
                } else {
                    rows.push(Row::info(
                        "TRANSMISIÓN",
                        format!("MANUAL {} VEL.", inspected.forward_gears),
                    ));
                }
                if let Some(bank) = entry.and_then(|e| e.fmod_bank.as_deref()) {
                    rows.push(Row::info("AUDIO FMOD", bank));
                }
                if let Some(note) = entry.and_then(|e| e.notes.first()) {
                    rows.push(Row::info("ESTADO", note.clone()));
                }
                if let Some(missing) = entry.and_then(|e| e.files.missing_label()) {
                    rows.push(Row::info("FILES", missing));
                }
                rows.push(Row::action(
                    "INSPECCIONAR ARCHIVOS",
                    Command::InspectContent,
                ));
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::Stages => {
                const PER_PAGE: usize = 8;
                let total_stages = ctx.content.stages.len();
                let total_pages = total_stages.div_ceil(PER_PAGE);
                let page_idx = self.stage_page.min(total_pages.saturating_sub(1));
                let start = page_idx * PER_PAGE;
                let end = (start + PER_PAGE).min(total_stages);

                let mut rows: Vec<Row> = ctx.content.stages[start..end]
                    .iter()
                    .enumerate()
                    .map(|(offset, entry)| {
                        let global_idx = start + offset;
                        let tag = if entry.descriptor.is_validated_reference() {
                            "[REF]"
                        } else {
                            "[EXP]"
                        };
                        Row::action(
                            format!(
                                "{} {} ({}) - {}",
                                tag,
                                entry.descriptor.name,
                                entry.descriptor.surface(),
                                entry.state.label()
                            ),
                            Command::OpenStage(global_idx),
                        )
                    })
                    .collect();
                if total_pages > 1 {
                    rows.push(Row::stepper(
                        "PÁGINA",
                        Command::PrevStagePage,
                        Command::NextStagePage,
                        format!("{}/{}", page_idx + 1, total_pages),
                    ));
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::StageDetail => {
                let entry = ctx.content.stages.get(self.inspected_stage);
                let inspected = entry.map(|e| &e.descriptor).unwrap_or(stage);
                let state = entry.map_or(stage_state, |e| e.state);
                let supported = super::stage_supported(inspected);
                let files_ready =
                    entry.is_none_or(|e| !e.files.checked || e.files.missing.is_empty());
                let mut rows = Vec::new();
                if state == EntryState::Selected {
                    rows.push(Row::info("SELECCIÓN", "SELECCIONADO"));
                } else {
                    rows.push(Row {
                        label: if !supported {
                            "TRAMO NO DISPONIBLE (SIN SOPORTE)".into()
                        } else if !files_ready {
                            "TRAMO BLOQUEADO - ARCHIVOS FALTANTES".into()
                        } else {
                            "SELECCIONAR TRAMO".into()
                        },
                        kind: RowKind::Action {
                            command: Command::SelectStage(self.inspected_stage),
                            enabled: supported && files_ready,
                        },
                    });
                }
                if let Some(entry) = entry {
                    rows.push(Row::info("SUPERFICIE", entry.descriptor.surface()));
                    rows.push(Row::info("PAÍS", entry.descriptor.country()));
                    rows.push(Row::info(
                        "BLOQUES TERRENO",
                        format!("{}", entry.descriptor.terrain_blocks),
                    ));
                    rows.push(Row::info(
                        "ESTADO",
                        if entry.descriptor.is_validated_reference() {
                            "REFERENCIA VALIDADA"
                        } else {
                            "EXPERIMENTAL RBR"
                        },
                    ));
                }
                if let Some(missing) = entry.and_then(|e| e.files.missing_label()) {
                    rows.push(Row::info("FILES", missing));
                }
                rows.push(Row::action(
                    "INSPECCIONAR ARCHIVOS",
                    Command::InspectContent,
                ));
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::Content => vec![
                Row::action("MANAGE CARS", Command::Open(Page::Garage)),
                Row::action("MANAGE STAGES", Command::Open(Page::Stages)),
                Row::disabled("IMPORT FROM IPAD FILES - UNAVAILABLE"),
                Row::disabled("PACKAGE INSPECTOR - UNAVAILABLE"),
                Row::action(
                    "COMPATIBILITY REPORT",
                    Command::Open(Page::ContentCompatibility),
                ),
                Row::action("REFRESH CONTENT AUDIT", Command::InspectContent),
                Row::action("BACK", Command::Back),
            ],
            Page::ContentImport => {
                let mut rows = IMPORT_STEPS
                    .iter()
                    .map(|step| Row::info(step.label, step.detail))
                    .collect::<Vec<_>>();
                rows.push(Row::action("BACK", Command::Back));
                rows
            }
            Page::ContentCompatibility => {
                let vehicle_ok = if ctx.ngp_experimental {
                    vehicle.validate_experimental_ngp().is_ok()
                } else {
                    vehicle.validate_current_backend().is_ok()
                };
                let stage_ok = stage.validate_current_backend().is_ok();
                vec![
                    Row::info(
                        "VEHICLE GATE",
                        if vehicle_ok { "ACCEPTED" } else { "REJECTED" },
                    ),
                    Row::info("STAGE GATE", if stage_ok { "ACCEPTED" } else { "REJECTED" }),
                    Row::info("PHYSICS DATA", "NGP NOT CERTIFIED"),
                    Row::info(
                        "BACKEND ACTIVO",
                        if ctx.ngp_experimental {
                            "MODO NGP EXPERIMENTAL (FOCUS 2006)"
                        } else {
                            super::ngp_proto::PhysicsBackend::OriginalRetail.label()
                        },
                    ),
                    Row::action(
                        format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                        Command::ToggleExperimentalMode,
                    ),
                    Row::action(
                        "DIAGNÓSTICO NGP / RUNTIME (10 ETAPAS)",
                        Command::RunFocusDiagnostics,
                    ),
                    Row::action("RUN DESCRIPTOR CHECK", Command::CheckPhysics),
                    Row::action("BACK", Command::Back),
                ]
            }
            Page::Settings => vec![
                Row::stepper(
                    "SENSIBILIDAD GIROSCOPIO",
                    Command::SensDown,
                    Command::SensUp,
                    format!("{:.2}", ctx.sensitivity),
                ),
                Row::stepper(
                    "ZONA MUERTA",
                    Command::DeadDown,
                    Command::DeadUp,
                    format!("{:.2}", ctx.dead_zone),
                ),
                Row::action(format!("CÁMARA: {}", ctx.camera), Command::CameraNext),
                Row::stepper(
                    "VOLUMEN MOTOR",
                    Command::VolumeDown,
                    Command::VolumeUp,
                    format!("{:.0}%", ctx.volume * 100.0),
                ),
                Row::stepper(
                    "VOLUMEN MÚSICA",
                    Command::MusicVolumeDown,
                    Command::MusicVolumeUp,
                    format!("{:.0}%", ctx.music_volume * 100.0),
                ),
                Row::stepper(
                    "VOLUMEN COPILOTO",
                    Command::CodriverVolumeDown,
                    Command::CodriverVolumeUp,
                    format!("{:.0}%", ctx.codriver_volume * 100.0),
                ),
                Row::action(
                    format!("INVERTIR PEDALES: {}", on_off(ctx.invert_pedals)),
                    Command::ToggleInvertPedals,
                ),
                Row::action(
                    format!("AYUDA EMBRAGUE: {}", on_off(ctx.clutch_help)),
                    Command::ToggleClutchHelp,
                ),
                Row::action(
                    format!("CONTADOR FPS: {}", on_off(ctx.fps)),
                    Command::ToggleFps,
                ),
                Row::action(
                    format!("TELEMETRÍA: {}", on_off(ctx.diagnostics)),
                    Command::ToggleDiagnostics,
                ),
                Row::action(
                    format!("HUD MÍNIMO: {}", on_off(ctx.minimal_hud)),
                    Command::ToggleMinimalHud,
                ),
                Row {
                    label: if ctx.scene_loaded {
                        "EDITAR CÁMARAS".into()
                    } else {
                        "EDITAR CÁMARAS (INICIA RALLY)".into()
                    },
                    kind: RowKind::Action {
                        command: Command::Open(Page::CameraEditor),
                        enabled: ctx.scene_loaded && cfg!(feature = "physics-stage"),
                    },
                },
                Row::action("EDITAR CONTROLES", Command::Open(Page::ControlsEditor)),
                Row::action(
                    "NOTAS VISUALES COPILOTO",
                    Command::Open(Page::PacenoteHudSettings),
                ),
                Row::action("VOLVER", Command::Back),
            ],
            Page::PacenoteHudSettings => {
                let pn = &ctx.pacenote_hud;
                vec![
                    Row::info(
                        "ICONOS ORIGINALES",
                        ctx.pacenote_atlas_status
                            .unwrap_or("LISTO (PC_Generic.bnk)"),
                    ),
                    Row::action(
                        format!("ICONOS EN PANTALLA: {}", on_off(pn.enabled)),
                        Command::PnToggle,
                    ),
                    Row::stepper(
                        "TAMAÑO ICONOS",
                        Command::PnScaleDown,
                        Command::PnScaleUp,
                        format!("{:.0}%", pn.scale * 100.0),
                    ),
                    Row::stepper(
                        "POSICIÓN X",
                        Command::PnPosXDown,
                        Command::PnPosXUp,
                        format!("{:.0}%", pn.pos_x * 100.0),
                    ),
                    Row::stepper(
                        "POSICIÓN Y",
                        Command::PnPosYDown,
                        Command::PnPosYUp,
                        format!("{:.0}%", pn.pos_y * 100.0),
                    ),
                    Row::stepper(
                        "DURACIÓN VISOR",
                        Command::PnAnticipationDown,
                        Command::PnAnticipationUp,
                        format!("{:.1}S", pn.anticipation_s),
                    ),
                    Row::action(
                        format!("DEBUG SPRITE BNK: {}", on_off(pn.debug_sprites)),
                        Command::PnToggleDebug,
                    ),
                    Row::action("GUARDAR NOTAS VISUALES", Command::PnSave),
                    Row::action("RESTABLECER NOTAS VISUALES", Command::PnReset),
                    Row::action("VOLVER", Command::Back),
                ]
            }
            Page::Championship => {
                let mut rows = Vec::new();
                if let Some(cal) = ctx.championship {
                    let rally_name = cal
                        .rallies
                        .get(cal.progress.rally_index)
                        .map(|r| r.name.to_ascii_uppercase())
                        .unwrap_or_else(|| "SIN SEASON.DAT".into());
                    let stage_label = cal
                        .current_stage_descriptor()
                        .map(|s| format!("{} (MAP {})", s.name, s.original_map))
                        .or_else(|| {
                            cal.current_stage_info()
                                .map(|i| format!("MAPA {} (NO EN CATÁLOGO)", i.map))
                        })
                        .unwrap_or_else(|| "NO DISPONIBLE".into());
                    rows.push(Row::info("CALENDARIO", cal.status.clone()));
                    rows.push(Row::action(
                        format!("CAMPEONATO ACTIVO: {}", on_off(cal.progress.active)),
                        Command::ChampToggleActive,
                    ));
                    rows.push(Row::action(
                        format!("NIVEL: {}", cal.progress.skill.label()),
                        Command::ChampCycleSkill,
                    ));
                    rows.push(Row::action(
                        format!(
                            "RALLY ACTUAL ({}/{}): {rally_name}",
                            cal.progress.rally_index + 1,
                            cal.rallies.len().max(1)
                        ),
                        Command::ChampNextRally,
                    ));
                    rows.push(Row::info(
                        format!("TRAMO {}", cal.progress.stage_index + 1),
                        stage_label,
                    ));
                    rows.push(Row::info(
                        "TIEMPO ACUMULADO",
                        format!(
                            "{} ({} COMPLETADOS)",
                            format_stage_time(cal.progress.total_time_seconds()),
                            cal.progress.completed_stages
                        ),
                    ));
                    rows.push(Row::action(
                        "INICIAR SIGUIENTE TRAMO",
                        Command::ChampStartStage,
                    ));
                    rows.push(Row::action("REINICIAR CAMPEONATO", Command::ChampReset));
                } else {
                    rows.push(Row::info("ESTADO", "CARGANDO SEASON.DAT"));
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::RallySchool => {
                let mut rows = Vec::new();
                if let Some(sch) = ctx.school {
                    rows.push(Row::info("ESCUELA", sch.status.clone()));
                    if let Some(entry) = sch.entries.get(sch.selected) {
                        rows.push(Row::info(
                            format!("LECCIÓN {}/17", sch.selected + 1),
                            format!("{} (MAP {})", entry.key, entry.map),
                        ));
                        rows.push(Row::info("SETUP", entry.setup_file));
                        rows.push(Row::info("ARCHIVO", entry.stem.as_str()));
                        rows.push(Row::info(
                            "NOTA / ARCHIVOS",
                            if entry.assets_present {
                                format!("GRADO {}/4 · ARCHIVOS OK", entry.grade)
                            } else {
                                format!(
                                    "GRADO {}/4 · FALTA {}",
                                    entry.grade,
                                    entry
                                        .missing_files
                                        .first()
                                        .map_or("ARCHIVOS", String::as_str)
                                )
                            },
                        ));
                        rows.push(Row {
                            label: "INICIAR LECCIÓN".into(),
                            kind: RowKind::Action {
                                command: Command::SchoolStartLesson,
                                enabled: entry.assets_present,
                            },
                        });
                    }
                    rows.push(Row::stepper(
                        "SELECCIONAR LECCIÓN",
                        Command::SchoolPrevLesson,
                        Command::SchoolNextLesson,
                        format!("{}/17", sch.selected + 1),
                    ));
                    rows.push(Row::action(
                        "COMPROBAR LECCIÓN EN DOCUMENTS",
                        Command::SchoolInspectLesson,
                    ));
                } else {
                    rows.push(Row::info("ESTADO", "SIN CATÁLOGO"));
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::CameraEditor => vec![
                Row::action(format!("CÁMARA: {}", ctx.camera), Command::CameraNext),
                Row::stepper(
                    "FOV",
                    Command::CamFovDown,
                    Command::CamFovUp,
                    format!("{:.0}°", ctx.cam_override.fov_degrees),
                ),
                Row::stepper(
                    "POS X",
                    Command::CamPosXDown,
                    Command::CamPosXUp,
                    format!("{:.2}", ctx.cam_override.pos_offset.x),
                ),
                Row::stepper(
                    "POS Y",
                    Command::CamPosYDown,
                    Command::CamPosYUp,
                    format!("{:.2}", ctx.cam_override.pos_offset.y),
                ),
                Row::stepper(
                    "POS Z",
                    Command::CamPosZDown,
                    Command::CamPosZUp,
                    format!("{:.2}", ctx.cam_override.pos_offset.z),
                ),
                Row::stepper(
                    "MIRA X",
                    Command::CamLookXDown,
                    Command::CamLookXUp,
                    format!("{:.2}", ctx.cam_override.target_offset.x),
                ),
                Row::stepper(
                    "MIRA Y",
                    Command::CamLookYDown,
                    Command::CamLookYUp,
                    format!("{:.2}", ctx.cam_override.target_offset.y),
                ),
                Row::stepper(
                    "MIRA Z",
                    Command::CamLookZDown,
                    Command::CamLookZUp,
                    format!("{:.2}", ctx.cam_override.target_offset.z),
                ),
                Row::action("GUARDAR", Command::CamSave),
                Row::action("RESTABLECER", Command::CamReset),
                Row::action("VOLVER", Command::Back),
            ],
            Page::ControlsEditor => vec![
                Row::action(
                    format!("CONTROL: {}", ctx.selected_control.label()),
                    Command::CtrlNext,
                ),
                Row::stepper("TAMAÑO", Command::CtrlSizeDown, Command::CtrlSizeUp, "±2%"),
                Row::stepper(
                    "OPACIDAD",
                    Command::CtrlOpacityDown,
                    Command::CtrlOpacityUp,
                    format!("{:.0}%", ctx.ctrl_opacity * 100.0),
                ),
                Row::stepper(
                    "OPACIDAD TODOS",
                    Command::GlobalOpacityDown,
                    Command::GlobalOpacityUp,
                    format!("{:.0}%", ctx.global_opacity * 100.0),
                ),
                Row::action("GUARDAR", Command::CtrlSave),
                Row::action("RESTABLECER", Command::CtrlReset),
                Row::action("VOLVER", Command::Back),
            ],
            Page::Controls => vec![
                Row {
                    label: "CALIBRAR GIROSCOPIO".into(),
                    kind: RowKind::Action {
                        command: Command::Calibrate,
                        enabled: ctx.steering_ready,
                    },
                },
                Row::action("RESTABLECER CALIBRACIÓN", Command::ResetCalibration),
                Row::action(
                    format!(
                        "MANDO / GAMEPAD: {}",
                        if ctx.gamepad_connected {
                            ctx.gamepad_name.unwrap_or("CONECTADO")
                        } else {
                            "SIN MANDO"
                        }
                    ),
                    Command::Open(Page::GamepadSettings),
                ),
                Row::info("TRANSMISIÓN", format!("MANUAL {}", vehicle.forward_gears)),
                Row::info("CAMBIO", "PULSA GEAR - / +"),
                Row::action("VOLVER", Command::Back),
            ],
            Page::GamepadSettings => {
                let gp = &ctx.gamepad_settings;
                vec![
                    Row::info(
                        "ESTADO",
                        if ctx.gamepad_connected {
                            ctx.gamepad_name.unwrap_or("CONECTADO")
                        } else {
                            "CONECTA MANDO BLUETOOTH / USB"
                        },
                    ),
                    Row::stepper(
                        "ZONA MUERTA",
                        Command::GpDeadDown,
                        Command::GpDeadUp,
                        format!("{:.0}%", gp.deadzone * 100.0),
                    ),
                    Row::stepper(
                        "SENSIBILIDAD",
                        Command::GpSensDown,
                        Command::GpSensUp,
                        format!("{:.2}", gp.sensitivity),
                    ),
                    Row::stepper(
                        "CURVA RESPUESTA",
                        Command::GpCurveDown,
                        Command::GpCurveUp,
                        format!("{:.2}", gp.response_curve),
                    ),
                    Row::action(
                        format!("INVERTIR DIRECCIÓN: {}", on_off(gp.invert_steering)),
                        Command::GpInvertSteer,
                    ),
                    Row::action(
                        format!("INVERTIR GATILLOS: {}", on_off(gp.invert_triggers)),
                        Command::GpInvertTriggers,
                    ),
                    Row::action(
                        format!("SUBIR MARCHA: {}", gp.gear_up.label()),
                        Command::GpBindGearUp,
                    ),
                    Row::action(
                        format!("BAJAR MARCHA: {}", gp.gear_down.label()),
                        Command::GpBindGearDown,
                    ),
                    Row::action(
                        format!("FRENO DE MANO: {}", gp.handbrake.label()),
                        Command::GpBindHandbrake,
                    ),
                    Row::action(
                        format!("EMBRAGUE: {}", gp.clutch.label()),
                        Command::GpBindClutch,
                    ),
                    Row::action(
                        format!("CÁMARA: {}", gp.camera.label()),
                        Command::GpBindCamera,
                    ),
                    Row::action(
                        format!("RESCATE: {}", gp.recover.label()),
                        Command::GpBindRecover,
                    ),
                    Row::action("GUARDAR PERFIL MANDO", Command::GpSave),
                    Row::action("RESTABLECER MANDO", Command::GpReset),
                    Row::action("VOLVER", Command::Back),
                ]
            }
            Page::About => vec![
                Row::info("VERSIÓN", env!("CARGO_PKG_VERSION")),
                Row::info("BUILD", option_env!("GITHUB_RUN_NUMBER").unwrap_or("LOCAL")),
                Row::info("LICENCIA", "GPL-3.0-OR-LATER"),
                Row::action("VOLVER", Command::Back),
            ],
            Page::Paused => vec![
                Row::action("REANUDAR", Command::Start),
                Row::action("RESCATAR COCHE (AYUDA)", Command::RecoverCar),
                Row::action("REINICIAR TRAMO", Command::Restart),
                Row::action("CONFIGURACIÓN", Command::Open(Page::Settings)),
                Row::action("CONTROLES", Command::Open(Page::Controls)),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::Driving => Vec::new(),
            Page::StageComplete => {
                let mut rows = Vec::new();
                if let Some(res) = ctx.stage_result {
                    rows.push(Row::info(
                        "DRIVING TIME",
                        format_stage_time(res.driving_seconds),
                    ));
                    rows.push(Row::info(
                        "PENALTIES",
                        if res.false_start {
                            format!("+{:.0}s FALSE START", res.penalty_seconds)
                        } else {
                            "0.0s".into()
                        },
                    ));
                    rows.push(Row::info(
                        "TOTAL TIME",
                        format_stage_time(res.total_seconds),
                    ));
                    rows.push(Row::info(
                        "CHECKPOINTS",
                        format!("{}/{}", res.splits_passed, res.splits_total),
                    ));
                }
                if ctx.has_replay {
                    rows.push(Row::action("VER REPETICIÓN", Command::StartReplay));
                }
                rows.push(Row::action("REINICIAR TRAMO", Command::Restart));
                rows.push(Row::action("MENÚ PRINCIPAL", Command::ResetToMain));
                rows
            }
            Page::Replay => Vec::new(),
            Page::NgpExperimental => {
                let visual_ready = ctx
                    .focus_diagnostics
                    .is_some_and(|r| r.can_enter_visual_mode());
                let pack_ready = ctx
                    .focus_diagnostics
                    .is_some_and(|r| r.can_boot_ngp_pack().is_some());
                vec![
                    Row::action(
                        format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                        Command::ToggleExperimentalMode,
                    ),
                    Row::action("ACTUALIZAR DIAGNÓSTICO", Command::RunFocusDiagnostics),
                    Row::action("EXPORTAR DIAGNÓSTICO (TXT)", Command::ExportDiagnostics),
                    Row {
                        label: if pack_ready {
                            "INICIAR PRUEBA NGP FOCUS (SILENCIO FMOD)".into()
                        } else if visual_ready {
                            "INSPECCIÓN VISUAL FOCUS (SIN FÍSICA NGP)".into()
                        } else {
                            "PRUEBA FOCUS BLOQUEADA (REVISAR CAR FILES)".into()
                        },
                        kind: RowKind::Action {
                            command: Command::StartFocusVisualInspection,
                            enabled: visual_ready,
                        },
                    },
                    Row::action("VOLVER", Command::Back),
                ]
            }
        }
    }
    pub fn subtitle(&self, ctx: &Context) -> Vec<String> {
        let vehicle = &ctx.selection.vehicle;
        let stage = &ctx.selection.stage;
        match self.page {
            Page::Main | Page::Loading => vec![
                "iOS ALPHA".into(),
                format!("{} — {}", vehicle.name, stage.name),
            ],
            Page::Drive => vec![
                format!("TRAMO {} ({})", stage.name, stage.stem),
                format!("{} / MANUAL {} VEL.", vehicle.name, vehicle.forward_gears),
            ],
            Page::Garage => {
                let audit = ctx
                    .content
                    .vehicle(&vehicle.id)
                    .map(|entry| entry.files.summary())
                    .unwrap_or_else(|| "ARCHIVOS PRIVADOS SIN INSPECCIONAR".into());
                vec!["CATÁLOGO DE VEHÍCULOS ORIGINALES RBR".into(), audit]
            }
            Page::GarageDetail => {
                let entry = ctx.content.vehicles.get(self.inspected_vehicle);
                let inspected = entry.map(|e| &e.descriptor).unwrap_or(vehicle);
                let audit = entry
                    .map(|e| e.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                let blocker = if inspected.validate_current_backend().is_ok() {
                    format!(
                        "{} FORWARD GEARS / {}",
                        inspected.forward_gears,
                        if ctx.cockpit_ready {
                            "COCKPIT READY"
                        } else {
                            "COCKPIT ASSETS MISSING"
                        }
                    )
                } else {
                    if inspected.id == "ford-focus-rs-wrc-2006" {
                        "BLOCKED: REQUIRES NGP 7.5 + FMOD BANK + SGC".into()
                    } else {
                        "BLOCKED: ORIGINAL CAR NOT VALIDATED IN LIVE PHYSICS".into()
                    }
                };
                vec![
                    format!(
                        "ID {} / SLOT {} / TYRE {}",
                        inspected.id, inspected.original_slot, inspected.tyre_type
                    ),
                    blocker,
                    format!("MODEL {}", inspected.model),
                    audit,
                ]
            }
            Page::Stages => {
                let audit = ctx
                    .content
                    .stage(&stage.id)
                    .map(|entry| entry.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                vec![
                    "READ-ONLY INSTALLED STAGE CATALOG".into(),
                    audit,
                    "HARWOOD / CHIRDONHEAD / COTE D'ARBROZ".into(),
                ]
            }
            Page::StageDetail => {
                let entry = ctx.content.stages.get(self.inspected_stage);
                let inspected = entry.map(|e| &e.descriptor).unwrap_or(stage);
                let audit = entry
                    .map(|e| e.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                vec![
                    format!(
                        "ID {} / ORIGINAL MAP {} / DRY WEATHER",
                        inspected.id, inspected.original_map
                    ),
                    format!(
                        "STEM {} / {} TERRAIN BLOCKS EXPECTED",
                        inspected.stem, inspected.terrain_blocks
                    ),
                    format!("MAPS {}", inspected.documents_maps),
                    audit,
                ]
            }
            Page::CameraEditor | Page::ControlsEditor => Vec::new(),
            Page::Content => vec![
                "PRIVATE DOCUMENTS CATALOG / NO NEW DIRECTORY LAYOUT".into(),
                "IMPORT AND INSTALL ARE NOT AVAILABLE IN THIS BUILD".into(),
                "ONLY VALIDATED CONTENT IS SELECTABLE".into(),
            ],
            Page::ContentImport => vec![
                "FUTURE FILES IMPORT PIPELINE - NOT AVAILABLE".into(),
                "NO PACKAGE FORMAT IS CLAIMED INSTALLABLE".into(),
                "STEPS BELOW ARE ARCHITECTURE, NOT ENABLED ACTIONS".into(),
            ],
            Page::ContentCompatibility => {
                let (_, report) = physics_report(ctx.selection);
                vec![
                    report,
                    "DESCRIPTOR PATHS AND BACKEND GATES ONLY".into(),
                    "ORIGINAL NGP CRCS AND FORCE PARITY ARE NOT CERTIFIED".into(),
                ]
            }
            Page::Settings => vec![
                "PREFERENCES SAVED IN DOCUMENTS".into(),
                "VOLUME CONTROLS ENGINE AUDIO OUTPUT".into(),
                "CAMERA CHANGES APPLY IMMEDIATELY".into(),
            ],
            Page::Controls => vec![
                "TILT TO STEER / CALIBRATE AT YOUR HOLDING ANGLE".into(),
                if ctx.invert_pedals {
                    "PEDALS: SLIDE UP FOR MORE PRESSURE (INVERTED)".into()
                } else {
                    "PEDALS: SLIDE DOWN FOR MORE PRESSURE".into()
                },
                "CLUTCH AND HANDBRAKE ARE HELD CONTROLS".into(),
                format!(
                    "GEAR - / + STEPS R N 1-{} / MANUAL ONLY",
                    vehicle.forward_gears
                ),
            ],
            Page::About => vec![
                "ReRBR BY JODUSNODUS / GPL-3.0-OR-LATER".into(),
                "iOS PROBE / PRIVATE GAME ASSETS REQUIRED".into(),
                "NO GAME ASSETS ARE BUNDLED IN THE IPA".into(),
            ],
            Page::Paused => vec![
                "SIMULATION PAUSED / INPUTS RELEASED".into(),
                "RESTART RESETS TO THE ORIGINAL SPAWN".into(),
            ],
            Page::Driving => Vec::new(),
            Page::StageComplete => vec![
                format!("{} — {}", vehicle.name, stage.name),
                if ctx.stage_result.is_some_and(|r| r.false_start) {
                    "TIMED FINISH WITH JUMP-START PENALTY".into()
                } else {
                    "TIMED FINISH VERIFIED".into()
                },
            ],
            Page::Replay => vec![
                "REPETICIÓN OFICIAL DE TRAMO".into(),
                format!("{} — {}", vehicle.name, stage.name),
            ],
            Page::NgpExperimental => vec![
                "DIAGNÓSTICO 9 ETAPAS — FORD FOCUS RS WRC 2006 (NO CERTIFICADO)".into(),
                format!("ESTADO MODO EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
            ],
            Page::GamepadSettings => vec![
                "CONDUCCIÓN: STICK IZQ DIRECCIÓN / RT ACELERAR / LT FRENO".into(),
                "MENÚS: D-PAD O STICK NAVEGAR / A CONFIRMAR / B VOLVER".into(),
            ],
            Page::PacenoteHudSettings => vec![
                "ICONOS ORIGINALES DE RBR CARGADOS DE GENERIC/PC_GENERIC_*.BNK".into(),
                "AJUSTA TAMAÑO, POSICIÓN Y DURACIÓN EN PANTALLA".into(),
            ],
            Page::Championship => vec![
                "CAMPEONATO ORIGINAL RBR (SEASON.DAT · 6 RALLIES)".into(),
                "TIEMPOS, PENALIZACIONES Y PROGRESO GUARDADOS".into(),
            ],
            Page::RallySchool => vec![
                "17 LECCIONES ORIGINALES (MAPS 73..89 / PHYSICS/SCHOOL)".into(),
                "SIN VOCES SINTÉTICAS: REQUIERE ARCHIVOS PRIVADOS".into(),
            ],
        }
    }
    fn row_bounds(&self) -> (f32, f32, f32, f32) {
        match self.page {
            Page::CameraEditor => (0.03, 0.43, 0.10, 0.94),
            Page::Loading => (0.28, 0.44, 0.76, 0.86),
            Page::ControlsEditor => (0.28, 0.44, 0.04, 0.36),
            Page::NgpExperimental => (0.055, 0.89, 0.615, 0.89),
            _ => (ROW_X, ROW_WIDTH, ROW_TOP, ROW_BOTTOM),
        }
    }
    pub(crate) fn row_rect_for(&self, index: usize, rows: usize) -> Rect {
        let (rx, rw, top, bottom) = self.row_bounds();
        if rows == 0 {
            return Rect::default();
        }
        let h = ((bottom - top) / rows as f32).min(0.078);
        let gap = (h * 0.16).min(0.009);
        Rect::new(rx, top + index as f32 * h, rw, (h - gap).max(0.02))
    }
    fn stepper_split_for(&self, value: &str) -> f32 {
        let (rx, rw, _, _) = self.row_bounds();
        let right_px = (rx + rw) * 640.0;
        let value_width = ui::texture_width(value, 0.78);
        (right_px - 34.0 - value_width * 0.5) / 640.0
    }
    /// Resolves a touch to a command using the same rectangles used for drawing.
    pub fn press(&self, x: f64, y: f64, rows: &[Row]) -> Option<Command> {
        if !x.is_finite() || !y.is_finite() || rows.is_empty() {
            return None;
        }
        let (xf, yf) = (x as f32, y as f32);
        let index = (0..rows.len()).find(|&i| {
            let r = self.row_rect_for(i, rows.len());
            xf >= r.x && xf <= r.x + r.w && yf >= r.y && yf <= r.y + r.h
        })?;
        match &rows[index].kind {
            RowKind::Action {
                command,
                enabled: true,
            } => Some(*command),
            RowKind::Stepper { down, up, value } => {
                let split_x = self.stepper_split_for(value);
                Some(if xf < split_x { *down } else { *up })
            }
            RowKind::Action { .. } | RowKind::Info { .. } => None,
        }
    }
    /// Draws the complete menu screen. Uses only the existing Metal HUD atlas.
    pub fn draw(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        if self.page == Page::Loading {
            self.draw_loading(canvas, ctx);
            return;
        }
        let compact = matches!(self.page, Page::CameraEditor | Page::ControlsEditor);
        let height = canvas.height();
        if !compact {
            ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 1.0), ui::BACKDROP);
            ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.11), [12, 15, 21, 255]);
            ui::fill(canvas, Rect::new(0.055, 0.042, 0.006, 0.045), ACCENT);
            canvas.text(0.072 * 640.0, 0.032 * height, self.title(), 1.45, TEXT);
            ui::rule(canvas, 0.055, 0.110, 0.89, LINE);

            let subtitle = self.subtitle(ctx);
            let mut lines = Vec::new();
            for line in subtitle {
                lines.extend(ui::wrap(&line, 56));
            }
            let scale = 0.78;
            for (index, line) in lines.iter().take(4).enumerate() {
                canvas.text(
                    0.055 * 640.0,
                    (0.125 + index as f32 * 0.030) * height,
                    line,
                    scale,
                    DIM,
                );
            }
        } else if self.page == Page::CameraEditor {
            ui::panel(
                canvas,
                Rect::new(0.02, 0.02, 0.45, 0.96),
                [8, 10, 14, 218],
                LINE,
            );
            canvas.text(0.035 * 640.0, 0.028 * height, self.title(), 0.95, TEXT);
        } else {
            ui::panel(
                canvas,
                Rect::new(0.27, 0.015, 0.46, 0.37),
                [8, 10, 14, 218],
                LINE,
            );
        }

        if self.page == Page::NgpExperimental {
            let panel = Rect::new(0.055, 0.195, 0.89, 0.405);
            ui::panel(canvas, panel, PANEL_DEEP, LINE);
            let (px, py, pw, ph) = ui::px(panel, canvas);
            if let Some(report) = ctx.focus_diagnostics {
                let row_h = (ph - 10.0) / 10.0f32.max(report.steps.len() as f32);
                for (idx, step) in report.steps.iter().take(10).enumerate() {
                    let sy = py + 5.0 + idx as f32 * row_h;
                    let badge_color = match step.status {
                        super::ngp_experimental::DiagnosticStatus::Ok => GOOD,
                        super::ngp_experimental::DiagnosticStatus::Warning => WARN,
                        super::ngp_experimental::DiagnosticStatus::Unsupported => ACCENT,
                        super::ngp_experimental::DiagnosticStatus::Blocked => BAD,
                        super::ngp_experimental::DiagnosticStatus::Pending => DIM,
                    };
                    ui::text_clipped(
                        canvas,
                        px + 10.0,
                        sy + 2.0,
                        pw * 0.29,
                        step.stage,
                        0.66,
                        TEXT,
                    );
                    ui::text_clipped(
                        canvas,
                        px + pw * 0.30,
                        sy + 2.0,
                        pw * 0.16,
                        step.status.label(),
                        0.66,
                        badge_color,
                    );
                    ui::text_clipped(
                        canvas,
                        px + pw * 0.46,
                        sy + 2.0,
                        pw * 0.52,
                        &step.detail,
                        0.62,
                        DIM,
                    );
                }
            } else {
                ui::text_center(
                    canvas,
                    panel,
                    "PULSA ACTUALIZAR DIAGNÓSTICO PARA INSPECCIONAR",
                    0.8,
                    DIM,
                );
            }
        }

        let rows = self.rows(ctx);
        let label_scale = if rows.len() > 7 { 0.74 } else { 0.82 };
        for (index, row) in rows.iter().enumerate() {
            let rect = self.row_rect_for(index, rows.len());
            let (x, y, w, h) = ui::px(rect, canvas);
            let interactive = matches!(
                row.kind,
                RowKind::Action { enabled: true, .. } | RowKind::Stepper { .. }
            );
            let is_primary = matches!(
                row.kind,
                RowKind::Action {
                    command: Command::StartRally,
                    enabled: true,
                }
            );
            let panel = if is_primary {
                [150, 28, 28, 235]
            } else if interactive {
                PANEL_ACTIVE
            } else {
                PANEL_DEEP
            };
            canvas.rect(x, y, w, h, panel);
            let focused = self.gamepad_focus_active && interactive && index == self.focused_row;
            if is_primary {
                ui::frame(
                    canvas,
                    rect,
                    if focused { WARN } else { ACCENT },
                    if focused { 2.5 } else { 1.5 },
                );
            } else if focused {
                ui::frame(canvas, rect, WARN, 2.0);
                canvas.rect(x, y, 5.0, h, WARN);
            } else {
                if interactive {
                    canvas.rect(x, y, 3.0, h, ACCENT);
                }
                canvas.rect(x, y, w, 1.0, LINE);
            }
            let text_color = if interactive { TEXT } else { DIM };
            let value_space = match &row.kind {
                RowKind::Info { value } => ui::texture_width(value, 0.7) + 30.0,
                RowKind::Stepper { value, .. } => ui::texture_width(value, 0.78) + 62.0,
                RowKind::Action { .. } => 0.0,
            };
            ui::text_clipped(
                canvas,
                x + 12.0,
                y + (h - 16.0 * label_scale) * 0.5,
                (w - 24.0 - value_space).max(40.0),
                &row.label,
                label_scale,
                text_color,
            );
            match &row.kind {
                RowKind::Info { value } => {
                    ui::text_right(canvas, rect, value, 0.7, 0.02, DIM);
                }
                RowKind::Stepper { value, .. } => {
                    let value_scale = 0.78;
                    let value_width = ui::texture_width(value, value_scale);
                    canvas.text(
                        x + w - value_width - 34.0,
                        y + (h - 16.0 * value_scale) * 0.5,
                        value,
                        value_scale,
                        TEXT,
                    );
                    canvas.text(
                        x + w - 22.0,
                        y + (h - 16.0 * value_scale) * 0.5,
                        "+",
                        value_scale,
                        TEXT,
                    );
                    canvas.text(
                        x + w - value_width - 48.0,
                        y + (h - 16.0 * value_scale) * 0.5,
                        "-",
                        value_scale,
                        TEXT,
                    );
                }
                RowKind::Action { .. } => {}
            }
        }

        if !compact {
            let footer = ui::wrap(status, 58);
            ui::rule(canvas, 0.055, 0.900, 0.89, LINE);
            let color = if status.contains("ERROR")
                || status.contains("FAIL")
                || status.contains("REJECTED")
            {
                BAD
            } else if status.contains("MISSING") || status.contains("UNAVAILABLE") {
                WARN
            } else {
                GOOD
            };
            for (index, line) in footer.iter().take(2).enumerate() {
                canvas.text(
                    0.055 * 640.0,
                    (0.912 + index as f32 * 0.026) * height,
                    line,
                    0.7,
                    color,
                );
            }
        }
    }
    fn draw_loading(&self, canvas: &mut Canvas, ctx: &Context) {
        let height = canvas.height();
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 1.0), ui::BACKDROP);
        ui::text_center(
            canvas,
            Rect::new(0.0, 0.16, 1.0, 0.08),
            "RICHARD BURNS RALLY",
            1.65,
            TEXT,
        );
        ui::text_center(
            canvas,
            Rect::new(0.0, 0.24, 1.0, 0.05),
            "iOS ALPHA",
            0.95,
            DIM,
        );
        if let Some(error) = ctx.loading_error {
            ui::panel(canvas, Rect::new(0.14, 0.36, 0.72, 0.22), PANEL_DEEP, BAD);
            let lines = ui::wrap(error, 48);
            for (i, line) in lines.iter().take(4).enumerate() {
                ui::text_center(
                    canvas,
                    Rect::new(0.16, 0.39 + i as f32 * 0.04, 0.68, 0.04),
                    line,
                    0.78,
                    BAD,
                );
            }
            if self.page == Page::NgpExperimental {
                let panel = Rect::new(0.055, 0.195, 0.89, 0.405);
                ui::panel(canvas, panel, PANEL_DEEP, LINE);
                let (px, py, pw, ph) = ui::px(panel, canvas);
                if let Some(report) = ctx.focus_diagnostics {
                    let row_h = (ph - 10.0) / 10.0f32.max(report.steps.len() as f32);
                    for (idx, step) in report.steps.iter().take(10).enumerate() {
                        let sy = py + 5.0 + idx as f32 * row_h;
                        let badge_color = match step.status {
                            super::ngp_experimental::DiagnosticStatus::Ok => GOOD,
                            super::ngp_experimental::DiagnosticStatus::Warning => WARN,
                            super::ngp_experimental::DiagnosticStatus::Unsupported => ACCENT,
                            super::ngp_experimental::DiagnosticStatus::Blocked => BAD,
                            super::ngp_experimental::DiagnosticStatus::Pending => DIM,
                        };
                        ui::text_clipped(
                            canvas,
                            px + 10.0,
                            sy + 2.0,
                            pw * 0.29,
                            step.stage,
                            0.66,
                            TEXT,
                        );
                        ui::text_clipped(
                            canvas,
                            px + pw * 0.30,
                            sy + 2.0,
                            pw * 0.16,
                            step.status.label(),
                            0.66,
                            badge_color,
                        );
                        ui::text_clipped(
                            canvas,
                            px + pw * 0.46,
                            sy + 2.0,
                            pw * 0.52,
                            &step.detail,
                            0.62,
                            DIM,
                        );
                    }
                } else {
                    ui::text_center(
                        canvas,
                        panel,
                        "PULSA ACTUALIZAR DIAGNÓSTICO PARA INSPECCIONAR",
                        0.8,
                        DIM,
                    );
                }
            }

            let rows = self.rows(ctx);
            for (index, row) in rows.iter().enumerate() {
                let rect = self.row_rect_for(index, rows.len());
                ui::panel(canvas, rect, PANEL_ACTIVE, ACCENT);
                ui::text_center(canvas, rect, &row.label, 0.85, TEXT);
            }
        } else {
            canvas.spinner(320.0, height * 0.45, 22.0, 3.5, ctx.loading_phase, TEXT);
            ui::text_center(
                canvas,
                Rect::new(0.0, 0.56, 1.0, 0.06),
                "Cargando tramo...",
                1.0,
                TEXT,
            );
            ui::text_center(
                canvas,
                Rect::new(0.0, 0.62, 1.0, 0.05),
                "Preparando coche y escenario",
                0.82,
                DIM,
            );
        }
        if ctx.loading_error.is_none() {
            if self.page == Page::NgpExperimental {
                let panel = Rect::new(0.055, 0.195, 0.89, 0.405);
                ui::panel(canvas, panel, PANEL_DEEP, LINE);
                let (px, py, pw, ph) = ui::px(panel, canvas);
                if let Some(report) = ctx.focus_diagnostics {
                    let row_h = (ph - 10.0) / 10.0f32.max(report.steps.len() as f32);
                    for (idx, step) in report.steps.iter().take(10).enumerate() {
                        let sy = py + 5.0 + idx as f32 * row_h;
                        let badge_color = match step.status {
                            super::ngp_experimental::DiagnosticStatus::Ok => GOOD,
                            super::ngp_experimental::DiagnosticStatus::Warning => WARN,
                            super::ngp_experimental::DiagnosticStatus::Unsupported => ACCENT,
                            super::ngp_experimental::DiagnosticStatus::Blocked => BAD,
                            super::ngp_experimental::DiagnosticStatus::Pending => DIM,
                        };
                        ui::text_clipped(
                            canvas,
                            px + 10.0,
                            sy + 2.0,
                            pw * 0.29,
                            step.stage,
                            0.66,
                            TEXT,
                        );
                        ui::text_clipped(
                            canvas,
                            px + pw * 0.30,
                            sy + 2.0,
                            pw * 0.16,
                            step.status.label(),
                            0.66,
                            badge_color,
                        );
                        ui::text_clipped(
                            canvas,
                            px + pw * 0.46,
                            sy + 2.0,
                            pw * 0.52,
                            &step.detail,
                            0.62,
                            DIM,
                        );
                    }
                } else {
                    ui::text_center(
                        canvas,
                        panel,
                        "PULSA ACTUALIZAR DIAGNÓSTICO PARA INSPECCIONAR",
                        0.8,
                        DIM,
                    );
                }
            }

            let rows = self.rows(ctx);
            for (index, row) in rows.iter().enumerate() {
                let rect = self.row_rect_for(index, rows.len());
                ui::panel(canvas, rect, PANEL_ACTIVE, LINE);
                ui::text_center(canvas, rect, &row.label, 0.85, TEXT);
            }
        }
    }
}

fn on_off(value: bool) -> &'static str {
    if value {
        "ON"
    } else {
        "OFF"
    }
}

pub const ROW_TOP: f32 = 0.23;
pub const ROW_BOTTOM: f32 = 0.88;
pub const ROW_X: f32 = 0.14;
pub const ROW_WIDTH: f32 = 0.72;

pub fn row_height(rows: usize) -> f32 {
    if rows == 0 {
        0.0
    } else {
        ((ROW_BOTTOM - ROW_TOP) / rows as f32).min(0.078)
    }
}
pub fn row_rect(index: usize, rows: usize) -> Rect {
    let height = row_height(rows);
    let gap = (height * 0.16).min(0.009);
    Rect::new(
        ROW_X,
        ROW_TOP + index as f32 * height + gap * 0.5,
        ROW_WIDTH,
        (height - gap).max(0.0),
    )
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub sensitivity: f64,
    pub dead_zone: f64,
    pub fps: bool,
    pub diagnostics: bool,
    pub minimal_hud: bool,
    pub invert_pedals: bool,
    pub clutch_help: bool,
    pub volume: f64,
    pub music_volume: f64,
    pub codriver_volume: f64,
    pub camera: u8,
    pub stage_id: String,
    pub ngp_experimental: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 2.0,
            dead_zone: 0.03,
            fps: false,
            diagnostics: false,
            minimal_hud: false,
            invert_pedals: false,
            clutch_help: true,
            volume: 0.8,
            music_volume: 0.70,
            codriver_volume: 0.85,
            camera: 0,
            stage_id: rerbr_content::StageDescriptor::default().id,
            ngp_experimental: false,
        }
    }
}
impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "sensitivity" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.sensitivity = v.clamp(0.25, 5.0);
                        }
                    }
                }
                "dead_zone" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.dead_zone = v.clamp(0.0, 0.2);
                        }
                    }
                }
                "fps" => settings.fps = value == "true",
                "diagnostics" => settings.diagnostics = value == "true",
                "minimal_hud" => settings.minimal_hud = value == "true",
                "invert_pedals" => settings.invert_pedals = value == "true",
                "clutch_help" => settings.clutch_help = value == "true",
                "volume" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.volume = v.clamp(0.0, 1.0);
                        }
                    }
                }
                "music_volume" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.music_volume = v.clamp(0.0, 1.0);
                        }
                    }
                }
                "codriver_volume" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.codriver_volume = v.clamp(0.0, 1.0);
                        }
                    }
                }
                "camera" => {
                    if let Ok(v) = value.parse::<u8>() {
                        settings.camera = v.min(2);
                    }
                }
                "stage_id" if !value.trim().is_empty() => {
                    settings.stage_id = value.trim().to_string();
                }
                "ngp_experimental" => settings.ngp_experimental = value == "true",
                _ => (),
            }
        }
        settings
    }
    fn path() -> Option<std::path::PathBuf> {
        std::env::var_os("RERBR_PROBE_DOCUMENTS")
            .map(|d| std::path::PathBuf::from(d).join("rerbr-driving-settings.txt"))
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |s| Self::parse(&s))
    }
    fn encode(&self) -> String {
        format!(
            "sensitivity={}\ndead_zone={}\nfps={}\ndiagnostics={}\nminimal_hud={}\ninvert_pedals={}\nclutch_help={}\nvolume={}\nmusic_volume={}\ncodriver_volume={}\ncamera={}\nstage_id={}\nngp_experimental={}\n",
            self.sensitivity, self.dead_zone, self.fps, self.diagnostics,
            self.minimal_hud, self.invert_pedals, self.clutch_help, self.volume, self.music_volume, self.codriver_volume, self.camera, self.stage_id, self.ngp_experimental
        )
    }
    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Err(error) = std::fs::write(path, self.encode()) {
                eprintln!("SETTINGS: could not save preferences: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driving::content_manager::Catalog;
    use rerbr_content::Selection;

    fn context<'a>(selection: &'a Selection, content: &'a Catalog) -> Context<'a> {
        Context {
            selection,
            content,
            camera: "CHASE",
            steering_ready: true,
            cockpit_ready: true,
            scene_loaded: true,
            loading_phase: 0.5,
            loading_error: None,
            cam_override: crate::driving::cameras::CameraOverride::default_for(
                crate::driving::cameras::View::Chase,
            ),
            selected_control: crate::driving::layout::EditableControl::Brake,
            sensitivity: 2.0,
            dead_zone: 0.03,
            volume: 0.8,
            music_volume: 0.70,
            codriver_volume: 0.85,
            fps: false,
            diagnostics: false,
            minimal_hud: false,
            invert_pedals: false,
            clutch_help: false,
            ctrl_opacity: 0.5,
            global_opacity: 0.5,
            stage_result: None,
            has_replay: false,
            ngp_experimental: false,
            focus_diagnostics: None,
            gamepad_connected: false,
            gamepad_name: None,
            gamepad_settings: crate::driving::gamepad::GamepadSettings::default(),
            pacenote_hud: crate::driving::pacenote_icons::PacenoteHudSettings::default(),
            pacenote_atlas_status: None,
            championship: None,
            school: None,
        }
    }

    #[test]
    fn main_menu_has_all_sections_and_navigation_uses_explicit_start() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu::default();
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), 9);
        assert!(rows[0].label.contains("SELECCIONAR COCHE"));
        assert!(rows[1].label.contains("SELECCIONAR TRAMO"));
        assert_eq!(rows[2].label, "INICIAR RALLY");
        assert_eq!(rows[3].label, "CONFIGURACIÓN");
        let start_rect = menu.row_rect_for(2, rows.len());
        assert_eq!(
            menu.press(
                start_rect.center_x() as f64,
                start_rect.y as f64 + 0.01,
                &rows
            ),
            Some(Command::StartRally)
        );
        menu.open(Page::Drive);
        let drive = menu.rows(&ctx);
        let start_row = drive
            .iter()
            .position(|row| {
                matches!(
                    row.kind,
                    RowKind::Action {
                        command: Command::StartRally,
                        ..
                    }
                )
            })
            .unwrap();
        let rect = menu.row_rect_for(start_row, drive.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &drive),
            Some(Command::StartRally)
        );
        menu.page = Page::Driving;
        menu.pause();
        assert_eq!(menu.page, Page::Paused);
        menu.back();
        assert_eq!(menu.page, Page::Main);
    }

    #[test]
    fn start_row_returns_start_only_when_landscape_is_ready() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let mut ctx = context(&selection, &content);
        let menu = Menu {
            page: Page::Drive,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        let start_row = rows
            .iter()
            .position(|row| {
                matches!(
                    row.kind,
                    RowKind::Action {
                        command: Command::StartRally,
                        ..
                    }
                )
            })
            .unwrap();
        let rect = menu.row_rect_for(start_row, rows.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
            Some(Command::StartRally)
        );
        ctx.steering_ready = false;
        let rows = menu.rows(&ctx);
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
            None
        );
    }

    #[test]
    fn settings_steppers_use_shared_rows_and_persist_bounded_values() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let menu = Menu {
            page: Page::Settings,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), 15);
        for (row_idx, value, expected_down, expected_up) in [
            (0usize, "2.00", Command::SensDown, Command::SensUp),
            (3usize, "80%", Command::VolumeDown, Command::VolumeUp),
            (
                4usize,
                "70%",
                Command::MusicVolumeDown,
                Command::MusicVolumeUp,
            ),
            (
                5usize,
                "85%",
                Command::CodriverVolumeDown,
                Command::CodriverVolumeUp,
            ),
        ] {
            let rect = menu.row_rect_for(row_idx, rows.len());
            let (rx, _ry, rw, _rh) = (rect.x * 640.0, rect.y, rect.w * 640.0, rect.h);
            let value_width = ui::texture_width(value, 0.78);
            let minus_center_x = f64::from((rx + rw - value_width - 42.0) / 640.0);
            let plus_center_x = f64::from((rx + rw - 16.0) / 640.0);
            let center_y = f64::from(rect.y + rect.h * 0.5);
            assert_eq!(
                menu.press(minus_center_x, center_y, &rows),
                Some(expected_down)
            );
            assert_eq!(
                menu.press(plus_center_x, center_y, &rows),
                Some(expected_up)
            );
        }
        let inv_rect = menu.row_rect_for(6, rows.len());
        assert_eq!(
            menu.press(
                inv_rect.center_x() as f64,
                (inv_rect.y + inv_rect.h * 0.5) as f64,
                &rows
            ),
            Some(Command::ToggleInvertPedals)
        );
        let clutch_rect = menu.row_rect_for(7, rows.len());
        assert_eq!(
            menu.press(
                clutch_rect.center_x() as f64,
                (clutch_rect.y + clutch_rect.h * 0.5) as f64,
                &rows
            ),
            Some(Command::ToggleClutchHelp)
        );
        let s = Settings::parse(
            "sensitivity=NaN\ndead_zone=inf\nvolume=nan\ncamera=9\nfps=true\ninvert_pedals=true\nclutch_help=false",
        );
        assert_eq!((s.sensitivity, s.dead_zone, s.volume), (2.0, 0.03, 0.8));
        assert_eq!(s.camera, 2);
        assert!(s.fps);
        assert!(s.invert_pedals);
        assert!(!s.clutch_help);
        assert!(Settings::default().clutch_help);
        let s = Settings::parse("sensitivity=100\ndead_zone=-4\nvolume=4");
        assert_eq!((s.sensitivity, s.dead_zone, s.volume), (5.0, 0.0, 1.0));
    }

    #[test]
    fn unsupported_and_unavailable_rows_are_not_actions() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu {
            page: Page::Content,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), 7);
        let import = rows
            .iter()
            .position(|row| row.label.starts_with("IMPORT FROM"))
            .unwrap();
        let rect = row_rect(import, rows.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
            None
        );
        menu.page = Page::ContentImport;
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), IMPORT_STEPS.len() + 1);
        for (index, step) in IMPORT_STEPS.iter().enumerate() {
            let rect = row_rect(index, rows.len());
            assert_eq!(
                menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
                None,
                "{} must not be actionable",
                step.label
            );
        }
    }

    #[test]
    fn garage_and_stages_expose_only_validated_entries() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu {
            page: Page::Garage,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), content.vehicles.len() + 1);
        assert!(rows[0]
            .label
            .to_ascii_uppercase()
            .contains("TOYOTA COROLLA WRC"));
        assert!(rows[0].label.contains("SELECTED"));
        menu.page = Page::Stages;
        let rows = menu.rows(&ctx);
        assert!(rows[0]
            .label
            .to_ascii_uppercase()
            .contains("HARWOOD FOREST"));
        menu.page = Page::GarageDetail;
        let rows = menu.rows(&ctx);
        assert!(rows
            .iter()
            .any(|row| row.label == "INSPECTOR ARCHIVOS" || row.label == "INSPECCIONAR ARCHIVOS"));
        assert!(!rows
            .iter()
            .any(|row| row.label.contains("SELECCIONAR COCHE")));
    }

    #[test]
    fn loading_and_editors_render_within_vertex_budget() {
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let mut ctx = context(&selection, &content);
        let report = crate::driving::ngp_experimental::FocusDiagnosticsReport::inspect(
            None,
            &rerbr_content::VehicleDescriptor::ford_focus_2006(),
        );
        ctx.focus_diagnostics = Some(&report);
        for page in [
            Page::Loading,
            Page::CameraEditor,
            Page::ControlsEditor,
            Page::NgpExperimental,
        ] {
            let menu = Menu {
                page,
                ..Menu::default()
            };
            let mut canvas = Canvas::new((1024, 768));
            menu.draw(&mut canvas, &ctx, "OK");
            let bytes = canvas.finish();
            assert!(!bytes.is_empty());
            assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
        }
        ctx.loading_error = Some("MISSING TRACK");
        let loading = Menu {
            page: Page::Loading,
            ..Menu::default()
        };
        let rows = loading.rows(&ctx);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "Volver al menú principal");
    }

    #[test]
    fn loading_animates_and_all_camera_axes_have_working_stepper_regions() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let mut ctx = context(&selection, &content);
        let mut menu = Menu {
            page: Page::Loading,
            ..Menu::default()
        };
        let mut first = Canvas::new((1024, 768));
        menu.draw(&mut first, &ctx, "");
        ctx.loading_phase += 1.0;
        let mut next = Canvas::new((1024, 768));
        menu.draw(&mut next, &ctx, "");
        assert_ne!(first.finish(), next.finish());
        let rows = menu.rows(&ctx);
        let r = menu.row_rect_for(0, rows.len());
        assert_eq!(
            menu.press(f64::from(r.center_x()), f64::from(r.y + r.h * 0.5), &rows),
            Some(Command::ResetToMain)
        );
        menu.page = Page::CameraEditor;
        let rows = menu.rows(&ctx);
        for expected in [
            Command::CamPosXUp,
            Command::CamPosYUp,
            Command::CamPosZUp,
            Command::CamLookXUp,
            Command::CamLookYUp,
            Command::CamLookZUp,
        ] {
            let i = rows
                .iter()
                .position(|r| matches!(r.kind, RowKind::Stepper { up, .. } if up == expected))
                .unwrap();
            let r = menu.row_rect_for(i, rows.len());
            assert_eq!(
                menu.press(
                    f64::from(r.right() - 0.01),
                    f64::from(r.y + r.h * 0.5),
                    &rows
                ),
                Some(expected)
            );
        }
        assert_eq!(content.stages.len(), 30);
        for (i, stage) in content.stages.iter().enumerate() {
            menu.inspected_stage = i;
            menu.page = Page::StageDetail;
            assert!(menu
                .subtitle(&ctx)
                .iter()
                .any(|line| line.contains(&stage.descriptor.stem)));
        }
    }

    #[test]
    fn drawing_fills_a_bounded_vertex_buffer() {
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let menu = Menu {
            page: Page::Settings,
            ..Menu::default()
        };
        let mut canvas = Canvas::new((1024, 768));
        menu.draw(
            &mut canvas,
            &ctx,
            "SCAN: 1 CARS / 1 STAGES / 0 MISSING FILES",
        );
        let bytes = canvas.finish();
        assert!(!bytes.is_empty());
        assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
    }

    #[test]
    fn stage_menu_lists_verified_stages_and_blocks_unsupported_focus() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu {
            page: Page::Stages,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert!(rows.iter().any(|r| r.label.contains("Chirdonhead")));
        assert!(rows.iter().any(|r| r.label.contains("Cote d'Arbroz")));
        menu.inspected_stage = 1;
        menu.page = Page::StageDetail;
        let detail = menu.rows(&ctx);
        assert!(detail.iter().any(|r| matches!(
            r.kind,
            RowKind::Action {
                command: Command::SelectStage(1),
                enabled
            } if enabled == cfg!(feature = "physics-stage")
        )));
        menu.inspected_vehicle = 8;
        menu.page = Page::GarageDetail;
        let car_detail = menu.rows(&ctx);
        assert!(car_detail
            .iter()
            .any(|r| matches!(r.kind, RowKind::Action { enabled: false, .. })));
        let parsed = Settings::parse("stage_id=chirdonhead\n");
        assert_eq!(parsed.stage_id, "chirdonhead");
        let preferences = Settings {
            invert_pedals: true,
            camera: 2,
            stage_id: "cote-darbroz-ii".into(),
            volume: 0.25,
            music_volume: 0.60,
            ..Settings::default()
        };
        let restored = Settings::parse(&preferences.encode());
        assert_eq!(restored.stage_id, preferences.stage_id);
        assert_eq!(restored.camera, 2);
        assert_eq!(restored.volume, 0.25);
        assert_eq!(restored.music_volume, 0.60);
        assert!(restored.invert_pedals);
    }

    #[test]
    fn stage_complete_and_missing_filenames_render_expected_rows() {
        use crate::driving::race::StageResult;
        let selection = Selection::default();
        let mut content = Catalog::descriptors(&selection);
        content.stages[1].files = crate::driving::content_manager::FileAudit {
            checked: true,
            present: 9,
            total: 10,
            missing: vec!["track-22_N_textures.rbz".into()],
        };
        let mut ctx = context(&selection, &content);
        ctx.stage_result = Some(StageResult {
            driving_seconds: 125.4,
            penalty_seconds: 10.0,
            total_seconds: 135.4,
            false_start: true,
            splits_passed: 2,
            splits_total: 2,
        });

        let mut menu = Menu {
            page: Page::StageComplete,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert!(rows.iter().any(|r| r.label == "DRIVING TIME"));
        assert!(rows.iter().any(|r| r.label == "TOTAL TIME"));
        assert!(rows.iter().any(|r| matches!(
            &r.kind,
            RowKind::Info { value } if value == "02:15.4"
        )));
        assert!(rows.iter().any(|r| matches!(
            &r.kind,
            RowKind::Info { value } if value == "2/2"
        )));
        assert!(rows.iter().any(|r| matches!(
            r.kind,
            RowKind::Action {
                command: Command::Restart,
                enabled: true
            }
        )));
        assert!(rows.iter().any(|r| matches!(
            r.kind,
            RowKind::Action {
                command: Command::ResetToMain,
                enabled: true
            }
        )));

        menu.page = Page::StageDetail;
        menu.inspected_stage = 1;
        let detail_rows = menu.rows(&ctx);
        assert!(detail_rows.iter().any(|r| matches!(
            &r.kind,
            RowKind::Info { value } if value.contains("track-22_N_textures.rbz")
        )));
    }

    #[test]
    fn menu_comprehensive_audit_and_navigation_suite() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu::default();

        // 1. Enter and exit all submenus
        for page in [
            Page::Garage,
            Page::Stages,
            Page::Settings,
            Page::Controls,
            Page::Content,
            Page::About,
        ] {
            menu.open(page);
            assert_eq!(menu.page, page);
            menu.back();
            assert_eq!(menu.page, Page::Main);
        }

        // Deep submenus
        menu.open(Page::Garage);
        menu.open(Page::GarageDetail);
        assert_eq!(menu.page, Page::GarageDetail);
        menu.back();
        assert_eq!(menu.page, Page::Garage);
        menu.back();
        assert_eq!(menu.page, Page::Main);

        menu.open(Page::Stages);
        menu.open(Page::StageDetail);
        assert_eq!(menu.page, Page::StageDetail);
        menu.back();
        assert_eq!(menu.page, Page::Stages);
        menu.back();
        assert_eq!(menu.page, Page::Main);

        menu.open(Page::Settings);
        menu.open(Page::CameraEditor);
        assert_eq!(menu.page, Page::CameraEditor);
        menu.back();
        assert_eq!(menu.page, Page::Settings);
        menu.open(Page::ControlsEditor);
        assert_eq!(menu.page, Page::ControlsEditor);
        menu.back();
        assert_eq!(menu.page, Page::Settings);
        menu.back();
        assert_eq!(menu.page, Page::Main);

        menu.open(Page::Content);
        menu.open(Page::ContentCompatibility);
        assert_eq!(menu.page, Page::ContentCompatibility);
        menu.back();
        assert_eq!(menu.page, Page::Content);
        menu.open(Page::ContentImport);
        assert_eq!(menu.page, Page::ContentImport);
        menu.back();
        assert_eq!(menu.page, Page::Content);
        menu.back();
        assert_eq!(menu.page, Page::Main);

        // 2. Pagination test across all 4 pages (30 stages)
        menu.page = Page::Stages;
        menu.stage_page = 0;
        let total_stages = content.stages.len();
        assert_eq!(total_stages, 30);
        let total_pages = total_stages.div_ceil(8);
        assert_eq!(total_pages, 4);

        for expected_page in 0..4 {
            let rows = menu.rows(&ctx);
            let page_row = rows.iter().find(|r| r.label == "PÁGINA");
            assert!(page_row.is_some());
            if let Some(RowKind::Stepper { value, .. }) = page_row.map(|r| &r.kind) {
                assert_eq!(value, &format!("{}/{}", expected_page + 1, total_pages));
            }
            menu.stage_page = (menu.stage_page + 1) % total_pages;
        }

        // Stepper press on PrevStagePage and NextStagePage
        let rows = menu.rows(&ctx);
        let p_rect = menu.row_rect_for(rows.len() - 2, rows.len());
        let split = menu.stepper_split_for(&format!("{}/{}", menu.stage_page + 1, total_pages));
        let left_x = f64::from(split - 0.05);
        let right_x = f64::from(split + 0.05);
        let cy = f64::from(p_rect.y + p_rect.h * 0.5);
        assert_eq!(menu.press(left_x, cy, &rows), Some(Command::PrevStagePage));
        assert_eq!(menu.press(right_x, cy, &rows), Some(Command::NextStagePage));

        // 3. ResetToMain clears navigation stack
        menu.open(Page::Settings);
        menu.open(Page::CameraEditor);
        menu.reset_to_main();
        assert_eq!(menu.page, Page::Main);
        menu.back();
        assert_eq!(menu.page, Page::Main);
    }

    #[test]
    fn hud_vertex_budget_all_screens() {
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);

        let all_pages = [
            Page::Main,
            Page::Loading,
            Page::Drive,
            Page::Garage,
            Page::GarageDetail,
            Page::Stages,
            Page::StageDetail,
            Page::Content,
            Page::ContentImport,
            Page::ContentCompatibility,
            Page::Settings,
            Page::Controls,
            Page::CameraEditor,
            Page::ControlsEditor,
            Page::About,
            Page::Paused,
            Page::StageComplete,
            Page::Replay,
        ];

        for page in all_pages {
            let menu = Menu {
                page,
                ..Menu::default()
            };
            let mut canvas = Canvas::new((1024, 768));
            menu.draw(&mut canvas, &ctx, "PRUEBA ESTADO HUD");
            let bytes = canvas.finish();
            assert!(
                bytes.len() / 20 <= MAX_CANVAS_VERTICES,
                "Page {:?} exceeded vertex budget: {} > {}",
                page,
                bytes.len() / 20,
                MAX_CANVAS_VERTICES
            );
        }
    }

    #[test]
    fn garage_detail_displays_tech_sheet_and_fits_vertex_budget() {
        use crate::driving::ngp_proto::{DrivetrainKind, PhysicsProfile, VehicleTechSheet};
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};

        let selection = Selection::default();
        let mut content = Catalog::descriptors(&selection);
        content.vehicles[3].tech_sheet = Some(VehicleTechSheet {
            profile: PhysicsProfile::OriginalRbr102,
            drivetrain: DrivetrainKind::FourWheelDrive,
            mass_kg: 1400.56,
            forward_gears: 5,
            total_gears: 7,
            turbo: true,
            rpm_limit: None,
            front_diff_nm: Some(320.0),
            center_diff_nm: Some(700.0),
            rear_diff_nm: Some(600.0),
            front_rollbar: Some(20000.0),
            rear_rollbar: Some(30000.0),
            tyre_radius_m: 0.315,
            tyre_brand: "MICHELIN",
            data_crc32: 0x1234_5678,
        });
        let ctx = context(&selection, &content);
        let menu = Menu {
            page: Page::GarageDetail,
            inspected_vehicle: 3,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert!(rows.iter().any(|r| r.label == "FÍSICA"));
        assert!(rows.iter().any(|r| r.label == "TRACCIÓN"));
        assert!(rows.iter().any(|r| r.label == "MOTOR / CAJA"));
        assert!(rows.iter().any(|r| r.label == "DIFERENCIALES"));
        assert!(rows.iter().any(|r| r.label == "NEUMÁTICOS"));

        let mut canvas = Canvas::new((1024, 768));
        menu.draw(&mut canvas, &ctx, "OK");
        let bytes = canvas.finish();
        assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
    }
}
