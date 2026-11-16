use rerbr_sim::drive::Control;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Pause,
    Calibrate,
    Restart,
    SensDown,
    SensUp,
    DeadDown,
    DeadUp,
}
pub struct Controls {
    touches: BTreeMap<u64, (f64, f64)>,
    pub centre: f64,
    pub sensitivity: f64,
    pub dead_zone: f64,
    filtered: f64,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            touches: BTreeMap::new(),
            centre: 0.0,
            sensitivity: 2.0,
            dead_zone: 0.03,
            filtered: 0.0,
        }
    }
}
impl Controls {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        self.touches.clear();
        self.filtered = 0.0;
    }
    pub fn touch(&mut self, id: u64, x: f64, y: f64, started: bool, ended: bool) -> Option<Action> {
        if ended {
            self.touches.remove(&id);
            return None;
        }
        if started && y < 0.15 {
            return Some(match (x * 5.0).floor() as u32 {
                0 => Action::Pause,
                1 => Action::Calibrate,
                2 => Action::Restart,
                3 => Action::SensDown,
                _ => Action::SensUp,
            });
        }
        if started && (0.15..0.28).contains(&y) && x > 0.65 {
            return Some(if x < 0.82 {
                Action::DeadDown
            } else {
                Action::DeadUp
            });
        }
        // Only touches that began in a pedal area can actuate pedals.
        if started && y >= 0.62 {
            self.touches.insert(id, (x, y));
        }
        if let Some(p) = self.touches.get_mut(&id) {
            p.1 = y;
        }
        None
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
            flags: 4,
            gear: Some(2),
            ..Default::default()
        };
        for &(x, y) in self.touches.values() {
            let pedal = ((y - 0.62) / 0.30).clamp(0.0, 1.0) as f32;
            if x < 0.42 {
                c.brake = c.brake.max(pedal);
            } else if x > 0.58 {
                c.throttle = c.throttle.max(pedal);
            } else {
                c.handbrake = 1.0;
            }
        }
        c
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_pedals_cancel_and_ui_isolation() {
        let mut c = Controls::new();
        c.touch(1, 0.1, 0.95, true, false);
        c.touch(2, 0.9, 0.95, true, false);
        let i = c.input(0.0, 0.02);
        assert_eq!((i.brake, i.throttle), (1.0, 1.0));
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
        c.touch(5, 0.5, 0.95, true, false);
        assert_eq!(c.input(0.0, 0.02).handbrake, 1.0);
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
