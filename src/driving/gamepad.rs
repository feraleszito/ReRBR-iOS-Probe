//! Native Apple `GameController` (`GCController`) gamepad runtime and menu/driving mapper.
//!
//! Supports Xbox Series/One, PlayStation DualSense/DualShock, and MFi controllers via
//! `rerbr_gamepad_poll` on iOS/iPadOS, with deterministic snapshot injection for host tests.

use std::path::PathBuf;

pub const BTN_A: u32 = 1 << 0;
pub const BTN_B: u32 = 1 << 1;
pub const BTN_X: u32 = 1 << 2;
pub const BTN_Y: u32 = 1 << 3;
pub const BTN_LB: u32 = 1 << 4;
pub const BTN_RB: u32 = 1 << 5;
pub const BTN_DPAD_UP: u32 = 1 << 6;
pub const BTN_DPAD_DOWN: u32 = 1 << 7;
pub const BTN_DPAD_LEFT: u32 = 1 << 8;
pub const BTN_DPAD_RIGHT: u32 = 1 << 9;
pub const BTN_MENU: u32 = 1 << 10;
pub const BTN_OPTIONS: u32 = 1 << 11;
pub const BTN_L3: u32 = 1 << 12;
pub const BTN_R3: u32 = 1 << 13;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadSnapshot {
    pub connected: u8,
    pub _pad: [u8; 3],
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons_mask: u32,
    pub name: [u8; 64],
}

impl Default for GamepadSnapshot {
    fn default() -> Self {
        Self {
            connected: 0,
            _pad: [0; 3],
            left_x: 0.0,
            left_y: 0.0,
            right_x: 0.0,
            right_y: 0.0,
            left_trigger: 0.0,
            right_trigger: 0.0,
            buttons_mask: 0,
            name: [0; 64],
        }
    }
}

impl GamepadSnapshot {
    pub fn controller_name(&self) -> String {
        let len = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        let s = String::from_utf8_lossy(&self.name[..len]).trim().to_owned();
        if s.is_empty() {
            "MANDO ACTIVO".into()
        } else {
            s
        }
    }

    pub fn is_pressed(&self, btn: GamepadButton) -> bool {
        self.buttons_mask & btn.mask() != 0
    }

    pub fn has_driving_activity(&self, deadzone: f32) -> bool {
        self.connected != 0
            && (self.left_x.abs() > deadzone
                || self.left_trigger > 0.03
                || self.right_trigger > 0.03
                || self.buttons_mask != 0)
    }
}

#[cfg(target_os = "ios")]
pub fn poll_system_gamepad() -> GamepadSnapshot {
    extern "C" {
        fn rerbr_gamepad_poll(out: *mut GamepadSnapshot);
    }
    let mut snap = GamepadSnapshot::default();
    unsafe {
        rerbr_gamepad_poll(&mut snap);
    }
    snap
}

#[cfg(not(target_os = "ios"))]
pub fn poll_system_gamepad() -> GamepadSnapshot {
    GamepadSnapshot::default()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamepadButton {
    A,
    B,
    X,
    Y,
    Lb,
    Rb,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Menu,
    Options,
    L3,
    R3,
}

impl GamepadButton {
    pub const REMAPPABLE: [Self; 10] = [
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::Lb,
        Self::Rb,
        Self::Menu,
        Self::Options,
        Self::L3,
        Self::R3,
    ];

    pub const fn mask(self) -> u32 {
        match self {
            Self::A => BTN_A,
            Self::B => BTN_B,
            Self::X => BTN_X,
            Self::Y => BTN_Y,
            Self::Lb => BTN_LB,
            Self::Rb => BTN_RB,
            Self::DpadUp => BTN_DPAD_UP,
            Self::DpadDown => BTN_DPAD_DOWN,
            Self::DpadLeft => BTN_DPAD_LEFT,
            Self::DpadRight => BTN_DPAD_RIGHT,
            Self::Menu => BTN_MENU,
            Self::Options => BTN_OPTIONS,
            Self::L3 => BTN_L3,
            Self::R3 => BTN_R3,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::A => "A / EQUIS",
            Self::B => "B / CÍRCULO",
            Self::X => "X / CUADRADO",
            Self::Y => "Y / TRIÁNGULO",
            Self::Lb => "LB / L1",
            Self::Rb => "RB / R1",
            Self::DpadUp => "D-PAD ARRIBA",
            Self::DpadDown => "D-PAD ABAJO",
            Self::DpadLeft => "D-PAD IZQ",
            Self::DpadRight => "D-PAD DER",
            Self::Menu => "START / OPTIONS",
            Self::Options => "SELECT / SHARE",
            Self::L3 => "L3",
            Self::R3 => "R3",
        }
    }

    pub fn next_remappable(self) -> Self {
        let idx = Self::REMAPPABLE
            .iter()
            .position(|&b| b == self)
            .unwrap_or(0);
        Self::REMAPPABLE[(idx + 1) % Self::REMAPPABLE.len()]
    }

    fn code(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::X => "X",
            Self::Y => "Y",
            Self::Lb => "LB",
            Self::Rb => "RB",
            Self::DpadUp => "DU",
            Self::DpadDown => "DD",
            Self::DpadLeft => "DL",
            Self::DpadRight => "DR",
            Self::Menu => "MENU",
            Self::Options => "OPT",
            Self::L3 => "L3",
            Self::R3 => "R3",
        }
    }

    fn from_code(s: &str) -> Option<Self> {
        match s.trim() {
            "A" => Some(Self::A),
            "B" => Some(Self::B),
            "X" => Some(Self::X),
            "Y" => Some(Self::Y),
            "LB" => Some(Self::Lb),
            "RB" => Some(Self::Rb),
            "DU" => Some(Self::DpadUp),
            "DD" => Some(Self::DpadDown),
            "DL" => Some(Self::DpadLeft),
            "DR" => Some(Self::DpadRight),
            "MENU" => Some(Self::Menu),
            "OPT" => Some(Self::Options),
            "L3" => Some(Self::L3),
            "R3" => Some(Self::R3),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamepadAction {
    GearUp,
    GearDown,
    Handbrake,
    Clutch,
    Camera,
    Recover,
    Pause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GamepadSettings {
    pub deadzone: f32,
    pub sensitivity: f32,
    pub response_curve: f32,
    pub invert_steering: bool,
    pub invert_triggers: bool,
    pub gear_up: GamepadButton,
    pub gear_down: GamepadButton,
    pub handbrake: GamepadButton,
    pub clutch: GamepadButton,
    pub camera: GamepadButton,
    pub recover: GamepadButton,
    pub pause: GamepadButton,
}

impl Default for GamepadSettings {
    fn default() -> Self {
        Self {
            deadzone: 0.08,
            sensitivity: 1.10,
            response_curve: 1.40,
            invert_steering: false,
            invert_triggers: false,
            gear_up: GamepadButton::Rb,
            gear_down: GamepadButton::Lb,
            handbrake: GamepadButton::B,
            clutch: GamepadButton::X,
            camera: GamepadButton::Y,
            recover: GamepadButton::Options,
            pause: GamepadButton::Menu,
        }
    }
}

impl GamepadSettings {
    pub fn sanitized(mut self) -> Self {
        self.deadzone = if self.deadzone.is_finite() {
            self.deadzone.clamp(0.0, 0.35)
        } else {
            0.08
        };
        self.sensitivity = if self.sensitivity.is_finite() {
            self.sensitivity.clamp(0.5, 2.5)
        } else {
            1.10
        };
        self.response_curve = if self.response_curve.is_finite() {
            self.response_curve.clamp(1.0, 2.5)
        } else {
            1.40
        };
        self
    }

    pub fn config_path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(root.join("RBR").join("gamepad_config.txt"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse_config(&text)
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.serialize_config());
    }

    pub fn serialize_config(&self) -> String {
        format!(
            "deadzone={:.3}\nsensitivity={:.3}\nresponse_curve={:.3}\ninvert_steering={}\ninvert_triggers={}\ngear_up={}\ngear_down={}\nhandbrake={}\nclutch={}\ncamera={}\nrecover={}\npause={}\n",
            self.deadzone,
            self.sensitivity,
            self.response_curve,
            self.invert_steering,
            self.invert_triggers,
            self.gear_up.code(),
            self.gear_down.code(),
            self.handbrake.code(),
            self.clutch.code(),
            self.camera.code(),
            self.recover.code(),
            self.pause.code(),
        )
    }

    pub fn parse_config(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            match k {
                "deadzone" => {
                    if let Ok(n) = v.parse() {
                        s.deadzone = n;
                    }
                }
                "sensitivity" => {
                    if let Ok(n) = v.parse() {
                        s.sensitivity = n;
                    }
                }
                "response_curve" => {
                    if let Ok(n) = v.parse() {
                        s.response_curve = n;
                    }
                }
                "invert_steering" => {
                    if let Ok(b) = v.parse() {
                        s.invert_steering = b;
                    }
                }
                "invert_triggers" => {
                    if let Ok(b) = v.parse() {
                        s.invert_triggers = b;
                    }
                }
                "gear_up" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.gear_up = btn;
                    }
                }
                "gear_down" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.gear_down = btn;
                    }
                }
                "handbrake" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.handbrake = btn;
                    }
                }
                "clutch" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.clutch = btn;
                    }
                }
                "camera" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.camera = btn;
                    }
                }
                "recover" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.recover = btn;
                    }
                }
                "pause" => {
                    if let Some(btn) = GamepadButton::from_code(v) {
                        s.pause = btn;
                    }
                }
                _ => {}
            }
        }
        s.sanitized()
    }

    pub fn cycle_binding(&mut self, action: GamepadAction) -> GamepadButton {
        let slot = match action {
            GamepadAction::GearUp => &mut self.gear_up,
            GamepadAction::GearDown => &mut self.gear_down,
            GamepadAction::Handbrake => &mut self.handbrake,
            GamepadAction::Clutch => &mut self.clutch,
            GamepadAction::Camera => &mut self.camera,
            GamepadAction::Recover => &mut self.recover,
            GamepadAction::Pause => &mut self.pause,
        };
        *slot = slot.next_remappable();
        *slot
    }

    pub fn shape_axis(&self, raw: f32) -> f32 {
        if !raw.is_finite() {
            return 0.0;
        }
        let mag = raw.abs();
        if mag <= self.deadzone {
            return 0.0;
        }
        let span = (1.0 - self.deadzone).max(0.05);
        let norm = ((mag - self.deadzone) / span).clamp(0.0, 1.0);
        let curved = norm.powf(self.response_curve) * self.sensitivity;
        let signed = raw.signum() * curved.clamp(0.0, 1.0);
        if self.invert_steering {
            -signed
        } else {
            signed
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuNavEvent {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Pause,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct GamepadDrivingFrame {
    pub active: bool,
    pub steering: f32,
    pub throttle: f32,
    pub brake: f32,
    pub handbrake: f32,
    pub clutch: f32,
    pub gear_delta: i32,
    pub camera_pressed: bool,
    pub pause_pressed: bool,
    pub recover_pressed: bool,
}

#[derive(Clone, Copy, Default, Debug)]
struct DirectionRepeat {
    held: bool,
    next_fire: f64,
}

impl DirectionRepeat {
    const INITIAL_DELAY: f64 = 0.36;
    const REPEAT_INTERVAL: f64 = 0.13;

    fn update(&mut self, active: bool, now: f64) -> bool {
        if !active {
            self.held = false;
            return false;
        }
        if !self.held {
            self.held = true;
            self.next_fire = now + Self::INITIAL_DELAY;
            return true;
        }
        if now >= self.next_fire {
            self.next_fire = now + Self::REPEAT_INTERVAL;
            return true;
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct GamepadRuntime {
    pub settings: GamepadSettings,
    pub connected: bool,
    pub controller_name: String,
    pub last_snapshot: GamepadSnapshot,
    prev_mask: u32,
    prev_menu_mask: u32,
    sticky_active: bool,
    up_repeat: DirectionRepeat,
    down_repeat: DirectionRepeat,
    left_repeat: DirectionRepeat,
    right_repeat: DirectionRepeat,
}

impl GamepadRuntime {
    pub fn load() -> Self {
        Self {
            settings: GamepadSettings::load(),
            ..Default::default()
        }
    }

    pub fn update_connection(&mut self, snap: &GamepadSnapshot) {
        self.connected = snap.connected != 0;
        self.last_snapshot = *snap;
        if self.connected {
            self.controller_name = snap.controller_name();
        } else {
            self.controller_name.clear();
            self.sticky_active = false;
            self.prev_mask = 0;
            self.prev_menu_mask = 0;
        }
    }

    pub fn clear_driving_edges(&mut self) {
        self.prev_mask = self.last_snapshot.buttons_mask;
        self.sticky_active = false;
    }

    pub fn menu_events(&mut self, snap: &GamepadSnapshot, now_secs: f64) -> Vec<MenuNavEvent> {
        self.update_connection(snap);
        if !self.connected {
            return Vec::new();
        }
        let mut events = Vec::new();
        let m = snap.buttons_mask;
        let just_pressed = m & !self.prev_menu_mask;
        self.prev_menu_mask = m;

        const STICK_NAV_THRESHOLD: f32 = 0.55;
        let up = (m & BTN_DPAD_UP != 0) || snap.left_y > STICK_NAV_THRESHOLD;
        let down = (m & BTN_DPAD_DOWN != 0) || snap.left_y < -STICK_NAV_THRESHOLD;
        let left = (m & BTN_DPAD_LEFT != 0) || snap.left_x < -STICK_NAV_THRESHOLD;
        let right = (m & BTN_DPAD_RIGHT != 0) || snap.left_x > STICK_NAV_THRESHOLD;

        if self.up_repeat.update(up && !down, now_secs) {
            events.push(MenuNavEvent::Up);
        }
        if self.down_repeat.update(down && !up, now_secs) {
            events.push(MenuNavEvent::Down);
        }
        if self.left_repeat.update(left && !right, now_secs) {
            events.push(MenuNavEvent::Left);
        }
        if self.right_repeat.update(right && !left, now_secs) {
            events.push(MenuNavEvent::Right);
        }
        if just_pressed & BTN_A != 0 {
            events.push(MenuNavEvent::Confirm);
        }
        if just_pressed & BTN_B != 0 {
            events.push(MenuNavEvent::Back);
        }
        if just_pressed & self.settings.pause.mask() != 0 {
            events.push(MenuNavEvent::Pause);
        }
        events
    }

    pub fn driving_input(&mut self, snap: &GamepadSnapshot) -> GamepadDrivingFrame {
        self.update_connection(snap);
        if !self.connected {
            return GamepadDrivingFrame::default();
        }
        let m = snap.buttons_mask;
        let just_pressed = m & !self.prev_mask;
        self.prev_mask = m;

        if snap.has_driving_activity(self.settings.deadzone) {
            self.sticky_active = true;
        }
        let (throttle, brake) = if self.settings.invert_triggers {
            (
                snap.left_trigger.clamp(0.0, 1.0),
                snap.right_trigger.clamp(0.0, 1.0),
            )
        } else {
            (
                snap.right_trigger.clamp(0.0, 1.0),
                snap.left_trigger.clamp(0.0, 1.0),
            )
        };
        let gear_delta = i32::from(just_pressed & self.settings.gear_up.mask() != 0)
            - i32::from(just_pressed & self.settings.gear_down.mask() != 0);

        GamepadDrivingFrame {
            active: self.sticky_active,
            steering: self.settings.shape_axis(snap.left_x),
            throttle,
            brake,
            handbrake: if snap.is_pressed(self.settings.handbrake) {
                1.0
            } else {
                0.0
            },
            clutch: if snap.is_pressed(self.settings.clutch) {
                1.0
            } else {
                0.0
            },
            gear_delta,
            camera_pressed: just_pressed & self.settings.camera.mask() != 0,
            pause_pressed: just_pressed & self.settings.pause.mask() != 0,
            recover_pressed: just_pressed & self.settings.recover.mask() != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected_snap() -> GamepadSnapshot {
        let mut s = GamepadSnapshot {
            connected: 1,
            ..Default::default()
        };
        s.name[..4].copy_from_slice(b"Xbox");
        s
    }

    #[test]
    fn steering_deadzone_response_curve_and_inversion() {
        let mut cfg = GamepadSettings::default();
        assert_eq!(cfg.shape_axis(0.05), 0.0);
        assert!(cfg.shape_axis(0.5) > 0.25 && cfg.shape_axis(0.5) < 0.7);
        assert_eq!(cfg.shape_axis(1.0), 1.0);
        cfg.invert_steering = true;
        assert!(cfg.shape_axis(0.6) < 0.0);
    }

    #[test]
    fn menu_events_handle_confirm_back_and_debounced_hold() {
        let mut rt = GamepadRuntime::default();
        let mut snap = connected_snap();
        snap.buttons_mask = BTN_DPAD_DOWN | BTN_A;
        let first = rt.menu_events(&snap, 1.0);
        assert_eq!(first, vec![MenuNavEvent::Down, MenuNavEvent::Confirm]);

        // Holding within initial delay emits neither Confirm nor repeated Down
        let held_early = rt.menu_events(&snap, 1.2);
        assert!(held_early.is_empty());

        // After initial delay (~0.36s), Down repeats once while Confirm stays edge-triggered
        let held_repeat = rt.menu_events(&snap, 1.40);
        assert_eq!(held_repeat, vec![MenuNavEvent::Down]);

        snap.buttons_mask = BTN_B;
        let back = rt.menu_events(&snap, 1.45);
        assert_eq!(back, vec![MenuNavEvent::Back]);
    }

    #[test]
    fn driving_edges_and_settings_roundtrip_work() {
        let mut cfg = GamepadSettings {
            deadzone: 0.12,
            sensitivity: 1.5,
            response_curve: 1.8,
            invert_steering: true,
            invert_triggers: true,
            gear_up: GamepadButton::Y,
            ..Default::default()
        };
        let parsed = GamepadSettings::parse_config(&cfg.serialize_config());
        assert_eq!(parsed, cfg);
        assert_eq!(cfg.cycle_binding(GamepadAction::GearUp), GamepadButton::Lb);

        let mut rt = GamepadRuntime::default();
        let mut snap = connected_snap();
        snap.right_trigger = 0.85;
        snap.buttons_mask = BTN_RB;
        let f1 = rt.driving_input(&snap);
        assert!(f1.active);
        assert_eq!(f1.throttle, 0.85);
        assert_eq!(f1.gear_delta, 1);

        let f2 = rt.driving_input(&snap);
        assert_eq!(f2.gear_delta, 0, "gear shift must remain edge-triggered");
    }
}
