//! Original physics-owned driving presentation, menus and session controls.
pub mod audio;
#[cfg(feature = "physics-stage")]
pub mod cameras;
pub mod content_manager;
pub mod controls;
pub mod layout;
pub mod menu;
#[cfg(test)]
mod private_tests;
mod ui;
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

pub struct Runtime {
    #[cfg(feature = "physics-stage")]
    pub view: cameras::View,
    #[cfg(feature = "physics-stage")]
    pub mounts: Option<cameras::Mounts>,
    worker: worker::Worker,
    selection: rerbr_content::Selection,
    pub controls: Controls,
    diagnostics: bool,
    minimal_hud: bool,
    volume: f64,
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
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
impl Runtime {
    pub fn new() -> Self {
        let settings = menu::Settings::load();
        let selection = rerbr_content::Selection::default();
        let mut controls = Controls::new();
        controls.sensitivity = settings.sensitivity;
        controls.dead_zone = settings.dead_zone;
        controls.invert_pedals = settings.invert_pedals;
        audio::set_master_volume(settings.volume as f32);
        Self {
            #[cfg(feature = "physics-stage")]
            view: cameras::View::from_index(settings.camera),
            #[cfg(feature = "physics-stage")]
            mounts: None,
            worker: worker::Worker::new_selected(selection.clone(), false),
            content: Catalog::scan(&selection),
            selection,
            controls,
            diagnostics: settings.diagnostics,
            minimal_hud: settings.minimal_hud,
            volume: settings.volume,
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
        }
    }
    pub fn install(&mut self, geometry: &Geometry) {
        self.pivots = Some(geometry.pivots);
    }
    pub fn configure_selection(
        &mut self,
        selection: rerbr_content::Selection,
    ) -> Result<(), String> {
        selection.validate_current_backend()?;
        self.controls.configure_vehicle(&selection.vehicle)?;
        if self.selection != selection {
            self.pause();
            self.worker.shutdown();
            self.worker = worker::Worker::new_selected(selection.clone(), false);
            self.offset = None;
            self.camera = None;
            self.content = Catalog::scan(&selection);
        }
        self.selection = selection;
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
            volume: self.volume,
            camera: self.camera_index(),
        }
        .save();
    }
    fn menu_command(&mut self, command: menu::Command) {
        use menu::Command::*;
        match command {
            Open(page) => self.menu.open(page),
            Back => self.menu.back(),
            ResetToMain => {
                audio::set_paused(true);
                self.worker.shutdown();
                self.worker = worker::Worker::new_selected(self.selection.clone(), false);
                self.offset = None;
                self.camera = None;
                self.controls.clear();
                self.controls.selected_gear = 2;
                self.menu.reset_to_main();
                self.message = Some("NEW SESSION READY".into());
            }
            Start => {
                let mut state = self.worker.state.lock().unwrap();
                if self.pivots.is_some()
                    && self.tilt.is_finite()
                    && self.scene_error.is_none()
                    && state.error.is_none()
                    && state.rest.is_some()
                {
                    self.controls.clear();
                    state.paused = false;
                    audio::set_paused(false);
                    self.menu.resume();
                    self.message = None;
                } else {
                    self.message = Some("START BLOCKED: ASSETS OR LANDSCAPE NOT READY".into());
                }
            }
            Restart => {
                audio::set_paused(true);
                self.worker.shutdown();
                self.worker = worker::Worker::new_selected(self.selection.clone(), false);
                self.offset = None;
                self.camera = None;
                self.controls.clear();
                self.controls.selected_gear = 2;
                self.menu.show(menu::Page::Paused);
                self.message = Some("SESSION RESET TO ORIGINAL SPAWN".into());
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
                    "VOLUME {:.0}% ({})",
                    self.volume * 100.0,
                    audio::status()
                ));
            }
            VolumeUp => {
                self.volume = (self.volume + 0.1).min(1.0);
                audio::set_master_volume(self.volume as f32);
                self.message = Some(format!(
                    "VOLUME {:.0}% ({})",
                    self.volume * 100.0,
                    audio::status()
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
            SelectVehicle => {
                let result = self.configure_selection(self.selection.clone());
                self.message = Some(match result {
                    Ok(()) => format!("VEHICLE SELECTED: {}", self.selection.vehicle.name),
                    Err(error) => format!("VEHICLE REJECTED: {error}"),
                });
            }
            SelectStage => {
                let result = self.configure_selection(self.selection.clone());
                self.message = Some(match result {
                    Ok(()) => format!("STAGE SELECTED: {}", self.selection.stage.name),
                    Err(error) => format!("STAGE REJECTED: {error}"),
                });
            }
        }
        self.save_settings();
    }
    pub fn touch(&mut self, t: Touch, size: (u32, u32)) {
        let x = t.location.x / f64::from(size.0.max(1));
        let y = t.location.y / f64::from(size.1.max(1));
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
        let layout = DrivingLayout::for_size(size);
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
    pub fn frame(&mut self, path: &ChasePath) -> Option<Frame> {
        let now = Instant::now();
        let seconds = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        if seconds > 0.0 {
            self.fps = self.fps * 0.9 + 0.1 / seconds;
        }
        self.tilt = motion(true);
        if now >= self.footprint_at {
            self.footprint = rerbr_sim::footprint();
            self.footprint_at = now + Duration::from_secs(1);
        }
        let (pair, rest, paused) = {
            let mut s = self.worker.state.lock().unwrap();
            if self.menu.page != menu::Page::Driving {
                s.paused = true;
            }
            if !self.tilt.is_finite() {
                s.paused = true;
                self.controls.clear();
            }
            s.input = self.controls.input(self.tilt, seconds);
            (s.pair.clone(), s.rest.clone(), s.paused)
        };
        audio::set_paused(paused);
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
        self.steering_angle = a.steering_angle + (b.steering_angle - a.steering_angle) * alpha;
        let position =
            swap.transform_point3(Vec3::from(a.position).lerp(Vec3::from(b.position), alpha));
        let forward =
            Vec3::new(-models[0].z_axis.x, 0.0, -models[0].z_axis.z).normalize_or(Vec3::NEG_Z);
        let forward = self.camera.map_or(forward, |c| {
            Vec3::new(c.target.x - c.eye.x, 0.0, c.target.z - c.eye.z)
                .normalize_or(forward)
                .lerp(forward, (1.0 - (-seconds / 0.1).exp()) as f32)
                .normalize_or(forward)
        });
        if !path.supports_driving(position) {
            audio::set_paused(true);
            let mut s = self.worker.state.lock().unwrap();
            s.paused = true;
            s.error = Some(if cfg!(feature = "physics-stage") {
                "OUTSIDE VERIFIED TERRAIN - RESET STAGE".into()
            } else {
                "VISIBLE TERRAIN LIMIT - RESET STAGE".into()
            });
            return None;
        }
        let target = position + Vec3::Y * 0.5;
        let target = self.camera.map_or(target, |c| {
            c.target
                .lerp(target, (1.0 - (-seconds / 0.06).exp()) as f32)
        });
        #[cfg(feature = "physics-stage")]
        audio::set_view_is_cockpit(
            self.view == cameras::View::CockpitFallback && self.cockpit_ready,
        );
        #[cfg(feature = "physics-stage")]
        if let Some(camera) = self
            .mounts
            .and_then(|m| m.camera_with_cockpit(self.view, models[0], self.cockpit_node))
        {
            if path.camera_supported(camera.eye) {
                return Some(Frame {
                    models,
                    camera,
                    boom: 0.0,
                    max_wheel_travel: b.wheel_travel.iter().map(|x| x.abs()).fold(0.0, f32::max),
                    min_wheel_clearance: 0.0,
                });
            }
            // Keep physics running and use the checked chase view if a mount is underground.
            self.view = cameras::View::Chase;
            self.camera = None;
            eprintln!("physics-stage CAMERA: mount underground/unsupported; returning to chase");
        }
        match path.chase_camera(target, forward) {
            Ok((camera, boom)) => {
                self.camera = Some(camera);
                Some(Frame {
                    models,
                    camera,
                    boom,
                    max_wheel_travel: b.wheel_travel.iter().map(|x| x.abs()).fold(0.0, f32::max),
                    min_wheel_clearance: 0.0,
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
    fn context(&self) -> menu::Context<'_> {
        menu::Context {
            selection: &self.selection,
            content: &self.content,
            camera: self.camera_label(),
            steering_ready: self.tilt.is_finite(),
            cockpit_ready: self.cockpit_ready,
            sensitivity: self.controls.sensitivity,
            dead_zone: self.controls.dead_zone,
            volume: self.volume,
            fps: self.fps_visible,
            diagnostics: self.diagnostics,
            minimal_hud: self.minimal_hud,
            invert_pedals: self.controls.invert_pedals,
        }
    }
    fn status(&self, state: &worker::Shared) -> String {
        if let Some(error) = self.scene_error.as_ref().or(state.error.as_ref()) {
            return format!("ERROR: {error}");
        }
        if state.rest.is_none() || self.pivots.is_none() {
            return "LOADING PRIVATE STAGE / CAR / PHYSICS".into();
        }
        if !self.tilt.is_finite() {
            return "STEERING UNAVAILABLE - HOLD IPAD IN LANDSCAPE".into();
        }
        if let Some(message) = &self.message {
            return message.clone();
        }
        if self.menu.page == menu::Page::Paused {
            "PAUSED / RESUME WHEN READY".into()
        } else {
            "READY - TAP A ROW".into()
        }
    }
    pub fn overlay(
        &self,
        hud: &Hud,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let state = self.worker.state.lock().unwrap().clone();
        let mut canvas = Canvas::new(size);
        if self.menu.page != menu::Page::Driving {
            let status = self.status(&state);
            self.menu.draw(&mut canvas, &self.context(), &status);
            hud.draw_bytes(queue, encoder, target, &canvas.finish());
            return;
        }
        let layout = DrivingLayout::for_size(size);
        self.draw_top_bar(&mut canvas, &layout);
        if let Some((_, sample, _)) = &state.pair {
            draw_cluster(&mut canvas, &layout, sample);
        }
        self.draw_side_controls(&mut canvas, &layout);
        if self.diagnostics {
            self.draw_telemetry(&mut canvas, &layout, &state);
        }
        self.draw_banners(&mut canvas, &layout, &state);
        hud.draw_bytes(queue, encoder, target, &canvas.finish());
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
        let levels = self.controls.levels();
        draw_control(canvas, layout.handbrake, "HB", levels.handbrake);
        draw_control(canvas, layout.clutch, "CLUTCH", levels.clutch);
        draw_control(canvas, layout.gear_down, "GEAR -", 0.0);
        draw_control(canvas, layout.gear_up, "GEAR +", 0.0);
        draw_control(canvas, layout.brake, "BRAKE", levels.brake);
        draw_control(canvas, layout.throttle, "THROTTLE", levels.throttle);
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
        if let Some(status) = &state.physics_status {
            lines.extend(ui::wrap(status, 34));
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
        if let Some(error) = state.error.as_ref().or(self.scene_error.as_ref()) {
            banners.push((format!("ERROR: {error}"), BAD));
        } else if state.rest.is_none() {
            banners.push(("LOADING PHYSICS".into(), WARN));
        } else if !self.tilt.is_finite() {
            banners.push(("HOLD IPAD IN LANDSCAPE TO STEER".into(), WARN));
        }
        if !self.cockpit_ready && self.camera_label() == "COCKPIT" {
            banners.push(("COCKPIT ASSETS MISSING - EXTERIOR VIEW".into(), WARN));
        }
        for (index, (text, color)) in banners.iter().take(2).enumerate() {
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
fn draw_control(canvas: &mut Canvas, rect: Rect, label: &str, level: f32) {
    let active = level > 0.0;
    ui::fill(canvas, rect, PANEL_DEEP);
    let (x, y, w, h) = ui::px(rect, canvas);
    if active {
        let fill_height = (h - 4.0) * level.clamp(0.0, 1.0);
        canvas.rect(
            x + 2.0,
            y + h - 2.0 - fill_height,
            w - 4.0,
            fill_height,
            [240, 184, 80, 90],
        );
    }
    ui::frame(canvas, rect, if active { ACCENT } else { LINE }, 1.5);
    ui::text_center(canvas, rect, label, 0.85, TEXT);
    if active && (label == "BRAKE" || label == "THROTTLE") {
        let value = format!("{:.0}%", level * 100.0);
        let (cx, cy, _w, _h) = ui::px(rect, canvas);
        canvas.text(cx + 10.0, cy + h - 17.0, &value, 0.6, ACCENT);
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
