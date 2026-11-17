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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Open(Page),
    Back,
    ResetToMain,
    StartRally,
    Start,
    Restart,
    Calibrate,
    ResetCalibration,
    SensDown,
    SensUp,
    DeadDown,
    DeadUp,
    CameraNext,
    VolumeDown,
    VolumeUp,
    ToggleFps,
    ToggleDiagnostics,
    ToggleMinimalHud,
    ToggleInvertPedals,
    InspectContent,
    CheckPhysics,
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
    CtrlSave,
    CtrlReset,
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
    pub fps: bool,
    pub diagnostics: bool,
    pub minimal_hud: bool,
    pub invert_pedals: bool,
}

#[derive(Debug, Default)]
pub struct Menu {
    pub page: Page,
    stack: Vec<Page>,
    pub inspected_vehicle: usize,
    pub inspected_stage: usize,
}
impl Menu {
    pub fn open(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        self.stack.push(self.page);
        self.page = page;
    }
    pub fn back(&mut self) {
        self.page = self.stack.pop().unwrap_or(Page::Main);
    }
    pub fn reset_to_main(&mut self) {
        self.stack.clear();
        self.page = Page::Main;
    }
    pub fn show(&mut self, page: Page) {
        self.stack.clear();
        self.page = page;
    }
    pub fn resume(&mut self) {
        self.stack.clear();
        self.page = Page::Driving;
    }
    pub fn pause(&mut self) {
        if self.page == Page::Driving {
            self.stack.clear();
            self.page = Page::Paused;
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
                let mut rows: Vec<Row> = ctx
                    .content
                    .vehicles
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        Row::action(
                            format!("{} - {}", entry.descriptor.name, entry.state.label()),
                            Command::OpenVehicle(index),
                        )
                    })
                    .collect();
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::GarageDetail => {
                let entry = ctx.content.vehicles.get(self.inspected_vehicle);
                let inspected = entry.map(|e| &e.descriptor).unwrap_or(vehicle);
                let state = entry.map_or(vehicle_state, |e| e.state);
                let supported = inspected.validate_current_backend().is_ok();
                let mut rows = Vec::new();
                if state == EntryState::Selected {
                    rows.push(Row::info("SELECCIÓN", "SELECCIONADO"));
                } else {
                    rows.push(Row {
                        label: if supported {
                            "SELECCIONAR COCHE".into()
                        } else {
                            "NO VALIDADO EN VIVO (NGP 7.4)".into()
                        },
                        kind: RowKind::Action {
                            command: Command::SelectVehicle(self.inspected_vehicle),
                            enabled: supported,
                        },
                    });
                }
                rows.push(Row::action(
                    "INSPECCIONAR ARCHIVOS",
                    Command::InspectContent,
                ));
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::Stages => {
                let mut rows: Vec<Row> = ctx
                    .content
                    .stages
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        Row::action(
                            format!("{} - {}", entry.descriptor.name, entry.state.label()),
                            Command::OpenStage(index),
                        )
                    })
                    .collect();
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
                    rows.push(Row::info("SELECTION", "SELECTED"));
                } else {
                    rows.push(Row {
                        label: if !supported {
                            "SELECT STAGE - UNSUPPORTED".into()
                        } else if !files_ready {
                            "SELECT STAGE - MISSING FILES".into()
                        } else {
                            "SELECT STAGE".into()
                        },
                        kind: RowKind::Action {
                            command: Command::SelectStage(self.inspected_stage),
                            enabled: supported && files_ready,
                        },
                    });
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
                let vehicle_ok = vehicle.validate_current_backend().is_ok();
                let stage_ok = stage.validate_current_backend().is_ok();
                vec![
                    Row::info(
                        "VEHICLE GATE",
                        if vehicle_ok { "ACCEPTED" } else { "REJECTED" },
                    ),
                    Row::info("STAGE GATE", if stage_ok { "ACCEPTED" } else { "REJECTED" }),
                    Row::info("PHYSICS DATA", "NGP NOT CERTIFIED"),
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
                    "VOLUMEN",
                    Command::VolumeDown,
                    Command::VolumeUp,
                    format!("{:.0}%", ctx.volume * 100.0),
                ),
                Row::action(
                    format!("INVERTIR PEDALES: {}", on_off(ctx.invert_pedals)),
                    Command::ToggleInvertPedals,
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
                Row::action("VOLVER", Command::Back),
            ],
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
                Row::info("TRANSMISIÓN", format!("MANUAL {}", vehicle.forward_gears)),
                Row::info("CAMBIO", "PULSA GEAR - / +"),
                Row::action("VOLVER", Command::Back),
            ],
            Page::About => vec![
                Row::info("VERSIÓN", env!("CARGO_PKG_VERSION")),
                Row::info("BUILD", option_env!("GITHUB_RUN_NUMBER").unwrap_or("LOCAL")),
                Row::info("LICENCIA", "GPL-3.0-OR-LATER"),
                Row::action("VOLVER", Command::Back),
            ],
            Page::Paused => vec![
                Row::action("REANUDAR", Command::Start),
                Row::action("REINICIAR TRAMO", Command::Restart),
                Row::action("CONFIGURACIÓN", Command::Open(Page::Settings)),
                Row::action("CONTROLES", Command::Open(Page::Controls)),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::Driving => Vec::new(),
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
        }
    }
    fn row_bounds(&self) -> (f32, f32, f32, f32) {
        match self.page {
            Page::CameraEditor => (0.03, 0.43, 0.10, 0.94),
            Page::Loading => (0.28, 0.44, 0.76, 0.86),
            Page::ControlsEditor => (0.28, 0.44, 0.04, 0.36),
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
            if is_primary {
                ui::frame(canvas, rect, ACCENT, 1.5);
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

pub const ROW_TOP: f32 = 0.27;
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
    pub volume: f64,
    pub camera: u8,
    pub stage_id: String,
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
            volume: 0.8,
            camera: 0,
            stage_id: rerbr_content::StageDescriptor::default().id,
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
                "volume" => {
                    if let Ok(v) = value.parse::<f64>() {
                        if v.is_finite() {
                            settings.volume = v.clamp(0.0, 1.0);
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
            "sensitivity={}\ndead_zone={}\nfps={}\ndiagnostics={}\nminimal_hud={}\ninvert_pedals={}\nvolume={}\ncamera={}\nstage_id={}\n",
            self.sensitivity, self.dead_zone, self.fps, self.diagnostics,
            self.minimal_hud, self.invert_pedals, self.volume, self.camera, self.stage_id
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
            fps: false,
            diagnostics: false,
            minimal_hud: false,
            invert_pedals: false,
        }
    }

    #[test]
    fn main_menu_has_all_sections_and_navigation_uses_explicit_start() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu::default();
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), 7);
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
        assert_eq!(rows.len(), 11);
        for (row_idx, value, expected_down, expected_up) in [
            (0usize, "2.00", Command::SensDown, Command::SensUp),
            (3usize, "80%", Command::VolumeDown, Command::VolumeUp),
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
        let inv_rect = menu.row_rect_for(4, rows.len());
        assert_eq!(
            menu.press(
                inv_rect.center_x() as f64,
                (inv_rect.y + inv_rect.h * 0.5) as f64,
                &rows
            ),
            Some(Command::ToggleInvertPedals)
        );
        let s = Settings::parse(
            "sensitivity=NaN\ndead_zone=inf\nvolume=nan\ncamera=9\nfps=true\ninvert_pedals=true",
        );
        assert_eq!((s.sensitivity, s.dead_zone, s.volume), (2.0, 0.03, 0.8));
        assert_eq!(s.camera, 2);
        assert!(s.fps);
        assert!(s.invert_pedals);
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
        for page in [Page::Loading, Page::CameraEditor, Page::ControlsEditor] {
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
        assert_eq!(content.stages.len(), 4);
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
        menu.inspected_vehicle = 1;
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
            ..Settings::default()
        };
        let restored = Settings::parse(&preferences.encode());
        assert_eq!(restored.stage_id, preferences.stage_id);
        assert_eq!(restored.camera, 2);
        assert_eq!(restored.volume, 0.25);
        assert!(restored.invert_pedals);
    }
}
