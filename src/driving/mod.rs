//! Original physics-owned driving presentation, menus and session controls.
pub mod audio;
pub mod cameras;
pub mod career;
#[cfg(test)]
mod catalog_audit;
pub mod championship;
pub mod codriver;
pub mod content_manager;
pub mod controls;
pub mod fmod_audit;
pub mod fmod_synth;
pub mod fsb5_vorbis_setup;
pub mod gamepad;
pub mod layout;
pub(crate) mod loading;
pub mod menu;
pub mod ngp_experimental;
pub mod ngp_proto;
pub mod pacenote_icons;
#[cfg(test)]
mod private_tests;
mod race;
pub mod records;
pub mod replay;
pub mod rsf_catalog;
pub mod school;
mod ui;
pub mod vorbis;
mod worker;
use crate::{
    car::{
        mesh::Geometry,
        placement::{ChasePath, Frame},
    },
    harwood::{
        benchmark::hud::{Canvas, Hud},
        Camera,
    },
};
use content_manager::{Catalog, PUBLIC_BETA};
use controls::Controls;
use layout::{DrivingLayout, Rect};
use rerbr_render_core::{coords, Mat4, Quat, Vec3};
use rerbr_sim::diagnostics::Sample;
use std::time::{Duration, Instant};
use ui::{ACCENT, BAD, INFO, LINE, PANEL, PANEL_ACTIVE, PANEL_DEEP, TEXT, WARN};
use winit::event::{Touch, TouchPhase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneAction {
    Load {
        selection: Box<rerbr_content::Selection>,
        experimental_ngp: bool,
    },
    Unload,
}

pub struct Runtime {
    #[cfg(feature = "physics-stage")]
    pub view: cameras::View,
    #[cfg(feature = "physics-stage")]
    pub mounts: Option<cameras::Mounts>,
    #[cfg(feature = "physics-stage")]
    pub camera_overrides: cameras::VehicleCameraStore,
    pub controls_layout: layout::ControlsLayoutOverrides,
    pub selected_control: layout::EditableControl,
    dragging_control: Option<(u64, layout::EditableControl, f32, f32)>,
    drawable_size: (u32, u32),
    effective_fov: f32,
    worker: worker::Worker,
    selection: rerbr_content::Selection,
    pending_scene_action: Option<SceneAction>,
    race: race::Clock,
    loading_started: Instant,

    pub controls: Controls,
    clutch_help: bool,
    diagnostics: bool,
    minimal_hud: bool,
    hud_rpm_style: menu::HudRpmStyle,
    volume: f64,
    music_volume: f64,
    codriver_volume: f64,
    codriver: Option<codriver::CoDriverSession>,
    content: Catalog,
    message: Option<String>,
    menu: menu::Menu,
    fps_visible: bool,
    scene_error: Option<String>,
    pub steering_angle: f32,
    pub cockpit_ready: bool,
    pub cockpit_node: Option<Mat4>,
    pub cockpit_error: Option<String>,
    pivots: Option<[Vec3; 4]>,
    wheel_visual_offsets: [Vec3; 4],
    offset: Option<Vec3>,
    last: Instant,
    tilt: f64,
    camera: Option<Camera>,
    fps: f64,
    cpu_ms: f64,
    footprint: Option<u64>,
    footprint_at: Instant,
    pub stage_cameras: Option<replay::StageCameras>,
    pub replay_recorder: replay::ReplayRecorder,
    pub saved_replay: Option<replay::Replay>,
    pub replay_player: Option<replay::ReplayPlayer>,
    pub ngp_experimental: bool,
    saved_ngp_preference: bool,
    pub focus_diagnostics: Option<ngp_experimental::FocusDiagnosticsReport>,
    pub experimental_visual_only: bool,
    pub experimental_physics_reason: Option<String>,
    pub visual_start_pose: Option<(Vec3, Vec3)>,
    pub gamepad: gamepad::GamepadRuntime,
    gamepad_started: Instant,
    gamepad_frame: gamepad::GamepadDrivingFrame,
    gamepad_polled_this_frame: bool,
    motion_polled_this_frame: bool,
    recover_car_frames: u32,
    pub pacenote_atlas: Option<pacenote_icons::PacenoteIconAtlas>,
    pacenote_atlas_status: String,
    pub pacenote_hud: pacenote_icons::PacenoteHudSettings,
    pub championship: championship::ChampionshipCalendar,
    pub school: school::SchoolCatalog,
    pub active_school_lesson: Option<u32>,
    pub active_school_result: Option<school::SchoolRunResult>,
    school_incident_banner: Option<(String, Instant)>,
    last_session_snapshot: Option<ngp_experimental::SessionTelemetrySnapshot>,
    pub records: records::RecordStore,
    pub last_record_outcome: Option<records::RecordOutcome>,
    split_tracker: records::SplitTracker,
    last_finish_splits_ms: Vec<u64>,
    last_run_prev_pb_splits_ms: Option<Vec<u64>>,
    last_finish_crossed_forward: bool,
    pub showroom_camera: crate::car::showroom::ShowroomCamera,
    showroom_drag: Option<(u64, f32, f32)>,
    title_started: Instant,
    transition_started: Option<Instant>,
    title_vehicle_order: Vec<rerbr_content::VehicleDescriptor>,
    pub last_driven_vehicle_id: Option<String>,
    showroom_active_name: Option<String>,
    showroom_loading: bool,
    showroom_error: Option<String>,
    pub showroom_dorsal_positions: Vec<(f32, f32)>,
    coming_soon_notice: Option<(String, Instant)>,
    pub career: career::CareerStore,
    active_career_stage: bool,
    last_career_stage_outcome: Option<career::StageRecordOutcome>,
    pilot_preset_index: usize,
    pilot_tag_index: usize,
    pilot_draft_name: String,
    pilot_draft_number: u8,
    pending_reset_career: bool,
    career_create_return_dealership: Option<menu::Page>,
    pending_buy_vehicle_id: Option<String>,
    pending_sell_vehicle_id: Option<String>,
    selected_career_difficulty: career::CareerDifficulty,
    selected_championship_kind: career::ChampionshipKind,
    selected_service_tyre: career::TyreCompound,
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
impl Runtime {
    pub fn new() -> Self {
        Self::new_for_catalog(PUBLIC_BETA)
    }

    fn new_for_catalog(public_beta: bool) -> Self {
        let settings = menu::Settings::load();
        let default_selection = rerbr_content::Selection::default();
        let scanned = Catalog::scan_for_app(&default_selection, public_beta);
        let stage = restored_stage(&scanned, &settings.stage_id);
        let preferred_vehicle_id = settings
            .last_driven_vehicle_id
            .as_deref()
            .unwrap_or(&settings.vehicle_id);
        let ngp_experimental = public_beta || settings.ngp_experimental;
        let vehicle = restored_vehicle(&scanned, preferred_vehicle_id, ngp_experimental);
        let selection = rerbr_content::Selection { vehicle, stage };
        let content = Catalog::scan_for_app(&selection, public_beta);
        let title_vehicle_order = shuffled_showroom_candidates(&content);
        let last_driven_vehicle_id = settings.last_driven_vehicle_id.clone();
        let mut controls = Controls::new();
        controls.sensitivity = settings.sensitivity;
        controls.dead_zone = settings.dead_zone;
        controls.invert_pedals = settings.invert_pedals;
        audio::set_master_volume(settings.volume as f32);
        audio::set_music_volume(settings.music_volume as f32);
        audio::set_codriver_volume(settings.codriver_volume as f32);
        audio::set_codriver_active(false);
        audio::clear_speech_queue();
        if let Some(doc) = std::env::var_os("RERBR_PROBE_DOCUMENTS") {
            audio::init_menu_music(std::path::Path::new(&doc));
        }
        let (pacenote_atlas, pacenote_atlas_status) =
            match std::env::var_os("RERBR_PROBE_DOCUMENTS") {
                Some(doc) => match pacenote_icons::PacenoteIconAtlas::load_from_documents(
                    std::path::Path::new(&doc),
                ) {
                    Ok(atlas) => {
                        let msg = format!("{} OK", atlas.bank_name);
                        (Some(atlas), msg)
                    }
                    Err(err) => (None, err),
                },
                None => (None, "DOCUMENTS NO DISPONIBLE".into()),
            };
        audio::set_menu_music_active(true);
        Self {
            #[cfg(feature = "physics-stage")]
            view: cameras::View::from_index(settings.camera),
            #[cfg(feature = "physics-stage")]
            mounts: None,
            #[cfg(feature = "physics-stage")]
            camera_overrides: cameras::VehicleCameraStore::load(),
            controls_layout: layout::ControlsLayoutOverrides::load(),
            selected_control: layout::EditableControl::Brake,
            dragging_control: None,
            drawable_size: (1024, 768),
            effective_fov: 65.0,
            worker: worker::Worker::idle(),
            content,
            selection,
            pending_scene_action: None,
            race: race::Clock::default(),
            loading_started: Instant::now(),

            controls,
            clutch_help: settings.clutch_help,
            diagnostics: settings.diagnostics,
            minimal_hud: settings.minimal_hud,
            hud_rpm_style: settings.hud_rpm_style,
            volume: settings.volume,
            music_volume: settings.music_volume,
            codriver_volume: settings.codriver_volume,
            codriver: None,
            message: None,
            menu: menu::Menu::default(),
            fps_visible: settings.fps,
            scene_error: None,
            steering_angle: 0.0,
            cockpit_ready: false,
            cockpit_node: None,
            cockpit_error: None,
            pivots: None,
            wheel_visual_offsets: [Vec3::ZERO; 4],
            offset: None,
            last: Instant::now(),
            tilt: f64::NAN,
            camera: None,
            fps: 0.0,
            cpu_ms: 0.0,
            footprint: None,
            footprint_at: Instant::now(),
            stage_cameras: None,
            replay_recorder: replay::ReplayRecorder::new(),
            saved_replay: None,
            replay_player: None,
            ngp_experimental,
            saved_ngp_preference: settings.ngp_experimental,
            focus_diagnostics: None,
            experimental_visual_only: false,
            experimental_physics_reason: None,
            visual_start_pose: None,
            gamepad: gamepad::GamepadRuntime::load(),
            gamepad_started: Instant::now(),
            gamepad_frame: gamepad::GamepadDrivingFrame::default(),
            gamepad_polled_this_frame: false,
            motion_polled_this_frame: false,
            recover_car_frames: 0,
            pacenote_atlas,
            pacenote_atlas_status,
            pacenote_hud: pacenote_icons::PacenoteHudSettings::load(),
            championship: championship::ChampionshipCalendar::load_default(),
            school: school::SchoolCatalog::load_default(),
            active_school_lesson: None,
            active_school_result: None,
            school_incident_banner: None,
            last_session_snapshot: None,
            records: records::RecordStore::load(),
            last_record_outcome: None,
            split_tracker: records::SplitTracker::default(),
            last_finish_splits_ms: Vec::new(),
            last_run_prev_pb_splits_ms: None,
            last_finish_crossed_forward: false,
            showroom_camera: crate::car::showroom::ShowroomCamera::default(),
            showroom_drag: None,
            title_started: Instant::now(),
            transition_started: None,
            title_vehicle_order,
            last_driven_vehicle_id,
            showroom_active_name: None,
            showroom_loading: false,
            showroom_error: None,
            showroom_dorsal_positions: Vec::new(),
            coming_soon_notice: None,
            career: career::CareerStore::load(),
            active_career_stage: false,
            last_career_stage_outcome: None,
            pilot_preset_index: 0,
            pilot_tag_index: 0,
            pilot_draft_name: career::PILOT_NAME_PRESETS[0].to_string(),
            pilot_draft_number: 7,
            pending_reset_career: false,
            career_create_return_dealership: None,
            pending_buy_vehicle_id: None,
            pending_sell_vehicle_id: None,
            selected_career_difficulty: career::CareerDifficulty::Beginner,
            selected_championship_kind: career::ChampionshipKind::Regional,
            selected_service_tyre: career::TyreCompound::GravelHard,
        }
    }

    fn career_boot_overrides(&self) -> rerbr_sim::drive::BootOverrides {
        if !self.active_career_stage {
            return rerbr_sim::drive::BootOverrides::default();
        }
        let state = self
            .career
            .profile
            .as_ref()
            .and_then(|profile| profile.vehicle_state(&self.selection.vehicle.id));
        rerbr_sim::drive::BootOverrides {
            tyre: state.map(|state| state.installed_tyre.rbr_tyre_id()),
            carried_damage_bytes: state.and_then(|state| state.condition.carried_damage_bytes()),
            damage_level: Some(rerbr_sim::drive::DamageLevel::Realistic),
        }
    }

    /// True when the device/window is in landscape orientation (`width > height`), or a finite
    /// CoreMotion tilt sample or external gamepad is active. This prevents false "Sujeta en horizontal"
    /// blocks in menus (`CareerChampionship`, `Drive`) before a 3D stage `ChasePath` has been loaded.
    pub fn is_landscape_ready(&self) -> bool {
        self.tilt.is_finite()
            || self.gamepad.connected
            || (self.drawable_size.0 > self.drawable_size.1 && self.drawable_size.1 > 0)
    }

    /// Returns `(primary, fallbacks)` for the 3D Showroom when a showroom page is active.
    /// - Title Screen: random car chosen once at app startup (independent of last-driven car).
    /// - Main Menu: last-driven car (persisted across restarts & stage returns), falling back cleanly.
    /// - Garage: currently inspected car (drivable or visual-only), with no silent car substitution.
    pub fn showroom_request(
        &self,
    ) -> Option<(
        crate::car::showroom::ShowroomMode,
        rerbr_content::VehicleDescriptor,
        Vec<rerbr_content::VehicleDescriptor>,
    )> {
        use crate::car::showroom::ShowroomMode;
        match self.menu.page {
            menu::Page::Title => {
                let mut candidates = self.title_vehicle_order.iter().filter(|car| {
                    !self.content.is_public_beta() || self.content.vehicle(&car.id).is_some()
                });
                let first = candidates.next()?.clone();
                let rest = candidates.cloned().collect();
                Some((ShowroomMode::Title, first, rest))
            }
            menu::Page::Main => {
                let mut candidates: Vec<rerbr_content::VehicleDescriptor> = Vec::new();
                if let Some(last_id) = &self.last_driven_vehicle_id {
                    if let Some(entry) = self
                        .content
                        .vehicle(last_id)
                        .filter(|e| e.can_showroom_view())
                    {
                        candidates.push(entry.descriptor.clone());
                    }
                }
                if let Some(entry) = self
                    .content
                    .vehicle(&self.selection.vehicle.id)
                    .filter(|e| e.can_showroom_view())
                {
                    if !candidates.iter().any(|c| c.id == entry.descriptor.id) {
                        candidates.push(entry.descriptor.clone());
                    }
                }
                for entry in self.content.showroom_candidates() {
                    if !candidates.iter().any(|c| c.id == entry.descriptor.id) {
                        candidates.push(entry.descriptor.clone());
                    }
                }
                let primary = candidates.first()?.clone();
                let fallbacks = candidates.into_iter().skip(1).collect();
                Some((ShowroomMode::MainMenu, primary, fallbacks))
            }
            menu::Page::Garage => {
                let ctx = self.context();
                let indices = self.menu.garage_indices(&ctx);
                let active_idx = if indices.contains(&self.menu.inspected_vehicle) {
                    self.menu.inspected_vehicle
                } else {
                    indices.first().copied().unwrap_or(0)
                };
                let entry = self.content.vehicles.get(active_idx)?;
                if !entry.can_showroom_view() {
                    return None;
                }
                Some((ShowroomMode::Garage, entry.descriptor.clone(), Vec::new()))
            }
            menu::Page::Dealership | menu::Page::NewDealership => {
                let ctx = self.context();
                let indices = self.menu.dealership_indices(&ctx);
                let active_idx = if indices.contains(&self.menu.dealership_vehicle) {
                    self.menu.dealership_vehicle
                } else {
                    *indices.first()?
                };
                let entry = self.content.vehicles.get(active_idx)?;
                if !entry.can_showroom_view() {
                    return None;
                }
                Some((ShowroomMode::Garage, entry.descriptor.clone(), Vec::new()))
            }
            menu::Page::CareerGarage => {
                let ctx = self.context();
                let indices = self.menu.career_garage_indices(&ctx);
                let cursor = self
                    .menu
                    .career_garage_cursor
                    .min(indices.len().saturating_sub(1));
                let active_idx = *indices.get(cursor)?;
                let entry = self.content.vehicles.get(active_idx)?;
                if !entry.can_showroom_view() {
                    return None;
                }
                Some((ShowroomMode::Garage, entry.descriptor.clone(), Vec::new()))
            }
            _ => None,
        }
    }

    pub fn is_showroom_page(&self) -> bool {
        self.menu.page.is_showroom_page()
    }

    pub fn sync_showroom_status(
        &mut self,
        active_id: Option<&str>,
        loading: bool,
        error: Option<&str>,
    ) {
        self.showroom_loading = loading;
        self.showroom_error = error.map(str::to_string);
        self.showroom_active_name = active_id.and_then(|id| {
            self.content
                .vehicle(id)
                .map(|e| e.descriptor.name.clone())
                .or_else(|| {
                    self.title_vehicle_order
                        .iter()
                        .find(|v| v.id == id)
                        .map(|v| v.name.clone())
                })
        });
    }
    pub fn process_gamepad_snapshot(&mut self, snap: &gamepad::GamepadSnapshot, now_secs: f64) {
        if self.menu.page == menu::Page::Driving {
            let f = self.gamepad.driving_input(snap);
            self.gamepad_frame = f;
            if f.pause_pressed {
                self.pause();
                return;
            }
            if f.camera_pressed {
                self.menu_command(menu::Command::CameraNext);
            }
            if f.recover_pressed {
                self.menu_command(menu::Command::RecoverCar);
            }
            if !self.controls.automatic && f.gear_delta != 0 {
                let max_gear = self.selection.vehicle.forward_gears + 1;
                if f.gear_delta > 0 {
                    self.controls.selected_gear = (self.controls.selected_gear + 1).min(max_gear);
                } else {
                    self.controls.selected_gear = self.controls.selected_gear.saturating_sub(1);
                }
            }
        } else {
            self.gamepad_frame = gamepad::GamepadDrivingFrame::default();
            let events = self.gamepad.menu_events(snap, now_secs);
            if events.is_empty() {
                return;
            }
            let rows = {
                let ctx = self.context();
                self.menu.rows(&ctx)
            };
            for ev in events {
                if self.menu.page == menu::Page::Title {
                    if matches!(
                        ev,
                        gamepad::MenuNavEvent::Confirm | gamepad::MenuNavEvent::Pause
                    ) {
                        self.menu_command(menu::Command::EnterMainMenu);
                    }
                    continue;
                }
                if self.menu.page == menu::Page::Main {
                    match ev {
                        gamepad::MenuNavEvent::Left => {
                            self.menu_command(menu::Command::MainSectionStep(-1));
                        }
                        gamepad::MenuNavEvent::Right => {
                            self.menu_command(menu::Command::MainSectionStep(1));
                        }
                        gamepad::MenuNavEvent::Confirm => {
                            let idx = self.menu.main_section;
                            self.menu_command(menu::Command::MainSectionActivate(idx));
                        }
                        _ => {}
                    }
                    continue;
                }
                if self.menu.page == menu::Page::Settings {
                    match ev {
                        gamepad::MenuNavEvent::Left | gamepad::MenuNavEvent::Up => {
                            self.menu_command(menu::Command::SettingsCardStep(-1));
                        }
                        gamepad::MenuNavEvent::Right | gamepad::MenuNavEvent::Down => {
                            self.menu_command(menu::Command::SettingsCardStep(1));
                        }
                        gamepad::MenuNavEvent::Confirm => {
                            let idx = self.menu.settings_card;
                            self.menu_command(menu::Command::SettingsCardActivate(idx));
                        }
                        gamepad::MenuNavEvent::Back => {
                            self.menu_command(menu::Command::Back);
                        }
                        _ => {}
                    }
                    continue;
                }
                if self.menu.page == menu::Page::Garage {
                    match ev {
                        gamepad::MenuNavEvent::Left => {
                            self.menu_command(menu::Command::GaragePrevCar);
                        }
                        gamepad::MenuNavEvent::Right => {
                            self.menu_command(menu::Command::GarageNextCar);
                        }
                        gamepad::MenuNavEvent::Confirm => {
                            let idx = self.menu.inspected_vehicle;
                            self.menu_command(menu::Command::SelectVehicle(idx));
                        }
                        gamepad::MenuNavEvent::Back => {
                            self.menu_command(menu::Command::Back);
                        }
                        _ => {}
                    }
                    continue;
                }
                if matches!(
                    self.menu.page,
                    menu::Page::Dealership | menu::Page::NewDealership
                ) {
                    match ev {
                        gamepad::MenuNavEvent::Left => {
                            self.menu_command(menu::Command::DealershipPrevCar);
                        }
                        gamepad::MenuNavEvent::Right => {
                            self.menu_command(menu::Command::DealershipNextCar);
                        }
                        gamepad::MenuNavEvent::Confirm => {
                            if self.career.profile.is_none() {
                                self.career_create_return_dealership = Some(self.menu.page);
                                self.menu_command(menu::Command::Open(menu::Page::CareerCreate));
                            } else if self.pending_buy_vehicle_id.is_some() {
                                self.menu_command(menu::Command::DealershipConfirmBuy);
                            } else {
                                self.menu_command(menu::Command::DealershipRequestBuy);
                            }
                        }
                        gamepad::MenuNavEvent::Back => {
                            if self.pending_buy_vehicle_id.is_some() {
                                self.menu_command(menu::Command::DealershipCancelBuy);
                            } else {
                                self.menu_command(menu::Command::Back);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                if self.menu.page == menu::Page::CareerGarage {
                    match ev {
                        gamepad::MenuNavEvent::Left => {
                            self.menu_command(menu::Command::CareerGaragePrevCar);
                        }
                        gamepad::MenuNavEvent::Right => {
                            self.menu_command(menu::Command::CareerGarageNextCar);
                        }
                        gamepad::MenuNavEvent::Confirm => {
                            self.menu_command(menu::Command::CareerGarageSelectActive);
                        }
                        gamepad::MenuNavEvent::Back => {
                            self.menu_command(menu::Command::Back);
                        }
                        _ => {}
                    }
                    continue;
                }
                match ev {
                    gamepad::MenuNavEvent::Up => self.menu.focus_move(&rows, -1),
                    gamepad::MenuNavEvent::Down => self.menu.focus_move(&rows, 1),
                    gamepad::MenuNavEvent::Left => {
                        if let Some(cmd) = self.menu.focus_adjust(&rows, -1) {
                            self.menu_command(cmd);
                        }
                    }
                    gamepad::MenuNavEvent::Right => {
                        if let Some(cmd) = self.menu.focus_adjust(&rows, 1) {
                            self.menu_command(cmd);
                        }
                    }
                    gamepad::MenuNavEvent::Confirm => {
                        if self.menu.page == menu::Page::Replay {
                            self.menu_command(menu::Command::ReplayTogglePause);
                        } else if let Some(cmd) = self.menu.focus_confirm(&rows) {
                            self.menu_command(cmd);
                        }
                    }
                    gamepad::MenuNavEvent::Back => {
                        if self.menu.page == menu::Page::Paused {
                            self.menu_command(menu::Command::Start);
                        } else if self.menu.page == menu::Page::Replay {
                            self.menu_command(menu::Command::ReplayExit);
                        } else {
                            self.menu_command(menu::Command::Back);
                        }
                    }
                    gamepad::MenuNavEvent::Pause => {
                        if self.menu.page == menu::Page::Paused {
                            self.menu_command(menu::Command::Start);
                        } else if self.menu.page == menu::Page::Replay {
                            self.menu_command(menu::Command::ReplayTogglePause);
                        }
                    }
                }
            }
        }
    }
    fn poll_gamepad(&mut self) {
        let snap = gamepad::poll_system_gamepad();
        let now_secs = self.gamepad_started.elapsed().as_secs_f64();
        self.process_gamepad_snapshot(&snap, now_secs);
    }
    pub fn take_scene_action(&mut self) -> Option<SceneAction> {
        self.pending_scene_action.take()
    }
    pub fn camera_fov_degrees(&self) -> f32 {
        self.effective_fov
    }
    fn effective_layout(&self, size: (u32, u32)) -> DrivingLayout {
        self.controls_layout.apply(DrivingLayout::for_size(size))
    }
    fn stage_elapsed_secs(&self) -> Option<f64> {
        self.race.seconds()
    }
    fn active_record_key(&self, stage: &rerbr_content::StageDescriptor) -> records::RecordKey {
        let ngp_rt = self.focus_diagnostics.as_ref().and_then(|d| {
            d.steps.iter().find(|s| s.stage == "NGP DATA").map(|s| {
                if s.detail.contains("7.4.778.507") {
                    "ngp-7.4.778.507"
                } else if s.detail.contains("7.5") {
                    "ngp-7.5"
                } else {
                    "ngp-7.4.778"
                }
            })
        });
        let physics_version = records::physics_version_for(&self.selection.vehicle, ngp_rt);
        records::RecordKey::from_parts(
            &self.selection.vehicle,
            stage,
            physics_version,
            self.clutch_help,
        )
    }
    fn personal_best_for_stage(&self, stage: &rerbr_content::StageDescriptor) -> Option<u64> {
        let key = self.active_record_key(stage);
        self.records.get(&key).map(|pb| pb.best_total_ms)
    }
    fn personal_best_splits_for_stage(
        &self,
        stage: &rerbr_content::StageDescriptor,
    ) -> Option<Vec<u64>> {
        let key = self.active_record_key(stage);
        self.records
            .get(&key)
            .filter(|pb| !pb.splits_ms.is_empty())
            .map(|pb| pb.splits_ms.clone())
    }
    pub fn install(&mut self, geometry: &Geometry) {
        self.install_pivots(geometry.pivots);
        self.install_wheel_visual_offsets(geometry.wheel_visual_offsets);
    }
    pub(crate) fn install_pivots(&mut self, pivots: [Vec3; 4]) {
        self.pivots = Some(pivots);
    }
    pub(crate) fn install_wheel_visual_offsets(&mut self, offsets: [Vec3; 4]) {
        self.wheel_visual_offsets = offsets;
    }
    pub(crate) fn set_visual_start_pose(&mut self, cam: Camera) {
        self.visual_start_pose = Some((cam.target, cam.target - cam.eye));
    }
    fn school_speech_allowed(&self) -> bool {
        self.active_school_lesson.is_some()
            && matches!(
                self.menu.page,
                menu::Page::SchoolBriefing | menu::Page::SchoolResult | menu::Page::Driving
            )
    }
    fn capture_session_snapshot(&mut self, active_now: bool) {
        let Ok(st) = self.worker.state.try_lock() else {
            return;
        };
        let latest = st.pair.as_ref().map(|(_, b, _)| b).or(st.rest.as_ref());
        let sim_steps = latest.map_or(0, |s| s.step);
        let physics_settled = st.rest.is_some();
        let has_data = physics_settled || st.error.is_some() || sim_steps > 0;
        if !has_data {
            return;
        }
        let loaded_report =
            crate::car::last_loaded_report().filter(|r| r.vehicle_id == self.selection.vehicle.id);
        let fmod_snap = audio::fmod_runtime_snapshot();
        self.last_session_snapshot = Some(ngp_experimental::SessionTelemetrySnapshot {
            vehicle_id: self.selection.vehicle.id.clone(),
            vehicle_name: self.selection.vehicle.name.clone(),
            model_path: self.selection.vehicle.model.clone(),
            stage_id: self.selection.stage.id.clone(),
            active_now,
            had_driving_phase: matches!(
                self.race.phase,
                race::Phase::Driving | race::Phase::Finishing | race::Phase::Finished
            ) || sim_steps > 0,
            worker_error: st.error.clone(),
            physics_settled,
            physics_status: st.physics_status.clone(),
            sim_steps,
            speed_mps: latest.map_or(0.0, Sample::speed),
            rpm: latest.map_or(0.0, |s| s.rpm),
            gear: latest.map_or(1, |s| s.gear),
            throttle_rx: latest.map_or(0.0, |s| s.throttle_received),
            steering_rx: latest.map_or(0.0, |s| s.steering_received),
            clutch_rx: latest.map_or(0.0, |s| s.payload_clutch),
            clutch_help: self.clutch_help,
            audio_status: audio::status(),
            loaded_model_path: loaded_report.as_ref().map(|r| r.model_path.clone()),
            loaded_draw_count: loaded_report.as_ref().map_or(0, |r| r.draw_count),
            loaded_textures: loaded_report.map_or_else(Vec::new, |r| r.textures),
            fmod_bank: fmod_snap.as_ref().map(|f| f.bank_path.clone()),
            fmod_decoded_samples: fmod_snap.as_ref().map_or(0, |f| f.decoded_samples),
            fmod_total_samples: fmod_snap.as_ref().map_or(0, |f| f.total_samples),
            fmod_active_events: fmod_snap
                .as_ref()
                .map_or_else(Vec::new, |f| f.active_events.clone()),
            fmod_rendered_frames: fmod_snap.as_ref().map_or(0, |f| f.rendered_frames),
        });
    }
    fn refresh_focus_diagnostics(&mut self) {
        let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(std::path::PathBuf::from);
        let active_now = !matches!(self.race.phase, race::Phase::Idle | race::Phase::Finished);
        self.capture_session_snapshot(active_now);
        let inspected = self
            .content
            .vehicles
            .get(self.menu.inspected_vehicle)
            .map(|e| &e.descriptor)
            .filter(|v| v.is_ngp_experimental_car());
        let last_ngp = self
            .last_session_snapshot
            .as_ref()
            .and_then(|snap| self.content.vehicle(&snap.vehicle_id))
            .map(|e| &e.descriptor)
            .filter(|v| v.is_ngp_experimental_car());
        let prefer_inspected = matches!(
            self.menu.page,
            menu::Page::GarageDetail | menu::Page::NgpExperimental
        );
        let target = if prefer_inspected {
            if let Some(v) = inspected {
                v.clone()
            } else if self.selection.vehicle.is_ngp_experimental_car() {
                self.selection.vehicle.clone()
            } else if let Some(v) = last_ngp {
                v.clone()
            } else {
                rerbr_content::VehicleDescriptor::ford_focus_2006()
            }
        } else if self.selection.vehicle.is_ngp_experimental_car() {
            self.selection.vehicle.clone()
        } else if let Some(v) = inspected {
            v.clone()
        } else if let Some(v) = last_ngp {
            v.clone()
        } else {
            rerbr_content::VehicleDescriptor::ford_focus_2006()
        };
        let mut report = ngp_experimental::FocusDiagnosticsReport::inspect(doc.as_deref(), &target);
        if let Some(snap) = &self.last_session_snapshot {
            report.apply_session_snapshot(snap);
        }
        self.focus_diagnostics = Some(report);
    }
    /// Always inspects the currently selected vehicle (used by StartRally / Restart so
    /// an unrelated `inspected_vehicle` index in Garage can never override the active car).
    fn refresh_diagnostics_for_selected(&mut self) {
        let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(std::path::PathBuf::from);
        let active_now = !matches!(self.race.phase, race::Phase::Idle | race::Phase::Finished);
        self.capture_session_snapshot(active_now);
        let mut report = ngp_experimental::FocusDiagnosticsReport::inspect(
            doc.as_deref(),
            &self.selection.vehicle,
        );
        if let Some(snap) = &self.last_session_snapshot {
            report.apply_session_snapshot(snap);
        }
        self.focus_diagnostics = Some(report);
    }
    pub fn selection(&self) -> &rerbr_content::Selection {
        &self.selection
    }
    fn clear_scene_pose(&mut self) {
        self.pivots = None;
        self.wheel_visual_offsets = [Vec3::ZERO; 4];
        self.offset = None;
        self.camera = None;
        self.scene_error = None;
        self.cockpit_node = None;
        self.cockpit_ready = false;
        self.cockpit_error = None;
        self.experimental_visual_only = false;
        self.experimental_physics_reason = None;
        self.visual_start_pose = None;
        self.steering_angle = 0.0;
        self.controls.clear();
        self.controls.selected_gear = 1;
        self.dragging_control = None;
        #[cfg(feature = "physics-stage")]
        {
            self.mounts = None;
        }
    }
    pub fn configure_selection(
        &mut self,
        selection: rerbr_content::Selection,
    ) -> Result<(), String> {
        selection.validate_for_mode(self.ngp_experimental)?;
        if !stage_supported(&selection.stage) {
            return Err("multiple stages require physics-stage mode".into());
        }
        // Upload may confirm the same selection. Browsing changes only idle metadata.
        if self.selection != selection && self.race.phase != race::Phase::Idle {
            return Err("RETURN TO MAIN MENU BEFORE CHANGING CONTENT".into());
        }
        let content = Catalog::scan_for_app(&selection, self.content.is_public_beta());
        if self.content.is_public_beta()
            && !content
                .vehicle(&selection.vehicle.id)
                .is_some_and(|e| e.descriptor == selection.vehicle)
        {
            return Err("COCHE NO DISPONIBLE EN LA BETA".into());
        }
        self.controls.configure_vehicle(&selection.vehicle)?;
        self.selection = selection;
        self.content = content;
        Ok(())
    }
    pub fn asset_error(&mut self, error: String) {
        self.pause();
        self.scene_error = Some(error.clone());
        self.worker.state.lock().unwrap().error = Some(error);
    }
    pub fn pause(&mut self) {
        self.menu.pause();
        self.controls.clear();
        self.worker.state.lock().unwrap().paused = true;
        self.race.pause();
        audio::set_paused(true);
        motion(false);
    }
    fn camera_index(&self) -> u8 {
        #[cfg(feature = "physics-stage")]
        {
            self.view.index()
        }
        #[cfg(not(feature = "physics-stage"))]
        {
            0
        }
    }
    fn camera_label(&self) -> &'static str {
        #[cfg(feature = "physics-stage")]
        {
            self.view.short_label()
        }
        #[cfg(not(feature = "physics-stage"))]
        {
            "CHASE"
        }
    }
    fn save_settings(&self) {
        menu::Settings {
            sensitivity: self.controls.sensitivity,
            dead_zone: self.controls.dead_zone,
            fps: self.fps_visible,
            diagnostics: self.diagnostics,
            minimal_hud: self.minimal_hud,
            hud_rpm_style: self.hud_rpm_style,
            invert_pedals: self.controls.invert_pedals,
            clutch_help: self.clutch_help,
            volume: self.volume,
            music_volume: self.music_volume,
            codriver_volume: self.codriver_volume,
            camera: self.camera_index(),
            vehicle_id: self.selection.vehicle.id.clone(),
            last_driven_vehicle_id: self.last_driven_vehicle_id.clone(),
            stage_id: self.selection.stage.id.clone(),
            ngp_experimental: if self.content.is_public_beta() {
                self.saved_ngp_preference
            } else {
                self.ngp_experimental
            },
        }
        .save();
    }
    fn menu_command(&mut self, command: menu::Command) {
        self.apply_command(command);
        self.save_settings();
    }
    fn apply_command(&mut self, command: menu::Command) {
        use menu::Command::*;
        match command {
            EnterMainMenu => {
                if self.menu.page == menu::Page::Title {
                    self.transition_started = Some(Instant::now());
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::MainMenu);
                    self.showroom_drag = None;
                    self.coming_soon_notice = None;
                    self.menu.reset_to_main();
                }
            }
            MainSectionStep(delta) => {
                let len = menu::MainSection::ALL.len() as isize;
                self.menu.main_section =
                    ((self.menu.main_section as isize + delta as isize).rem_euclid(len)) as usize;
                self.coming_soon_notice = None;
            }
            MainSectionActivate(index) => {
                let len = menu::MainSection::ALL.len();
                self.menu.main_section = index % len;
                let section = menu::MainSection::from_index(self.menu.main_section);
                if let Some(detail) = section.coming_soon_detail() {
                    self.coming_soon_notice =
                        Some((detail.into(), Instant::now() + Duration::from_secs(4)));
                } else {
                    self.coming_soon_notice = None;
                    match section {
                        menu::MainSection::Career => {
                            self.career_create_return_dealership = None;
                            self.pending_reset_career = false;
                            let populated_slots =
                                self.career.slots.iter().filter(|s| s.is_some()).count();
                            if populated_slots > 1 || self.career.pending_legacy_profile.is_some() {
                                self.menu.open(menu::Page::CareerSlots);
                            } else if self.career.profile.is_some() {
                                self.menu.open(menu::Page::CareerHub);
                            } else {
                                self.menu.open(menu::Page::CareerCreate);
                            }
                        }
                        menu::MainSection::TimeTrial => self.menu.open(menu::Page::Drive),
                        menu::MainSection::Garage => {
                            if let Some(pos) = self
                                .content
                                .vehicles
                                .iter()
                                .position(|v| v.descriptor.id == self.selection.vehicle.id)
                            {
                                self.menu.inspected_vehicle = pos;
                            }
                            self.showroom_camera
                                .set_mode(crate::car::showroom::ShowroomMode::Garage);
                            self.menu.open(menu::Page::Garage);
                        }
                        menu::MainSection::NewCars => {
                            self.pending_buy_vehicle_id = None;
                            let first = {
                                let ctx = self.context();
                                self.menu
                                    .dealership_indices_for(&ctx, career::DealershipKind::NewSalon)
                                    .first()
                                    .copied()
                            };
                            if let Some(idx) = first {
                                self.menu.dealership_vehicle = idx;
                            }
                            self.showroom_camera
                                .set_mode(crate::car::showroom::ShowroomMode::Garage);
                            self.menu.open(menu::Page::NewDealership);
                        }
                        menu::MainSection::UsedCars => {
                            self.pending_buy_vehicle_id = None;
                            let first = {
                                let ctx = self.context();
                                self.menu
                                    .dealership_indices_for(&ctx, career::DealershipKind::UsedCars)
                                    .first()
                                    .copied()
                            };
                            if let Some(idx) = first {
                                self.menu.dealership_vehicle = idx;
                            }
                            self.showroom_camera
                                .set_mode(crate::car::showroom::ShowroomMode::Garage);
                            self.menu.open(menu::Page::Dealership);
                        }
                        menu::MainSection::RallySchool => {
                            let grades = self
                                .career
                                .profile
                                .as_ref()
                                .map_or_else(school::SchoolCatalog::load_results, |profile| {
                                    profile.school_progress.grades
                                });
                            self.school.set_results(grades);
                            self.menu.open(menu::Page::RallySchool);
                        }
                        menu::MainSection::Settings => self.menu.open(menu::Page::Settings),
                        _ => {}
                    }
                }
            }
            GaragePrevCar | GarageNextCar => {
                let indices = {
                    let ctx = self.context();
                    self.menu.garage_indices(&ctx)
                };
                if !indices.is_empty() {
                    let pos = indices
                        .iter()
                        .position(|&i| i == self.menu.inspected_vehicle)
                        .unwrap_or(0);
                    let delta = if matches!(command, GarageNextCar) {
                        1isize
                    } else {
                        -1isize
                    };
                    let next = ((pos as isize + delta).rem_euclid(indices.len() as isize)) as usize;
                    self.menu.inspected_vehicle = indices[next];
                    self.message = None;
                }
            }
            GarageFilterCycle => {
                if self.content.is_public_beta() {
                    return;
                }
                self.menu.garage_filter = self.menu.garage_filter.next();
                let indices = {
                    let ctx = self.context();
                    self.menu.garage_indices(&ctx)
                };
                if !indices.contains(&self.menu.inspected_vehicle) {
                    if let Some(&first) = indices.first() {
                        self.menu.inspected_vehicle = first;
                    }
                }
            }
            Open(page) => {
                if page == menu::Page::NgpExperimental {
                    if self.content.is_public_beta() {
                        return;
                    }
                    self.refresh_focus_diagnostics();
                } else if page == menu::Page::Garage {
                    if let Some(pos) = self
                        .content
                        .vehicles
                        .iter()
                        .position(|v| v.descriptor.id == self.selection.vehicle.id)
                    {
                        self.menu.inspected_vehicle = pos;
                    }
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::Garage);
                } else if let Some(kind) = page.dealership_kind() {
                    self.pending_buy_vehicle_id = None;
                    let first = {
                        let ctx = self.context();
                        self.menu
                            .dealership_indices_for(&ctx, kind)
                            .first()
                            .copied()
                    };
                    if let Some(idx) = first {
                        self.menu.dealership_vehicle = idx;
                    }
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::Garage);
                } else if page == menu::Page::CareerGarage {
                    self.menu.career_garage_cursor = 0;
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::Garage);
                } else if page == menu::Page::CareerCreate {
                    self.career_create_return_dealership =
                        self.menu.page.dealership_kind().map(|_| self.menu.page);
                }
                self.coming_soon_notice = None;
                self.menu.open(page);
            }
            Back => {
                self.pending_buy_vehicle_id = None;
                self.menu.back();
                if self.menu.page == menu::Page::Main {
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::MainMenu);
                } else if matches!(
                    self.menu.page,
                    menu::Page::Garage | menu::Page::Dealership | menu::Page::CareerGarage
                ) {
                    self.showroom_camera
                        .set_mode(crate::car::showroom::ShowroomMode::Garage);
                }
            }
            ResetToMain => {
                self.pause();
                self.capture_session_snapshot(false);
                self.worker.shutdown();
                self.worker = worker::Worker::idle();
                self.clear_scene_pose();
                self.race = race::Clock::default();
                self.codriver = None;
                audio::set_codriver_active(false);
                audio::clear_speech_queue();
                self.replay_player = None;
                self.replay_recorder.clear();
                self.saved_replay = None;
                self.active_school_lesson = None;
                self.active_school_result = None;
                self.school_incident_banner = None;
                self.active_career_stage = false;
                self.last_career_stage_outcome = None;
                self.pending_buy_vehicle_id = None;
                self.split_tracker.reset();
                self.last_finish_splits_ms.clear();
                self.last_run_prev_pb_splits_ms = None;
                self.last_record_outcome = None;
                self.showroom_drag = None;
                self.coming_soon_notice = None;
                self.showroom_camera
                    .set_mode(crate::car::showroom::ShowroomMode::MainMenu);
                self.pending_scene_action = Some(SceneAction::Unload);
                self.menu.reset_to_main();
                self.message = None;
                audio::set_menu_music_active(true);
            }
            StartRally => {
                if self.menu.page != menu::Page::CareerChampionship {
                    self.active_career_stage = false;
                    self.last_career_stage_outcome = None;
                }
                self.content =
                    Catalog::scan_for_app(&self.selection, self.content.is_public_beta());
                if self.content.is_public_beta()
                    && self.content.vehicle(&self.selection.vehicle.id).is_none()
                {
                    self.message = Some("COCHE NO DISPONIBLE EN LA BETA".into());
                    return;
                }
                if self.selection.vehicle.is_ngp_experimental_car() {
                    if let Some(entry) = self.content.vehicle(&self.selection.vehicle.id) {
                        if !entry.using_fallback_model && entry.descriptor != self.selection.vehicle
                        {
                            self.selection.vehicle = entry.descriptor.clone();
                        }
                    }
                }
                let result = self
                    .selection
                    .validate_for_mode(self.ngp_experimental)
                    .and_then(|()| {
                        if self.race.phase != race::Phase::Idle {
                            return Err("RALLY ALREADY LOADING OR ACTIVE".into());
                        }
                        if !stage_supported(&self.selection.stage) {
                            return Err("STAGE UNAVAILABLE IN THIS MODE".into());
                        }
                        if let Some(entry) = self.content.stage(&self.selection.stage.id) {
                            if let Some(missing) = entry.files.missing_label() {
                                return Err(missing);
                            }
                        }
                        Ok(())
                    });
                if let Err(error) = result {
                    self.message = Some(error);
                } else if self.selection.vehicle.is_ngp_experimental_car() {
                    self.refresh_diagnostics_for_selected();
                    let can_visual = self
                        .focus_diagnostics
                        .as_ref()
                        .is_some_and(|r| r.can_enter_visual_mode());
                    if !can_visual {
                        let reason = self
                            .focus_diagnostics
                            .as_ref()
                            .and_then(|r| r.boot_blocker_reason())
                            .unwrap_or_else(|| "REVISA EL DIAGNÓSTICO".into());
                        self.message = Some(format!("NGP BLOQUEADO ({reason})"));
                        if !self.content.is_public_beta() {
                            self.menu.open(menu::Page::NgpExperimental);
                        }
                    } else {
                        let pack_path = self
                            .focus_diagnostics
                            .as_ref()
                            .and_then(|r| r.can_boot_ngp_pack().cloned());
                        let blocker_reason = self
                            .focus_diagnostics
                            .as_ref()
                            .and_then(|r| r.boot_blocker_reason());
                        if self.content.is_public_beta() && pack_path.is_none() {
                            self.message =
                                blocker_reason.or_else(|| Some("FÍSICA NGP NO DISPONIBLE".into()));
                            return;
                        }
                        audio::set_menu_music_active(false);
                        audio::set_paused(true);
                        audio::set_codriver_active(false);
                        audio::clear_speech_queue();
                        self.codriver = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(|doc| {
                            let doc_path = std::path::PathBuf::from(&doc);
                            let maps = doc_path.join(&self.selection.stage.documents_maps);
                            codriver::CoDriverSession::load_for_stage(
                                &doc_path,
                                &maps,
                                &self.selection.stage.stem,
                            )
                        });
                        self.capture_session_snapshot(false);
                        self.worker.shutdown();
                        self.clear_scene_pose();
                        self.loading_started = Instant::now();
                        self.race.prepare();
                        self.replay_player = None;
                        self.replay_recorder.clear();
                        self.active_school_lesson = None;
                        self.active_school_result = None;
                        self.school_incident_banner = None;
                        self.last_record_outcome = None;
                        self.split_tracker.reset();
                        self.last_finish_splits_ms.clear();
                        self.last_run_prev_pb_splits_ms = None;
                        self.last_finish_crossed_forward = false;
                        if let Some(pack) = pack_path {
                            self.last_driven_vehicle_id = Some(self.selection.vehicle.id.clone());
                            self.experimental_visual_only = false;
                            self.experimental_physics_reason = None;
                            eprintln!(
                                "experimental-ngp BOOT: {} pack={}",
                                self.selection.vehicle.id,
                                pack.display()
                            );
                            self.worker = worker::Worker::new_experimental_pack_with_overrides(
                                self.selection.clone(),
                                pack,
                                false,
                                self.clutch_help,
                                self.career_boot_overrides(),
                            );
                        } else {
                            let reason = blocker_reason
                                .unwrap_or_else(|| "PAQUETE NGP O BACKEND NO DISPONIBLE".into());
                            eprintln!(
                                "experimental-ngp VISUAL-ONLY: {} reason={reason}",
                                self.selection.vehicle.id
                            );
                            self.experimental_visual_only = true;
                            self.experimental_physics_reason = Some(reason);
                            self.worker = worker::Worker::idle();
                        }
                        self.pending_scene_action = Some(SceneAction::Load {
                            selection: Box::new(self.selection.clone()),
                            experimental_ngp: true,
                        });
                        self.menu.show(menu::Page::Loading);
                        self.message = None;
                    }
                } else {
                    audio::set_menu_music_active(false);
                    audio::set_paused(true);
                    audio::set_codriver_active(false);
                    audio::clear_speech_queue();
                    self.codriver = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(|doc| {
                        let doc_path = std::path::PathBuf::from(&doc);
                        let maps = doc_path.join(&self.selection.stage.documents_maps);
                        codriver::CoDriverSession::load_for_stage(
                            &doc_path,
                            &maps,
                            &self.selection.stage.stem,
                        )
                    });
                    self.capture_session_snapshot(false);
                    self.worker.shutdown();
                    self.clear_scene_pose();
                    self.loading_started = Instant::now();
                    self.race.prepare();
                    self.replay_player = None;
                    self.replay_recorder.clear();
                    self.active_school_lesson = None;
                    self.active_school_result = None;
                    self.school_incident_banner = None;
                    self.last_record_outcome = None;
                    self.split_tracker.reset();
                    self.last_finish_splits_ms.clear();
                    self.last_run_prev_pb_splits_ms = None;
                    self.last_finish_crossed_forward = false;
                    self.last_driven_vehicle_id = Some(self.selection.vehicle.id.clone());
                    self.worker = worker::Worker::new_selected_with_overrides(
                        self.selection.clone(),
                        false,
                        self.clutch_help,
                        self.career_boot_overrides(),
                    );
                    self.pending_scene_action = Some(SceneAction::Load {
                        selection: Box::new(self.selection.clone()),
                        experimental_ngp: false,
                    });
                    self.menu.show(menu::Page::Loading);
                    self.message = None;
                }
            }
            Start => {
                let state = self.worker.state.lock().unwrap();
                let worker_ok = self.experimental_visual_only
                    || (state.error.is_none() && state.rest.is_some());
                if !matches!(
                    self.race.phase,
                    race::Phase::Finishing | race::Phase::Finished
                ) && self.pivots.is_some()
                    && (self.experimental_visual_only || self.is_landscape_ready())
                    && self.scene_error.is_none()
                    && worker_ok
                {
                    self.controls.clear();
                    self.menu.resume();
                    self.message = None;
                } else {
                    self.message = Some("START BLOCKED: ASSETS OR LANDSCAPE NOT READY".into());
                }
            }
            Restart => {
                if self.content.is_public_beta() {
                    self.content =
                        Catalog::scan_for_app(&self.selection, self.content.is_public_beta());
                    if self.content.vehicle(&self.selection.vehicle.id).is_none() {
                        self.message = Some("COCHE NO DISPONIBLE EN LA BETA".into());
                        return;
                    }
                }
                self.pause();
                self.capture_session_snapshot(false);
                self.worker.shutdown();
                self.active_school_result = None;
                self.school_incident_banner = None;
                self.last_record_outcome = None;
                self.split_tracker.reset();
                self.last_finish_splits_ms.clear();
                self.last_run_prev_pb_splits_ms = None;
                self.last_finish_crossed_forward = false;
                if self.selection.vehicle.is_ngp_experimental_car() {
                    self.refresh_diagnostics_for_selected();
                    if let Some(pack) = self
                        .focus_diagnostics
                        .as_ref()
                        .and_then(|r| r.can_boot_ngp_pack().cloned())
                    {
                        self.experimental_visual_only = false;
                        self.experimental_physics_reason = None;
                        self.worker = worker::Worker::new_experimental_pack_with_overrides(
                            self.selection.clone(),
                            pack,
                            false,
                            self.clutch_help,
                            self.career_boot_overrides(),
                        );
                    } else {
                        let reason = self
                            .focus_diagnostics
                            .as_ref()
                            .and_then(|r| r.boot_blocker_reason())
                            .unwrap_or_else(|| "PAQUETE NGP O BACKEND NO DISPONIBLE".into());
                        if self.content.is_public_beta() {
                            self.message = Some(reason);
                            return;
                        }
                        self.experimental_visual_only = true;
                        self.experimental_physics_reason = Some(reason);
                        self.worker = worker::Worker::idle();
                    }
                } else if let Some(lesson_idx) = self.active_school_lesson {
                    self.experimental_visual_only = false;
                    self.worker = worker::Worker::new_school_lesson(
                        self.selection.clone(),
                        lesson_idx,
                        false,
                        self.clutch_help,
                    );
                } else {
                    self.experimental_visual_only = false;
                    self.worker = worker::Worker::new_selected_with_overrides(
                        self.selection.clone(),
                        false,
                        self.clutch_help,
                        self.career_boot_overrides(),
                    );
                }
                self.offset = None;
                self.camera = None;
                self.scene_error = None;
                self.controls.clear();
                self.controls.selected_gear = 1;
                self.loading_started = Instant::now();
                self.race.prepare();
                audio::set_codriver_active(false);
                if let Some(codriver) = &mut self.codriver {
                    codriver.reset();
                } else {
                    audio::clear_speech_queue();
                }
                self.replay_player = None;
                self.replay_recorder.clear();
                self.menu.show(menu::Page::Loading);
                self.message = None;
            }
            StartReplay => {
                if let Some(replay) = &self.saved_replay {
                    audio::set_codriver_active(false);
                    audio::clear_speech_queue();
                    if self.selection.vehicle.is_ngp_experimental_car() {
                        if let Some(doc) = std::env::var_os("RERBR_PROBE_DOCUMENTS") {
                            audio::init_rsf_fmod(
                                std::path::Path::new(&doc),
                                &self.selection.vehicle,
                                44100,
                            );
                        }
                    }
                    let mut player = replay::ReplayPlayer::with_pivots(
                        replay.clone(),
                        self.stage_cameras.clone(),
                        self.pivots,
                    );
                    player.restart();
                    self.replay_player = Some(player);
                    self.menu.show(menu::Page::Replay);
                    self.message = None;
                    audio::set_menu_music_active(false);
                }
            }
            ReplayTogglePause => {
                if let Some(player) = &mut self.replay_player {
                    player.toggle_pause();
                }
            }
            ReplayRestart => {
                if let Some(player) = &mut self.replay_player {
                    player.restart();
                }
            }
            ReplayCycleCamera => {
                if let Some(player) = &mut self.replay_player {
                    player.cycle_camera();
                    self.message = Some(format!("CÁMARA: {}", player.camera_mode.label()));
                }
            }
            ReplayExit => {
                self.replay_player = None;
                audio::set_paused(true);
                audio::set_menu_music_active(true);
                if self.active_school_result.is_some() {
                    self.menu.show(menu::Page::SchoolResult);
                } else {
                    self.menu.show(menu::Page::StageComplete);
                }
            }
            NextGaragePage => {
                let total = self.content.browse_vehicle_count().div_ceil(9);
                if total > 0 {
                    self.menu.garage_page = (self.menu.garage_page + 1) % total;
                }
            }
            PrevGaragePage => {
                let total = self.content.browse_vehicle_count().div_ceil(9);
                if total > 0 {
                    self.menu.garage_page = (self.menu.garage_page + total - 1) % total;
                }
            }
            NextStagePage => {
                let total = self.content.stages.len().div_ceil(8);
                if total > 0 {
                    self.menu.stage_page = (self.menu.stage_page + 1) % total;
                }
            }
            PrevStagePage => {
                let total = self.content.stages.len().div_ceil(8);
                if total > 0 {
                    self.menu.stage_page = (self.menu.stage_page + total - 1) % total;
                }
            }
            Calibrate => {
                if self.tilt.is_finite() {
                    self.controls.centre = self.tilt;
                    self.controls.clear();
                    self.message = Some("STEERING CENTRED AT CURRENT ANGLE".into());
                } else {
                    self.message = Some("CALIBRATION NEEDS A LANDSCAPE HOLD".into());
                }
            }
            ResetCalibration => {
                self.controls.centre = 0.0;
                self.controls.clear();
                self.message = Some("STEERING CENTRE RESET".into());
            }
            SensDown => {
                self.controls.sensitivity = (self.controls.sensitivity - 0.25).max(0.25);
                self.message = Some(format!("SENSITIVITY {:.2}", self.controls.sensitivity));
            }
            SensUp => {
                self.controls.sensitivity = (self.controls.sensitivity + 0.25).min(5.0);
                self.message = Some(format!("SENSITIVITY {:.2}", self.controls.sensitivity));
            }
            DeadDown => {
                self.controls.dead_zone = (self.controls.dead_zone - 0.01).max(0.0);
                self.message = Some(format!("DEAD ZONE {:.2}", self.controls.dead_zone));
            }
            DeadUp => {
                self.controls.dead_zone = (self.controls.dead_zone + 0.01).min(0.2);
                self.message = Some(format!("DEAD ZONE {:.2}", self.controls.dead_zone));
            }
            CameraNext => {
                #[cfg(feature = "physics-stage")]
                {
                    self.view = self.view.next();
                    self.camera = None;
                }
                self.message = Some(format!("CAMERA {}", self.camera_label()));
            }
            VolumeDown => {
                self.volume = (self.volume - 0.1).max(0.0);
                audio::set_master_volume(self.volume as f32);
                self.message = Some(format!(
                    "VOLUMEN MOTOR {:.0}% ({})",
                    self.volume * 100.0,
                    audio::status()
                ));
            }
            VolumeUp => {
                self.volume = (self.volume + 0.1).min(1.0);
                audio::set_master_volume(self.volume as f32);
                self.message = Some(format!(
                    "VOLUMEN MOTOR {:.0}% ({})",
                    self.volume * 100.0,
                    audio::status()
                ));
            }
            MusicVolumeDown => {
                self.music_volume = (self.music_volume - 0.1).max(0.0);
                audio::set_music_volume(self.music_volume as f32);
                self.message = Some(format!(
                    "VOLUMEN MÚSICA {:.0}% ({})",
                    self.music_volume * 100.0,
                    audio::menu_music_status()
                ));
            }
            MusicVolumeUp => {
                self.music_volume = (self.music_volume + 0.1).min(1.0);
                audio::set_music_volume(self.music_volume as f32);
                self.message = Some(format!(
                    "VOLUMEN MÚSICA {:.0}% ({})",
                    self.music_volume * 100.0,
                    audio::menu_music_status()
                ));
            }
            CodriverVolumeDown => {
                self.codriver_volume = (self.codriver_volume - 0.1).max(0.0);
                audio::set_codriver_volume(self.codriver_volume as f32);
                self.message = Some(format!(
                    "VOLUMEN COPILOTO {:.0}% ({})",
                    self.codriver_volume * 100.0,
                    audio::codriver_status()
                ));
            }
            CodriverVolumeUp => {
                self.codriver_volume = (self.codriver_volume + 0.1).min(1.0);
                audio::set_codriver_volume(self.codriver_volume as f32);
                self.message = Some(format!(
                    "VOLUMEN COPILOTO {:.0}% ({})",
                    self.codriver_volume * 100.0,
                    audio::codriver_status()
                ));
            }
            ToggleFps => {
                self.fps_visible = !self.fps_visible;
                self.message = Some(format!(
                    "FPS COUNTER {}",
                    if self.fps_visible { "ON" } else { "OFF" }
                ));
            }
            ToggleDiagnostics => {
                self.diagnostics = !self.diagnostics;
                self.message = Some(format!(
                    "ADVANCED TELEMETRY {}",
                    if self.diagnostics { "ON" } else { "OFF" }
                ));
            }
            ToggleMinimalHud => {
                self.minimal_hud = !self.minimal_hud;
                self.message = Some(format!(
                    "MINIMAL HUD {}",
                    if self.minimal_hud { "ON" } else { "OFF" }
                ));
            }
            CycleHudRpmStyle => {
                self.hud_rpm_style = self.hud_rpm_style.next();
                self.message = Some(format!("ESTILO HUD RPM: {}", self.hud_rpm_style.label()));
            }
            ToggleInvertPedals => {
                self.controls.invert_pedals = !self.controls.invert_pedals;
                self.message = Some(format!(
                    "INVERT PEDALS {}",
                    if self.controls.invert_pedals {
                        "ON"
                    } else {
                        "OFF"
                    }
                ));
            }
            ToggleClutchHelp => {
                self.clutch_help = !self.clutch_help;
                self.message = Some(format!(
                    "AYUDA EMBRAGUE {} (APLICA AL INICIAR)",
                    if self.clutch_help { "ON" } else { "OFF" }
                ));
            }
            InspectContent => {
                self.content =
                    Catalog::scan_for_app(&self.selection, self.content.is_public_beta());
                self.message = Some(self.content.summary());
            }
            CheckPhysics => {
                let (passed, report) = content_manager::physics_report(&self.selection);
                self.message = Some(if passed {
                    format!("PASS: {report}")
                } else {
                    format!("FAIL: {report}")
                });
            }
            ToggleExperimentalMode => {
                if self.content.is_public_beta() {
                    return;
                }
                if self.ngp_experimental
                    && self.selection.vehicle.is_ngp_experimental_car()
                    && self.race.phase != race::Phase::Idle
                {
                    self.message = Some("VUELVE AL MENÚ PRINCIPAL PARA DESACTIVAR MODO NGP".into());
                } else {
                    self.ngp_experimental = !self.ngp_experimental;
                    if !self.ngp_experimental && self.selection.vehicle.is_ngp_experimental_car() {
                        self.selection.vehicle =
                            rerbr_content::VehicleDescriptor::toyota_corolla_wrc();
                        let _ = self.controls.configure_vehicle(&self.selection.vehicle);
                    }
                    if self.ngp_experimental {
                        self.refresh_focus_diagnostics();
                    }
                    self.message = Some(if self.ngp_experimental {
                        "MODO NGP EXPERIMENTAL ACTIVADO (NO CERTIFICADO)".into()
                    } else {
                        "MODO RBR ORIGINAL ACTIVADO".into()
                    });
                }
            }
            RunFocusDiagnostics => {
                if self.content.is_public_beta() {
                    return;
                }
                self.refresh_focus_diagnostics();
                self.menu.open(menu::Page::NgpExperimental);
            }
            StartFocusVisualInspection => {
                if self.content.is_public_beta() {
                    return;
                }
                if self.race.phase != race::Phase::Idle {
                    self.message = Some("VUELVE AL MENÚ PRINCIPAL ANTES DE PROBAR".into());
                } else {
                    self.ngp_experimental = true;
                    let inspected = self
                        .content
                        .vehicles
                        .get(self.menu.inspected_vehicle)
                        .map(|e| &e.descriptor)
                        .filter(|v| v.is_ngp_experimental_car())
                        .cloned();
                    self.selection.vehicle =
                        inspected.unwrap_or_else(rerbr_content::VehicleDescriptor::ford_focus_2006);
                    let _ = self.controls.configure_vehicle(&self.selection.vehicle);
                    self.apply_command(StartRally);
                }
            }
            OpenVehicle(index) => {
                if self.content.is_public_beta() && self.content.vehicles.get(index).is_none() {
                    return;
                }
                self.menu.inspected_vehicle = index;
                self.menu.open(menu::Page::GarageDetail);
            }
            SelectVehicle(index) => {
                let candidate = match self.content.vehicles.get(index) {
                    None => Err("UNKNOWN VEHICLE".to_string()),
                    Some(entry)
                        if entry.classification
                            == rerbr_content::VehicleClassification::VisualInspectionOnly =>
                    {
                        Err("SOLO VISUALIZACIÓN 3D (SIN FÍSICA NGP INTEGRADA)".into())
                    }
                    Some(entry) if entry.using_fallback_model || entry.model_missing => {
                        Err(format!(
                            "MODELO 3D PROPIO AUSENTE PARA {} (SIN FALLBACK)",
                            entry.descriptor.name
                        ))
                    }
                    Some(entry) if self.content.inspected && !entry.files.missing.is_empty() => {
                        Err(entry
                            .files
                            .missing_label()
                            .unwrap_or_else(|| "MISSING VEHICLE FILES".into()))
                    }
                    Some(entry) => Ok(entry.descriptor.clone()),
                };
                let result = candidate.and_then(|vehicle| {
                    self.configure_selection(rerbr_content::Selection {
                        vehicle,
                        stage: self.selection.stage.clone(),
                    })
                });
                self.message = Some(match result {
                    Ok(()) => format!("COCHE SELECCIONADO: {}", self.selection.vehicle.name),
                    Err(error) => format!("COCHE RECHAZADO: {error}"),
                });
            }
            OpenStage(index) => {
                self.menu.inspected_stage = index;
                self.menu.open(menu::Page::StageDetail);
            }
            SelectStage(index) => {
                let result = match self.content.stages.get(index).cloned() {
                    None => Err("UNKNOWN STAGE".into()),
                    Some(entry) if entry.files.missing_label().is_some() => {
                        Err(entry.files.missing_label().unwrap())
                    }
                    Some(entry) => self.configure_selection(rerbr_content::Selection {
                        vehicle: self.selection.vehicle.clone(),
                        stage: entry.descriptor,
                    }),
                };
                self.message = Some(match result {
                    Ok(()) => format!("SELECTED STAGE: {}", self.selection.stage.name),
                    Err(error) => format!("STAGE REJECTED: {error}"),
                });
            }
            CamFovDown | CamFovUp | CamPosXDown | CamPosXUp | CamPosYDown | CamPosYUp
            | CamPosZDown | CamPosZUp | CamLookXDown | CamLookXUp | CamLookZDown | CamLookZUp
            | CamLookYDown | CamLookYUp | CamSave | CamReset => {
                #[cfg(feature = "physics-stage")]
                {
                    let vid = self.selection.vehicle.id.clone();
                    let ov = self
                        .camera_overrides
                        .for_vehicle_mut(&vid)
                        .for_view_mut(self.view);
                    match command {
                        CamFovDown => ov.fov_degrees -= 2.0,
                        CamFovUp => ov.fov_degrees += 2.0,
                        CamPosXDown => ov.pos_offset.x -= 0.05,
                        CamPosXUp => ov.pos_offset.x += 0.05,
                        CamPosYDown => ov.pos_offset.y -= 0.05,
                        CamPosYUp => ov.pos_offset.y += 0.05,
                        CamPosZDown => ov.pos_offset.z -= 0.05,
                        CamPosZUp => ov.pos_offset.z += 0.05,
                        CamLookXDown => ov.target_offset.x -= 0.05,
                        CamLookXUp => ov.target_offset.x += 0.05,
                        CamLookZDown => ov.target_offset.z -= 0.05,
                        CamLookZUp => ov.target_offset.z += 0.05,
                        CamLookYDown => ov.target_offset.y -= 0.05,
                        CamLookYUp => ov.target_offset.y += 0.05,
                        CamReset => {
                            self.camera_overrides.reset_vehicle_view(&vid, self.view);
                            self.camera_overrides.save();
                        }
                        CamSave => self.camera_overrides.save(),
                        _ => {}
                    }
                    let sanitized = self
                        .camera_overrides
                        .for_vehicle(&vid)
                        .for_view(self.view)
                        .sanitized();
                    *self
                        .camera_overrides
                        .for_vehicle_mut(&vid)
                        .for_view_mut(self.view) = sanitized;
                    if !matches!(command, CamSave | CamReset) {
                        self.camera_overrides.save();
                    }
                }
            }
            CtrlNext => {
                self.selected_control =
                    layout::EditableControl::ALL[(self.selected_control as usize + 1) % 6];
            }
            CtrlSizeDown => {
                let base = DrivingLayout::for_size(self.drawable_size);
                self.controls_layout
                    .resize(self.selected_control, base, -0.02, -0.02);
            }
            CtrlSizeUp => {
                let base = DrivingLayout::for_size(self.drawable_size);
                self.controls_layout
                    .resize(self.selected_control, base, 0.02, 0.02);
            }
            CtrlOpacityDown => {
                self.controls_layout
                    .adjust_opacity(self.selected_control, -0.05);
            }
            CtrlOpacityUp => {
                self.controls_layout
                    .adjust_opacity(self.selected_control, 0.05);
            }
            GlobalOpacityDown => {
                self.controls_layout.adjust_global_opacity(-0.05);
            }
            GlobalOpacityUp => {
                self.controls_layout.adjust_global_opacity(0.05);
            }
            CtrlSave => {
                self.controls_layout.save();
                self.message = Some("DISTRIBUCIÓN DE CONTROLES GUARDADA".into());
            }
            CtrlReset => {
                self.controls_layout.reset();
                self.controls_layout.save();
                self.message = Some("CONTROLES RESTABLECIDOS".into());
            }
            RecoverCar => {
                if self.race.phase == race::Phase::Driving {
                    self.recover_car_frames = 4;
                    if self.menu.page == menu::Page::Paused {
                        self.controls.clear();
                        self.gamepad.clear_driving_edges();
                        self.menu.resume();
                    }
                    self.message = Some("RESCATE DE COCHE SOLICITADO".into());
                } else {
                    self.message = Some("RESCATE DISPONIBLE EN CONDUCCIÓN".into());
                }
            }
            GpDeadDown => {
                self.gamepad.settings.deadzone = (self.gamepad.settings.deadzone - 0.02).max(0.0);
            }
            GpDeadUp => {
                self.gamepad.settings.deadzone = (self.gamepad.settings.deadzone + 0.02).min(0.35);
            }
            GpSensDown => {
                self.gamepad.settings.sensitivity =
                    (self.gamepad.settings.sensitivity - 0.05).max(0.5);
            }
            GpSensUp => {
                self.gamepad.settings.sensitivity =
                    (self.gamepad.settings.sensitivity + 0.05).min(2.5);
            }
            GpCurveDown => {
                self.gamepad.settings.response_curve =
                    (self.gamepad.settings.response_curve - 0.1).max(1.0);
            }
            GpCurveUp => {
                self.gamepad.settings.response_curve =
                    (self.gamepad.settings.response_curve + 0.1).min(2.5);
            }
            GpInvertSteer => {
                self.gamepad.settings.invert_steering = !self.gamepad.settings.invert_steering;
            }
            GpInvertTriggers => {
                self.gamepad.settings.invert_triggers = !self.gamepad.settings.invert_triggers;
            }
            GpBindGearUp => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::GearUp);
            }
            GpBindGearDown => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::GearDown);
            }
            GpBindHandbrake => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::Handbrake);
            }
            GpBindClutch => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::Clutch);
            }
            GpBindCamera => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::Camera);
            }
            GpBindRecover => {
                self.gamepad
                    .settings
                    .cycle_binding(gamepad::GamepadAction::Recover);
            }
            GpSave => {
                self.gamepad.settings.save();
                self.message = Some("PERFIL DE MANDO GUARDADO".into());
            }
            GpReset => {
                self.gamepad.settings = gamepad::GamepadSettings::default();
                self.gamepad.settings.save();
                self.message = Some("MANDO RESTABLECIDO".into());
            }
            PnToggle => {
                self.pacenote_hud.enabled = !self.pacenote_hud.enabled;
            }
            PnScaleDown => {
                self.pacenote_hud.scale = (self.pacenote_hud.scale - 0.1).max(0.5);
            }
            PnScaleUp => {
                self.pacenote_hud.scale = (self.pacenote_hud.scale + 0.1).min(2.0);
            }
            PnPosXDown => {
                self.pacenote_hud.pos_x = (self.pacenote_hud.pos_x - 0.05).max(0.10);
            }
            PnPosXUp => {
                self.pacenote_hud.pos_x = (self.pacenote_hud.pos_x + 0.05).min(0.90);
            }
            PnPosYDown => {
                self.pacenote_hud.pos_y = (self.pacenote_hud.pos_y - 0.04).max(0.06);
            }
            PnPosYUp => {
                self.pacenote_hud.pos_y = (self.pacenote_hud.pos_y + 0.04).min(0.55);
            }
            PnAnticipationDown => {
                self.pacenote_hud.anticipation_s =
                    (self.pacenote_hud.anticipation_s - 0.5).max(1.5);
            }
            PnAnticipationUp => {
                self.pacenote_hud.anticipation_s =
                    (self.pacenote_hud.anticipation_s + 0.5).min(8.0);
            }
            PnToggleDebug => {
                self.pacenote_hud.debug_sprites = !self.pacenote_hud.debug_sprites;
            }
            PnSave => {
                self.pacenote_hud.save();
                self.message = Some("AJUSTES DE NOTAS GUARDADOS".into());
            }
            PnReset => {
                self.pacenote_hud = pacenote_icons::PacenoteHudSettings::default();
                self.pacenote_hud.save();
                self.message = Some("NOTAS VISUALES RESTABLECIDAS".into());
            }
            ChampToggleActive => {
                self.championship.progress.active = !self.championship.progress.active;
                self.championship.progress.save();
            }
            ChampCycleSkill => {
                self.championship.progress.skill = self.championship.progress.skill.next();
                self.championship.progress.stage_index = 0;
                self.championship.progress.save();
            }
            ChampNextRally => {
                let len = self.championship.rallies.len();
                if len > 0 {
                    self.championship.progress.rally_index =
                        (self.championship.progress.rally_index + 1) % len;
                    self.championship.progress.stage_index = 0;
                    self.championship.progress.save();
                }
            }
            ChampStartStage => {
                if let Some(stage_desc) = self.championship.current_stage_descriptor() {
                    self.championship.progress.active = true;
                    self.championship.progress.save();
                    if let Some(entry) = self.content.stage(&stage_desc.id) {
                        if let Some(missing) = entry.files.missing_label() {
                            self.message = Some(missing);
                            return;
                        }
                    }
                    let next_sel = rerbr_content::Selection {
                        vehicle: self.selection.vehicle.clone(),
                        stage: stage_desc,
                    };
                    match self.configure_selection(next_sel) {
                        Ok(()) => self.apply_command(StartRally),
                        Err(e) => self.message = Some(format!("CAMPEONATO BLOQUEADO: {e}")),
                    }
                } else {
                    self.message = Some("TRAMO DEL CAMPEONATO NO INSTALADO EN DOCUMENTS".into());
                }
            }
            ChampReset => {
                self.championship.progress = championship::ChampionshipProgress::default();
                self.championship.progress.save();
                self.message = Some("CAMPEONATO REINICIADO".into());
            }
            SchoolPrevLesson => {
                let len = self.school.entries.len();
                if len > 0 {
                    self.school.selected = (self.school.selected + len - 1) % len;
                }
            }
            SchoolNextLesson => {
                let len = self.school.entries.len();
                if len > 0 {
                    self.school.selected = (self.school.selected + 1) % len;
                }
            }
            SchoolInspectLesson => {
                if let Some(entry) = self.school.entries.get(self.school.selected) {
                    self.message = Some(if entry.assets_present {
                        format!(
                            "{} ({}) · {}",
                            entry.display_title(),
                            entry.stem,
                            entry.grade_label()
                        )
                    } else {
                        format!(
                            "{}: FALTA {}",
                            entry.display_title(),
                            entry
                                .missing_files
                                .first()
                                .map_or("ARCHIVOS DE LECCIÓN", String::as_str)
                        )
                    });
                }
            }
            SchoolStartLesson => {
                let Some(entry) = self.school.entries.get(self.school.selected).cloned() else {
                    return;
                };
                if !entry.assets_present {
                    self.message = Some(format!(
                        "{}: FALTA {}",
                        entry.display_title(),
                        entry
                            .missing_files
                            .first()
                            .map_or("ARCHIVOS DE LECCIÓN", String::as_str)
                    ));
                    return;
                }
                if self.race.phase != race::Phase::Idle {
                    self.message = Some("RALLY ALREADY LOADING OR ACTIVE".into());
                    return;
                }
                let Some(impreza) = rerbr_content::VehicleDescriptor::original_rbr_presets()
                    .into_iter()
                    .find(|v| v.original_slot == 5)
                else {
                    self.message = Some("FALTA PRESET SUBARU IMPREZA 2003 (CAR05)".into());
                    return;
                };
                match rerbr_content::StageDescriptor::school_lesson(
                    entry.map,
                    entry.stem.clone(),
                    "RBR/Maps",
                    1,
                ) {
                    Ok(stage) => {
                        let next_sel = rerbr_content::Selection {
                            vehicle: impreza,
                            stage,
                        };
                        if let Err(err) = self.configure_selection(next_sel) {
                            self.message = Some(format!("ESCUELA BLOQUEADA: {err}"));
                            return;
                        }
                        self.capture_session_snapshot(false);
                        audio::set_menu_music_active(false);
                        audio::set_paused(true);
                        audio::set_codriver_active(false);
                        audio::clear_speech_queue();
                        self.codriver = None;
                        self.capture_session_snapshot(false);
                        self.worker.shutdown();
                        self.clear_scene_pose();
                        self.loading_started = Instant::now();
                        self.race.prepare();
                        self.replay_player = None;
                        self.replay_recorder.clear();
                        self.active_school_lesson = None;
                        self.active_school_result = None;
                        self.school_incident_banner = None;
                        self.active_school_lesson = Some(entry.lesson.index());
                        self.active_school_result = None;
                        self.school_incident_banner = None;
                        self.split_tracker.reset();
                        self.last_finish_splits_ms.clear();
                        self.last_run_prev_pb_splits_ms = None;
                        self.experimental_visual_only = false;
                        self.worker = worker::Worker::new_school_lesson(
                            self.selection.clone(),
                            entry.lesson.index(),
                            false,
                            self.clutch_help,
                        );
                        self.pending_scene_action = Some(SceneAction::Load {
                            selection: Box::new(self.selection.clone()),
                            experimental_ngp: false,
                        });
                        self.menu.show(menu::Page::Loading);
                        self.message = None;
                    }
                    Err(err) => {
                        self.message = Some(format!("ESCUELA BLOQUEADA: {err}"));
                    }
                }
            }
            SchoolConfirmStart => {
                if self.active_school_lesson.is_none() {
                    return;
                }
                let worker_ok = {
                    let mut st = self.worker.state.lock().unwrap();
                    let ok = st.error.is_none() && st.rest.is_some();
                    if ok {
                        st.school_start_requested = true;
                        st.school_events.clear();
                    }
                    ok
                };
                if worker_ok && self.pivots.is_some() && self.scene_error.is_none() {
                    self.controls.clear();
                    self.replay_recorder.clear();
                    self.school_incident_banner = None;
                    self.active_school_result = None;
                    self.race.start_school_driving(Instant::now());
                    self.menu.resume();
                    self.message = None;
                } else {
                    self.message = Some("ESPERANDO INICIALIZACIÓN DE LA LECCIÓN".into());
                }
            }
            SchoolFinishNow => {
                if self.active_school_lesson.is_some() {
                    let st = self.worker.state.lock().unwrap().school_state.clone();
                    if let Some(mut st) = st.filter(|st| st.started || st.ended.is_some()) {
                        st.ended
                            .get_or_insert(rerbr_sim::drive::SchoolOutcome::Finished);
                        self.finalize_school_completion(&st);
                    }
                }
            }
            SchoolRetryLesson => {
                if self.active_school_lesson.is_some() {
                    self.apply_command(Restart);
                }
            }
            SchoolNextStartLesson => {
                let next_idx = self
                    .active_school_result
                    .as_ref()
                    .map(|r| r.lesson_index as usize + 1)
                    .or_else(|| self.active_school_lesson.map(|i| i as usize + 1))
                    .unwrap_or(self.school.selected + 1);
                self.apply_command(ResetToMain);
                if next_idx < self.school.entries.len() {
                    self.school.selected = next_idx;
                    self.apply_command(SchoolStartLesson);
                }
            }
            ExportDiagnostics => {
                self.refresh_focus_diagnostics();
                let doc = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(std::path::PathBuf::from);
                match (doc.as_deref(), self.focus_diagnostics.as_ref()) {
                    (Some(dir), Some(report)) => {
                        let extra = vec![
                            format!(
                                "SELECCION MENU ACTUAL: {} / {} ({})",
                                self.selection.vehicle.id,
                                self.selection.vehicle.model,
                                self.selection.stage.id
                            ),
                            format!(
                                "OPCIONES: clutch_help={} ngp_experimental={} visual_only={}",
                                self.clutch_help,
                                self.ngp_experimental,
                                self.experimental_visual_only
                            ),
                            format!(
                                "ESTADO FÍSICA NGP: {}",
                                self.experimental_physics_reason
                                    .as_deref()
                                    .unwrap_or("SIN SESIÓN EXPERIMENTAL ACTIVA")
                            ),
                            format!("AUDIO STATUS: {}", audio::status()),
                            format!("MUSIC STATUS: {}", audio::menu_music_status()),
                            format!("CODRIVER STATUS: {}", audio::codriver_status()),
                            format!(
                                "ESCUELA RALLY: {} ({})",
                                self.school.status,
                                self.school.audio.status.label()
                            ),
                        ];
                        match report.write_export_file(dir, &extra) {
                            Ok(path) => {
                                let name = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("rerbr-diagnostico.txt");
                                self.message =
                                    Some(format!("DIAGNÓSTICO GUARDADO: Documents/{name}"));
                            }
                            Err(e) => {
                                self.message = Some(format!("ERROR EXPORTANDO DIAGNÓSTICO: {e}"));
                            }
                        }
                    }
                    (None, _) => {
                        self.message = Some("CARPETA DOCUMENTS NO DISPONIBLE".into());
                    }
                    (_, None) => {
                        self.message = Some("SIN REPORTE DE DIAGNÓSTICO".into());
                    }
                }
            }
            CareerPilotPrev | CareerPilotNext => {
                let len = career::PILOT_NAME_PRESETS.len();
                let delta = if matches!(command, CareerPilotNext) {
                    1isize
                } else {
                    -1isize
                };
                self.pilot_preset_index =
                    ((self.pilot_preset_index as isize + delta).rem_euclid(len as isize)) as usize;
                self.refresh_pilot_draft_name();
            }
            CareerPilotTagNext => {
                self.pilot_tag_index = (self.pilot_tag_index + 1) % 6;
                self.refresh_pilot_draft_name();
            }
            CareerPilotEditKeyboard => {
                prompt_pilot_name_ios(&self.pilot_draft_name);
                self.message = Some("INTRODUCE EL NOMBRE EN EL TECLADO".into());
            }
            CareerNumberDown => {
                self.pilot_draft_number = if self.pilot_draft_number <= 1 {
                    99
                } else {
                    self.pilot_draft_number - 1
                };
            }
            CareerNumberUp => {
                self.pilot_draft_number = if self.pilot_draft_number >= 99 {
                    1
                } else {
                    self.pilot_draft_number + 1
                };
            }
            CareerCreateConfirm => match self
                .career
                .create_profile_with_number(&self.pilot_draft_name, self.pilot_draft_number)
            {
                Ok(()) => {
                    self.message = Some(format!(
                        "PERFIL #{} {} CREADO · SALDO {}",
                        self.pilot_draft_number,
                        self.pilot_draft_name,
                        career::format_usd(career::INITIAL_CASH_USD)
                    ));
                    self.menu.reset_to_main();
                    if let Some(return_page) = self.career_create_return_dealership.take() {
                        self.showroom_camera
                            .set_mode(crate::car::showroom::ShowroomMode::Garage);
                        self.menu.open(return_page);
                    } else {
                        self.menu.open(menu::Page::CareerHub);
                    }
                }
                Err(err) => {
                    self.message = Some(err);
                }
            },
            CareerSlotSelect(slot_idx) => {
                self.pending_reset_career = false;
                self.pending_buy_vehicle_id = None;
                self.pending_sell_vehicle_id = None;
                self.career.select_slot(slot_idx);
                if self.career.profile.is_some() {
                    self.menu.open(menu::Page::CareerHub);
                } else {
                    self.menu.open(menu::Page::CareerCreate);
                }
            }
            CareerSlotImportLegacy => match self.career.import_pending_legacy_into_active_slot() {
                Ok(()) => {
                    self.message = Some(format!(
                        "GUARDADO ANTERIOR IMPORTADO AL ESPACIO {}",
                        self.career.active_slot + 1
                    ));
                    self.menu.open(menu::Page::CareerHub);
                }
                Err(err) => {
                    self.message = Some(err);
                }
            },
            CareerLoadDevMode => {
                #[cfg(feature = "career-dev-mode")]
                {
                    if let Some(root) =
                        std::env::var_os("RERBR_PROBE_DOCUMENTS").map(std::path::PathBuf::from)
                    {
                        let dev = career::CareerStore::load_dev_profile_from_root(&root);
                        self.career.profile = Some(dev);
                        self.message = Some("MODO DESARROLLADOR QA ACTIVO (AISLADO)".into());
                        self.menu.open(menu::Page::CareerHub);
                    } else {
                        self.career.profile = Some(career::CareerProfile::create_dev_profile());
                        self.message = Some("MODO DESARROLLADOR QA ACTIVO (MEMORIA)".into());
                        self.menu.open(menu::Page::CareerHub);
                    }
                }
                #[cfg(not(feature = "career-dev-mode"))]
                {
                    self.message = Some("MODO DESARROLLADOR NO DISPONIBLE EN RELEASE".into());
                }
            }
            CareerDevResetCash => {
                #[cfg(feature = "career-dev-mode")]
                {
                    let res = match self.career.profile.as_mut() {
                        Some(p) => p.reset_dev_cash(),
                        None => Err("SIN PERFIL QA ACTIVO".into()),
                    };
                    match res {
                        Ok(()) => {
                            self.save_active_career();
                            self.message = Some("SALDO QA RESTABLECIDO A US$9,999,999,999".into());
                        }
                        Err(err) => self.message = Some(err),
                    }
                }
                #[cfg(not(feature = "career-dev-mode"))]
                {
                    self.message = Some("MODO DESARROLLADOR NO DISPONIBLE EN RELEASE".into());
                }
            }
            CareerDevToggleUnlocks => {
                #[cfg(feature = "career-dev-mode")]
                {
                    let res = match self.career.profile.as_mut() {
                        Some(p) => p.toggle_dev_unlocks(),
                        None => Err("SIN PERFIL QA ACTIVO".into()),
                    };
                    match res {
                        Ok(unlocked) => {
                            self.save_active_career();
                            self.message = Some(if unlocked {
                                "DESBLOQUEOS QA ACTIVADOS (HISTÓRICOS Y CATÁLOGO)".into()
                            } else {
                                "DESBLOQUEOS QA RESTABLECIDOS A PROGRESIÓN NORMAL".into()
                            });
                        }
                        Err(err) => self.message = Some(err),
                    }
                }
                #[cfg(not(feature = "career-dev-mode"))]
                {
                    self.message = Some("MODO DESARROLLADOR NO DISPONIBLE EN RELEASE".into());
                }
            }
            CareerResetRequest => {
                self.pending_reset_career = true;
                self.message = Some(format!(
                    "PULSA CONFIRMAR PARA REINICIAR ESPACIO {}",
                    self.career.active_slot + 1
                ));
            }
            CareerResetCancel => {
                self.pending_reset_career = false;
                self.message = None;
            }
            CareerResetConfirm => {
                let slot = self.career.active_slot + 1;
                self.career.reset_active_slot();
                self.pending_reset_career = false;
                self.active_career_stage = false;
                self.last_career_stage_outcome = None;
                self.pending_buy_vehicle_id = None;
                self.pending_sell_vehicle_id = None;
                self.message = Some(format!("ESPACIO {slot} REINICIADO — CONFIGURA TU PILOTO"));
                self.menu.open(menu::Page::CareerCreate);
            }
            DealershipPrevCar | DealershipNextCar => {
                self.pending_buy_vehicle_id = None;
                let indices = {
                    let ctx = self.context();
                    self.menu.dealership_indices(&ctx)
                };
                if !indices.is_empty() {
                    let pos = indices
                        .iter()
                        .position(|&i| i == self.menu.dealership_vehicle)
                        .unwrap_or(0);
                    let delta = if matches!(command, DealershipNextCar) {
                        1isize
                    } else {
                        -1isize
                    };
                    let next = ((pos as isize + delta).rem_euclid(indices.len() as isize)) as usize;
                    self.menu.dealership_vehicle = indices[next];
                    self.message = None;
                }
            }
            DealershipFilterCycle => {
                self.pending_buy_vehicle_id = None;
                self.menu.dealership_filter = self.menu.dealership_filter.next();
                let indices = {
                    let ctx = self.context();
                    self.menu.dealership_indices(&ctx)
                };
                if !indices.contains(&self.menu.dealership_vehicle) {
                    if let Some(&first) = indices.first() {
                        self.menu.dealership_vehicle = first;
                    }
                }
            }
            DealershipRequestBuy => {
                let kind = self.menu.active_dealership_kind();
                let indices = {
                    let ctx = self.context();
                    self.menu.dealership_indices_for(&ctx, kind)
                };
                let active_idx = if indices.contains(&self.menu.dealership_vehicle) {
                    self.menu.dealership_vehicle
                } else {
                    let Some(&first) = indices.first() else {
                        return;
                    };
                    first
                };
                let Some(entry) = self.content.vehicles.get(active_idx) else {
                    return;
                };
                let Some(profile) = self.career.profile.as_ref() else {
                    self.career_create_return_dealership =
                        self.menu.page.dealership_kind().map(|_| self.menu.page);
                    self.menu.open(menu::Page::CareerCreate);
                    return;
                };
                let cap = entry.capability(self.ngp_experimental);
                let spec = career::vehicle_market_spec(entry);
                let Some(active_price) = spec.price_for(kind) else {
                    self.message = Some(format!(
                        "ESTE VEHÍCULO NO SE VENDE EN {}",
                        kind.short_name()
                    ));
                    return;
                };
                if cap != content_manager::DriveCapability::Drivable {
                    self.message = Some("SOLO VISUALIZACIÓN 3D — NO COMPRABLE SIN FÍSICA".into());
                } else if profile.owns_vehicle(&entry.descriptor.id) {
                    self.message = Some("YA TIENES ESTE COCHE EN TU GARAJE DE CARRERA".into());
                } else if !profile.can_afford(active_price) {
                    self.message = Some(format!(
                        "SALDO INSUFICIENTE (NECESITAS {})",
                        career::format_usd(active_price)
                    ));
                } else {
                    self.pending_buy_vehicle_id = Some(entry.descriptor.id.clone());
                    self.message = Some(format!(
                        "PULSA CONFIRMAR COMPRA PARA ADQUIRIR {} ({}) POR {}",
                        entry.descriptor.name,
                        kind.condition_label(),
                        career::format_usd(active_price)
                    ));
                }
            }
            DealershipConfirmBuy => {
                let Some(target_id) = self.pending_buy_vehicle_id.take() else {
                    return;
                };
                let Some(entry) = self.content.vehicle(&target_id).cloned() else {
                    self.message = Some("VEHÍCULO NO ENCONTRADO EN CATÁLOGO".into());
                    return;
                };
                let kind = self.menu.active_dealership_kind();
                let ngp_exp = self.ngp_experimental;
                let res = match self.career.profile.as_mut() {
                    Some(profile) => profile.buy_vehicle_in_dealership(&entry, kind, ngp_exp),
                    None => Err("CREA UN PERFIL DE CARRERA PRIMERO".into()),
                };
                match res {
                    Ok(paid) => {
                        if let Err(err) = self.career.save() {
                            self.message = Some(format!("ERROR GUARDANDO CARRERA: {err}"));
                        } else {
                            let rem = self.career.profile.as_ref().map_or(0, |p| p.cash_usd);
                            self.message = Some(format!(
                                "COMPRADO {} ({}) POR {} · SALDO {}",
                                entry.descriptor.name,
                                kind.condition_label(),
                                career::format_usd(paid),
                                career::format_usd(rem)
                            ));
                        }
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            DealershipCancelBuy => {
                self.pending_buy_vehicle_id = None;
                self.message = Some("COMPRA CANCELADA".into());
            }
            CareerGaragePrevCar | CareerGarageNextCar => {
                let indices = {
                    let ctx = self.context();
                    self.menu.career_garage_indices(&ctx)
                };
                if !indices.is_empty() {
                    let delta = if matches!(command, CareerGarageNextCar) {
                        1isize
                    } else {
                        -1isize
                    };
                    self.menu.career_garage_cursor =
                        ((self.menu.career_garage_cursor as isize + delta)
                            .rem_euclid(indices.len() as isize)) as usize;
                    self.message = None;
                }
            }
            CareerGarageSelectActive => {
                self.pending_sell_vehicle_id = None;
                let indices = {
                    let ctx = self.context();
                    self.menu.career_garage_indices(&ctx)
                };
                let cursor = self
                    .menu
                    .career_garage_cursor
                    .min(indices.len().saturating_sub(1));
                let Some(&veh_idx) = indices.get(cursor) else {
                    self.menu.open(menu::Page::Dealership);
                    return;
                };
                let Some(entry) = self.content.vehicles.get(veh_idx).cloned() else {
                    return;
                };
                let res = match self.career.profile.as_mut() {
                    Some(profile) => profile.select_career_vehicle(&entry.descriptor.id),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(()) => {
                        let _ = self.career.save();
                        self.message = Some(format!(
                            "COCHE DE CARRERA ACTIVO: {}",
                            entry.descriptor.name
                        ));
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerGarageRequestSell => {
                let indices = {
                    let ctx = self.context();
                    self.menu.career_garage_indices(&ctx)
                };
                let cursor = self
                    .menu
                    .career_garage_cursor
                    .min(indices.len().saturating_sub(1));
                if let Some(entry) = indices
                    .get(cursor)
                    .and_then(|&idx| self.content.vehicles.get(idx))
                {
                    let id = entry.descriptor.id.clone();
                    let resale = self
                        .career
                        .profile
                        .as_ref()
                        .map_or(0, |p| p.resale_value_usd(entry));
                    self.pending_sell_vehicle_id = Some(id);
                    self.message = Some(format!(
                        "CONFIRMA VENTA DE {} POR {}",
                        entry.descriptor.name,
                        career::format_usd(resale)
                    ));
                }
            }
            CareerGarageCancelSell => {
                self.pending_sell_vehicle_id = None;
                self.message = None;
            }
            CareerGarageConfirmSell => {
                let Some(target_id) = self.pending_sell_vehicle_id.take() else {
                    return;
                };
                let Some(entry) = self.content.vehicle(&target_id).cloned() else {
                    self.message = Some("VEHÍCULO NO ENCONTRADO".into());
                    return;
                };
                let res = match self.career.profile.as_mut() {
                    Some(profile) => profile.sell_vehicle(&entry),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(resale) => {
                        let _ = self.career.save();
                        self.menu.career_garage_cursor = 0;
                        self.message = Some(format!(
                            "VENDIDO: {} (+{})",
                            entry.descriptor.name,
                            career::format_usd(resale)
                        ));
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerDiffPrev => {
                self.selected_career_difficulty = self.selected_career_difficulty.prev();
            }
            CareerDiffNext => {
                self.selected_career_difficulty = self.selected_career_difficulty.next();
            }
            CareerChampKindPrev | CareerChampKindNext => {
                self.selected_championship_kind = match self.selected_championship_kind {
                    career::ChampionshipKind::Regional => career::ChampionshipKind::Historic,
                    career::ChampionshipKind::Historic => career::ChampionshipKind::Regional,
                };
            }
            CareerChampStart => {
                let ngp_exp = self.ngp_experimental;
                let diff = self.selected_career_difficulty;
                let kind = self.selected_championship_kind;
                let is_eligible = |e: &content_manager::VehicleEntry| {
                    let tier = career::vehicle_market_spec(e).tier;
                    match kind {
                        career::ChampionshipKind::Regional => tier.eligible_for_regional(),
                        career::ChampionshipKind::Historic => tier.eligible_for_historic(),
                    }
                };
                let candidate = self
                    .career
                    .profile
                    .as_ref()
                    .and_then(|p| {
                        p.active_career_vehicle_id
                            .as_deref()
                            .and_then(|id| self.content.vehicle(id))
                            .filter(|e| {
                                e.capability(ngp_exp) == content_manager::DriveCapability::Drivable
                                    && is_eligible(e)
                            })
                            .or_else(|| {
                                self.content.vehicles.iter().find(|e| {
                                    p.owns_vehicle(&e.descriptor.id)
                                        && e.capability(ngp_exp)
                                            == content_manager::DriveCapability::Drivable
                                        && is_eligible(e)
                                })
                            })
                    })
                    .cloned();
                let Some(car_entry) = candidate else {
                    self.message = Some(match kind {
                        career::ChampionshipKind::Regional => {
                            "COMPRA UN COCHE DE INICIACIÓN (EJ. MG ZR S1600) EN 2ª MANO".into()
                        }
                        career::ChampionshipKind::Historic => {
                            "COMPRA UN COCHE HISTÓRICO / GRUPO A / KIT CAR EN 2ª MANO".into()
                        }
                    });
                    return;
                };
                let res = match self.career.profile.as_mut() {
                    Some(p) => p.start_championship(kind, &car_entry, ngp_exp, diff),
                    None => Err("CREA UN PERFIL DE CARRERA PRIMERO".into()),
                };
                match res {
                    Ok(()) => {
                        self.save_active_career();
                        self.message = Some(format!(
                            "{} INICIADO ({} · {})",
                            kind.title(),
                            car_entry.descriptor.name,
                            diff.label()
                        ));
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerChampRunNextStage => {
                let Some(champ) = self
                    .career
                    .profile
                    .as_ref()
                    .and_then(|p| p.active_championship.clone())
                else {
                    self.message = Some("NO HAY CAMPEONATO ACTIVO".into());
                    return;
                };
                let Some(stage_idx) = champ.next_stage_index() else {
                    self.message = Some("EL CAMPEONATO YA HA FINALIZADO".into());
                    return;
                };
                let Some(stage_desc) = career::championship_stage_descriptor(champ.kind, stage_idx)
                else {
                    self.message = Some("ETAPA DEL CAMPEONATO NO ENCONTRADA".into());
                    return;
                };
                let Some(car_entry) = self.content.vehicle(&champ.locked_vehicle_id).cloned()
                else {
                    self.message = Some("COCHE BLOQUEADO DEL CAMPEONATO NO INSTALADO".into());
                    return;
                };
                if car_entry.capability(self.ngp_experimental)
                    != content_manager::DriveCapability::Drivable
                {
                    self.message =
                        Some("EL COCHE DEL CAMPEONATO NO TIENE FÍSICA DISPONIBLE".into());
                    return;
                }
                if let Some(st_entry) = self.content.stage(&stage_desc.id) {
                    if let Some(missing) = st_entry.files.missing_label() {
                        self.message = Some(missing);
                        return;
                    }
                }
                let next_sel = rerbr_content::Selection {
                    vehicle: car_entry.descriptor,
                    stage: stage_desc,
                };
                match self.configure_selection(next_sel) {
                    Ok(()) => {
                        self.active_career_stage = true;
                        self.last_career_stage_outcome = None;
                        self.apply_command(StartRally);
                    }
                    Err(e) => {
                        self.active_career_stage = false;
                        self.message = Some(format!("COPA REGIONAL BLOQUEADA: {e}"));
                    }
                }
            }
            CareerChampAbandon => {
                let res = match self.career.profile.as_mut() {
                    Some(p) => p.abandon_championship(),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(()) => {
                        self.active_career_stage = false;
                        self.last_career_stage_outcome = None;
                        let _ = self.career.save();
                        self.message = Some("CAMPEONATO ABANDONADO (SIN PREMIO)".into());
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerChampRestoreAndDismiss => {
                let res = match self.career.profile.as_mut() {
                    Some(p) => p
                        .settle_championship_restoration(true)
                        .and_then(|cost| p.dismiss_finished_championship().map(|()| cost)),
                    None => Ok(0),
                };
                match res {
                    Ok(cost) => {
                        self.active_career_stage = false;
                        self.last_career_stage_outcome = None;
                        self.save_active_career();
                        self.menu.reset_to_main();
                        self.menu.open(menu::Page::CareerHub);
                        self.message = Some(format!(
                            "CAMPEONATO COMPLETADO · COCHE RESTAURADO ({})",
                            career::format_usd(cost)
                        ));
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerChampDismissFinished => {
                let res = match self.career.profile.as_mut() {
                    Some(p) => p
                        .settle_championship_restoration(false)
                        .and_then(|_| p.dismiss_finished_championship()),
                    None => Ok(()),
                };
                match res {
                    Ok(()) => {
                        self.active_career_stage = false;
                        self.last_career_stage_outcome = None;
                        self.save_active_career();
                        self.menu.reset_to_main();
                        self.menu.open(menu::Page::CareerHub);
                        self.message = Some("CAMPEONATO COMPLETADO Y GUARDADO".into());
                    }
                    Err(err) => {
                        self.message = Some(err);
                    }
                }
            }
            CareerServiceRepair(comp) => {
                let target_id = self.career.profile.as_ref().and_then(|p| {
                    p.active_championship
                        .as_ref()
                        .filter(|c| !c.finished)
                        .map(|c| c.locked_vehicle_id.clone())
                        .or_else(|| p.active_career_vehicle_id.clone())
                });
                let Some(entry) = target_id.and_then(|id| self.content.vehicle(&id)).cloned()
                else {
                    self.message = Some("SIN COCHE ACTIVO PARA ASISTENCIA".into());
                    return;
                };
                let res = match self.career.profile.as_mut() {
                    Some(p) => p.repair_component(&entry, comp),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(cost) => {
                        self.save_active_career();
                        self.message = Some(format!(
                            "REPARADO: {} (-{})",
                            comp.label(),
                            career::format_usd(cost)
                        ));
                    }
                    Err(err) => self.message = Some(err),
                }
            }
            CareerServiceRepairAll => {
                let target_id = self.career.profile.as_ref().and_then(|p| {
                    p.active_championship
                        .as_ref()
                        .filter(|c| !c.finished)
                        .map(|c| c.locked_vehicle_id.clone())
                        .or_else(|| p.active_career_vehicle_id.clone())
                });
                let Some(entry) = target_id.and_then(|id| self.content.vehicle(&id)).cloned()
                else {
                    self.message = Some("SIN COCHE ACTIVO PARA ASISTENCIA".into());
                    return;
                };
                let res = match self.career.profile.as_mut() {
                    Some(p) => p.repair_all_components(&entry),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(cost) => {
                        self.save_active_career();
                        self.message = Some(format!(
                            "ASISTENCIA COMPLETA (-{})",
                            career::format_usd(cost)
                        ));
                    }
                    Err(err) => self.message = Some(err),
                }
            }
            CareerServiceTyrePrev => {
                self.selected_service_tyre = self.selected_service_tyre.prev();
            }
            CareerServiceTyreNext => {
                self.selected_service_tyre = self.selected_service_tyre.next();
            }
            CareerServiceMountTyre => {
                let target_id = self.career.profile.as_ref().and_then(|p| {
                    p.active_championship
                        .as_ref()
                        .filter(|c| !c.finished)
                        .map(|c| c.locked_vehicle_id.clone())
                        .or_else(|| p.active_career_vehicle_id.clone())
                });
                let Some(entry) = target_id.and_then(|id| self.content.vehicle(&id)).cloned()
                else {
                    self.message = Some("SIN COCHE ACTIVO PARA CAMBIAR NEUMÁTICOS".into());
                    return;
                };
                let tyre = self.selected_service_tyre;
                let res = match self.career.profile.as_mut() {
                    Some(p) => p.buy_and_mount_tyres(&entry, tyre),
                    None => Err("SIN PERFIL DE CARRERA".into()),
                };
                match res {
                    Ok(cost) => {
                        self.save_active_career();
                        self.message = Some(format!(
                            "MONTADOS {} (-{})",
                            tyre.label(),
                            career::format_usd(cost)
                        ));
                    }
                    Err(err) => self.message = Some(err),
                }
            }
            OpenCareerAfterStage => {
                self.apply_command(ResetToMain);
                self.menu.open(menu::Page::CareerHub);
                self.menu.open(menu::Page::CareerChampionship);
            }
            ScrollRowsUp => {
                let total = {
                    let ctx = self.context();
                    self.menu.rows(&ctx).len()
                };
                self.menu.scroll_rows(total, -1);
            }
            ScrollRowsDown => {
                let total = {
                    let ctx = self.context();
                    self.menu.rows(&ctx).len()
                };
                self.menu.scroll_rows(total, 1);
            }
            SettingsCardStep(delta) => {
                let len = menu::SettingsCard::ALL.len() as isize;
                self.menu.settings_card =
                    ((self.menu.settings_card as isize + delta as isize).rem_euclid(len)) as usize;
                self.menu.focused_row = self.menu.settings_card;
            }
            SettingsCardActivate(index) => {
                let len = menu::SettingsCard::ALL.len();
                self.menu.settings_card = index % len;
                let card = menu::SettingsCard::from_index(self.menu.settings_card);
                self.apply_command(Open(card.target_page()));
            }
        }
    }
    fn refresh_pilot_draft_name(&mut self) {
        let base =
            career::PILOT_NAME_PRESETS[self.pilot_preset_index % career::PILOT_NAME_PRESETS.len()];
        let suffix = match self.pilot_tag_index % 6 {
            0 => "",
            1 => " #01",
            2 => " #07",
            3 => " #24",
            4 => " #88",
            _ => " PRO",
        };
        self.pilot_draft_name = format!("{base}{suffix}");
    }
    pub fn touch(&mut self, t: Touch, size: (u32, u32)) {
        self.drawable_size = size;
        if t.phase == TouchPhase::Started {
            self.menu.gamepad_focus_active = false;
        }
        let x = t.location.x / f64::from(size.0.max(1));
        let y = t.location.y / f64::from(size.1.max(1));
        if self.menu.page == menu::Page::ControlsEditor {
            let base = DrivingLayout::for_size(size);
            if t.phase == TouchPhase::Started {
                let rows = {
                    let context = self.context();
                    self.menu.rows(&context)
                };
                if let Some(command) = self.menu.press(x, y, &rows) {
                    self.menu_command(command);
                    return;
                }
                let effective = self.controls_layout.apply(base);
                for ctrl in layout::EditableControl::ALL.into_iter().rev() {
                    if effective.control_rect(ctrl).contains(x, y) {
                        self.selected_control = ctrl;
                        self.dragging_control = Some((
                            t.id,
                            ctrl,
                            x as f32 - effective.control_rect(ctrl).center_x(),
                            y as f32
                                - (effective.control_rect(ctrl).y
                                    + effective.control_rect(ctrl).h * 0.5),
                        ));
                        return;
                    }
                }
            } else if t.phase == TouchPhase::Moved {
                if let Some((id, ctrl, dx, dy)) = self.dragging_control {
                    if id != t.id {
                        return;
                    }
                    self.controls_layout
                        .set_center(ctrl, base, x as f32 - dx, y as f32 - dy);
                }
            } else if self.dragging_control.is_some_and(|(id, ..)| id == t.id) {
                self.dragging_control = None;
            }
            return;
        }
        if self.menu.page == menu::Page::Replay {
            if t.phase == TouchPhase::Started {
                let layout = layout::ReplayLayout::for_size(size);
                if layout.play_pause.contains(x, y) {
                    self.menu_command(menu::Command::ReplayTogglePause);
                } else if layout.restart.contains(x, y) {
                    self.menu_command(menu::Command::ReplayRestart);
                } else if layout.camera.contains(x, y) {
                    self.menu_command(menu::Command::ReplayCycleCamera);
                } else if layout.exit.contains(x, y) {
                    self.menu_command(menu::Command::ReplayExit);
                }
            }
            return;
        }
        if self.menu.page != menu::Page::Driving {
            if self.menu.page.is_showroom_page() {
                match t.phase {
                    TouchPhase::Started => {
                        if self.menu.is_showroom_orbit_zone(x, y) {
                            self.showroom_drag = Some((t.id, x as f32, y as f32));
                            return;
                        }
                        let cmd = {
                            let context = self.context();
                            let rows = self.menu.rows(&context);
                            self.menu.press_with_ctx(x, y, &context, &rows)
                        };
                        if let Some(command) = cmd {
                            self.menu_command(command);
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some((id, lx, ly)) = self.showroom_drag {
                            if id == t.id {
                                let (xf, yf) = (x as f32, y as f32);
                                self.showroom_camera.drag(xf - lx, yf - ly);
                                self.showroom_drag = Some((id, xf, yf));
                            }
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if self.showroom_drag.is_some_and(|(id, ..)| id == t.id) {
                            self.showroom_camera.end_drag();
                            self.showroom_drag = None;
                        }
                    }
                }
                return;
            }
            if t.phase == TouchPhase::Started {
                let rows = {
                    let context = self.context();
                    self.menu.rows(&context)
                };
                if let Some(command) = self.menu.press(x, y, &rows) {
                    self.menu_command(command);
                }
            }
            return;
        }
        let layout = self.effective_layout(size);
        if t.phase == TouchPhase::Started {
            if layout.pause.contains(x, y) {
                self.pause();
                return;
            }
            if layout.camera.contains(x, y) {
                self.menu_command(menu::Command::CameraNext);
                return;
            }
            if layout.diag.contains(x, y) {
                self.menu_command(menu::Command::ToggleDiagnostics);
                return;
            }
            if layout.fps.contains(x, y) {
                self.menu_command(menu::Command::ToggleFps);
                return;
            }
        }
        self.controls.touch_layout(
            &layout,
            t.id,
            x,
            y,
            t.phase == TouchPhase::Started,
            matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
        );
    }
    fn finalize_stage_completion(&mut self) {
        self.capture_session_snapshot(false);
        self.race.complete_finish();
        if let Some(res) = self.race.result() {
            if self.championship.progress.active {
                self.championship.record_stage_finish(
                    res.driving_seconds,
                    res.penalty_seconds,
                    res.false_start,
                );
            }
            let key = self.active_record_key(&self.selection.stage);
            self.last_run_prev_pb_splits_ms = self
                .records
                .get(&key)
                .filter(|pb| !pb.splits_ms.is_empty())
                .map(|pb| pb.splits_ms.clone());
            let effective_splits_passed = if self.split_tracker.has_order_violation() {
                0
            } else {
                res.splits_passed
            };
            let submission = records::RunSubmission {
                key,
                car_name: self.selection.vehicle.name.clone(),
                stage_name: self.selection.stage.name.clone(),
                driving_ms: res.driving_ms,
                penalty_ms: res.penalty_ms,
                false_start: res.false_start,
                splits_ms: self.last_finish_splits_ms.clone(),
                splits_passed: effective_splits_passed,
                splits_total: res.splits_total,
                finished: true,
                finish_crossed_forward: self.last_finish_crossed_forward,
                visual_only: self.experimental_visual_only,
                is_school: self.active_school_lesson.is_some(),
            };
            let outcome = self.records.submit(&submission);
            if outcome.valid {
                if let Err(err) = self.records.save() {
                    eprintln!("records SAVE ERROR: {err}");
                }
            }
            if self.active_career_stage {
                let stage_id = self.selection.stage.id.clone();
                let vehicle_id = self.selection.vehicle.id.clone();
                let vehicle_tier = self
                    .content
                    .vehicle(&vehicle_id)
                    .map(|e| career::vehicle_market_spec(e).tier)
                    .unwrap_or(career::CareerTier::Regional);
                let latest_damage = self.worker.state.lock().unwrap().latest_damage.clone();
                if let Some(profile) = self.career.profile.as_mut() {
                    match profile.record_championship_stage_result_with_telemetry(
                        &stage_id,
                        &vehicle_id,
                        res.total_ms,
                        outcome.valid,
                        latest_damage.as_ref(),
                        vehicle_tier,
                    ) {
                        Ok(career_out) => {
                            self.save_active_career();
                            self.last_career_stage_outcome = Some(career_out);
                        }
                        Err(err) => {
                            eprintln!("career STAGE RECORD REJECTED: {err}");
                            self.last_career_stage_outcome = None;
                        }
                    }
                }
            }
            self.last_record_outcome = Some(outcome);
        }
        self.saved_replay = self.replay_recorder.finish();
        self.controls.clear();
        self.menu.show(menu::Page::StageComplete);
        self.message = None;
        audio::set_codriver_active(false);
        audio::clear_speech_queue();
        audio::set_menu_music_active(true);
    }
    fn finalize_school_completion(&mut self, st: &rerbr_sim::drive::SchoolSessionSnapshot) {
        self.capture_session_snapshot(false);
        self.race.complete_school_finish();
        audio::set_paused(true);
        audio::clear_speech_queue();
        audio::set_codriver_active(true);
        let result = self.school.record_verdict_result(
            st,
            self.career
                .profile
                .as_mut()
                .map(|profile| &mut profile.school_progress),
        );
        self.saved_replay = self.replay_recorder.finish();
        self.controls.clear();
        self.active_school_result = Some(result);
        self.school_incident_banner = None;
        self.menu.show(menu::Page::SchoolResult);
        self.message = if self.career.profile.is_some() {
            self.career
                .save()
                .err()
                .map(|err| format!("ACADEMIA: ERROR AL GUARDAR · {err}"))
        } else {
            None
        };
        audio::set_menu_music_active(true);
    }
    fn sync_race_state(
        &mut self,
        now: Instant,
        seconds: f64,
        layout: &DrivingLayout,
        steering_ready: bool,
    ) -> (Option<(Sample, Sample, Instant)>, Option<Sample>, bool) {
        let mut state = self.worker.state.lock().unwrap();
        if self.selection.vehicle.is_ngp_experimental_car() && self.pivots.is_some() {
            if let Some(err) = state.error.take() {
                if let Some(diag) = &mut self.focus_diagnostics {
                    diag.record_physics_failure(&err);
                }
                let reason = format!("FALLO BACKEND NGP: {err}");
                eprintln!(
                    "experimental-ngp RUNTIME FAILURE: {} -> {reason}",
                    self.selection.vehicle.id
                );
                self.experimental_visual_only = true;
                self.experimental_physics_reason = Some(reason.clone());
                self.message = Some(format!("{reason} -> MODO VISUAL"));
            } else if !self.experimental_visual_only && state.rest.is_some() {
                self.experimental_physics_reason = Some("FÍSICA NGP CARGADA CORRECTAMENTE".into());
                if let Some(diag) = &mut self.focus_diagnostics {
                    diag.record_physics_ready();
                }
            }
        }
        let healthy = self.pivots.is_some()
            && self.scene_error.is_none()
            && (self.experimental_visual_only || (state.rest.is_some() && state.error.is_none()));
        if self.menu.page == menu::Page::Loading && healthy {
            if let Some(lesson_idx) = self.active_school_lesson {
                drop(state);
                audio::clear_speech_queue();
                audio::set_codriver_active(true);
                self.school.play_briefing_audio(lesson_idx as usize);
                self.menu.show(menu::Page::SchoolBriefing);
                self.message = None;
                state = self.worker.state.lock().unwrap();
            } else {
                self.race.start_presentation(now);
                self.menu.resume();
                self.message = None;
            }
        }
        let active = !self.experimental_visual_only
            && self.menu.page == menu::Page::Driving
            && steering_ready
            && healthy;
        self.race.tick(now, active);
        if active && state.timing.sim_elapsed_seconds > 0.0 {
            self.race
                .sync_simulation_time(state.timing.sim_elapsed_seconds);
        }
        if self.active_school_lesson.is_some() {
            let drained = std::mem::take(&mut state.school_events);
            let school_snapshot = state.school_state.clone();
            if self.menu.page == menu::Page::Driving {
                for ev in &drained {
                    if !ev.speech_ids.is_empty() {
                        self.school.play_event_speech(&ev.speech_ids);
                    }
                    if let Some(code) = ev.incident_code {
                        self.school_incident_banner = Some((
                            self.school.explain_incident(code),
                            now + Duration::from_secs(4),
                        ));
                    }
                }
                if let Some(st) = school_snapshot {
                    if st.ended.is_some() {
                        drop(state);
                        self.finalize_school_completion(&st);
                        state = self.worker.state.lock().unwrap();
                    }
                }
            }
        } else {
            if self.race.phase == race::Phase::Driving
                && !self.experimental_visual_only
                && state.timing.splits_total > 0
                && state.timing.splits_passed > self.split_tracker.recorded_ms().len()
            {
                let splits_passed = state.timing.splits_passed;
                let splits_total = state.timing.splits_total;
                let split_times = state.timing.split_times.clone();
                drop(state);
                let pb_splits = self.personal_best_splits_for_stage(&self.selection.stage);
                if let Some(banner) = self.split_tracker.sync(
                    now,
                    splits_passed,
                    splits_total,
                    &split_times,
                    self.race.raw_seconds(),
                    pb_splits.as_deref(),
                ) {
                    eprintln!(
                        "physics-stage SPLIT {}/{}: {}",
                        banner.split_number,
                        banner.splits_total,
                        banner.headline()
                    );
                }
                state = self.worker.state.lock().unwrap();
            }
            if self.race.phase == race::Phase::Driving
                && state.timing.finished
                && state.timing.finish_crossed_forward
            {
                let timing_splits: Vec<u64> = state
                    .timing
                    .split_times
                    .iter()
                    .filter_map(|t| {
                        t.filter(|v| v.is_finite() && *v > 0.0)
                            .map(|v| (v * 1000.0).round() as u64)
                    })
                    .collect();
                self.last_finish_splits_ms = if !timing_splits.is_empty() {
                    timing_splits
                } else {
                    self.split_tracker.recorded_ms().to_vec()
                };
                self.last_finish_crossed_forward = state.timing.finish_crossed_forward;
                self.race.begin_finish_rollout_with_sim_time(
                    now,
                    state.timing.splits_passed,
                    state.timing.splits_total,
                    state.timing.finish_distance,
                    state.timing.official_finish_seconds,
                );
            }
            if self
                .race
                .rollout_complete(now, state.timing.driveline_distance)
            {
                drop(state);
                self.finalize_stage_completion();
                state = self.worker.state.lock().unwrap();
            }
        }
        let running = self.menu.page == menu::Page::Driving && steering_ready && healthy;
        let driveline_dist = state.timing.driveline_distance;
        let speed_mps = state
            .pair
            .as_ref()
            .map_or(0.0, |(_, b, _)| Vec3::from(b.velocity).length());
        // Physics and engine revving are active during countdown, driving, and brief finish rollout;
        // paused during presentation, menus, and final completed state.
        state.paused = !running
            || !matches!(
                self.race.phase,
                race::Phase::Driving | race::Phase::Countdown | race::Phase::Finishing
            );
        if state.paused || self.race.phase == race::Phase::Finishing {
            self.controls.clear();
        }
        state.input = if self.race.phase == race::Phase::Finishing {
            rerbr_sim::drive::Control::default()
        } else {
            let mut input = self.controls.input_in(layout, self.tilt, seconds);
            if self.gamepad_frame.active {
                input.steering = self.gamepad_frame.steering;
                input.throttle = input.throttle.max(self.gamepad_frame.throttle);
                input.brake = input.brake.max(self.gamepad_frame.brake);
                input.handbrake = input.handbrake.max(self.gamepad_frame.handbrake);
                input.clutch = input.clutch.max(self.gamepad_frame.clutch);
            }
            if self.recover_car_frames > 0 {
                self.recover_car_frames -= 1;
                input.flags |= 0x10;
            }
            input
        };
        let codriver_active = running
            && matches!(
                self.race.phase,
                race::Phase::Driving | race::Phase::Finishing
            );
        audio::set_codriver_active(codriver_active || self.school_speech_allowed());
        if running && self.race.phase == race::Phase::Driving {
            if let Some(codriver) = &mut self.codriver {
                codriver.tick(driveline_dist, speed_mps);
            }
        }
        let ret = (state.pair.clone(), state.rest.clone(), state.paused);
        drop(state);
        if ret.1.is_some() {
            self.capture_session_snapshot(running);
        }
        ret
    }
    pub fn frame(&mut self, path: &ChasePath) -> Option<Frame> {
        let now = Instant::now();
        let seconds = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        if seconds > 0.0 {
            self.fps = self.fps * 0.9 + 0.1 / seconds;
        }
        self.tilt = motion(true);
        self.motion_polled_this_frame = true;
        self.poll_gamepad();
        self.gamepad_polled_this_frame = true;
        if now >= self.footprint_at {
            self.footprint = rerbr_sim::footprint();
            self.footprint_at = now + Duration::from_secs(1);
        }

        if self.menu.page == menu::Page::Replay {
            if let Some(player) = &mut self.replay_player {
                player.step(seconds as f32);
                let current_frame = player.current_frame()?;
                let is_paused = player.state != replay::PlaybackState::Playing;
                let car_pos = current_frame.car_position();
                let car_model = current_frame.models[0];
                let (camera, fov, fixed_listener) = player.current_camera(
                    car_pos,
                    car_model,
                    |m| {
                        let target = m.w_axis.truncate() + Vec3::Y * 0.75;
                        let forward = -m.z_axis.truncate().normalize_or(Vec3::NEG_Z);
                        let (cam, _) = path.chase_camera(target, forward).unwrap_or((
                            Camera {
                                eye: target - forward * 5.5 + Vec3::Y * 1.8,
                                target,
                                up: Vec3::Y,
                            },
                            0.0,
                        ));
                        cam
                    },
                    |m| {
                        #[cfg(feature = "physics-stage")]
                        if let Some(cam) = self.mounts.and_then(|mnt| {
                            mnt.camera_with_cockpit(cameras::View::Bonnet, m, self.cockpit_node)
                        }) {
                            return cam;
                        }
                        let eye = m.w_axis.truncate() + m.y_axis.truncate() * 0.9
                            - m.z_axis.truncate() * 0.5;
                        let target = eye - m.z_axis.truncate() * 10.0;
                        Camera {
                            eye,
                            target,
                            up: m.y_axis.truncate(),
                        }
                    },
                );
                let mix = player.audio_mix(camera, car_pos + Vec3::Y * 0.65, fixed_listener);
                audio::set_replay_audio(mix, (!is_paused).then_some(&current_frame));
                self.effective_fov = fov;
                return Some(Frame {
                    models: current_frame.models,
                    camera,
                    boom: 0.0,
                    max_wheel_travel: 0.0,
                    min_wheel_clearance: 0.0,
                    interior: false,
                });
            }
        }

        let layout = self.effective_layout(self.drawable_size);
        let steering_ready = self.is_landscape_ready();
        let (pair, rest, paused) = self.sync_race_state(now, seconds, &layout, steering_ready);
        audio::set_paused(paused);
        if self.experimental_visual_only {
            if let Some(pivots) = self.pivots {
                let frame = if let Ok(f) = path.frame(0.0) {
                    Some(f)
                } else if let Some((anchor, heading)) = self.visual_start_pose {
                    path.parked_frame(anchor, heading).ok()
                } else {
                    None
                };
                if let Some(mut f) = frame {
                    let car_forward = Vec3::new(-f.models[0].z_axis.x, 0.0, -f.models[0].z_axis.z)
                        .normalize_or(Vec3::NEG_Z);
                    if let Some(progress) = self.race.presentation_progress() {
                        let target = f.models[0].w_axis.truncate() + Vec3::Y * 0.5;
                        let cinematic =
                            cameras::presentation_camera(target, car_forward, progress, f.camera);
                        f.camera =
                            safe_presentation_camera(path, cinematic, f.camera, self.drawable_size);
                        f.interior = false;
                        self.effective_fov = 65.0;
                    } else {
                        #[cfg(feature = "physics-stage")]
                        if let Some(base) = self.mounts.and_then(|m| {
                            m.camera_with_cockpit(self.view, f.models[0], self.cockpit_node)
                        }) {
                            let ov = self
                                .camera_overrides
                                .for_vehicle(&self.selection.vehicle.id)
                                .for_view(self.view);
                            let edited = ov.apply(base, f.models[0]);
                            if path.camera_clear(edited, ov.fov_degrees, self.drawable_size) {
                                f.camera = edited;
                                f.interior = self.view == cameras::View::CockpitFallback
                                    && self.cockpit_ready;
                                self.effective_fov = ov.fov_degrees;
                            } else if path.camera_clear(base, 65.0, self.drawable_size) {
                                f.camera = base;
                                f.interior = self.view == cameras::View::CockpitFallback
                                    && self.cockpit_ready;
                                self.effective_fov = 65.0;
                            }
                        }
                    }
                    let _ = pivots;
                    return Some(f);
                }
            }
            return None;
        }
        let ((a, b, stamp), rest, pivots) = (pair?, rest?, self.pivots?);
        let swap = coords::physics_to_stage();
        let offset = *self.offset.get_or_insert_with(|| {
            (0..4)
                .map(|k| {
                    Vec3::from(rest.wheel_centre[k])
                        - swap.transform_point3(pivots[k] + self.wheel_visual_offsets[k])
                })
                .sum::<Vec3>()
                * 0.25
        });
        let alpha = if paused {
            1.0
        } else {
            (stamp.elapsed().as_secs_f64() * 720.0).clamp(0.0, 1.0) as f32
        };
        let models = models(&a, &b, alpha, pivots, offset);
        if matches!(
            self.race.phase,
            race::Phase::Driving | race::Phase::Finishing
        ) {
            let car_dist = self.worker.state.lock().unwrap().timing.driveline_distance;
            let speed_mps = Vec3::from(b.velocity).length();
            let replay_time =
                self.race.raw_seconds().unwrap_or(0.0) + self.race.rollout_elapsed_secs(now);
            self.replay_recorder.set_pivots(pivots);
            self.replay_recorder.record(
                replay_time as f32,
                models,
                car_dist,
                speed_mps,
                b.rpm,
                b.gear as i8,
            );
        }
        self.steering_angle = a.steering_angle + (b.steering_angle - a.steering_angle) * alpha;
        let position =
            swap.transform_point3(Vec3::from(a.position).lerp(Vec3::from(b.position), alpha));
        let car_forward =
            Vec3::new(-models[0].z_axis.x, 0.0, -models[0].z_axis.z).normalize_or(Vec3::NEG_Z);
        if self.race.phase == race::Phase::Countdown {
            self.race
                .record_start_origin(position.to_array(), car_forward.to_array());
            self.race.check_jump_start(now, position.to_array());
        }
        let forward = self.camera.map_or(car_forward, |c| {
            Vec3::new(c.target.x - c.eye.x, 0.0, c.target.z - c.eye.z)
                .normalize_or(car_forward)
                .lerp(car_forward, (1.0 - (-seconds / 0.1).exp()) as f32)
                .normalize_or(car_forward)
        });
        if !path.supports_driving(position) {
            audio::set_paused(true);
            {
                let mut s = self.worker.state.lock().unwrap();
                s.paused = true;
                if self.race.phase != race::Phase::Finishing {
                    s.error = Some(if cfg!(feature = "physics-stage") {
                        "OUTSIDE VERIFIED TERRAIN - RESET STAGE".into()
                    } else {
                        "VISIBLE TERRAIN LIMIT - RESET STAGE".into()
                    });
                }
            }
            if self.race.phase == race::Phase::Finishing {
                self.finalize_stage_completion();
            }
            return None;
        }
        let target = position + Vec3::Y * 0.5;
        let target = self.camera.map_or(target, |c| {
            c.target
                .lerp(target, (1.0 - (-seconds / 0.06).exp()) as f32)
        });
        #[cfg(feature = "physics-stage")]
        audio::set_view_is_cockpit(self.draws_interior());
        #[cfg(feature = "physics-stage")]
        if self.race.phase != race::Phase::Presentation {
            if let Some(base) = self
                .mounts
                .and_then(|m| m.camera_with_cockpit(self.view, models[0], self.cockpit_node))
            {
                let ov = self
                    .camera_overrides
                    .for_vehicle(&self.selection.vehicle.id)
                    .for_view(self.view);
                let edited = ov.apply(base, models[0]);
                let selected = if path.camera_clear(edited, ov.fov_degrees, self.drawable_size) {
                    Some((edited, ov.fov_degrees))
                } else if path.camera_clear(base, 65.0, self.drawable_size) {
                    Some((base, 65.0))
                } else {
                    None
                };
                if let Some((camera, fov)) = selected {
                    self.effective_fov = fov;
                    return Some(Frame {
                        models,
                        camera,
                        boom: 0.0,
                        max_wheel_travel: b
                            .wheel_travel
                            .iter()
                            .map(|x| x.abs())
                            .fold(0.0, f32::max),
                        min_wheel_clearance: 0.0,
                        interior: self.draws_interior(),
                    });
                }
                // Keep physics running and use the checked chase view if a mount is underground.
                self.view = cameras::View::Chase;
                self.camera = None;
                eprintln!(
                    "physics-stage CAMERA: mount underground/unsupported; returning to chase"
                );
            }
        }
        match path.chase_camera(target, forward) {
            Ok((camera, boom)) => {
                self.effective_fov = 65.0;
                // Cache the unedited chase camera, so target offsets cannot feed back.
                self.camera = Some(camera);
                #[cfg(feature = "physics-stage")]
                let camera = {
                    let ov = self
                        .camera_overrides
                        .for_vehicle(&self.selection.vehicle.id)
                        .for_view(cameras::View::Chase);
                    let edited = ov.apply(camera, models[0]);
                    if path.camera_clear(edited, ov.fov_degrees, self.drawable_size) {
                        self.effective_fov = ov.fov_degrees;
                        edited
                    } else {
                        camera
                    }
                };
                if let Some(progress) = self.race.presentation_progress() {
                    let cinematic =
                        cameras::presentation_camera(target, car_forward, progress, camera);
                    let cinematic =
                        safe_presentation_camera(path, cinematic, camera, self.drawable_size);
                    return Some(Frame {
                        models,
                        camera: cinematic,
                        boom,
                        max_wheel_travel: b
                            .wheel_travel
                            .iter()
                            .map(|x| x.abs())
                            .fold(0.0, f32::max),
                        min_wheel_clearance: 0.0,
                        interior: false,
                    });
                }
                Some(Frame {
                    models,
                    camera,
                    boom,
                    max_wheel_travel: b.wheel_travel.iter().map(|x| x.abs()).fold(0.0, f32::max),
                    min_wheel_clearance: 0.0,
                    interior: false,
                })
            }
            Err(e) => {
                audio::set_paused(true);
                let mut s = self.worker.state.lock().unwrap();
                s.paused = true;
                s.error = Some(format!("TERRAIN CAMERA: {e}; RESET"));
                None
            }
        }
    }
    pub fn submitted(&mut self, cpu: Duration) {
        self.cpu_ms = cpu.as_secs_f64() * 1000.0;
    }
    /// Interior cockpit geometry is valid only when rendering from inside an active cockpit view.
    /// Exterior presentation cinematics, chase/bonnet cameras, and exterior replays must always
    /// render complete exterior bodywork, bumpers, and all four wheels.
    pub fn draws_interior(&self) -> bool {
        if self.menu.page == menu::Page::Replay {
            return false;
        }
        #[cfg(feature = "physics-stage")]
        {
            self.race.phase != race::Phase::Presentation
                && self.view == cameras::View::CockpitFallback
                && self.cockpit_ready
                && self.mounts.is_some()
        }
        #[cfg(not(feature = "physics-stage"))]
        {
            false
        }
    }
    fn context(&self) -> menu::Context<'_> {
        #[cfg(feature = "physics-stage")]
        let cam_override = self
            .camera_overrides
            .for_vehicle(&self.selection.vehicle.id)
            .for_view(self.view);
        #[cfg(not(feature = "physics-stage"))]
        let cam_override = cameras::CameraOverride::default_for(cameras::View::Chase);
        menu::Context {
            selection: &self.selection,
            content: &self.content,
            camera: self.camera_label(),
            steering_ready: self.is_landscape_ready(),
            cockpit_ready: self.cockpit_ready,
            scene_loaded: self.pivots.is_some(),
            loading_phase: self.loading_started.elapsed().as_secs_f32() * 5.0,
            loading_error: self.scene_error.as_deref(),
            cam_override,
            selected_control: self.selected_control,
            sensitivity: self.controls.sensitivity,
            dead_zone: self.controls.dead_zone,
            volume: self.volume,
            music_volume: self.music_volume,
            codriver_volume: self.codriver_volume,
            fps: self.fps_visible,
            diagnostics: self.diagnostics,
            minimal_hud: self.minimal_hud,
            hud_rpm_style: self.hud_rpm_style,
            invert_pedals: self.controls.invert_pedals,
            clutch_help: self.clutch_help,
            ctrl_opacity: self.controls_layout.opacity_for(self.selected_control),
            global_opacity: self.controls_layout.global_opacity,
            stage_result: self.race.result(),
            has_replay: self.saved_replay.is_some(),
            ngp_experimental: self.ngp_experimental,
            focus_diagnostics: self.focus_diagnostics.as_ref(),
            gamepad_connected: self.gamepad.connected,
            gamepad_name: self
                .gamepad
                .connected
                .then_some(self.gamepad.controller_name.as_str()),
            gamepad_settings: self.gamepad.settings.clone(),
            pacenote_hud: self.pacenote_hud.clone(),
            pacenote_atlas_status: Some(self.pacenote_atlas_status.as_str()),
            championship: Some(&self.championship),
            school: Some(&self.school),
            active_school_lesson: self.active_school_lesson,
            school_result: self.active_school_result.as_ref(),
            personal_best_ms: self.personal_best_for_stage(&self.selection.stage),
            inspected_stage_pb_ms: self
                .content
                .stages
                .get(self.menu.inspected_stage)
                .and_then(|e| self.personal_best_for_stage(&e.descriptor)),
            record_outcome: self.last_record_outcome.as_ref(),
            run_splits_ms: &self.last_finish_splits_ms,
            comparison_pb_splits_ms: self.last_run_prev_pb_splits_ms.as_deref(),
            showroom_vehicle_name: self.showroom_active_name.as_deref(),
            showroom_loading: self.showroom_loading,
            showroom_error: self.showroom_error.as_deref(),
            title_pulse: (self.title_started.elapsed().as_secs_f32() * 2.4).sin() * 0.5 + 0.5,
            transition_alpha: self.transition_started.map_or(0.0, |t| {
                let dt = t.elapsed().as_secs_f32();
                (1.0 - dt / 0.32).clamp(0.0, 1.0)
            }),
            coming_soon_notice: self
                .coming_soon_notice
                .as_ref()
                .and_then(|(msg, until)| (Instant::now() < *until).then_some(msg.as_str())),
            career: Some(&self.career),
            active_career_stage: self.active_career_stage,
            last_career_stage_outcome: self.last_career_stage_outcome.as_ref(),
            pilot_draft_name: self.pilot_draft_name.as_str(),
            pilot_draft_number: self.pilot_draft_number,
            pending_reset_career: self.pending_reset_career,
            pending_buy_vehicle_id: self.pending_buy_vehicle_id.as_deref(),
            pending_sell_vehicle_id: self.pending_sell_vehicle_id.as_deref(),
            selected_career_difficulty: self.selected_career_difficulty,
            selected_championship_kind: self.selected_championship_kind,
            selected_service_tyre: self.selected_service_tyre,
        }
    }
    fn save_active_career(&mut self) {
        if let Some(p) = self.career.profile.as_ref() {
            if p.is_dev_profile {
                if let Some(root) =
                    std::env::var_os("RERBR_PROBE_DOCUMENTS").map(std::path::PathBuf::from)
                {
                    if let Err(err) = career::CareerStore::save_dev_profile_in_root(&root, p) {
                        eprintln!("dev career SAVE ERROR: {err}");
                    }
                }
                return;
            }
        }
        if let Err(err) = self.career.save() {
            eprintln!("career SAVE ERROR: {err}");
        }
    }
    fn status(&self, state: &worker::Shared) -> String {
        if let Some(error) = self.scene_error.as_ref().or(state.error.as_ref()) {
            return format!("ERROR: {error}");
        }
        if (!self.experimental_visual_only && state.rest.is_none()) || self.pivots.is_none() {
            return "LOADING PRIVATE STAGE / CAR / PHYSICS".into();
        }
        if !self.experimental_visual_only && !self.is_landscape_ready() {
            return "STEERING UNAVAILABLE - HOLD IPAD IN LANDSCAPE".into();
        }
        if let Some(message) = &self.message {
            return message.clone();
        }
        if self.menu.page == menu::Page::StageComplete {
            "STAGE COMPLETE / SELECT RESTART OR MAIN MENU".into()
        } else if self.menu.page == menu::Page::Paused {
            "PAUSED / RESUME WHEN READY".into()
        } else {
            "READY - TAP A ROW".into()
        }
    }
    pub fn overlay(
        &mut self,
        hud: &Hud,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.drawable_size = size;
        if !std::mem::take(&mut self.motion_polled_this_frame)
            && self.menu.page != menu::Page::Paused
        {
            let live_tilt = motion(true);
            if live_tilt.is_finite() {
                self.tilt = live_tilt;
            }
        }
        if !std::mem::take(&mut self.gamepad_polled_this_frame) {
            self.poll_gamepad();
        }
        let state = self.worker.state.lock().unwrap().clone();
        let mut canvas = Canvas::new(size);
        let layout = self.effective_layout(size);
        if self.menu.page == menu::Page::ControlsEditor {
            self.draw_controls_editor(&mut canvas, &layout);
            let status = self.status(&state);
            let mut ctx = self.context();
            ctx.loading_error = self.scene_error.as_deref().or(state.error.as_deref());
            self.menu.draw(&mut canvas, &ctx, &status);
            hud.draw_bytes(queue, encoder, target, &canvas.finish());
            return;
        }
        if self.menu.page == menu::Page::Replay {
            if let Some(player) = &self.replay_player {
                self.draw_replay_overlay(&mut canvas, player);
            }
            hud.draw_bytes(queue, encoder, target, &canvas.finish());
            return;
        }
        if self.menu.page == menu::Page::CareerCreate {
            if let Some(typed) = poll_pilot_name_ios() {
                let clean = career::sanitize_pilot_name(&typed);
                if !clean.is_empty() {
                    self.pilot_draft_name = clean;
                    self.message = Some(format!("NOMBRE ACTUALIZADO: {}", self.pilot_draft_name));
                }
            }
        }
        if self.menu.page != menu::Page::Driving {
            let status = self.status(&state);
            let mut ctx = self.context();
            ctx.loading_error = self.scene_error.as_deref().or(state.error.as_deref());
            self.menu.draw(&mut canvas, &ctx, &status);
            if self.menu.page.is_showroom_page() {
                if let Some(p) = self.career.profile.as_ref() {
                    let badge = format!("#{}", p.competition_number);
                    for &(nx, ny) in &self.showroom_dorsal_positions {
                        let r = Rect::new(
                            (nx - 0.024).clamp(0.02, 0.94),
                            (ny - 0.022).clamp(0.12, 0.68),
                            0.048,
                            0.044,
                        );
                        ui::panel(&mut canvas, r, [242, 244, 248, 228], [24, 28, 36, 255]);
                        ui::text_center(&mut canvas, r, &badge, 0.66, [18, 22, 28, 255]);
                    }
                }
            }
            hud.draw_bytes(queue, encoder, target, &canvas.finish());
            return;
        }
        self.draw_top_bar(&mut canvas, &layout);
        if let Some((_, sample, _)) = &state.pair {
            let rpm_spec = self.active_vehicle_rpm_spec();
            draw_cluster_with_rpm_hud(
                &mut canvas,
                &layout,
                sample,
                &rpm_spec,
                self.hud_rpm_style,
                self.minimal_hud,
            );
            if self.draws_interior() && !self.minimal_hud {
                draw_cockpit_instruments(&mut canvas, &layout, sample, &rpm_spec);
            }
        }
        self.draw_side_controls(&mut canvas, &layout);
        self.draw_countdown_and_timer(&mut canvas);
        if self.diagnostics {
            self.draw_telemetry(&mut canvas, &layout, &state);
        }
        self.draw_banners(&mut canvas, &layout, &state);
        let mut sprite_canvas = Canvas::new(size);
        self.draw_pacenote_visual_hud(&mut canvas, &mut sprite_canvas);
        hud.draw_bytes_and_sprites(
            queue,
            encoder,
            target,
            &canvas.finish(),
            &sprite_canvas.finish(),
        );
    }
    fn draw_pacenote_visual_hud(&self, canvas: &mut Canvas, sprite_canvas: &mut Canvas) {
        if !self.pacenote_hud.enabled || self.race.phase != race::Phase::Driving {
            return;
        }
        let Some(visual) = self
            .codriver
            .as_ref()
            .and_then(|c| c.current_visual(Instant::now(), self.pacenote_hud.anticipation_s))
        else {
            return;
        };
        let icon_sz = 38.0 * self.pacenote_hud.scale;
        let ch = canvas.height();
        let cx = self.pacenote_hud.pos_x * 640.0;
        let cy = self.pacenote_hud.pos_y * ch;
        let pad = 6.0;
        let (main_ref, mod_ref) = pacenote_icons::sprite_indices_for_callout(
            visual.tag,
            visual.slot_word_id,
            visual.flag_word_id,
        );
        let box_w = icon_sz
            + if mod_ref.is_some() {
                icon_sz * 0.58
            } else {
                0.0
            }
            + pad * 2.0;
        let box_h = icon_sz
            + if self.pacenote_hud.debug_sprites {
                30.0
            } else {
                18.0
            }
            + pad * 2.0;
        let bx = (cx - box_w * 0.5).clamp(8.0, 632.0 - box_w);
        let by = (cy - box_h * 0.5).clamp(8.0, ch - box_h - 8.0);
        canvas.rect(bx, by, box_w, box_h, [10, 14, 22, 200]);

        if let Some(atlas) = &self.pacenote_atlas {
            let mut drew_sprite = false;
            if let Some(sref) = main_ref.and_then(|r| {
                atlas.sprite_uv(r.sprite).map(|mut q| {
                    if r.mirror_x {
                        q.uv.swap(0, 2);
                    }
                    q
                })
            }) {
                let (dw, dh) = if sref.aspect < 1.0 {
                    (icon_sz * sref.aspect.max(0.24), icon_sz)
                } else {
                    (icon_sz, icon_sz / sref.aspect.max(1.0))
                };
                let ox = bx + pad + (icon_sz - dw) * 0.5;
                let oy = by + pad + (icon_sz - dh) * 0.5;
                sprite_canvas.quad(ox, oy, dw, dh, sref.uv, [255, 255, 255, 255]);
                drew_sprite = true;
            }
            if let Some(mref) = mod_ref.and_then(|r| {
                atlas.sprite_uv(r.sprite).map(|mut q| {
                    if r.mirror_x {
                        q.uv.swap(0, 2);
                    }
                    q
                })
            }) {
                let max_m = icon_sz * 0.52;
                let (dw, dh) = if mref.aspect < 1.0 {
                    (max_m * mref.aspect.max(0.24), max_m)
                } else {
                    (max_m, max_m / mref.aspect.max(1.0))
                };
                sprite_canvas.quad(
                    bx + pad + icon_sz + 3.0 + (max_m - dw) * 0.5,
                    by + pad + (icon_sz - dh) * 0.5,
                    dw,
                    dh,
                    mref.uv,
                    [255, 255, 255, 255],
                );
            }
            if !drew_sprite {
                canvas.text(
                    bx + pad,
                    by + pad + icon_sz * 0.35,
                    &visual.label,
                    0.56,
                    TEXT,
                );
            }
            if self.pacenote_hud.debug_sprites {
                let dbg = match (main_ref, mod_ref) {
                    (Some(m), Some(f)) => format!(
                        "T{} {:#x}{} +{:#x}",
                        visual.tag,
                        m.sprite,
                        if m.mirror_x { "M" } else { "" },
                        f.sprite
                    ),
                    (Some(m), None) => format!(
                        "T{} {:#x}{}",
                        visual.tag,
                        m.sprite,
                        if m.mirror_x { "M" } else { "" }
                    ),
                    _ => format!("T{} NO_SPR", visual.tag),
                };
                ui::text_clipped(
                    canvas,
                    bx + pad,
                    by + pad + icon_sz + 14.0,
                    box_w - pad * 2.0,
                    &dbg,
                    0.42,
                    WARN,
                );
            }
        } else {
            // Honest indicator when PC_Generic_*.bnk is absent: never draw fake corner arrows.
            canvas.text(bx + pad, by + pad + 4.0, "SIN BNK", 0.45, WARN);
        }
        ui::text_clipped(
            canvas,
            bx + pad,
            by + pad + icon_sz + 3.0,
            box_w - pad * 2.0,
            &visual.label,
            0.55,
            TEXT,
        );
    }
    fn draw_replay_overlay(&self, canvas: &mut Canvas, player: &replay::ReplayPlayer) {
        let layout = layout::ReplayLayout::for_size(self.drawable_size);

        // Top bar
        ui::panel(canvas, layout.top_bar, PANEL_DEEP, LINE);
        let cam_label = match player.camera_mode {
            replay::ReplayCameraMode::StageTV => {
                let dist = player.current_frame().map_or(0.0, |f| f.driveline_distance);
                if let Some((idx, total)) = self
                    .stage_cameras
                    .as_ref()
                    .and_then(|sc| sc.active_camera_index(dist))
                {
                    format!("TV CAM ({idx}/{total})")
                } else {
                    "TV CAM (CHASE FALLBACK)".into()
                }
            }
            _ => player.camera_mode.label().into(),
        };
        let banner = format!(
            "REPETICIÓN — {} — {} | {}",
            self.selection.vehicle.name, self.selection.stage.name, cam_label
        );
        let (x, y, w, h) = ui::px(layout.top_bar, canvas);
        ui::text_clipped(
            canvas,
            x + 10.0,
            y + (h - 11.0).max(0.0) * 0.5,
            w - 20.0,
            &banner,
            0.78,
            TEXT,
        );

        // Timeline bar
        ui::fill(canvas, layout.timeline, [30, 35, 45, 200]);
        let progress = player.progress();
        if progress > 0.0 {
            let filled = Rect::new(
                layout.timeline.x,
                layout.timeline.y,
                layout.timeline.w * progress,
                layout.timeline.h,
            );
            ui::fill(canvas, filled, ACCENT);
        }

        // Time text
        let time_str = format!(
            "{} / {}",
            menu::format_stage_time(player.current_time as f64),
            menu::format_stage_time(player.total_duration() as f64)
        );
        let time_rect = Rect::new(0.40, 0.115, 0.20, 0.04);
        ui::panel(canvas, time_rect, PANEL_DEEP, LINE);
        ui::text_center(canvas, time_rect, &time_str, 0.80, TEXT);

        // Bottom control buttons
        let pause_label = if player.state == replay::PlaybackState::Paused {
            "PLAY"
        } else {
            "PAUSA"
        };
        ui::button(
            canvas,
            layout.play_pause,
            pause_label,
            player.state == replay::PlaybackState::Playing,
            true,
        );
        ui::button(canvas, layout.restart, "REINICIAR", false, true);
        ui::button(
            canvas,
            layout.camera,
            &format!("CAM: {}", player.camera_mode.label()),
            false,
            true,
        );
        ui::button(canvas, layout.exit, "SALIR", false, true);
    }
    fn draw_controls_editor(&self, canvas: &mut Canvas, layout: &DrivingLayout) {
        for ctrl in layout::EditableControl::ALL {
            let rect = layout.control_rect(ctrl);
            let selected = ctrl == self.selected_control;
            let op = self.controls_layout.opacity_for(ctrl);
            ui::panel(
                canvas,
                rect,
                ui::with_alpha(if selected { PANEL_ACTIVE } else { PANEL_DEEP }, op),
                ui::with_alpha(if selected { ACCENT } else { INFO }, op),
            );
            ui::text_center(canvas, rect, ctrl.label(), 0.8, ui::with_alpha(TEXT, op));
        }
    }
    fn draw_countdown_and_timer(&self, canvas: &mut Canvas) {
        if let Some(rem) = self.race.countdown() {
            let rect = Rect::new(0.41, 0.24, 0.18, 0.18);
            ui::panel(canvas, rect, PANEL_DEEP, ACCENT);
            ui::text_center(canvas, rect, &rem.to_string(), 2.6, TEXT);
        } else if self.race.go_visible(Instant::now()) {
            let rect = Rect::new(0.39, 0.24, 0.22, 0.16);
            ui::panel(canvas, rect, PANEL_DEEP, ui::GOOD);
            ui::text_center(canvas, rect, "GO!", 2.2, ui::GOOD);
        }
        if let Some(secs) = self.stage_elapsed_secs() {
            let cur_ms = (secs.max(0.0) * 1000.0).round() as u64;
            let timer = records::format_time_ms(cur_ms);
            let r = Rect::new(0.415, 0.015, 0.17, 0.055);
            ui::panel(canvas, r, PANEL_DEEP, LINE);
            ui::text_center(canvas, r, &timer, 0.85, TEXT);

            if self.active_school_lesson.is_none() {
                if let Some(pb_ms) = self.personal_best_for_stage(&self.selection.stage) {
                    let delta_ms = (cur_ms as i64) - (pb_ms as i64);
                    let pb_label = if matches!(
                        self.race.phase,
                        race::Phase::Finishing | race::Phase::Finished
                    ) {
                        format!(
                            "PB {} ({})",
                            records::format_time_ms(pb_ms),
                            records::format_delta_ms(delta_ms)
                        )
                    } else {
                        format!("PB {}", records::format_time_ms(pb_ms))
                    };
                    let pb_color = if delta_ms <= 0 { ui::GOOD } else { WARN };
                    let pb_rect = Rect::new(0.415, 0.074, 0.17, 0.032);
                    ui::panel(canvas, pb_rect, PANEL_DEEP, pb_color);
                    ui::text_center(canvas, pb_rect, &pb_label, 0.56, pb_color);
                }
                if let Some(banner) = self.split_tracker.active_banner(Instant::now()) {
                    let color = match banner.is_faster() {
                        Some(true) => ui::GOOD,
                        Some(false) => BAD,
                        None => INFO,
                    };
                    let split_rect = Rect::new(0.24, 0.112, 0.52, 0.044);
                    ui::panel(canvas, split_rect, PANEL_DEEP, color);
                    let headline = banner.headline();
                    let (_, _, width, _) = ui::px(split_rect, canvas);
                    let needed = ui::texture_width(&headline, 0.74);
                    let fit = (width - 14.0).max(1.0);
                    let scale = if needed > fit {
                        (0.74 * fit / needed).clamp(0.48, 0.74)
                    } else {
                        0.74
                    };
                    ui::text_center(canvas, split_rect, &headline, scale, color);
                }
            }
        }
    }
    fn draw_top_bar(&self, canvas: &mut Canvas, layout: &DrivingLayout) {
        ui::button(canvas, layout.pause, "PAUSE", false, true);
        let camera = if self.minimal_hud {
            "CAM".to_string()
        } else {
            format!("CAM {}", self.camera_label())
        };
        ui::button(canvas, layout.camera, &camera, false, true);
        ui::button(canvas, layout.diag, "DIAG", self.diagnostics, true);
        if let Some(rect) = layout.engine_start_rect() {
            let held = self.controls.starter_held();
            ui::panel(canvas, rect, PANEL_DEEP, if held { ACCENT } else { LINE });
            let (x, y, w, h) = ui::px(rect, canvas);
            canvas.power_symbol(
                x + w * 0.5,
                y + h * 0.51,
                w.min(h) * 0.25,
                if held { ACCENT } else { TEXT },
            );
        }
        if self.fps_visible {
            ui::button(
                canvas,
                layout.fps,
                &format!("{:.0} FPS", self.fps),
                true,
                true,
            );
        } else {
            ui::button(canvas, layout.fps, "FPS", false, true);
        }
    }
    fn draw_side_controls(&self, canvas: &mut Canvas, layout: &DrivingLayout) {
        let levels = self.controls.levels_in(layout);
        let hb_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::Handbrake);
        let clutch_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::Clutch);
        let gdown_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::GearDown);
        let gup_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::GearUp);
        let brake_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::Brake);
        let throttle_op = self
            .controls_layout
            .opacity_for(layout::EditableControl::Throttle);

        draw_control(canvas, layout.handbrake, "HB", levels.handbrake, hb_op);
        draw_control(canvas, layout.clutch, "CLUTCH", levels.clutch, clutch_op);
        draw_control(canvas, layout.gear_down, "GEAR -", 0.0, gdown_op);
        draw_control(canvas, layout.gear_up, "GEAR +", 0.0, gup_op);
        draw_control(canvas, layout.brake, "BRAKE", levels.brake, brake_op);
        draw_control(
            canvas,
            layout.throttle,
            "THROTTLE",
            levels.throttle,
            throttle_op,
        );
    }
    fn draw_telemetry(&self, canvas: &mut Canvas, layout: &DrivingLayout, state: &worker::Shared) {
        let rect = layout.telemetry;
        ui::panel(canvas, rect, PANEL_DEEP, LINE);
        let (x, y, w, h) = ui::px(rect, canvas);
        canvas.text(x + 10.0, y + 7.0, "TELEMETRY", 0.7, ACCENT);
        let mut lines = vec![format!(
            "FPS {:.0} / CPU {:.1} MS / MEM {:.0} MIB",
            self.fps,
            self.cpu_ms,
            self.footprint.unwrap_or(0) as f64 / 1048576.0
        )];
        if self.selection.vehicle.is_experimental() {
            lines.insert(
                0,
                format!("VEHICLE: {} (RSF NGP)", self.selection.vehicle.name),
            );
        }
        if let Some(status) = &state.physics_status {
            lines.extend(ui::wrap(status, 34));
        }
        if state.timing.finish_distance > 0.0 || state.timing.splits_total > 0 {
            lines.push(format!(
                "STAGE {:.0}M / {:.0}M | SPLITS {}/{} | FIN {}",
                state.timing.driveline_distance,
                state.timing.finish_distance,
                state.timing.splits_passed,
                state.timing.splits_total,
                if state.timing.finished { "YES" } else { "NO" }
            ));
        }
        if let Some((_, b, _)) = &state.pair {
            lines.extend([
                format!(
                    "STEER {:.3} / SENS {:.2} / DEAD {:.2}",
                    b.steering_received, self.controls.sensitivity, self.controls.dead_zone
                ),
                format!("GEAR {} / TARGET {}", b.gear, b.commanded_gear),
                format!(
                    "CLUTCH {:.2} / SLIP {:.2}",
                    b.payload_clutch, b.drive_clutch_slip
                ),
                format!("STEP {:.3} MS", state.last_step_ms),
                format!("VEL {:.1?}", b.velocity),
                format!("ANG VEL {:.2?}", b.angular_velocity),
                format!("TRAVEL {:.3?}", b.wheel_travel),
                format!("GROUND {:?}", b.wheel_ground),
            ]);
        }
        let max_width = w - 20.0;
        let max_lines = (((h - 28.0) / 10.4).floor() as usize).clamp(1, 12);
        for (index, line) in lines.iter().take(max_lines).enumerate() {
            ui::text_clipped(
                canvas,
                x + 10.0,
                y + 24.0 + index as f32 * 10.4,
                max_width,
                line,
                0.62,
                if index == 0 { TEXT } else { INFO },
            );
        }
    }
    fn draw_banners(&self, canvas: &mut Canvas, layout: &DrivingLayout, state: &worker::Shared) {
        let mut banners: Vec<(String, [u8; 4])> = Vec::new();
        if self.race.false_start_visible(Instant::now()) {
            banners.push(("FALSE START +10s".into(), BAD));
        }
        if self.experimental_visual_only {
            let detail = self
                .experimental_physics_reason
                .as_deref()
                .unwrap_or("SIN FÍSICA NGP");
            banners.push((format!("MODO VISUAL EXPERIMENTAL — {detail}"), BAD));
        } else if self.selection.vehicle.is_ngp_experimental_car() {
            let fmod_tag = if audio::fmod_runtime_snapshot().is_some_and(|s| s.decoded_samples > 0)
            {
                "FMOD ACTIVO"
            } else {
                "SIN FMOD"
            };
            banners.push((
                format!(
                    "RSF NGP 7.4 ACTIVO: {} ({fmod_tag})",
                    self.selection.vehicle.name
                ),
                WARN,
            ));
        }
        if let Some(error) = state.error.as_ref().or(self.scene_error.as_ref()) {
            banners.push((format!("ERROR: {error}"), BAD));
        } else if !self.experimental_visual_only && state.rest.is_none() {
            banners.push(("LOADING PHYSICS".into(), WARN));
        } else if !self.experimental_visual_only
            && !self.tilt.is_finite()
            && !self.gamepad.connected
        {
            banners.push(("HOLD IPAD IN LANDSCAPE TO STEER".into(), WARN));
        }
        if !self.cockpit_ready && self.camera_label() == "COCKPIT" {
            banners.push(("COCKPIT ASSETS MISSING - EXTERIOR VIEW".into(), WARN));
        }
        if let Some(note) = self
            .codriver
            .as_ref()
            .and_then(|c| c.current_text(Instant::now()))
        {
            banners.push((format!("NOTA: {note}"), INFO));
        }
        if self.active_school_lesson.is_some() {
            if let Some((text, until)) = &self.school_incident_banner {
                if Instant::now() < *until {
                    banners.push((text.clone(), WARN));
                }
            }
            if let Some(st) = &state.school_state {
                let target = if st.perfect_time > 0.0 && st.pass_time > 0.0 {
                    format!(
                        "ESCUELA · OBJ <= {:.1}S / PERF <= {:.1}S",
                        st.pass_time, st.perfect_time
                    )
                } else if let Some(d) = st.target_distance {
                    format!(
                        "ESCUELA · DISTANCIA {:.0}M / ZONA {:.0}M",
                        state.timing.driveline_distance, d
                    )
                } else {
                    "ESCUELA RBR ACTIVA".into()
                };
                banners.push((target, INFO));
            }
        }
        let split_offset = if self.active_school_lesson.is_none()
            && self.split_tracker.active_banner(Instant::now()).is_some()
        {
            0.052
        } else {
            0.0
        };
        for (index, (text, color)) in banners.iter().take(3).enumerate() {
            let rect = Rect::new(
                0.18,
                layout.pause.bottom() + 0.018 + split_offset + index as f32 * 0.040,
                0.64,
                0.034,
            );
            ui::panel(canvas, rect, PANEL_ACTIVE, *color);
            let (x, y, w, h) = ui::px(rect, canvas);
            ui::text_clipped(
                canvas,
                x + 10.0,
                y + (h - 11.0).max(0.0) * 0.5,
                w - 20.0,
                text,
                0.68,
                *color,
            );
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct VehicleRpmSpec {
    pub redline_rpm: Option<f32>,
    pub optimal_shift_rpm: Option<f32>,
    pub scale_max_rpm: f32,
}

impl VehicleRpmSpec {
    pub fn from_tech_sheet(sheet: Option<&ngp_proto::VehicleTechSheet>) -> Self {
        let Some(sheet) = sheet else {
            return Self {
                redline_rpm: None,
                optimal_shift_rpm: None,
                scale_max_rpm: 8000.0,
            };
        };
        let redline = sheet
            .rpm_limit
            .filter(|&r| (3500..=15000).contains(&r))
            .map(|r| r as f32);
        let power_rpm = match sheet.profile {
            ngp_proto::PhysicsProfile::NgpDetected {
                power_rpm: Some(p), ..
            } if p.is_finite() && p >= 2500.0 => Some(p),
            _ => None,
        };
        // Only expose a distinct optimal shift point when both peak-power RPM and
        // real rev-limit RPM are present in the car's `.lsp` data.
        let optimal_shift_rpm = match (power_rpm, redline) {
            (Some(p), Some(r)) if p < r * 0.96 => Some((p + (r - p) * 0.45).clamp(p, r * 0.96)),
            _ => None,
        };
        let scale_max_rpm = redline
            .or_else(|| power_rpm.map(|p| (p * 1.18).max(7000.0)))
            .unwrap_or(8000.0);
        Self {
            redline_rpm: redline,
            optimal_shift_rpm,
            scale_max_rpm,
        }
    }
}

impl Runtime {
    pub(crate) fn active_vehicle_rpm_spec(&self) -> VehicleRpmSpec {
        let sheet = self
            .content
            .vehicle(&self.selection.vehicle.id)
            .and_then(|v| v.tech_sheet.as_ref());
        VehicleRpmSpec::from_tech_sheet(sheet)
    }
}

fn draw_cluster_with_rpm_hud(
    canvas: &mut Canvas,
    layout: &DrivingLayout,
    sample: &Sample,
    spec: &VehicleRpmSpec,
    style: menu::HudRpmStyle,
    minimal_hud: bool,
) {
    let rect = layout.cluster;
    let gear = match sample.gear {
        0 => "R".to_string(),
        1 => "N".to_string(),
        g => (g - 1).to_string(),
    };
    let speed_kmh = format!("{:.0}", sample.speed() * 3.6);
    let rpm_val = sample.rpm.max(0.0);
    let max_rpm = spec.scale_max_rpm.max(4000.0);
    let frac = (rpm_val / max_rpm).clamp(0.0, 1.0);
    let redline = spec.redline_rpm.unwrap_or(max_rpm * 0.92);
    let shift_rpm = spec.optimal_shift_rpm.unwrap_or(redline * 0.90);
    let near_cut = rpm_val >= redline * 0.94;
    let at_shift = rpm_val >= shift_rpm;

    let border = if near_cut {
        BAD
    } else if at_shift {
        WARN
    } else {
        LINE
    };
    ui::panel(canvas, rect, PANEL, border);

    let (rx, ry, rw, rh) = ui::px(rect, canvas);
    let gear_w = rw * 0.24;
    let speed_w = rw * 0.30;
    let right_x = rx + gear_w + speed_w;
    let right_w = (rw - gear_w - speed_w).max(20.0);

    // Dividers between GEAR | KM/H | RPM gauge
    canvas.rect(rx + gear_w, ry + rh * 0.16, 1.0, rh * 0.68, LINE);
    canvas.rect(right_x, ry + rh * 0.16, 1.0, rh * 0.68, LINE);

    // 1. Gear (large, clear)
    let gear_col = if near_cut { BAD } else { ACCENT };
    let gw = ui::texture_width(&gear, 1.55);
    canvas.text(
        rx + (gear_w - gw).max(0.0) * 0.5,
        ry + 3.0,
        &gear,
        1.55,
        gear_col,
    );
    let glw = ui::texture_width("MARCHA", 0.45);
    canvas.text(
        rx + (gear_w - glw).max(0.0) * 0.5,
        ry + rh - 10.0,
        "MARCHA",
        0.45,
        ui::DIM,
    );

    // 2. Speed KM/H
    let sw = ui::texture_width(&speed_kmh, 1.30);
    canvas.text(
        rx + gear_w + (speed_w - sw).max(0.0) * 0.5,
        ry + 4.0,
        &speed_kmh,
        1.30,
        TEXT,
    );
    let kw = ui::texture_width("KM/H", 0.48);
    canvas.text(
        rx + gear_w + (speed_w - kw).max(0.0) * 0.5,
        ry + rh - 10.0,
        "KM/H",
        0.48,
        ui::DIM,
    );

    // 3. RPM HUD (Style A: Horizontal Bar or Style B: Shift Lights)
    let pad = 6.0;
    let gx = right_x + pad;
    let gw_inner = (right_w - pad * 2.0).max(16.0);
    match style {
        menu::HudRpmStyle::HorizontalBar => {
            let bar_y = ry + 8.0;
            let bar_h = (rh * 0.34).clamp(8.0, 16.0);
            canvas.rect(gx, bar_y, gw_inner, bar_h, [20, 24, 32, 240]);
            let red_frac = (redline / max_rpm).clamp(0.75, 0.98);
            // Subtle redline zone backdrop on the right end
            let red_x = gx + gw_inner * red_frac;
            canvas.rect(
                red_x,
                bar_y,
                (gx + gw_inner) - red_x,
                bar_h,
                [85, 22, 26, 180],
            );

            // Optimal shift marker tick when verified from real car data
            if let Some(opt) = spec.optimal_shift_rpm {
                let opt_x = gx + gw_inner * (opt / max_rpm).clamp(0.4, 0.96);
                canvas.rect(opt_x - 1.0, bar_y - 2.0, 2.0, bar_h + 4.0, WARN);
            }

            // Segmented GT-style progress fill
            let segs = 16usize;
            let seg_gap = 1.5;
            let seg_w = ((gw_inner - seg_gap * (segs - 1) as f32) / segs as f32).max(1.5);
            for i in 0..segs {
                let thresh = (i + 1) as f32 / segs as f32;
                if frac + 0.01 < thresh {
                    continue;
                }
                let seg_col = if thresh >= red_frac {
                    BAD
                } else if thresh >= 0.74 {
                    WARN
                } else {
                    [56, 214, 152, 255]
                };
                let sx = gx + i as f32 * (seg_w + seg_gap);
                canvas.rect(sx, bar_y + 1.0, seg_w, bar_h - 2.0, seg_col);
            }
        }
        menu::HudRpmStyle::ShiftLights => {
            let leds = 12usize;
            let gap = 2.5;
            let led_w = ((gw_inner - gap * (leds - 1) as f32) / leds as f32).max(2.0);
            let led_h = (rh * 0.36).clamp(8.0, 16.0);
            let led_y = ry + 7.0;
            let active_leds = ((frac * leds as f32).ceil() as usize).min(leds);
            for i in 0..leds {
                let lx = gx + i as f32 * (led_w + gap);
                let lit = i < active_leds;
                let base = if i < 5 {
                    [46, 214, 118, 255]
                } else if i < 9 {
                    [245, 178, 52, 255]
                } else {
                    [242, 62, 62, 255]
                };
                let col = if lit {
                    base
                } else {
                    [base[0] / 5, base[1] / 5, base[2] / 5, 190]
                };
                canvas.rect(lx, led_y, led_w, led_h, col);
            }
        }
    }

    // Bottom RPM numeric + cut/shift indicator
    if !minimal_hud {
        let rpm_txt = if near_cut {
            format!("{:.0} CORTE!", rpm_val)
        } else if let Some(lim) = spec.redline_rpm {
            format!("{:.0} / {:.0}", rpm_val, lim)
        } else {
            format!("{:.0} RPM", rpm_val)
        };
        let txt_col = if near_cut {
            BAD
        } else if at_shift {
            WARN
        } else {
            INFO
        };
        canvas.text(gx, ry + rh - 11.0, &rpm_txt, 0.52, txt_col);
    }
}

/// Functional cockpit analog-style tachometer & speedometer needles + digital gear/kmh readout,
/// driven directly by 720 Hz physics telemetry when rendering inside `COCKPIT` view.
fn draw_cockpit_instruments(
    canvas: &mut Canvas,
    layout: &DrivingLayout,
    sample: &Sample,
    spec: &VehicleRpmSpec,
) {
    let dial_y = (layout.cluster.y - 0.135).max(0.55);
    let tacho_rect = Rect::new(layout.cluster.x + 0.01, dial_y, 0.088, 0.125);
    let speed_rect = Rect::new(
        layout.cluster.x + layout.cluster.w - 0.098,
        dial_y,
        0.088,
        0.125,
    );
    let rpm_frac = (sample.rpm.max(0.0) / spec.scale_max_rpm.max(4000.0)).clamp(0.0, 1.0);
    let speed_kmh = sample.speed() * 3.6;
    let speed_frac = (speed_kmh as f32 / 220.0).clamp(0.0, 1.0);

    draw_analog_dial(
        canvas,
        tacho_rect,
        rpm_frac,
        &format!("{:.0}", sample.rpm / 100.0),
        "x100",
        if rpm_frac >= 0.90 { BAD } else { ACCENT },
    );
    draw_analog_dial(
        canvas,
        speed_rect,
        speed_frac,
        &format!("{:.0}", speed_kmh),
        "KM/H",
        INFO,
    );
}

fn draw_analog_dial(
    canvas: &mut Canvas,
    rect: Rect,
    frac: f32,
    value: &str,
    unit: &str,
    needle_color: [u8; 4],
) {
    ui::panel(canvas, rect, [10, 13, 19, 196], LINE);
    let (x, y, w, h) = ui::px(rect, canvas);
    let cx = x + w * 0.5;
    let cy = y + h * 0.48;
    let r = (w.min(h) * 0.36).max(10.0);
    let start_ang = std::f32::consts::PI * 0.78;
    let sweep = std::f32::consts::PI * 1.44;
    for t in 0..=8 {
        let tf = t as f32 / 8.0;
        let a = start_ang + sweep * tf;
        let tx = cx + a.cos() * r;
        let ty = cy + a.sin() * r;
        let tcol = if tf >= 0.875 { BAD } else { ui::DIM };
        canvas.rect(tx - 1.0, ty - 1.0, 2.0, 2.0, tcol);
    }
    let ang = start_ang + sweep * frac.clamp(0.0, 1.0);
    for step in 1..=6 {
        let d = r * (step as f32 / 6.5);
        canvas.rect(
            cx + ang.cos() * d - 1.2,
            cy + ang.sin() * d - 1.2,
            2.4,
            2.4,
            needle_color,
        );
    }
    let vw = ui::texture_width(value, 0.58);
    canvas.text(cx - vw * 0.5, y + h - 18.0, value, 0.58, TEXT);
    let uw = ui::texture_width(unit, 0.42);
    canvas.text(cx - uw * 0.5, y + h - 9.0, unit, 0.42, ui::DIM);
}
fn draw_control(canvas: &mut Canvas, rect: Rect, label: &str, level: f32, opacity: f32) {
    let active = level > 0.0;
    ui::fill(canvas, rect, ui::with_alpha(PANEL_DEEP, opacity));
    let (x, y, w, h) = ui::px(rect, canvas);
    if active {
        let fill_height = (h - 4.0) * level.clamp(0.0, 1.0);
        let fill_alpha = (90.0 * opacity.clamp(0.0, 1.0)).round() as u8;
        canvas.rect(
            x + 2.0,
            y + h - 2.0 - fill_height,
            w - 4.0,
            fill_height,
            [240, 184, 80, fill_alpha],
        );
    }
    ui::frame(
        canvas,
        rect,
        ui::with_alpha(if active { ACCENT } else { LINE }, opacity),
        1.5,
    );
    ui::text_center(canvas, rect, label, 0.85, ui::with_alpha(TEXT, opacity));
    if active && (label == "BRAKE" || label == "THROTTLE") {
        let value = format!("{:.0}%", level * 100.0);
        let (cx, cy, _w, _h) = ui::px(rect, canvas);
        canvas.text(
            cx + 10.0,
            cy + h - 17.0,
            &value,
            0.6,
            ui::with_alpha(ACCENT, opacity),
        );
    }
}
/// Ensures the presentation sweep eye stays clear of nearby embankments/terrain so bumpers and wheels
/// are never occluded on the starting grid.
#[cfg_attr(not(feature = "physics-stage"), allow(unused_variables))]
fn safe_presentation_camera(
    path: &ChasePath,
    cinematic: Camera,
    fallback: Camera,
    size: (u32, u32),
) -> Camera {
    #[cfg(feature = "physics-stage")]
    {
        if path.camera_clear(cinematic, 65.0, size) {
            return cinematic;
        }
        for lift in [0.4_f32, 0.9, 1.5] {
            let raised = Camera {
                eye: cinematic.eye + Vec3::Y * lift,
                ..cinematic
            };
            if path.camera_clear(raised, 65.0, size) {
                return raised;
            }
        }
        for blend in [0.35_f32, 0.7] {
            let mixed = Camera {
                eye: cinematic.eye.lerp(fallback.eye, blend),
                target: cinematic.target.lerp(fallback.target, blend),
                up: cinematic.up.lerp(fallback.up, blend).normalize_or(Vec3::Y),
            };
            if path.camera_clear(mixed, 65.0, size) {
                return mixed;
            }
        }
        fallback
    }
    #[cfg(not(feature = "physics-stage"))]
    {
        let _ = (path, fallback, size);
        cinematic
    }
}
/// Same mirror/pose/model composition as reRBR playback/frames.rs and playback::model_to_stage.
pub fn models(a: &Sample, b: &Sample, t: f32, pivots: [Vec3; 4], offset: Vec3) -> [Mat4; 5] {
    let t = t.clamp(0.0, 1.0);
    let swap = coords::physics_to_stage();
    let q = Quat::from_array(a.orientation)
        .normalize()
        .slerp(Quat::from_array(b.orientation).normalize(), t);
    let car = swap
        * Mat4::from_rotation_translation(
            q,
            Vec3::from(a.position).lerp(Vec3::from(b.position), t),
        );
    std::array::from_fn(|i| {
        if i == 0 {
            car * Mat4::from_translation(offset) * swap
        } else {
            let k = i - 1;
            let roll = a.wheel_roll[k]
                + ((b.wheel_roll[k] - a.wheel_roll[k] + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI)
                    * t;
            let steer =
                wheel_heading(Vec3::from(a.wheel_axle[k]).lerp(Vec3::from(b.wheel_axle[k]), t));
            car * Mat4::from_translation(
                Vec3::from(a.wheel_centre[k]).lerp(Vec3::from(b.wheel_centre[k]), t),
            ) * swap
                * Mat4::from_rotation_y(steer)
                * Mat4::from_axis_angle(Vec3::NEG_X, roll)
                * Mat4::from_translation(-pivots[k])
        }
    })
}
/// Project the original outward hub axle onto the car's horizontal XY plane.
/// The y/z mirror maps physics axle (x,y,z) to model (x,z,y), so model Y yaw
/// is -atan2(y,x). Opposite outward axle signs describe the same wheel plane.
/// This follows measured wheel heading (including toe), not a guessed rack ratio.
/// Camber remains outside this yaw-only presentation; unavailable axles stay neutral.
pub fn wheel_heading(axle: Vec3) -> f32 {
    if !axle.is_finite() || axle.x * axle.x + axle.y * axle.y < 1e-8 {
        return 0.0;
    }
    let axle = if axle.x < 0.0 { -axle } else { axle };
    -axle.y.atan2(axle.x)
}
fn motion(enabled: bool) -> f64 {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn rerbr_drive_tilt(enabled: i32) -> f64;
        }
        unsafe { rerbr_drive_tilt(i32::from(enabled)) }
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = enabled;
        f64::NAN
    }
}

fn prompt_pilot_name_ios(initial: &str) {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn rerbr_prompt_pilot_name(initial: *const std::os::raw::c_char);
        }
        if let Ok(cstr) = std::ffi::CString::new(initial) {
            unsafe { rerbr_prompt_pilot_name(cstr.as_ptr()) };
        }
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = initial;
    }
}

fn poll_pilot_name_ios() -> Option<String> {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn rerbr_poll_pilot_name(
                out: *mut std::os::raw::c_char,
                cap: std::os::raw::c_int,
            ) -> std::os::raw::c_int;
        }
        let mut buf = [0i8; 64];
        let ready = unsafe { rerbr_poll_pilot_name(buf.as_mut_ptr(), buf.len() as i32) };
        if ready != 0 {
            let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
            return cstr.to_str().ok().map(str::to_string);
        }
    }
    None
}

fn restored_stage(catalog: &Catalog, id: &str) -> rerbr_content::StageDescriptor {
    let default = rerbr_content::StageDescriptor::default();
    catalog
        .stage(id)
        .filter(|entry| {
            stage_supported(&entry.descriptor)
                && (!entry.files.checked || entry.files.missing.is_empty())
        })
        .or_else(|| catalog.stage(&default.id))
        .map(|entry| entry.descriptor.clone())
        .unwrap_or(default)
}

fn restored_vehicle(
    catalog: &Catalog,
    id: &str,
    ngp_experimental: bool,
) -> rerbr_content::VehicleDescriptor {
    let default = rerbr_content::VehicleDescriptor::default();
    if catalog.is_public_beta() {
        return catalog
            .vehicle(id)
            .or_else(|| catalog.vehicle(&default.id))
            .or_else(|| catalog.vehicles.first())
            .map_or(default, |entry| entry.descriptor.clone());
    }
    catalog
        .vehicle(id)
        .filter(|entry| {
            entry.classification != rerbr_content::VehicleClassification::VisualInspectionOnly
                && (ngp_experimental || !entry.descriptor.is_ngp_experimental_car())
                && (!entry.files.checked || entry.files.missing.is_empty())
        })
        .or_else(|| catalog.vehicle(&default.id))
        .map(|entry| entry.descriptor.clone())
        .unwrap_or(default)
}

/// Builds a startup-shuffled list of all installed 3D car models (drivable + visual-only)
/// for the Title Screen. Executed ONCE at app startup — never per frame — using lightweight
/// SplitMix64 entropy from system clock + PID with zero extra crate dependencies.
fn shuffled_showroom_candidates(catalog: &Catalog) -> Vec<rerbr_content::VehicleDescriptor> {
    let mut items: Vec<rerbr_content::VehicleDescriptor> = catalog
        .showroom_candidates()
        .into_iter()
        .map(|e| e.descriptor.clone())
        .collect();
    if items.len() <= 1 {
        return items;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9E37_79B9_7F4A_7C15u64, |d| d.as_nanos() as u64);
    let mut state = nanos ^ (u64::from(std::process::id()).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    let mut next_u64 = || -> u64 {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for i in (1..items.len()).rev() {
        let j = (next_u64() as usize) % (i + 1);
        items.swap(i, j);
    }
    items
}

// The partial Harwood benchmark is intentionally kept available as a fallback.
fn stage_supported(stage: &rerbr_content::StageDescriptor) -> bool {
    stage.validate_current_backend().is_ok()
        && (cfg!(feature = "physics-stage") || stage.id == "harwood-forest")
}
#[cfg(test)]
mod reload_tests {
    use super::*;
    #[test]
    fn saved_stage_restores_only_supported_available_content() {
        let mut catalog = Catalog::descriptors(&rerbr_content::Selection::default());
        let chosen = restored_stage(&catalog, "chirdonhead");
        assert_eq!(
            chosen.id,
            if cfg!(feature = "physics-stage") {
                "chirdonhead"
            } else {
                "harwood-forest"
            }
        );
        catalog.stages[1].files.checked = true;
        catalog.stages[1]
            .files
            .missing
            .push("track-22_O.col".into());
        assert_eq!(restored_stage(&catalog, "chirdonhead").id, "harwood-forest");
        assert_eq!(
            restored_stage(&catalog, "../invalid-stage").id,
            "harwood-forest"
        );
    }

    #[test]
    fn main_menu_browsing_keeps_all_four_stages_and_never_starts_scene_workers() {
        let mut app = Runtime::new_for_catalog(false);
        assert_eq!(app.menu.page, menu::Page::Title);
        app.apply_command(menu::Command::EnterMainMenu);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert!(app.worker.is_idle());
        assert!(app.pending_scene_action.is_none());
        assert!(app.pivots.is_none());
        for _ in 0..3 {
            for stage in rerbr_content::StageDescriptor::presets() {
                if !stage_supported(&stage) {
                    continue;
                }
                let selected = rerbr_content::Selection {
                    stage,
                    ..Default::default()
                };
                app.configure_selection(selected.clone()).unwrap();
                assert_eq!(app.selection(), &selected);
                assert!(app.worker.is_idle());
                assert!(app.pending_scene_action.is_none());
            }
        }
        let previous = app.selection.clone();
        let bad = rerbr_content::Selection {
            vehicle: rerbr_content::VehicleDescriptor::ford_focus_2006(),
            ..previous.clone()
        };
        assert!(app.configure_selection(bad).is_err());
        assert_eq!(app.selection, previous);
        app.race.prepare();
        let other = rerbr_content::Selection::default();
        if app.selection != other {
            assert!(app.configure_selection(other).is_err());
        }
        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.race.phase, race::Phase::Idle);
        assert!(matches!(app.take_scene_action(), Some(SceneAction::Unload)));
        assert!(app.worker.is_idle());
        assert!(app.cockpit_node.is_none() && !app.cockpit_ready);
    }

    #[test]
    fn loading_cancel_restart_and_countdown_preserve_physics_pause_and_timer() {
        let mut app = Runtime::new_for_catalog(false);
        app.apply_command(menu::Command::StartRally);
        assert_eq!(app.menu.page, menu::Page::Loading);
        assert_eq!(app.race.phase, race::Phase::Preparing);
        assert!(matches!(
            app.take_scene_action(),
            Some(SceneAction::Load { .. })
        ));

        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert_eq!(app.race.phase, race::Phase::Idle);
        assert!(matches!(app.take_scene_action(), Some(SceneAction::Unload)));

        // Cancel followed immediately by restart queues a clean fresh load.
        app.apply_command(menu::Command::StartRally);
        assert_eq!(app.menu.page, menu::Page::Loading);
        assert!(matches!(
            app.take_scene_action(),
            Some(SceneAction::Load { .. })
        ));

        // Swap to an isolated idle worker so filesystem absence cannot race this state-machine test.
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.install_pivots([Vec3::ZERO; 4]);
        app.tilt = 0.0;
        {
            let mut st = app.worker.state.lock().unwrap();
            st.rest = Some(Sample::default());
        }
        let layout = DrivingLayout::default();
        let t0 = Instant::now();
        let (_, _, paused) = app.sync_race_state(t0, 1.0 / 60.0, &layout, true);
        assert_eq!(app.menu.page, menu::Page::Driving);
        assert_eq!(app.race.phase, race::Phase::Presentation);
        assert!(paused, "physics must remain paused during presentation");
        assert_eq!(app.stage_elapsed_secs(), None);

        // Advancing past 3.5s presentation starts countdown with active physics
        let t1 = t0 + Duration::from_millis(3500);
        let (_, _, paused) = app.sync_race_state(t1, 1.0 / 60.0, &layout, true);
        assert_eq!(app.race.phase, race::Phase::Countdown);
        assert!(
            !paused,
            "physics is active during countdown so engine can rev"
        );
        assert_eq!(app.race.countdown(), Some(5));
        assert_eq!(app.stage_elapsed_secs(), None);

        // Mid-countdown pause and resume preserves exact countdown progress
        let t2 = t1 + Duration::from_secs(2);
        let (_, _, paused) = app.sync_race_state(t2, 1.0 / 60.0, &layout, true);
        assert!(!paused);
        assert_eq!(app.race.countdown(), Some(3));
        app.pause();
        assert_eq!(app.menu.page, menu::Page::Paused);
        let (_, _, paused) =
            app.sync_race_state(t2 + Duration::from_secs(50), 1.0 / 60.0, &layout, true);
        assert!(paused);
        assert_eq!(app.race.countdown(), Some(3));

        app.apply_command(menu::Command::Start);
        assert_eq!(app.menu.page, menu::Page::Driving);
        let t3 = t2 + Duration::from_secs(51);
        let (_, _, paused) = app.sync_race_state(t3, 1.0 / 60.0, &layout, true);
        assert!(!paused);
        assert_eq!(app.race.countdown(), Some(3));

        // Advancing 3 more active seconds hits GO!
        let t4 = t3 + Duration::from_secs(3);
        let (_, _, paused) = app.sync_race_state(t4, 1.0 / 60.0, &layout, true);
        assert!(!paused, "physics continues running once countdown hits GO");
        assert_eq!(app.race.phase, race::Phase::Driving);
        assert_eq!(app.stage_elapsed_secs(), Some(0.0));

        let (_, _, paused) =
            app.sync_race_state(t4 + Duration::from_secs(2), 1.0 / 60.0, &layout, true);
        assert!(!paused);
        assert_eq!(app.stage_elapsed_secs(), Some(2.0));
    }

    #[test]
    fn controls_editor_respects_panel_priority_and_drags_without_driving() {
        let mut app = Runtime::new_for_catalog(false);
        app.apply_command(menu::Command::Open(menu::Page::ControlsEditor));
        assert_eq!(app.menu.page, menu::Page::ControlsEditor);
        let rows = {
            let ctx = app.context();
            app.menu.rows(&ctx)
        };
        let next_row = app.menu.row_rect_for(0, rows.len());
        let before = app.selected_control;
        app.touch(
            winit::event::Touch {
                device_id: winit::event::DeviceId::dummy(),
                phase: TouchPhase::Started,
                location: winit::dpi::PhysicalPosition::new(
                    f64::from(next_row.center_x()) * 1024.0,
                    f64::from(next_row.y + next_row.h * 0.5) * 768.0,
                ),
                force: None,
                id: 1,
            },
            (1024, 768),
        );
        assert_ne!(app.selected_control, before);
        assert!(app.dragging_control.is_none());

        // Drag throttle without changing selected gear or driving pedals.
        let base = DrivingLayout::for_size((1024, 768));
        let throttle = app.controls_layout.apply(base).throttle;
        for (phase, nx, ny) in [
            (
                TouchPhase::Started,
                f64::from(throttle.center_x()),
                f64::from(throttle.y + throttle.h * 0.5),
            ),
            (TouchPhase::Moved, 0.5, 0.55),
            (TouchPhase::Ended, 0.5, 0.55),
        ] {
            app.touch(
                winit::event::Touch {
                    device_id: winit::event::DeviceId::dummy(),
                    phase,
                    location: winit::dpi::PhysicalPosition::new(nx * 1024.0, ny * 768.0),
                    force: None,
                    id: 2,
                },
                (1024, 768),
            );
        }
        let moved = app.controls_layout.apply(base).throttle;
        assert!((moved.center_x() - 0.5).abs() < 0.02);
        assert_eq!(app.controls.levels().throttle, 0.0);
        assert_eq!(app.controls.selected_gear, 1);

        // Even if a control overlaps the top editor panel, tapping the panel button wins by draw order.
        app.controls_layout.set_center(
            layout::EditableControl::Throttle,
            base,
            next_row.center_x(),
            next_row.y + next_row.h * 0.5,
        );
        let selected_before = app.selected_control;
        app.touch(
            winit::event::Touch {
                device_id: winit::event::DeviceId::dummy(),
                phase: TouchPhase::Started,
                location: winit::dpi::PhysicalPosition::new(
                    f64::from(next_row.center_x()) * 1024.0,
                    f64::from(next_row.y + next_row.h * 0.5) * 768.0,
                ),
                force: None,
                id: 3,
            },
            (1024, 768),
        );
        assert_ne!(app.selected_control, selected_before);
        assert!(app.dragging_control.is_none());

        app.apply_command(menu::Command::CtrlReset);
        assert_eq!(app.controls_layout.rects, [None; 6]);
    }

    #[test]
    fn stage_complete_triggers_only_on_forward_finish_with_checkpoints_and_resets_cleanly() {
        let mut app = Runtime::new_for_catalog(false);
        app.apply_command(menu::Command::StartRally);
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.install_pivots([Vec3::ZERO; 4]);
        app.tilt = 0.0;
        {
            let mut st = app.worker.state.lock().unwrap();
            st.rest = Some(Sample::default());
        }
        let layout = DrivingLayout::default();
        let t0 = Instant::now();
        app.sync_race_state(t0, 1.0 / 60.0, &layout, true);
        let t_count = t0 + Duration::from_millis(3500);
        app.sync_race_state(t_count, 1.0 / 60.0, &layout, true);
        assert_eq!(app.race.phase, race::Phase::Countdown);
        let t_go = t_count + Duration::from_secs(5);
        app.sync_race_state(t_go, 1.0 / 60.0, &layout, true);
        assert_eq!(app.race.phase, race::Phase::Driving);

        // Reaching finish distance without clearing splits or without forward crossing must NOT finish.
        {
            let mut st = app.worker.state.lock().unwrap();
            st.timing = rerbr_sim::drive::StageTiming {
                started: true,
                driveline_distance: 7150.0,
                start_distance: 65.0,
                finish_distance: 7148.0,
                splits_passed: 2,
                splits_total: 2,
                split_distances: vec![2444.0, 4890.0],
                split_times: vec![Some(45.0), Some(90.0)],
                finished: true,
                finish_crossed_forward: false,
                ..Default::default()
            };
        }
        let t_drive = t_go + Duration::from_secs(120);
        let (_, _, paused) = app.sync_race_state(t_drive, 1.0 / 60.0, &layout, true);
        assert!(!paused);
        assert_eq!(app.menu.page, menu::Page::Driving);
        assert_eq!(app.race.phase, race::Phase::Driving);

        // Now cross the finish forward with all splits validated: freezes official clock immediately
        // while keeping the car moving through a 3.0m visual rollout past the finish line.
        {
            let mut st = app.worker.state.lock().unwrap();
            st.timing.driveline_distance = 7148.5;
            st.timing.finish_crossed_forward = true;
        }
        let (_, _, paused_rollout) = app.sync_race_state(
            t_drive + Duration::from_millis(500),
            1.0 / 60.0,
            &layout,
            true,
        );
        assert!(!paused_rollout, "car keeps rolling past the finish line");
        assert_eq!(app.menu.page, menu::Page::Driving);
        assert_eq!(app.race.phase, race::Phase::Finishing);
        let result = app
            .race
            .result()
            .expect("official result frozen at crossing");
        assert!((result.driving_seconds - 120.5).abs() < 1e-6);
        assert!((result.total_seconds - 120.5).abs() < 1e-6);

        // Once the car advances 3.0m beyond finish_distance (7148.0 + 3.0 = 7151.0), rollout finishes.
        {
            let mut st = app.worker.state.lock().unwrap();
            st.timing.driveline_distance = 7151.0;
        }
        let (_, _, paused) = app.sync_race_state(
            t_drive + Duration::from_millis(620),
            1.0 / 60.0,
            &layout,
            true,
        );
        assert!(paused);
        assert_eq!(app.menu.page, menu::Page::StageComplete);
        assert_eq!(app.race.phase, race::Phase::Finished);
        assert_eq!(app.race.result(), Some(result));
        assert_eq!(result.splits_passed, 2);
        assert_eq!(result.splits_total, 2);

        // Further sync calls stay frozen at StageComplete.
        let (_, _, paused_after) =
            app.sync_race_state(t_drive + Duration::from_secs(30), 1.0 / 60.0, &layout, true);
        assert!(paused_after);
        assert_eq!(app.race.result(), Some(result));
        assert_eq!(app.stage_elapsed_secs(), Some(result.total_seconds));

        // Restart clears old result and enters Loading cleanly.
        app.apply_command(menu::Command::Restart);
        assert_eq!(app.menu.page, menu::Page::Loading);
        assert_eq!(app.race.phase, race::Phase::Preparing);
        assert_eq!(app.race.result(), None);

        // ResetToMain returns to Idle and clears result.
        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert_eq!(app.race.phase, race::Phase::Idle);
        assert_eq!(app.race.result(), None);
    }

    #[test]
    fn menu_runtime_actions_and_replay_integration_tests() {
        let mut app = Runtime::new_for_catalog(false);

        // 1. Vehicles selection audit (all vehicles in catalog: 8 original RBR + 1 NGP candidate)
        let total_vehicles = app.content.vehicles.len();
        assert_eq!(total_vehicles, 9);
        for i in 0..total_vehicles {
            let (expected_id, ok) = {
                let v = &app.content.vehicles[i].descriptor;
                (v.id.clone(), v.validate_current_backend().is_ok())
            };
            if ok {
                app.apply_command(menu::Command::SelectVehicle(i));
                assert_eq!(app.selection.vehicle.id, expected_id);
            } else {
                let prev_id = app.selection.vehicle.id.clone();
                app.apply_command(menu::Command::SelectVehicle(i));
                assert_eq!(app.selection.vehicle.id, prev_id);
            }
        }
        // Restore Corolla
        app.apply_command(menu::Command::SelectVehicle(0));
        assert_eq!(app.selection.vehicle.id, "toyota-corolla-wrc");

        // 2. Stage selection & missing files gate
        assert_eq!(app.content.stages.len(), 36);
        app.apply_command(menu::Command::SelectStage(0));
        assert_eq!(app.selection.stage.id, "harwood-forest");

        // Mark stage 1 as having missing files
        app.content.stages[1].files.checked = true;
        app.content.stages[1].files.missing = vec!["track-22_O.col".into()];
        app.apply_command(menu::Command::SelectStage(1));
        assert_eq!(app.selection.stage.id, "harwood-forest");

        // Attempting StartRally with unsupported vehicle is blocked
        app.apply_command(menu::Command::EnterMainMenu);
        app.selection.vehicle = rerbr_content::VehicleDescriptor::ford_focus_2006();
        app.apply_command(menu::Command::StartRally);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert!(app.message.is_some());

        // Restore valid vehicle
        app.selection.vehicle = app.content.vehicles[0].descriptor.clone();
        app.message = None;

        // 3. Start race, finish, open replay, exit replay to StageComplete
        app.apply_command(menu::Command::StartRally);
        assert_eq!(app.menu.page, menu::Page::Loading);
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.install_pivots([Vec3::ZERO; 4]);
        app.tilt = 0.0;
        {
            let mut st = app.worker.state.lock().unwrap();
            st.rest = Some(Sample::default());
            st.timing = rerbr_sim::drive::StageTiming {
                started: true,
                driveline_distance: 7151.5,
                start_distance: 65.0,
                finish_distance: 7148.0,
                splits_passed: 2,
                splits_total: 2,
                split_distances: vec![2444.0, 4890.0],
                split_times: vec![Some(45.0), Some(90.0)],
                finished: true,
                finish_crossed_forward: true,
                sim_elapsed_seconds: 135.420,
                official_finish_seconds: Some(135.420),
            };
        }
        let layout = DrivingLayout::default();
        let t0 = Instant::now();
        app.replay_recorder
            .record(0.0, [Mat4::IDENTITY; 5], 0.0, 0.0, 1000.0, 1);
        app.replay_recorder
            .record(1.0, [Mat4::IDENTITY; 5], 100.0, 20.0, 3000.0, 2);
        app.race.phase = race::Phase::Driving;
        app.menu.page = menu::Page::Driving;
        app.sync_race_state(t0, 1.0 / 60.0, &layout, true);
        assert_eq!(app.menu.page, menu::Page::StageComplete);
        assert!(app.saved_replay.is_some());

        // Open replay
        app.apply_command(menu::Command::StartReplay);
        assert_eq!(app.menu.page, menu::Page::Replay);
        assert!(app.replay_player.is_some());

        // Cycle replay cameras
        app.apply_command(menu::Command::ReplayCycleCamera);
        app.apply_command(menu::Command::ReplayTogglePause);
        app.apply_command(menu::Command::ReplayRestart);

        // Exit replay returns to StageComplete
        app.apply_command(menu::Command::ReplayExit);
        assert_eq!(app.menu.page, menu::Page::StageComplete);
        assert!(app.replay_player.is_none());

        // 4. Settings modification and audio
        let initial_vol = app.volume;
        app.apply_command(menu::Command::VolumeDown);
        assert!((app.volume - (initial_vol - 0.1).max(0.0)).abs() < 1e-6);

        let initial_music_vol = app.music_volume;
        app.apply_command(menu::Command::MusicVolumeDown);
        assert!((app.music_volume - (initial_music_vol - 0.1).max(0.0)).abs() < 1e-6);
        app.apply_command(menu::Command::MusicVolumeUp);
        assert!((app.music_volume - initial_music_vol).abs() < 1e-6);

        // ResetToMain from StageComplete
        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.menu.page, menu::Page::Main);
    }

    #[test]
    fn ngp_experimental_toggle_isolates_focus_and_returns_to_main_cleanly() {
        let mut app = Runtime::new_for_catalog(false);
        app.ngp_experimental = false;
        let focus = rerbr_content::VehicleDescriptor::ford_focus_2006();

        // In default stable mode, Focus is strictly rejected.
        assert!(app
            .configure_selection(rerbr_content::Selection {
                vehicle: focus.clone(),
                ..app.selection.clone()
            })
            .is_err());

        // Enabling experimental NGP allows selecting Focus & populates 9 diagnostics stages.
        app.apply_command(menu::Command::ToggleExperimentalMode);
        assert!(app.ngp_experimental);
        assert!(app
            .configure_selection(rerbr_content::Selection {
                vehicle: focus,
                ..app.selection.clone()
            })
            .is_ok());

        app.apply_command(menu::Command::RunFocusDiagnostics);
        assert_eq!(app.menu.page, menu::Page::NgpExperimental);
        let report = app.focus_diagnostics.as_ref().unwrap();
        assert_eq!(report.steps.len(), 10);
        assert_eq!(
            report.steps[8].status,
            ngp_experimental::DiagnosticStatus::Unsupported
        );

        // Disabling experimental mode while Focus is selected reverts safely to Toyota Corolla WRC.
        app.apply_command(menu::Command::ToggleExperimentalMode);
        assert!(!app.ngp_experimental);
        assert_eq!(app.selection.vehicle.id, "toyota-corolla-wrc");

        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.menu.page, menu::Page::Main);
    }

    #[test]
    fn school_briefing_drive_verdict_and_replay_flow_works_end_to_end() {
        let mut app = Runtime::new_for_catalog(false);
        app.school.entries[2].assets_present = true;
        app.school.selected = 2;

        // 1. Open lesson -> Loading
        app.apply_command(menu::Command::SchoolStartLesson);
        assert_eq!(app.menu.page, menu::Page::Loading);
        assert_eq!(app.active_school_lesson, Some(2));

        // Simulate settled school session + loaded scenery
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.install_pivots([Vec3::ZERO; 4]);
        app.tilt = 0.0;
        {
            let mut st = app.worker.state.lock().unwrap();
            st.rest = Some(Sample::default());
        }
        let layout = DrivingLayout::default();
        app.sync_race_state(Instant::now(), 1.0 / 60.0, &layout, true);
        assert_eq!(app.menu.page, menu::Page::SchoolBriefing);

        // 2. Confirm briefing -> Driving immediately (no quick-race countdown)
        app.apply_command(menu::Command::SchoolConfirmStart);
        assert_eq!(app.menu.page, menu::Page::Driving);
        assert_eq!(app.race.phase, race::Phase::Driving);
        assert!(app.worker.state.lock().unwrap().school_start_requested);

        app.replay_recorder
            .record(0.0, [Mat4::IDENTITY; 5], 0.0, 0.0, 1000.0, 1);
        app.replay_recorder
            .record(1.0, [Mat4::IDENTITY; 5], 100.0, 20.0, 3000.0, 2);

        // 3. SchoolRun reaches verdict -> SchoolResult & persisted grade
        {
            let mut st = app.worker.state.lock().unwrap();
            st.school_state = Some(rerbr_sim::drive::SchoolSessionSnapshot {
                lesson_index: 2,
                started: true,
                untimed: false,
                ride: false,
                ended: Some(rerbr_sim::drive::SchoolOutcome::Finished),
                elapsed_seconds: 9.5,
                perfect_time: 9.0,
                pass_time: 10.0,
                time_limit: 11.0,
                target_distance: Some(160.0),
                verdict: 3,
                failure_reasons: Vec::new(),
            });
        }
        app.sync_race_state(Instant::now(), 1.0 / 60.0, &layout, true);
        assert_eq!(app.menu.page, menu::Page::SchoolResult);
        let res = app.active_school_result.as_ref().unwrap();
        assert_eq!(res.verdict, 3);
        assert_eq!(app.school.results[0..=2], [3, 3, 3]);

        // 4. Open replay and return cleanly to SchoolResult
        app.apply_command(menu::Command::StartReplay);
        assert_eq!(app.menu.page, menu::Page::Replay);
        app.apply_command(menu::Command::ReplayExit);
        assert_eq!(app.menu.page, menu::Page::SchoolResult);

        // 5. Exit cleanly to Main
        app.apply_command(menu::Command::ResetToMain);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert!(app.active_school_lesson.is_none());
    }

    #[test]
    fn diagnostics_retain_focus_session_across_reset_to_main_without_corolla_leak() {
        let mut app = Runtime::new_for_catalog(false);
        app.ngp_experimental = true;
        let focus = rerbr_content::VehicleDescriptor::ford_focus_2006();
        app.configure_selection(rerbr_content::Selection {
            vehicle: focus.clone(),
            ..app.selection.clone()
        })
        .unwrap();
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.install_pivots([Vec3::ZERO; 4]);
        app.tilt = 0.0;
        {
            let mut st = app.worker.state.lock().unwrap();
            let mut sample = Sample {
                step: 1440,
                rpm: 6800.0,
                gear: 3,
                velocity: [0.0, 0.0, 20.0],
                ..Sample::default()
            };
            sample.throttle_received = 1.0;
            st.rest = Some(sample.clone());
            st.pair = Some((sample.clone(), sample, Instant::now()));
            st.physics_status = Some("NGP 7.4 FOCUS READY".into());
        }
        app.race.phase = race::Phase::Driving;
        app.menu.page = menu::Page::Driving;
        let layout = DrivingLayout::default();
        app.sync_race_state(Instant::now(), 1.0 / 60.0, &layout, true);

        // Exit to main menu (destroys worker); retained snapshot must preserve Focus telemetry
        app.apply_command(menu::Command::ResetToMain);
        app.refresh_focus_diagnostics();
        let report = app.focus_diagnostics.as_ref().unwrap();
        assert_eq!(report.vehicle_id, focus.id);
        let export = report.export_text(&[]);
        assert!(export.contains("ÚLTIMA SESIÓN"));
        assert!(export.contains("STEP 1440"));
        assert!(export.contains(&focus.model));
    }

    #[cfg(feature = "physics-stage")]
    #[test]
    fn presentation_cinematic_never_forces_cockpit_interior_geometry() {
        let mut app = Runtime::new_for_catalog(false);
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.view = cameras::View::CockpitFallback;
        app.cockpit_ready = true;
        app.mounts = Some(cameras::Mounts {
            bonnet: cameras::Mount {
                eye: Vec3::new(0.0, 1.0, 0.2),
                target: Vec3::new(0.0, 1.0, -3.0),
            },
            cockpit: cameras::Mount {
                eye: Vec3::new(-0.35, 1.0, 0.1),
                target: Vec3::new(-0.35, 1.0, -3.0),
            },
        });
        app.menu.page = menu::Page::Driving;
        app.race.start_presentation(Instant::now());
        assert!(
            !app.draws_interior(),
            "exterior presentation cinematic must render full exterior car (bumpers + wheels)"
        );

        app.race.ready(Instant::now());
        assert!(
            app.draws_interior(),
            "once countdown/driving begins in CockpitFallback with loaded cockpit, interior renders"
        );

        app.menu.page = menu::Page::Replay;
        assert!(
            !app.draws_interior(),
            "exterior replay cameras must never hide bumpers or wheels behind CockpitFallback"
        );
    }

    #[test]
    fn start_rally_from_ngp_page_diagnoses_selected_vehicle_and_preserves_visual_only_reason() {
        let mut app = Runtime::new_for_catalog(false);
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.ngp_experimental = true;

        let mut yaris = rerbr_content::VehicleDescriptor::ford_focus_2006();
        yaris.id = "rsf-toyota-gr-yaris-rally2".into();
        yaris.name = "Toyota GR Yaris Rally2".into();
        yaris.model = "Cars/yaris_rally2/yaris_rally2.sgc".into();
        yaris.rsf_pack = Some("Toyota_GR_Yaris_Rally2_ngp7".into());

        app.configure_selection(rerbr_content::Selection {
            vehicle: yaris.clone(),
            ..app.selection.clone()
        })
        .unwrap();

        // Simulate being on NgpExperimental while inspected_vehicle points to index 0 (Corolla),
        // not the selected Yaris.
        app.menu.page = menu::Page::NgpExperimental;
        app.menu.inspected_vehicle = 0;

        app.apply_command(menu::Command::StartRally);
        assert_eq!(
            app.focus_diagnostics
                .as_ref()
                .map(|r| r.vehicle_id.as_str()),
            Some("rsf-toyota-gr-yaris-rally2"),
            "StartRally must diagnose selection.vehicle, never menu.inspected_vehicle"
        );
        // Without RERBR_PROBE_DOCUMENTS, StartRally blocks cleanly with an explicit reason
        // instead of silently substituting another vehicle.
        assert!(
            app.message
                .as_deref()
                .is_some_and(|m| m.starts_with("NGP BLOQUEADO")),
            "expected explicit NGP BLOQUEADO message, got {:?}",
            app.message
        );

        // Simulate runtime NGP boot error during Loading: sync_race_state must enter
        // visual-only mode AND preserve the exact error in experimental_physics_reason
        // across Loading -> Driving (never wiping it).
        app.menu.show(menu::Page::Loading);
        app.experimental_visual_only = false;
        app.experimental_physics_reason = None;
        {
            let mut st = app.worker.state.lock().unwrap();
            st.error = Some("CAR PACK: PAQUETE PRESENTE PERO INCOMPLETO".into());
        }
        let layout = DrivingLayout::default();
        app.install_pivots([Vec3::ZERO; 4]);
        app.sync_race_state(Instant::now(), 1.0 / 60.0, &layout, true);
        assert_eq!(app.menu.page, menu::Page::Driving);
        assert!(app.experimental_visual_only);
        assert!(
            app.experimental_physics_reason
                .as_deref()
                .is_some_and(|r| r.contains("INCOMPLETO")),
            "Loading -> Driving must preserve experimental_physics_reason, got {:?}",
            app.experimental_physics_reason
        );
    }

    #[test]
    fn title_screen_handles_missing_assets_and_transitions_cleanly_to_horizontal_main() {
        let mut app = Runtime::new_for_catalog(false);
        assert_eq!(app.menu.page, menu::Page::Title);
        // Without private assets installed, showroom_request is None and never panics or blocks.
        app.title_vehicle_order.clear();
        assert!(app.showroom_request().is_none());

        app.apply_command(menu::Command::EnterMainMenu);
        assert_eq!(app.menu.page, menu::Page::Main);
        assert!(app.transition_started.is_some());

        // Online remains clearly labeled as coming soon without inventing unimplemented systems.
        app.apply_command(menu::Command::MainSectionActivate(2)); // Online
        assert_eq!(app.menu.page, menu::Page::Main);
        assert!(app.coming_soon_notice.is_some());

        // Carrera opens pilot creation when no profile exists yet.
        app.career = career::CareerStore::default();
        app.apply_command(menu::Command::MainSectionActivate(0)); // Carrera
        assert_eq!(app.menu.page, menu::Page::CareerCreate);
        app.apply_command(menu::Command::Back);
        assert_eq!(app.menu.page, menu::Page::Main);

        // Salón 0KM (index 4) and 2ª Mano (index 5) open their respective 3D Showroom Dealerships.
        app.apply_command(menu::Command::MainSectionActivate(4)); // Salón 0KM
        assert_eq!(app.menu.page, menu::Page::NewDealership);
        app.apply_command(menu::Command::Back);
        assert_eq!(app.menu.page, menu::Page::Main);
        app.apply_command(menu::Command::MainSectionActivate(5)); // 2ª Mano
        assert_eq!(app.menu.page, menu::Page::Dealership);
        app.apply_command(menu::Command::Back);
        assert_eq!(app.menu.page, menu::Page::Main);

        // Contrarreloj opens the working stage/driving hub.
        app.apply_command(menu::Command::MainSectionActivate(1));
        assert_eq!(app.menu.page, menu::Page::Drive);
        app.apply_command(menu::Command::Back);
        assert_eq!(app.menu.page, menu::Page::Main);

        // Garage opens the 3D Showroom Garage.
        app.apply_command(menu::Command::MainSectionActivate(3));
        assert_eq!(app.menu.page, menu::Page::Garage);
    }

    #[test]
    fn career_runtime_flow_create_buy_garage_and_championship_keeps_time_trial_free() {
        let mut app = Runtime::new_for_catalog(false);
        app.worker.shutdown();
        app.worker = worker::Worker::idle();
        app.career = career::CareerStore::default();
        // 1. Time Trial can select Corolla WRC freely without owning any career cars.
        let corolla_idx = app
            .content
            .vehicles
            .iter()
            .position(|v| v.descriptor.id == "toyota-corolla-wrc")
            .unwrap();
        app.apply_command(menu::Command::SelectVehicle(corolla_idx));
        assert_eq!(app.selection.vehicle.id, "toyota-corolla-wrc");
        for v in &mut app.content.vehicles {
            v.has_3d_model = true;
        }

        // 2. Enter Career -> create pilot -> receives US$15,000 and 0 cars.
        app.apply_command(menu::Command::EnterMainMenu);
        app.apply_command(menu::Command::MainSectionActivate(0));
        assert_eq!(app.menu.page, menu::Page::CareerCreate);
        app.apply_command(menu::Command::CareerPilotNext);
        app.apply_command(menu::Command::CareerCreateConfirm);
        assert_eq!(app.menu.page, menu::Page::CareerHub);
        assert_eq!(app.career.profile.as_ref().unwrap().cash_usd, 15_000);
        assert!(app
            .career
            .profile
            .as_ref()
            .unwrap()
            .owned_vehicle_ids
            .is_empty());

        // 3. Open Dealership -> initiation filter surfaces MG ZR S1600 first -> 2-step buy.
        app.apply_command(menu::Command::Open(menu::Page::Dealership));
        assert_eq!(app.menu.page, menu::Page::Dealership);
        let zr_idx = app
            .content
            .vehicles
            .iter()
            .position(|v| v.descriptor.id == "mg-zr-s1600")
            .unwrap();
        assert_eq!(app.menu.dealership_vehicle, zr_idx);
        let (mode, primary, _) = app.showroom_request().expect("dealership 3D showroom");
        assert_eq!(mode, crate::car::showroom::ShowroomMode::Garage);
        assert_eq!(primary.id, "mg-zr-s1600");

        // Request buy does NOT deduct funds until explicitly confirmed!
        app.apply_command(menu::Command::DealershipRequestBuy);
        assert_eq!(app.pending_buy_vehicle_id.as_deref(), Some("mg-zr-s1600"));
        assert_eq!(app.career.profile.as_ref().unwrap().cash_usd, 15_000);

        // Confirm buy deducts US$12,500 and adds MG ZR S1600 to Career Garage.
        app.apply_command(menu::Command::DealershipConfirmBuy);
        assert!(app.pending_buy_vehicle_id.is_none());
        assert_eq!(app.career.profile.as_ref().unwrap().cash_usd, 2_500);
        assert!(app
            .career
            .profile
            .as_ref()
            .unwrap()
            .owns_vehicle("mg-zr-s1600"));

        // 4. Career Garage shows ONLY owned vehicles (1 car), separate from General Garage.
        app.apply_command(menu::Command::Open(menu::Page::CareerGarage));
        let ctx = app.context();
        assert_eq!(app.menu.career_garage_indices(&ctx).len(), 1);
        assert!(app.content.vehicles.len() >= 8);

        // 5. Start Regional Championship with MG ZR S1600 on Intermediate difficulty.
        app.apply_command(menu::Command::Open(menu::Page::CareerChampionship));
        app.apply_command(menu::Command::CareerDiffNext);
        assert_eq!(
            app.selected_career_difficulty,
            career::CareerDifficulty::Intermediate
        );
        app.apply_command(menu::Command::CareerChampStart);
        let champ = app
            .career
            .profile
            .as_ref()
            .unwrap()
            .active_championship
            .as_ref()
            .expect("active regional championship");
        assert_eq!(champ.locked_vehicle_id, "mg-zr-s1600");
        assert_eq!(champ.difficulty, career::CareerDifficulty::Intermediate);
        assert_eq!(champ.total_stages(), 3);

        // 6. Verify Regional Championship stage start is NOT blocked by tilt == NaN
        // when the iPad is in landscape (drawable_size.0 > drawable_size.1) before any
        // 3D stage ChasePath has been loaded.
        app.tilt = f64::NAN;
        app.gamepad.connected = false;
        app.drawable_size = (1024, 768);
        assert!(app.is_landscape_ready());
        let champ_rows = {
            let ctx = app.context();
            assert!(ctx.steering_ready);
            app.menu.rows(&ctx)
        };
        assert!(
            !champ_rows
                .iter()
                .any(|r| r.label.contains("SUJETA EN HORIZONTAL")),
            "Must NOT display 'SUJETA EN HORIZONTAL PARA CORRER' when iPad is in landscape"
        );
        let run_row = champ_rows
            .iter()
            .find(|r| r.label.starts_with("CORRER ETAPA 1/3:"))
            .expect("CORRER ETAPA 1/3 row must exist");
        assert_eq!(
            run_row.kind,
            menu::RowKind::Action {
                command: menu::Command::CareerChampRunNextStage,
                enabled: true,
            }
        );

        // Conversely, if held in portrait (768x1024) with no tilt and no gamepad, it blocks cleanly.
        app.drawable_size = (768, 1024);
        assert!(!app.is_landscape_ready());
        let portrait_rows = {
            let ctx = app.context();
            assert!(!ctx.steering_ready);
            app.menu.rows(&ctx)
        };
        assert!(portrait_rows
            .iter()
            .any(|r| r.label == "SUJETA EN HORIZONTAL PARA CORRER"));

        // Restore landscape and verify Settings 5-card navigation works end-to-end.
        app.drawable_size = (1024, 768);
        app.menu.open(menu::Page::Settings);
        for (idx, expected_page) in [
            (0usize, menu::Page::Controls),
            (1usize, menu::Page::SoundSettings),
            (2usize, menu::Page::HudCameraSettings),
            (3usize, menu::Page::About),
            (4usize, menu::Page::Content),
        ] {
            app.apply_command(menu::Command::SettingsCardActivate(idx));
            assert_eq!(app.menu.page, expected_page);
            app.apply_command(menu::Command::Back);
            assert_eq!(app.menu.page, menu::Page::Settings);
        }
    }

    #[test]
    fn main_menu_remembers_last_driven_car_separately_from_random_title_car() {
        let mut app = Runtime::new_for_catalog(false);
        for v in &mut app.content.vehicles {
            v.has_3d_model = true;
        }
        let impreza = app.content.vehicles[5].descriptor.clone();
        let corolla = app.content.vehicles[0].descriptor.clone();
        // Title screen shows a random car (e.g. Impreza), independent of last-driven Corolla.
        app.title_vehicle_order = vec![impreza.clone(), corolla.clone()];
        app.last_driven_vehicle_id = Some(corolla.id.clone());
        app.selection.vehicle = corolla.clone();

        // On Title Screen -> showcases random title car (Impreza).
        let (mode, primary, _) = app.showroom_request().expect("title showroom");
        assert_eq!(mode, crate::car::showroom::ShowroomMode::Title);
        assert_eq!(primary.id, impreza.id);

        // Entering Main Menu -> switches to last-driven car (Corolla), NOT random title car.
        app.apply_command(menu::Command::EnterMainMenu);
        let (mode, primary, _) = app.showroom_request().expect("main showroom");
        assert_eq!(mode, crate::car::showroom::ShowroomMode::MainMenu);
        assert_eq!(primary.id, corolla.id);

        // If last_driven_vehicle_id is uninstalled/missing, falls back cleanly to a valid model.
        app.last_driven_vehicle_id = Some("rsf-deleted-car".into());
        let (_, fallback_primary, _) = app.showroom_request().expect("fallback main showroom");
        assert_eq!(fallback_primary.id, corolla.id);
    }

    #[test]
    fn visual_only_car_is_viewable_in_garage_but_strictly_blocked_from_driving() {
        let mut app = Runtime::new_for_catalog(false);
        let mut visual_desc = rerbr_content::VehicleDescriptor::ford_focus_2006();
        visual_desc.id = "rsf-alpine-a110-2020".into();
        visual_desc.name = "Alpine A110 2020".into();
        visual_desc.model = "Cars/Alpine_A110_2020/Alpine_A110_2020.sgc".into();
        visual_desc.rsf_pack = None;
        let idx = app.content.vehicles.len();
        app.content.vehicles.push(content_manager::VehicleEntry {
            descriptor: visual_desc.clone(),
            state: content_manager::EntryState::Unsupported,
            files: content_manager::FileAudit::unchecked(),
            tech_sheet: None,
            classification: rerbr_content::VehicleClassification::VisualInspectionOnly,
            year: Some(2020),
            drivetrain: None,
            fmod_bank: None,
            using_fallback_model: false,
            model_missing: false,
            has_3d_model: true,
            notes: vec!["SOLO VISUALIZACIÓN 3D".into()],
        });

        assert_eq!(
            app.content.vehicles[idx].capability(true),
            content_manager::DriveCapability::VisualOnly
        );
        assert!(app.content.vehicles[idx].can_showroom_view());

        // Inspecting in 3D Garage serves its real 3D model.
        app.menu.open(menu::Page::Garage);
        app.menu.inspected_vehicle = idx;
        let (mode, primary, fallbacks) = app.showroom_request().expect("garage visual car");
        assert_eq!(mode, crate::car::showroom::ShowroomMode::Garage);
        assert_eq!(primary.id, "rsf-alpine-a110-2020");
        assert!(fallbacks.is_empty());

        // Attempting to select it for driving MUST be rejected without changing active car.
        let prev_car = app.selection.vehicle.id.clone();
        app.apply_command(menu::Command::SelectVehicle(idx));
        assert_eq!(app.selection.vehicle.id, prev_car);
        assert!(app
            .message
            .as_deref()
            .is_some_and(|m| m.contains("SOLO VISUALIZACIÓN")));
    }

    #[test]
    fn showroom_camera_orbit_clamps_pitch_and_keeps_finite_bounds() {
        let mut cam = crate::car::showroom::ShowroomCamera::default();
        cam.set_mode(crate::car::showroom::ShowroomMode::Garage);
        cam.drag(0.35, 10.0); // Extreme pitch drag
        assert!(cam.pitch <= 0.64 && cam.pitch >= 0.10);
        cam.drag(-0.5, -10.0);
        assert!(cam.pitch <= 0.64 && cam.pitch >= 0.10);
        cam.end_drag();
        let (c, fov) = cam.camera_and_fov(0.72, 2.2, 2556.0 / 1179.0);
        assert!(c.eye.is_finite() && c.target.is_finite() && fov > 30.0 && fov < 60.0);
        assert!(c.eye.y >= 0.35);
    }

    #[test]
    fn clio_rally5_and_rally4_load_distinct_ngp_packs_and_rpm_hud_styles() {
        let transfer_rbr = std::path::Path::new(
            r"C:\Users\adria\Desktop\RBR Mobile - Autos Listos para iPad\LISTOS PARA CONDUCIR\RBRRRRR",
        );
        if transfer_rbr.join("rsfdata/cache/cars.json").is_file() {
            let discovered = rsf_catalog::discover(transfer_rbr);
            let r5 = discovered
                .iter()
                .find(|d| d.descriptor.id == "rsf-renault-clio-rally5")
                .expect("Clio Rally5 discovered");
            let r4 = discovered
                .iter()
                .find(|d| d.descriptor.id == "rsf-renault-clio-rally4")
                .expect("Clio Rally4 discovered");
            assert_eq!(
                r5.descriptor.rsf_pack.as_deref(),
                Some("Renault_Clio_Rally5_ngp6")
            );
            assert_eq!(
                r4.descriptor.rsf_pack.as_deref(),
                Some("Renault_Clio_Rally4_ngp6")
            );
            assert_ne!(r5.fmod_bank, r4.fmod_bank);
            // Both share the RSF 3D model, yet their NGP packs & tech sheets stay isolated.
            assert_eq!(r5.descriptor.model, r4.descriptor.model);
            let s5 = r5.tech_sheet.as_ref().expect("Rally5 tech sheet");
            let s4 = r4.tech_sheet.as_ref().expect("Rally4 tech sheet");
            assert_ne!(s5.profile, s4.profile);
        }

        // Verify both RPM HUD styles render cleanly within vertex budget and respect vehicle RPM spec.
        use crate::harwood::benchmark::hud::{Canvas, MAX_CANVAS_VERTICES};
        let layout = DrivingLayout::for_size((1024, 768));
        let sample = rerbr_sim::diagnostics::Sample {
            rpm: 6750.0,
            gear: 4,
            ..Default::default()
        };
        let spec = VehicleRpmSpec {
            redline_rpm: Some(7200.0),
            optimal_shift_rpm: Some(6500.0),
            scale_max_rpm: 7200.0,
        };
        for style in [
            menu::HudRpmStyle::HorizontalBar,
            menu::HudRpmStyle::ShiftLights,
        ] {
            let mut canvas = Canvas::new((1024, 768));
            draw_cluster_with_rpm_hud(&mut canvas, &layout, &sample, &spec, style, false);
            draw_cockpit_instruments(&mut canvas, &layout, &sample, &spec);
            let bytes = canvas.finish();
            assert!(!bytes.is_empty());
            assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
        }
    }
}
#[test]
#[ignore = "requires real RSF Documents and the supported NGP runtime"]
fn edition_filters_private_catalog_across_vehicle_routes() {
    let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").expect("set RERBR_PROBE_DOCUMENTS");
    let raw = Catalog::scan_root(
        std::path::Path::new(&root),
        &rerbr_content::Selection::default(),
    );
    let visual = raw
        .vehicles
        .iter()
        .find(|entry| entry.capability(true) == content_manager::DriveCapability::VisualOnly)
        .expect("visual-only fixture")
        .descriptor
        .clone();
    let mut app = Runtime::new();
    assert_eq!(app.content.is_public_beta(), PUBLIC_BETA);
    assert!(app.content.vehicle("rsf-citroen-ds3-wrc").is_some());
    let mut profile = career::CareerProfile::create("Catalog QA").unwrap();
    profile.owned_vehicle_ids = vec!["rsf-citroen-ds3-wrc".into(), visual.id.clone()];
    app.career.profile = Some(profile);
    let garage = app.menu.career_garage_indices(&app.context());
    assert!(garage
        .iter()
        .any(|&i| app.content.vehicles[i].descriptor.id == "rsf-citroen-ds3-wrc"));
    assert_eq!(
        garage
            .iter()
            .any(|&i| app.content.vehicles[i].descriptor.id == visual.id),
        !PUBLIC_BETA
    );
    let check = |entry: &content_manager::VehicleEntry| {
        if PUBLIC_BETA {
            assert_eq!(
                entry.capability(true),
                content_manager::DriveCapability::Drivable
            );
            assert!(entry.files.checked && entry.files.missing.is_empty());
        }
    };
    for index in app.menu.garage_indices(&app.context()) {
        check(&app.content.vehicles[index]);
    }
    for kind in [
        career::DealershipKind::UsedCars,
        career::DealershipKind::NewSalon,
    ] {
        for index in app.menu.dealership_indices_for(&app.context(), kind) {
            check(&app.content.vehicles[index]);
        }
    }
    for page in [menu::Page::Title, menu::Page::Main] {
        app.menu.page = page;
        let (_, primary, fallbacks) = app.showroom_request().expect("installed vehicles");
        for car in std::iter::once(primary).chain(fallbacks) {
            check(app.content.vehicle(&car.id).unwrap());
        }
    }
    println!(
        "public_beta={PUBLIC_BETA} visible={} raw={}",
        app.content.browse_vehicle_count(),
        raw.browse_vehicle_count()
    );
    if PUBLIC_BETA {
        assert!(app.content.browse_vehicle_count() < raw.browse_vehicle_count());
        assert!(app.content.vehicle(&visual.id).is_none());
        app.last_driven_vehicle_id = Some(visual.id.clone());
        assert_ne!(app.showroom_request().unwrap().1.id, visual.id);
        let stage = app.selection.stage.clone();
        assert!(app
            .configure_selection(rerbr_content::Selection {
                vehicle: visual.clone(),
                stage
            })
            .is_err());
        app.apply_command(menu::Command::StartFocusVisualInspection);
        assert!(app.take_scene_action().is_none());
        app.selection.vehicle = visual;
        app.apply_command(menu::Command::StartRally);
        assert!(app.take_scene_action().is_none());
    } else {
        assert_eq!(
            app.content.browse_vehicle_count(),
            raw.browse_vehicle_count()
        );
        let index = app
            .content
            .vehicles
            .iter()
            .position(|entry| entry.descriptor.id == visual.id)
            .unwrap();
        app.apply_command(menu::Command::OpenVehicle(index));
        assert_eq!(app.menu.page, menu::Page::GarageDetail);
    }
}
