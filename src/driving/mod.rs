//! Original physics-owned driving presentation, menus and session controls.
pub mod audio;
pub mod cameras;
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
use content_manager::Catalog;
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
    pub camera_overrides: cameras::CameraOverrides,
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
    pub focus_diagnostics: Option<ngp_experimental::FocusDiagnosticsReport>,
    pub experimental_visual_only: bool,
    pub visual_start_pose: Option<(Vec3, Vec3)>,
    pub gamepad: gamepad::GamepadRuntime,
    gamepad_started: Instant,
    gamepad_frame: gamepad::GamepadDrivingFrame,
    gamepad_polled_this_frame: bool,
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
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
impl Runtime {
    pub fn new() -> Self {
        let settings = menu::Settings::load();
        let default_selection = rerbr_content::Selection::default();
        let scanned = Catalog::scan(&default_selection);
        let stage = restored_stage(&scanned, &settings.stage_id);
        let vehicle = restored_vehicle(&scanned, &settings.vehicle_id, settings.ngp_experimental);
        let selection = rerbr_content::Selection { vehicle, stage };
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
            camera_overrides: cameras::CameraOverrides::load(),
            controls_layout: layout::ControlsLayoutOverrides::load(),
            selected_control: layout::EditableControl::Brake,
            dragging_control: None,
            drawable_size: (1024, 768),
            effective_fov: 65.0,
            worker: worker::Worker::idle(),
            content: Catalog::scan(&selection),
            selection,
            pending_scene_action: None,
            race: race::Clock::default(),
            loading_started: Instant::now(),

            controls,
            clutch_help: settings.clutch_help,
            diagnostics: settings.diagnostics,
            minimal_hud: settings.minimal_hud,
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
            ngp_experimental: settings.ngp_experimental,
            focus_diagnostics: None,
            experimental_visual_only: false,
            visual_start_pose: None,
            gamepad: gamepad::GamepadRuntime::load(),
            gamepad_started: Instant::now(),
            gamepad_frame: gamepad::GamepadDrivingFrame::default(),
            gamepad_polled_this_frame: false,
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
        }
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
    pub fn install(&mut self, geometry: &Geometry) {
        self.install_pivots(geometry.pivots);
    }
    pub(crate) fn install_pivots(&mut self, pivots: [Vec3; 4]) {
        self.pivots = Some(pivots);
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
    pub fn selection(&self) -> &rerbr_content::Selection {
        &self.selection
    }
    fn clear_scene_pose(&mut self) {
        self.pivots = None;
        self.offset = None;
        self.camera = None;
        self.scene_error = None;
        self.cockpit_node = None;
        self.cockpit_ready = false;
        self.cockpit_error = None;
        self.experimental_visual_only = false;
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
        self.controls.configure_vehicle(&selection.vehicle)?;
        self.selection = selection;
        self.content = Catalog::scan(&self.selection);
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
            invert_pedals: self.controls.invert_pedals,
            clutch_help: self.clutch_help,
            volume: self.volume,
            music_volume: self.music_volume,
            codriver_volume: self.codriver_volume,
            camera: self.camera_index(),
            vehicle_id: self.selection.vehicle.id.clone(),
            stage_id: self.selection.stage.id.clone(),
            ngp_experimental: self.ngp_experimental,
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
            Open(page) => {
                if page == menu::Page::NgpExperimental {
                    self.refresh_focus_diagnostics();
                }
                self.menu.open(page);
            }
            Back => self.menu.back(),
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
                self.pending_scene_action = Some(SceneAction::Unload);
                self.menu.reset_to_main();
                self.message = None;
                audio::set_menu_music_active(true);
            }
            StartRally => {
                self.content = Catalog::scan(&self.selection);
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
                    self.refresh_focus_diagnostics();
                    let can_visual = self
                        .focus_diagnostics
                        .as_ref()
                        .is_some_and(|r| r.can_enter_visual_mode());
                    if !can_visual {
                        self.message = Some("FOCUS NGP BLOQUEADO - REVISA EL DIAGNÓSTICO".into());
                        self.menu.open(menu::Page::NgpExperimental);
                    } else {
                        let pack_path = self
                            .focus_diagnostics
                            .as_ref()
                            .and_then(|r| r.can_boot_ngp_pack().cloned());
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
                        if let Some(pack) = pack_path {
                            self.experimental_visual_only = false;
                            self.worker = worker::Worker::new_experimental_pack_with_aids(
                                self.selection.clone(),
                                pack,
                                false,
                                self.clutch_help,
                            );
                        } else {
                            self.experimental_visual_only = true;
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
                    self.worker = worker::Worker::new_selected_with_aids(
                        self.selection.clone(),
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
            }
            Start => {
                let state = self.worker.state.lock().unwrap();
                let worker_ok = self.experimental_visual_only
                    || (state.error.is_none() && state.rest.is_some());
                if !matches!(
                    self.race.phase,
                    race::Phase::Finishing | race::Phase::Finished
                ) && self.pivots.is_some()
                    && (self.experimental_visual_only
                        || self.tilt.is_finite()
                        || self.gamepad.connected)
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
                self.pause();
                self.capture_session_snapshot(false);
                self.worker.shutdown();
                self.active_school_result = None;
                self.school_incident_banner = None;
                if self.selection.vehicle.is_ngp_experimental_car() {
                    self.refresh_focus_diagnostics();
                    if let Some(pack) = self
                        .focus_diagnostics
                        .as_ref()
                        .and_then(|r| r.can_boot_ngp_pack().cloned())
                    {
                        self.experimental_visual_only = false;
                        self.worker = worker::Worker::new_experimental_pack_with_aids(
                            self.selection.clone(),
                            pack,
                            false,
                            self.clutch_help,
                        );
                    } else {
                        self.experimental_visual_only = true;
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
                    self.worker = worker::Worker::new_selected_with_aids(
                        self.selection.clone(),
                        false,
                        self.clutch_help,
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
                let total = self.content.vehicles.len().div_ceil(9);
                if total > 0 {
                    self.menu.garage_page = (self.menu.garage_page + 1) % total;
                }
            }
            PrevGaragePage => {
                let total = self.content.vehicles.len().div_ceil(9);
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
                self.content = Catalog::scan(&self.selection);
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
                self.refresh_focus_diagnostics();
                self.menu.open(menu::Page::NgpExperimental);
            }
            StartFocusVisualInspection => {
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
                self.menu.inspected_vehicle = index;
                self.menu.open(menu::Page::GarageDetail);
            }
            SelectVehicle(index) => {
                let candidate = match self.content.vehicles.get(index) {
                    None => Err("UNKNOWN VEHICLE".to_string()),
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
                    let ov = self.camera_overrides.for_view_mut(self.view);
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
                            self.camera_overrides.reset_view(self.view);
                            self.camera_overrides.save();
                        }
                        CamSave => self.camera_overrides.save(),
                        _ => {}
                    }
                    *self.camera_overrides.for_view_mut(self.view) =
                        self.camera_overrides.for_view(self.view).sanitized();
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
                    if let Some(st) = st {
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
                                "OPCIONES: clutch_help={} ngp_experimental={}",
                                self.clutch_help, self.ngp_experimental
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
        }
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
        let result = self.school.record_verdict_result(st);
        self.saved_replay = self.replay_recorder.finish();
        self.controls.clear();
        self.active_school_result = Some(result);
        self.school_incident_banner = None;
        self.menu.show(menu::Page::SchoolResult);
        self.message = None;
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
                self.experimental_visual_only = true;
                self.message = Some(format!("SIN FÍSICA NGP ({err}) -> MODO VISUAL"));
            } else if !self.experimental_visual_only && state.rest.is_some() {
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
                && state.timing.finished
                && state.timing.finish_crossed_forward
            {
                self.race.begin_finish_rollout(
                    now,
                    state.timing.splits_passed,
                    state.timing.splits_total,
                    state.timing.finish_distance,
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
                audio::set_paused(is_paused);
                if !is_paused {
                    let noise = rerbr_sim::engine_audio::EngineNoise::from_rpm_throttle(
                        current_frame.rpm,
                        0.7,
                        current_frame.rpm * 0.1,
                    );
                    audio::update_engine_state(&noise, current_frame.gear as i32);
                }
                let car_pos = current_frame.car_position();
                let car_model = current_frame.models[0];
                let (camera, fov) = player.current_camera(
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
        let steering_ready = self.tilt.is_finite() || self.gamepad.connected;
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
                            let ov = self.camera_overrides.for_view(self.view);
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
                .map(|k| Vec3::from(rest.wheel_centre[k]) - swap.transform_point3(pivots[k]))
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
                let ov = self.camera_overrides.for_view(self.view);
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
                    let ov = self.camera_overrides.for_view(cameras::View::Chase);
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
        let cam_override = self.camera_overrides.for_view(self.view);
        #[cfg(not(feature = "physics-stage"))]
        let cam_override = cameras::CameraOverride::default_for(cameras::View::Chase);
        menu::Context {
            selection: &self.selection,
            content: &self.content,
            camera: self.camera_label(),
            steering_ready: self.tilt.is_finite(),
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
        }
    }
    fn status(&self, state: &worker::Shared) -> String {
        if let Some(error) = self.scene_error.as_ref().or(state.error.as_ref()) {
            return format!("ERROR: {error}");
        }
        if (!self.experimental_visual_only && state.rest.is_none()) || self.pivots.is_none() {
            return "LOADING PRIVATE STAGE / CAR / PHYSICS".into();
        }
        if !self.experimental_visual_only && !self.tilt.is_finite() && !self.gamepad.connected {
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
        if self.menu.page != menu::Page::Driving {
            let status = self.status(&state);
            let mut ctx = self.context();
            ctx.loading_error = self.scene_error.as_deref().or(state.error.as_deref());
            self.menu.draw(&mut canvas, &ctx, &status);
            hud.draw_bytes(queue, encoder, target, &canvas.finish());
            return;
        }
        self.draw_top_bar(&mut canvas, &layout);
        if let Some((_, sample, _)) = &state.pair {
            draw_cluster(&mut canvas, &layout, sample);
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
            let timer = menu::format_stage_time(secs);
            let r = Rect::new(0.43, 0.015, 0.14, 0.055);
            ui::panel(canvas, r, PANEL_DEEP, LINE);
            ui::text_center(canvas, r, &timer, 0.85, TEXT);
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
            banners.push((
                "MODO VISUAL EXPERIMENTAL — SIN FÍSICA NGP (FMOD SILENCIADO)".into(),
                BAD,
            ));
        } else if self.selection.vehicle.is_ngp_experimental_car() {
            banners.push((
                format!(
                    "RSF NGP 7.4 ACTIVO: {} (FMOD SILENCIADO)",
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
        for (index, (text, color)) in banners.iter().take(3).enumerate() {
            let rect = Rect::new(
                0.18,
                layout.pause.bottom() + 0.018 + index as f32 * 0.040,
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
fn draw_cluster(canvas: &mut Canvas, layout: &DrivingLayout, sample: &Sample) {
    let rect = layout.cluster;
    ui::panel(canvas, rect, PANEL, LINE);
    let gear = match sample.gear {
        0 => "R".to_string(),
        1 => "N".to_string(),
        g => (g - 1).to_string(),
    };
    let third = rect.w / 3.0;
    for index in 1..3 {
        let x = rect.x + third * index as f32;
        let (px, py, _w, ph) = ui::px(Rect::new(x, rect.y, 0.0, rect.h), canvas);
        canvas.rect(px, py + ph * 0.18, 1.0, ph * 0.64, LINE);
    }
    let cells = [
        (
            Rect::new(rect.x, rect.y, third, rect.h),
            gear,
            "GEAR",
            ACCENT,
            1.45,
        ),
        (
            Rect::new(rect.x + third, rect.y, third, rect.h),
            format!("{:.0}", sample.speed() * 3.6),
            "KM/H",
            TEXT,
            1.25,
        ),
        (
            Rect::new(rect.x + third * 2.0, rect.y, third, rect.h),
            format!("{:.0}", sample.rpm),
            "RPM",
            INFO,
            1.05,
        ),
    ];
    for (cell, value, label, color, scale) in cells {
        let (x, y, w, h) = ui::px(cell, canvas);
        canvas.text(
            x + (w - ui::texture_width(&value, scale)).max(0.0) * 0.5,
            y + 4.0,
            &value,
            scale,
            color,
        );
        canvas.text(
            x + (w - ui::texture_width(label, 0.55)).max(0.0) * 0.5,
            y + h - 12.0,
            label,
            0.55,
            ui::DIM,
        );
    }
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
    catalog
        .vehicle(id)
        .filter(|entry| {
            (ngp_experimental || !entry.descriptor.is_ngp_experimental_car())
                && (!entry.files.checked || entry.files.missing.is_empty())
        })
        .or_else(|| catalog.vehicle(&default.id))
        .map(|entry| entry.descriptor.clone())
        .unwrap_or(default)
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
                split_times: vec![Some(45.0), Some(90.0)],
                finished: true,
                finish_crossed_forward: false,
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
        let mut app = Runtime::new();

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
        assert_eq!(app.content.stages.len(), 30);
        app.apply_command(menu::Command::SelectStage(0));
        assert_eq!(app.selection.stage.id, "harwood-forest");

        // Mark stage 1 as having missing files
        app.content.stages[1].files.checked = true;
        app.content.stages[1].files.missing = vec!["track-22_O.col".into()];
        app.apply_command(menu::Command::SelectStage(1));
        assert_eq!(app.selection.stage.id, "harwood-forest");

        // Attempting StartRally with unsupported vehicle is blocked
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
                split_times: vec![Some(45.0), Some(90.0)],
                finished: true,
                finish_crossed_forward: true,
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
        let mut app = Runtime::new();
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
}
