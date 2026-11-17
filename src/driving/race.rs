//! Presentation clock; the original simulation remains paused until GO.
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Preparing,
    Countdown,
    Driving,
}

#[derive(Default)]
pub struct Clock {
    pub phase: Phase,
    elapsed: Duration,
    last: Option<Instant>,
    go_until: Option<Instant>,
}
impl Clock {
    pub fn prepare(&mut self) {
        *self = Self {
            phase: Phase::Preparing,
            ..Self::default()
        };
    }
    pub fn ready(&mut self, now: Instant) {
        self.phase = Phase::Countdown;
        self.elapsed = Duration::ZERO;
        self.last = Some(now);
    }
    pub fn pause(&mut self) {
        self.last = None;
    }
    pub fn tick(&mut self, now: Instant, active: bool) {
        if !active {
            self.pause();
            return;
        }
        let dt = self
            .last
            .replace(now)
            .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
        if matches!(self.phase, Phase::Countdown | Phase::Driving) {
            self.elapsed += dt;
        }
        if self.phase == Phase::Countdown && self.elapsed >= Duration::from_secs(5) {
            self.phase = Phase::Driving;
            self.elapsed = Duration::ZERO;
            self.go_until = Some(now + Duration::from_millis(900));
        }
    }
    pub fn countdown(&self) -> Option<u32> {
        (self.phase == Phase::Countdown)
            .then(|| (5.0 - self.elapsed.as_secs_f64()).ceil().clamp(1.0, 5.0) as u32)
    }
    pub fn seconds(&self) -> Option<f64> {
        (self.phase == Phase::Driving).then_some(self.elapsed.as_secs_f64())
    }
    pub fn go_visible(&self, now: Instant) -> bool {
        self.go_until.is_some_and(|until| now < until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn countdown_go_and_timer_exclude_paused_intervals() {
        let t = Instant::now();
        let mut clock = Clock::default();
        clock.prepare();
        assert_eq!(clock.phase, Phase::Preparing);
        clock.ready(t);
        for i in 0..5 {
            clock.tick(t + Duration::from_secs(i), true);
            assert_eq!(clock.countdown(), Some(5 - i as u32));
            assert_eq!(clock.seconds(), None);
        }
        clock.pause();
        clock.tick(t + Duration::from_secs(100), true);
        assert_eq!(clock.countdown(), Some(1));
        clock.tick(t + Duration::from_secs(101), true);
        assert_eq!(clock.seconds(), Some(0.0));
        assert!(clock.go_visible(t + Duration::from_secs(101)));
        clock.tick(t + Duration::from_secs(103), true);
        assert_eq!(clock.seconds(), Some(2.0));
        clock.tick(t + Duration::from_secs(104), false);
        clock.tick(t + Duration::from_secs(200), true);
        assert_eq!(clock.seconds(), Some(2.0));
        clock.tick(t + Duration::from_secs(201), true);
        assert_eq!(clock.seconds(), Some(3.0));
        clock.prepare();
        assert_eq!(clock.seconds(), None);
    }
}
