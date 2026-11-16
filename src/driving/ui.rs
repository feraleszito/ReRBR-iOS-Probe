//! Small shared drawing helpers for menus, HUD and controls.
use super::layout::Rect;
use crate::harwood::benchmark::hud::Canvas;

pub const TEXT: [u8; 4] = [236, 243, 249, 255];
pub const DIM: [u8; 4] = [128, 139, 152, 255];
pub const ACCENT: [u8; 4] = [240, 184, 80, 255];
pub const PANEL: [u8; 4] = [6, 10, 16, 214];
pub const PANEL_DEEP: [u8; 4] = [3, 5, 9, 236];
pub const PANEL_ACTIVE: [u8; 4] = [20, 34, 50, 226];
pub const LINE: [u8; 4] = [64, 82, 102, 255];
pub const GOOD: [u8; 4] = [104, 214, 154, 255];
pub const WARN: [u8; 4] = [242, 202, 96, 255];
pub const BAD: [u8; 4] = [236, 112, 112, 255];
pub const INFO: [u8; 4] = [118, 198, 236, 255];

pub const BACKDROP: [u8; 4] = [3, 5, 8, 248];

pub fn px(r: Rect, canvas: &Canvas) -> (f32, f32, f32, f32) {
    (
        r.x * 640.0,
        r.y * canvas.height(),
        r.w * 640.0,
        r.h * canvas.height(),
    )
}

pub fn fill(canvas: &mut Canvas, r: Rect, color: [u8; 4]) {
    let (x, y, w, h) = px(r, canvas);
    canvas.rect(x, y, w, h, color);
}

pub fn frame(canvas: &mut Canvas, r: Rect, color: [u8; 4], thickness: f32) {
    let (x, y, w, h) = px(r, canvas);
    let t = thickness.max(1.0);
    canvas.rect(x, y, w, t, color);
    canvas.rect(x, y + h - t, w, t, color);
    canvas.rect(x, y, t, h, color);
    canvas.rect(x + w - t, y, t, h, color);
}

pub fn panel(canvas: &mut Canvas, r: Rect, fill_color: [u8; 4], border: [u8; 4]) {
    fill(canvas, r, fill_color);
    frame(canvas, r, border, 1.5);
}

pub fn texture_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * 14.0 * scale
}

pub fn text_center(canvas: &mut Canvas, r: Rect, value: &str, scale: f32, color: [u8; 4]) {
    let (x, y, w, h) = px(r, canvas);
    let tw = texture_width(value, scale);
    canvas.text(
        x + (w - tw).max(0.0) * 0.5,
        y + (h - 16.0 * scale).max(0.0) * 0.5,
        value,
        scale,
        color,
    );
}

pub fn text_right(canvas: &mut Canvas, r: Rect, value: &str, scale: f32, pad: f32, color: [u8; 4]) {
    let (x, y, w, h) = px(r, canvas);
    let tw = texture_width(value, scale);
    canvas.text(
        x + w - tw - pad * 640.0,
        y + (h - 16.0 * scale).max(0.0) * 0.5,
        value,
        scale,
        color,
    );
}

/// Draws as many characters as fit, ending with `..` when clipped.
pub fn text_clipped(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    max_width: f32,
    value: &str,
    scale: f32,
    color: [u8; 4],
) {
    let advance = 14.0 * scale;
    let max = (max_width / advance).floor().max(1.0) as usize;
    let upper = value.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() <= max {
        canvas.text(x, y, &upper, scale, color);
        return;
    }
    let keep = max.saturating_sub(2);
    let mut clipped = String::from_utf8_lossy(&bytes[..keep.min(bytes.len())]).into_owned();
    while clipped.len() < keep {
        clipped.push(' ');
    }
    clipped.push_str("..");
    canvas.text(x, y, &clipped, scale, color);
}

pub fn button(canvas: &mut Canvas, r: Rect, label: &str, active: bool, enabled: bool) {
    let (fill_color, border, text_color) = match (enabled, active) {
        (false, _) => (PANEL_DEEP, LINE, DIM),
        (true, true) => (PANEL_ACTIVE, ACCENT, TEXT),
        (true, false) => (PANEL, LINE, TEXT),
    };
    panel(canvas, r, fill_color, border);
    let (_, _, width, _) = px(r, canvas);
    let scale = 0.85;
    let needed = texture_width(label, scale);
    let fit = (width - 10.0).max(1.0);
    let scale = if needed > fit {
        (scale * fit / needed).clamp(0.45, scale)
    } else {
        scale
    };
    text_center(canvas, r, label, scale, text_color);
}

/// Word-wraps into uppercase ASCII lines of at most `max_chars` characters.
pub fn wrap(value: &str, max_chars: usize) -> Vec<String> {
    let max = max_chars.max(4);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.to_ascii_uppercase().split_whitespace() {
        if word.len() > max {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let bytes = word.as_bytes();
            for chunk in bytes.chunks(max) {
                lines.push(String::from_utf8_lossy(chunk).into_owned());
            }
            continue;
        }
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn rule(canvas: &mut Canvas, x: f32, y: f32, w: f32, color: [u8; 4]) {
    let line = Rect::new(x, y, w, 1.5 / canvas.height());
    fill(canvas, line, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harwood::benchmark::hud::MAX_CANVAS_VERTICES;

    #[test]
    fn canvas_stays_within_the_shared_vertex_budget() {
        let mut canvas = Canvas::new((1024, 768));
        for i in 0..24 {
            button(
                &mut canvas,
                Rect::new(0.02, 0.02 + i as f32 * 0.03, 0.5, 0.025),
                "TOYOTA COROLLA WRC SELECTED",
                i % 2 == 0,
                i % 3 != 0,
            );
        }
        text_clipped(
            &mut canvas,
            4.0,
            4.0,
            200.0,
            "A LONGER STATUS LINE IS CLIPPED",
            0.8,
            TEXT,
        );
        let bytes = canvas.finish();
        assert!(!bytes.is_empty());
        assert!(bytes.len() / 20 <= MAX_CANVAS_VERTICES);
    }
}
