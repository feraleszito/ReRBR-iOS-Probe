//! Monotonic wall-clock session. Submission cadence is not GPU execution time.
use std::{collections::VecDeque, fmt};

pub const DURATION: f64 = 20.0;
pub const WARMUP: f64 = 2.0;
const MAX_FRAMES: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Warmup,
    Running,
    Complete,
    Aborted,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub frames: usize,
    pub duration: f64,
    pub average_fps: f64,
    pub min_window_fps: f64,
    pub max_window_fps: f64,
    pub mean_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub cpu_mean_ms: f64,
    pub cpu_p95_ms: f64,
}
impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,"frames={}, real duration={:.6}s, average FPS={:.3}, full 1s windows min/max={:.1}/{:.1}, cadence mean/p95/p99={:.3}/{:.3}/{:.3}ms, CPU preparation mean/p95={:.3}/{:.3}ms; GPU time UNAVAILABLE (no timestamp queries); present requests, not confirmed scanout; FIFO and HUD included",self.frames,self.duration,self.average_fps,self.min_window_fps,self.max_window_fps,self.mean_ms,self.p95_ms,self.p99_ms,self.cpu_mean_ms,self.cpu_p95_ms)
    }
}

pub struct Session {
    state: State,
    warmup_since: f64,
    start: f64,
    last_completed: f64,
    elapsed: f64,
    intervals: Vec<f64>,
    cpu: Vec<f64>,
    windows: [u64; 20],
    recent: VecDeque<f64>,
    summary: Option<Summary>,
}
impl Session {
    pub fn new(now: f64) -> Self {
        Self {
            state: State::Warmup,
            warmup_since: now,
            start: 0.0,
            last_completed: 0.0,
            elapsed: 0.0,
            intervals: Vec::with_capacity(4096),
            cpu: Vec::with_capacity(4096),
            windows: [0; 20],
            recent: VecDeque::new(),
            summary: None,
        }
    }
    pub fn restart(&mut self, now: f64) {
        *self = Self::new(now);
    }
    pub fn abort(&mut self) {
        self.state = State::Aborted;
        self.summary = None;
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }
    pub fn summary(&self) -> Option<&Summary> {
        self.summary.as_ref()
    }
    pub fn distance(&self, length: f32) -> f32 {
        length * (self.elapsed / DURATION).clamp(0.0, 1.0) as f32
    }

    /// Called after drawable acquisition, just before preparing a real scene frame.
    pub fn begin_frame(&mut self, now: f64) {
        if !now.is_finite() || now < self.warmup_since {
            self.abort();
            return;
        }
        if self.state == State::Warmup && now - self.warmup_since >= WARMUP {
            self.state = State::Running;
            self.start = now;
            self.last_completed = now;
        }
        if self.state == State::Running {
            if now < self.last_completed {
                self.abort();
                return;
            }
            self.elapsed = now - self.start;
            while self.recent.front().is_some_and(|&t| t <= now - 1.0) {
                self.recent.pop_front();
            }
        }
    }

    /// Called after queue.submit and the present request return, never from a GPU callback.
    pub fn submitted(&mut self, now: f64, cpu_seconds: f64) -> bool {
        if self.state != State::Running {
            return false;
        }
        if !now.is_finite()
            || now <= self.last_completed
            || !cpu_seconds.is_finite()
            || cpu_seconds < 0.0
            || self.intervals.len() >= MAX_FRAMES
        {
            self.abort();
            return false;
        }
        let dt = now - self.last_completed;
        self.last_completed = now;
        self.elapsed = now - self.start;
        self.intervals.push(dt);
        self.cpu.push(cpu_seconds);
        let bucket = (self.elapsed.ceil() as usize).saturating_sub(1);
        if bucket < self.windows.len() {
            self.windows[bucket] += 1;
        }
        self.recent.push_back(now);
        while self.recent.front().is_some_and(|&t| t <= now - 1.0) {
            self.recent.pop_front();
        }
        if self.elapsed >= DURATION {
            let frames = self.intervals.len();
            let mean_ms = self.intervals.iter().sum::<f64>() * 1000.0 / frames as f64;
            self.summary = Some(Summary {
                frames,
                duration: self.elapsed,
                average_fps: frames as f64 / self.elapsed,
                min_window_fps: *self.windows.iter().min().unwrap() as f64,
                max_window_fps: *self.windows.iter().max().unwrap() as f64,
                mean_ms,
                p95_ms: percentile_ms(&self.intervals, 0.95),
                p99_ms: percentile_ms(&self.intervals, 0.99),
                cpu_mean_ms: self.cpu.iter().sum::<f64>() * 1000.0 / frames as f64,
                cpu_p95_ms: percentile_ms(&self.cpu, 0.95),
            });
            self.state = State::Complete;
            return true;
        }
        false
    }
    pub fn current_fps(&self) -> f64 {
        if self.state != State::Running {
            return 0.0;
        }
        let window = self.elapsed.min(1.0);
        if window > 0.0 {
            self.recent.len() as f64 / window
        } else {
            0.0
        }
    }
    pub fn last_cpu_ms(&self) -> f64 {
        self.cpu.last().copied().unwrap_or(0.0) * 1000.0
    }
}

/// Nearest-rank percentile over all completed-frame intervals, including stalls.
pub fn percentile_ms(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (p.clamp(0.0, 1.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)] * 1000.0
}
