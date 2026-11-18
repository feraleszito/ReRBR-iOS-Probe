//! Presentation clock, free countdown start, jump-start penalty, and race timing.
use std::time::{Duration, Instant};

pub const PRESENTATION_SECS: f64 = 3.5;
pub const COUNTDOWN_SECS: f64 = 5.0;
pub const FALSE_START_THRESHOLD_METERS: f32 = 0.40;
pub const FALSE_START_PENALTY_SECS: u64 = 10;
/// Distance (m) past the `.dls` finish trigger so the entire vehicle clears the visible finish line before stopping.
pub const FINISH_ROLLOUT_METERS: f32 = 3.0;
/// Safety cap (s) for the visual finish rollout if crossing speed is very low or terrain ends early.
pub const FINISH_ROLLOUT_MAX_SECS: f64 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Preparing,
    Presentation,
    Countdown,
    Driving,
    Finishing,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageResult {
    pub driving_seconds: f64,
    pub penalty_seconds: f64,
    pub total_seconds: f64,
    pub false_start: bool,
    pub splits_passed: usize,
    pub splits_total: usize,
}

#[derive(Default)]
pub struct Clock {
    pub phase: Phase,
    elapsed: Duration,
    last: Option<Instant>,
    go_until: Option<Instant>,
    pub false_start: bool,
    pub penalty: Duration,
    start_pos: Option<[f32; 3]>,
    start_forward: Option<[f32; 3]>,
    false_start_notice_until: Option<Instant>,
    finish_rollout_target_dist: Option<f32>,
    finish_rollout_started: Option<Instant>,
    finish_rollout_until: Option<Instant>,
    result: Option<StageResult>,
}

impl Clock {
    pub fn prepare(&mut self) {
        *self = Self {
            phase: Phase::Preparing,
            ..Self::default()
        };
    }

    /// Begins the cinematic presentation sequence around the car on the starting line.
    pub fn start_presentation(&mut self, now: Instant) {
        self.phase = Phase::Presentation;
        self.elapsed = Duration::ZERO;
        self.last = Some(now);
        self.false_start = false;
        self.penalty = Duration::ZERO;
        self.start_pos = None;
        self.start_forward = None;
        self.false_start_notice_until = None;
        self.finish_rollout_target_dist = None;
        self.finish_rollout_started = None;
        self.finish_rollout_until = None;
        self.result = None;
    }

    /// Progress of the cinematic camera from 0.0 to 1.0.
    pub fn presentation_progress(&self) -> Option<f32> {
        (self.phase == Phase::Presentation)
            .then(|| (self.elapsed.as_secs_f64() / PRESENTATION_SECS).clamp(0.0, 1.0) as f32)
    }

    /// Transitions directly into Countdown (e.g. at end of presentation or immediate ready).
    #[allow(dead_code)]
    pub fn ready(&mut self, now: Instant) {
        self.phase = Phase::Countdown;
        self.elapsed = Duration::ZERO;
        self.last = Some(now);
        self.result = None;
    }

    /// Starts Rally School driving immediately (no countdown, matching `Stage::school_start`).
    pub fn start_school_driving(&mut self, now: Instant) {
        self.phase = Phase::Driving;
        self.elapsed = Duration::ZERO;
        self.last = Some(now);
        self.go_until = Some(now + Duration::from_millis(900));
        self.false_start = false;
        self.penalty = Duration::ZERO;
        self.start_pos = None;
        self.start_forward = None;
        self.false_start_notice_until = None;
        self.finish_rollout_target_dist = None;
        self.finish_rollout_started = None;
        self.finish_rollout_until = None;
        self.result = None;
    }

    /// Freezes the race clock when a Rally School exercise concludes (`ModeEnd::Finished` / `Retired`).
    pub fn complete_school_finish(&mut self) {
        self.phase = Phase::Finished;
        self.last = None;
        self.go_until = None;
        self.false_start_notice_until = None;
    }

    pub fn pause(&mut self) {
        self.last = None;
    }

    /// Concepto A: freezes the official race stopwatch once at the exact instant of finish crossing,
    /// while allowing the car to roll through the visible finish line (`Phase::Finishing`).
    pub fn begin_finish_rollout(
        &mut self,
        now: Instant,
        splits_passed: usize,
        splits_total: usize,
        finish_distance: f32,
    ) -> bool {
        if self.phase != Phase::Driving || self.result.is_some() {
            return false;
        }
        let driving_seconds = self.elapsed.as_secs_f64();
        let penalty_seconds = self.penalty.as_secs_f64();
        self.result = Some(StageResult {
            driving_seconds,
            penalty_seconds,
            total_seconds: driving_seconds + penalty_seconds,
            false_start: self.false_start,
            splits_passed,
            splits_total,
        });
        self.phase = Phase::Finishing;
        self.last = None;
        self.go_until = None;
        self.false_start_notice_until = None;
        self.finish_rollout_target_dist = Some(finish_distance + FINISH_ROLLOUT_METERS);
        self.finish_rollout_started = Some(now);
        self.finish_rollout_until = Some(now + Duration::from_secs_f64(FINISH_ROLLOUT_MAX_SECS));
        true
    }

    /// Elapsed seconds since crossing the official finish line (used only to keep replay timestamps advancing during rollout).
    pub fn rollout_elapsed_secs(&self, now: Instant) -> f64 {
        if self.phase == Phase::Finishing {
            self.finish_rollout_started.map_or(0.0, |start| {
                now.saturating_duration_since(start).as_secs_f64()
            })
        } else {
            0.0
        }
    }

    /// Checks whether the visual rollout has cleared the finish line or hit the safety timeout.
    pub fn rollout_complete(&self, now: Instant, driveline_distance: f32) -> bool {
        self.phase == Phase::Finishing
            && (self
                .finish_rollout_target_dist
                .is_some_and(|target| driveline_distance >= target)
                || self.finish_rollout_until.is_some_and(|until| now >= until))
    }

    /// Concepto B: finalizes the stage visually and stops the vehicle without touching the frozen official result.
    pub fn complete_finish(&mut self) -> bool {
        if self.phase != Phase::Finishing {
            return false;
        }
        self.phase = Phase::Finished;
        self.finish_rollout_target_dist = None;
        self.finish_rollout_started = None;
        self.finish_rollout_until = None;
        true
    }

    /// Test helper: freezes the stage clock and finishes immediately without rollout delay.
    #[cfg(test)]
    pub fn finish(&mut self, splits_passed: usize, splits_total: usize) -> bool {
        self.begin_finish_rollout(Instant::now(), splits_passed, splits_total, 0.0)
            && self.complete_finish()
    }

    pub fn result(&self) -> Option<StageResult> {
        self.result
    }

    /// Records the car's initial resting position and forward direction at start of countdown.
    pub fn record_start_origin(&mut self, pos: [f32; 3], forward: [f32; 3]) {
        if self.start_pos.is_none() {
            self.start_pos = Some(pos);
            self.start_forward = Some(forward);
        }
    }

    /// Checks whether the car crossed the starting line in the race direction before GO.
    pub fn check_jump_start(&mut self, now: Instant, current_pos: [f32; 3]) {
        if self.phase == Phase::Countdown && !self.false_start {
            if let (Some(p0), Some(fwd)) = (self.start_pos, self.start_forward) {
                let dx = current_pos[0] - p0[0];
                let dy = current_pos[1] - p0[1];
                let dz = current_pos[2] - p0[2];
                let forward_disp = dx * fwd[0] + dy * fwd[1] + dz * fwd[2];
                if forward_disp > FALSE_START_THRESHOLD_METERS {
                    self.false_start = true;
                    self.penalty = Duration::from_secs(FALSE_START_PENALTY_SECS);
                    self.false_start_notice_until = Some(now + Duration::from_secs(5));
                }
            }
        }
    }

    pub fn tick(&mut self, now: Instant, active: bool) {
        if matches!(self.phase, Phase::Finishing | Phase::Finished) {
            return;
        }
        if !active {
            self.pause();
            return;
        }
        let dt = self
            .last
            .replace(now)
            .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));

        if matches!(
            self.phase,
            Phase::Presentation | Phase::Countdown | Phase::Driving
        ) {
            self.elapsed += dt;
        }

        if self.phase == Phase::Presentation
            && self.elapsed >= Duration::from_secs_f64(PRESENTATION_SECS)
        {
            self.phase = Phase::Countdown;
            self.elapsed = Duration::ZERO;
        } else if self.phase == Phase::Countdown
            && self.elapsed >= Duration::from_secs_f64(COUNTDOWN_SECS)
        {
            self.phase = Phase::Driving;
            self.elapsed = Duration::ZERO;
            self.go_until = Some(now + Duration::from_millis(900));
        }
    }

    pub fn countdown(&self) -> Option<u32> {
        (self.phase == Phase::Countdown).then(|| {
            (COUNTDOWN_SECS - self.elapsed.as_secs_f64())
                .ceil()
                .clamp(1.0, 5.0) as u32
        })
    }

    /// Official elapsed race time including any jump-start penalty.
    pub fn seconds(&self) -> Option<f64> {
        match self.phase {
            Phase::Driving => Some(self.elapsed.as_secs_f64() + self.penalty.as_secs_f64()),
            Phase::Finishing | Phase::Finished => self.result.map(|r| r.total_seconds),
            _ => None,
        }
    }

    /// Pure driving time without penalty.
    #[allow(dead_code)]
    pub fn raw_seconds(&self) -> Option<f64> {
        match self.phase {
            Phase::Driving => Some(self.elapsed.as_secs_f64()),
            Phase::Finishing | Phase::Finished => self.result.map(|r| r.driving_seconds),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn penalty_seconds(&self) -> f64 {
        self.penalty.as_secs_f64()
    }

    #[allow(dead_code)]
    pub fn is_false_start(&self) -> bool {
        self.false_start
    }

    pub fn go_visible(&self, now: Instant) -> bool {
        self.go_until.is_some_and(|until| now < until)
    }

    pub fn false_start_visible(&self, now: Instant) -> bool {
        self.false_start
            && (self.phase == Phase::Countdown
                || self.false_start_notice_until.is_some_and(|u| now < u)
                || self.go_visible(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_transitions_to_countdown_then_driving() {
        let t = Instant::now();
        let mut clock = Clock::default();
        clock.prepare();
        assert_eq!(clock.phase, Phase::Preparing);

        clock.start_presentation(t);
        assert_eq!(clock.phase, Phase::Presentation);
        assert_eq!(clock.presentation_progress(), Some(0.0));

        // Advance presentation halfway (1.75s)
        clock.tick(t + Duration::from_millis(1750), true);
        assert_eq!(clock.phase, Phase::Presentation);
        let prog = clock.presentation_progress().unwrap();
        assert!((prog - 0.5).abs() < 0.01);
        assert_eq!(clock.countdown(), None);
        assert_eq!(clock.seconds(), None);

        // Advance to end of presentation (3.5s)
        clock.tick(t + Duration::from_millis(3500), true);
        assert_eq!(clock.phase, Phase::Countdown);
        assert_eq!(clock.presentation_progress(), None);
        assert_eq!(clock.countdown(), Some(5));

        // Advance countdown 5s
        let t_count = t + Duration::from_millis(3500);
        for i in 1..=4 {
            clock.tick(t_count + Duration::from_secs(i), true);
            assert_eq!(clock.countdown(), Some(5 - i as u32));
            assert_eq!(clock.seconds(), None);
        }

        // Hit GO at 5s
        clock.tick(t_count + Duration::from_secs(5), true);
        assert_eq!(clock.phase, Phase::Driving);
        assert_eq!(clock.countdown(), None);
        assert_eq!(clock.seconds(), Some(0.0));
        assert!(clock.go_visible(t_count + Duration::from_secs(5)));
    }

    #[test]
    fn jump_start_applies_single_ten_second_penalty_to_final_time() {
        let t = Instant::now();
        let mut clock = Clock::default();
        clock.ready(t);
        clock.record_start_origin([100.0, 10.0, 200.0], [0.0, 0.0, 1.0]);

        // Car settles slightly (< 0.40m): no false start
        clock.check_jump_start(t, [100.02, 10.0, 200.05]);
        assert!(!clock.is_false_start());
        assert_eq!(clock.penalty_seconds(), 0.0);

        // Lateral displacement: no false start
        clock.check_jump_start(t, [101.5, 10.0, 200.1]);
        assert!(!clock.is_false_start());

        // Car drives forward > 0.40m before GO: false start!
        clock.check_jump_start(t + Duration::from_secs(2), [100.0, 10.0, 200.55]);
        assert!(clock.is_false_start());
        assert_eq!(clock.penalty_seconds(), 10.0);
        assert!(clock.false_start_visible(t + Duration::from_secs(2)));

        // Subsequent motion does not add another penalty
        clock.check_jump_start(t + Duration::from_secs(3), [100.0, 10.0, 205.0]);
        assert_eq!(clock.penalty_seconds(), 10.0);

        // Advance to GO
        clock.tick(t + Duration::from_secs(5), true);
        assert_eq!(clock.phase, Phase::Driving);

        // Official time includes the 10s penalty
        assert_eq!(clock.raw_seconds(), Some(0.0));
        assert_eq!(clock.seconds(), Some(10.0));

        // Drive for 15 seconds
        clock.tick(t + Duration::from_secs(20), true);
        assert_eq!(clock.raw_seconds(), Some(15.0));
        assert_eq!(clock.seconds(), Some(25.0));

        // Pause and resume preserves penalty
        clock.pause();
        clock.tick(t + Duration::from_secs(100), true);
        assert_eq!(clock.seconds(), Some(25.0));
        clock.tick(t + Duration::from_secs(102), true);
        assert_eq!(clock.seconds(), Some(27.0));

        // Prepare clears penalty
        clock.prepare();
        assert!(!clock.is_false_start());
        assert_eq!(clock.penalty_seconds(), 0.0);
        assert_eq!(clock.result(), None);
    }

    #[test]
    fn finish_freezes_time_once_preserves_penalty_and_resets_on_prepare() {
        let t = Instant::now();
        let mut clock = Clock::default();
        clock.ready(t);
        assert!(!clock.finish(2, 2));

        clock.record_start_origin([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        clock.check_jump_start(t + Duration::from_secs(1), [0.0, 0.0, 1.0]);
        clock.tick(t + Duration::from_secs(5), true);
        assert_eq!(clock.phase, Phase::Driving);

        clock.tick(t + Duration::from_millis(68400), true);
        assert!(clock.finish(2, 2));
        assert!(!clock.finish(2, 2));
        assert_eq!(clock.phase, Phase::Finished);

        let res = clock.result().expect("stage result must be stored");
        assert!((res.driving_seconds - 63.4).abs() < 1e-6);
        assert!((res.penalty_seconds - 10.0).abs() < 1e-6);
        assert!((res.total_seconds - 73.4).abs() < 1e-6);
        assert!(res.false_start);
        assert_eq!(res.splits_passed, 2);
        assert_eq!(res.splits_total, 2);

        // Further ticks or pauses must not alter the frozen final time.
        clock.tick(t + Duration::from_secs(200), true);
        clock.pause();
        assert_eq!(clock.result(), Some(res));
        assert_eq!(clock.seconds(), Some(res.total_seconds));
        assert_eq!(clock.raw_seconds(), Some(res.driving_seconds));

        clock.prepare();
        assert_eq!(clock.phase, Phase::Preparing);
        assert_eq!(clock.result(), None);
        assert_eq!(clock.seconds(), None);
    }

    #[test]
    fn finish_rollout_freezes_official_time_at_crossing_and_completes_by_distance_or_timeout() {
        let t = Instant::now();
        let mut clock = Clock::default();
        clock.ready(t);
        clock.tick(t + Duration::from_secs(5), true);
        assert_eq!(clock.phase, Phase::Driving);

        let t_cross = t + Duration::from_millis(65400);
        clock.tick(t_cross, true);
        assert!(clock.begin_finish_rollout(t_cross, 2, 2, 1000.0));
        // Duplicate begin calls are ignored.
        assert!(!clock.begin_finish_rollout(t_cross, 2, 2, 1000.0));
        assert_eq!(clock.phase, Phase::Finishing);

        let frozen = clock
            .result()
            .expect("official time must freeze immediately");
        assert!((frozen.driving_seconds - 60.4).abs() < 1e-6);

        // At 0.5m past finish and +50ms: rollout continues without altering official time.
        assert!(!clock.rollout_complete(t_cross + Duration::from_millis(50), 1000.5));
        clock.tick(t_cross + Duration::from_millis(200), true);
        assert_eq!(clock.seconds(), Some(frozen.total_seconds));

        // Advancing >= 3.0m beyond finish triggers Concepto B completion.
        assert!(clock.rollout_complete(t_cross + Duration::from_millis(120), 1003.0));
        assert!(clock.complete_finish());
        assert_eq!(clock.phase, Phase::Finished);
        assert_eq!(clock.result(), Some(frozen));

        // Safety timeout path also completes rollout if car crossed slowly.
        let mut slow = Clock::default();
        slow.ready(t);
        slow.tick(t + Duration::from_secs(5), true);
        slow.tick(t_cross, true);
        assert!(slow.begin_finish_rollout(t_cross, 2, 2, 1000.0));
        assert!(!slow.rollout_complete(t_cross + Duration::from_millis(100), 1000.2));
        assert!(slow.rollout_complete(t_cross + Duration::from_millis(450), 1000.5));
        assert!(slow.complete_finish());
        assert_eq!(slow.phase, Phase::Finished);
    }
}
