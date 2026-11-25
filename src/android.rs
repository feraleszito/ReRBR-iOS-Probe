//! NativeActivity owns Java services. Winit runs the shared game on android_main.
use super::{EventLoop, Probe};
use jni_sys::{jboolean, jclass, jobject, JNIEnv};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};

static TILT: Mutex<Option<(f64, Instant)>> = Mutex::new(None);
static LAYOUT: Mutex<([f64; 4], bool)> = Mutex::new(([0.0; 4], true));

#[no_mangle]
fn android_main(app: AndroidApp) {
    if let Some(files) = app.external_data_path() {
        std::env::set_var("RERBR_PROBE_DOCUMENTS", &files);
        eprintln!("Android game resources: {}", files.join("RBR").display());
    } else {
        std::env::remove_var("RERBR_PROBE_DOCUMENTS");
        eprintln!("External app storage unavailable");
    }
    let result = EventLoop::builder()
        .with_android_app(app)
        .build()
        .and_then(|events| events.run_app(&mut Probe::default()));
    if let Err(error) = result {
        eprintln!("Android game loop: {error}");
    }
}

pub(super) fn tilt(enabled: bool) -> f64 {
    TILT.lock()
        .unwrap()
        .filter(|(_, when)| enabled && when.elapsed() < Duration::from_millis(250))
        .map_or(f64::NAN, |(angle, _)| angle)
}

pub(super) fn layout() -> ([f64; 4], bool) {
    *LAYOUT.lock().unwrap()
}

#[no_mangle]
extern "system" fn Java_org_rerbr_mobile_GameActivity_sensorRoll(
    _: *mut JNIEnv,
    _: jclass,
    angle: f64,
) {
    *TILT.lock().unwrap() = angle.is_finite().then(|| (angle, Instant::now()));
}

#[no_mangle]
extern "system" fn Java_org_rerbr_mobile_GameActivity_safeArea(
    _: *mut JNIEnv,
    _: jclass,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    phone: jboolean,
) {
    *LAYOUT.lock().unwrap() = ([left, top, right, bottom], phone != 0);
}

#[cfg(feature = "physics-drive")]
#[no_mangle]
unsafe extern "system" fn Java_org_rerbr_mobile_GameAudio_render(
    env: *mut JNIEnv,
    _: jclass,
    buffer: jobject,
) {
    // The audio thread owns this direct ByteBuffer until this synchronous call returns.
    let jni = &**env;
    let capacity = (jni.GetDirectBufferCapacity.unwrap())(env, buffer);
    if !(8..=4096 * 8).contains(&capacity) || capacity % 8 != 0 {
        return;
    }
    let data = (jni.GetDirectBufferAddress.unwrap())(env, buffer).cast::<f32>();
    if !data.is_null() && data.is_aligned() {
        crate::driving::audio::rerbr_audio_render(data, (capacity / 8) as u32, 44100);
    }
}
