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
    About,
    Paused,
    Driving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Open(Page),
    Back,
    ResetToMain,
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
    SelectVehicle,
    SelectStage,
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
            Page::Main => "ReRBR iOS",
            Page::Drive => "DRIVE",
            Page::Garage | Page::GarageDetail => "GARAGE",
            Page::Stages | Page::StageDetail => "STAGES",
            Page::Content | Page::ContentImport | Page::ContentCompatibility => "CONTENT MANAGER",
            Page::Settings => "SETTINGS",
            Page::Controls => "CONTROLS",
            Page::About => "ABOUT",
            Page::Paused => "PAUSED",
            Page::Driving => "DRIVING",
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
        let vehicle_supported = vehicle.validate_current_backend().is_ok();
        let stage_supported = stage.validate_current_backend().is_ok();
        match self.page {
            Page::Main => vec![
                Row::action("DRIVE", Command::Open(Page::Drive)),
                Row::action("GARAGE", Command::Open(Page::Garage)),
                Row::action("STAGES", Command::Open(Page::Stages)),
                Row::action("CONTENT MANAGER", Command::Open(Page::Content)),
                Row::action("SETTINGS", Command::Open(Page::Settings)),
                Row::action("CONTROLS", Command::Open(Page::Controls)),
                Row::action("ABOUT", Command::Open(Page::About)),
            ],
            Page::Drive => vec![
                Row::info("STAGE", stage.name.clone()),
                Row::info("VEHICLE", vehicle.name.clone()),
                Row {
                    label: if ctx.steering_ready {
                        "START DRIVING".into()
                    } else {
                        "START DRIVING - HOLD LANDSCAPE".into()
                    },
                    kind: RowKind::Action {
                        command: Command::Start,
                        enabled: ctx.steering_ready,
                    },
                },
                Row::action("BACK", Command::Back),
            ],
            Page::Garage => {
                let mut rows = vec![Row {
                    label: format!("{} - {}", vehicle.name, vehicle_state.label()),
                    kind: RowKind::Action {
                        command: Command::Open(Page::GarageDetail),
                        enabled: vehicle_supported,
                    },
                }];
                rows.push(Row::info(
                    "INSTALLED",
                    format!("{} CARS / 1 PRESET", ctx.content.vehicles.len()),
                ));
                rows.push(Row::action("BACK", Command::Back));
                rows
            }
            Page::GarageDetail => {
                let mut rows = Vec::new();
                if vehicle_state == EntryState::Selected {
                    rows.push(Row::info("SELECTION", "SELECTED"));
                } else {
                    rows.push(Row {
                        label: "SELECT VEHICLE".into(),
                        kind: RowKind::Action {
                            command: Command::SelectVehicle,
                            enabled: vehicle_supported,
                        },
                    });
                }
                rows.push(Row::action(
                    "INSPECT PRIVATE FILES",
                    Command::InspectContent,
                ));
                rows.push(Row::action("BACK", Command::Back));
                rows
            }
            Page::Stages => {
                let mut rows = vec![Row {
                    label: format!("{} - {}", stage.name, stage_state.label()),
                    kind: RowKind::Action {
                        command: Command::Open(Page::StageDetail),
                        enabled: stage_supported,
                    },
                }];
                rows.push(Row::info(
                    "INSTALLED",
                    format!("{} STAGES / 1 PRESET", ctx.content.stages.len()),
                ));
                rows.push(Row::action("BACK", Command::Back));
                rows
            }
            Page::StageDetail => {
                let mut rows = Vec::new();
                if stage_state == EntryState::Selected {
                    rows.push(Row::info("SELECTION", "SELECTED"));
                } else {
                    rows.push(Row {
                        label: "SELECT STAGE".into(),
                        kind: RowKind::Action {
                            command: Command::SelectStage,
                            enabled: stage_supported,
                        },
                    });
                }
                rows.push(Row::action(
                    "INSPECT PRIVATE FILES",
                    Command::InspectContent,
                ));
                rows.push(Row::action("BACK", Command::Back));
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
                    "STEERING SENSITIVITY",
                    Command::SensDown,
                    Command::SensUp,
                    format!("{:.2}", ctx.sensitivity),
                ),
                Row::stepper(
                    "STEERING DEAD ZONE",
                    Command::DeadDown,
                    Command::DeadUp,
                    format!("{:.2}", ctx.dead_zone),
                ),
                Row::action(format!("CAMERA: {}", ctx.camera), Command::CameraNext),
                Row::stepper(
                    "MASTER VOLUME",
                    Command::VolumeDown,
                    Command::VolumeUp,
                    format!("{:.0}%", ctx.volume * 100.0),
                ),
                Row::action(
                    format!("FPS COUNTER: {}", on_off(ctx.fps)),
                    Command::ToggleFps,
                ),
                Row::action(
                    format!("ADVANCED TELEMETRY: {}", on_off(ctx.diagnostics)),
                    Command::ToggleDiagnostics,
                ),
                Row::action(
                    format!("MINIMAL HUD: {}", on_off(ctx.minimal_hud)),
                    Command::ToggleMinimalHud,
                ),
                Row::action(
                    format!("INVERT PEDALS: {}", on_off(ctx.invert_pedals)),
                    Command::ToggleInvertPedals,
                ),
                Row::action("BACK", Command::Back),
            ],
            Page::Controls => vec![
                Row {
                    label: "CALIBRATE STEERING".into(),
                    kind: RowKind::Action {
                        command: Command::Calibrate,
                        enabled: ctx.steering_ready,
                    },
                },
                Row::action("RESET CALIBRATION", Command::ResetCalibration),
                Row::info("TRANSMISSION", format!("MANUAL {}", vehicle.forward_gears)),
                Row::info("SHIFTING", "TAP GEAR - / +"),
                Row::action("BACK", Command::Back),
            ],
            Page::About => vec![
                Row::info("VERSION", env!("CARGO_PKG_VERSION")),
                Row::info("BUILD", option_env!("GITHUB_RUN_NUMBER").unwrap_or("LOCAL")),
                Row::info("LICENSE", "GPL-3.0-OR-LATER"),
                Row::action("BACK", Command::Back),
            ],
            Page::Paused => vec![
                Row::action("RESUME", Command::Start),
                Row::action("RESTART / RESET TO START", Command::Restart),
                Row::action("SETTINGS", Command::Open(Page::Settings)),
                Row::action("CONTROLS", Command::Open(Page::Controls)),
                Row::action("MAIN MENU", Command::ResetToMain),
            ],
            Page::Driving => Vec::new(),
        }
    }
    pub fn subtitle(&self, ctx: &Context) -> Vec<String> {
        let vehicle = &ctx.selection.vehicle;
        let stage = &ctx.selection.stage;
        match self.page {
            Page::Main => vec![
                format!("{} / {}", stage.name, vehicle.name),
                format!("{} SPEED MANUAL / TILT STEERING", vehicle.forward_gears),
                "COROLLA AND HARWOOD ARE THE ONLY VALIDATED CONTENT".into(),
            ],
            Page::Drive => vec![
                format!("STAGE {} / {}", stage.name, stage.stem),
                format!(
                    "{} / {} FORWARD GEARS / MANUAL",
                    vehicle.name, vehicle.forward_gears
                ),
                if ctx.cockpit_ready {
                    "ORIGINAL COCKPIT READY".into()
                } else {
                    "COCKPIT ASSETS MISSING - EXTERIOR VIEWS REMAIN".into()
                },
            ],
            Page::Garage => {
                let audit = ctx
                    .content
                    .vehicle(&vehicle.id)
                    .map(|entry| entry.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                vec![
                    "READ-ONLY INSTALLED VEHICLE CATALOG".into(),
                    audit,
                    "PHYSICS STOCK DATA / NGP NOT CERTIFIED".into(),
                ]
            }
            Page::GarageDetail => {
                let audit = ctx
                    .content
                    .vehicle(&vehicle.id)
                    .map(|entry| entry.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                vec![
                    format!(
                        "ID {} / ORIGINAL SLOT {} / TYRE {}",
                        vehicle.id, vehicle.original_slot, vehicle.tyre_type
                    ),
                    format!(
                        "{} FORWARD GEARS / {}",
                        vehicle.forward_gears,
                        if ctx.cockpit_ready {
                            "COCKPIT READY"
                        } else {
                            "COCKPIT ASSETS MISSING"
                        }
                    ),
                    format!("MODEL {}", vehicle.model),
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
                    "HARWOOD FOREST IS THE ONLY VALIDATED STAGE".into(),
                ]
            }
            Page::StageDetail => {
                let audit = ctx
                    .content
                    .stage(&stage.id)
                    .map(|entry| entry.files.summary())
                    .unwrap_or_else(|| "PRIVATE FILES NOT INSPECTED".into());
                vec![
                    format!(
                        "ID {} / ORIGINAL MAP {} / DRY WEATHER",
                        stage.id, stage.original_map
                    ),
                    format!(
                        "STEM {} / {} TERRAIN BLOCKS EXPECTED",
                        stage.stem, stage.terrain_blocks
                    ),
                    format!("MAPS {}", stage.documents_maps),
                    audit,
                ]
            }
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
    /// Resolves a touch to a command using the same rectangles used for drawing.
    pub fn press(&self, x: f64, y: f64, rows: &[Row]) -> Option<Command> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let index = row_index(x, y, rows.len())?;
        match &rows[index].kind {
            RowKind::Action {
                command,
                enabled: true,
            } => Some(*command),
            RowKind::Stepper { down, up, value } => {
                let split_x = stepper_split_x(value);
                Some(if (x as f32) < split_x { *down } else { *up })
            }
            RowKind::Action { .. } | RowKind::Info { .. } => None,
        }
    }
    /// Draws the complete menu screen. Uses only the existing Metal HUD atlas.
    pub fn draw(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        let height = canvas.height();
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 1.0), ui::BACKDROP);
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.10), [10, 15, 24, 255]);
        ui::fill(canvas, Rect::new(0.055, 0.045, 0.008, 0.045), ACCENT);
        canvas.text(0.075 * 640.0, 0.034 * height, self.title(), 1.55, TEXT);
        ui::rule(canvas, 0.055, 0.105, 0.89, LINE);

        let subtitle = self.subtitle(ctx);
        let mut lines = Vec::new();
        for line in subtitle {
            lines.extend(ui::wrap(&line, 52));
        }
        let scale = 0.78;
        for (index, line) in lines.iter().take(5).enumerate() {
            canvas.text(
                0.055 * 640.0,
                (0.118 + index as f32 * 0.029) * height,
                line,
                scale,
                DIM,
            );
        }

        let rows = self.rows(ctx);
        let label_scale = if rows.len() > 7 { 0.76 } else { 0.82 };
        for (index, row) in rows.iter().enumerate() {
            let rect = row_rect(index, rows.len());
            let (x, y, w, h) = ui::px(rect, canvas);
            let interactive = matches!(
                row.kind,
                RowKind::Action { enabled: true, .. } | RowKind::Stepper { .. }
            );
            let panel = if interactive {
                PANEL_ACTIVE
            } else {
                PANEL_DEEP
            };
            canvas.rect(x, y, w, h, panel);
            if interactive {
                canvas.rect(x, y, 3.0, h, ACCENT);
            }
            canvas.rect(x, y, w, 1.0, LINE);
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
                        ACCENT,
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

        let footer = ui::wrap(status, 58);
        ui::rule(canvas, 0.055, 0.900, 0.89, LINE);
        let color =
            if status.contains("ERROR") || status.contains("FAIL") || status.contains("REJECTED") {
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

/// Normalized X midpoint between the drawn `-` and `+` stepper buttons in a row.
fn stepper_split_x(value: &str) -> f32 {
    let right_px = (ROW_X + ROW_WIDTH) * 640.0;
    let value_width = ui::texture_width(value, 0.78);
    (right_px - 34.0 - value_width * 0.5) / 640.0
}

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
fn row_index(x: f64, y: f64, rows: usize) -> Option<usize> {
    let height = row_height(rows);
    if rows == 0 || height <= 0.0 {
        return None;
    }
    let relative = (y as f32 - ROW_TOP) / height;
    if relative < 0.0 {
        return None;
    }
    let index = relative.floor() as usize;
    if index >= rows {
        return None;
    }
    let rect = row_rect(index, rows);
    rect.contains(x, y).then_some(index)
}

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub sensitivity: f64,
    pub dead_zone: f64,
    pub fps: bool,
    pub diagnostics: bool,
    pub minimal_hud: bool,
    pub invert_pedals: bool,
    pub volume: f64,
    pub camera: u8,
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
    pub fn save(self) {
        if let Some(path) = Self::path() {
            let text = format!(
                "sensitivity={}\ndead_zone={}\nfps={}\ndiagnostics={}\nminimal_hud={}\ninvert_pedals={}\nvolume={}\ncamera={}\n",
                self.sensitivity,
                self.dead_zone,
                self.fps,
                self.diagnostics,
                self.minimal_hud,
                self.invert_pedals,
                self.volume,
                self.camera
            );
            if let Err(error) = std::fs::write(path, text) {
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
        assert_eq!(rows[0].label, "DRIVE");
        for label in [
            "DRIVE",
            "GARAGE",
            "STAGES",
            "CONTENT MANAGER",
            "SETTINGS",
            "CONTROLS",
            "ABOUT",
        ] {
            assert!(rows.iter().any(|row| row.label == label), "{label}");
        }
        let drive_row = row_rect(0, rows.len());
        assert_eq!(
            menu.press(
                drive_row.center_x() as f64,
                drive_row.y as f64 + 0.01,
                &rows
            ),
            Some(Command::Open(Page::Drive))
        );
        menu.open(Page::Drive);
        let drive = menu.rows(&ctx);
        let start_row = drive
            .iter()
            .position(|row| {
                matches!(
                    row.kind,
                    RowKind::Action {
                        command: Command::Start,
                        ..
                    }
                )
            })
            .unwrap();
        let rect = row_rect(start_row, drive.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &drive),
            Some(Command::Start)
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
                        command: Command::Start,
                        ..
                    }
                )
            })
            .unwrap();
        let rect = row_rect(start_row, rows.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
            Some(Command::Start)
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
        assert_eq!(rows.len(), 9);
        // Verify tapping the drawn `-` glyph (on the right side, left of the value) decreases,
        // and tapping the drawn `+` glyph increases, for both Sensitivity (row 0) and Volume (row 3).
        for (row_idx, value, expected_down, expected_up) in [
            (0usize, "2.00", Command::SensDown, Command::SensUp),
            (3usize, "80%", Command::VolumeDown, Command::VolumeUp),
        ] {
            let rect = row_rect(row_idx, rows.len());
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
        let inv_rect = row_rect(7, rows.len());
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
        assert_eq!(rows.len(), 3);
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
        assert!(rows.iter().any(|row| row.label == "INSPECT PRIVATE FILES"));
        assert!(!rows.iter().any(|row| row.label.contains("SELECT VEHICLE")));
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
}
