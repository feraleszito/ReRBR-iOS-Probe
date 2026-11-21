//! Native menu model for the validated Corolla/Harwood preset.
//! Every row and hit region is shared; unsupported content is never offered as playable.

use super::content_manager::{physics_report, Catalog, DriveCapability, EntryState, IMPORT_STEPS};
use super::layout::Rect;
use super::ui::{self, ACCENT, BAD, DIM, GOOD, INFO, LINE, PANEL_ACTIVE, PANEL_DEEP, TEXT, WARN};
use crate::harwood::benchmark::hud::Canvas;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Title,
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
    SoundSettings,
    HudCameraSettings,
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
    SchoolBriefing,
    SchoolResult,
    CareerSlots,
    CareerCreate,
    CareerHub,
    Dealership,
    NewDealership,
    CareerGarage,
    CareerChampionship,
    CareerService,
}

impl Page {
    /// Pages that render transparently over the live 3D Rally Showroom.
    pub fn is_showroom_page(self) -> bool {
        matches!(
            self,
            Self::Title
                | Self::Main
                | Self::Garage
                | Self::Dealership
                | Self::NewDealership
                | Self::CareerGarage
        )
    }

    pub fn dealership_kind(self) -> Option<super::career::DealershipKind> {
        match self {
            Self::Dealership => Some(super::career::DealershipKind::UsedCars),
            Self::NewDealership => Some(super::career::DealershipKind::NewSalon),
            _ => None,
        }
    }

    /// True for the five main Settings sub-pages and their nested configuration screens,
    /// which display top-bar quick buttons for `< AJUSTES` and `MENÚ PRINCIPAL`.
    pub fn is_settings_subpage(self) -> bool {
        matches!(
            self,
            Self::Controls
                | Self::SoundSettings
                | Self::HudCameraSettings
                | Self::About
                | Self::Content
                | Self::ContentImport
                | Self::ContentCompatibility
                | Self::GamepadSettings
                | Self::PacenoteHudSettings
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCard {
    Controls,
    Sound,
    HudCamera,
    Credits,
    Content,
}

impl SettingsCard {
    pub const ALL: [Self; 5] = [
        Self::Controls,
        Self::Sound,
        Self::HudCamera,
        Self::Credits,
        Self::Content,
    ];

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Controls => "CONTROLES",
            Self::Sound => "SONIDO",
            Self::HudCamera => "HUD Y CÁMARA",
            Self::Credits => "CRÉDITOS",
            Self::Content => "CONTENIDO",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Controls => "GIROSCOPIO · TÁCTIL · MANDO",
            Self::Sound => "MOTOR FMOD · COPILOTO · MÚSICA",
            Self::HudCamera => "VISTAS · INDICADORES · NOTAS",
            Self::Credits => "PROYECTO · VERSIÓN · LICENCIAS",
            Self::Content => "COCHES · TRAMOS · DIAGNÓSTICO",
        }
    }

    pub fn target_page(self) -> Page {
        match self {
            Self::Controls => Page::Controls,
            Self::Sound => Page::SoundSettings,
            Self::HudCamera => Page::HudCameraSettings,
            Self::Credits => Page::About,
            Self::Content => Page::Content,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainSection {
    Career,
    TimeTrial,
    Online,
    Garage,
    NewCars,
    UsedCars,
    RallySchool,
    Settings,
}

impl MainSection {
    pub const ALL: [Self; 8] = [
        Self::Career,
        Self::TimeTrial,
        Self::Online,
        Self::Garage,
        Self::NewCars,
        Self::UsedCars,
        Self::RallySchool,
        Self::Settings,
    ];

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Career => "CARRERA",
            Self::TimeTrial => "CONTRARRELOJ",
            Self::Online => "ONLINE",
            Self::Garage => "GARAJE",
            Self::NewCars => "SALÓN 0KM",
            Self::UsedCars => "2ª MANO",
            Self::RallySchool => "ACADEMIA",
            Self::Settings => "AJUSTES",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Career => "MODO CARRERA",
            Self::TimeTrial => "TRAMOS Y PB",
            Self::Online => "PRÓXIMAMENTE",
            Self::Garage => "SHOWROOM 3D",
            Self::NewCars => "0 KM NUEVOS",
            Self::UsedCars => "USADOS 3D",
            Self::RallySchool => "17 LECCIONES",
            Self::Settings => "OPCIONES",
        }
    }

    pub fn is_coming_soon(self) -> bool {
        matches!(self, Self::Online)
    }

    pub fn coming_soon_detail(self) -> Option<&'static str> {
        match self {
            Self::Online => Some("ONLINE Y RANKINGS — PRÓXIMAMENTE EN UNA FASE POSTERIOR"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DealershipFilter {
    #[default]
    All,
    Initiation,
    Drivable,
}

impl DealershipFilter {
    pub fn next(self) -> Self {
        match self {
            Self::Initiation => Self::Drivable,
            Self::Drivable => Self::All,
            Self::All => Self::Initiation,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Initiation => "INICIACIÓN",
            Self::Drivable => "CONDUCIBLES",
            Self::All => "TODOS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GarageFilter {
    #[default]
    All,
    Drivable,
    VisualOnly,
}

impl GarageFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Drivable,
            Self::Drivable => Self::VisualOnly,
            Self::VisualOnly => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "TODOS",
            Self::Drivable => "CONDUCIBLES",
            Self::VisualOnly => "SOLO VISUAL",
        }
    }

    pub fn matches(self, cap: DriveCapability) -> bool {
        match self {
            Self::All => true,
            Self::Drivable => cap == DriveCapability::Drivable,
            Self::VisualOnly => cap == DriveCapability::VisualOnly,
        }
    }
}

pub(crate) fn format_stage_time(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    let mins = tenths / 600;
    let secs = (tenths % 600) as f64 / 10.0;
    format!("{mins:02}:{secs:04.1}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    EnterMainMenu,
    MainSectionActivate(usize),
    MainSectionStep(i32),
    GaragePrevCar,
    GarageNextCar,
    GarageFilterCycle,
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
    CycleHudRpmStyle,
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
    SchoolConfirmStart,
    SchoolFinishNow,
    SchoolRetryLesson,
    SchoolNextStartLesson,
    ExportDiagnostics,
    CareerPilotPrev,
    CareerPilotNext,
    CareerPilotTagNext,
    CareerPilotEditKeyboard,
    CareerNumberDown,
    CareerNumberUp,
    CareerCreateConfirm,
    CareerSlotSelect(usize),
    CareerSlotImportLegacy,
    CareerLoadDevMode,
    CareerDevResetCash,
    CareerDevToggleUnlocks,
    CareerResetRequest,
    CareerResetConfirm,
    CareerResetCancel,
    DealershipPrevCar,
    DealershipNextCar,
    DealershipFilterCycle,
    DealershipRequestBuy,
    DealershipConfirmBuy,
    DealershipCancelBuy,
    CareerGaragePrevCar,
    CareerGarageNextCar,
    CareerGarageSelectActive,
    CareerGarageRequestSell,
    CareerGarageConfirmSell,
    CareerGarageCancelSell,
    CareerDiffPrev,
    CareerDiffNext,
    CareerChampKindPrev,
    CareerChampKindNext,
    CareerChampStart,
    CareerChampRunNextStage,
    CareerChampAbandon,
    CareerChampRestoreAndDismiss,
    CareerChampDismissFinished,
    CareerServiceRepair(super::career::RepairComponent),
    CareerServiceRepairAll,
    CareerServiceTyrePrev,
    CareerServiceTyreNext,
    CareerServiceMountTyre,
    OpenCareerAfterStage,
    ScrollRowsUp,
    ScrollRowsDown,
    SettingsCardStep(i32),
    SettingsCardActivate(usize),
}

pub const MAX_VISIBLE_ROWS: usize = 10;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudRpmStyle {
    #[default]
    HorizontalBar,
    ShiftLights,
}

impl HudRpmStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::HorizontalBar => "BARRA HORIZONTAL",
            Self::ShiftLights => "LUCES DE CAMBIO",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::HorizontalBar => Self::ShiftLights,
            Self::ShiftLights => Self::HorizontalBar,
        }
    }
    pub fn encode(self) -> &'static str {
        match self {
            Self::HorizontalBar => "bar",
            Self::ShiftLights => "lights",
        }
    }
    pub fn parse(raw: &str) -> Self {
        match raw.trim() {
            "lights" | "shift_lights" => Self::ShiftLights,
            _ => Self::HorizontalBar,
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
    pub hud_rpm_style: HudRpmStyle,
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
    pub active_school_lesson: Option<u32>,
    pub school_result: Option<&'a super::school::SchoolRunResult>,
    pub personal_best_ms: Option<u64>,
    pub inspected_stage_pb_ms: Option<u64>,
    pub record_outcome: Option<&'a super::records::RecordOutcome>,
    /// Cumulative split times recorded during the finished run (ms, 720 Hz sim time).
    pub run_splits_ms: &'a [u64],
    /// Stored PB run's split times (snapshotted before overwrite on new PB) for exact per-split deltas.
    pub comparison_pb_splits_ms: Option<&'a [u64]>,
    /// Name of the vehicle currently showcased in the 3D Showroom (random on Title, last-driven on Main, inspected on Garage).
    pub showroom_vehicle_name: Option<&'a str>,
    pub showroom_loading: bool,
    pub showroom_error: Option<&'a str>,
    /// Smooth 0..1 pulse for the "TOCA PARA COMENZAR" prompt on Title Screen.
    pub title_pulse: f32,
    /// Smooth 0..1 screen transition fade (e.g. Title -> Main Menu).
    pub transition_alpha: f32,
    /// Optional non-blocking notice shown in the horizontal menu for "Próximamente" sections.
    pub coming_soon_notice: Option<&'a str>,
    /// Career profile store (independent from Time Trial PBs & retail championship).
    pub career: Option<&'a super::career::CareerStore>,
    /// True when the active/just-finished stage belongs to the Career Regional Championship.
    pub active_career_stage: bool,
    pub last_career_stage_outcome: Option<&'a super::career::StageRecordOutcome>,
    pub pilot_draft_name: &'a str,
    pub pilot_draft_number: u8,
    pub pending_reset_career: bool,
    pub pending_buy_vehicle_id: Option<&'a str>,
    pub pending_sell_vehicle_id: Option<&'a str>,
    pub selected_career_difficulty: super::career::CareerDifficulty,
    pub selected_championship_kind: super::career::ChampionshipKind,
    pub selected_service_tyre: super::career::TyreCompound,
}

#[derive(Debug)]
pub struct Menu {
    pub page: Page,
    stack: Vec<Page>,
    pub main_section: usize,
    pub settings_card: usize,
    pub garage_filter: GarageFilter,
    pub dealership_filter: DealershipFilter,
    pub inspected_vehicle: usize,
    pub dealership_vehicle: usize,
    pub career_garage_cursor: usize,
    pub inspected_stage: usize,
    pub garage_page: usize,
    pub stage_page: usize,
    pub row_scroll: usize,
    pub focused_row: usize,
    pub gamepad_focus_active: bool,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            page: Page::Title,
            stack: Vec::new(),
            main_section: 1, // Contrarreloj focused by default for immediate driving access
            settings_card: 0,
            garage_filter: GarageFilter::All,
            dealership_filter: DealershipFilter::Initiation,
            inspected_vehicle: 0,
            dealership_vehicle: 0,
            career_garage_cursor: 0,
            inspected_stage: 0,
            garage_page: 0,
            stage_page: 0,
            row_scroll: 0,
            focused_row: 0,
            gamepad_focus_active: false,
        }
    }
}
impl Menu {
    pub fn open(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        self.stack.push(self.page);
        self.page = page;
        self.row_scroll = 0;
        self.focused_row = 0;
    }
    pub fn back(&mut self) {
        self.page = self.stack.pop().unwrap_or(Page::Main);
        self.row_scroll = 0;
        self.focused_row = 0;
    }
    pub fn reset_to_main(&mut self) {
        self.stack.clear();
        self.page = Page::Main;
        self.row_scroll = 0;
        self.focused_row = 0;
    }
    pub fn show(&mut self, page: Page) {
        self.stack.clear();
        self.page = page;
        self.row_scroll = 0;
        self.focused_row = 0;
    }
    pub fn resume(&mut self) {
        self.stack.clear();
        self.page = Page::Driving;
        self.row_scroll = 0;
        self.focused_row = 0;
    }
    pub fn pause(&mut self) {
        if self.page == Page::Driving {
            self.stack.clear();
            self.page = Page::Paused;
            self.row_scroll = 0;
            self.focused_row = 0;
        }
    }

    pub fn visible_row_window(&self, total_rows: usize) -> (usize, usize) {
        if matches!(self.page, Page::CameraEditor | Page::ControlsEditor)
            || total_rows <= MAX_VISIBLE_ROWS
        {
            return (0, total_rows);
        }
        let max_scroll = total_rows.saturating_sub(MAX_VISIBLE_ROWS);
        let start = self.row_scroll.min(max_scroll);
        let end = (start + MAX_VISIBLE_ROWS).min(total_rows);
        (start, end)
    }

    pub fn scroll_rows(&mut self, total_rows: usize, delta: isize) {
        if total_rows <= MAX_VISIBLE_ROWS {
            self.row_scroll = 0;
            return;
        }
        let max_scroll = total_rows.saturating_sub(MAX_VISIBLE_ROWS) as isize;
        let next = (self.row_scroll as isize + delta).clamp(0, max_scroll);
        self.row_scroll = next as usize;
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
        if !matches!(self.page, Page::CameraEditor | Page::ControlsEditor)
            && rows.len() > MAX_VISIBLE_ROWS
        {
            if self.focused_row < self.row_scroll {
                self.row_scroll = self.focused_row;
            } else if self.focused_row >= self.row_scroll + MAX_VISIBLE_ROWS {
                self.row_scroll = self.focused_row + 1 - MAX_VISIBLE_ROWS;
            }
        }
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
            Page::Title | Page::Main | Page::Loading => "RICHARD BURNS RALLY",
            Page::Drive => "CONTRARRELOJ",
            Page::Garage | Page::GarageDetail => "GARAJE 3D",
            Page::Stages | Page::StageDetail => "SELECCIONAR TRAMO",
            Page::Content | Page::ContentImport | Page::ContentCompatibility => {
                "AJUSTES — CONTENIDO"
            }
            Page::Settings => "AJUSTES",
            Page::Controls => "AJUSTES — CONTROLES",
            Page::SoundSettings => "AJUSTES — SONIDO",
            Page::HudCameraSettings => "AJUSTES — HUD Y CÁMARA",
            Page::CameraEditor => "EDITOR DE CÁMARAS",
            Page::ControlsEditor => "EDITOR DE CONTROLES",
            Page::About => "AJUSTES — CRÉDITOS",
            Page::Paused => "PAUSA",
            Page::Driving => "CONDUCCIÓN",
            Page::StageComplete => "STAGE COMPLETE",
            Page::Replay => "REPETICIÓN",
            Page::NgpExperimental => "NGP EXPERIMENTAL",
            Page::GamepadSettings => "MANDO / GAMEPAD",
            Page::PacenoteHudSettings => "NOTAS VISUALES",
            Page::Championship => "CAMPEONATO RBR",
            Page::RallySchool => "ACADEMIA DE RALLY",
            Page::SchoolBriefing => "INSTRUCCIONES DE LECCIÓN",
            Page::SchoolResult => "RESULTADO ESCUELA DE RALLY",
            Page::CareerSlots => "ESPACIOS DE CARRERA (1-3)",
            Page::CareerCreate => "NUEVO PILOTO — CARRERA",
            Page::CareerHub => "MODO CARRERA",
            Page::Dealership => "CONCESIONARIO 2ª MANO",
            Page::NewDealership => "SALÓN AUTOMOTOR — 0 KM",
            Page::CareerGarage => "GARAJE DE CARRERA",
            Page::CareerChampionship => "CAMPEONATOS DE CARRERA",
            Page::CareerService => "PARQUE DE SERVICIO / TALLER",
        }
    }
    pub fn active_dealership_kind(&self) -> super::career::DealershipKind {
        self.page
            .dealership_kind()
            .unwrap_or(super::career::DealershipKind::UsedCars)
    }
    /// Filtered indices into `ctx.content.vehicles` for the active `GarageFilter`.
    pub fn garage_indices(&self, ctx: &Context) -> Vec<usize> {
        let indices = ctx
            .content
            .browse_vehicle_indices(|entry| entry.descriptor.id == ctx.selection.vehicle.id);
        let filtered: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&index| {
                self.garage_filter
                    .matches(ctx.content.vehicles[index].capability(ctx.ngp_experimental))
            })
            .collect();
        if filtered.is_empty() {
            indices
        } else {
            filtered
        }
    }
    /// Filtered indices into `ctx.content.vehicles` for a specific Dealership (`2ª MANO` or `SALÓN AUTOMOTOR 0 KM`),
    /// showing ALL catalog vehicles in `DealershipFilter::All` (including locked ones with clear badges),
    /// and sorting `DISPONIBLE` first, then `DINERO INSUFICIENTE`, then `BLOQUEADO`.
    pub fn dealership_indices_for(
        &self,
        ctx: &Context,
        kind: super::career::DealershipKind,
    ) -> Vec<usize> {
        let profile = ctx.career.and_then(|c| c.profile.as_ref());
        let candidates = ctx.content.browse_vehicle_indices(|entry| {
            profile.is_some_and(|profile| profile.owns_vehicle(&entry.descriptor.id))
        });
        let mut indices: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&index| {
                let e = &ctx.content.vehicles[index];
                let cap = e.capability(ctx.ngp_experimental);
                let spec = super::career::vehicle_market_spec(e);
                match self.dealership_filter {
                    DealershipFilter::Initiation => {
                        spec.offered_in(kind) && spec.tier.eligible_for_regional()
                    }
                    DealershipFilter::Drivable => {
                        spec.offered_in(kind) && cap == DriveCapability::Drivable
                    }
                    DealershipFilter::All => {
                        spec.offered_in(kind)
                            || spec.concesionario
                                == super::career::DealershipChannel::PendingCommercialRule
                    }
                }
            })
            .collect();
        if indices.is_empty() {
            indices = candidates
                .into_iter()
                .filter(|&index| {
                    super::career::vehicle_market_spec(&ctx.content.vehicles[index])
                        .offered_in(kind)
                })
                .collect();
        }
        indices.sort_by_key(|&i| {
            let e = &ctx.content.vehicles[i];
            let cap = e.capability(ctx.ngp_experimental);
            let spec = super::career::vehicle_market_spec(e);
            let price = spec.price_for(kind).unwrap_or(u64::MAX);
            let avail_rank = match profile {
                Some(p) => match p.dealership_availability(e, kind, ctx.ngp_experimental) {
                    super::career::DealershipAvailability::Available => 0u8,
                    super::career::DealershipAvailability::Owned => 1u8,
                    super::career::DealershipAvailability::InsufficientFunds { .. } => 2u8,
                    super::career::DealershipAvailability::Locked { .. } => 3u8,
                },
                None => 0u8,
            };
            let drivable_rank = if cap == DriveCapability::Drivable {
                0u8
            } else {
                1u8
            };
            let regional_rank = if spec.tier.eligible_for_regional() {
                0u8
            } else {
                1u8
            };
            (avail_rank, regional_rank, drivable_rank, price, i)
        });
        indices
    }
    /// Filtered indices for whichever dealership page is active (defaults to `2ª MANO`).
    pub fn dealership_indices(&self, ctx: &Context) -> Vec<usize> {
        self.dealership_indices_for(ctx, self.active_dealership_kind())
    }
    /// Indices into `ctx.content.vehicles` for vehicles OWNED in the active Career profile.
    pub fn career_garage_indices(&self, ctx: &Context) -> Vec<usize> {
        let Some(profile) = ctx.career.and_then(|c| c.profile.as_ref()) else {
            return Vec::new();
        };
        ctx.content
            .browse_vehicle_indices(|entry| profile.owns_vehicle(&entry.descriptor.id))
            .into_iter()
            .filter(|&i| profile.owns_vehicle(&ctx.content.vehicles[i].descriptor.id))
            .collect()
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
            Page::Title => vec![Row::action("TOCA PARA COMENZAR", Command::EnterMainMenu)],
            Page::Main => MainSection::ALL
                .iter()
                .enumerate()
                .map(|(i, sec)| Row::action(sec.title(), Command::MainSectionActivate(i)))
                .collect(),
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
            Page::Drive => {
                let pb_text = ctx
                    .personal_best_ms
                    .map(|ms| format!(" · PB {}", super::records::format_time_ms(ms)))
                    .unwrap_or_default();
                vec![
                    Row::action(
                        format!("COCHE ACTIVO: {}", vehicle.name),
                        Command::Open(Page::Garage),
                    ),
                    Row::action(
                        format!("TRAMO: {} ({}){pb_text}", stage.name, stage.surface()),
                        Command::Open(Page::Stages),
                    ),
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
                    Row::action(
                        format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                        Command::ToggleExperimentalMode,
                    ),
                    Row::action("DIAGNÓSTICO NGP (10 ETAPAS)", Command::RunFocusDiagnostics),
                    Row::action("VOLVER", Command::Back),
                ]
            }
            Page::Garage => {
                const PER_PAGE: usize = 9;
                let indices = ctx.content.browse_vehicle_indices(|entry| {
                    entry.descriptor.id == ctx.selection.vehicle.id
                });
                let total_vehicles = indices.len();
                let total_pages = total_vehicles.div_ceil(PER_PAGE);
                let page_idx = self.garage_page.min(total_pages.saturating_sub(1));
                let start = page_idx * PER_PAGE;
                let end = (start + PER_PAGE).min(total_vehicles);

                let mut rows: Vec<Row> = indices[start..end]
                    .iter()
                    .map(|&index| {
                        let entry = &ctx.content.vehicles[index];
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
                let incomplete_rsf = entry.is_some_and(|e| {
                    e.using_fallback_model
                        || e.model_missing
                        || (ctx.content.inspected && !e.files.missing.is_empty())
                });
                let is_visual_only = entry.is_some_and(|e| {
                    e.classification == rerbr_content::VehicleClassification::VisualInspectionOnly
                });
                let supported = if is_visual_only {
                    false
                } else if is_ngp_car {
                    ctx.ngp_experimental && !incomplete_rsf
                } else {
                    inspected.validate_current_backend().is_ok()
                };
                let mut rows = Vec::new();
                if is_visual_only {
                    rows.push(Row::info(
                        "CLASIFICACIÓN",
                        "SOLO VISUALIZACIÓN 3D (SIN FÍSICA INTEGRADA)",
                    ));
                } else if is_ngp_car {
                    let class_text = entry.map(|e| e.classification.badge()).unwrap_or("RSF NGP");
                    let fb_warn =
                        if entry.is_some_and(|e| e.using_fallback_model || e.model_missing) {
                            " [FALTA MODELO PROPIO]"
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
                if let Some(entry) = entry {
                    let spec = super::career::vehicle_market_spec(entry);
                    if let Some(yr) = spec.anio {
                        rows.push(Row::info("AÑO", yr.to_string()));
                    }
                    rows.push(Row::info("CANAL CARRERA", spec.concesionario.label()));
                }
                rows.push(Row::info("MODELO 3D", inspected.model.clone()));
                if let Some(pack) = inspected.rsf_pack.as_deref() {
                    rows.push(Row::info("PACK NGP", pack));
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
                        } else if is_visual_only {
                            "SOLO VISUALIZACIÓN (SIN FÍSICA NGP)".into()
                        } else if incomplete_rsf {
                            "INCOMPLETO (FALTAN ARCHIVOS PROPIOS)".into()
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
                        "DIAGNÓSTICO NGP (10 ETAPAS)",
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
                if let Some(entry) = entry {
                    for (idx, note) in entry.notes.iter().enumerate() {
                        rows.push(Row::info(
                            if idx == 0 { "ESTADO" } else { "DETALLE" },
                            note.clone(),
                        ));
                    }
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
                    rows.push(Row::info(
                        "RÉCORD PERSONAL",
                        ctx.inspected_stage_pb_ms
                            .map(super::records::format_time_ms)
                            .unwrap_or_else(|| "SIN RÉCORD".into()),
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
                Row::action(
                    format!(
                        "COCHES DISPONIBLES ({} EN CATÁLOGO)",
                        ctx.content.vehicles.len()
                    ),
                    Command::Open(Page::Garage),
                ),
                Row::action(
                    format!(
                        "TRAMOS DISPONIBLES ({} EN CATÁLOGO)",
                        ctx.content.stages.len()
                    ),
                    Command::Open(Page::Stages),
                ),
                Row::action(
                    format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                    Command::ToggleExperimentalMode,
                ),
                Row::action(
                    "DIAGNÓSTICO NGP / FMOD (10 ETAPAS)",
                    Command::RunFocusDiagnostics,
                ),
                Row::action(
                    "INFORME DE COMPATIBILIDAD Y BACKEND",
                    Command::Open(Page::ContentCompatibility),
                ),
                Row::action("ACTUALIZAR AUDITORÍA DE DOCUMENTS", Command::InspectContent),
                Row::disabled("IMPORT FROM IPAD FILES - UNAVAILABLE"),
                Row::action("VOLVER A AJUSTES", Command::Back),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::ContentImport => {
                let mut rows = IMPORT_STEPS
                    .iter()
                    .map(|step| Row::info(step.label, step.detail))
                    .collect::<Vec<_>>();
                rows.push(Row::action("VOLVER A AJUSTES", Command::Back));
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
                            format!("MODO NGP EXPERIMENTAL ({})", vehicle.name)
                        } else {
                            super::ngp_proto::PhysicsBackend::OriginalRetail
                                .label()
                                .into()
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
                    Row::action("VOLVER A CONTENIDO", Command::Back),
                ]
            }
            Page::Settings => SettingsCard::ALL
                .iter()
                .enumerate()
                .map(|(i, card)| Row::action(card.title(), Command::SettingsCardActivate(i)))
                .collect(),
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
                            format!("{} (MAP {})", entry.display_title(), entry.map),
                        ));
                        let rule_summary = if entry.perfect_time > 0.0
                            && entry.pass_time > 0.0
                            && entry.time_limit > 0.0
                        {
                            format!(
                                "{:.0}s / {:.0}s / {:.0}s",
                                entry.perfect_time, entry.pass_time, entry.time_limit
                            )
                        } else if entry.untimed {
                            "EJERCICIO GUIADO (SIN CRONO)".into()
                        } else {
                            entry.setup_file.into()
                        };
                        rows.push(Row::info("REGLAS", rule_summary));
                        let profile_progress = ctx
                            .career
                            .and_then(|career| career.profile.as_ref())
                            .map(|profile| &profile.school_progress);
                        let grade = profile_progress.map_or_else(
                            || entry.grade_label(),
                            |progress| {
                                super::school::medal_label(progress.grades[sch.selected]).into()
                            },
                        );
                        rows.push(Row::info(
                            "NOTA / ARCHIVOS",
                            if entry.assets_present {
                                format!("{grade} · ARCHIVOS OK")
                            } else {
                                format!(
                                    "{} · FALTA {}",
                                    grade,
                                    entry
                                        .missing_files
                                        .first()
                                        .map_or("ARCHIVOS", String::as_str)
                                )
                            },
                        ));
                        if let Some(last) = profile_progress.and_then(|progress| {
                            progress
                                .recent
                                .iter()
                                .rev()
                                .find(|attempt| attempt.lesson == sch.selected)
                        }) {
                            let time = last.elapsed_ms.map_or_else(
                                || "SIN CRONO".into(),
                                |elapsed| format_stage_time(f64::from(elapsed) / 1000.0),
                            );
                            rows.push(Row::info(
                                "ÚLTIMO INTENTO",
                                format!("{} · {time}", super::school::medal_label(last.grade)),
                            ));
                        }
                        rows.push(Row::info("TEXTOS / VOZ", sch.audio.status.label()));
                        rows.push(Row {
                            label: "ABRIR LECCIÓN (VER INSTRUCCIONES)".into(),
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
            Page::SchoolBriefing => vec![
                Row::action("INICIAR EJERCICIO", Command::SchoolConfirmStart),
                Row::action("SALIR AL MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::SchoolResult => {
                let mut rows = Vec::new();
                if ctx.has_replay {
                    rows.push(Row::action("VER REPETICIÓN", Command::StartReplay));
                }
                rows.push(Row::action("REPETIR LECCIÓN", Command::SchoolRetryLesson));
                if ctx.school_result.is_some_and(|r| r.lesson_index + 1 < 17) {
                    rows.push(Row::action(
                        "SIGUIENTE LECCIÓN",
                        Command::SchoolNextStartLesson,
                    ));
                }
                rows.push(Row::action("MENÚ PRINCIPAL", Command::ResetToMain));
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
                Row::stepper(
                    "SENSIBILIDAD GIROSCOPIO",
                    Command::SensDown,
                    Command::SensUp,
                    format!("{:.2}", ctx.sensitivity),
                ),
                Row::stepper(
                    "ZONA MUERTA DIRECCIÓN",
                    Command::DeadDown,
                    Command::DeadUp,
                    format!("{:.2}", ctx.dead_zone),
                ),
                Row {
                    label: "CALIBRAR GIROSCOPIO (POSICIÓN ACTUAL)".into(),
                    kind: RowKind::Action {
                        command: Command::Calibrate,
                        enabled: ctx.steering_ready,
                    },
                },
                Row::action("RESTABLECER CALIBRACIÓN", Command::ResetCalibration),
                Row::action(
                    format!(
                        "INVERTIR PEDALES (ACEL. / FRENO): {}",
                        on_off(ctx.invert_pedals)
                    ),
                    Command::ToggleInvertPedals,
                ),
                Row::action(
                    format!("AYUDA DE EMBRAGUE: {}", on_off(ctx.clutch_help)),
                    Command::ToggleClutchHelp,
                ),
                Row::action(
                    "EDITAR CONTROLES TÁCTILES EN PANTALLA",
                    Command::Open(Page::ControlsEditor),
                ),
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
                Row::action("VOLVER A AJUSTES", Command::Back),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::SoundSettings => vec![
                Row::stepper(
                    "VOLUMEN MOTOR Y EFECTOS",
                    Command::VolumeDown,
                    Command::VolumeUp,
                    format!("{:.0}%", ctx.volume * 100.0),
                ),
                Row::stepper(
                    "VOLUMEN COPILOTO (VOZ)",
                    Command::CodriverVolumeDown,
                    Command::CodriverVolumeUp,
                    format!("{:.0}%", ctx.codriver_volume * 100.0),
                ),
                Row::stepper(
                    "VOLUMEN MÚSICA DE MENÚ",
                    Command::MusicVolumeDown,
                    Command::MusicVolumeUp,
                    format!("{:.0}%", ctx.music_volume * 100.0),
                ),
                Row::info("AUDIO COCHE ACTIVO", vehicle.audio_index.clone()),
                Row::info("SALIDA DE AUDIO", "ESTÉREO 44.1 KHZ · FMOD / SÍNTESIS"),
                Row::action("VOLVER A AJUSTES", Command::Back),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::HudCameraSettings => vec![
                Row::action(
                    format!("VISTA DE CÁMARA ACTIVA: {}", ctx.camera),
                    Command::CameraNext,
                ),
                Row::action(
                    format!("MODO HUD MÍNIMO (VELOCÍMETRO): {}", on_off(ctx.minimal_hud)),
                    Command::ToggleMinimalHud,
                ),
                Row::action(
                    format!("ESTILO HUD RPM: {}", ctx.hud_rpm_style.label()),
                    Command::CycleHudRpmStyle,
                ),
                Row::action(
                    format!("CONTADOR DE FPS EN PANTALLA: {}", on_off(ctx.fps)),
                    Command::ToggleFps,
                ),
                Row::action(
                    format!("TELEMETRÍA E INDICADORES: {}", on_off(ctx.diagnostics)),
                    Command::ToggleDiagnostics,
                ),
                Row::action(
                    format!(
                        "NOTAS VISUALES COPILOTO: {}",
                        on_off(ctx.pacenote_hud.enabled)
                    ),
                    Command::Open(Page::PacenoteHudSettings),
                ),
                Row::action(
                    "POSICIÓN Y OPACIDAD CONTROLES HUD",
                    Command::Open(Page::ControlsEditor),
                ),
                Row {
                    label: if ctx.scene_loaded {
                        "EDITAR CÁMARAS 3D (FOV Y POSICIÓN)".into()
                    } else {
                        "EDITAR CÁMARAS 3D (INICIA UN TRAMO)".into()
                    },
                    kind: RowKind::Action {
                        command: Command::Open(Page::CameraEditor),
                        enabled: ctx.scene_loaded && cfg!(feature = "physics-stage"),
                    },
                },
                Row::action("VOLVER A AJUSTES", Command::Back),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
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
                    Row::action("VOLVER A CONTROLES", Command::Back),
                ]
            }
            Page::About => vec![
                Row::info("PROYECTO", "RBR MOBILE — PORT iOS / iPadOS (METAL / WGPU)"),
                Row::info("MOTOR BASE", "ReRBR POR JODUSNODUS"),
                Row::info(
                    "FÍSICAS Y RECURSOS",
                    "RICHARD BURNS RALLY · RALLYSIMFANS (NGP)",
                ),
                Row::info("VERSIÓN", env!("CARGO_PKG_VERSION")),
                Row::info(
                    "BUILD / LICENCIA",
                    format!(
                        "{} · GPL-3.0-OR-LATER",
                        option_env!("GITHUB_RUN_NUMBER").unwrap_or("LOCAL")
                    ),
                ),
                Row::action("VOLVER A AJUSTES", Command::Back),
                Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
            ],
            Page::Paused => {
                let mut rows = vec![
                    Row::action("REANUDAR", Command::Start),
                    Row::action("RESCATAR COCHE (AYUDA)", Command::RecoverCar),
                ];
                if ctx.active_school_lesson.is_some() {
                    rows.push(Row::action(
                        "FINALIZAR / EVALUAR LECCIÓN",
                        Command::SchoolFinishNow,
                    ));
                }
                rows.extend([
                    Row::action("REINICIAR TRAMO", Command::Restart),
                    Row::action("CONFIGURACIÓN", Command::Open(Page::Settings)),
                    Row::action("CONTROLES", Command::Open(Page::Controls)),
                    Row::action("MENÚ PRINCIPAL", Command::ResetToMain),
                ]);
                rows
            }
            Page::Driving => Vec::new(),
            Page::StageComplete => {
                let mut rows = Vec::new();
                if let Some(res) = ctx.stage_result {
                    rows.push(Row::info(
                        "TIEMPO CONDUCCIÓN",
                        super::records::format_time_ms(res.driving_ms),
                    ));
                    rows.push(Row::info(
                        "PENALIZACIONES",
                        if res.false_start {
                            format!("+{:.0}s SALIDA EN FALSO", res.penalty_seconds)
                        } else {
                            "0.000s".into()
                        },
                    ));
                    rows.push(Row::info(
                        "TIEMPO TOTAL",
                        super::records::format_time_ms(res.total_ms),
                    ));
                    rows.push(Row::info(
                        "CHECKPOINTS",
                        format!("{}/{}", res.splits_passed, res.splits_total),
                    ));
                }
                if !ctx.run_splits_ms.is_empty() {
                    let format_entry = |idx: usize, ms: u64| -> String {
                        let base = super::records::format_time_ms(ms);
                        match ctx
                            .comparison_pb_splits_ms
                            .and_then(|pb| pb.get(idx).copied())
                            .filter(|&pb_ms| pb_ms > 0)
                        {
                            Some(pb_ms) => format!(
                                "{base} ({})",
                                super::records::format_split_delta_ms((ms as i64) - (pb_ms as i64))
                            ),
                            None => base,
                        }
                    };
                    if ctx.run_splits_ms.len() <= 4 {
                        for (idx, &ms) in ctx.run_splits_ms.iter().enumerate() {
                            rows.push(Row::info(
                                format!("PARCIAL {}", idx + 1),
                                format_entry(idx, ms),
                            ));
                        }
                    } else {
                        for (chunk_idx, pair) in ctx.run_splits_ms.chunks(2).enumerate() {
                            let i0 = chunk_idx * 2;
                            if pair.len() == 2 {
                                rows.push(Row::info(
                                    format!("PARCIAL {}/{}", i0 + 1, i0 + 2),
                                    format!(
                                        "{} | {}",
                                        format_entry(i0, pair[0]),
                                        format_entry(i0 + 1, pair[1])
                                    ),
                                ));
                            } else {
                                rows.push(Row::info(
                                    format!("PARCIAL {}", i0 + 1),
                                    format_entry(i0, pair[0]),
                                ));
                            }
                        }
                    }
                }
                if let Some(outcome) = ctx.record_outcome {
                    rows.push(Row::info("RÉCORD PERSONAL", outcome.summary_line()));
                } else if let Some(pb) = ctx.personal_best_ms {
                    rows.push(Row::info(
                        "RÉCORD PERSONAL",
                        super::records::format_time_ms(pb),
                    ));
                }
                if ctx.active_career_stage {
                    if let Some(out) = ctx.last_career_stage_outcome {
                        let summary = if out.championship_finished {
                            format!(
                                "P{} FINAL · PREMIO {}",
                                out.current_position,
                                super::career::format_usd(out.prize_awarded_usd)
                            )
                        } else {
                            format!(
                                "ETAPA {}/3 REGISTRADA · P{} GENERAL",
                                out.stage_index + 1,
                                out.current_position
                            )
                        };
                        rows.push(Row::info("COPA REGIONAL", summary));
                    }
                    rows.push(Row::action(
                        "VOLVER A COPA REGIONAL",
                        Command::OpenCareerAfterStage,
                    ));
                }
                if ctx.has_replay {
                    rows.push(Row::action("VER REPETICIÓN", Command::StartReplay));
                }
                if !ctx.active_career_stage {
                    rows.push(Row::action("REINICIAR TRAMO", Command::Restart));
                }
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
                            "INICIAR PRUEBA NGP + AUDIO FMOD".into()
                        } else if visual_ready {
                            "INSPECCIÓN VISUAL (SIN FÍSICA NGP)".into()
                        } else {
                            "PRUEBA BLOQUEADA (REVISAR ARCHIVOS)".into()
                        },
                        kind: RowKind::Action {
                            command: Command::StartFocusVisualInspection,
                            enabled: visual_ready,
                        },
                    },
                    Row::action("VOLVER", Command::Back),
                ]
            }
            Page::CareerSlots => {
                let mut rows = Vec::new();
                if let Some(career) = ctx.career {
                    for i in 0..super::career::CAREER_SLOT_COUNT {
                        let active_tag = if career.active_slot == i {
                            " [ACTIVO]"
                        } else {
                            ""
                        };
                        let label = match (&career.slots[i], &career.slot_errors[i]) {
                            (Some(p), _) => format!(
                                "ESPACIO {}{active_tag}: #{} {} · {} · {} COCHES",
                                i + 1,
                                p.competition_number,
                                p.pilot_name,
                                super::career::format_usd(p.cash_usd),
                                p.owned_vehicle_ids.len()
                            ),
                            (None, Some(err)) => format!("ESPACIO {}{active_tag}: {err}", i + 1),
                            (None, None) => {
                                format!("ESPACIO {}{active_tag}: VACÍO (NUEVA CARRERA)", i + 1)
                            }
                        };
                        rows.push(Row::action(label, Command::CareerSlotSelect(i)));
                    }
                    if let Some(legacy) = &career.pending_legacy_profile {
                        rows.push(Row::action(
                            format!(
                                "IMPORTAR GUARDADO ANTERIOR (#{} {} · {}) AL ESPACIO {}",
                                legacy.competition_number,
                                legacy.pilot_name,
                                super::career::format_usd(legacy.cash_usd),
                                career.active_slot + 1
                            ),
                            Command::CareerSlotImportLegacy,
                        ));
                    }
                }
                #[cfg(feature = "career-dev-mode")]
                rows.push(Row::action(
                    "PERFIL DESARROLLADOR QA (US$9,999,999,999 · AISLADO)",
                    Command::CareerLoadDevMode,
                ));
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::CareerCreate => vec![
                Row::stepper(
                    "NOMBRE DEL PILOTO",
                    Command::CareerPilotPrev,
                    Command::CareerPilotNext,
                    ctx.pilot_draft_name,
                ),
                Row::action(
                    "ESCRIBIR NOMBRE CON TECLADO",
                    Command::CareerPilotEditKeyboard,
                ),
                Row::stepper(
                    "DORSAL DE COMPETICIÓN",
                    Command::CareerNumberDown,
                    Command::CareerNumberUp,
                    format!("#{}", ctx.pilot_draft_number),
                ),
                Row::info(
                    "PRESUPUESTO INICIAL",
                    super::career::format_usd(super::career::INITIAL_CASH_USD),
                ),
                Row::info("COCHES EN PROPIEDAD", "0 (COMPRA EN 2ª MANO)"),
                Row::action(
                    format!(
                        "CREAR PERFIL ({})",
                        super::career::format_usd(super::career::INITIAL_CASH_USD)
                    ),
                    Command::CareerCreateConfirm,
                ),
                Row::action("ELEGIR ESPACIO (1-3)", Command::Open(Page::CareerSlots)),
                #[cfg(feature = "career-dev-mode")]
                Row::action(
                    "PERFIL DESARROLLADOR QA (US$9,999,999,999)",
                    Command::CareerLoadDevMode,
                ),
                Row::action("VOLVER", Command::Back),
            ],
            Page::CareerHub => {
                let mut rows = Vec::new();
                let slot_num = ctx.career.map_or(1, |c| c.active_slot + 1);
                if let Some(p) = ctx.career.and_then(|c| c.profile.as_ref()) {
                    let active_car_name = p
                        .active_career_vehicle_id
                        .as_deref()
                        .and_then(|id| ctx.content.vehicle(id))
                        .map(|e| e.descriptor.name.clone())
                        .unwrap_or_else(|| "NINGUNO (COMPRA EN 2ª MANO)".into());
                    rows.push(Row::info(
                        if p.is_dev_profile {
                            "MODO DESARROLLADOR QA (AISLADO)".to_string()
                        } else {
                            format!("ESPACIO {slot_num}/3 · PILOTO")
                        },
                        format!(
                            "#{} {} · {}",
                            p.competition_number,
                            p.pilot_name,
                            super::career::format_usd(p.cash_usd)
                        ),
                    ));
                    rows.push(Row::info("COCHE DE CARRERA", active_car_name));
                    rows.push(Row::info(
                        "PALMARÉS",
                        format!(
                            "{} VICTORIAS · {} PODIOS · HISTÓRICOS {}",
                            p.wins,
                            p.podiums,
                            if p.historics_unlocked {
                                "DESBLOQUEADOS"
                            } else {
                                "BLOQUEADOS"
                            }
                        ),
                    ));
                    let champ_label = match &p.active_championship {
                        Some(c) if !c.finished => format!(
                            "CONTINUAR {} ({}/{})",
                            c.title,
                            c.completed_stages(),
                            c.total_stages()
                        ),
                        Some(c) => format!("VER RESULTADOS {}", c.title),
                        None => "CAMPEONATOS (REGIONAL / HISTÓRICO)".into(),
                    };
                    rows.push(Row::action(
                        champ_label,
                        Command::Open(Page::CareerChampionship),
                    ));
                    if p.active_career_vehicle_id.is_some() {
                        rows.push(Row::action(
                            "PARQUE DE SERVICIO / NEUMÁTICOS",
                            Command::Open(Page::CareerService),
                        ));
                    }
                    rows.push(Row::action(
                        format!("GARAJE DE CARRERA ({} PROPIOS)", p.owned_vehicle_ids.len()),
                        Command::Open(Page::CareerGarage),
                    ));
                    rows.push(Row::action(
                        "SALÓN AUTOMOTOR (COCHES 0 KM · 3D)",
                        Command::Open(Page::NewDealership),
                    ));
                    rows.push(Row::action(
                        "CONCESIONARIO 2ª MANO (USADOS · 3D)",
                        Command::Open(Page::Dealership),
                    ));
                    rows.push(Row::action(
                        format!("CAMBIAR ESPACIO DE GUARDADO ({slot_num}/3)"),
                        Command::Open(Page::CareerSlots),
                    ));
                    #[cfg(feature = "career-dev-mode")]
                    {
                        if p.is_dev_profile {
                            rows.push(Row::action(
                                "RESTABLECER SALDO DEV (US$9,999,999,999)",
                                Command::CareerDevResetCash,
                            ));
                            rows.push(Row::action(
                                if p.dev_unlock_all && p.historics_unlocked {
                                    "DESBLOQUEOS QA: ACTIVADOS (PULSA PARA RESTABLECER)"
                                } else {
                                    "DESBLOQUEOS QA: DESACTIVADOS (PULSA PARA DESBLOQUEAR TODO)"
                                },
                                Command::CareerDevToggleUnlocks,
                            ));
                        } else {
                            rows.push(Row::action(
                                "PERFIL DESARROLLADOR QA (US$9,999,999,999)",
                                Command::CareerLoadDevMode,
                            ));
                        }
                    }
                    if !p.is_dev_profile {
                        if ctx.pending_reset_career {
                            rows.push(Row::action(
                                format!("CONFIRMAR REINICIO ESPACIO {slot_num} (BORRAR TODO)"),
                                Command::CareerResetConfirm,
                            ));
                            rows.push(Row::action(
                                "CANCELAR REINICIO DE CARRERA",
                                Command::CareerResetCancel,
                            ));
                        } else {
                            rows.push(Row::action(
                                format!("REINICIAR CARRERA (ESPACIO {slot_num})"),
                                Command::CareerResetRequest,
                            ));
                        }
                    }
                } else {
                    rows.push(Row::action(
                        format!("CREAR PERFIL EN ESPACIO {slot_num}/3"),
                        Command::Open(Page::CareerCreate),
                    ));
                    rows.push(Row::action(
                        "ELEGIR ESPACIO DE GUARDADO (1-3)",
                        Command::Open(Page::CareerSlots),
                    ));
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::Dealership | Page::NewDealership => vec![
                Row::action("ANTERIOR", Command::DealershipPrevCar),
                Row::action("SIGUIENTE", Command::DealershipNextCar),
                Row::action("VOLVER", Command::Back),
            ],
            Page::CareerGarage => vec![
                Row::action("ANTERIOR", Command::CareerGaragePrevCar),
                Row::action("SIGUIENTE", Command::CareerGarageNextCar),
                Row::action("VOLVER", Command::Back),
            ],
            Page::CareerChampionship => {
                let mut rows = Vec::new();
                if let Some(p) = ctx.career.and_then(|c| c.profile.as_ref()) {
                    if let Some(champ) = &p.active_championship {
                        if champ.finished {
                            if !champ.restoration_settled && champ.end_restoration_cost_usd > 0 {
                                let can_afford = p.cash_usd >= champ.end_restoration_cost_usd;
                                rows.push(Row {
                                    label: format!(
                                        "RESTAURAR COCHE ({}) Y FINALIZAR",
                                        super::career::format_usd(champ.end_restoration_cost_usd)
                                    ),
                                    kind: RowKind::Action {
                                        command: Command::CareerChampRestoreAndDismiss,
                                        enabled: can_afford,
                                    },
                                });
                                rows.push(Row::action(
                                    "FINALIZAR SIN RESTAURAR (CONSERVAR DAÑOS)",
                                    Command::CareerChampDismissFinished,
                                ));
                            } else {
                                rows.push(Row::action(
                                    "FINALIZAR Y VOLVER A CARRERA",
                                    Command::CareerChampDismissFinished,
                                ));
                            }
                        } else {
                            let next_idx = champ.next_stage_index().unwrap_or(0);
                            let stage_name = champ
                                .kind
                                .stages()
                                .get(next_idx)
                                .map_or("TRAMO", |s| s.display_name);
                            if champ.service_window_open {
                                rows.push(Row::action(
                                    "PARQUE DE SERVICIO (REPARAR / NEUMÁTICOS)",
                                    Command::Open(Page::CareerService),
                                ));
                            }
                            rows.push(Row {
                                label: if ctx.steering_ready {
                                    format!(
                                        "CORRER ETAPA {}/{}: {}",
                                        next_idx + 1,
                                        champ.total_stages(),
                                        stage_name
                                    )
                                } else {
                                    "SUJETA EN HORIZONTAL PARA CORRER".into()
                                },
                                kind: RowKind::Action {
                                    command: Command::CareerChampRunNextStage,
                                    enabled: ctx.steering_ready,
                                },
                            });
                            rows.push(Row::action(
                                "ABANDONAR CAMPEONATO (SIN PREMIO)",
                                Command::CareerChampAbandon,
                            ));
                        }
                    } else {
                        let kind = ctx.selected_championship_kind;
                        let diff = ctx.selected_career_difficulty;
                        rows.push(Row::stepper(
                            "CAMPEONATO",
                            Command::CareerChampKindPrev,
                            Command::CareerChampKindNext,
                            if kind == super::career::ChampionshipKind::Historic
                                && !p.historics_unlocked
                            {
                                format!("{} [BLOQUEADO]", kind.title())
                            } else {
                                kind.title().to_string()
                            },
                        ));
                        rows.push(Row::stepper(
                            "DIFICULTAD",
                            Command::CareerDiffPrev,
                            Command::CareerDiffNext,
                            format!(
                                "{} (1º {})",
                                diff.label(),
                                super::career::format_usd(diff.prize_for_position(kind, 1))
                            ),
                        ));
                        let is_eligible = |e: &super::content_manager::VehicleEntry| {
                            let tier = super::career::vehicle_market_spec(e).tier;
                            match kind {
                                super::career::ChampionshipKind::Regional => {
                                    tier.eligible_for_regional()
                                }
                                super::career::ChampionshipKind::Historic => {
                                    tier.eligible_for_historic()
                                }
                            }
                        };
                        let eligible_owned = p
                            .active_career_vehicle_id
                            .as_deref()
                            .and_then(|id| ctx.content.vehicle(id))
                            .filter(|e| {
                                e.capability(ctx.ngp_experimental) == DriveCapability::Drivable
                                    && is_eligible(e)
                            })
                            .or_else(|| {
                                ctx.content.vehicles.iter().find(|e| {
                                    p.owns_vehicle(&e.descriptor.id)
                                        && e.capability(ctx.ngp_experimental)
                                            == DriveCapability::Drivable
                                        && is_eligible(e)
                                })
                            });
                        if kind == super::career::ChampionshipKind::Historic
                            && !p.historics_unlocked
                        {
                            rows.push(Row::info(
                                "REQUISITO HISTÓRICOS",
                                "CONSIGUE UN PODIO (TOP 3) EN COPA REGIONAL",
                            ));
                        } else if let Some(car) = eligible_owned {
                            rows.push(Row::action(
                                format!("INICIAR CAMPEONATO CON {}", car.descriptor.name),
                                Command::CareerChampStart,
                            ));
                        } else {
                            rows.push(Row::action(
                                match kind {
                                    super::career::ChampionshipKind::Regional => {
                                        "COMPRAR COCHE DE INICIACIÓN EN 2ª MANO"
                                    }
                                    super::career::ChampionshipKind::Historic => {
                                        "COMPRAR COCHE HISTÓRICO / GRUPO A EN 2ª MANO"
                                    }
                                },
                                Command::Open(Page::Dealership),
                            ));
                        }
                    }
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
            Page::CareerService => {
                let mut rows = Vec::new();
                if let Some(p) = ctx.career.and_then(|c| c.profile.as_ref()) {
                    let target_id = p
                        .active_championship
                        .as_ref()
                        .filter(|c| !c.finished)
                        .map(|c| c.locked_vehicle_id.as_str())
                        .or(p.active_career_vehicle_id.as_deref());
                    if let Some(entry) = target_id.and_then(|id| ctx.content.vehicle(id)) {
                        let st = p
                            .vehicle_state(&entry.descriptor.id)
                            .cloned()
                            .unwrap_or_else(|| {
                                super::career::OwnedVehicleState::factory_new(&entry.descriptor.id)
                            });
                        let quote = p.service_quote(entry, ctx.selected_service_tyre);
                        let service_allowed = p.can_service_vehicle(&entry.descriptor.id).is_ok();
                        let next_surface = p
                            .active_championship
                            .as_ref()
                            .and_then(|c| c.next_stage_index().and_then(|i| c.kind.stages().get(i)))
                            .map(|s| s.surface);

                        rows.push(Row::info(
                            "COCHE / SALDO",
                            format!(
                                "{} ({}%) · {}",
                                entry.descriptor.name,
                                st.condition.overall_pct(),
                                super::career::format_usd(p.cash_usd)
                            ),
                        ));
                        let compat_note = match next_surface {
                            Some(surf)
                                if !st.installed_tyre.is_compatible_with_stage_surface(surf) =>
                            {
                                format!(
                                    "{} ({}% REST.) [INCOMPATIBLE {}]",
                                    st.installed_tyre.label(),
                                    st.condition.tyre_remaining_pct(),
                                    super::career::TyreSurface::from_stage_surface(surf).label()
                                )
                            }
                            _ => format!(
                                "{} ({}% RESTANTE)",
                                st.installed_tyre.label(),
                                st.condition.tyre_remaining_pct()
                            ),
                        };
                        rows.push(Row::info("NEUMÁTICOS ACTUALES", compat_note));

                        for comp in super::career::RepairComponent::ALL {
                            let pct = match comp {
                                super::career::RepairComponent::Engine => st.condition.engine_pct,
                                super::career::RepairComponent::Transmission => {
                                    st.condition.transmission_pct
                                }
                                super::career::RepairComponent::Suspension => {
                                    st.condition.suspension_pct
                                }
                                super::career::RepairComponent::BodyElectrics => {
                                    st.condition.body_electrics_pct
                                }
                            };
                            let cost = quote.cost_for(comp);
                            let label = if cost == 0 {
                                format!("{} — 100% (INTACTO)", comp.label())
                            } else {
                                format!(
                                    "REPARAR {} ({}% -> 100% · {})",
                                    comp.label(),
                                    pct,
                                    super::career::format_usd(cost)
                                )
                            };
                            rows.push(Row {
                                label,
                                kind: RowKind::Action {
                                    command: Command::CareerServiceRepair(comp),
                                    enabled: service_allowed && cost > 0 && p.cash_usd >= cost,
                                },
                            });
                        }

                        rows.push(Row {
                            label: if quote.total_repair_cost_usd > 0 {
                                format!(
                                    "REPARAR TODO ({})",
                                    super::career::format_usd(quote.total_repair_cost_usd)
                                )
                            } else {
                                "MECÁNICA AL 100% (SIN REPARACIONES)".into()
                            },
                            kind: RowKind::Action {
                                command: Command::CareerServiceRepairAll,
                                enabled: service_allowed
                                    && quote.total_repair_cost_usd > 0
                                    && p.cash_usd >= quote.total_repair_cost_usd,
                            },
                        });

                        rows.push(Row::stepper(
                            "COMPUESTO NEUMÁTICOS",
                            Command::CareerServiceTyrePrev,
                            Command::CareerServiceTyreNext,
                            ctx.selected_service_tyre.label().to_string(),
                        ));
                        rows.push(Row {
                            label: format!(
                                "MONTAR JUEGO NUEVO ({})",
                                super::career::format_usd(quote.tyre_replacement_cost_usd)
                            ),
                            kind: RowKind::Action {
                                command: Command::CareerServiceMountTyre,
                                enabled: service_allowed
                                    && p.cash_usd >= quote.tyre_replacement_cost_usd,
                            },
                        });
                    } else {
                        rows.push(Row::info(
                            "SIN COCHE ACTIVO",
                            "COMPRA UN COCHE EN EL CONCESIONARIO",
                        ));
                    }
                }
                rows.push(Row::action("VOLVER", Command::Back));
                rows
            }
        }
    }
    pub fn subtitle(&self, ctx: &Context) -> Vec<String> {
        let vehicle = &ctx.selection.vehicle;
        let stage = &ctx.selection.stage;
        match self.page {
            Page::Title | Page::Main | Page::Loading => vec![
                "RBR MOBILE".into(),
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
                        if ctx.ngp_experimental {
                            "EXPERIMENTAL NGP: MODEL + PACK (SIN FMOD)".into()
                        } else {
                            "BLOCKED: ACTIVA MODO NGP EXPERIMENTAL".into()
                        }
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
                "GESTIÓN DE COCHES, TRAMOS Y AUDITORÍA DE ARCHIVOS EN DOCUMENTS".into(),
                "DIAGNÓSTICO DE FÍSICAS NGP 7.4 Y BANCOS DE AUDIO FMOD".into(),
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
                "SELECCIONA UN APARTADO DE CONFIGURACIÓN".into(),
                "TODOS LOS CAMBIOS SE GUARDAN AUTOMÁTICAMENTE EN DOCUMENTS".into(),
            ],
            Page::Controls => vec![
                "GIROSCOPIO, CALIBRACIÓN, PEDALES, EMBRAGUE Y MANDO".into(),
                format!(
                    "TRANSMISIÓN MANUAL {} VEL. · PULSA GEAR - / + PARA CAMBIAR",
                    vehicle.forward_gears
                ),
            ],
            Page::SoundSettings => vec![
                "VOLUMEN DE MOTOR FMOD, VOZ DEL COPILOTO Y MÚSICA DE MENÚ".into(),
                "LOS AJUSTES DE AUDIO SE APLICAN Y GUARDAN AL INSTANTE".into(),
            ],
            Page::HudCameraSettings => vec![
                "VISTAS DE CÁMARA, VELOCÍMETRO, INDICADORES, FPS Y NOTAS VISUALES".into(),
                "LOS CAMBIOS DE CÁMARA Y HUD SE APLICAN INMEDIATAMENTE".into(),
            ],
            Page::About => vec![
                "CRÉDITOS DEL PROYECTO, VERSIÓN, RECONOCIMIENTOS Y LICENCIAS".into(),
                "ReRBR BY JODUSNODUS / GPL-3.0-OR-LATER · PORT iOS / iPadOS".into(),
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
                "DIAGNÓSTICO 10 ETAPAS — FORD FOCUS RS WRC 2006 (NO CERTIFICADO)".into(),
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
            Page::RallySchool => {
                let summary = ctx
                    .career
                    .and_then(|career| career.profile.as_ref())
                    .map(|profile| {
                        let [bronze, silver, gold] = profile.school_progress.medals();
                        let summary = format!(
                            "{} · {bronze} BRONCE / {silver} PLATA / {gold} ORO",
                            profile.pilot_name
                        );
                        match profile.school_progress.achievement() {
                            Some(award) => format!("{summary} · {award}"),
                            None => summary,
                        }
                    })
                    .unwrap_or_else(|| "17 LECCIONES ORIGINALES".into());
                vec![
                    summary,
                    ctx.school
                        .map_or_else(|| "SIN STRINGS.RBZ".into(), |s| s.strings_status.clone()),
                ]
            }
            Page::SchoolBriefing => {
                if let Some((idx, sch)) = ctx.active_school_lesson.zip(ctx.school) {
                    if let Some(entry) = sch.entries.get(idx as usize) {
                        return vec![
                            format!("LECCIÓN {}/17 · {}", idx + 1, entry.display_title()),
                            format!(
                                "MAP {} · {} · CAR05 IMPREZA 2003",
                                entry.map, entry.setup_file
                            ),
                        ];
                    }
                }
                vec!["ESCUELA DE RALLY · PREPARADO EN SALIDA".into()]
            }
            Page::SchoolResult => {
                if let Some(res) = ctx.school_result {
                    vec![
                        format!("LECCIÓN {}/17 · {}", res.lesson_index + 1, res.title),
                        res.headline.clone(),
                    ]
                } else {
                    vec!["EVALUACIÓN DE LECCIÓN FINALIZADA".into()]
                }
            }
            Page::CareerCreate => vec![
                "CREA TU PERFIL DE PILOTO PARA INICIAR EL MODO CARRERA".into(),
                "EMPIEZAS CON US$15,000 Y 0 COCHES · GUARDADO INDEPENDIENTE".into(),
            ],
            Page::CareerHub => vec![
                "PERFIL, ECONOMÍA USD, GARAJE PROPIO Y COPA REGIONAL".into(),
                "CONTRARRELOJ SIGUE LIBRE CON CUALQUIER COCHE CONDUCIBLE".into(),
            ],
            Page::Dealership => vec![
                "CONCESIONARIO 2ª MANO · COCHES USADOS EN USD".into(),
                "SOLO COCHES CONDUCIBLES SE PUEDEN COMPRAR".into(),
            ],
            Page::NewDealership => vec![
                "SALÓN AUTOMOTOR · COCHES 0 KM NUEVOS EN USD".into(),
                "SOLO COCHES CONDUCIBLES SE PUEDEN COMPRAR".into(),
            ],
            Page::CareerGarage => vec![
                "GARAJE DE CARRERA · SOLO COCHES DE TU PROPIEDAD".into(),
                "SEPARADO DEL GARAJE GENERAL DE CONTRARRELOJ".into(),
            ],
            Page::CareerSlots => vec![
                "TRES PARTIDAS INDEPENDIENTES DE MODO CARRERA".into(),
                "MIGRACIÓN AUTOMÁTICA DE GUARDADOS ANTERIORES".into(),
            ],
            Page::CareerChampionship => vec![
                "REGIONAL (INICIACIÓN) Y CAMPEONATO HISTÓRICO (DESBLOQUEABLE CON PODIO)".into(),
                "ASISTENCIA MECÁNICA Y NEUMÁTICOS DISPONIBLE ENTRE ETAPAS".into(),
            ],
            Page::CareerService => vec![
                "REPARACIÓN DE DAÑOS NGP, DESGASTE Y ELECCIÓN DE NEUMÁTICOS".into(),
                "LOS COSTES ESCALAN SEGÚN CATEGORÍA Y DAÑO REAL DEL VEHÍCULO".into(),
            ],
        }
    }
    fn row_bounds(&self) -> (f32, f32, f32, f32) {
        match self.page {
            Page::CameraEditor => (0.03, 0.43, 0.10, 0.94),
            Page::Loading => (0.28, 0.44, 0.76, 0.86),
            Page::ControlsEditor => (0.28, 0.44, 0.04, 0.36),
            Page::NgpExperimental => (0.055, 0.89, 0.615, 0.89),
            Page::SchoolBriefing | Page::SchoolResult | Page::CareerChampionship => {
                (0.055, 0.89, 0.655, 0.89)
            }
            _ => (ROW_X, ROW_WIDTH, ROW_TOP, ROW_BOTTOM),
        }
    }
    pub(crate) fn row_rect_for(&self, index: usize, rows: usize) -> Rect {
        let (rx, rw, top, bottom) = self.row_bounds();
        if rows == 0 {
            return Rect::default();
        }
        let (start, end) = self.visible_row_window(rows);
        if index < start || index >= end {
            return Rect::default();
        }
        let visible_rows = (end - start).max(1);
        let slot = index - start;
        let h = ((bottom - top) / visible_rows as f32).min(0.078);
        let gap = (h * 0.16).min(0.010);
        Rect::new(rx, top + slot as f32 * h, rw, (h - gap).max(0.02))
    }
    fn stepper_split_for(&self, value: &str) -> f32 {
        let (rx, rw, _, _) = self.row_bounds();
        let right_px = (rx + rw) * 640.0;
        let value_width = ui::texture_width(value, 0.78);
        (right_px - 34.0 - value_width * 0.5) / 640.0
    }
    /// Returns `true` if a touch at normalized `(x, y)` lies in the open 3D Showroom viewport
    /// where dragging orbits the camera around the car without interfering with menu controls.
    pub fn is_showroom_orbit_zone(&self, x: f64, y: f64) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let yf = y as f32;
        match self.page {
            Page::Main => (0.105..=0.745).contains(&yf),
            Page::Garage | Page::Dealership | Page::NewDealership | Page::CareerGarage => {
                (0.105..=0.695).contains(&yf)
            }
            _ => false,
        }
    }

    fn main_quick_drive_rect() -> Rect {
        Rect::new(0.755, 0.022, 0.220, 0.068)
    }

    fn main_section_rect(&self, index: usize) -> Rect {
        let count = MainSection::ALL.len();
        let total_w = 0.956;
        let gap = 0.010;
        let card_w = (total_w - gap * (count - 1) as f32) / count as f32;
        let x = 0.022 + index as f32 * (card_w + gap);
        let active = index == self.main_section;
        if active {
            Rect::new(x, 0.766, card_w, 0.194)
        } else {
            Rect::new(x, 0.788, card_w, 0.172)
        }
    }

    pub fn settings_card_rect(index: usize) -> Rect {
        match index {
            0..=2 => {
                let total_w = 0.930;
                let gap = 0.024;
                let card_w = (total_w - gap * 2.0) / 3.0;
                let x = 0.035 + index as f32 * (card_w + gap);
                Rect::new(x, 0.190, card_w, 0.325)
            }
            _ => {
                let col = index.saturating_sub(3).min(1);
                let total_w = 0.930;
                let gap = 0.026;
                let card_w = (total_w - gap) / 2.0;
                let x = 0.035 + col as f32 * (card_w + gap);
                Rect::new(x, 0.545, card_w, 0.325)
            }
        }
    }

    fn subpage_back_rect() -> Rect {
        Rect::new(0.555, 0.021, 0.185, 0.068)
    }

    fn subpage_home_rect() -> Rect {
        Rect::new(0.752, 0.021, 0.193, 0.068)
    }

    fn scroll_up_rect() -> Rect {
        Rect::new(0.680, 0.130, 0.125, 0.060)
    }

    fn scroll_down_rect() -> Rect {
        Rect::new(0.815, 0.130, 0.130, 0.060)
    }

    fn garage_back_rect() -> Rect {
        Rect::new(0.025, 0.022, 0.160, 0.068)
    }

    fn garage_filter_rect() -> Rect {
        Rect::new(0.205, 0.022, 0.445, 0.068)
    }

    fn garage_details_rect() -> Rect {
        Rect::new(0.670, 0.022, 0.305, 0.068)
    }

    fn garage_prev_rect() -> Rect {
        Rect::new(0.025, 0.720, 0.075, 0.228)
    }

    fn garage_next_rect() -> Rect {
        Rect::new(0.900, 0.720, 0.075, 0.228)
    }

    fn garage_action_rect() -> Rect {
        Rect::new(0.560, 0.858, 0.310, 0.074)
    }

    /// Context-aware hit test supporting Title Screen, Horizontal Main Menu, 3D Garage,
    /// 5-Card Settings Hub, and all standard vertical row pages.
    pub fn press_with_ctx(&self, x: f64, y: f64, ctx: &Context, rows: &[Row]) -> Option<Command> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        match self.page {
            Page::Title => Some(Command::EnterMainMenu),
            Page::Main => {
                if Self::main_quick_drive_rect().contains(x, y) {
                    return Some(Command::StartRally);
                }
                for idx in 0..MainSection::ALL.len() {
                    if self.main_section_rect(idx).contains(x, y) {
                        return Some(Command::MainSectionActivate(idx));
                    }
                }
                None
            }
            Page::Settings => {
                if Self::subpage_back_rect().contains(x, y) {
                    return Some(Command::Back);
                }
                if Self::subpage_home_rect().contains(x, y) {
                    return Some(Command::ResetToMain);
                }
                for idx in 0..SettingsCard::ALL.len() {
                    if Self::settings_card_rect(idx).contains(x, y) {
                        return Some(Command::SettingsCardActivate(idx));
                    }
                }
                None
            }
            Page::Garage => {
                if Self::garage_back_rect().contains(x, y) {
                    return Some(Command::Back);
                }
                if Self::garage_filter_rect().contains(x, y) {
                    return Some(Command::GarageFilterCycle);
                }
                let indices = self.garage_indices(ctx);
                let active_idx = if indices.contains(&self.inspected_vehicle) {
                    self.inspected_vehicle
                } else {
                    indices.first().copied().unwrap_or(0)
                };
                if Self::garage_details_rect().contains(x, y)
                    && active_idx < ctx.content.vehicles.len()
                {
                    return Some(Command::OpenVehicle(active_idx));
                }
                if Self::garage_prev_rect().contains(x, y) {
                    return Some(Command::GaragePrevCar);
                }
                if Self::garage_next_rect().contains(x, y) {
                    return Some(Command::GarageNextCar);
                }
                if Self::garage_action_rect().contains(x, y) {
                    if let Some(entry) = ctx.content.vehicles.get(active_idx) {
                        let cap = entry.capability(ctx.ngp_experimental);
                        if cap == DriveCapability::Drivable {
                            if entry.descriptor.id == ctx.selection.vehicle.id {
                                return Some(Command::Open(Page::Drive));
                            }
                            return Some(Command::SelectVehicle(active_idx));
                        }
                        if !ctx.ngp_experimental
                            && entry.descriptor.is_ngp_experimental_car()
                            && matches!(
                                entry.classification,
                                rerbr_content::VehicleClassification::RsfNgpVerified
                                    | rerbr_content::VehicleClassification::RsfNgpPartial
                            )
                        {
                            return Some(Command::ToggleExperimentalMode);
                        }
                    }
                }
                None
            }
            Page::Dealership | Page::NewDealership => {
                if Self::garage_back_rect().contains(x, y) {
                    return Some(Command::Back);
                }
                if Self::garage_filter_rect().contains(x, y) {
                    return Some(Command::DealershipFilterCycle);
                }
                if Self::garage_prev_rect().contains(x, y) {
                    return Some(Command::DealershipPrevCar);
                }
                if Self::garage_next_rect().contains(x, y) {
                    return Some(Command::DealershipNextCar);
                }
                if Self::garage_action_rect().contains(x, y) {
                    let indices = self.dealership_indices(ctx);
                    let active_idx = if indices.contains(&self.dealership_vehicle) {
                        self.dealership_vehicle
                    } else {
                        indices.first().copied().unwrap_or(0)
                    };
                    if let Some(entry) = ctx.content.vehicles.get(active_idx) {
                        let profile = ctx.career.and_then(|c| c.profile.as_ref());
                        if profile.is_none() {
                            return Some(Command::Open(Page::CareerCreate));
                        }
                        if ctx.pending_buy_vehicle_id == Some(entry.descriptor.id.as_str()) {
                            return Some(Command::DealershipConfirmBuy);
                        }
                        let cap = entry.capability(ctx.ngp_experimental);
                        let owned = profile.is_some_and(|p| p.owns_vehicle(&entry.descriptor.id));
                        if cap == DriveCapability::Drivable && !owned {
                            return Some(Command::DealershipRequestBuy);
                        }
                    }
                }
                if Self::garage_details_rect().contains(x, y) {
                    if ctx.pending_buy_vehicle_id.is_some() {
                        return Some(Command::DealershipCancelBuy);
                    }
                    return Some(Command::Open(Page::CareerHub));
                }
                None
            }
            Page::CareerGarage => {
                if Self::garage_back_rect().contains(x, y) {
                    return Some(Command::Back);
                }
                let indices = self.career_garage_indices(ctx);
                let cursor = self
                    .career_garage_cursor
                    .min(indices.len().saturating_sub(1));
                let active_entry = indices
                    .get(cursor)
                    .and_then(|&idx| ctx.content.vehicles.get(idx));
                let pending_sell_this = active_entry
                    .is_some_and(|e| ctx.pending_sell_vehicle_id == Some(e.descriptor.id.as_str()));
                if Self::garage_details_rect().contains(x, y) {
                    if pending_sell_this {
                        return Some(Command::CareerGarageCancelSell);
                    }
                    if active_entry.is_some() {
                        return Some(Command::CareerGarageRequestSell);
                    }
                    return Some(Command::Open(Page::Dealership));
                }
                if Self::garage_prev_rect().contains(x, y) {
                    return Some(Command::CareerGaragePrevCar);
                }
                if Self::garage_next_rect().contains(x, y) {
                    return Some(Command::CareerGarageNextCar);
                }
                if Self::garage_action_rect().contains(x, y) {
                    if pending_sell_this {
                        return Some(Command::CareerGarageConfirmSell);
                    }
                    if indices.is_empty() {
                        return Some(Command::Open(Page::Dealership));
                    }
                    return Some(Command::CareerGarageSelectActive);
                }
                None
            }
            _ => self.press(x, y, rows),
        }
    }

    /// Resolves a touch to a command using the same rectangles used for drawing.
    pub fn press(&self, x: f64, y: f64, rows: &[Row]) -> Option<Command> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        if self.page == Page::Settings {
            if Self::subpage_back_rect().contains(x, y) {
                return Some(Command::Back);
            }
            if Self::subpage_home_rect().contains(x, y) {
                return Some(Command::ResetToMain);
            }
            for idx in 0..SettingsCard::ALL.len() {
                if Self::settings_card_rect(idx).contains(x, y) {
                    return Some(Command::SettingsCardActivate(idx));
                }
            }
            return None;
        }
        if self.page.is_settings_subpage() {
            if Self::subpage_back_rect().contains(x, y) {
                return Some(Command::Back);
            }
            if Self::subpage_home_rect().contains(x, y) {
                return Some(Command::ResetToMain);
            }
        }
        if !matches!(self.page, Page::CameraEditor | Page::ControlsEditor)
            && rows.len() > MAX_VISIBLE_ROWS
        {
            if Self::scroll_up_rect().contains(x, y) {
                return Some(Command::ScrollRowsUp);
            }
            if Self::scroll_down_rect().contains(x, y) {
                return Some(Command::ScrollRowsDown);
            }
        }
        if rows.is_empty() {
            return None;
        }
        let (xf, yf) = (x as f32, y as f32);
        let (vis_start, vis_end) = self.visible_row_window(rows.len());
        let index = (vis_start..vis_end).find(|&i| {
            let r = self.row_rect_for(i, rows.len());
            r.w > 0.0 && xf >= r.x && xf <= r.x + r.w && yf >= r.y && yf <= r.y + r.h
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
        if self.page == Page::Title {
            self.draw_title(canvas, ctx);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
        if self.page == Page::Main {
            self.draw_horizontal_main(canvas, ctx, status);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
        if self.page == Page::Settings {
            self.draw_settings_hub(canvas, ctx, status);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
        if self.page == Page::Garage {
            self.draw_showroom_garage(canvas, ctx, status);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
        if matches!(self.page, Page::Dealership | Page::NewDealership) {
            self.draw_dealership_showroom(canvas, ctx, status);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
        if self.page == Page::CareerGarage {
            self.draw_career_garage_showroom(canvas, ctx, status);
            self.draw_transition_fade(canvas, ctx.transition_alpha);
            return;
        }
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
            canvas.text(0.072 * 640.0, 0.032 * height, self.title(), 1.35, TEXT);
            if self.page.is_settings_subpage() {
                ui::button(canvas, Self::subpage_back_rect(), "< AJUSTES", false, true);
                ui::button(
                    canvas,
                    Self::subpage_home_rect(),
                    "MENÚ PRINCIPAL",
                    false,
                    true,
                );
            } else {
                #[cfg(feature = "career-dev-mode")]
                {
                    let dev_badge = Rect::new(0.66, 0.032, 0.285, 0.052);
                    ui::panel(canvas, dev_badge, [60, 38, 10, 225], WARN);
                    ui::text_center(canvas, dev_badge, "RBR MOBILE — DEV BUILD", 0.56, WARN);
                }
            }
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
        } else if self.page == Page::SchoolBriefing {
            self.draw_school_briefing_panel(canvas, ctx);
        } else if self.page == Page::SchoolResult {
            self.draw_school_result_panel(canvas, ctx);
        } else if self.page == Page::CareerChampionship {
            self.draw_career_championship_panel(canvas, ctx);
        }

        let rows = self.rows(ctx);
        let (vis_start, vis_end) = self.visible_row_window(rows.len());
        let visible_count = (vis_end - vis_start).max(1);
        if !compact && rows.len() > MAX_VISIBLE_ROWS {
            ui::button(
                canvas,
                Self::scroll_up_rect(),
                "SUBIR ▲",
                false,
                vis_start > 0,
            );
            let down_lbl = format!("BAJAR ▼ ({}/{})", vis_end, rows.len());
            ui::button(
                canvas,
                Self::scroll_down_rect(),
                &down_lbl,
                false,
                vis_end < rows.len(),
            );
        }
        let label_scale = if visible_count > 7 { 0.74 } else { 0.82 };
        for (index, row) in rows
            .iter()
            .enumerate()
            .skip(vis_start)
            .take(vis_end - vis_start)
        {
            let rect = self.row_rect_for(index, rows.len());
            let (x, y, w, h) = ui::px(rect, canvas);
            let interactive = matches!(
                row.kind,
                RowKind::Action { enabled: true, .. } | RowKind::Stepper { .. }
            );
            let is_primary = matches!(
                row.kind,
                RowKind::Action {
                    command: Command::StartRally
                        | Command::SchoolConfirmStart
                        | Command::CareerCreateConfirm
                        | Command::CareerChampStart
                        | Command::CareerChampRunNextStage
                        | Command::OpenCareerAfterStage,
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
    fn draw_school_briefing_panel(&self, canvas: &mut Canvas, ctx: &Context) {
        let panel = Rect::new(0.055, 0.195, 0.89, 0.440);
        ui::panel(canvas, panel, PANEL_DEEP, LINE);
        let (px, py, pw, _) = ui::px(panel, canvas);
        let Some((idx, sch)) = ctx.active_school_lesson.zip(ctx.school) else {
            return;
        };
        let Some(entry) = sch.entries.get(idx as usize) else {
            return;
        };
        let mut lines: Vec<(String, [u8; 4])> = Vec::new();
        let objective = entry
            .instruction
            .clone()
            .unwrap_or_else(|| format!("SIN STRINGS.RBZ (CLAVE {})", entry.key));
        for part in objective.lines() {
            for w in ui::wrap(&format!("OBJETIVO: {part}"), 60) {
                lines.push((w, TEXT));
            }
        }
        if let Some(cones) = &entry.cones_guide {
            for (ci, part) in cones.lines().enumerate() {
                let prefix = if ci == 0 { "CONOS / ZONAS: " } else { "  " };
                for w in ui::wrap(&format!("{prefix}{part}"), 60) {
                    lines.push((w, WARN));
                }
            }
        }
        let timing_line =
            if entry.perfect_time > 0.0 && entry.pass_time > 0.0 && entry.time_limit > 0.0 {
                format!(
                    "TIEMPOS: PERFECTO <= {:.1}S · APROBADO <= {:.1}S · LÍMITE {:.1}S",
                    entry.perfect_time, entry.pass_time, entry.time_limit
                )
            } else if entry.untimed {
                "EVALUACIÓN: PRUEBA DE CONTROL (SIN LÍMITE ESTRICTO DE TIEMPO)".into()
            } else {
                "EVALUACIÓN: CONDUCCIÓN DE PRUEBA".into()
            };
        lines.push((timing_line, GOOD));
        if let Some(dist) = entry.target_distance {
            lines.push((
                format!("REFERENCIA / ZONA DE PARADA: {dist:.0}M POR PISTA"),
                DIM,
            ));
        }
        if entry.ride {
            lines.push((
                "MODO DEMO: REPRODUCCIÓN AUTOMÁTICA .ARP NO ACTIVA; CONDUCE EL TRAMO".into(),
                WARN,
            ));
        }
        lines.push((sch.audio.status.label(), DIM));
        let max_lines = 9;
        for (i, (line, color)) in lines.into_iter().take(max_lines).enumerate() {
            ui::text_clipped(
                canvas,
                px + 12.0,
                py + 10.0 + i as f32 * 16.0,
                pw - 24.0,
                &line,
                0.70,
                color,
            );
        }
    }
    fn draw_school_result_panel(&self, canvas: &mut Canvas, ctx: &Context) {
        let panel = Rect::new(0.055, 0.195, 0.89, 0.440);
        let border = ctx
            .school_result
            .map_or(LINE, |r| if r.verdict > 0 { GOOD } else { BAD });
        ui::panel(canvas, panel, PANEL_DEEP, border);
        let (px, py, pw, _) = ui::px(panel, canvas);
        let Some(res) = ctx.school_result else {
            return;
        };
        let mut lines: Vec<(String, [u8; 4])> = Vec::new();
        let verdict_color = if res.verdict > 0 { GOOD } else { BAD };
        lines.push((
            format!(
                "{} · {}/4 · TIEMPO: {} · MEJOR: {} -> {}",
                super::school::medal_label(res.verdict),
                res.verdict,
                format_stage_time(f64::from(res.elapsed_seconds)),
                res.previous_best,
                res.new_best,
            ),
            verdict_color,
        ));
        if res.perfect_time > 0.0 && res.pass_time > 0.0 && res.time_limit > 0.0 {
            lines.push((
                format!(
                    "OBJETIVOS: PERFECTO <= {:.1}S · APROBADO <= {:.1}S · LÍMITE {:.1}S",
                    res.perfect_time, res.pass_time, res.time_limit
                ),
                DIM,
            ));
        }
        if res.verdict == 0 {
            for reason in &res.failure_reasons {
                for w in ui::wrap(&format!("MOTIVO: {reason}"), 60) {
                    lines.push((w, BAD));
                }
            }
            if let Some(advice) = &res.advice {
                for w in ui::wrap(&format!("INSTRUCTOR: {advice}"), 60) {
                    lines.push((w, WARN));
                }
            }
        } else {
            lines.push((
                format!("PROGRESO: {}/17 LECCIONES", res.unlocked_count),
                TEXT,
            ));
        }
        for (i, (line, color)) in lines.into_iter().take(9).enumerate() {
            ui::text_clipped(
                canvas,
                px + 12.0,
                py + 10.0 + i as f32 * 16.0,
                pw - 24.0,
                &line,
                0.70,
                color,
            );
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

    fn draw_transition_fade(&self, canvas: &mut Canvas, alpha: f32) {
        if alpha > 0.005 {
            let a = (alpha.clamp(0.0, 1.0) * 230.0).round() as u8;
            ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 1.0), [8, 10, 14, a]);
        }
    }

    /// Transparent Title Screen rendered over the cinematic 3D Showroom car.
    fn draw_title(&self, canvas: &mut Canvas, ctx: &Context) {
        let h = canvas.height();
        // Subtle top-left brand badge.
        let badge = Rect::new(0.025, 0.028, 0.23, 0.056);
        ui::fill(canvas, badge, [10, 13, 19, 175]);
        ui::fill(canvas, Rect::new(badge.x, badge.y, 0.006, badge.h), ACCENT);
        canvas.text(
            (badge.x + 0.016) * 640.0,
            (badge.y + 0.014) * h,
            "RBR MOBILE",
            0.92,
            TEXT,
        );

        // Sleek bottom gradient strip so title & CTA stay razor-sharp without hiding the car.
        ui::fill(canvas, Rect::new(0.0, 0.72, 1.0, 0.28), [8, 10, 15, 165]);
        ui::fill(canvas, Rect::new(0.0, 0.72, 1.0, 0.004), ACCENT);

        // Main title lockup.
        ui::text_center(
            canvas,
            Rect::new(0.05, 0.742, 0.90, 0.078),
            "RICHARD BURNS RALLY",
            1.65,
            TEXT,
        );
        ui::text_center(
            canvas,
            Rect::new(0.18, 0.816, 0.64, 0.042),
            "iOS / iPadOS · SHOWROOM 3D",
            0.70,
            DIM,
        );

        // Pulsing "TOCA PARA COMENZAR" prompt pill.
        let pulse = 0.62 + 0.38 * ctx.title_pulse.clamp(0.0, 1.0);
        let prompt_rect = Rect::new(0.30, 0.875, 0.40, 0.072);
        ui::panel(
            canvas,
            prompt_rect,
            [18, 22, 32, 220],
            ui::with_alpha(ACCENT, pulse),
        );
        ui::text_center(
            canvas,
            prompt_rect,
            "TOCA PARA COMENZAR",
            0.92,
            ui::with_alpha(TEXT, pulse),
        );

        // Subtle showcase car caption bottom-left (honest loading/fallback status).
        let car_caption = if let Some(name) = ctx.showroom_vehicle_name {
            format!("MODELO: {name}")
        } else if ctx.showroom_loading {
            "CARGANDO MODELO 3D...".into()
        } else if ctx.showroom_error.is_some() {
            "SIN MODELOS 3D EN DOCUMENTS/RBR/CARS".into()
        } else {
            "LISTO PARA INICIAR".into()
        };
        let cap_rect = Rect::new(0.025, 0.915, 0.26, 0.046);
        let (cx, cy, cw, _) = ui::px(cap_rect, canvas);
        ui::text_clipped(canvas, cx, cy, cw, &car_caption, 0.56, DIM);
    }

    /// Transparent Horizontal Main Menu rendered over the interactive 3D Showroom car
    /// (showing the last-driven car by default).
    fn draw_horizontal_main(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        let h = canvas.height();
        let vehicle = &ctx.selection.vehicle;
        let stage = &ctx.selection.stage;

        // Top translucent header strip.
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.106), [10, 13, 19, 195]);
        ui::rule(canvas, 0.0, 0.106, 1.0, LINE);

        // Top-left brand mark.
        ui::fill(canvas, Rect::new(0.025, 0.026, 0.006, 0.056), ACCENT);
        canvas.text(0.040 * 640.0, 0.024 * h, "RBR MOBILE", 1.15, TEXT);
        #[cfg(feature = "career-dev-mode")]
        canvas.text(
            0.040 * 640.0,
            0.068 * h,
            "RBR MOBILE — DEV BUILD",
            0.46,
            WARN,
        );
        #[cfg(not(feature = "career-dev-mode"))]
        canvas.text(0.040 * 640.0, 0.068 * h, "SHOWROOM", 0.56, DIM);

        // Top-center active car & stage summary pill.
        let summary_rect = Rect::new(0.245, 0.022, 0.495, 0.068);
        ui::panel(canvas, summary_rect, [16, 20, 28, 210], LINE);
        let (sx, sy, sw, _) = ui::px(summary_rect, canvas);
        let car_label = format!("COCHE: {}", vehicle.name);
        let stage_label = format!("TRAMO: {} ({})", stage.name, stage.surface());
        ui::text_clipped(
            canvas,
            sx + 10.0,
            sy + 4.0,
            sw - 20.0,
            &car_label,
            0.68,
            TEXT,
        );
        ui::text_clipped(
            canvas,
            sx + 10.0,
            sy + 17.5,
            sw - 20.0,
            &stage_label,
            0.58,
            INFO,
        );

        // Top-right 1-tap quick drive CTA so existing driving & stage testing are always 1 tap away.
        let drive_rect = Self::main_quick_drive_rect();
        ui::panel(canvas, drive_rect, [156, 30, 30, 238], [240, 110, 110, 255]);
        ui::text_center(canvas, drive_rect, "INICIAR TRAMO", 0.78, TEXT);

        // Optional non-blocking notice (coming soon / status / showroom loading).
        if let Some(notice) = ctx.coming_soon_notice {
            let nrect = Rect::new(0.18, 0.120, 0.64, 0.052);
            ui::panel(canvas, nrect, [16, 20, 28, 224], WARN);
            let (nx, ny, nw, nh) = ui::px(nrect, canvas);
            ui::text_clipped(
                canvas,
                nx + 10.0,
                ny + (nh - 10.0).max(0.0) * 0.5,
                nw - 20.0,
                notice,
                0.64,
                WARN,
            );
        } else if !status.is_empty()
            && status != "READY - TAP A ROW"
            && !status.starts_with("LOADING PRIVATE")
        {
            let nrect = Rect::new(0.20, 0.120, 0.60, 0.048);
            ui::panel(canvas, nrect, [16, 20, 28, 210], INFO);
            let (nx, ny, nw, nh) = ui::px(nrect, canvas);
            ui::text_clipped(
                canvas,
                nx + 10.0,
                ny + (nh - 10.0).max(0.0) * 0.5,
                nw - 20.0,
                status,
                0.62,
                TEXT,
            );
        }

        // Subtle interactive orbit hint just above the dock (never covers the car).
        let hint_text = if ctx.showroom_loading {
            "CARGANDO COCHE 3D..."
        } else if ctx.showroom_vehicle_name.is_some() {
            "ARRASTRA PARA GIRAR EL COCHE 3D"
        } else {
            "SIN MODELO 3D INSTALADO"
        };
        let hint_rect = Rect::new(0.68, 0.718, 0.295, 0.036);
        let (hx, hy, hw, _) = ui::px(hint_rect, canvas);
        ui::text_clipped(canvas, hx, hy, hw, hint_text, 0.52, DIM);

        // Bottom horizontal dock background.
        ui::fill(canvas, Rect::new(0.0, 0.758, 1.0, 0.242), [9, 12, 18, 205]);
        ui::rule(canvas, 0.0, 0.758, 1.0, LINE);

        // All 7 horizontal sections side-by-side.
        for (idx, sec) in MainSection::ALL.iter().enumerate() {
            let rect = self.main_section_rect(idx);
            let active = idx == self.main_section;
            let soon = sec.is_coming_soon();
            let fill = if active {
                [28, 36, 50, 240]
            } else if soon {
                [14, 17, 24, 215]
            } else {
                PANEL_ACTIVE
            };
            let border = if active {
                ACCENT
            } else if soon {
                [38, 44, 56, 220]
            } else {
                LINE
            };
            ui::panel(canvas, rect, fill, border);
            if active {
                ui::fill(canvas, Rect::new(rect.x, rect.y, rect.w, 0.010), ACCENT);
            }
            let (rx, ry, rw, rh) = ui::px(rect, canvas);
            let title = sec.title();
            let base_scale = if active { 0.68 } else { 0.60 };
            let needed = ui::texture_width(title, base_scale);
            let fit = (rw - 8.0).max(1.0);
            let t_scale = if needed > fit {
                (base_scale * fit / needed).clamp(0.42, base_scale)
            } else {
                base_scale
            };
            let title_col = if soon && !active { DIM } else { TEXT };
            let tw = ui::texture_width(title, t_scale);
            canvas.text(
                rx + (rw - tw).max(0.0) * 0.5,
                ry + rh * 0.24,
                title,
                t_scale,
                title_col,
            );

            let sub = if *sec == MainSection::Garage {
                format!("{} COCHES", ctx.content.vehicles.len())
            } else if *sec == MainSection::Career {
                ctx.career
                    .and_then(|c| c.profile.as_ref())
                    .map(|p| super::career::format_usd(p.cash_usd))
                    .unwrap_or_else(|| sec.subtitle().to_string())
            } else {
                sec.subtitle().to_string()
            };
            let sub_col = if soon {
                WARN
            } else if active {
                GOOD
            } else {
                INFO
            };
            let s_base = 0.46;
            let s_needed = ui::texture_width(&sub, s_base);
            let s_scale = if s_needed > fit {
                (s_base * fit / s_needed).clamp(0.36, s_base)
            } else {
                s_base
            };
            let sw = ui::texture_width(&sub, s_scale);
            canvas.text(
                rx + (rw - sw).max(0.0) * 0.5,
                ry + rh * 0.64,
                &sub,
                s_scale,
                sub_col,
            );
        }
    }

    /// Full-screen 5-Card Settings Hub (`AJUSTES`) styled consistently with the horizontal Main Menu:
    /// Top row: `CONTROLES`, `SONIDO`, `HUD Y CÁMARA`.
    /// Bottom row: `CRÉDITOS`, `CONTENIDO`.
    fn draw_settings_hub(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        let height = canvas.height();
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 1.0), ui::BACKDROP);
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.11), [12, 15, 21, 255]);
        ui::fill(canvas, Rect::new(0.035, 0.032, 0.006, 0.048), ACCENT);
        canvas.text(0.052 * 640.0, 0.030 * height, "AJUSTES", 1.45, TEXT);
        ui::button(canvas, Self::subpage_back_rect(), "< VOLVER", false, true);
        ui::button(
            canvas,
            Self::subpage_home_rect(),
            "MENÚ PRINCIPAL",
            false,
            true,
        );
        ui::rule(canvas, 0.035, 0.110, 0.930, LINE);

        canvas.text(
            0.035 * 640.0,
            0.130 * height,
            "TOCA UNA TARJETA PARA ABRIR SU PANTALLA DE CONFIGURACIÓN · GUARDADO AUTOMÁTICO",
            0.70,
            DIM,
        );

        let active_card = if self.gamepad_focus_active {
            self.focused_row.min(SettingsCard::ALL.len() - 1)
        } else {
            self.settings_card.min(SettingsCard::ALL.len() - 1)
        };

        for (idx, card) in SettingsCard::ALL.iter().enumerate() {
            let rect = Self::settings_card_rect(idx);
            let active = idx == active_card;
            let fill = if active {
                [26, 34, 48, 244]
            } else {
                PANEL_ACTIVE
            };
            let border = if active { ACCENT } else { LINE };
            ui::panel(canvas, rect, fill, border);
            ui::fill(
                canvas,
                Rect::new(rect.x, rect.y, rect.w, 0.012),
                if active { ACCENT } else { INFO },
            );

            let (rx, ry, rw, rh) = ui::px(rect, canvas);
            ui::text_clipped(
                canvas,
                rx + 14.0,
                ry + 12.0,
                rw - 28.0,
                card.title(),
                0.98,
                TEXT,
            );
            ui::text_clipped(
                canvas,
                rx + 14.0,
                ry + 31.0,
                rw - 28.0,
                card.subtitle(),
                0.56,
                INFO,
            );
            canvas.rect(rx + 14.0, ry + 48.0, (rw - 28.0).max(1.0), 1.0, LINE);

            let summaries: [String; 3] = match card {
                SettingsCard::Controls => [
                    format!(
                        "GIROSCOPIO: SENS {:.2} · ZONA MUERTA {:.2}",
                        ctx.sensitivity, ctx.dead_zone
                    ),
                    format!(
                        "PEDALES: {} · EMBRAGUE AUTO: {}",
                        if ctx.invert_pedals {
                            "INVERTIDOS"
                        } else {
                            "NORMAL"
                        },
                        on_off(ctx.clutch_help)
                    ),
                    format!(
                        "MANDO: {}",
                        if ctx.gamepad_connected {
                            ctx.gamepad_name.unwrap_or("CONECTADO")
                        } else {
                            "TÁCTIL / GIROSCOPIO"
                        }
                    ),
                ],
                SettingsCard::Sound => [
                    format!("MOTOR Y EFECTOS: {:.0}%", ctx.volume * 100.0),
                    format!("VOZ COPILOTO: {:.0}%", ctx.codriver_volume * 100.0),
                    format!("MÚSICA DE MENÚ: {:.0}%", ctx.music_volume * 100.0),
                ],
                SettingsCard::HudCamera => [
                    format!("CÁMARA ACTIVA: {}", ctx.camera),
                    format!(
                        "HUD MÍN: {} · FPS: {} · TELEMETRÍA: {}",
                        on_off(ctx.minimal_hud),
                        on_off(ctx.fps),
                        on_off(ctx.diagnostics)
                    ),
                    format!(
                        "NOTAS VISUALES COPILOTO: {}",
                        on_off(ctx.pacenote_hud.enabled)
                    ),
                ],
                SettingsCard::Credits => [
                    format!(
                        "VERSIÓN {} · BUILD {}",
                        env!("CARGO_PKG_VERSION"),
                        option_env!("GITHUB_RUN_NUMBER").unwrap_or("LOCAL")
                    ),
                    "MOTOR BASE: ReRBR POR JODUSNODUS".into(),
                    "GPL-3.0-OR-LATER · RBR / RALLYSIMFANS NGP".into(),
                ],
                SettingsCard::Content => [
                    format!(
                        "INSTALADOS: {} COCHES · {} TRAMOS",
                        ctx.content.vehicles.len(),
                        ctx.content.stages.len()
                    ),
                    format!("MODO NGP EXPERIMENTAL: {}", on_off(ctx.ngp_experimental)),
                    "AUDITORÍA DE ARCHIVOS Y DIAGNÓSTICO 10 ETAPAS".into(),
                ],
            };

            for (li, line) in summaries.iter().enumerate() {
                ui::text_clipped(
                    canvas,
                    rx + 14.0,
                    ry + 58.0 + li as f32 * 16.5,
                    rw - 28.0,
                    line,
                    0.56,
                    DIM,
                );
            }

            let cta = "ABRIR APARTADO >";
            let cta_scale = 0.58;
            let cta_w = ui::texture_width(cta, cta_scale);
            canvas.text(
                rx + (rw - cta_w - 14.0).max(14.0),
                ry + (rh - 22.0).max(60.0),
                cta,
                cta_scale,
                if active { GOOD } else { ACCENT },
            );
        }

        let footer = ui::wrap(status, 58);
        ui::rule(canvas, 0.035, 0.900, 0.930, LINE);
        for (index, line) in footer.iter().take(2).enumerate() {
            canvas.text(
                0.035 * 640.0,
                (0.914 + index as f32 * 0.026) * height,
                line,
                0.70,
                GOOD,
            );
        }
    }

    /// Transparent 3D Showroom Garage letting the user inspect ALL installed cars (drivable + visual-only).
    fn draw_showroom_garage(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        // Top translucent bar.
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.106), [10, 13, 19, 198]);
        ui::rule(canvas, 0.0, 0.106, 1.0, LINE);

        let indices = self.garage_indices(ctx);
        let pos = indices
            .iter()
            .position(|&i| i == self.inspected_vehicle)
            .unwrap_or(0);
        let active_idx = indices.get(pos).copied().unwrap_or(0);
        let entry = ctx.content.vehicles.get(active_idx);

        // Top buttons: BACK, FILTER, DETAILS.
        ui::button(canvas, Self::garage_back_rect(), "VOLVER", false, true);
        let drivable_count = ctx
            .content
            .vehicles
            .iter()
            .filter(|v| v.capability(ctx.ngp_experimental) == DriveCapability::Drivable)
            .count();
        let visual_count = ctx
            .content
            .vehicles
            .iter()
            .filter(|v| v.capability(ctx.ngp_experimental) == DriveCapability::VisualOnly)
            .count();
        let filter_label = format!(
            "FILTRO: {} ({}) · COND {} / VIS {}",
            self.garage_filter.label(),
            indices.len(),
            drivable_count,
            visual_count
        );
        ui::button(
            canvas,
            Self::garage_filter_rect(),
            &filter_label,
            false,
            true,
        );
        ui::button(
            canvas,
            Self::garage_details_rect(),
            "FICHA Y DIAGNÓSTICO",
            false,
            entry.is_some(),
        );

        // Bottom inspector dock.
        ui::fill(canvas, Rect::new(0.0, 0.705, 1.0, 0.295), [9, 12, 18, 210]);
        ui::rule(canvas, 0.0, 0.705, 1.0, LINE);

        let can_nav = indices.len() > 1;
        ui::button(canvas, Self::garage_prev_rect(), "<", false, can_nav);
        ui::button(canvas, Self::garage_next_rect(), ">", false, can_nav);

        let card = Rect::new(0.115, 0.720, 0.770, 0.228);
        ui::panel(canvas, card, [16, 20, 28, 232], LINE);
        let (cx, cy, cw, _) = ui::px(card, canvas);

        if let Some(entry) = entry {
            let cap = entry.capability(ctx.ngp_experimental);
            let cap_color = match cap {
                DriveCapability::Drivable => GOOD,
                DriveCapability::VisualOnly => WARN,
                DriveCapability::Unavailable => BAD,
            };
            let title = format!(
                "{}  ({}/{})",
                entry.display_name_with_year(),
                pos + 1,
                indices.len().max(1)
            );
            ui::text_clipped(canvas, cx + 12.0, cy + 8.0, cw * 0.62, &title, 0.96, TEXT);

            let badge_rect = Rect::new(0.60, 0.732, 0.270, 0.044);
            ui::panel(canvas, badge_rect, PANEL_DEEP, cap_color);
            ui::text_center(canvas, badge_rect, cap.label(), 0.56, cap_color);

            let cat = entry.descriptor.category.as_deref().unwrap_or("RBR / RSF");
            let meta = if let Some(pack) = entry.descriptor.rsf_pack.as_deref() {
                format!(
                    "{} · {} · PACK {} · {}",
                    entry.classification.badge(),
                    cat,
                    pack,
                    entry.descriptor.model
                )
            } else {
                format!(
                    "{} · {} · {}",
                    entry.classification.badge(),
                    cat,
                    entry.descriptor.model
                )
            };
            ui::text_clipped(canvas, cx + 12.0, cy + 28.0, cw - 24.0, &meta, 0.58, DIM);

            let state_line = if ctx.showroom_loading {
                "CARGANDO MODELO 3D BAJO DEMANDA...".to_string()
            } else if let Some(err) = ctx.showroom_error {
                format!("VISUAL 3D NO DISPONIBLE: {err}")
            } else if cap == DriveCapability::VisualOnly {
                "SOLO VISUALIZACIÓN 3D · ARRASTRA PARA GIRAR (SIN FÍSICA INTEGRADA)".to_string()
            } else if !status.is_empty() && status != "READY - TAP A ROW" {
                status.to_string()
            } else {
                "ARRASTRA ARRIBA PARA GIRAR LA CÁMARA ALREDEDOR DEL COCHE".to_string()
            };
            let state_col = if ctx.showroom_error.is_some() {
                BAD
            } else if cap == DriveCapability::VisualOnly {
                WARN
            } else {
                INFO
            };
            ui::text_clipped(
                canvas,
                cx + 12.0,
                cy + 46.0,
                cw * 0.54,
                &state_line,
                0.56,
                state_col,
            );

            // Bottom-right contextual action button.
            let action_rect = Self::garage_action_rect();
            if cap == DriveCapability::Drivable {
                if entry.descriptor.id == ctx.selection.vehicle.id {
                    ui::panel(canvas, action_rect, [22, 68, 48, 235], GOOD);
                    ui::text_center(canvas, action_rect, "ACTIVO · IR A TRAMOS", 0.68, TEXT);
                } else {
                    ui::panel(
                        canvas,
                        action_rect,
                        [156, 30, 30, 238],
                        [240, 110, 110, 255],
                    );
                    ui::text_center(canvas, action_rect, "SELECCIONAR PARA CONDUCIR", 0.64, TEXT);
                }
            } else if !ctx.ngp_experimental
                && entry.descriptor.is_ngp_experimental_car()
                && matches!(
                    entry.classification,
                    rerbr_content::VehicleClassification::RsfNgpVerified
                        | rerbr_content::VehicleClassification::RsfNgpPartial
                )
            {
                ui::panel(canvas, action_rect, [95, 62, 16, 235], WARN);
                ui::text_center(canvas, action_rect, "ACTIVAR MODO NGP", 0.66, TEXT);
            } else {
                ui::panel(canvas, action_rect, PANEL_DEEP, LINE);
                ui::text_center(canvas, action_rect, "SOLO VISUALIZACIÓN", 0.64, DIM);
            }
        } else {
            ui::text_center(
                canvas,
                card,
                "NO HAY COCHES DETECTADOS EN DOCUMENTS/RBR/CARS",
                0.82,
                WARN,
            );
        }
    }

    /// Transparent 3D Showroom Dealership for both `2ª MANO (USADOS)` and `SALÓN AUTOMOTOR (0 KM)`:
    /// Shows vehicle name & year, real category, `0 KM` vs `USADO` badge (with discount when applicable),
    /// USD price for the active dealership, driving capability, ownership, balance, and 2-step confirmation.
    fn draw_dealership_showroom(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        let kind = self.active_dealership_kind();
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.106), [10, 13, 19, 200]);
        ui::rule(canvas, 0.0, 0.106, 1.0, LINE);

        let indices = self.dealership_indices_for(ctx, kind);
        let pos = indices
            .iter()
            .position(|&i| i == self.dealership_vehicle)
            .unwrap_or(0);
        let active_idx = indices.get(pos).copied().unwrap_or(0);
        let entry = indices
            .get(pos)
            .and_then(|_| ctx.content.vehicles.get(active_idx));
        let profile = ctx.career.and_then(|c| c.profile.as_ref());

        ui::button(canvas, Self::garage_back_rect(), "VOLVER", false, true);
        let filter_label = format!(
            "{}: {} ({})",
            kind.short_name(),
            self.dealership_filter.label(),
            indices.len()
        );
        ui::button(
            canvas,
            Self::garage_filter_rect(),
            &filter_label,
            false,
            true,
        );

        let right_label = if ctx.pending_buy_vehicle_id.is_some() {
            "CANCELAR COMPRA".to_string()
        } else if let Some(p) = profile {
            format!("SALDO: {}", super::career::format_usd(p.cash_usd))
        } else {
            "SIN PERFIL CARRERA".to_string()
        };
        ui::button(
            canvas,
            Self::garage_details_rect(),
            &right_label,
            ctx.pending_buy_vehicle_id.is_some(),
            true,
        );

        // Bottom dock.
        ui::fill(canvas, Rect::new(0.0, 0.705, 1.0, 0.295), [9, 12, 18, 212]);
        ui::rule(canvas, 0.0, 0.705, 1.0, LINE);

        let can_nav = indices.len() > 1;
        ui::button(canvas, Self::garage_prev_rect(), "<", false, can_nav);
        ui::button(canvas, Self::garage_next_rect(), ">", false, can_nav);

        let card = Rect::new(0.115, 0.720, 0.770, 0.228);
        ui::panel(canvas, card, [16, 20, 28, 234], LINE);
        let (cx, cy, cw, _) = ui::px(card, canvas);

        if let Some(entry) = entry {
            let cap = entry.capability(ctx.ngp_experimental);
            let spec = super::career::vehicle_market_spec(entry);
            let active_price = spec.price_for(kind).unwrap_or(0);
            let owned = profile.is_some_and(|p| p.owns_vehicle(&entry.descriptor.id));
            let affordable = profile.is_some_and(|p| p.can_afford(active_price));
            let pending_this = ctx.pending_buy_vehicle_id == Some(entry.descriptor.id.as_str());

            let title = format!(
                "{}  ({}/{})",
                entry.display_name_with_year(),
                pos + 1,
                indices.len().max(1)
            );
            ui::text_clipped(canvas, cx + 12.0, cy + 8.0, cw * 0.58, &title, 0.92, TEXT);

            let badge_rect = Rect::new(0.585, 0.732, 0.285, 0.044);
            let (badge_text, badge_col) = if owned {
                ("EN TU GARAJE".to_string(), GOOD)
            } else {
                (
                    format!(
                        "{} · {}",
                        spec.condition_badge(kind),
                        super::career::format_usd(active_price)
                    ),
                    if cap == DriveCapability::Drivable && affordable {
                        GOOD
                    } else if cap == DriveCapability::Drivable {
                        WARN
                    } else {
                        DIM
                    },
                )
            };
            ui::panel(canvas, badge_rect, PANEL_DEEP, badge_col);
            ui::text_center(canvas, badge_rect, &badge_text, 0.56, badge_col);

            let meta = format!(
                "{} · {} · {} · {}",
                spec.condition_badge(kind),
                spec.categoria,
                spec.elegibilidad_carrera.label(),
                cap.short_badge()
            );
            ui::text_clipped(canvas, cx + 12.0, cy + 28.0, cw - 24.0, &meta, 0.58, DIM);

            let avail =
                profile.map(|p| p.dealership_availability(entry, kind, ctx.ngp_experimental));
            let info_line = if pending_this {
                format!(
                    "CONFIRMA COMPRA {} POR {} (SALDO RESTANTE {})",
                    kind.condition_label(),
                    super::career::format_usd(active_price),
                    profile
                        .map(|p| super::career::format_usd(p.cash_usd.saturating_sub(active_price)))
                        .unwrap_or_else(|| "US$0".into())
                )
            } else if let Some(super::career::DealershipAvailability::Locked { reason }) = &avail {
                format!("BLOQUEADO: {reason}")
            } else if owned {
                "EN GARAJE · YA LO TIENES EN TU GARAJE DE CARRERA".to_string()
            } else if !status.is_empty() && status != "READY - TAP A ROW" {
                status.to_string()
            } else if kind == super::career::DealershipKind::UsedCars {
                if let Some(p) = profile {
                    let listing = p.used_car_listing(entry);
                    format!(
                        "INSP: {}% GRAL (MOT {}% CAJ {}% SUSP {}% NEUM {}%) · REP.EST {}",
                        listing.condition.overall_pct(),
                        listing.condition.engine_pct,
                        listing.condition.transmission_pct,
                        listing.condition.suspension_pct,
                        listing.condition.tyre_remaining_pct(),
                        super::career::format_usd(listing.estimated_repair_cost_usd),
                    )
                } else {
                    "2ª MANO · INSPECCIÓN MECÁNICA DISPONIBLE AL CREAR PERFIL".to_string()
                }
            } else {
                "0 KM · 100% ESTADO MECÁNICO · 0% DESGASTE · 0 DAÑOS".to_string()
            };
            let info_col = if pending_this
                || matches!(
                    avail,
                    Some(super::career::DealershipAvailability::Locked { .. })
                ) {
                WARN
            } else if owned
                || matches!(
                    avail,
                    Some(super::career::DealershipAvailability::Available)
                )
            {
                GOOD
            } else {
                INFO
            };
            ui::text_clipped(
                canvas,
                cx + 12.0,
                cy + 46.0,
                cw * 0.54,
                &info_line,
                0.52,
                info_col,
            );

            let action_rect = Self::garage_action_rect();
            if profile.is_none() {
                ui::panel(canvas, action_rect, [156, 30, 30, 238], ACCENT);
                ui::text_center(canvas, action_rect, "CREAR PERFIL (US$15,000)", 0.60, TEXT);
            } else if owned {
                ui::panel(canvas, action_rect, [22, 68, 48, 235], GOOD);
                ui::text_center(canvas, action_rect, "EN PROPIEDAD", 0.66, TEXT);
            } else if matches!(
                avail,
                Some(super::career::DealershipAvailability::Locked { .. })
            ) {
                ui::panel(canvas, action_rect, PANEL_DEEP, LINE);
                ui::text_center(canvas, action_rect, "BLOQUEADO", 0.62, DIM);
            } else if pending_this {
                ui::panel(canvas, action_rect, [180, 105, 16, 245], WARN);
                ui::text_center(canvas, action_rect, "CONFIRMAR COMPRA", 0.66, TEXT);
            } else if affordable {
                ui::panel(
                    canvas,
                    action_rect,
                    [156, 30, 30, 238],
                    [240, 110, 110, 255],
                );
                let buy_txt = format!(
                    "COMPRAR {} {}",
                    kind.condition_label(),
                    super::career::format_usd(active_price)
                );
                ui::text_center(canvas, action_rect, &buy_txt, 0.60, TEXT);
            } else {
                ui::panel(canvas, action_rect, PANEL_DEEP, BAD);
                ui::text_center(canvas, action_rect, "DINERO INSUFICIENTE", 0.58, BAD);
            }
        } else {
            ui::text_center(
                canvas,
                card,
                "NO HAY COCHES INSTALADOS PARA ESTE CONCESIONARIO",
                0.78,
                WARN,
            );
        }
    }

    /// Transparent 3D Showroom Personal Career Garage (ONLY owned vehicles).
    fn draw_career_garage_showroom(&self, canvas: &mut Canvas, ctx: &Context, status: &str) {
        ui::fill(canvas, Rect::new(0.0, 0.0, 1.0, 0.106), [10, 13, 19, 200]);
        ui::rule(canvas, 0.0, 0.106, 1.0, LINE);

        let indices = self.career_garage_indices(ctx);
        let profile = ctx.career.and_then(|c| c.profile.as_ref());
        let cursor = self
            .career_garage_cursor
            .min(indices.len().saturating_sub(1));
        let entry = indices
            .get(cursor)
            .and_then(|&idx| ctx.content.vehicles.get(idx));

        ui::button(canvas, Self::garage_back_rect(), "VOLVER", false, true);
        let header_label = match profile {
            Some(p) => format!(
                "GARAJE CARRERA: #{} {} ({} COCHES)",
                p.competition_number,
                p.pilot_name,
                indices.len()
            ),
            None => "GARAJE DE CARRERA".into(),
        };
        ui::button(
            canvas,
            Self::garage_filter_rect(),
            &header_label,
            false,
            false,
        );
        let pending_sell_this =
            entry.is_some_and(|e| ctx.pending_sell_vehicle_id == Some(e.descriptor.id.as_str()));
        let top_right_label = if pending_sell_this {
            "CANCELAR VENTA"
        } else if entry.is_some() {
            "VENDER COCHE (70%)"
        } else {
            "IR A 2ª MANO"
        };
        ui::button(
            canvas,
            Self::garage_details_rect(),
            top_right_label,
            pending_sell_this,
            true,
        );

        ui::fill(canvas, Rect::new(0.0, 0.705, 1.0, 0.295), [9, 12, 18, 212]);
        ui::rule(canvas, 0.0, 0.705, 1.0, LINE);

        let can_nav = indices.len() > 1;
        ui::button(canvas, Self::garage_prev_rect(), "<", false, can_nav);
        ui::button(canvas, Self::garage_next_rect(), ">", false, can_nav);

        let card = Rect::new(0.115, 0.720, 0.770, 0.228);
        ui::panel(canvas, card, [16, 20, 28, 234], LINE);
        let (cx, cy, cw, _) = ui::px(card, canvas);

        if let Some(entry) = entry {
            let spec = super::career::vehicle_market_spec(entry);
            let is_active = profile.is_some_and(|p| {
                p.active_career_vehicle_id.as_deref() == Some(entry.descriptor.id.as_str())
            });
            let locked = profile.is_some_and(|p| p.has_locked_championship());

            let title = format!(
                "{}  ({}/{})",
                entry.descriptor.name,
                cursor + 1,
                indices.len().max(1)
            );
            ui::text_clipped(canvas, cx + 12.0, cy + 8.0, cw * 0.62, &title, 0.92, TEXT);

            let badge_rect = Rect::new(0.60, 0.732, 0.270, 0.044);
            let (b_txt, b_col) = if is_active {
                ("ACTIVO CARRERA", GOOD)
            } else {
                ("EN PROPIEDAD", INFO)
            };
            ui::panel(canvas, badge_rect, PANEL_DEEP, b_col);
            ui::text_center(canvas, badge_rect, b_txt, 0.56, b_col);

            let cat = entry.descriptor.category.as_deref().unwrap_or("RBR / RSF");
            let resale_usd = profile.map_or(0, |p| p.resale_value_usd(entry));
            let meta = format!(
                "{} · {} · REVENTA 70%: {}",
                spec.tier.label(),
                cat,
                super::career::format_usd(resale_usd)
            );
            ui::text_clipped(canvas, cx + 12.0, cy + 28.0, cw - 24.0, &meta, 0.58, DIM);

            let info = if pending_sell_this {
                format!(
                    "CONFIRMA VENTA DE {} POR {} (70% DEL PRECIO PAGADO)",
                    entry.descriptor.name,
                    super::career::format_usd(resale_usd)
                )
            } else if locked {
                "COCHE BLOQUEADO MIENTRAS DURA LA COPA REGIONAL".to_string()
            } else if !status.is_empty() && status != "READY - TAP A ROW" {
                status.to_string()
            } else {
                "ARRASTRA ARRIBA PARA INSPECCIONAR TU COCHE EN 3D".to_string()
            };
            ui::text_clipped(
                canvas,
                cx + 12.0,
                cy + 46.0,
                cw * 0.54,
                &info,
                0.55,
                if pending_sell_this || locked {
                    WARN
                } else {
                    INFO
                },
            );

            let action_rect = Self::garage_action_rect();
            if pending_sell_this {
                ui::panel(canvas, action_rect, [180, 105, 16, 245], WARN);
                let sell_txt = format!(
                    "CONFIRMAR VENTA (+{})",
                    super::career::format_usd(resale_usd)
                );
                ui::text_center(canvas, action_rect, &sell_txt, 0.58, TEXT);
            } else if is_active {
                ui::panel(canvas, action_rect, [22, 68, 48, 235], GOOD);
                ui::text_center(canvas, action_rect, "COCHE ACTIVO", 0.66, TEXT);
            } else if locked {
                ui::panel(canvas, action_rect, PANEL_DEEP, WARN);
                ui::text_center(canvas, action_rect, "BLOQUEADO EN COPA", 0.60, WARN);
            } else {
                ui::panel(canvas, action_rect, [156, 30, 30, 238], ACCENT);
                ui::text_center(canvas, action_rect, "USAR EN CARRERA", 0.64, TEXT);
            }
        } else {
            ui::text_clipped(
                canvas,
                cx + 12.0,
                cy + 14.0,
                cw - 24.0,
                "AÚN NO TIENES COCHES EN PROPIEDAD",
                0.86,
                WARN,
            );
            ui::text_clipped(
                canvas,
                cx + 12.0,
                cy + 40.0,
                cw * 0.56,
                "COMPRA UN COCHE DE INICIACIÓN EN 2ª MANO CON TU SALDO USD",
                0.56,
                DIM,
            );
            let action_rect = Self::garage_action_rect();
            ui::panel(canvas, action_rect, [156, 30, 30, 238], ACCENT);
            ui::text_center(canvas, action_rect, "IR A 2ª MANO", 0.66, TEXT);
        }
    }

    /// Standings, calendar, and prize summary panel for `Page::CareerChampionship`.
    fn draw_career_championship_panel(&self, canvas: &mut Canvas, ctx: &Context) {
        let panel = Rect::new(0.055, 0.195, 0.89, 0.440);
        ui::panel(canvas, panel, PANEL_DEEP, LINE);
        let (px, py, pw, _) = ui::px(panel, canvas);

        let Some(p) = ctx.career.and_then(|c| c.profile.as_ref()) else {
            ui::text_center(
                canvas,
                panel,
                "CREA UN PERFIL DE CARRERA PRIMERO",
                0.80,
                WARN,
            );
            return;
        };

        let mut lines: Vec<(String, [u8; 4])> = Vec::new();
        if let Some(champ) = &p.active_championship {
            let status_hdr = if champ.finished {
                let pos = champ.final_position.unwrap_or(1);
                format!(
                    "FINALIZADO · P{} · PREMIO {} · {}",
                    pos,
                    super::career::format_usd(champ.prize_awarded_usd),
                    champ.difficulty.label()
                )
            } else {
                format!(
                    "EN CURSO ({}/{} ETAPAS) · {} · {}",
                    champ.completed_stages(),
                    champ.total_stages(),
                    champ.locked_vehicle_name,
                    champ.difficulty.label()
                )
            };
            lines.push((status_hdr, if champ.finished { GOOD } else { INFO }));
            if champ.finished && champ.final_position.is_some_and(|pos| pos <= 3) {
                lines.push((
                    "PODIO REGIONAL LOGRADO · CAMPEONATOS HISTÓRICOS DESBLOQUEADOS".into(),
                    WARN,
                ));
            }
            let standings = champ.standings(&p.pilot_name);
            for row in standings.iter().take(7) {
                let gap = if row.position == 1 || champ.completed_stages() == 0 {
                    String::new()
                } else {
                    format!(
                        " (+{})",
                        super::records::format_time_ms(row.gap_to_leader_ms)
                    )
                };
                let time_str = if champ.completed_stages() == 0 {
                    "--:--.---".into()
                } else {
                    super::records::format_time_ms(row.total_ms)
                };
                let marker = if row.is_player { ">" } else { " " };
                lines.push((
                    format!(
                        "{marker}P{} {:<12} {:<14} {}{}",
                        row.position, row.name, row.car_label, time_str, gap
                    ),
                    if row.is_player { GOOD } else { TEXT },
                ));
            }
        } else {
            let kind = ctx.selected_championship_kind;
            let diff = ctx.selected_career_difficulty;
            lines.push((
                format!(
                    "{} · PILOTO: {} · SALDO: {}",
                    kind.title(),
                    p.pilot_name,
                    super::career::format_usd(p.cash_usd),
                ),
                INFO,
            ));
            lines.push((
                format!(
                    "PREMIOS ({}): 1º {} · 2º {} · 3º {} · 4º {}",
                    diff.label(),
                    super::career::format_usd(diff.prize_for_position(kind, 1)),
                    super::career::format_usd(diff.prize_for_position(kind, 2)),
                    super::career::format_usd(diff.prize_for_position(kind, 3)),
                    super::career::format_usd(diff.prize_for_position(kind, 4)),
                ),
                GOOD,
            ));
            for (i, st) in kind.stages().iter().enumerate() {
                lines.push((
                    format!(
                        "ETAPA {}: {} ({} · {:.2} KM)",
                        i + 1,
                        st.display_name,
                        st.surface,
                        st.length_km
                    ),
                    TEXT,
                ));
            }
            lines.push((
                "REGLAS: DAÑOS Y NEUMÁTICOS SE ARRASTRAN ENTRE ETAPAS · SERVICIO ENTRE TRAMOS"
                    .into(),
                DIM,
            ));
            lines.push((
                if p.historics_unlocked {
                    "HISTÓRICOS DESBLOQUEADOS · ELEGIBLE CON KIT CAR / GRUPO A / GRUPO B".into()
                } else {
                    "PODIO EN COPA REGIONAL DESBLOQUEA EL CAMPEONATO HISTÓRICO".into()
                },
                WARN,
            ));
        }

        for (i, (line, col)) in lines.into_iter().take(9).enumerate() {
            ui::text_clipped(
                canvas,
                px + 12.0,
                py + 10.0 + i as f32 * 16.0,
                pw - 24.0,
                &line,
                0.68,
                col,
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
    pub hud_rpm_style: HudRpmStyle,
    pub invert_pedals: bool,
    pub clutch_help: bool,
    pub volume: f64,
    pub music_volume: f64,
    pub codriver_volume: f64,
    pub camera: u8,
    pub vehicle_id: String,
    pub last_driven_vehicle_id: Option<String>,
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
            hud_rpm_style: HudRpmStyle::default(),
            invert_pedals: false,
            clutch_help: true,
            volume: 0.8,
            music_volume: 0.70,
            codriver_volume: 0.85,
            camera: 0,
            vehicle_id: rerbr_content::VehicleDescriptor::default().id,
            last_driven_vehicle_id: None,
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
                "vehicle_id" if !value.trim().is_empty() => {
                    settings.vehicle_id = value.trim().to_string();
                }
                "last_driven_vehicle_id" if !value.trim().is_empty() => {
                    settings.last_driven_vehicle_id = Some(value.trim().to_string());
                }
                "stage_id" if !value.trim().is_empty() => {
                    settings.stage_id = value.trim().to_string();
                }
                "ngp_experimental" => settings.ngp_experimental = value == "true",
                "hud_rpm_style" => settings.hud_rpm_style = HudRpmStyle::parse(value),
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
        let mut out = format!(
            "sensitivity={}\ndead_zone={}\nfps={}\ndiagnostics={}\nminimal_hud={}\nhud_rpm_style={}\ninvert_pedals={}\nclutch_help={}\nvolume={}\nmusic_volume={}\ncodriver_volume={}\ncamera={}\nvehicle_id={}\nstage_id={}\nngp_experimental={}\n",
            self.sensitivity, self.dead_zone, self.fps, self.diagnostics,
            self.minimal_hud, self.hud_rpm_style.encode(), self.invert_pedals, self.clutch_help, self.volume, self.music_volume, self.codriver_volume, self.camera, self.vehicle_id, self.stage_id, self.ngp_experimental
        );
        if let Some(id) = &self.last_driven_vehicle_id {
            if !id.trim().is_empty() {
                out.push_str(&format!("last_driven_vehicle_id={}\n", id.trim()));
            }
        }
        out
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
            hud_rpm_style: HudRpmStyle::default(),
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
            active_school_lesson: None,
            school_result: None,
            personal_best_ms: None,
            inspected_stage_pb_ms: None,
            record_outcome: None,
            run_splits_ms: &[],
            comparison_pb_splits_ms: None,
            showroom_vehicle_name: Some("Toyota Corolla WRC"),
            showroom_loading: false,
            showroom_error: None,
            title_pulse: 0.5,
            transition_alpha: 0.0,
            coming_soon_notice: None,
            career: None,
            active_career_stage: false,
            last_career_stage_outcome: None,
            pilot_draft_name: "PILOTO RBR",
            pilot_draft_number: 7,
            pending_reset_career: false,
            pending_buy_vehicle_id: None,
            pending_sell_vehicle_id: None,
            selected_career_difficulty: crate::driving::career::CareerDifficulty::Beginner,
            selected_championship_kind: crate::driving::career::ChampionshipKind::Regional,
            selected_service_tyre: crate::driving::career::TyreCompound::GravelHard,
        }
    }

    #[test]
    fn main_menu_has_all_sections_and_navigation_uses_explicit_start() {
        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let ctx = context(&selection, &content);
        let mut menu = Menu::default();
        // Starts on Title Screen; tapping enters horizontal Main Menu.
        assert_eq!(menu.page, Page::Title);
        assert_eq!(
            menu.press_with_ctx(0.5, 0.5, &ctx, &menu.rows(&ctx)),
            Some(Command::EnterMainMenu)
        );
        menu.reset_to_main();
        assert_eq!(menu.page, Page::Main);
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0].label, "CARRERA");
        assert_eq!(rows[1].label, "CONTRARRELOJ");
        assert_eq!(rows[2].label, "ONLINE");
        assert_eq!(rows[3].label, "GARAJE");
        assert_eq!(rows[4].label, "SALÓN 0KM");
        assert_eq!(rows[5].label, "2ª MANO");
        assert_eq!(rows[6].label, "ACADEMIA");
        assert_eq!(rows[7].label, "AJUSTES");
        // Quick drive button on horizontal main menu starts rally explicitly.
        let drive_btn = Menu::main_quick_drive_rect();
        assert_eq!(
            menu.press_with_ctx(
                drive_btn.center_x() as f64,
                (drive_btn.y + drive_btn.h * 0.5) as f64,
                &ctx,
                &rows
            ),
            Some(Command::StartRally)
        );
        // Open center viewport is touch-orbit zone and does NOT accidentally trigger menu buttons.
        assert!(menu.is_showroom_orbit_zone(0.5, 0.45));
        assert_eq!(menu.press_with_ctx(0.5, 0.45, &ctx, &rows), None);

        // Drive hub retains explicit StartRally and no longer exposes legacy "CAMPEONATO RBR".
        menu.open(Page::Drive);
        let drive = menu.rows(&ctx);
        assert!(
            !drive.iter().any(|r| r.label.contains("CAMPEONATO RBR")),
            "Legacy CAMPEONATO RBR row must not appear in CONTRARRELOJ (Page::Drive)"
        );
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
        let mut menu = Menu {
            page: Page::Settings,
            ..Menu::default()
        };
        // 1. Page::Settings is a 5-card visual hub with non-overlapping card hit-boxes.
        let hub_rows = menu.rows(&ctx);
        assert_eq!(hub_rows.len(), 5);
        assert_eq!(hub_rows[0].label, "CONTROLES");
        assert_eq!(hub_rows[1].label, "SONIDO");
        assert_eq!(hub_rows[2].label, "HUD Y CÁMARA");
        assert_eq!(hub_rows[3].label, "CRÉDITOS");
        assert_eq!(hub_rows[4].label, "CONTENIDO");

        let back_rect = Menu::subpage_back_rect();
        let main_rect = Menu::subpage_home_rect();
        assert_eq!(
            menu.press_with_ctx(
                back_rect.center_x() as f64,
                (back_rect.y + back_rect.h * 0.5) as f64,
                &ctx,
                &hub_rows
            ),
            Some(Command::Back)
        );
        assert_eq!(
            menu.press_with_ctx(
                main_rect.center_x() as f64,
                (main_rect.y + main_rect.h * 0.5) as f64,
                &ctx,
                &hub_rows
            ),
            Some(Command::ResetToMain)
        );
        for i in 0..SettingsCard::ALL.len() {
            let r_i = Menu::settings_card_rect(i);
            assert_eq!(
                menu.press_with_ctx(
                    r_i.center_x() as f64,
                    (r_i.y + r_i.h * 0.5) as f64,
                    &ctx,
                    &hub_rows
                ),
                Some(Command::SettingsCardActivate(i))
            );
            for j in (i + 1)..SettingsCard::ALL.len() {
                let r_j = Menu::settings_card_rect(j);
                let overlap_x = r_i.x < r_j.right() && r_i.right() > r_j.x;
                let overlap_y = r_i.y < r_j.bottom() && r_i.bottom() > r_j.y;
                assert!(
                    !(overlap_x && overlap_y),
                    "Settings cards {i} and {j} must not overlap"
                );
            }
        }

        // 2. Page::Controls holds gyroscope sensitivity, dead zone, invert pedals, and clutch help.
        menu.open(Page::Controls);
        let ctrl_rows = menu.rows(&ctx);
        assert_eq!(ctrl_rows.len(), 10);
        for (row_idx, value, expected_down, expected_up) in [
            (0usize, "2.00", Command::SensDown, Command::SensUp),
            (1usize, "0.03", Command::DeadDown, Command::DeadUp),
        ] {
            let rect = menu.row_rect_for(row_idx, ctrl_rows.len());
            let (rx, _ry, rw, _rh) = (rect.x * 640.0, rect.y, rect.w * 640.0, rect.h);
            let value_width = ui::texture_width(value, 0.78);
            let minus_center_x = f64::from((rx + rw - value_width - 42.0) / 640.0);
            let plus_center_x = f64::from((rx + rw - 16.0) / 640.0);
            let center_y = f64::from(rect.y + rect.h * 0.5);
            assert_eq!(
                menu.press(minus_center_x, center_y, &ctrl_rows),
                Some(expected_down)
            );
            assert_eq!(
                menu.press(plus_center_x, center_y, &ctrl_rows),
                Some(expected_up)
            );
        }
        let inv_rect = menu.row_rect_for(4, ctrl_rows.len());
        assert_eq!(
            menu.press(
                inv_rect.center_x() as f64,
                (inv_rect.y + inv_rect.h * 0.5) as f64,
                &ctrl_rows
            ),
            Some(Command::ToggleInvertPedals)
        );
        let clutch_rect = menu.row_rect_for(5, ctrl_rows.len());
        assert_eq!(
            menu.press(
                clutch_rect.center_x() as f64,
                (clutch_rect.y + clutch_rect.h * 0.5) as f64,
                &ctrl_rows
            ),
            Some(Command::ToggleClutchHelp)
        );

        // 3. Page::SoundSettings holds engine/effects, codriver, and menu music volume steppers.
        menu.open(Page::SoundSettings);
        let sound_rows = menu.rows(&ctx);
        assert_eq!(sound_rows.len(), 7);
        for (row_idx, value, expected_down, expected_up) in [
            (0usize, "80%", Command::VolumeDown, Command::VolumeUp),
            (
                1usize,
                "85%",
                Command::CodriverVolumeDown,
                Command::CodriverVolumeUp,
            ),
            (
                2usize,
                "70%",
                Command::MusicVolumeDown,
                Command::MusicVolumeUp,
            ),
        ] {
            let rect = menu.row_rect_for(row_idx, sound_rows.len());
            let (rx, _ry, rw, _rh) = (rect.x * 640.0, rect.y, rect.w * 640.0, rect.h);
            let value_width = ui::texture_width(value, 0.78);
            let minus_center_x = f64::from((rx + rw - value_width - 42.0) / 640.0);
            let plus_center_x = f64::from((rx + rw - 16.0) / 640.0);
            let center_y = f64::from(rect.y + rect.h * 0.5);
            assert_eq!(
                menu.press(minus_center_x, center_y, &sound_rows),
                Some(expected_down)
            );
            assert_eq!(
                menu.press(plus_center_x, center_y, &sound_rows),
                Some(expected_up)
            );
        }

        // 4. Page::HudCameraSettings holds camera view, minimal HUD, RPM HUD style, FPS, telemetry, pacenotes, controls editor, and 3D camera editor.
        menu.open(Page::HudCameraSettings);
        let hud_cam_rows = menu.rows(&ctx);
        assert_eq!(hud_cam_rows.len(), 10);
        let cam_rect = menu.row_rect_for(0, hud_cam_rows.len());
        assert_eq!(
            menu.press(
                cam_rect.center_x() as f64,
                (cam_rect.y + cam_rect.h * 0.5) as f64,
                &hud_cam_rows
            ),
            Some(Command::CameraNext)
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
        assert_eq!(rows.len(), 9);
        let import = rows
            .iter()
            .position(|row| row.label.starts_with("IMPORT"))
            .unwrap();
        let rect = menu.row_rect_for(import, rows.len());
        assert_eq!(
            menu.press(rect.center_x() as f64, rect.y as f64 + 0.01, &rows),
            None
        );
        menu.page = Page::ContentImport;
        let rows = menu.rows(&ctx);
        assert_eq!(rows.len(), IMPORT_STEPS.len() + 1);
        for (index, step) in IMPORT_STEPS.iter().enumerate() {
            let rect = menu.row_rect_for(index, rows.len());
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
        assert_eq!(content.stages.len(), 36);
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
            driving_ms: 125_400,
            penalty_ms: 10_000,
            total_ms: 135_400,
            false_start: true,
            splits_passed: 2,
            splits_total: 2,
        });

        let mut menu = Menu {
            page: Page::StageComplete,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        assert!(rows.iter().any(|r| r.label == "TIEMPO CONDUCCIÓN"));
        assert!(rows.iter().any(|r| r.label == "TIEMPO TOTAL"));
        assert!(rows.iter().any(|r| matches!(
            &r.kind,
            RowKind::Info { value } if value == "02:15.400"
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

        // 1. Enter and exit all submenus from Main
        menu.reset_to_main();
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

        // 2. Pagination test across all 5 pages (36 retail stages including Australia)
        menu.page = Page::Stages;
        menu.stage_page = 0;
        let total_stages = content.stages.len();
        assert_eq!(total_stages, 36);
        let total_pages = total_stages.div_ceil(8);
        assert_eq!(total_pages, 5);

        for expected_page in 0..5 {
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
            Page::Title,
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
            Page::SoundSettings,
            Page::HudCameraSettings,
            Page::CameraEditor,
            Page::ControlsEditor,
            Page::GamepadSettings,
            Page::PacenoteHudSettings,
            Page::About,
            Page::Paused,
            Page::StageComplete,
            Page::Replay,
            Page::RallySchool,
            Page::SchoolBriefing,
            Page::SchoolResult,
            Page::CareerSlots,
            Page::CareerCreate,
            Page::CareerHub,
            Page::Dealership,
            Page::NewDealership,
            Page::CareerGarage,
            Page::CareerChampionship,
            Page::CareerService,
        ];

        // Verify both iPad mini A17 Pro (1024x768) and iPhone 15 wide landscape (2556x1179).
        for size in [(1024, 768), (2556, 1179)] {
            for page in all_pages {
                let menu = Menu {
                    page,
                    ..Menu::default()
                };
                let mut canvas = Canvas::new(size);
                menu.draw(&mut canvas, &ctx, "PRUEBA ESTADO HUD");
                let bytes = canvas.finish();
                assert!(
                    bytes.len() / 20 <= MAX_CANVAS_VERTICES,
                    "Page {:?} at {:?} exceeded vertex budget: {} > {}",
                    page,
                    size,
                    bytes.len() / 20,
                    MAX_CANVAS_VERTICES
                );
            }
        }
    }

    #[test]
    fn settings_persists_last_driven_vehicle_id_independently() {
        let s = Settings {
            vehicle_id: "ford-focus-rs-wrc-2006".into(),
            last_driven_vehicle_id: Some("rsf-toyota-gr-yaris-rally2".into()),
            ..Default::default()
        };
        let encoded = s.encode();
        let parsed = Settings::parse(&encoded);
        assert_eq!(parsed.vehicle_id, "ford-focus-rs-wrc-2006");
        assert_eq!(
            parsed.last_driven_vehicle_id.as_deref(),
            Some("rsf-toyota-gr-yaris-rally2")
        );
        // Legacy settings without `last_driven_vehicle_id` parse cleanly as None.
        let legacy = Settings::parse("vehicle_id=toyota-corolla-wrc\nstage_id=harwood-forest\n");
        assert_eq!(legacy.vehicle_id, "toyota-corolla-wrc");
        assert_eq!(legacy.last_driven_vehicle_id, None);
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

    #[test]
    fn stage_complete_displays_split_summary_with_pb_deltas_and_fits_vertex_budget() {
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};

        let selection = Selection::default();
        let content = Catalog::descriptors(&selection);
        let run_splits = [84_650u64, 168_920u64];
        let prev_pb_splits = [85_750u64, 171_230u64];
        let mut ctx = context(&selection, &content);
        ctx.stage_result = Some(crate::driving::race::StageResult {
            driving_seconds: 245.5,
            penalty_seconds: 0.0,
            total_seconds: 245.5,
            driving_ms: 245_500,
            penalty_ms: 0,
            total_ms: 245_500,
            false_start: false,
            splits_passed: 2,
            splits_total: 2,
        });
        ctx.run_splits_ms = &run_splits;
        ctx.comparison_pb_splits_ms = Some(&prev_pb_splits);
        ctx.has_replay = true;

        let menu = Menu {
            page: Page::StageComplete,
            ..Menu::default()
        };
        let rows = menu.rows(&ctx);
        let p1 = rows
            .iter()
            .find(|r| r.label == "PARCIAL 1")
            .expect("PARCIAL 1");
        let p2 = rows
            .iter()
            .find(|r| r.label == "PARCIAL 2")
            .expect("PARCIAL 2");
        assert_eq!(
            p1.kind,
            RowKind::Info {
                value: "01:24.650 (-00:01.100)".into()
            }
        );
        assert_eq!(
            p2.kind,
            RowKind::Info {
                value: "02:48.920 (-00:02.310)".into()
            }
        );
        let mut canvas = Canvas::new((1024, 768));
        menu.draw(&mut canvas, &ctx, "STAGE COMPLETE");
        let bytes = canvas.finish();
        assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
    }
}
