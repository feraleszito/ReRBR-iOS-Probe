//! Keep the native audio ABI linkable in every pre-existing non-driving mode.
#[no_mangle]
pub extern "C" fn rerbr_audio_prepare() -> i32 {
    0
}

/// # Safety
/// A non-null pointer must reference `frames * 2` writable float samples.
#[no_mangle]
pub unsafe extern "C" fn rerbr_audio_render(out: *mut f32, frames: u32, _rate: u32) {
    if !out.is_null() {
        std::slice::from_raw_parts_mut(out, frames as usize * 2).fill(0.0);
    }
}

#[no_mangle]
pub extern "C" fn rerbr_audio_set_paused(_paused: i32) {}
#[no_mangle]
pub extern "C" fn rerbr_audio_set_interrupted(_interrupted: i32) {}
#[no_mangle]
pub extern "C" fn rerbr_audio_set_volume(_volume: f32) {}
