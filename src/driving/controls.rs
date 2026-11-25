use super::layout::DrivingLayout;
use rerbr_sim::drive::Control;
use std::collections::BTreeMap;

/// A direct selection is sent until accepted, then left to the original shift/clutch logic.
pub fn pending_selection(mut input: Control, commanded_gear: i32) -> Control {
    if input.gear == u32::try_from(commanded_gear).ok() {
        input.flags &= !4;
        input.gear = None;
    }
    input
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Levels {
    pub brake: f32,
    pub throttle: f32,
    pub handbrake: f32,
    pub clutch: f32,
}

#[derive(Clone, Copy)]
enum Pedal {
    Brake,
    Throttle,
    Handbrake,
    Clutch,
    SteerLeft,
    SteerRight,
}
fn pressure(y: f64, rect: super::layout::Rect, invert_pedals: bool) -> f32 {
    let top_margin = 0.02_f64.min(f64::from(rect.h) * 0.2);
    let top = f64::from(rect.y) + top_margin;
    let span = (f64::from(rect.h) - 0.06).max(0.05);
    let raw = if invert_pedals {
        (top + span) - y
    } else {
        y - top
    };
    let v = (raw / span).clamp(0.0, 1.0) as f32;
    if v < 0.001 {
        0.0
    } else if v > 0.999 {
        1.0
    } else {
        v
    }
}

pub struct Controls {
    touches: BTreeMap<u64, (Pedal, f64)>,
    starter_touch: Option<u64>,
    pub centre: f64,
    pub sensitivity: f64,
    pub dead_zone: f64,
    filtered: f64,
    pub selected_gear: u32,
    pub automatic: bool,
    pub invert_pedals: bool,
    pub touch_steering: bool,
    forward_gears: u32,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            touches: BTreeMap::new(),
            starter_touch: None,
            centre: 0.0,
            sensitivity: 2.0,
            dead_zone: 0.03,
            filtered: 0.0,
            selected_gear: 1,
            automatic: false,
            invert_pedals: false,
            touch_steering: cfg!(target_os = "android"),
            forward_gears: rerbr_content::VehicleDescriptor::default().forward_gears,
        }
    }
}
impl Controls {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn configure_vehicle(
        &mut self,
        vehicle: &rerbr_content::VehicleDescriptor,
    ) -> Result<(), String> {
        vehicle.validate_schema()?;
        self.clear();
        self.forward_gears = vehicle.forward_gears;
        self.selected_gear = 1;
        Ok(())
    }
    pub fn clear(&mut self) {
        self.touches.clear();
        self.starter_touch = None;
        self.filtered = 0.0;
    }
    pub fn starter_held(&self) -> bool {
        self.starter_touch.is_some()
    }
    /// Uses the default side-cluster layout; runtime code passes its safe-area layout.
    pub fn touch(&mut self, id: u64, x: f64, y: f64, started: bool, ended: bool) {
        self.touch_layout(&DrivingLayout::default(), id, x, y, started, ended);
    }
    pub fn touch_layout(
        &mut self,
        layout: &DrivingLayout,
        id: u64,
        x: f64,
        y: f64,
        started: bool,
        ended: bool,
    ) {
        if ended {
            self.touches.remove(&id);
            if self.starter_touch == Some(id) {
                self.starter_touch = None;
            }
            return;
        }
        if started {
            if layout.engine_start_rect().is_some_and(|r| r.contains(x, y)) {
                self.starter_touch.get_or_insert(id);
                return;
            }
            use super::layout::EditableControl;
            let hit = EditableControl::ALL
                .into_iter()
                .rev()
                .find(|&c| layout.control_rect(c).contains(x, y));
            let pedal = match hit {
                Some(EditableControl::GearDown) => {
                    if !self.automatic {
                        self.selected_gear = self.selected_gear.saturating_sub(1);
                    }
                    return;
                }
                Some(EditableControl::GearUp) => {
                    if !self.automatic {
                        self.selected_gear = (self.selected_gear + 1).min(self.forward_gears + 1);
                    }
                    return;
                }
                Some(EditableControl::Handbrake) => Some(Pedal::Handbrake),
                Some(EditableControl::Clutch) => Some(Pedal::Clutch),
                Some(EditableControl::Brake) => Some(Pedal::Brake),
                Some(EditableControl::Throttle) => Some(Pedal::Throttle),
                None if self.touch_steering && layout.steer_left_rect().contains(x, y) => {
                    Some(Pedal::SteerLeft)
                }
                None if self.touch_steering && layout.steer_right_rect().contains(x, y) => {
                    Some(Pedal::SteerRight)
                }
                None => None,
            };
            if let Some(pedal) = pedal {
                // The role is captured at touch start and cannot change while dragging.
                self.touches.insert(id, (pedal, y));
            }
        }
        if let Some(pedal) = self.touches.get_mut(&id) {
            pedal.1 = y;
        }
    }
    pub fn levels(&self) -> Levels {
        self.levels_in(&DrivingLayout::default())
    }
    pub fn levels_in(&self, layout: &DrivingLayout) -> Levels {
        let mut levels = Levels::default();
        for &(pedal, y) in self.touches.values() {
            match pedal {
                Pedal::Clutch => levels.clutch = 1.0,
                Pedal::Handbrake => levels.handbrake = 1.0,
                Pedal::Brake => {
                    levels.brake = levels
                        .brake
                        .max(pressure(y, layout.brake, self.invert_pedals))
                }
                Pedal::Throttle => {
                    levels.throttle =
                        levels
                            .throttle
                            .max(pressure(y, layout.throttle, self.invert_pedals))
                }
                Pedal::SteerLeft | Pedal::SteerRight => {}
            }
        }
        levels
    }
    pub fn touch_steer_axis(&self) -> f64 {
        let mut left = false;
        let mut right = false;
        for &(pedal, _) in self.touches.values() {
            match pedal {
                Pedal::SteerLeft => left = true,
                Pedal::SteerRight => right = true,
                _ => {}
            }
        }
        match (left, right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }
    pub fn input(&mut self, tilt: f64, seconds: f64) -> Control {
        self.input_in(&DrivingLayout::default(), tilt, seconds)
    }
    pub fn input_in(&mut self, layout: &DrivingLayout, tilt: f64, seconds: f64) -> Control {
        let touch_axis = self.touch_steer_axis();
        let target = if touch_axis != 0.0 {
            touch_axis
        } else {
            let d = if tilt.is_finite() {
                (tilt - self.centre + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI
            } else {
                0.0
            };
            d.signum() * ((d.abs() - self.dead_zone).max(0.0) * self.sensitivity).min(1.0)
        };
        let a = 1.0 - (-seconds.clamp(0.0, 0.1) / 0.035).exp();
        self.filtered += (target - self.filtered) * a;
        let levels = self.levels_in(layout);
        Control {
            steering: self.filtered as f32,
            flags: if self.automatic { 0 } else { 4 },
            gear: (!self.automatic).then_some(self.selected_gear),
            clutch: levels.clutch,
            handbrake: levels.handbrake,
            brake: levels.brake,
            throttle: levels.throttle,
            // NGP polls the ignition level in the low bit of Control::aux.
            aux: u32::from(self.starter_held()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_touch_holds_the_ngp_input_bit_without_capturing_a_pedal() {
        let layout = DrivingLayout::default();
        let start = layout.engine_start_rect().unwrap();
        let px = f64::from(layout.brake.center_x());
        let py = f64::from(layout.brake.bottom() - 0.01);
        let sx = f64::from(start.center_x());
        let sy = f64::from(start.y + start.h * 0.5);
        let mut controls = Controls::new();
        controls.touch_layout(&layout, 1, px, py, true, false);
        controls.touch_layout(&layout, 1, sx, sy, false, false);
        assert_eq!(controls.input_in(&layout, 0.0, 0.016).aux, 0);
        controls.touch_layout(&layout, 1, px, py, false, false);
        controls.touch_layout(&layout, 2, sx, sy, true, false);
        controls.touch_layout(&layout, 2, px, py, false, false);
        for _ in 0..65 {
            let input = controls.input_in(&layout, 0.25, 0.016);
            assert_eq!(pending_selection(input, 1).words()[7], Some(1));
            assert!(input.brake > 0.95 && input.steering > 0.0);
            assert_eq!(input.throttle, 0.0);
        }
        controls.touch_layout(&layout, 2, px, py, false, true);
        let input = controls.input_in(&layout, 0.25, 0.016);
        assert_eq!(input.aux, 0);
        assert!(input.brake > 0.95);
        controls.touch_layout(&layout, 3, sx, sy, true, false);
        controls.clear();
        assert_eq!(controls.input_in(&layout, 0.0, 0.016).words()[7], Some(0));
    }

    #[test]
    fn starter_button_avoids_a_custom_pedal_in_the_top_bar() {
        let mut layout = DrivingLayout::default();
        let first = layout.engine_start_rect().unwrap();
        layout.throttle = first;
        assert_ne!(layout.engine_start_rect(), Some(first));
        let mut controls = Controls::new();
        controls.touch_layout(
            &layout,
            1,
            f64::from(first.center_x()),
            f64::from(first.bottom() - 0.001),
            true,
            false,
        );
        assert!(!controls.starter_held());
        assert!(controls.levels_in(&layout).throttle > 0.0);
    }

    #[test]
    fn overlapping_targets_follow_draw_order_and_moved_pedals_invert_together() {
        use crate::driving::layout::{ControlsLayoutOverrides, EditableControl, Rect};
        let mut ov = ControlsLayoutOverrides::default();
        ov.rects[EditableControl::Throttle as usize] = Some(Rect::new(0.1, 0.55, 0.2, 0.2));
        let layout = ov.apply(DrivingLayout::default());
        let mut c = Controls::new();
        c.touch_layout(&layout, 1, 0.12, 0.63, true, false);
        assert_eq!(c.selected_gear, 1, "visible throttle covers gear-down");
        assert!(c.levels_in(&layout).throttle > 0.0);
        for inverted in [false, true] {
            c.clear();
            c.invert_pedals = inverted;
            let layout = DrivingLayout::default();
            for (id, rect) in [(1, layout.brake), (2, layout.throttle)] {
                let y = if inverted {
                    rect.y + 0.02
                } else {
                    rect.bottom() - 0.01
                };
                c.touch_layout(
                    &layout,
                    id,
                    f64::from(rect.center_x()),
                    f64::from(y),
                    true,
                    false,
                );
            }
            let levels = c.levels_in(&layout);
            assert!(levels.brake > 0.95 && levels.throttle > 0.95);
        }
    }

    #[test]
    fn shifts_are_edge_triggered_bounded_and_independent_of_pedals() {
        let mut c = Controls::new();
        for expected in [2, 3, 4, 5, 6, 7, 7] {
            c.touch(1, 0.9, 0.63, true, false);
            c.touch(1, 0.9, 0.63, false, false);
            let input = c.input(0.0, 0.02);
            assert_eq!(input.gear, Some(expected));
            assert_eq!(input.throttle, 0.0);
            assert_eq!(pending_selection(input, expected as i32).flags & 4, 0);
            assert_eq!(pending_selection(input, 1).gear, Some(expected));
        }
        for expected in [6, 5, 4, 3, 2, 1, 0, 0] {
            c.touch(2, 0.1, 0.63, true, false);
            assert_eq!(c.input(0.0, 0.02).gear, Some(expected));
        }
        c.touch(3, 0.9, 0.49, true, false);
        c.touch(3, 0.5, 0.95, false, false);
        assert_eq!(c.input(0.0, 0.02).clutch, 1.0);
        assert_eq!(c.input(0.0, 0.02).handbrake, 0.0);
        c.touch(3, 0.5, 0.95, false, true);
        assert_eq!(c.input(0.0, 0.02).clutch, 0.0);
        c.automatic = true;
        c.touch(1, 0.9, 0.63, true, false);
        let input = c.input(0.0, 0.02);
        assert_eq!((input.flags, input.gear), (0, None));
    }
    #[test]
    fn simultaneous_pedals_cancel_and_ui_isolation() {
        let mut c = Controls::new();
        c.touch(1, 0.1, 0.95, true, false);
        c.touch(2, 0.9, 0.95, true, false);
        let i = c.input(0.0, 0.02);
        assert_eq!((i.brake, i.throttle), (1.0, 1.0));
        assert_eq!((c.levels().brake, c.levels().throttle), (1.0, 1.0));
        c.touch(3, 0.2, 0.1, true, false);
        c.touch(3, 0.9, 0.95, false, false);
        c.touch(2, 0.9, 0.95, false, true);
        assert_eq!(c.input(0.0, 0.02).throttle, 0.0);
        c.clear();
        assert_eq!(c.input(0.0, 0.02).brake, 0.0);
        c.touch(4, 0.9, 0.95, true, false);
        c.touch(4, 0.1, 0.95, false, false);
        let captured = c.input(0.0, 0.02);
        assert_eq!((captured.throttle, captured.brake), (1.0, 0.0));
        c.touch(5, 0.1, 0.49, true, false);
        assert_eq!(c.input(0.0, 0.02).handbrake, 1.0);
    }
    #[test]
    fn top_bar_buttons_never_change_gears_or_pedals() {
        let layout = DrivingLayout::default();
        let mut c = Controls::new();
        c.touch_layout(&layout, 9, 0.05, 0.03, true, false);
        c.touch_layout(&layout, 9, 0.05, 0.03, false, true);
        assert_eq!(c.input(0.0, 0.02).gear, Some(1));
        assert_eq!(c.levels().brake, 0.0);
        assert_eq!(c.levels().throttle, 0.0);
    }
    #[test]
    fn tilt_dead_zone_calibration_and_response() {
        let mut c = Controls::new();
        c.centre = 0.2;
        assert_eq!(c.input(0.21, 0.02).steering, 0.0);
        assert!(c.input(0.5, 0.05).steering > 0.3);
        c.clear();
        assert!(c.input(-0.1, 0.05).steering < -0.3);
        c.clear();
        c.centre = std::f64::consts::PI - 0.01;
        assert_eq!(c.input(-std::f64::consts::PI + 0.01, 0.02).steering, 0.0);
    }
    #[test]
    fn inverted_pedals_reverse_throttle_and_brake_together() {
        let mut c = Controls::new();
        assert!(!c.invert_pedals);
        // Normal mode: down (y=0.92) is 100%, up (y=0.72) is 0%
        c.touch(1, 0.1, 0.92, true, false);
        c.touch(2, 0.9, 0.72, true, false);
        let normal = c.input(0.0, 0.02);
        assert_eq!((normal.brake, normal.throttle), (1.0, 0.0));

        // Inverted mode: up (y=0.72) is 100%, down (y=0.92) is 0%
        c.invert_pedals = true;
        let inverted = c.input(0.0, 0.02);
        assert_eq!((inverted.brake, inverted.throttle), (0.0, 1.0));
        let lv = c.levels();
        assert_eq!((lv.brake, lv.throttle), (0.0, 1.0));
    }
    #[test]
    fn touch_steering_buttons_steer_when_tilt_is_nan_without_blocking_pedals() {
        let layout = DrivingLayout::default();
        let mut c = Controls::new();
        c.touch_steering = true;
        let sl = layout.steer_left_rect();
        let sr = layout.steer_right_rect();
        assert!(sl.right() < layout.cluster.x);
        assert!(sr.x > layout.cluster.right());
        assert!(sl.x > layout.brake.right());
        assert!(sr.right() < layout.throttle.x);

        // Hold throttle + steer left with NaN sensor tilt (e.g. BrowserStack flat device)
        c.touch_layout(
            &layout,
            1,
            f64::from(layout.throttle.center_x()),
            f64::from(layout.throttle.bottom() - 0.01),
            true,
            false,
        );
        c.touch_layout(
            &layout,
            2,
            f64::from(sl.center_x()),
            f64::from(sl.y + sl.h * 0.5),
            true,
            false,
        );
        for _ in 0..8 {
            let inp = c.input_in(&layout, f64::NAN, 0.02);
            assert!(inp.throttle > 0.95);
        }
        assert!(c.input_in(&layout, f64::NAN, 0.02).steering < -0.8);

        // Release steer left and press steer right
        c.touch_layout(&layout, 2, 0.0, 0.0, false, true);
        c.touch_layout(
            &layout,
            3,
            f64::from(sr.center_x()),
            f64::from(sr.y + sr.h * 0.5),
            true,
            false,
        );
        for _ in 0..12 {
            let _ = c.input_in(&layout, f64::NAN, 0.02);
        }
        assert!(c.input_in(&layout, f64::NAN, 0.02).steering > 0.8);
    }
}
