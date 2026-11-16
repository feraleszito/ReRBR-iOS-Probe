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
}
fn pressure(y: f64) -> f32 {
    ((y - 0.72) / 0.20).clamp(0.0, 1.0) as f32
}

pub struct Controls {
    touches: BTreeMap<u64, (Pedal, f64)>,
    pub centre: f64,
    pub sensitivity: f64,
    pub dead_zone: f64,
    filtered: f64,
    pub selected_gear: u32,
    pub automatic: bool,
    forward_gears: u32,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            touches: BTreeMap::new(),
            centre: 0.0,
            sensitivity: 2.0,
            dead_zone: 0.03,
            filtered: 0.0,
            selected_gear: 2,
            automatic: false,
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
        self.selected_gear = 2;
        Ok(())
    }
    pub fn clear(&mut self) {
        self.touches.clear();
        self.filtered = 0.0;
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
            return;
        }
        if started {
            if layout.gear_down.contains(x, y) {
                if !self.automatic {
                    self.selected_gear = self.selected_gear.saturating_sub(1);
                }
                return;
            }
            if layout.gear_up.contains(x, y) {
                if !self.automatic {
                    self.selected_gear = (self.selected_gear + 1).min(self.forward_gears + 1);
                }
                return;
            }
            let pedal = if layout.handbrake.contains(x, y) {
                Some(Pedal::Handbrake)
            } else if layout.clutch.contains(x, y) {
                Some(Pedal::Clutch)
            } else if layout.brake.contains(x, y) {
                Some(Pedal::Brake)
            } else if layout.throttle.contains(x, y) {
                Some(Pedal::Throttle)
            } else {
                None
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
        let mut levels = Levels::default();
        for &(pedal, y) in self.touches.values() {
            match pedal {
                Pedal::Clutch => levels.clutch = 1.0,
                Pedal::Handbrake => levels.handbrake = 1.0,
                Pedal::Brake => levels.brake = levels.brake.max(pressure(y)),
                Pedal::Throttle => levels.throttle = levels.throttle.max(pressure(y)),
            }
        }
        levels
    }
    pub fn input(&mut self, tilt: f64, seconds: f64) -> Control {
        let d = if tilt.is_finite() {
            (tilt - self.centre + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI
        } else {
            0.0
        };
        let target = d.signum() * ((d.abs() - self.dead_zone).max(0.0) * self.sensitivity).min(1.0);
        let a = 1.0 - (-seconds.clamp(0.0, 0.1) / 0.035).exp();
        self.filtered += (target - self.filtered) * a;
        let mut c = Control {
            steering: self.filtered as f32,
            flags: if self.automatic { 0 } else { 4 },
            gear: (!self.automatic).then_some(self.selected_gear),
            ..Default::default()
        };
        for &(pedal, y) in self.touches.values() {
            match pedal {
                Pedal::Clutch => c.clutch = 1.0,
                Pedal::Handbrake => c.handbrake = 1.0,
                Pedal::Brake => c.brake = c.brake.max(pressure(y)),
                Pedal::Throttle => c.throttle = c.throttle.max(pressure(y)),
            }
        }
        c
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifts_are_edge_triggered_bounded_and_independent_of_pedals() {
        let mut c = Controls::new();
        for expected in [3, 4, 5, 6, 7, 7] {
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
        assert_eq!(c.input(0.0, 0.02).gear, Some(2));
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
}
