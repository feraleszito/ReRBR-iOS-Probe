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
    /// Reads UIKit safe-area insets. Non-iOS builds and unavailable views use zero.
    pub fn query() -> Self {
        #[cfg(target_os = "ios")]
        {
            unsafe extern "C" {
                fn rerbr_drive_safe_area(out: *mut f64);
            }
            let mut values = [0.0; 4];
            unsafe { rerbr_drive_safe_area(values.as_mut_ptr()) };
            return Self::sanitize(values);
        }
        #[cfg(not(target_os = "ios"))]
        Self::ZERO
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
    pub fn for_size(_size: (u32, u32)) -> Self {
        Self::with_insets(Insets::query())
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

#[cfg(test)]
mod tests {
    use super::*;

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
