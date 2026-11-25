//! Shared normalized layout for menu hit testing and HUD drawing.
//! Safe-area insets come from UIKit; both draw and touch use the same rectangles.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (x, y) = (x as f32, y as f32);
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
    pub fn center_x(&self) -> f32 {
        self.x + self.w * 0.5
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn inset(&self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            w: (self.w - 2.0 * x).max(0.0),
            h: (self.h - 2.0 * y).max(0.0),
        }
    }
    pub fn clamped_to_screen(self) -> Self {
        let w = if self.w.is_finite() {
            self.w.clamp(0.06, 0.35)
        } else {
            0.14
        };
        let h = if self.h.is_finite() {
            self.h.clamp(0.06, 0.45)
        } else {
            0.14
        };
        let x = if self.x.is_finite() {
            self.x.clamp(0.01, (0.99 - w).max(0.01))
        } else {
            0.05
        };
        let y = if self.y.is_finite() {
            self.y.clamp(0.08, (0.99 - h).max(0.08))
        } else {
            0.50
        };
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableControl {
    Handbrake = 0,
    Clutch = 1,
    GearDown = 2,
    GearUp = 3,
    Brake = 4,
    Throttle = 5,
}
impl EditableControl {
    pub const ALL: [Self; 6] = [
        Self::Handbrake,
        Self::Clutch,
        Self::GearDown,
        Self::GearUp,
        Self::Brake,
        Self::Throttle,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Handbrake => "HB",
            Self::Clutch => "CLUTCH",
            Self::GearDown => "GEAR -",
            Self::GearUp => "GEAR +",
            Self::Brake => "BRAKE",
            Self::Throttle => "THROTTLE",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Handbrake => "handbrake",
            Self::Clutch => "clutch",
            Self::GearDown => "gear_down",
            Self::GearUp => "gear_up",
            Self::Brake => "brake",
            Self::Throttle => "throttle",
        }
    }
}

pub const DEFAULT_CONTROLS_OPACITY: f32 = 0.50;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlsLayoutOverrides {
    pub rects: [Option<Rect>; 6],
    pub opacities: [Option<f32>; 6],
    pub global_opacity: f32,
}
impl Default for ControlsLayoutOverrides {
    fn default() -> Self {
        Self {
            rects: [None; 6],
            opacities: [None; 6],
            global_opacity: DEFAULT_CONTROLS_OPACITY,
        }
    }
}
impl ControlsLayoutOverrides {
    pub fn apply(self, mut layout: DrivingLayout) -> DrivingLayout {
        let slots = [
            &mut layout.handbrake,
            &mut layout.clutch,
            &mut layout.gear_down,
            &mut layout.gear_up,
            &mut layout.brake,
            &mut layout.throttle,
        ];
        for (slot, ov) in slots.into_iter().zip(self.rects) {
            if let Some(rect) = ov {
                *slot = rect.clamped_to_screen();
            }
        }
        layout
    }
    pub fn opacity_for(&self, control: EditableControl) -> f32 {
        self.opacities[control as usize]
            .unwrap_or(self.global_opacity)
            .clamp(0.10, 1.00)
    }
    pub fn set_opacity(&mut self, control: EditableControl, opacity: f32) {
        self.opacities[control as usize] = Some(opacity.clamp(0.10, 1.00));
    }
    pub fn adjust_opacity(&mut self, control: EditableControl, delta: f32) {
        let cur = self.opacity_for(control);
        self.set_opacity(control, cur + delta);
    }
    pub fn set_global_opacity(&mut self, opacity: f32) {
        self.global_opacity = opacity.clamp(0.10, 1.00);
        self.opacities = [None; 6];
    }
    pub fn adjust_global_opacity(&mut self, delta: f32) {
        let cur = self.global_opacity;
        self.set_global_opacity(cur + delta);
    }
    pub fn rect_of(self, control: EditableControl, base: DrivingLayout) -> Rect {
        self.apply(base).control_rect(control)
    }
    pub fn set_center(&mut self, control: EditableControl, base: DrivingLayout, cx: f32, cy: f32) {
        let cur = self.rect_of(control, base);
        let moved = Rect::new(cx - cur.w * 0.5, cy - cur.h * 0.5, cur.w, cur.h).clamped_to_screen();
        self.rects[control as usize] = Some(moved);
    }
    pub fn resize(&mut self, control: EditableControl, base: DrivingLayout, dw: f32, dh: f32) {
        let cur = self.rect_of(control, base);
        let resized = Rect::new(cur.x - dw * 0.5, cur.y - dh * 0.5, cur.w + dw, cur.h + dh)
            .clamped_to_screen();
        self.rects[control as usize] = Some(resized);
    }
    pub fn reset(&mut self) {
        self.rects = [None; 6];
        self.opacities = [None; 6];
        self.global_opacity = DEFAULT_CONTROLS_OPACITY;
    }
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            if key == "opacity.global" || key == "opacity" || key == "global_opacity" {
                if let Ok(op) = val.parse::<f32>() {
                    out.global_opacity = op.clamp(0.10, 1.00);
                }
                continue;
            }
            if let Some(ctrl_key) = key.strip_prefix("opacity.") {
                if let Some(ctrl) = EditableControl::ALL
                    .into_iter()
                    .find(|c| c.key() == ctrl_key)
                {
                    if let Ok(op) = val.parse::<f32>() {
                        out.opacities[ctrl as usize] = Some(op.clamp(0.10, 1.00));
                    }
                }
                continue;
            }
            let Some(ctrl) = EditableControl::ALL.into_iter().find(|c| c.key() == key) else {
                continue;
            };
            let parts: Vec<f32> = val
                .split(',')
                .map(|s| s.trim().parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
            if parts.len() == 4 {
                out.rects[ctrl as usize] =
                    Some(Rect::new(parts[0], parts[1], parts[2], parts[3]).clamped_to_screen());
            }
        }
        out
    }
    pub fn serialize(self) -> String {
        let mut s = String::new();
        for ctrl in EditableControl::ALL {
            if let Some(r) = self.rects[ctrl as usize] {
                let r = r.clamped_to_screen();
                s.push_str(&format!(
                    "{}={:.4},{:.4},{:.4},{:.4}\n",
                    ctrl.key(),
                    r.x,
                    r.y,
                    r.w,
                    r.h
                ));
            }
            if let Some(op) = self.opacities[ctrl as usize] {
                s.push_str(&format!(
                    "opacity.{}={:.2}\n",
                    ctrl.key(),
                    op.clamp(0.10, 1.00)
                ));
            }
        }
        s.push_str(&format!(
            "opacity.global={:.2}\n",
            self.global_opacity.clamp(0.10, 1.00)
        ));
        s
    }
    fn path() -> Option<std::path::PathBuf> {
        std::env::var_os("RERBR_PROBE_DOCUMENTS")
            .map(|d| std::path::PathBuf::from(d).join("rerbr-controls-layout.txt"))
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |s| Self::parse(&s))
    }
    pub fn save(self) {
        if let Some(path) = Self::path() {
            if let Err(error) = std::fs::write(path, self.serialize()) {
                eprintln!("CONTROL SETTINGS: {error}");
            }
        }
    }
}

/// Normalized fractions of the drawable. UIKit reports points; the ratio is
/// independent of the drawable's physical pixel size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl Insets {
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };
    pub fn sanitize(values: [f64; 4]) -> Self {
        let part = |v: f64| {
            if v.is_finite() && v > 0.0 {
                (v as f32).clamp(0.0, 0.12)
            } else {
                0.0
            }
        };
        Self {
            left: part(values[0]),
            top: part(values[1]),
            right: part(values[2]),
            bottom: part(values[3]),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct WindowLayout {
    insets: Insets,
    phone: bool,
}

impl WindowLayout {
    /// Reads UIKit safe-area insets. The iPad retains its existing layout.
    pub fn query() -> Self {
        #[cfg(target_os = "ios")]
        {
            unsafe extern "C" {
                fn rerbr_drive_safe_area(out: *mut f64) -> i32;
            }
            let mut values = [0.0; 4];
            let phone = unsafe { rerbr_drive_safe_area(values.as_mut_ptr()) } != 0;
            Self {
                insets: Insets::sanitize(values),
                phone,
            }
        }
        #[cfg(target_os = "android")]
        {
            let (values, phone) = crate::android::layout();
            Self {
                insets: Insets::sanitize(values),
                phone,
            }
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        Self::default()
    }

    pub fn viewport(self) -> Rect {
        if !self.phone {
            return Rect::new(0.0, 0.0, 1.0, 1.0);
        }
        let i = self.insets;
        Rect::new(
            i.left,
            i.top,
            1.0 - i.left - i.right,
            1.0 - i.top - i.bottom,
        )
    }

    pub fn point(self, x: f64, y: f64) -> (f64, f64) {
        let r = self.viewport();
        (
            (x - f64::from(r.x)) / f64::from(r.w),
            (y - f64::from(r.y)) / f64::from(r.h),
        )
    }

    pub fn accepts_touch_start(self, x: f64, y: f64) -> bool {
        !self.phone || self.viewport().contains(x, y)
    }

    pub fn driving(self) -> DrivingLayout {
        if !self.phone {
            return DrivingLayout::with_insets(self.insets);
        }
        // The whole phone HUD is already inset by the shared viewport.
        let mut layout = DrivingLayout::default();
        layout.pause.h = 0.12;
        layout.camera.h = layout.pause.h;
        layout.diag.h = layout.pause.h;
        layout.fps.h = layout.pause.h;
        let telemetry_bottom = layout.telemetry.bottom();
        layout.telemetry.y = layout.pause.bottom() + 0.020;
        layout.telemetry.h = telemetry_bottom - layout.telemetry.y;
        layout
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrivingLayout {
    pub pause: Rect,
    pub camera: Rect,
    pub diag: Rect,
    pub fps: Rect,
    pub handbrake: Rect,
    pub clutch: Rect,
    pub gear_down: Rect,
    pub gear_up: Rect,
    pub brake: Rect,
    pub throttle: Rect,
    pub cluster: Rect,
    pub telemetry: Rect,
}
impl Default for DrivingLayout {
    fn default() -> Self {
        Self::with_insets(Insets::ZERO)
    }
}
impl DrivingLayout {
    pub fn steer_left_rect(&self) -> Rect {
        let x = (self.brake.right() + 0.012).clamp(0.16, 0.24);
        Rect::new(x, 0.76, 0.115, 0.19)
    }
    pub fn steer_right_rect(&self) -> Rect {
        let x = (self.throttle.x - 0.127).clamp(0.645, 0.725);
        Rect::new(x, 0.76, 0.115, 0.19)
    }
    pub fn engine_start_rect(&self) -> Option<Rect> {
        (0..4)
            .map(|i| {
                Rect::new(
                    self.fps.x - 0.062 - i as f32 * 0.06,
                    self.fps.y,
                    0.052,
                    self.pause.h,
                )
            })
            .find(|r| {
                r.x > self.diag.right()
                    && EditableControl::ALL.into_iter().all(|control| {
                        let pedal = self.control_rect(control);
                        r.right() < pedal.x
                            || pedal.right() < r.x
                            || r.bottom() < pedal.y
                            || pedal.bottom() < r.y
                    })
            })
    }
    pub fn control_rect(&self, control: EditableControl) -> Rect {
        match control {
            EditableControl::Handbrake => self.handbrake,
            EditableControl::Clutch => self.clutch,
            EditableControl::GearDown => self.gear_down,
            EditableControl::GearUp => self.gear_up,
            EditableControl::Brake => self.brake,
            EditableControl::Throttle => self.throttle,
        }
    }
    pub fn for_size(_size: (u32, u32)) -> Self {
        WindowLayout::query().driving()
    }
    /// Hit and draw rectangles shared by the runtime, controls and HUD.
    /// Side clusters keep the centre of the road clear.
    pub fn with_insets(insets: Insets) -> Self {
        let l = insets.left.max(0.012);
        let r = insets.right.max(0.012);
        let t = insets.top.max(0.012);
        let b = insets.bottom.max(0.012);
        let col = 0.16;
        Self {
            pause: Rect::new(l, t, 0.09, 0.055),
            camera: Rect::new(l + 0.10, t, 0.14, 0.055),
            diag: Rect::new(l + 0.25, t, 0.09, 0.055),
            fps: Rect::new(1.0 - r - 0.10, t, 0.10, 0.045),
            handbrake: Rect::new(l, 0.42, col, 0.10),
            clutch: Rect::new(1.0 - r - col, 0.42, col, 0.10),
            gear_down: Rect::new(l, 0.55, col, 0.10),
            gear_up: Rect::new(1.0 - r - col, 0.55, col, 0.10),
            brake: Rect::new(l, 0.70, col, 0.26),
            throttle: Rect::new(1.0 - r - col, 0.70, col, 0.26),
            cluster: Rect::new(0.33, 1.0 - b - 0.115, 0.34, 0.10),
            telemetry: Rect::new(l, t + 0.075, 0.44, 0.31),
        }
    }
}

/// Screen layout for replay playback overlay and touch interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayLayout {
    pub top_bar: Rect,
    pub timeline: Rect,
    pub play_pause: Rect,
    pub restart: Rect,
    pub camera: Rect,
    pub exit: Rect,
}

impl ReplayLayout {
    pub fn for_size(_size: (u32, u32)) -> Self {
        Self {
            top_bar: Rect::new(0.04, 0.02, 0.92, 0.06),
            timeline: Rect::new(0.04, 0.088, 0.92, 0.016),
            play_pause: Rect::new(0.04, 0.88, 0.20, 0.08),
            restart: Rect::new(0.26, 0.88, 0.20, 0.08),
            camera: Rect::new(0.48, 0.88, 0.24, 0.08),
            exit: Rect::new(0.74, 0.88, 0.22, 0.08),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_safe_area_aligns_hud_and_touches_in_both_landscape_directions() {
        use crate::{driving::controls::Controls, harwood::benchmark::hud::Canvas};

        for (left, right) in [(59.0, 47.0), (47.0, 59.0)] {
            let window = WindowLayout {
                phone: true,
                insets: Insets::sanitize([left / 852.0, 0.0, right / 852.0, 21.0 / 393.0]),
            };
            let r = window.viewport();
            let layout = window.driving();
            assert!(!window.accepts_touch_start(0.0, 0.5));
            assert!(!window.accepts_touch_start(0.5, 0.99));
            assert!(layout.pause.bottom() <= layout.telemetry.y);
            assert!(layout.telemetry.bottom() < layout.handbrake.y);

            let (width, height) = (2556, 1179);
            let mut canvas = Canvas::in_viewport((width, height), [r.x, r.y, r.w, r.h]);
            let expected_height = 640.0 * (393.0 - 21.0) / (852.0 - left - right) as f32;
            assert!((canvas.height() - expected_height).abs() < 1e-4);
            canvas.rect(0.0, 0.0, 640.0, canvas.height(), [255; 4]);
            let bytes = canvas.finish();
            for (vertex, expected) in [(0, (r.x, r.y)), (5, (r.right(), r.bottom()))] {
                let offset = vertex * 20;
                let x = f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let y = f32::from_ne_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
                assert!(((x + 1.0) / 2.0 - expected.0).abs() < 1e-6);
                assert!(((1.0 - y) / 2.0 - expected.1).abs() < 1e-6);
            }

            let mut controls = Controls::new();
            let start = layout.engine_start_rect().unwrap();
            let tap = |button: Rect, y| {
                let x = f64::from(r.x + button.center_x() * r.w);
                let y = f64::from(r.y + y * r.h);
                assert!(window.accepts_touch_start(x, y));
                window.point(x, y)
            };
            let (x, y) = tap(layout.brake, layout.brake.bottom() - 0.01);
            controls.touch_layout(&layout, 1, x, y, true, false);
            let (x, y) = tap(start, start.y + start.h * 0.5);
            controls.touch_layout(&layout, 2, x, y, true, false);
            assert_ne!(controls.input_in(&layout, 0.0, 0.016).aux, 0);
            let (x, y) = window.point(0.0, 0.99);
            controls.touch_layout(&layout, 2, x, y, false, true);
            let input = controls.input_in(&layout, 0.0, 0.016);
            assert_eq!(input.aux, 0);
            assert!(input.brake > 0.95);
        }
    }

    #[test]
    fn ipad_keeps_the_full_canvas_and_original_driving_insets() {
        let insets = Insets::sanitize([0.03, 0.02, 0.04, 0.03]);
        let window = WindowLayout {
            insets,
            phone: false,
        };
        assert_eq!(window.viewport(), Rect::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(window.point(0.27, 0.81), (0.27, 0.81));
        let layout = window.driving();
        let previous = DrivingLayout::with_insets(insets);
        assert_eq!(layout.pause, previous.pause);
        assert_eq!(layout.cluster, previous.cluster);
        assert_eq!(layout.telemetry, previous.telemetry);
        assert_eq!(layout.engine_start_rect(), previous.engine_start_rect());
        for control in EditableControl::ALL {
            assert_eq!(layout.control_rect(control), previous.control_rect(control));
        }
    }

    #[test]
    fn moved_and_resized_controls_restore_the_same_hit_regions() {
        use crate::driving::controls::Controls;
        for ctrl in EditableControl::ALL {
            let base = DrivingLayout::with_insets(Insets {
                left: 0.05,
                right: 0.03,
                ..Insets::ZERO
            });
            let mut overrides = ControlsLayoutOverrides::default();
            overrides.set_center(ctrl, base, 0.5, 0.6);
            overrides.resize(ctrl, base, 0.04, 0.02);
            let restored = ControlsLayoutOverrides::parse(&overrides.serialize());
            let layout = restored.apply(base);
            let r = layout.control_rect(ctrl);
            let a = overrides.rects[ctrl as usize].unwrap();
            let b = restored.rects[ctrl as usize].unwrap();
            assert!(
                (a.x - b.x).abs() < 1e-4
                    && (a.y - b.y).abs() < 1e-4
                    && (a.w - b.w).abs() < 1e-4
                    && (a.h - b.h).abs() < 1e-4,
                "restored rect {b:?} diverged from {a:?}"
            );
            assert_eq!(
                ControlsLayoutOverrides::parse(&restored.serialize()),
                restored
            );
            let mut controls = Controls::new();
            controls.touch_layout(
                &layout,
                1,
                f64::from(r.center_x()),
                f64::from(r.y + r.h * 0.5),
                true,
                false,
            );
            let levels = controls.levels_in(&layout);
            match ctrl {
                EditableControl::GearDown => assert_eq!(controls.selected_gear, 0),
                EditableControl::GearUp => assert_eq!(controls.selected_gear, 2),
                EditableControl::Handbrake => assert_eq!(levels.handbrake, 1.0),
                EditableControl::Clutch => assert_eq!(levels.clutch, 1.0),
                EditableControl::Brake => assert!(levels.brake > 0.0),
                EditableControl::Throttle => assert!(levels.throttle > 0.0),
            }
            controls.touch_layout(&layout, 1, 0.0, 0.0, false, true);
            assert_eq!(controls.levels_in(&layout).throttle, 0.0);
            overrides.reset();
            assert_eq!(overrides.rects, [None; 6]);
        }
        assert!(ControlsLayoutOverrides::parse("brake=1,bad,2,3,4")
            .rects
            .iter()
            .all(Option::is_none));
    }

    #[test]
    fn opacity_overrides_persist_and_clamp_between_ten_and_one_hundred_percent() {
        let mut ov = ControlsLayoutOverrides::default();
        assert_eq!(ov.global_opacity, 0.50);
        assert_eq!(ov.opacity_for(EditableControl::Brake), 0.50);

        // Adjust global opacity and clamp
        ov.adjust_global_opacity(0.60);
        assert_eq!(ov.global_opacity, 1.00);
        ov.adjust_global_opacity(-1.50);
        assert_eq!(ov.global_opacity, 0.10);

        // Individual opacity override
        ov.set_global_opacity(0.40);
        ov.set_opacity(EditableControl::Throttle, 0.85);
        ov.set_opacity(EditableControl::Brake, 0.05); // clamps to 0.10
        assert_eq!(ov.opacity_for(EditableControl::Throttle), 0.85);
        assert_eq!(ov.opacity_for(EditableControl::Brake), 0.10);
        assert_eq!(ov.opacity_for(EditableControl::Handbrake), 0.40);

        // Serialize and restore
        let serialized = ov.serialize();
        let restored = ControlsLayoutOverrides::parse(&serialized);
        assert_eq!(restored.global_opacity, 0.40);
        assert_eq!(restored.opacity_for(EditableControl::Throttle), 0.85);
        assert_eq!(restored.opacity_for(EditableControl::Brake), 0.10);
        assert_eq!(restored.opacity_for(EditableControl::Handbrake), 0.40);

        // Reset restores 50% default
        ov.reset();
        assert_eq!(ov.global_opacity, 0.50);
        assert_eq!(ov.opacity_for(EditableControl::Throttle), 0.50);
    }

    #[test]
    fn legacy_side_targets_remain_reachable() {
        let layout = DrivingLayout::default();
        for (x, y, rect) in [
            (0.1, 0.63, layout.gear_down),
            (0.9, 0.63, layout.gear_up),
            (0.1, 0.49, layout.handbrake),
            (0.9, 0.49, layout.clutch),
            (0.1, 0.95, layout.brake),
            (0.9, 0.95, layout.throttle),
        ] {
            assert!(rect.contains(x, y), "{rect:?} misses {x},{y}");
        }
        assert!(!layout.gear_down.contains(0.5, 0.63));
    }

    #[test]
    fn insets_are_bounded_and_never_nan() {
        let i = Insets::sanitize([f64::NAN, -1.0, 0.04, 9.0]);
        assert_eq!((i.left, i.top, i.right), (0.0, 0.0, 0.04));
        assert_eq!(i.bottom, 0.12);
        let shifted = DrivingLayout::with_insets(Insets {
            left: 0.04,
            top: 0.03,
            right: 0.04,
            bottom: 0.03,
        });
        assert!(shifted.pause.x >= 0.04);
        assert!(shifted.throttle.right() <= 1.0 - 0.04 + 1e-6);
        assert!(shifted.cluster.bottom() <= 0.97);
    }
}
